//! The bill of materials of a SolidCraft assembly, read from the design
//! itself (`.sc`, `.solidcraft`; #366) – the same [`BomTable`] a BOM table or
//! a STEP assembly gives the BOM import. Findings and sources:
//! `docs/src/research/solidcraft-design.md`.
//!
//! The design lists its components (`components`: id, name, parent, part
//! properties in `metadata`) and their placements (`occurrences`: which
//! component, placed in which parent component, 0 = the design). One line
//! per component placed in a parent, its quantity the number of placements
//! there, how it counts from its BOM type (the occurrence's, else the
//! component's `bom_type`):
//!
//! * `normal` and `inseparable`: a line, with the lines of what it holds
//!   below it;
//! * `purchased` (and standard parts, `parts`): a line marked *Purchased*,
//!   nothing below it;
//! * `phantom`: no line – what it holds counts as placed in its parent;
//! * `reference`: not counted, nor anything inside it.
//!
//! A plain number in `bom_quantity` (pieces per placement) multiplies the
//! quantity; expressions and lengths are left to SolidCraft's own BOM CSV.
//! Unknown fields are ignored; nothing here depends on the format number.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use serde::Deserialize;

use crate::bomtable::{field_label, BomRow, BomTable, Column, MAX_ROWS};

/// Deepest nesting followed (SolidCraft's own limit is 64 levels).
const MAX_DEPTH: usize = 64;

/// Whether TeamCraft reads a BOM from a file with this name as a SolidCraft
/// design.
pub fn is_design(filename: &str) -> bool {
    filename
        .rsplit_once('.')
        .is_some_and(|(_, ext)| matches!(ext.to_ascii_lowercase().as_str(), "sc" | "solidcraft"))
}

#[derive(Deserialize, Default)]
struct Doc {
    #[serde(default)]
    metadata: HashMap<String, serde_json::Value>,
    #[serde(default)]
    components: Vec<Component>,
    #[serde(default)]
    occurrences: Vec<Occurrence>,
    #[serde(default)]
    links: HashMap<String, Link>,
    #[serde(default)]
    parts: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct Component {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    metadata: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct Occurrence {
    component: u64,
    #[serde(default)]
    parent: u64,
    #[serde(default)]
    bom_type: Option<String>,
}

#[derive(Deserialize)]
struct Link {
    #[serde(default)]
    path: String,
}

fn text(map: &HashMap<String, serde_json::Value>, key: &str) -> String {
    map.get(key)
        .and_then(serde_json::Value::as_str)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Reads the bill of materials of a SolidCraft design (ZIP or JSON).
pub fn read(bytes: &[u8]) -> Result<BomTable, String> {
    let document = crate::scdesign::document(&mut Cursor::new(bytes)).ok_or_else(|| {
        "Not a SolidCraft design: neither a design container nor a JSON design (or its \
         document is larger than 16 MiB)."
            .to_string()
    })?;
    let doc: Doc = serde_json::from_slice(&document)
        .map_err(|e| format!("The SolidCraft design cannot be read: {e}"))?;
    from_doc(&doc)
}

struct Walk<'a> {
    components: HashMap<u64, &'a Component>,
    /// Placements per parent component, in file order: (component, type).
    placed: BTreeMap<u64, Vec<(u64, String)>>,
    standard: std::collections::HashSet<u64>,
    files: HashMap<u64, String>,
    rows: Vec<BomRow>,
    warnings: Vec<String>,
    path: Vec<u64>,
}

impl Walk<'_> {
    fn bom_type(&self, component: u64, occurrence: Option<&str>) -> String {
        let own = self
            .components
            .get(&component)
            .map(|c| text(&c.metadata, "bom_type"))
            .unwrap_or_default();
        let t = occurrence.map(str::to_string).unwrap_or(own);
        if t.is_empty() {
            "normal".into()
        } else {
            t
        }
    }

    /// What `parent` holds, `times` over (phantoms in between), as
    /// (component, type, count) in first-placement order.
    fn contents(&self, parent: u64, times: f64, out: &mut Vec<(u64, String, f64)>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        for (component, t) in self
            .placed
            .get(&parent)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            match t.as_str() {
                "reference" => {}
                "phantom" if !self.path.contains(component) => {
                    self.contents(*component, times, out, depth + 1);
                }
                "phantom" => {}
                _ => match out.iter_mut().find(|(c, ty, _)| c == component && ty == t) {
                    Some(entry) => entry.2 += times,
                    None => out.push((*component, t.clone(), times)),
                },
            }
        }
    }

