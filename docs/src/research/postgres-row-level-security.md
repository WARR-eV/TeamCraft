# PostgreSQL row-level security as a second layer

Question: should PostgreSQL row-level security (RLS) back up the visibility
guards of the service layer, so that a query that forgets its guard still
returns nothing of a hidden project group? Issue #312 (and #438, Phase 0.2,
which asks for this evaluation without building it). Date: 2026-11-25.

The [read permissions note](read-permissions.md) and
[ADR 0001](../dev/adr/0001-access-model.md) turned RLS down as *the*
enforcement. This page looks at it as a *second* layer behind the guards that
exist.

## How reads are guarded today

* **The actor carries a deny list.** `Actor::hidden_projects` holds the
  restricted groups the user may not read. Part of it comes from the database
  (`access::hidden_of_user`: restricted groups without membership, the
  administrators' need-to-know policy `admin_read` and their elevations,
  identity-provider claims and the age of the last sign-in). The rest depends
  on the request and is added in Rust: the client network and the web
  session's age (`membership_conditions::apply`) and the interface switches of
  a group (`project_policy::narrow`: MCP, tokens, assistant, …).
* **One module writes the guards.** `src/services/access.rs` returns SQL
  fragments (`item_visible`, `project_visible`, `change_visible`,
  `request_visible`, `user_sees_project`) that bind the deny list, and
  `ensure_visible` checks a loaded row. A hidden object answers exactly like a
  missing one.
* **Placeholders and counts.** A BOM line whose child is hidden stays as a
  placeholder row (quantity and find number, no identity); where-used lists
  drop hidden parents and count them (`x-hidden-count`); a revision page says
  "part of N change orders you cannot see"; notifications are redacted per
  recipient. These queries read rows of hidden groups *on purpose* and give out
  only a count or a placeholder.
* **Tests.** `tests/architecture_guards.rs` fails when a service function
  reads items, revisions, files or a table that copies their content without a
  guard – for an actor, or without one for a handler (reviewed exceptions on
  `NOT_GUARDED`). `tests/read_permissions.rs` calls every web route, REST route
  and MCP tool as two outsiders and fails on any marker of the hidden group.

What this does not catch: a guard that is wrong, and a query the scanner
cannot see (a guard elsewhere in the same function, SQL built from constants).

## What PostgreSQL offers

From the PostgreSQL documentation ([Row Security Policies][rls],
[CREATE POLICY][policy], [ALTER TABLE][alter]):

* `ALTER TABLE … ENABLE ROW LEVEL SECURITY` turns it on per table. With no
  policy for a role, nothing is visible ("default deny").
* A policy has a `USING` expression (which existing rows a command sees) and a
  `WITH CHECK` expression (which new or changed rows are allowed). It applies
  to commands (`SELECT`, `INSERT`, `UPDATE`, `DELETE`, `ALL`) and roles.
* Policies are `PERMISSIVE` (combined with OR) or `RESTRICTIVE` (combined with
  AND). At least one permissive policy must allow a row; every restrictive one
  must. A "second layer" is a restrictive policy next to a permissive
  `USING (true)`.
* **Who bypasses it:** superusers and roles with `BYPASSRLS`
  ([CREATE ROLE][role]) always; the table owner unless the table has
  `FORCE ROW LEVEL SECURITY`.
* Referential integrity checks (foreign keys, unique constraints) bypass RLS;
  the documentation warns that they can be a covert channel.
* `row_security = off` ([client settings][guc]) does not bypass anything: a
  query that a policy would filter fails instead. `pg_dump` sets it by default
  ([pg_dump][dump]), so a dump run by a role that cannot bypass RLS fails
  rather than missing rows.
* Conditions of a query whose functions or operators are not `LEAKPROOF` are
  evaluated after the policy conditions ([CREATE FUNCTION][function]).
* A policy expression may contain sub-queries. The documentation shows that a
  policy that depends on rows of another table can be raced by concurrent
  changes to those rows.

## What the policies would look like

The deny list cannot be computed by the database alone: the request-dependent
parts (network, session age, interface switch) exist only in the server. The
policy would read the actor's final list from a setting the server sets per
transaction ([`set_config` / `current_setting`][setconfig], custom settings
with a dot in the name: [customized options][custom]):

