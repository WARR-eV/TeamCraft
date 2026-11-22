//! SolidCraft designs (`.sc`, legacy `.solidcraft`): the ZIP design
//! container and the parts TeamCraft reads from it. Findings and sources in
//! `docs/src/research/solidcraft-design.md`.
//!
//! The format changes with almost every SolidCraft release, so nothing here
//! is an error for the caller: an unknown container version, a missing or
//! oversized entry, a damaged archive or a stale cache is "nothing found"
//! (`None`). Entries are read into memory within fixed limits; nothing is
//! extracted to disk. Only stored and Deflate entries are read, with their
//! size and CRC-32 checked.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};

use sha2::{Digest, Sha256};
use zip::CompressionMethod;

const ZIP_MAGIC: &[u8; 4] = b"PK\x03\x04";
/// SolidCraft's own limit on the number of entries.
const MAX_ENTRIES: usize = 10_000;
/// `container.json` and the thumbnail key are a few dozen bytes.
const MAX_SMALL: u64 = 4096;
/// SolidCraft caps the thumbnail at 256 KiB.
const MAX_THUMBNAIL: u64 = 256 * 1024;
/// Largest `document.json` read (what SolidCraft allows for previews).
pub const MAX_DOCUMENT: u64 = 16 * 1024 * 1024;
/// Container formats of designs (`solvecraft-` from early drafts).
const CONTAINER_FORMATS: [&str; 2] = ["solidcraft-container/", "solvecraft-container/"];
/// The thumbnail key's format this reader knows.
const THUMBNAIL_FORMAT: &str = "solidcraft-thumbnail/1";

/// Whether `head` (the first bytes of a file) starts a ZIP archive.
pub fn is_zip(head: &[u8]) -> bool {
    head.starts_with(ZIP_MAGIC)
}

/// An opened SolidCraft design container.
pub struct Container<R> {
    zip: zip::ZipArchive<R>,
}

impl<R: Read + Seek> Container<R> {
    /// Opens a design container; `None` when the file is not a ZIP, has too
    /// many entries, or its `container.json` does not name a design
    /// container (a drawing container, another program's ZIP).
    pub fn open(reader: R) -> Option<Self> {
        let zip = zip::ZipArchive::new(reader).ok()?;
        if zip.len() > MAX_ENTRIES {
            return None;
        }
        let mut container = Container { zip };
        let marker = container.read("container.json", MAX_SMALL)?;
        let marker: serde_json::Value = serde_json::from_slice(&marker).ok()?;
        let format = marker.get("format")?.as_str()?;
        let content = marker.get("content").and_then(serde_json::Value::as_str);
        let design = CONTAINER_FORMATS.iter().any(|f| format.starts_with(f))
            && content.is_none_or(|c| c == "design");
        design.then_some(container)
    }

    /// The content of the entry `name`, if it exists, is at most `limit`
    /// bytes, is stored or Deflate-compressed and matches its size and CRC.
    pub fn read(&mut self, name: &str, limit: u64) -> Option<Vec<u8>> {
        let mut file = self.zip.by_name(name).ok()?;
        let method = file.compression();
        if method != CompressionMethod::Stored && method != CompressionMethod::Deflated {
            return None;
        }
        let size = file.size();
        let crc = file.crc32();
        if size > limit {
            return None;
        }
        let mut data = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        (&mut file)
            .take(size.saturating_add(1))
            .read_to_end(&mut data)
            .ok()?;
        (data.len() as u64 == size && crc32fast::hash(&data) == crc).then_some(data)
    }