    fn expand(&mut self, parent: u64, level: usize) -> Result<(), String> {
        if level > MAX_DEPTH {
            self.warnings.push(format!(
                "The assembly is nested deeper than {MAX_DEPTH} levels – the levels below are left out."
            ));
            return Ok(());
        }
        let mut contents = Vec::new();
        self.contents(parent, 1.0, &mut contents, 0);
        for (component, t, count) in contents {
            if self.rows.len() >= MAX_ROWS {
                return Err(format!("The assembly has more than {MAX_ROWS} lines."));
            }
            let Some(c) = self.components.get(&component).copied() else {
                self.warnings.push(format!(
                    "A placement names component {component}, which the design does not have – left out."
                ));
                continue;
            };
            let purchased = t == "purchased" || self.standard.contains(&component);
            let per_piece = text(&c.metadata, "bom_quantity");
            let pieces = match per_piece.parse::<f64>() {
                Ok(n) if n.is_finite() && n > 0.0 => n,
                _ => {
                    if !per_piece.is_empty() {
                        self.warnings.push(format!(
                            "{}: the BOM quantity “{per_piece}” is not a plain number – the placements are counted instead.",
                            c.name
                        ));
                    }
                    1.0
                }
            };
            let mut extra = vec![text(&c.metadata, "revision")];
            extra.push(self.files.get(&component).cloned().unwrap_or_default());
            self.rows.push(BomRow {
                line: self.rows.len() + 1,
                level,
                find_number: None,
                number: text(&c.metadata, "part_number"),
                name: c.name.trim().to_string(),
                description: text(&c.metadata, "description"),
                quantity: count * pieces,
                unit: String::new(),
                material: text(&c.metadata, "material"),
                mass_kg: None,
                structure: if purchased {
                    "Purchased".into()
                } else {
                    String::new()
                },
                extra,
            });
            if !purchased && !self.path.contains(&component) {
                self.path.push(component);
                self.expand(component, level + 1)?;
                self.path.pop();
            } else if self.path.contains(&component) {
                self.warnings
                    .push(format!("{} places itself – not followed.", c.name));
            }
        }
        Ok(())
    }
}

