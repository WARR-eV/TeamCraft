// SPDX-License-Identifier: MPL-2.0
//! What the plugin's entries do, over SolidCraft's command surface – pure, so
//! the tests run on the build machine against a fake host ([`Host`]).
//!
//! The plugin has no network access (SolidCraft's plugin runtime refuses it),
//! so it works with what is in the open design: its properties, the object
//! TeamCraft keeps under [`KEY`] (written by the desktop app's *Item data* or
//! by [`link`]), the bill of materials and the linked designs.

use serde_json::{json, Map, Value};

/// The reverse-DNS key of TeamCraft's object in a design's properties – the
/// same as the desktop app's (`desktop/src/scprops.rs`).
pub const KEY: &str = "teamcraft.pdm";

/// SolidCraft's commands, as the plugin calls them.
pub trait Host {
    /// Runs `command` with `args` and returns its JSON result, or why it
    /// failed.
    fn run(&mut self, command: &str, args: Value) -> Result<Value, String>;
    /// A file of the bridge folder ([`bridge::GRANT`]); `None` when it is not
    /// there.
    fn read_file(&mut self, path: &str) -> Result<Option<Vec<u8>>, String>;
    /// Writes a file of the bridge folder (it appears when the run succeeds).
    fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String>;
    /// Unix seconds; 0 without the clock grant.
    fn now(&mut self) -> u64;
}

use crate::bridge::{self, Read};

/// Why an entry failed.
#[derive(Debug, PartialEq)]
pub enum EntryError {
    /// The input is broken: which field, and why.
    Input { field: String, reason: String },
    /// A SolidCraft command failed.
    Command { command: String, reason: String },
    /// An entry this plugin does not have.
    Unsupported(String),
}

/// The label of a field of *Link to TeamCraft item*, as its dialog shows it.
fn label(field: &str) -> &str {
    match field {
        "item_number" => "Item number",
        "revision" => "Revision",
        "name" => "Item name",
        "server" => "TeamCraft server",
        other => other,
    }
}

/// A broken input. SolidCraft shows only the reason, so the reason names the
/// field itself (`Item number is required`, not `is required`).
fn input(field: &str, reason: &str) -> EntryError {
    EntryError::Input {
        field: field.into(),
        reason: format!("{} {reason}. Nothing was changed.", label(field)),
    }
}

/// Why SolidCraft refused or failed `command`, in words a user can act on.
/// The plugin never talks to a TeamCraft server or the desktop app (it has no
/// network), so a failure here is always SolidCraft's answer about the open
/// design – TeamCraft not running is never the cause.
pub fn explain(command: &str, reason: &str) -> String {
    let reason = reason.trim();
    let lower = reason.to_ascii_lowercase();
    let hint = if lower.contains("permission denied") || lower.contains("not granted") {
        "This SolidCraft version does not let the TeamCraft plugin use it – install the TeamCraft \
         plugin that matches your SolidCraft version."
    } else if lower.contains("cancel") {
        "The run was stopped."
    } else if lower.contains("limit") || lower.contains("fuel") || lower.contains("deadline") {
        "The design is too large for the plugin's limits."
    } else {
        "Is a design open? Nothing in the design was changed."
    };
    let reason = if reason.is_empty() {
        "no reason given"
    } else {
        reason
    };
    format!("SolidCraft could not run {command} ({reason}). {hint}")
}

fn call(host: &mut dyn Host, command: &str, args: Value) -> Result<Value, EntryError> {
    host.run(command, args)
        .map_err(|reason| EntryError::Command {
            command: command.into(),
            reason: explain(command, &reason),
        })
}

/// The entries of the manifest (`plugin.json`).
pub const ENTRIES: [&str; 8] = [
    "status",
    "check",
    "link",
    "unlink",
    "live",
    "check-out",
    "check-in",
    "open",
];

/// Runs the entry `entry` with its `params`.
pub fn execute(host: &mut dyn Host, entry: &str, params: &Value) -> Result<Value, EntryError> {
    match entry {
        "status" => status(host),
        "check" => check(host),
        "link" => link(host, params),
        "unlink" => unlink(host),
        "live" => live(host),
        "check-out" => ask_desktop(host, "check_out"),
        "check-in" => ask_desktop(host, "check_in"),
        "open" => ask_desktop(host, "open"),
        other => Err(EntryError::Unsupported(format!(
            "The TeamCraft plugin has no entry `{other}`."
        ))),
    }
}

/// The design's `metadata` as saved and SolidCraft's `properties` (its keys as text).
type Properties = (Map<String, Value>, Map<String, Value>);

/// The design's properties: (`metadata` as saved, SolidCraft's `properties`).
fn properties(host: &mut dyn Host) -> Result<Properties, EntryError> {
    let read = call(host, "document.metadata", json!({}))?;
    let object = |name: &str| {
        read.get(name)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    Ok((object("metadata"), object("properties")))
}

fn text<'a>(map: &'a Map<String, Value>, key: &str) -> &'a str {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
}