    /// The embedded picture (`cache/thumbnail.png`), when its key
    /// (`cache/thumbnail.json`) has a known format and names the SHA-256 of
    /// both this exact `document.json` and the PNG. A picture left from an
    /// older document (another tool rewrote the document) is not shown.
    pub fn thumbnail(&mut self) -> Option<Vec<u8>> {
        let key = self.read("cache/thumbnail.json", MAX_SMALL)?;
        let key: serde_json::Value = serde_json::from_slice(&key).ok()?;
        let field = |name: &str| key.get(name).and_then(serde_json::Value::as_str);
        if field("format") != Some(THUMBNAIL_FORMAT) {
            return None;
        }
        let png = self.read("cache/thumbnail.png", MAX_THUMBNAIL)?;
        if !field("payload").is_some_and(|d| same_digest(d, &png)) {
            return None;
        }
        let document = self.read("document.json", MAX_DOCUMENT)?;
        field("document")
            .is_some_and(|d| same_digest(d, &document))
            .then_some(png)
    }
}

/// Whether `hex_digest` is the SHA-256 of `data` (hex, either case).
fn same_digest(hex_digest: &str, data: &[u8]) -> bool {
    hex_digest.eq_ignore_ascii_case(&hex::encode(Sha256::digest(data)))
}

/// Whether `head` (the first bytes of a file) starts a plain-JSON design
/// (`"format": "solidcraft/N"`, early drafts `solvecraft/N`).
pub fn is_json_design(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    text.trim_start().starts_with('{')
        && (text.contains("\"solidcraft/") || text.contains("\"solvecraft/"))
}

/// The parametric document of a design file: `document.json` of a ZIP
/// design container, or the whole file when it is a plain-JSON design.
/// `None` for any other file and for a document larger than
/// [`MAX_DOCUMENT`]. Reads from the start of `file`.
pub fn document<R: Read + Seek>(file: &mut R) -> Option<Vec<u8>> {
    file.rewind().ok()?;
    let mut head = Vec::new();
    file.by_ref().take(1024).read_to_end(&mut head).ok()?;
    file.rewind().ok()?;
    if is_zip(&head) {
        return Container::open(&mut *file)?.read("document.json", MAX_DOCUMENT);
    }
    if !is_json_design(&head) {
        return None;
    }
    let mut data = Vec::new();
    file.by_ref()
        .take(MAX_DOCUMENT.saturating_add(1))
        .read_to_end(&mut data)
        .ok()?;
    (data.len() as u64 <= MAX_DOCUMENT).then_some(data)
}

/// Longest file name taken from a link (as `cadrefs` accepts).
const MAX_NAME: usize = 160;

/// The designs an assembly links (*Insert Design*, `links` of the
/// document): the file name each link's path ends in, with how often the
/// components linked to it are placed – each placement is an occurrence; a
/// component without one counts once. Several links to one file add up.
/// Standard parts (`parts`) are no files and are not listed. `None` when
/// `document` is not a readable document.
pub fn linked_designs(document: &[u8]) -> Option<BTreeMap<String, u32>> {
    #[derive(serde::Deserialize)]
    struct Doc {
        #[serde(default)]
        links: HashMap<String, Link>,
        #[serde(default)]
        occurrences: Vec<Occurrence>,
    }
    #[derive(serde::Deserialize)]
    struct Link {
        #[serde(default)]
        path: String,
    }
    #[derive(serde::Deserialize)]
    struct Occurrence {
        #[serde(default)]
        component: Option<u64>,
    }
    let doc: Doc = serde_json::from_slice(document).ok()?;
    let mut placed: HashMap<u64, u32> = HashMap::new();
    for component in doc.occurrences.iter().filter_map(|o| o.component) {
        let n = placed.entry(component).or_default();
        *n = n.saturating_add(1);
    }
    let mut found: BTreeMap<String, u32> = BTreeMap::new();
    for (id, link) in &doc.links {
        let Some(name) = link
            .path
            .rsplit(['/', '\\'])
            .next()
            .map(str::trim)
            .filter(|n| !n.is_empty() && n.len() <= MAX_NAME)
        else {
            continue;
        };
        let count = id
            .parse::<u64>()
            .ok()
            .and_then(|id| placed.get(&id).copied())
            .unwrap_or(1);
        let total = found.entry(name.to_string()).or_default();
        *total = total.saturating_add(count);
    }
    Some(found)
}

