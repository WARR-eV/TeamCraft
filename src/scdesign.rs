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
}