/// The message for a design whose TeamCraft data is not an object (written
/// by hand or by another tool), with what to do about it.
const DAMAGED: &str = "The design's TeamCraft data (teamcraft.pdm) is damaged. Use *Link to \
                       TeamCraft item* to write it again, or *Unlink from TeamCraft* to remove it.";

/// Whether the design has a TeamCraft entry that is not an object.
fn damaged(metadata: &Map<String, Value>) -> bool {
    metadata.get(KEY).is_some_and(|v| !v.is_object())
}

/// TeamCraft's object in the design, if there is one.
fn teamcraft(metadata: &Map<String, Value>) -> Option<&Map<String, Value>> {
    metadata.get(KEY).and_then(Value::as_object)
}

/// The TeamCraft page of the revision, from the server and the revision id.
fn revision_url(tc: &Map<String, Value>) -> Option<String> {
    let server = text(tc, "server").trim_end_matches('/');
    let id = text(tc, "revision_id");
    let web = server.starts_with("https://") || server.starts_with("http://");
    (web && !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| format!("{server}/revisions/{id}"))
}

/// *TeamCraft status*: which item and revision the design belongs to, and
/// whether its part number and revision still say the same.
pub fn status(host: &mut dyn Host) -> Result<Value, EntryError> {
    let (metadata, props) = properties(host)?;
    let part_number = text(&props, "part_number");
    let revision = text(&props, "revision");
    if damaged(&metadata) {
        return Ok(json!({
            "managed": false,
            "damaged": true,
            "part_number": part_number,
            "revision": revision,
            "message": DAMAGED,
        }));
    }
    let Some(tc) = teamcraft(&metadata) else {
        return Ok(json!({
            "managed": false,
            "part_number": part_number,
            "revision": revision,
            "message": "This design is not linked to a TeamCraft item. Check it out in the TeamCraft \
                        desktop app and use *Item data*, or use *Link to TeamCraft item* here.",
        }));
    };
    let item = text(tc, "item_number");
    let rev = text(tc, "revision");
    let mut out = json!({
        "managed": true,
        "item_number": item,
        "revision": rev,
        "server": text(tc, "server"),
        "part_number_matches": part_number == item,
        "revision_matches": revision == rev,
        "message": format!("TeamCraft item {item}, revision {rev}."),
    });
    if let Some(url) = revision_url(tc) {
        out["url"] = json!(url);
    }
    if part_number != item || revision != rev {
        out["message"] = json!(format!(
            "TeamCraft item {item}, revision {rev} – but the design says part number \
             “{part_number}”, revision “{revision}”. Write the item data again or correct them."
        ));
    }
    Ok(out)
}

/// One finding of [`check`].
fn finding(level: &str, message: String) -> Value {
    json!({"level": level, "message": message})
}

