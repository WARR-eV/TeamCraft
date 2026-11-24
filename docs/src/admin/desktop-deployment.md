| `ShowDonateTag`     | switch | `0`: the *Support us on Ko-fi* tag is never shown and users cannot turn it on; `1`: it shows once per start of the app and users cannot turn it off; not set: it shows once per start and each user can switch it off in *Settings → Support TeamCraft*. The link (`https://ko-fi.com/warr_ev`) is fixed in the app and cannot be changed (#403) |
# Deploying the desktop app

An IT department that rolls TeamCraft Desktop out to many computers can preset
settings with the operating system's usual tools. The app only **reads**
these *policies*, once at start; nothing about them is sent anywhere
(`desktop/src/policy.rs`, #378).

## Policies

| Policy              | Type   | Values                                                                                                   |
|---------------------|--------|----------------------------------------------------------------------------------------------------------|
| `ServerUrl`         | text   | The TeamCraft server (`https://…`). The app starts with it; the connect screen is prefilled and read-only; other servers – and the local test server – are refused, unless `AllowOtherServers` is `1` |
| `AllowOtherServers` | switch | `1`: users may add other servers although `ServerUrl` is set (it is then only the default)              |
| `CheckForUpdates`   | switch | `1`: the daily release check is on and cannot be turned off; `0`: no checks at all                      |
| `CrashReportPrompt` | switch | `0`: crash reports stay on the computer and are never offered for sharing; `1`: always offered          |
| `WorkspaceRoot`     | text   | The folder new working folders are created in (an absolute path); users can still choose another one. *Getting started* then shows the folder as a line instead of asking. CAD programs are configured with your own deployment tools ([Set up CAD programs](cad-setup.md)) |
| `ReportUrl`         | text   | Where *Report a problem* sends reports: your help desk's issue form (`https://…`), opened with the reviewed report as `title` and `body`; replaces *Settings → Report problems to*, which shows *Set by your organization* |
| `OrganizationName`  | text   | Shown as *Managed by …* in *Settings* and *About* (one line, at most 64 characters)                     |
| `UsageStatistics`   | switch | `0`: the app counts, keeps and sends no usage report to its server (#383); `1`: it reports to its own server and users cannot switch it off; not set: each user chooses (*Settings → Privacy*, on unless switched off, #473). Fixed either way, the switch shows *Set by your organization*. Whether the server shares a summary with the developers is its administrator's choice ([Privacy](privacy.md)) |

A *switch* is the number `0` or `1` (`REG_DWORD` on Windows; an integer or a
boolean in a profile or the JSON file). A value of the **wrong type or out of
range counts as not set** – an address that is not `http(s)`, a relative
folder, a name over 64 characters or with a line break. Removing a policy
brings back the user's own choice: the app never overwrites it.

A setting fixed by a policy is shown in *Settings* as **Set by your
organization** and is disabled; *Settings* and *About* name the organisation.

## Windows

Registry values under `Software\Policies\TeamCraft` in
`HKEY_LOCAL_MACHINE` or `HKEY_CURRENT_USER`; the machine wins, value by value.

* **Group Policy:** copy `TeamCraft.admx` to `%SystemRoot%\PolicyDefinitions`
  (or the central store `\\<domain>\SYSVOL\<domain>\Policies\PolicyDefinitions`)
  and `en-US\TeamCraft.adml` to its `en-US` folder (for German editors also
  `de-DE\TeamCraft.adml` to its `de-DE` folder). The policies are under
  *Administrative Templates → TeamCraft Desktop*, for the computer and the user.
* **Intune:** *Devices → Configuration → Import ADMX* with `TeamCraft.admx` and
  `en-US\TeamCraft.adml` (the German ADML is only for the Group Policy editor).
* **By hand or script:** `reg add "HKLM\Software\Policies\TeamCraft" /v ServerUrl /t REG_SZ /d https://plm.example.org`.
* **Silent install:** `msiexec /i TeamCraft_<version>_x64_en-US.msi /qn`
  installs without questions (a per-machine MSI is still open in #378).

## macOS

A configuration profile for the preference domain
`io.github.rudalabs.teamcraft`, deployed with your MDM. The computer's
managed preferences win over the user's
(`/Library/Managed Preferences/[<user>/]io.github.rudalabs.teamcraft.plist`,
read with `plutil`). `macos/teamcraft-policies.mobileconfig` is an example:
change the values, give both `PayloadUUID`s new values (`uuidgen`) and leave
out a key to let users choose.

## Linux

A JSON object in `/etc/teamcraft/policies.json` (readable by the users, owned
by root), e.g.

```json
{
  "ServerUrl": "https://plm.example.org",
  "CheckForUpdates": 0,
  "OrganizationName": "Example Lab"
}
```

## The templates

The templates are in
[`deploy/desktop-policies/`](https://github.com/WARR-eV/TeamCraft/tree/main/deploy/desktop-policies)
and attached to every release as
`teamcraft-desktop-policies-<version>.zip`, listed in the release's
`SHA256SUMS.txt`.

## Checking a computer

*Settings* shows *Managed by …* and the fixed settings. Policies are read at
start: restart the app after changing them.