```sql
CREATE FUNCTION teamcraft_hidden() RETURNS uuid[] LANGUAGE sql STABLE AS $$
    SELECT NULLIF(current_setting('teamcraft.hidden_projects', true), '')::uuid[]
$$;

ALTER TABLE items ENABLE ROW LEVEL SECURITY;
CREATE POLICY items_base ON items AS PERMISSIVE FOR ALL TO teamcraft_app
    USING (true) WITH CHECK (true);
CREATE POLICY items_visible ON items AS RESTRICTIVE FOR ALL TO teamcraft_app
    USING (project_id IS NULL OR project_id <> ALL (teamcraft_hidden()));

-- revisions, files and file_versions have no group of their own
CREATE POLICY revisions_visible ON revisions AS RESTRICTIVE FOR ALL TO teamcraft_app
    USING (EXISTS (SELECT 1 FROM items i WHERE i.id = revisions.item_id
                   AND (i.project_id IS NULL OR i.project_id <> ALL (teamcraft_hidden()))));
```

* The guard is the same as `access::item_visible`, including `IS NULL OR` for
  items without a group.
* **Fails closed:** without the setting `teamcraft_hidden()` is `NULL`,
  `project_id <> ALL (NULL)` is not true, and only items without a group are
  visible. `'{}'` (nothing hidden, an administrator without need-to-know
  groups) has to be set explicitly.
* `files` and `file_versions` need the same through one or two more joins, and
  the tables that copy item content (BOM lines, references, simulation values,
  3D notes, deviations, notifications, …) through theirs. Each policy is a
  sub-query per row; a `project_id` copied onto those tables (kept by
  triggers, also when an item moves to another group) would make them cheap.
* Change orders and requests would need `change_visible` / `request_visible`
  as policies: author, reviewers and assignee plus "every affected item
  visible" – a sub-query over all items of the change per row.

## Per-request state on a pooled connection

TeamCraft has one pool (`src/db.rs`, `TEAMCRAFT_DB_MAX_CONNECTIONS`, shared by
requests and background jobs). From [SET][set]:

* `SET` without `LOCAL` lasts for the session – the pooled connection – and
  would carry one user's deny list into the next request that gets the
  connection.
* `SET LOCAL` and `set_config(…, true)` last until the end of the transaction.
  Outside a transaction block `SET LOCAL` has no effect.

So every request would have to run its queries in one transaction that sets
`teamcraft.user_id` and `teamcraft.hidden_projects` first. Today most service
functions take `&PgPool` (about 440 signatures in `src/services`) and run each
statement on its own; about 140 places open a transaction. The services would
need one connection type that begins the transaction and sets the actor –
the only way to reach the database for a request – and long work (downloads,
preview conversion, file scans) must not hold it.

A custom setting can be changed by any SQL the connection runs. RLS built on
it therefore does not help against SQL injection (the attacker sets the list
to `'{}'`); TeamCraft binds every value and builds SQL text only from its own
fragments.

## Roles: owner, application, bypass

Today the server connects with the role that runs the migrations
(`TEAMCRAFT_DB_USER`) and owns every table. As owner it bypasses RLS. In
`teamcraft local` that role is the bootstrap superuser of `initdb`
(`--username`, [initdb][initdb]), and superusers bypass RLS even with `FORCE`.
An installation can check its role with
`SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user`.

RLS needs at least:

1. an **owner** role for the migrations (`teamcraft migrate`, the migration at
   start-up), which bypasses as owner;
2. an **application** role for requests – not the owner, no superuser, no
   `BYPASSRLS`, with `SELECT`/`INSERT`/`UPDATE`/`DELETE` granted;
3. a way to do **system work** across groups: the outbox and delivery workers,
   housekeeping, the backup (`pg_dump`), maintenance commands
   (`rescan-references`), change propagation inside a release, notification
   recipients, and the placeholders and counts above. Either a second pool with
   a `BYPASSRLS` role, or narrow `SECURITY DEFINER` functions owned by such a
   role that return only ids, groups or counts ([CREATE FUNCTION][function]).
   A "system" setting the application can switch on would be no layer at all.

`FORCE ROW LEVEL SECURITY` is only needed if the application keeps connecting
as the owner; the migrations would then need a `BYPASSRLS` role. Every
installation (Docker install, `teamcraft local`, packages) would get a second
connection setting and new documentation.

## Need-to-know and the administrators

The administrators' policy (`admin_read`: implicit, with a reason, none) and
their elevations are already part of the deny list, so a policy on that list
follows them. The groups' own rows (`projects`: names, codes, member counts,
which administrators manage even under `none`) stay outside RLS, as
management data. A policy that recomputes `hidden_of_user` from
`teamcraft.user_id` would be independent of the server's list but would miss
the request-dependent parts; requiring both is possible at the cost of one
more sub-query per statement.

## What it would protect against, and what not