/// Whether `path` is absolute (Unix, Windows drive or network path).
fn is_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with('/')
        || path.starts_with("\\\\")
        || (b.len() > 2
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/'))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// *Check for TeamCraft*: what keeps the design from going into TeamCraft
/// cleanly – missing part numbers, a design that says another item than the
/// one it is linked to, BOM rows TeamCraft cannot match, linked designs it
/// cannot find by name. Reads only.
pub fn check(host: &mut dyn Host) -> Result<Value, EntryError> {
    let (metadata, props) = properties(host)?;
    let mut findings = Vec::new();
    let part_number = text(&props, "part_number");
    if part_number.is_empty() {
        findings.push(finding(
            "warning",
            "The design has no part number (Design Properties) – TeamCraft matches the item by it."
                .into(),
        ));
    }
    if text(&props, "description").is_empty() {
        findings.push(finding(
            "info",
            "The design has no description – TeamCraft uses it as the item name of a new item."
                .into(),
        ));
    }
    match teamcraft(&metadata) {
        None if damaged(&metadata) => findings.push(finding("warning", DAMAGED.into())),
        None => findings.push(finding(
            "info",
            "Not linked to a TeamCraft item yet.".into(),
        )),
        Some(tc) => {
            let item = text(tc, "item_number");
            if !part_number.is_empty() && part_number != item {
                findings.push(finding(
                    "warning",
                    format!("The part number “{part_number}” is not the TeamCraft item {item} the design is linked to."),
                ));
            }
            let rev = text(tc, "revision");
            if text(&props, "revision") != rev {
                findings.push(finding(
                    "warning",
                    format!(
                        "The revision “{}” is not TeamCraft's revision {rev}.",
                        text(&props, "revision")
                    ),
                ));
            }
        }
    }
    check_bom(host, &mut findings);
    check_links(host, &mut findings);
    let ready = findings.iter().all(|f| f["level"] == "info");
    let problems = findings.iter().filter(|f| f["level"] != "info").count();
    Ok(json!({
        "ready": ready,
        "summary": if ready {
            "Ready for TeamCraft.".to_string()
        } else {
            format!("{problems} thing(s) to fix before checking in to TeamCraft.")
        },
        "findings": findings,
    }))
}

/// BOM rows TeamCraft cannot match: no part number, a quantity SolidCraft
/// cannot count, one part number with different descriptions.
fn check_bom(host: &mut dyn Host, findings: &mut Vec<Value>) {
    // a part without components answers with no rows; a failure means the
    // rows were not checked, which must not read as "ready"
    let bom = match host.run("assembly.bom", json!({"levels": "all"})) {
        Ok(bom) => bom,
        Err(reason) => {
            findings.push(finding(
                "warning",
                format!(
                    "The bill of materials was not checked: {}",
                    explain("assembly.bom", &reason)
                ),
            ));
            return;
        }
    };
    let rows = bom
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen: Map<String, Value> = Map::new();
    for row in &rows {
        let item = match &row["item"] {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => "?".into(),
        };
        let name = row["definition"].as_str().unwrap_or("").to_string();
        let number = row["part_number"].as_str().unwrap_or("").trim().to_string();
        let description = row["description"].as_str().unwrap_or("").trim().to_string();
        if row["bom_type"] == "reference" || row["bom_type"] == "phantom" {
            continue;
        }
        if number.is_empty() {
            findings.push(finding(
                "warning",
                format!("BOM item {item} ({name}) has no part number – TeamCraft's BOM import would create a new item for it."),
            ));
        } else if let Some(other) = seen.get(&number).and_then(Value::as_str) {
            if other != description {
                findings.push(finding(
                    "warning",
                    format!("Part number {number} has two descriptions: “{other}” and “{description}” (item {item})."),
                ));
            }
        } else {
            seen.insert(number.clone(), json!(description));
        }
        if row.get("quantity_unknown").is_some() {
            findings.push(finding(
                "warning",
                format!("BOM item {item} ({name}) has a quantity SolidCraft cannot add up."),
            ));
        }
        if row.get("duplicate").is_some() {
            findings.push(finding(
                "warning",
                format!("BOM item number {item} is used twice."),
            ));
        }
    }
}

/// Linked designs TeamCraft cannot match: an absolute path (outside the
/// working folder) or two links with the same file name in different
/// folders (TeamCraft finds references by file name).
fn check_links(host: &mut dyn Host, findings: &mut Vec<Value>) {
    let links = match host.run("component.links", json!({})) {
        Ok(links) => links,
        Err(reason) => {
            findings.push(finding(
                "warning",
                format!(
                    "The linked designs were not checked: {}",
                    explain("component.links", &reason)
                ),
            ));
            return;
        }
    };
    let links = links
        .get("links")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut by_name: Map<String, Value> = Map::new();
    for link in &links {
        let path = link["path"].as_str().unwrap_or("");
        let component = link["name"].as_str().unwrap_or("?");
        if path.is_empty() {
            continue;
        }
        if is_absolute(path) {
            findings.push(finding(
                "warning",
                format!("{component} links {path} by an absolute path – keep linked designs in the TeamCraft working folder and save the assembly there, so the link is relative."),
            ));
        }
        let key = file_name(path).to_lowercase();
        match by_name.get(&key).and_then(Value::as_str) {
            Some(other) if other != path => findings.push(finding(
                "warning",
                format!("Two linked designs are named {}: {other} and {path} – TeamCraft tells references apart by file name.", file_name(path)),
            )),
            Some(_) => {}
            None => {
                by_name.insert(key, json!(path));
            }
        }
    }
}

/// Longest value written (SolidCraft's limit for a property's text is 4 KiB).
const MAX_FIELD: usize = 256;

fn field(params: &Value, name: &str, required: bool) -> Result<String, EntryError> {
    let value = match params.get(name) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.trim().to_string(),
        Some(_) => return Err(input(name, "must be text")),
    };
    if required && value.is_empty() {
        return Err(input(name, "is required"));
    }
    if value.len() > MAX_FIELD || value.chars().any(char::is_control) {
        return Err(input(name, "must be one line of at most 256 bytes"));
    }
    Ok(value)
}

/// *Link to TeamCraft item*: writes the item number, revision and name into
/// the design's properties and TeamCraft's object, like the desktop app's
/// *Item data* – for designs edited without the desktop app. One undo step.
pub fn link(host: &mut dyn Host, params: &Value) -> Result<Value, EntryError> {
    let item_number = field(params, "item_number", true)?;
    let revision = field(params, "revision", true)?;
    let name = field(params, "name", false)?;
    let server = field(params, "server", false)?;
    if !server.is_empty() && !(server.starts_with("https://") || server.starts_with("http://")) {
        return Err(input("server", "must start with https:// or http://"));
    }
    let (metadata, _) = properties(host)?;
    // keep the ids the desktop app wrote when the item stays the same
    let mut tc = teamcraft(&metadata)
        .filter(|tc| text(tc, "item_number") == item_number)
        .cloned()
        .unwrap_or_default();
    if text(&tc, "revision") != revision {
        tc.remove("revision_id");
    }
    tc.insert("item_number".into(), json!(item_number));
    tc.insert("revision".into(), json!(revision));
    if !server.is_empty() {
        tc.insert("server".into(), json!(server));
    }
    let mut values = json!({
        "part_number": item_number,
        "revision": revision,
        KEY: tc,
    });
    if !name.is_empty() {
        values["description"] = json!(name);
    }
    call(host, "document.metadata.set", json!({"values": values}))?;
    Ok(json!({
        "linked": true,
        "item_number": item_number,
        "revision": revision,
        "message": format!("Linked to TeamCraft item {item_number}, revision {revision}. Save the design."),
    }))
}

