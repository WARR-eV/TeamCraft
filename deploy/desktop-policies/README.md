# TeamCraft Desktop policies

Templates for managed deployments of TeamCraft Desktop (#378). The app only
reads these values, once at start; nothing about them is sent anywhere. The
full description is in the admin guide, *Deploying the desktop app*
(`docs/src/admin/desktop-deployment.md`).

| Folder     | Use                                                                                     |
|------------|-----------------------------------------------------------------------------------------|
| `windows/` | `TeamCraft.admx` + `en-US/TeamCraft.adml` (English) and `de-DE/TeamCraft.adml` (German) for Group Policy and Intune (custom ADMX import with the ADMX and the en-US ADML); values under `Software\Policies\TeamCraft` in `HKLM` (wins) or `HKCU` |
| `macos/`   | an example configuration profile for the preference domain `io.github.rudalabs.teamcraft` |
| `linux/`   | an example of `/etc/teamcraft/policies.json`                                            |

Each release ships this folder as `teamcraft-desktop-policies-<version>.zip`
(listed in `SHA256SUMS.txt`).