/// Document formats of designs (`solvecraft/` from early drafts). Only the
/// prefix is checked: the number changes with almost every release.
const DOCUMENT_FORMATS: [&str; 2] = ["solidcraft/", "solvecraft/"];
/// Longest text kept of one design property, in characters (SolidCraft
/// allows 4 KiB; a part number or a description is far shorter).
pub const MAX_PROPERTY_CHARS: usize = 500;

/// The design properties SolidCraft keeps in the `metadata` object of the
/// document (*Design properties*): text values only, trimmed, at most
/// [`MAX_PROPERTY_CHARS`] each. Read from a user's file: data, never
/// instructions.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DesignProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// The known keys of [`DesignProperties`] with their labels, in the order
/// they are shown.
pub const PROPERTY_KEYS: [(&str, &str); 7] = [
    ("part_number", "Part number"),
    ("revision", "Revision"),
    ("description", "Description"),
    ("project", "Project"),
    ("author", "Author"),
    ("organization", "Organization"),
    ("license", "License"),
];

/// A design property that differs from the item it is attached to.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Mismatch {
    /// Key of the property (`part_number`, `revision`, `description`).
    pub key: &'static str,
    /// What the design calls it (`Part number`).
    pub label: &'static str,
    /// The value in the design.
    pub file: String,
    /// What it is compared with on the item (`item number`).
    pub item_label: &'static str,
    /// The item's value.
    pub item: String,
}

impl DesignProperties {
    fn slot(&mut self, key: &str) -> Option<&mut Option<String>> {
        Some(match key {
            "project" => &mut self.project,
            "part_number" => &mut self.part_number,
            "revision" => &mut self.revision,
            "author" => &mut self.author,
            "organization" => &mut self.organization,
            "description" => &mut self.description,
            "license" => &mut self.license,
            _ => return None,
        })
    }

    /// The value of a known key (see [`PROPERTY_KEYS`]).
    pub fn get(&self, key: &str) -> Option<&str> {
        match key {
            "project" => self.project.as_deref(),
            "part_number" => self.part_number.as_deref(),
            "revision" => self.revision.as_deref(),
            "author" => self.author.as_deref(),
            "organization" => self.organization.as_deref(),
            "description" => self.description.as_deref(),
            "license" => self.license.as_deref(),
            _ => None,
        }
    }

    /// Whether no property is set.
    pub fn is_empty(&self) -> bool {
        PROPERTY_KEYS.iter().all(|(key, _)| self.get(key).is_none())
    }

    /// The set properties as (label, value), in [`PROPERTY_KEYS`] order.
    pub fn entries(&self) -> Vec<(&'static str, &str)> {
        PROPERTY_KEYS
            .iter()
            .filter_map(|(key, label)| self.get(key).map(|v| (*label, v)))
            .collect()
    }