fn from_doc(doc: &Doc) -> Result<BomTable, String> {
    if doc.components.is_empty() || doc.occurrences.is_empty() {
        return Err(
            "The SolidCraft design has no components – it is a part, not an assembly.".into(),
        );
    }
    let mut walk = Walk {
        components: doc.components.iter().map(|c| (c.id, c)).collect(),
        placed: BTreeMap::new(),
        standard: doc.parts.keys().filter_map(|k| k.parse().ok()).collect(),
        files: doc
            .links
            .iter()
            .filter_map(|(id, l)| {
                let name = l.path.rsplit(['/', '\\']).next()?.trim();
                Some((id.parse().ok()?, name.to_string()))
            })
            .filter(|(_, name): &(u64, String)| !name.is_empty())
            .collect(),
        rows: Vec::new(),
        warnings: Vec::new(),
        path: Vec::new(),
    };
    for o in &doc.occurrences {
        let t = walk.bom_type(o.component, o.bom_type.as_deref());
        walk.placed
            .entry(o.parent)
            .or_default()
            .push((o.component, t));
    }
    walk.expand(0, 1)?;
    if walk.rows.is_empty() {
        return Err(
            "Nothing in the SolidCraft assembly counts in a bill of materials (only reference \
             components)."
                .into(),
        );
    }
    let number = text(&doc.metadata, "part_number");
    let description = text(&doc.metadata, "description");
    let root = (!number.is_empty() || !description.is_empty()).then(|| BomRow {
        line: 0,
        level: 0,
        find_number: None,
        number,
        name: description.clone(),
        description,
        quantity: 1.0,
        unit: String::new(),
        material: String::new(),
        mass_kg: None,
        structure: String::new(),
        extra: vec![text(&doc.metadata, "revision"), String::new()],
    });
    let columns = [
        ("part_number", "number"),
        ("name", "name"),
        ("description", "description"),
        ("occurrences", "quantity"),
        ("material", "material"),
        ("bom_type", "structure"),
    ]
    .into_iter()
    .map(|(header, field)| Column {
        header: header.into(),
        field,
        label: field_label(field),
    })
    .collect();
    Ok(BomTable {
        root,
        rows: walk.rows,
        columns,
        extra_columns: vec!["Revision".into(), "Linked design".into()],
        warnings: walk.warnings,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Frame (TC-100): a sub-assembly "Side" placed twice holding two bolts
    /// each, a purchased motor, a phantom group with a cover, a reference
    /// sketch body, a standard nut placed four times, a linked plate.
    const FRAME: &str = r#"{
      "format": "solidcraft/40",
      "metadata": {"part_number": "TC-100", "description": "Frame", "revision": "C"},
      "components": [
        {"id": 1, "name": "Side", "metadata": {"part_number": "TC-110", "description": "Side wall", "revision": "A"}},
        {"id": 2, "name": "Bolt", "parent": 1, "metadata": {"part_number": "ISO 4017 M6x20", "material": "Steel"}},
        {"id": 3, "name": "Motor", "metadata": {"part_number": "M-42", "bom_type": "purchased"}},
        {"id": 4, "name": "Rotor", "parent": 3, "metadata": {"part_number": "never-listed"}},
        {"id": 5, "name": "Group", "metadata": {"bom_type": "phantom"}},
        {"id": 6, "name": "Cover", "parent": 5, "metadata": {"part_number": "TC-120"}},
        {"id": 7, "name": "Sketch body", "metadata": {"bom_type": "reference"}},
        {"id": 8, "name": "Nut M6"},
        {"id": 9, "name": "Plate", "metadata": {"part_number": "TC-130", "bom_quantity": "2"}}
      ],
      "occurrences": [
        {"id": 10, "component": 1, "parent": 0, "name": "Side:1"},
        {"id": 11, "component": 1, "parent": 0, "name": "Side:2"},
        {"id": 12, "component": 2, "parent": 1, "name": "Bolt:1"},
        {"id": 13, "component": 2, "parent": 1, "name": "Bolt:2"},
        {"id": 14, "component": 3, "parent": 0, "name": "Motor:1"},
        {"id": 15, "component": 4, "parent": 3, "name": "Rotor:1"},
        {"id": 16, "component": 5, "parent": 0, "name": "Group:1"},
        {"id": 17, "component": 6, "parent": 5, "name": "Cover:1"},
        {"id": 18, "component": 7, "parent": 0, "name": "Sketch:1"},
        {"id": 19, "component": 8, "parent": 0, "name": "Nut:1"},
        {"id": 20, "component": 8, "parent": 0, "name": "Nut:2"},
        {"id": 21, "component": 8, "parent": 0, "name": "Nut:3"},
        {"id": 22, "component": 8, "parent": 0, "name": "Nut:4"},
        {"id": 23, "component": 9, "parent": 0, "name": "Plate:1"},
        {"id": 24, "component": 1, "parent": 0, "name": "Side:3", "bom_type": "reference"}
      ],
      "links": {"9": {"path": "parts/plate.sc", "feature": 30}},
      "parts": {"8": {"standard": "iso4032", "size": "M6"}}
    }"#;

    fn lines(t: &BomTable) -> Vec<(usize, String, f64, String)> {
        t.rows
            .iter()
            .map(|r| (r.level, r.name.clone(), r.quantity, r.structure.clone()))
            .collect()
    }

    #[test]
    fn the_design_gives_the_lines_a_bom_import_needs() {
        let t = read(FRAME.as_bytes()).unwrap();
        assert_eq!(
            lines(&t),
            vec![
                (1, "Side".into(), 2.0, String::new()),
                (2, "Bolt".into(), 2.0, String::new()),
                (1, "Motor".into(), 1.0, "Purchased".into()),
                (1, "Cover".into(), 1.0, String::new()),
                (1, "Nut M6".into(), 4.0, "Purchased".into()),
                (1, "Plate".into(), 2.0, String::new()),
            ],
            "{:?}",
            t.warnings
        );
        let root = t.root.as_ref().unwrap();
        assert_eq!(
            (root.number.as_str(), root.name.as_str()),
            ("TC-100", "Frame")
        );
        let bolt = &t.rows[1];
        assert_eq!(bolt.number, "ISO 4017 M6x20");
        assert_eq!(bolt.material, "Steel");
        assert_eq!(t.rows[0].extra, vec!["A".to_string(), String::new()]);
        assert_eq!(t.rows[5].extra, vec![String::new(), "plate.sc".to_string()]);
        assert!(t.warnings.is_empty(), "{:?}", t.warnings);
    }

    #[test]
    fn the_same_design_zipped_reads_the_same() {
        let zipped = crate::scdesign::sample::design(FRAME.as_bytes(), None);
        assert_eq!(
            lines(&read(&zipped).unwrap()),
            lines(&read(FRAME.as_bytes()).unwrap())
        );
    }

    #[test]
    fn parts_odd_files_and_cycles_are_reported() {
        let part = r#"{"format":"solidcraft/40","features":[]}"#;
        assert!(read(part.as_bytes()).unwrap_err().contains("is a part"));
        assert!(read(b"hello")
            .unwrap_err()
            .contains("Not a SolidCraft design"));
        // a component placed inside itself (a broken file) does not loop
        let cycle = r#"{"format":"solidcraft/40",
          "components":[{"id":1,"name":"A"},{"id":2,"name":"B"}],
          "occurrences":[{"component":1,"parent":0},{"component":2,"parent":1},{"component":1,"parent":2}]}"#;
        let t = read(cycle.as_bytes()).unwrap();
        assert_eq!(t.rows.len(), 3, "{:?}", lines(&t));
        assert!(t.warnings.iter().any(|w| w.contains("places itself")));
        // a BOM quantity that is an expression is reported, the placements count
        let expr = r#"{"format":"solidcraft/40",
          "components":[{"id":1,"name":"Rail","metadata":{"bom_quantity":"2 * span"}}],
          "occurrences":[{"component":1,"parent":0}]}"#;
        let t = read(expr.as_bytes()).unwrap();
        assert_eq!(t.rows[0].quantity, 1.0);
        assert_eq!(t.warnings.len(), 1);
        assert!(is_design("frame.SC") && is_design("a.solidcraft") && !is_design("a.step"));
    }
}
