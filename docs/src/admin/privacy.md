| *Support us on Ko-fi* tag | nothing | a static note in the desktop status bar, once per start; nothing is loaded from ko-fi.com, and only a click opens `https://ko-fi.com/warr_ev` in the browser (#403) | shown once per start | *Settings → Support TeamCraft*; desktop policy `ShowDonateTag = 0` |
# Privacy: what TeamCraft sends

Everything TeamCraft sends over the network **by itself** – not as an action
a user starts – and how to turn each off.

| What | From → to | Content | Default | Off with |
|---|---|---|---|---|
| Update check | desktop app → `api.github.com` | the request for the latest release (the app's version in the `User-Agent`) | on, at most once a day | *Settings → Updates*; desktop policy `CheckForUpdates = 0` ([Deploying the desktop app](desktop-deployment.md)) |
| Usage report | desktop app → **your own** TeamCraft server | the daily report below | on (it stays in your organisation) | *Settings → Privacy → Send an anonymous usage report* on each computer; desktop policy `UsageStatistics = 0` for all (`1` keeps it on): nothing is counted, kept or sent |
| Usage statistics | your TeamCraft server → the TeamCraft developers | an aggregate of the server and its desktop apps (below) | **off** | stays off unless an administrator turns it on; `TEAMCRAFT_USAGE_STATISTICS=0`, `DO_NOT_TRACK=1`, `TEAMCRAFT_NO_TELEMETRY=1` |

Crash reports of the desktop app are written on the computer and never sent;
the user can open a prefilled GitHub issue from them
([TeamCraft Desktop](../guide/desktop.md#crash-reports)).

## The desktop report (to your server)

Once a day of use, for the day before, the desktop app sends the server it is
signed in to (`POST /api/v1/usage`) exactly these fields:

```json
{
  "v": 1,
  "install_id": "<random UUID, made on this computer, not derived from anything>",
  "day": "2026-11-21",
  "version": "0.2.9",
  "os": "windows", "arch": "x86_64",
  "package": "msi",
  "server_version": null,
  "active_minutes": 37,
  "actions": { "checkout": 4, "checkin": 3, "open_3d_preview": 7 },
  "cad_exports": { "inventor": 2 }
}
```

`actions` counts only the fixed names `checkout`, `checkin`, `park`,
`cancel_checkout`, `get_files`, `open_cad`, `import_bom`, `import_folder`,
`new_item`, `release`, `open_3d_preview`, `search` and `assistant`;
`cad_exports` only `inventor`, `solid-edge` and `solidcraft`. The server
refuses any other field and drops unknown names. Never content: no user,
item, file, group or server names, numbers, paths, search terms, assistant
text or anything typed – the report does not even say which user sent it.
*Settings → Privacy → Show what is sent* shows today's report so far, a
finished one waiting to be sent and the one sent last.

The server keeps the reports for 400 days (`usage_reports`); *Admin → Usage
statistics* shows installations per version, system and package and the
action counts of the last 30 days.

## Sharing with the developers (off by default)

On *Admin → Usage statistics* an administrator can turn on **Share anonymous
usage statistics**. The server then sends, at most once a day over HTTPS
(10 s timeout, no redirects, no retries before the next day; a failure is
silent), exactly the JSON shown under *Show what is sent* on that page:

* a random instance id, the server version, OS and architecture, the install
  type (`docker` or `binary`) and the PostgreSQL major version;
* the numbers of items, files and active users **in classes** (`0`, `1-10`,
  `11-100`, `101-1000`, `1001-10000`, `10001+`);
* which optional features are on (OIDC, assistant, webhooks, Mattermost,
  SMTP) – on/off only, never their settings;
* the desktop reports of the last two days summed up: installations per version,
  system and package in classes, and the action and export counts.

Nothing is sent until a destination is configured with
`TEAMCRAFT_USAGE_STATISTICS_URL` – the TeamCraft project does not run a
collection service yet. `TEAMCRAFT_USAGE_STATISTICS=0|1` fixes the switch;
`DO_NOT_TRACK=1` and `TEAMCRAFT_NO_TELEMETRY=1` always turn it off, and the
local test server (`teamcraft local`) and a live demo never send. See
[Configuration](configuration.md#usage-statistics-optional).