    /// One line for file lists: `Part number BR-1 · Revision B · Bracket`.
    pub fn line(&self) -> String {
        self.entries()
            .into_iter()
            .map(|(label, value)| format!("{label} {value}"))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// The values for the full-text search, one per line.
    pub fn search_text(&self) -> String {
        self.entries()
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The properties as stored in `file_versions.properties`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }

    /// The properties stored in `file_versions.properties`; `None` for an
    /// empty object or anything else.
    pub fn from_json(value: &serde_json::Value) -> Option<DesignProperties> {
        let mut props = DesignProperties::default();
        for (key, value) in value.as_object()? {
            if let (Some(slot), Some(text)) = (props.slot(key), value.as_str()) {
                let text = clean_property(text);
                if !text.is_empty() {
                    *slot = Some(text);
                }
            }
        }
        (!props.is_empty()).then_some(props)
    }

    /// Where the design differs from the item revision it is attached to:
    /// the part number from the item number, the revision from the
    /// revision code, the description from the revision's name (and its
    /// description). Case and runs of spaces do not count; a property the
    /// design does not set is no difference. Nothing is changed – the
    /// result is a hint for the user.
    pub fn mismatches(
        &self,
        item_number: &str,
        revision_code: &str,
        name: &str,
        description: &str,
    ) -> Vec<Mismatch> {
        let mut found = Vec::new();
        let mut check = |key: &'static str,
                         label: &'static str,
                         item_label: &'static str,
                         item: &str,
                         also: &str| {
            let Some(value) = self.get(key) else {
                return;
            };
            if same_text(value, item) || (!also.is_empty() && same_text(value, also)) {
                return;
            }
            found.push(Mismatch {
                key,
                label,
                file: value.to_string(),
                item_label,
                item: item.to_string(),
            });
        };
        check("part_number", "Part number", "item number", item_number, "");
        check("revision", "Revision", "revision", revision_code, "");
        check("description", "Description", "name", name, description);
        found
    }
}

/// Whether two texts are equal apart from case and runs of white space.
fn same_text(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

/// A property value as kept: control characters as spaces, trimmed, at
/// most [`MAX_PROPERTY_CHARS`] characters.
fn clean_property(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_PROPERTY_CHARS)
        .collect();
    text.trim().to_string()
}

/// The design properties of a document (see [`document`]): the known text
/// keys of its `metadata` object. Unknown keys (another tool's data under a
/// reverse-DNS key) and values that are not text are left out. `None` when
/// `document` is no design document (malformed JSON, another `format`),
/// larger than [`MAX_DOCUMENT`], has no `metadata` object or no known
/// property set – never an error.
pub fn properties(document: &[u8]) -> Option<DesignProperties> {
    #[derive(serde::Deserialize)]
    struct Doc {
        #[serde(default)]
        format: Option<serde_json::Value>,
        #[serde(default)]
        metadata: Option<serde_json::Value>,
    }
    if document.len() as u64 > MAX_DOCUMENT {
        return None;
    }
    let doc: Doc = serde_json::from_slice(document).ok()?;
    let format = doc.format.as_ref()?.as_str()?;
    if !DOCUMENT_FORMATS.iter().any(|f| format.starts_with(f)) {
        return None;
    }
    DesignProperties::from_json(doc.metadata.as_ref()?)
}

/// Builders for small design containers, for the tests and the fuzz corpus.
#[doc(hidden)]
pub mod sample {
    use std::io::{Cursor, Write};

    use sha2::{Digest, Sha256};

