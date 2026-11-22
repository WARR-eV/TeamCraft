//! Item data written into a SolidCraft design's properties (#369): the
//! design then carries its TeamCraft item number, revision and name, so
//! SolidCraft's drawings, bill of materials and exports show the managed
//! values without retyping them.
//!
//! SolidCraft's own command `document.metadata.set` does the writing,
//! headless: `solidcraft-cli run <script.json> --in <design> --save <design>`
//! (sources: `docs/src/research/solidcraft-design.md`). It sets the design
//! properties `part_number`, `revision` and `description`, and an object
//! under the reverse-DNS key [`KEY`] with the ids and the server, so that
//! tools (the TeamCraft plugin for SolidCraft) find the item again. Keys
//! SolidCraft does not know are kept by SolidCraft.
//!
//! Only for a design of my own check-out (writable) that SolidCraft does not
//! have open (its lock file, `cadlock`). The file changes, so it is a local
//! modification like any other and goes to the server with the next
//! check-in.

use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

/// The reverse-DNS key of TeamCraft's object in a design's properties.
pub const KEY: &str = "teamcraft.pdm";

/// What is written into the design.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ItemData {
    pub item_number: String,
    pub revision: String,
    pub name: String,
    pub item_id: String,
    pub revision_id: String,
}

/// The script `solidcraft-cli run` executes: one `document.metadata.set`
/// with all values, so the change is all or nothing.
pub fn script(data: &ItemData, server: &str) -> Value {
    json!({
        "commands": [{
            "command": "document.metadata.set",
            "params": {
                "values": {
                    "part_number": data.item_number,
                    "revision": data.revision,
                    "description": data.name,
                    KEY: {
                        "item_number": data.item_number,
                        "revision": data.revision,
                        "item_id": data.item_id,
                        "revision_id": data.revision_id,
                        "server": server,
                    }
                }
            }
        }]
    })
}

/// Why the design cannot be written now, if it cannot.
pub fn refused(design: &Path) -> Option<String> {
    let name = design
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if crate::cadexport::program_for(design) != Some(crate::cadexport::CadProgram::SolidCraft) {
        return Some(format!("{name} is not a SolidCraft design."));
    }
    if crate::workspace::is_read_only(design) {
        return Some(format!(
            "{name} is read-only – check the revision out before writing into it."
        ));
    }
    if let Some(open) = crate::cadlock::open_in(design) {
        return Some(format!(
            "{name} is open in SolidCraft on {} – close it there first, then write the item data.",
            open.computer
        ));
    }
    None
}

/// Writes `data` into the design at `design` with SolidCraft's command-line
/// tool.
pub fn write(design: &Path, data: &ItemData, server: &str) -> Result<(), String> {
    if let Some(why) = refused(design) {
        return Err(why);
    }
    let cli = crate::cadexport::solidcraft_cli().ok_or_else(|| {
        "SolidCraft's command-line tool (solidcraft-cli) was not found – install SolidCraft \
         or set SOLIDCRAFT_CLI to its path."
            .to_string()
    })?;
    let script_path = std::env::temp_dir().join(format!(
        "teamcraft-solidcraft-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    std::fs::write(&script_path, script(data, server).to_string())
        .map_err(|e| format!("Cannot write the SolidCraft script: {e}"))?;
    let result = crate::cadexport::run_solidcraft_script(&cli, &script_path, design);
    let _ = std::fs::remove_file(&script_path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> ItemData {
        ItemData {
            item_number: "TC-1042".into(),
            revision: "B".into(),
            name: "Motor mount".into(),
            item_id: "0b6f…".into(),
            revision_id: "7c1e…".into(),
        }
    }

    #[test]
    fn one_metadata_set_with_all_values() {
        let s = script(&data(), "https://plm.example.org");
        let commands = s["commands"].as_array().unwrap();
        assert_eq!(commands.len(), 1, "all or nothing");
        assert_eq!(commands[0]["command"], "document.metadata.set");
        let values = &commands[0]["params"]["values"];
        assert_eq!(values["part_number"], "TC-1042");
        assert_eq!(values["revision"], "B");
        assert_eq!(values["description"], "Motor mount");
        assert_eq!(values[KEY]["revision_id"], "7c1e…");
        assert_eq!(values[KEY]["server"], "https://plm.example.org");
        // SolidCraft wants another tool's key as reverse-DNS labels
        assert!(KEY.split('.').count() >= 2);
        assert!(KEY
            .split('.')
            .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')));
    }

    #[test]
    fn only_writable_designs_not_open_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let step = dir.path().join("plate.step");
        std::fs::write(&step, b"ISO-10303-21;").unwrap();
        assert!(refused(&step).unwrap().contains("not a SolidCraft design"));
        let design = dir.path().join("plate.sc");
        std::fs::write(&design, b"PK\x03\x04").unwrap();
        assert_eq!(refused(&design), None);
        crate::workspace::set_read_only(&design, true).unwrap();
        assert!(refused(&design).unwrap().contains("read-only"));
        crate::workspace::set_read_only(&design, false).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        std::fs::write(
            dir.path().join(".~plate.sc.lock"),
            format!(r#"{{"host":"WS-7","user":"kim","opened":{now},"refreshed":{now}}}"#),
        )
        .unwrap();
        assert!(refused(&design)
            .unwrap()
            .contains("open in SolidCraft on WS-7"));
    }
}