/// *Unlink from TeamCraft*: removes TeamCraft's object; the part number and
/// revision stay as they are.
pub fn unlink(host: &mut dyn Host) -> Result<Value, EntryError> {
    let removed = call(host, "document.metadata.remove", json!({"key": KEY}))?;
    let was = removed
        .get("removed")
        .and_then(Value::as_array)
        .is_some_and(|r| !r.is_empty());
    Ok(json!({
        "unlinked": was,
        "message": if was {
            "The design is no longer linked to a TeamCraft item."
        } else {
            "The design was not linked to a TeamCraft item."
        },
    }))
}

/// The item number and revision of a linked design, or the message why
/// there are none.
fn identity(metadata: &Map<String, Value>) -> Result<(String, String), &'static str> {
    if damaged(metadata) {
        return Err(DAMAGED);
    }
    let tc = teamcraft(metadata).ok_or(
        "This design is not linked to a TeamCraft item. Check it out in the TeamCraft desktop \
         app and use *Item data*, or use *Link to TeamCraft item* here.",
    )?;
    let (item, revision) = (text(tc, "item_number"), text(tc, "revision"));
    if item.is_empty() || revision.is_empty() {
        return Err(DAMAGED);
    }
    Ok((item.to_string(), revision.to_string()))
}

const UPDATE_PLUGIN: &str = "The TeamCraft desktop app is newer than this plugin. Install the \
                             TeamCraft plugin that comes with it.";

/// *TeamCraft live status*: what the desktop app last shared about this
/// revision (bridge folder). Never fails because the desktop app is not
/// running: it says so, and what it knew when it last ran.
pub fn live(host: &mut dyn Host) -> Result<Value, EntryError> {
    let (metadata, _) = properties(host)?;
    let (item, revision) = match identity(&metadata) {
        Ok(id) => id,
        Err(message) => return Ok(json!({"live": false, "message": message})),
    };
    let now = host.now();
    let beat = read_bridge(host, "desktop.json", "teamcraft.solidcraft.desktop")?;
    let state = read_bridge(
        host,
        &bridge::state_file(&item, &revision),
        "teamcraft.solidcraft.state",
    )?;
    let mut out = json!({
        "item_number": item,
        "revision": revision,
        "desktop_running": false,
        "live": false,
    });
    if matches!(beat, Read::Newer(_)) || matches!(state, Read::Newer(_)) {
        out["message"] = json!(UPDATE_PLUGIN);
        return Ok(out);
    }
    let alive = bridge::desktop_alive(&beat, now);
    out["desktop_running"] = json!(alive.is_some());
    out["server_reachable"] = json!(alive == Some(true));
    let message = match (&state, alive) {
        (Read::Missing, None) => format!(
            "The TeamCraft desktop app is not running, and it has shared nothing about {item} \
             revision {revision} yet. Start it – it shares the state of the revisions in your \
             working folder."
        ),
        (Read::Missing, Some(_)) => format!(
            "The TeamCraft desktop app runs, but has shared nothing about {item} revision \
             {revision} yet. Open the revision in the desktop app's Workspace once, or check that \
             this design is in its working folder."
        ),
        (Read::Damaged, _) => format!(
            "The state TeamCraft shared about {item} revision {revision} cannot be read. The \
             desktop app writes it again the next time it shows the revision."
        ),
        (Read::Ok(s), alive) => {
            out["state"] = s.clone();
            let since = bridge::age(now, s["written_at"].as_u64().unwrap_or(0));
            let said = bridge::describe_state(s);
            match alive {
                Some(true) => {
                    out["live"] = json!(true);
                    format!("{item} revision {revision}: {said}. (as of {since})")
                }
                Some(false) => format!(
                    "The TeamCraft desktop app has no connection to the server. As of {since}: \
                     {said}."
                ),
                None => format!("The TeamCraft desktop app is not running. As of {since}: {said}."),
            }
        }
        (Read::Newer(_), _) => unreachable!("handled above"),
    };
    out["message"] = json!(message);
    Ok(out)
}

/// Reads and checks a bridge file.
fn read_bridge(host: &mut dyn Host, path: &str, schema: &str) -> Result<Read, EntryError> {
    let bytes = host.read_file(path).map_err(|reason| EntryError::Command {
        command: format!("files.read-file {}/{path}", bridge::GRANT),
        reason: format!(
            "The TeamCraft plugin could not read its folder ({reason}). Install the plugin \
                 again and allow the folder <working folder>/.teamcraft/solidcraft as \
                 `teamcraft`."
        ),
    })?;
    Ok(bridge::parse(bytes.as_deref(), schema))
}