    /// A ZIP of `entries` (name, content), Deflate-compressed.
    pub fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            if writer.start_file(*name, options).is_err() || writer.write_all(data).is_err() {
                return Vec::new();
            }
        }
        writer.finish().map(Cursor::into_inner).unwrap_or_default()
    }

    /// The thumbnail key SolidCraft writes for `png` beside `document`.
    pub fn thumbnail_key(document: &[u8], png: &[u8]) -> Vec<u8> {
        let digest = |d: &[u8]| hex::encode(Sha256::digest(d));
        serde_json::json!({
            "format": "solidcraft-thumbnail/1",
            "app": "0.46.1",
            "document": digest(document),
            "payload": digest(png),
        })
        .to_string()
        .into_bytes()
    }

    /// A design container with `document` and, if given, a thumbnail.
    pub fn design(document: &[u8], png: Option<&[u8]>) -> Vec<u8> {
        let container: &[u8] = br#"{"format":"solidcraft-container/1"}"#;
        match png {
            Some(png) => {
                let key = thumbnail_key(document, png);
                zip(&[
                    ("document.json", document),
                    ("container.json", container),
                    ("cache/thumbnail.json", &key),
                    ("cache/thumbnail.png", png),
                ])
            }
            None => zip(&[("document.json", document), ("container.json", container)]),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const DOC: &[u8] = br#"{"format":"solidcraft/35","metadata":{}}"#;

    fn thumbnail(bytes: Vec<u8>) -> Option<Vec<u8>> {
        Container::open(Cursor::new(bytes))?.thumbnail()
    }

    #[test]
    fn the_keyed_thumbnail_is_found() {
        let png = crate::thumbnail::tests::png(240, 150);
        assert_eq!(thumbnail(sample::design(DOC, Some(&png))), Some(png));
    }

    #[test]
    fn no_thumbnail_is_none() {
        assert_eq!(thumbnail(sample::design(DOC, None)), None);
    }

    #[test]
    fn a_picture_of_another_document_is_not_shown() {
        let png = crate::thumbnail::tests::png(240, 150);
        let key = sample::thumbnail_key(b"{\"older\":1}", &png);
        let bytes = sample::zip(&[
            ("document.json", DOC),
            ("container.json", br#"{"format":"solidcraft-container/1"}"#),
            ("cache/thumbnail.json", &key),
            ("cache/thumbnail.png", &png),
        ]);
        assert_eq!(thumbnail(bytes), None);
    }

    #[test]
    fn a_changed_picture_or_unknown_key_is_not_shown() {
        let png = crate::thumbnail::tests::png(240, 150);
        let other = crate::thumbnail::tests::png(16, 16);
        let key = sample::thumbnail_key(DOC, &png);
        let container: &[u8] = br#"{"format":"solidcraft-container/1"}"#;
        let swapped = sample::zip(&[
            ("document.json", DOC),
            ("container.json", container),
            ("cache/thumbnail.json", &key),
            ("cache/thumbnail.png", &other),
        ]);
        assert_eq!(thumbnail(swapped), None);
        let newer = String::from_utf8(key)
            .unwrap()
            .replace("solidcraft-thumbnail/1", "solidcraft-thumbnail/2");
        let bytes = sample::zip(&[
            ("document.json", DOC),
            ("container.json", container),
            ("cache/thumbnail.json", newer.as_bytes()),
            ("cache/thumbnail.png", &png),
        ]);
        assert_eq!(thumbnail(bytes), None);
    }

    #[test]
    fn only_design_containers_open() {
        let doc =
            |marker: &[u8]| sample::zip(&[("document.json", DOC), ("container.json", marker)]);
        assert!(
            Container::open(Cursor::new(doc(br#"{"format":"solidcraft-container/1"}"#))).is_some()
        );
        assert!(
            Container::open(Cursor::new(doc(br#"{"format":"solvecraft-container/1"}"#))).is_some()
        );
        assert!(Container::open(Cursor::new(doc(br#"{"format":"other/1"}"#))).is_none());
        assert!(Container::open(Cursor::new(doc(
            br#"{"content":"drawing","format":"solidcraft-container/1"}"#
        )))
        .is_none());
        assert!(Container::open(Cursor::new(sample::zip(&[("document.json", DOC)]))).is_none());
        assert!(Container::open(Cursor::new(b"PK\x03\x04garbage".to_vec())).is_none());
    }

    #[test]
    fn a_truncated_or_oversized_entry_is_none() {
        let png = crate::thumbnail::tests::png(240, 150);
        let bytes = sample::design(DOC, Some(&png));
        for cut in [bytes.len() / 2, bytes.len() - 10] {
            assert_eq!(thumbnail(bytes[..cut].to_vec()), None, "cut at {cut}");
        }
        let big = vec![0u8; MAX_SMALL as usize + 1];
        let bytes = sample::zip(&[
            ("document.json", DOC),
            ("container.json", br#"{"format":"solidcraft-container/1"}"#),
            ("cache/thumbnail.json", &big),
            ("cache/thumbnail.png", &png),
        ]);
        assert_eq!(thumbnail(bytes), None);
    }

    /// An assembly linking `plate.sc` (placed three times) and `bolt.sc`
    /// through two components (one placed twice, one without placement),
    /// and a standard part, which is no file.
    const ASSEMBLY: &str = r#"{
        "format": "solidcraft/40",
        "components": [{"id": 1, "name": "Plate"}, {"id": 2, "name": "Bolt"},
                       {"id": 3, "name": "Bolt long"}, {"id": 4, "name": "Nut"}],
        "occurrences": [
            {"id": 10, "component": 1, "name": "Plate:1"},
            {"id": 11, "component": 1, "name": "Plate:2"},
            {"id": 12, "component": 1, "name": "Plate:3"},
            {"id": 13, "component": 2, "name": "Bolt:1"},
            {"id": 14, "component": 2, "name": "Bolt:2"},
            {"id": 15, "component": 4, "name": "Nut:1"}
        ],
        "links": {
            "1": {"path": "plate.sc", "feature": 7},
            "2": {"path": "../lib/bolt.sc", "feature": 8, "configuration": 3},
            "3": {"path": "C:\\parts\\lib\\bolt.sc", "feature": 9}
        },
        "parts": {"4": {"standard": "iso4032", "size": "M5"}},
        "unknown_future_field": [1, 2, 3]
    }"#;

    #[test]
    fn linked_designs_are_counted_by_placement() {
        let links = linked_designs(ASSEMBLY.as_bytes()).unwrap();
        assert_eq!(
            links.into_iter().collect::<Vec<_>>(),
            vec![("bolt.sc".to_string(), 3), ("plate.sc".to_string(), 3)]
        );
        assert_eq!(
            linked_designs(DOC),
            Some(BTreeMap::new()),
            "a part links nothing"
        );
        assert_eq!(linked_designs(br#"{"links": 5}"#), None);
        assert_eq!(linked_designs(b"not json"), None);
    }

    #[test]
    fn the_document_comes_from_either_encoding() {
        let json = ASSEMBLY.as_bytes().to_vec();
        assert_eq!(document(&mut Cursor::new(json.clone())), Some(json.clone()));
        let zipped = sample::design(&json, None);
        assert_eq!(document(&mut Cursor::new(zipped)), Some(json));
        let other = br#"{"format":"other/1"}"#.to_vec();
        assert_eq!(document(&mut Cursor::new(other)), None);
        assert_eq!(
            document(&mut Cursor::new(sample::zip(&[("a.txt", b"x")]))),
            None
        );
        assert_eq!(document(&mut Cursor::new(Vec::new())), None);
    }

    /// A part with every known property, another tool's data under a
    /// reverse-DNS key and values that are not text.
    const PROPERTIES: &str = r#"{
        "format": "solidcraft/46",
        "metadata": {
            "project": " Rover 2027 ",
            "part_number": "BR-7731",
            "revision": "B",
            "author": "Erika",
            "organization": "Workshop",
            "description": "Mounting   bracket\nleft",
            "license": "CC-BY-4.0",
            "org.example.tool": {"part_number": "ignored"},
            "material": "AlMg3",
            "unknown": "ignored"
        },
        "features": []
    }"#;

    #[test]
    fn properties_come_from_json_and_zip_designs() {
        let expected = DesignProperties {
            project: Some("Rover 2027".into()),
            part_number: Some("BR-7731".into()),
            revision: Some("B".into()),
            author: Some("Erika".into()),
            organization: Some("Workshop".into()),
            description: Some("Mounting   bracket left".into()),
            license: Some("CC-BY-4.0".into()),
        };
        assert_eq!(properties(PROPERTIES.as_bytes()), Some(expected.clone()));
        let zipped = sample::design(PROPERTIES.as_bytes(), None);
        let document = document(&mut Cursor::new(zipped)).unwrap();
        assert_eq!(properties(&document), Some(expected.clone()));
        assert_eq!(
            expected.line(),
            "Part number BR-7731 · Revision B · Description Mounting   bracket left · \
             Project Rover 2027 · Author Erika · Organization Workshop · License CC-BY-4.0"
        );
        assert!(expected.search_text().contains("BR-7731\nB\n"));
        assert_eq!(
            DesignProperties::from_json(&expected.to_json()),
            Some(expected)
        );
    }

    #[test]
    fn missing_metadata_or_another_document_has_no_properties() {
        assert_eq!(properties(DOC), None, "empty metadata");
        assert_eq!(properties(br#"{"format":"solidcraft/46"}"#), None);
        assert_eq!(
            properties(br#"{"format":"solidcraft/46","metadata":[1]}"#),
            None
        );
        assert_eq!(
            properties(br#"{"format":"other/1","metadata":{"part_number":"X"}}"#),
            None,
            "another program's JSON"
        );
        assert_eq!(properties(br#"{"metadata":{"part_number":"X"}}"#), None);
        assert_eq!(properties(b"not json"), None);
        assert_eq!(properties(b""), None);
        assert_eq!(
            properties(br#"{"format":"solvecraft/30","metadata":{"part_number":"X"}}"#)
                .and_then(|p| p.part_number),
            Some("X".into()),
            "early drafts"
        );
        assert_eq!(DesignProperties::from_json(&serde_json::json!({})), None);
        assert_eq!(DesignProperties::from_json(&serde_json::json!(null)), None);
    }

    #[test]
    fn values_that_are_not_text_are_left_out() {
        let doc = br#"{"format":"solidcraft/46","metadata":{
            "part_number": 7731, "revision": true, "author": null,
            "organization": ["a"], "project": {"x": 1}, "license": "   ",
            "description": "Plate"}}"#;
        assert_eq!(
            properties(doc),
            Some(DesignProperties {
                description: Some("Plate".into()),
                ..DesignProperties::default()
            })
        );
        let only_numbers = br#"{"format":"solidcraft/46","metadata":{"part_number":1}}"#;
        assert_eq!(properties(only_numbers), None);
    }

    #[test]
    fn long_values_are_cut_and_oversized_documents_refused() {
        let long = "x".repeat(MAX_PROPERTY_CHARS * 3);
        let doc = format!(r#"{{"format":"solidcraft/46","metadata":{{"description":"{long}"}}}}"#);
        let cut = properties(doc.as_bytes()).unwrap().description.unwrap();
        assert_eq!(cut.chars().count(), MAX_PROPERTY_CHARS);
        let mut big =
            br#"{"format":"solidcraft/46","metadata":{"part_number":"X"},"pad":""#.to_vec();
        big.resize(MAX_DOCUMENT as usize + 1, b' ');
        big.extend_from_slice(b"\"}");
        assert_eq!(properties(&big), None);
        // a plain-JSON design above the limit has no document at all
        assert_eq!(document(&mut Cursor::new(big)), None);
    }

    #[test]
    fn mismatches_name_what_differs_from_the_item() {
        let props = properties(PROPERTIES.as_bytes()).unwrap();
        assert_eq!(
            props.mismatches("br-7731", "b", "Mounting bracket LEFT", ""),
            Vec::new(),
            "case and spaces do not count"
        );
        let found = props.mismatches("P-000012", "A", "Holder", "");
        assert_eq!(
            found.iter().map(|m| m.key).collect::<Vec<_>>(),
            vec!["part_number", "revision", "description"]
        );
        assert_eq!(found[0].file, "BR-7731");
        assert_eq!(found[0].item, "P-000012");
        assert!(
            props
                .mismatches("BR-7731", "B", "bracket", "Mounting bracket left")
                .is_empty(),
            "the description may also match the revision's description"
        );
        let only_number = DesignProperties {
            part_number: Some("X-1".into()),
            ..DesignProperties::default()
        };
        assert!(only_number
            .mismatches("X-1", "C", "Anything", "")
            .is_empty());
    }
}