| Threat                                                                  | RLS on the deny list                                                                                                             |
|-------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| A query of a request that forgets its guard (new route, tool, channel)  | **protected** for the covered tables – the case #312 is about; the guard scanner finds the same class before merge              |
| A guard that is wrong, or a wrong deny list                             | not protected: the server computes the list the policy trusts                                                                    |
| Placeholders and counts                                                 | broken unless they move to system functions: a hidden row is gone, so the placeholder disappears and the count drops to 0        |
| Code running as system (workers, propagation, notifications, backup)    | not protected; these bypass by design                                                                                            |
| SQL injection                                                           | not protected (the setting can be changed); bound parameters are the defence                                                     |
| Operator, database superuser, host, backups, vault, preview cache       | not protected (see the [threat model](../dev/threat-model.md))                                                                   |
| Existence through foreign keys and unique constraints (item numbers)    | not protected (integrity checks bypass RLS); unique numbers and codes stay a residual oracle, as listed in `SECURITY.md`         |
| Copies outside the covered tables                                       | only if every copy table gets a policy                                                                                           |

## Cost and risks

* **Refactoring:** a request-scoped transaction for every request path, the
  system paths split off, the placeholder and count queries rewritten.
* **Deployment:** two or three roles in every installation, a migration path
  for existing ones whose application role is the owner or a superuser.
* **Performance:** a sub-query per row for every table without a group column;
  the search uses trigram and full-text indexes (migrations `0023`, `0072`)
  whose `ILIKE` and `@@` conditions are not leakproof, so whether those
  indexes are still used under a policy has to be measured with the scale
  check (#47).
* **Failure mode:** a policy filters silently. A missing system context shows
  up as missing rows, not as an error – easy to miss in review, so the tests
  would have to cover every placeholder and count.

## Decision

Not now. The service layer stays the one layer of record, with the widened
guard scanner (#312) and the route-complete leak test. RLS would protect
against exactly the class of bug the scanner already finds before a merge,
at the price of a refactoring of every request path, new database roles in
every installation and placeholder logic that has to move into the database.

If it is taken up later, in this order, each step its own issue and useful on
its own:

1. **Roles without RLS:** an application role that is neither owner nor
   superuser, a separate connection setting for the migrations, and a start-up
   warning when the application role is a superuser, has `BYPASSRLS` or owns
   the tables. Least privilege even without policies.
2. **A request-scoped connection type** in `services::*` that opens the
   transaction and sets `teamcraft.user_id` / `teamcraft.hidden_projects`
   with `set_config(…, true)`; the guard scanner checks that request paths use
   it.
3. **RLS as a test oracle:** restrictive policies created only in the test
   database, `tests/read_permissions.rs` and the need-to-know tests run with
   the application role. Differences show which queries need system context –
   no production change.
4. **Production policies on leaf content tables** that need no placeholders
   (`file_versions` text, 3D notes, simulation values and proposals,
   deviations, assistant messages), with a copied `project_id`, failing closed
   without the setting.
5. **Items, revisions and files** only after placeholders and counts go
   through `SECURITY DEFINER` functions and the search is measured.

## Sources

PostgreSQL documentation (current version):

* [Row Security Policies][rls] – default deny, owners and `BYPASSRLS`,
  `FORCE`, referential integrity, `row_security`, policies that look up other
  tables
* [CREATE POLICY][policy] – `USING`, `WITH CHECK`, permissive and restrictive
  policies, commands and roles
* [ALTER TABLE][alter] – `ENABLE` / `FORCE ROW LEVEL SECURITY`
* [CREATE ROLE][role] – `BYPASSRLS`, superusers
* [Client connection defaults – `row_security`][guc]
* [SET][set] – `SET` and `SET LOCAL`
* [Configuration settings functions][setconfig] – `set_config`,
  `current_setting`
* [Customized options][custom] – settings with a dot in the name
* [CREATE FUNCTION][function] – `LEAKPROOF`, `SECURITY DEFINER`
* [pg_dump][dump] – `--enable-row-security`
* [initdb][initdb] – `--username`, the bootstrap superuser

[rls]: https://www.postgresql.org/docs/current/ddl-rowsecurity.html
[policy]: https://www.postgresql.org/docs/current/sql-createpolicy.html
[alter]: https://www.postgresql.org/docs/current/sql-altertable.html
[role]: https://www.postgresql.org/docs/current/sql-createrole.html
[guc]: https://www.postgresql.org/docs/current/runtime-config-client.html#GUC-ROW-SECURITY
[set]: https://www.postgresql.org/docs/current/sql-set.html
[setconfig]: https://www.postgresql.org/docs/current/functions-admin.html#FUNCTIONS-ADMIN-SET
[custom]: https://www.postgresql.org/docs/current/runtime-config-custom.html
[function]: https://www.postgresql.org/docs/current/sql-createfunction.html
[dump]: https://www.postgresql.org/docs/current/app-pgdump.html
[initdb]: https://www.postgresql.org/docs/current/app-initdb.html
