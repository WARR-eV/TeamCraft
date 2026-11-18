# A hosted live demo

Question: how can visitors try TeamCraft in seconds – without installing it –
without the demo becoming a target for abuse or a maintenance burden for a
volunteer project? Date: 2026-11-14.

## How other open-source projects do it

| Project   | What visitors get                                                                   | Reset                                       | Source                                                                                                                                                                                        |
|-----------|-------------------------------------------------------------------------------------|---------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| BookStack | a public admin login; user and settings changes blocked, everything else allowed    | database and images every half hour         | [bookstackapp.com](https://bookstackapp.com/)                                                                                                                                                 |
| Nextcloud | an instant trial: a personal account after entering an e-mail address               | the trial account is deleted after one hour | [Nextcloud instant trial](https://nextcloud.com/instant-trial/)                                                                                                                               |
| Gitea     | a public instance with open sign-up (moved from `try.gitea.io` to `demo.gitea.com`) | occasionally; "do not store important data" | [demo location change](https://gitea.psi.ch/gitea/gitea/src/commit/603c7389b8e56740033e51b36b79e61136b46002), [forum](https://forum.gitea.com/t/creating-organizations-on-test-instance/4031) |

Lessons from operators: open sign-up on a public instance attracts abuse
reports (spam, phishing, network scans), so GitLab's guidance is to disable
sign-up on public instances that do not expect public users
([GitLab trust & safety](https://handbook.gitlab.com/handbook/security/security-operations/trustandsafety/diy/));
the Cloudron demo team found that a demo is a showcase, not a place for
users' own tests, and that resetting only nightly leaves abuse in place until
then ([Cloudron forum](https://forum.cloudron.io/topic/15470/cloudron-demo/18)).

## Options for TeamCraft

| Option                                                 | Pros                                                                                                                                                     | Cons                                                                                                                                        |
|--------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------|
| Open sign-up, reset every few hours                    | visitors try everything, editing included                                                                                                                | uploads are file hosting for strangers (malware, illegal content, phishing pages as "CAD files"), abuse reports, disk use; needs moderation |
| Shared admin account, some actions blocked (BookStack) | close to the real thing                                                                                                                                  | uploads still possible; one visitor's changes confuse the next; the block list must grow with every feature                                 |
| **One public read-only account, periodic reset**       | nothing a stranger can upload or break; every page, the BOM, the 3D viewer, changes, simulations and the restricted-group view still work; no moderation | editing cannot be tried – the banner points to the install instead                                                                          |
| Per-visitor throw-away instances                       | full product, isolated                                                                                                                                   | far too much infrastructure for a volunteer project                                                                                         |

## Decision

One public, **read-only** account (`TEAMCRAFT_DEMO_USER`, password shown on
the login page) on an instance loaded with the demo data of the README
screenshots – the openly licensed NIST CAD models, BOMs, change orders,
simulations, an as-built unit and a restricted group. The server refuses every
request of that account that is not `GET`/`HEAD` (sign-out excepted), in the
web UI and the API, with a friendly message; a banner on every page says it is
a demo and links to the install. A stranger's wrong passwords cannot lock the
account (per-address throttle only). The administrator account is unaffected,
so the same instance is seeded through the normal API.

Even read-only, the instance is reset regularly: the database goes back to a
snapshot taken right after seeding (`pg_restore` through the database
container – no Docker volume is deleted), which also clears sessions, sign-in
audit entries and notifications. The vault is never written by visitors, so it
keeps its cached previews.

Not now: editing in the demo, per-visitor instances, open sign-up. Runbook:
[Live demo](../admin/live-demo.md).