/// *Check out in TeamCraft*, *Check in with TeamCraft*, *Open in TeamCraft*:
/// leaves a request for the desktop app. It carries it out when it runs (or
/// when it starts, within 10 minutes); a check-in always opens the desktop
/// app's own check-in dialog, never runs by itself.
pub fn ask_desktop(host: &mut dyn Host, action: &str) -> Result<Value, EntryError> {
    let (metadata, _) = properties(host)?;
    let (item, revision) =
        identity(&metadata).map_err(|m| EntryError::Unsupported(m.to_string()))?;
    let now = host.now();
    let beat = read_bridge(host, "desktop.json", "teamcraft.solidcraft.desktop")?;
    let request = bridge::request(&item, &revision, action, now);
    let file = bridge::request_file(&item, &revision, now, action);
    host.write_file(&file, request.to_string().as_bytes())
        .map_err(|reason| EntryError::Command {
            command: format!("files.write-file {}/{file}", bridge::GRANT),
            reason: format!(
                "The TeamCraft plugin could not write to its folder ({reason}). Install the \
                 plugin again and allow the folder <working folder>/.teamcraft/solidcraft as \
                 `teamcraft`, with writing."
            ),
        })?;
    let what = match action {
        "check_out" => format!("check out {item} revision {revision}"),
        "check_in" => format!("open the check-in of {item} revision {revision}"),
        _ => format!("show {item} revision {revision}"),
    };
    let save = if action == "check_in" {
        " Save the design first."
    } else {
        ""
    };
    let message = match bridge::desktop_alive(&beat, now) {
        Some(_) => format!("Sent to the TeamCraft desktop app: it will {what}.{save}"),
        None => format!(
            "The TeamCraft desktop app is not running. The request is kept: it will {what} when \
             it starts within 10 minutes.{save}"
        ),
    };
    Ok(json!({"requested": action, "file": file, "message": message}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A design in memory: its metadata, BOM rows and links; records calls.
    #[derive(Default)]
    struct Fake {
        metadata: Map<String, Value>,
        rows: Vec<Value>,
        links: Vec<Value>,
        calls: Vec<(String, Value)>,
        /// Commands that fail, with SolidCraft's reason.
        fail: Vec<(&'static str, &'static str)>,
        /// The bridge folder.
        files: std::collections::BTreeMap<String, Vec<u8>>,
        clock: u64,
    }

    impl Host for Fake {
        fn run(&mut self, command: &str, args: Value) -> Result<Value, String> {
            self.calls.push((command.to_string(), args.clone()));
            if let Some((_, why)) = self.fail.iter().find(|(c, _)| *c == command) {
                return Err((*why).to_string());
            }
            let props = |m: &Map<String, Value>| {
                let mut p = Map::new();
                for k in [
                    "project",
                    "part_number",
                    "revision",
                    "author",
                    "organization",
                    "description",
                    "license",
                ] {
                    p.insert(
                        k.into(),
                        m.get(k)
                            .filter(|v| v.is_string())
                            .cloned()
                            .unwrap_or(Value::Null),
                    );
                }
                Value::Object(p)
            };
            match command {
                "document.metadata" => {
                    Ok(json!({"metadata": self.metadata, "properties": props(&self.metadata)}))
                }
                "document.metadata.set" => {
                    for (k, v) in args["values"].as_object().unwrap() {
                        self.metadata.insert(k.clone(), v.clone());
                    }
                    Ok(json!({"metadata": self.metadata}))
                }
                "document.metadata.remove" => {
                    let key = args["key"].as_str().unwrap();
                    let was = self.metadata.remove(key).is_some();
                    Ok(json!({"removed": if was { vec![key] } else { vec![] }}))
                }
                "assembly.bom" => Ok(json!({"rows": self.rows})),
                "component.links" => Ok(json!({"links": self.links})),
                other => Err(format!("unknown command {other}")),
            }
        }
        fn read_file(&mut self, path: &str) -> Result<Option<Vec<u8>>, String> {
            self.calls.push(("read-file".into(), json!(path)));
            Ok(self.files.get(path).cloned())
        }
        fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
            self.calls.push(("write-file".into(), json!(path)));
            self.files.insert(path.into(), data.to_vec());
            Ok(())
        }
        fn now(&mut self) -> u64 {
            self.clock
        }
    }

    fn linked() -> Fake {
        let metadata = json!({
            "part_number": "TC-1042", "revision": "B", "description": "Motor mount",
            KEY: {"item_number": "TC-1042", "revision": "B", "item_id": "i-1",
                  "revision_id": "0b6f1c2e-aaaa-bbbb-cccc-1234567890ab", "server": "https://plm.example.org/"}
        });
        Fake {
            metadata: metadata.as_object().unwrap().clone(),
            ..Fake::default()
        }
    }

    #[test]
    fn status_of_a_linked_design_names_item_and_page() {
        let s = status(&mut linked()).unwrap();
        assert_eq!(s["managed"], true);
        assert_eq!(s["item_number"], "TC-1042");
        assert_eq!(
            s["url"],
            "https://plm.example.org/revisions/0b6f1c2e-aaaa-bbbb-cccc-1234567890ab"
        );
        assert_eq!(s["part_number_matches"], true);
        let mut changed = linked();
        changed.metadata.insert("revision".into(), json!("C"));
        let s = status(&mut changed).unwrap();
        assert_eq!(s["revision_matches"], false);
        assert!(s["message"].as_str().unwrap().contains("revision “C”"));
    }

    #[test]
    fn status_of_an_unlinked_design_says_how_to_link() {
        let s = status(&mut Fake::default()).unwrap();
        assert_eq!(s["managed"], false);
        assert!(s["message"].as_str().unwrap().contains("Item data"));
    }

    #[test]
    fn no_url_from_an_odd_server_or_id() {
        let mut f = linked();
        f.metadata[KEY]["server"] = json!("javascript:alert(1)");
        assert!(status(&mut f).unwrap().get("url").is_none());
        let mut f = linked();
        f.metadata[KEY]["revision_id"] = json!("../admin");
        assert!(status(&mut f).unwrap().get("url").is_none());
    }

    #[test]
    fn a_clean_part_is_ready() {
        let c = check(&mut linked()).unwrap();
        assert_eq!(c["ready"], true, "{c}");
        assert_eq!(c["summary"], "Ready for TeamCraft.");
    }

    #[test]
    fn check_finds_what_teamcraft_cannot_match() {
        let mut f = linked();
        f.metadata.insert("part_number".into(), json!("TC-9999"));
        f.rows = vec![
            json!({"item": 1, "part_number": "TC-200", "description": "Frame", "definition": "Frame", "bom_type": "normal", "count": 1}),
            json!({"item": "1.1", "part_number": "", "description": "", "definition": "Bolt", "bom_type": "normal", "count": 4}),
            json!({"item": 2, "part_number": "TC-200", "description": "Frame welded", "definition": "Frame2", "bom_type": "normal", "count": 1}),
            json!({"item": 3, "part_number": "", "definition": "Sketch body", "bom_type": "reference", "count": 1}),
            json!({"item": 4, "part_number": "TC-300", "description": "Rail", "definition": "Rail", "bom_type": "normal", "quantity": null, "quantity_unknown": "mixed"}),
        ];
        f.links = vec![
            json!({"component": 3, "name": "Plate", "path": "/home/kim/parts/plate.sc"}),
            json!({"component": 4, "name": "Plate B", "path": "lib/plate.sc"}),
            json!({"component": 5, "name": "Bolt", "path": "bolt.sc"}),
        ];
        let c = check(&mut f).unwrap();
        assert_eq!(c["ready"], false);
        let text: Vec<String> = c["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["message"].as_str().unwrap().to_string())
            .collect();
        let has = |s: &str| text.iter().any(|t| t.contains(s));
        assert!(
            has("“TC-9999” is not the TeamCraft item TC-1042"),
            "{text:?}"
        );
        assert!(has("BOM item 1.1 (Bolt) has no part number"), "{text:?}");
        assert!(has("Part number TC-200 has two descriptions"), "{text:?}");
        assert!(
            !has("Sketch body"),
            "reference components are not BOM lines"
        );
        assert!(has("BOM item 4 (Rail) has a quantity"), "{text:?}");
        assert!(has("by an absolute path"), "{text:?}");
        assert!(has("Two linked designs are named plate.sc"), "{text:?}");
        assert!(
            c["summary"].as_str().unwrap().starts_with("6 thing(s)"),
            "{c}"
        );
    }

    #[test]
    fn link_writes_one_change_and_keeps_known_ids() {
        let mut f = linked();
        let r = link(
            &mut f,
            &json!({"item_number": "TC-1042", "revision": "C", "name": "Motor mount v2"}),
        )
        .unwrap();
        assert_eq!(r["linked"], true);
        let sets: Vec<_> = f
            .calls
            .iter()
            .filter(|(c, _)| c == "document.metadata.set")
            .collect();
        assert_eq!(sets.len(), 1, "one undo step");
        assert_eq!(f.metadata["part_number"], "TC-1042");
        assert_eq!(f.metadata["revision"], "C");
        assert_eq!(f.metadata["description"], "Motor mount v2");
        assert_eq!(f.metadata[KEY]["item_id"], "i-1", "same item: id kept");
        assert!(
            f.metadata[KEY].get("revision_id").is_none(),
            "another revision: its id is gone"
        );
        assert_eq!(f.metadata[KEY]["server"], "https://plm.example.org/");
        // another item: TeamCraft's old ids are not carried over
        link(&mut f, &json!({"item_number": "TC-2000", "revision": "A"})).unwrap();
        assert!(f.metadata[KEY].get("item_id").is_none());
    }

    #[test]
    fn link_refuses_bad_input() {
        let mut f = Fake::default();
        for (params, field) in [
            (json!({"revision": "A"}), "item_number"),
            (json!({"item_number": "X", "revision": ""}), "revision"),
            (json!({"item_number": 5, "revision": "A"}), "item_number"),
            (
                json!({"item_number": "X\nY", "revision": "A"}),
                "item_number",
            ),
            (
                json!({"item_number": "X", "revision": "A", "server": "ftp://x"}),
                "server",
            ),
        ] {
            match link(&mut f, &params) {
                Err(EntryError::Input { field: got, reason }) => {
                    assert_eq!(got, field, "{params}");
                    // SolidCraft shows only the reason: it names the field
                    assert!(reason.starts_with(label(field)), "{reason}");
                }
                other => panic!("{params}: {other:?}"),
            }
        }
        assert!(
            f.calls.iter().all(|(c, _)| c != "document.metadata.set"),
            "nothing written"
        );
    }

    /// No hard, unexplained failure: a refused or failed SolidCraft command
    /// says which command, SolidCraft's reason and what to do.
    #[test]
    fn a_failed_command_is_explained() {
        let mut refused = linked();
        refused.fail = vec![(
            "document.metadata",
            "permission denied: edit command document.metadata from read entry",
        )];
        let Err(EntryError::Command { command, reason }) = status(&mut refused) else {
            panic!("status must fail when the properties cannot be read");
        };
        assert_eq!(command, "document.metadata");
        assert!(reason.contains("permission denied") && reason.contains("matches your SolidCraft"));
        let mut closed = Fake {
            fail: vec![("document.metadata", "no design is open")],
            ..Fake::default()
        };
        let Err(EntryError::Command { reason, .. }) = check(&mut closed) else {
            panic!("check must fail without properties");
        };
        assert!(reason.contains("no design is open") && reason.contains("Nothing in the design"));
        assert!(explain("x", "  ").contains("no reason given"));
        let mut write = linked();
        write.fail = vec![("document.metadata.set", "the design is read-only")];
        let params = json!({"item_number": "TC-1", "revision": "A"});
        assert!(matches!(
            link(&mut write, &params),
            Err(EntryError::Command { reason, .. }) if reason.contains("read-only")
        ));
    }

    /// A BOM or link list SolidCraft cannot give is a warning, never "ready".
    #[test]
    fn unread_bom_and_links_are_not_ready() {
        let mut f = linked();
        f.fail = vec![
            ("assembly.bom", "evaluation failed"),
            ("component.links", "evaluation failed"),
        ];
        let c = check(&mut f).unwrap();
        assert_eq!(c["ready"], false);
        let text = c["findings"].to_string();
        assert!(text.contains("bill of materials was not checked"), "{text}");
        assert!(text.contains("linked designs were not checked"), "{text}");
        // a plain part answers with no rows and no links: ready
        assert_eq!(check(&mut linked()).unwrap()["ready"], true);
    }

    /// TeamCraft data that is not an object is reported, and link repairs it.
    #[test]
    fn damaged_teamcraft_data_is_reported_and_repaired() {
        let mut f = linked();
        f.metadata.insert(KEY.into(), json!("garbage"));
        let s = status(&mut f).unwrap();
        assert_eq!(s["damaged"], true);
        assert!(s["message"].as_str().unwrap().contains("damaged"));
        let c = check(&mut f).unwrap();
        assert_eq!(c["ready"], false);
        let params = json!({"item_number": "TC-1042", "revision": "B"});
        link(&mut f, &params).unwrap();
        assert_eq!(status(&mut f).unwrap()["managed"], true);
    }

    fn heartbeat(at: u64, online: bool) -> Vec<u8> {
        json!({"schema": "teamcraft.solidcraft.desktop", "version": 1,
               "written_at": at, "server_reachable": online})
        .to_string()
        .into_bytes()
    }

    fn state(at: u64) -> Vec<u8> {
        json!({"schema": "teamcraft.solidcraft.state", "version": 1, "written_at": at,
               "item_number": "TC-1042", "revision": "B", "latest_revision": "C",
               "lifecycle": "In work", "checked_out": {"me": true}})
        .to_string()
        .into_bytes()
    }

    /// The entries that worked before the bridge never touch files, so they
    /// keep working without the folder grant.
    #[test]
    fn the_old_entries_need_no_folder() {
        let mut f = linked();
        status(&mut f).unwrap();
        check(&mut f).unwrap();
        link(&mut f, &json!({"item_number": "TC-1042", "revision": "B"})).unwrap();
        unlink(&mut f).unwrap();
        assert!(
            f.calls.iter().all(|(c, _)| !c.ends_with("-file")),
            "{:?}",
            f.calls
        );
    }

    #[test]
    fn live_says_whether_the_desktop_app_runs() {
        let state_file = bridge::state_file("TC-1042", "B");
        // never ran
        let mut f = linked();
        f.clock = 10_000;
        let l = live(&mut f).unwrap();
        assert_eq!(l["desktop_running"], false);
        assert!(l["message"].as_str().unwrap().contains("not running"));
        // runs, shared the state
        f.files
            .insert("desktop.json".into(), heartbeat(9_990, true));
        f.files.insert(state_file.clone(), state(9_900));
        let l = live(&mut f).unwrap();
        assert_eq!(l["live"], true);
        let m = l["message"].as_str().unwrap();
        assert!(
            m.contains("Checked out by you") && m.contains("latest on the server C"),
            "{m}"
        );
        // stopped an hour ago: what it knew then
        f.clock = 10_000 + 3_600;
        let l = live(&mut f).unwrap();
        assert_eq!(l["live"], false);
        assert!(l["message"]
            .as_str()
            .unwrap()
            .starts_with("The TeamCraft desktop app is not running. As of"));
        // offline from the server
        f.clock = 10_000;
        f.files
            .insert("desktop.json".into(), heartbeat(9_990, false));
        assert!(live(&mut f).unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("no connection"));
        // damaged state, newer desktop app
        f.files.insert(state_file.clone(), b"{".to_vec());
        assert!(live(&mut f).unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("cannot be read"));
        f.files.insert(
            state_file,
            br#"{"schema":"teamcraft.solidcraft.state","version":9}"#.to_vec(),
        );
        assert_eq!(live(&mut f).unwrap()["message"], UPDATE_PLUGIN);
        // not linked: no file is read
        let mut plain = Fake::default();
        assert_eq!(live(&mut plain).unwrap()["live"], false);
        assert!(plain.calls.iter().all(|(c, _)| c != "read-file"));
    }

    #[test]
    fn requests_are_left_for_the_desktop_app() {
        let mut f = linked();
        f.clock = 1_700_000_000;
        let r = ask_desktop(&mut f, "check_in").unwrap();
        let file = "request-TC_2D1042.B.1700000000.check_in.json";
        assert_eq!(r["file"], file);
        assert!(r["message"].as_str().unwrap().contains("when it starts"));
        let written: Value = serde_json::from_slice(&f.files[file]).unwrap();
        assert_eq!(written["action"], "check_in");
        assert_eq!(written["item_number"], "TC-1042");
        assert_eq!(written["schema"], "teamcraft.solidcraft.request");
        f.files
            .insert("desktop.json".into(), heartbeat(1_700_000_000 - 5, true));
        let r = ask_desktop(&mut f, "check_out").unwrap();
        assert!(r["message"]
            .as_str()
            .unwrap()
            .starts_with("Sent to the TeamCraft desktop app"));
        // a design without TeamCraft data writes nothing and says why
        let mut plain = Fake::default();
        assert!(matches!(
            ask_desktop(&mut plain, "open"),
            Err(EntryError::Unsupported(m)) if m.contains("not linked")
        ));
        assert!(plain.files.is_empty());
    }

    #[test]
    fn unlink_removes_only_teamcrafts_object() {
        let mut f = linked();
        assert_eq!(unlink(&mut f).unwrap()["unlinked"], true);
        assert!(f.metadata.get(KEY).is_none());
        assert_eq!(f.metadata["part_number"], "TC-1042");
        assert_eq!(unlink(&mut f).unwrap()["unlinked"], false);
    }

    #[test]
    fn unknown_entries_and_the_manifest_agree() {
        assert!(matches!(
            execute(&mut Fake::default(), "nope", &json!({})),
            Err(EntryError::Unsupported(_))
        ));
        let manifest: Value = serde_json::from_str(include_str!("../plugin.json")).unwrap();
        let declared: Vec<&str> = manifest["runtime"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap())
            .collect();
        assert_eq!(declared, ENTRIES);
        let commands: Vec<&str> = manifest["runtime"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        for command in [
            "document.metadata",
            "document.metadata.set",
            "document.metadata.remove",
            "assembly.bom",
            "component.links",
        ] {
            assert!(commands.contains(&command), "{command} not declared");
        }
        assert_eq!(manifest["id"], "teamcraft");
        // A ribbon slot's name is the button's label: four different ones,
        // not four buttons that all read "TeamCraft".
        let entries = manifest["runtime"]["entries"].as_array().unwrap();
        let mut labels: Vec<&str> = entries
            .iter()
            .map(|e| e["slots"][0]["name"].as_str().unwrap())
            .collect();
        assert!(labels.iter().all(|l| *l != "TeamCraft"), "{labels:?}");
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), ENTRIES.len(), "one label per entry");
        // SolidCraft refuses a command it does not class as `read` from a
        // read entry, and it classes document.metadata, assembly.bom and
        // component.links as edit commands. So every entry is `edit`; status
        // and check only read.
        for e in entries {
            assert_eq!(e["effect"], "edit", "{}", e["name"]);
        }
    }
}
