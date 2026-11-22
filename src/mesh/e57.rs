//! E57 point clouds (ASTM E2807), the exchange format of laser scanners.
//!
//! The whole file is cut into 1024-byte pages, each ending in a CRC-32C of
//! its 1020 data bytes; every checksum is verified before anything else is
//! read, so a damaged file is refused instead of shown as a wrong cloud. The
//! XML section describes the scans (`data3D`); each scan's points sit in a
//! binary section of packets with one bit-packed byte stream per field.
//! Every scan is moved by its pose, metres become millimetres, and large
//! clouds are thinned evenly over all scans. Format notes and sources:
//! `docs/src/research/e57-preview.md`.

use anyhow::{bail, Context, Result};

use super::*;

/// Physical page size (the only one the standard defines).
const PAGE: usize = 1024;
/// Data bytes of a page (before its checksum).
const PAGE_DATA: usize = PAGE - 4;
/// The XML section is read whole: at most this many bytes.
const MAX_XML: u64 = 64 * 1024 * 1024;
/// Elements of the XML tree at most.
const MAX_NODES: usize = 1_000_000;
/// Nesting depth of the XML tree at most.
const MAX_DEPTH: usize = 64;
/// Characters of text kept per element (numbers are short).
const MAX_TEXT: usize = 256;
/// Scans (`data3D` entries) at most.
const MAX_SCANS: usize = 4096;
/// Fields of a scan's prototype at most.
const MAX_PROTOTYPE_FIELDS: usize = 256;
/// E57 stores metres; the viewer shows millimetres.
const MM_PER_M: f64 = 1000.0;
/// Colour of points without one in a cloud where others have one.
const NEUTRAL: [u8; 3] = [176, 188, 200];

pub(super) fn read_e57(data: &[u8]) -> Result<Mesh> {
    read_within(data, MAX_POINTS)
}

/// [`read_e57`] with a point budget (tests use a small one).
fn read_within(data: &[u8], budget: usize) -> Result<Mesh> {
    if data.get(..8) != Some(b"ASTM-E57".as_slice()) {
        bail!("Not an E57 file.");
    }
    let major = u32_le(data, 8)?;
    if major != 1 {
        bail!("E57 version {major} is not previewed (only version 1).");
    }
    let length = u64_le(data, 16)?;
    let xml_offset = u64_le(data, 24)?;
    let xml_length = u64_le(data, 32)?;
    let page_size = u64_le(data, 40)?;
    if page_size != PAGE as u64 {
        bail!("The E57 file has pages of {page_size} bytes – the standard defines 1024.");
    }
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    if length > data.len() {
        return Err(ends_early(data.len()));
    }
    if length == 0 || length % PAGE != 0 {
        bail!("The E57 file length ({length} bytes) is not a whole number of pages.");
    }
    let data = range(data, 0, length)?;
    check_pages(data)?;
    let paged = Paged { data };

    if xml_length > MAX_XML {
        bail!("The E57 XML section is too large to preview ({xml_length} bytes).");
    }
    let xml = paged.read(Paged::logical(xml_offset)?, xml_length as usize)?;
    let tree = parse_xml(&xml)?;
    let root = 0;
    if tree.node(root).map(|n| n.name.as_str()) != Some("e57Root") {
        bail!("The E57 XML section has no e57Root.");
    }
    let scan_nodes: Vec<usize> = tree
        .child(root, "data3D")
        .and_then(|d| tree.node(d))
        .map(|d| d.children.clone())
        .unwrap_or_default();
    if scan_nodes.is_empty() {
        bail!("The E57 file contains no 3D scan (only images or nothing).");
    }
    if scan_nodes.len() > MAX_SCANS {
        bail!(
            "The E57 file has {} scans – the preview reads at most {MAX_SCANS}.",
            scan_nodes.len()
        );
    }
    let mut scans = scan_nodes
        .iter()
        .map(|&n| Scan::from_xml(&tree, n))
        .collect::<Result<Vec<_>>>()?;

    // evenly over all scans, by the point counts the XML gives
    let total = scans
        .iter()
        .fold(0u64, |sum, s| sum.saturating_add(s.records));
    let stride = stride_within(usize::try_from(total).unwrap_or(usize::MAX), budget) as u64;
    let mut cloud = Cloud::default();
    let mut seen = 0u64;
    for scan in &mut scans {
        scan.read_points(&paged, stride, &mut seen, &mut cloud)?;
    }
    Ok(cloud.into_mesh(scans.len(), (stride > 1).then_some(total)))
}

// ------------------------------------------------------------------ pages

fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0x82F6_3B78
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    table
}

/// CRC-32C (Castagnoli) of `data`.
fn crc32c(table: &[u32; 256], data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        let i = usize::from((crc as u8) ^ b);
        crc = (crc >> 8) ^ table.get(i).copied().unwrap_or(0);
    }
    !crc
}

/// Verifies the checksum of every page. libE57 writes it big endian (the
/// standard says little endian for numbers); both orders are accepted.
fn check_pages(data: &[u8]) -> Result<()> {
    let table = crc_table();
    for (n, page) in data.chunks_exact(PAGE).enumerate() {
        let (body, sum) = page.split_at(PAGE_DATA);
        let crc = crc32c(&table, body);
        if sum != crc.to_be_bytes() && sum != crc.to_le_bytes() {
            bail!(
                "The E57 file is damaged: the checksum of page {} (bytes {}–{}) does not match, \
                 so no preview is shown.",
                n + 1,
                n * PAGE,
                n * PAGE + PAGE - 1
            );
        }
    }
    Ok(())
}

/// Writes the checksum of every whole page (the tests' writer and the fuzz
/// harness, which would otherwise never get past the checksums).
pub(crate) fn seal_pages(data: &mut [u8]) {
    let table = crc_table();
    for page in data.chunks_exact_mut(PAGE) {
        let (body, sum) = page.split_at_mut(PAGE_DATA);
        sum.copy_from_slice(&crc32c(&table, body).to_be_bytes());
    }
}

/// The file's logical bytes: the pages without their checksums.
struct Paged<'a> {
    /// Whole pages, checksums verified.
    data: &'a [u8],
}

impl Paged<'_> {
    fn logical_len(&self) -> u64 {
        (self.data.len() / PAGE * PAGE_DATA) as u64
    }

    /// The logical offset of a physical one (offsets in the file are
    /// physical, lengths logical).
    fn logical(physical: u64) -> Result<u64> {
        let (page, at) = (physical / PAGE as u64, physical % PAGE as u64);
        if at >= PAGE_DATA as u64 {
            bail!("The E57 file points into a page checksum (offset {physical}).");
        }
        Ok(page * PAGE_DATA as u64 + at)
    }

    /// Appends `len` logical bytes from `from` to `out`.
    fn read_into(&self, from: u64, len: usize, out: &mut Vec<u8>) -> Result<()> {
        let end = from
            .checked_add(len as u64)
            .filter(|end| *end <= self.logical_len())
            .ok_or_else(|| ends_early(usize::try_from(from).unwrap_or(usize::MAX)))?;
        let mut at = from;
        while at < end {
            let page = (at / PAGE_DATA as u64) as usize;
            let offset = (at % PAGE_DATA as u64) as usize;
            let take = ((end - at) as usize).min(PAGE_DATA - offset);
            let start = page * PAGE + offset;
            out.extend_from_slice(range(self.data, start, start + take)?);
            at += take as u64;
        }
        Ok(())
    }

    fn read(&self, from: u64, len: usize) -> Result<Vec<u8>> {
        if from.saturating_add(len as u64) > self.logical_len() {
            return Err(ends_early(usize::try_from(from).unwrap_or(usize::MAX)));
        }
        let mut out = Vec::with_capacity(len);
        self.read_into(from, len, &mut out)?;
        Ok(out)
    }
}

// ------------------------------------------------------------------ XML

#[derive(Default)]
struct Node {
    /// Local element name.
    name: String,
    /// The `type` attribute (`Structure`, `Float`, `Integer` …).
    kind: String,
    /// The attributes the reader uses.
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<usize>,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim())
    }

    fn attr_f64(&self, name: &str) -> Option<f64> {
        self.attr(name)?
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
    }

    fn attr_i64(&self, name: &str) -> Option<i64> {
        self.attr(name)?.parse().ok()
    }

    fn attr_u64(&self, name: &str) -> Option<u64> {
        self.attr(name)?.parse().ok()
    }
}

/// The XML tree; node 0 is the root.
struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn node(&self, i: usize) -> Option<&Node> {
        self.nodes.get(i)
    }

    fn child(&self, i: usize, name: &str) -> Option<usize> {
        self.node(i)?
            .children
            .iter()
            .copied()
            .find(|&c| self.node(c).is_some_and(|n| n.name == name))
    }

    fn path(&self, i: usize, path: &[&str]) -> Option<usize> {
        path.iter().try_fold(i, |at, name| self.child(at, name))
    }

    /// The value of an `Integer`, `ScaledInteger` or `Float` element (an
    /// empty one is 0, as the standard says).
    fn number(&self, i: usize) -> Option<f64> {
        let n = self.node(i)?;
        let text = n.text.trim();
        let value = match n.kind.as_str() {
            "Integer" if text.is_empty() => 0.0,
            "Integer" => text.parse::<i64>().ok()? as f64,
            "ScaledInteger" => {
                let raw = if text.is_empty() {
                    0
                } else {
                    text.parse::<i64>().ok()?
                };
                raw as f64 * n.attr_f64("scale").unwrap_or(1.0)
                    + n.attr_f64("offset").unwrap_or(0.0)
            }
            "Float" if text.is_empty() => 0.0,
            "Float" => text.parse::<f64>().ok()?,
            _ => return None,
        };
        value.is_finite().then_some(value)
    }

    fn number_at(&self, i: usize, path: &[&str]) -> Option<f64> {
        self.number(self.path(i, path)?)
    }
}

/// Attributes kept from the XML (the rest is not needed).
const ATTRIBUTES: &[&str] = &[
    "precision",
    "minimum",
    "maximum",
    "scale",
    "offset",
    "fileOffset",
    "recordCount",
];

fn parse_xml(xml: &[u8]) -> Result<Tree> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        let event = reader
            .read_event()
            .context("The E57 XML section is invalid")?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                if nodes.len() >= MAX_NODES {
                    bail!("The E57 XML section has more than {MAX_NODES} elements.");
                }
                if stack.len() >= MAX_DEPTH {
                    bail!("The E57 XML section is nested more than {MAX_DEPTH} levels deep.");
                }
                let mut node = Node {
                    name: e.local_name().as_ref().to_owned(),
                    ..Node::default()
                };
                for a in e.attributes().flatten() {
                    let key = a.key.local_name().as_ref().to_owned();
                    let wanted = key == "type" || ATTRIBUTES.contains(&key.as_str());
                    if !wanted {
                        continue;
                    }
                    let value = a
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map(|v| v.chars().take(MAX_TEXT).collect::<String>())
                        .unwrap_or_default();
                    if key == "type" {
                        node.kind = value;
                    } else {
                        node.attrs.push((key, value));
                    }
                }
                let index = nodes.len();
                match stack.last() {
                    Some(&parent) => {
                        if let Some(p) = nodes.get_mut(parent) {
                            p.children.push(index);
                        }
                    }
                    None if index == 0 => {}
                    None => bail!("The E57 XML section has more than one root element."),
                }
                nodes.push(node);
                if !empty {
                    stack.push(index);
                }
            }
            Event::Text(t) => {
                if let Some(node) = stack.last().and_then(|&i| nodes.get_mut(i)) {
                    let room = MAX_TEXT.saturating_sub(node.text.chars().count());
                    let text = t.into_inner();
                    node.text.extend(text.chars().take(room));
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if nodes.is_empty() {
        bail!("The E57 XML section is empty.");
    }
    Ok(Tree { nodes })
}

// ------------------------------------------------------------------ fields

/// What a prototype field means for the preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    X,
    Y,
    Z,
    Range,
    Azimuth,
    Elevation,
    CartesianInvalid,
    SphericalInvalid,
    Red,
    Green,
    Blue,
    ColorInvalid,
    Intensity,
    IntensityInvalid,
    /// Read (the stream must be walked) and dropped.
    Other,
}

/// Number of roles that carry a value (all but `Other`).
const ROLES: usize = Role::Other as usize;

impl Role {
    fn of(name: &str) -> Role {
        match name {
            "cartesianX" => Role::X,
            "cartesianY" => Role::Y,
            "cartesianZ" => Role::Z,
            "sphericalRange" => Role::Range,
            "sphericalAzimuth" => Role::Azimuth,
            "sphericalElevation" => Role::Elevation,
            "cartesianInvalidState" => Role::CartesianInvalid,
            "sphericalInvalidState" => Role::SphericalInvalid,
            "colorRed" => Role::Red,
            "colorGreen" => Role::Green,
            "colorBlue" => Role::Blue,
            "isColorInvalid" => Role::ColorInvalid,
            "intensity" => Role::Intensity,
            "isIntensityInvalid" => Role::IntensityInvalid,
            _ => Role::Other,
        }
    }
}

/// How a byte stream stores its values (the bit-pack codec of E57 1.0).
#[derive(Clone, Copy, Debug)]
enum Codec {
    F32,
    F64,
    /// `minimum + raw` in `bits` bits, then `· scale + offset`.
    Int {
        min: i64,
        bits: u32,
        scale: f64,
        offset: f64,
    },
}

/// One field's byte stream: it continues from packet to packet and is read
/// least significant bit first.
struct Stream {
    role: Role,
    codec: Codec,
    /// The range of the values from the prototype (limits fall back to it).
    bounds: Option<(f64, f64)>,
    bytes: Vec<u8>,
    /// Bits of `bytes` already read.
    bit: usize,
}

impl Stream {
    fn from_xml(node: &Node, role: Role) -> Result<Stream> {
        let int = |scale: f64, offset: f64| -> Result<(Codec, Option<(f64, f64)>)> {
            let min = node.attr_i64("minimum").unwrap_or(i64::MIN);
            let max = node.attr_i64("maximum").unwrap_or(i64::MAX);
            if max < min {
                bail!(
                    "The E57 field '{}' has its maximum below its minimum.",
                    truncate(&node.name)
                );
            }
            let span = (i128::from(max) - i128::from(min)) as u128;
            let bits = 128 - span.leading_zeros();
            let bounds = (min as f64 * scale + offset, max as f64 * scale + offset);
            let (lo, hi) = if scale < 0.0 {
                (bounds.1, bounds.0)
            } else {
                bounds
            };
            Ok((
                Codec::Int {
                    min,
                    bits,
                    scale,
                    offset,
                },
                Some((lo, hi)),
            ))
        };
        let (codec, bounds) = match node.kind.as_str() {
            "Float" => {
                let codec = if node.attr("precision") == Some("single") {
                    Codec::F32
                } else {
                    Codec::F64
                };
                let bounds = node.attr_f64("minimum").zip(node.attr_f64("maximum"));
                (codec, bounds)
            }
            "Integer" => int(1.0, 0.0)?,
            "ScaledInteger" => int(
                node.attr_f64("scale").unwrap_or(1.0),
                node.attr_f64("offset").unwrap_or(0.0),
            )?,
            other => bail!(
                "The E57 point field '{}' has the type '{}', which the preview does not read.",
                truncate(&node.name),
                truncate(other)
            ),
        };
        Ok(Stream {
            role,
            codec,
            bounds,
            bytes: Vec::new(),
            bit: 0,
        })
    }

    /// Bits one value takes.
    fn width(&self) -> usize {
        match self.codec {
            Codec::F32 => 32,
            Codec::F64 => 64,
            Codec::Int { bits, .. } => bits as usize,
        }
    }

    fn available(&self) -> usize {
        (self.bytes.len() * 8).saturating_sub(self.bit)
    }

    /// The next `bits` (at most 64) bits; the caller checked `available`.
    fn take(&mut self, bits: u32) -> u64 {
        let mut value = 0u64;
        let mut got = 0u32;
        while got < bits {
            let byte = self.bytes.get(self.bit / 8).copied().unwrap_or(0);
            let shift = (self.bit % 8) as u32;
            let n = (8 - shift).min(bits - got);
            let part = (u64::from(byte) >> shift) & ((1u64 << n) - 1);
            value |= part << got;
            got += n;
            self.bit += n as usize;
        }
        value
    }

    fn next(&mut self) -> f64 {
        match self.codec {
            Codec::F32 => f64::from(f32::from_bits(self.take(32) as u32)),
            Codec::F64 => f64::from_bits(self.take(64)),
            Codec::Int {
                min,
                bits,
                scale,
                offset,
            } => {
                let raw = i128::from(min) + i128::from(self.take(bits));
                raw as f64 * scale + offset
            }
        }
    }

    /// Drops the bytes already read.
    fn compact(&mut self) {
        let used = (self.bit / 8).min(self.bytes.len());
        self.bytes.drain(..used);
        self.bit -= used * 8;
    }
}

/// The terminal fields under `node` in document order (one byte stream
/// each); only direct children of the prototype get a role.
fn terminals(tree: &Tree, node: usize, depth: usize, out: &mut Vec<Stream>) -> Result<()> {
    let Some(n) = tree.node(node) else {
        return Ok(());
    };
    for &c in &n.children {
        let Some(child) = tree.node(c) else {
            continue;
        };
        if child.kind == "Structure" {
            if depth >= MAX_DEPTH {
                bail!("The E57 prototype is nested too deeply.");
            }
            terminals(tree, c, depth + 1, out)?;
            continue;
        }
        if out.len() >= MAX_PROTOTYPE_FIELDS {
            bail!("The E57 scan has more than {MAX_PROTOTYPE_FIELDS} point fields.");
        }
        let role = if depth == 0 {
            Role::of(&child.name)
        } else {
            Role::Other
        };
        out.push(Stream::from_xml(child, role)?);
    }
    Ok(())
}

// ------------------------------------------------------------------ scans

/// A unit quaternion (`w x y z`) as a rotation matrix (identity when it is
/// zero or not finite).
fn rotation(q: [f64; 4]) -> [[f64; 3]; 3] {
    let [w, x, y, z] = q;
    let n = (w * w + x * x + y * y + z * z).sqrt();
    if !n.is_finite() || n < 1e-12 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let (w, x, y, z) = (w / n, x / n, y / n, z / n);
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - w * z),
            2.0 * (x * z + w * y),
        ],
        [
            2.0 * (x * y + w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - w * x),
        ],
        [
            2.0 * (x * z - w * y),
            2.0 * (y * z + w * x),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

struct Scan {
    /// Physical offset of the binary section.
    section: u64,
    records: u64,
    streams: Vec<Stream>,
    spherical: bool,
    color: bool,
    intensity: bool,
    rotation: [[f64; 3]; 3],
    translation: [f64; 3],
    /// Value ranges of red, green, blue.
    color_limits: [(f64, f64); 3],
    intensity_limits: (f64, f64),
}

impl Scan {
    fn from_xml(tree: &Tree, scan: usize) -> Result<Scan> {
        let points = tree
            .child(scan, "points")
            .context("An E57 scan has no points.")?;
        let p = tree.node(points).context("An E57 scan has no points.")?;
        if p.kind != "CompressedVector" {
            bail!("The points of an E57 scan are not a compressed vector.");
        }
        let section = p
            .attr_u64("fileOffset")
            .context("The points of an E57 scan have no file offset.")?;
        let records = p.attr_u64("recordCount").unwrap_or(0);
        let prototype = tree
            .child(points, "prototype")
            .context("The points of an E57 scan have no prototype.")?;
        let mut streams = Vec::new();
        terminals(tree, prototype, 0, &mut streams)?;
        let has = |r: Role| streams.iter().any(|s| s.role == r);
        let spherical = if has(Role::X) && has(Role::Y) && has(Role::Z) {
            false
        } else if has(Role::Range) && has(Role::Azimuth) && has(Role::Elevation) {
            true
        } else {
            bail!("An E57 scan has neither cartesian nor spherical coordinates.");
        };
        let color = has(Role::Red) && has(Role::Green) && has(Role::Blue);
        let intensity = has(Role::Intensity);
        let bounds = |r: Role| streams.iter().find(|s| s.role == r).and_then(|s| s.bounds);
        let limit =
            |group: &str, lo: &str, hi: &str, fallback: Option<(f64, f64)>, default: (f64, f64)| {
                let (flo, fhi) = fallback.unwrap_or(default);
                (
                    tree.number_at(scan, &[group, lo]).unwrap_or(flo),
                    tree.number_at(scan, &[group, hi]).unwrap_or(fhi),
                )
            };
        let color_limits = [
            limit(
                "colorLimits",
                "colorRedMinimum",
                "colorRedMaximum",
                bounds(Role::Red),
                (0.0, 255.0),
            ),
            limit(
                "colorLimits",
                "colorGreenMinimum",
                "colorGreenMaximum",
                bounds(Role::Green),
                (0.0, 255.0),
            ),
            limit(
                "colorLimits",
                "colorBlueMinimum",
                "colorBlueMaximum",
                bounds(Role::Blue),
                (0.0, 255.0),
            ),
        ];
        let intensity_limits = limit(
            "intensityLimits",
            "intensityMinimum",
            "intensityMaximum",
            bounds(Role::Intensity),
            (0.0, 1.0),
        );
        let q = |c: &str, default: f64| {
            tree.number_at(scan, &["pose", "rotation", c])
                .unwrap_or(default)
        };
        let t = |c: &str| {
            tree.number_at(scan, &["pose", "translation", c])
                .unwrap_or(0.0)
        };
        Ok(Scan {
            section,
            records,
            spherical,
            color,
            intensity,
            rotation: rotation([q("w", 1.0), q("x", 0.0), q("y", 0.0), q("z", 0.0)]),
            translation: [t("x"), t("y"), t("z")],
            color_limits,
            intensity_limits,
            streams,
        })
    }

    /// Reads the points of the scan; every `stride`-th record over all scans
    /// (`seen` counts them) goes into `cloud`.
    fn read_points(
        &mut self,
        paged: &Paged,
        stride: u64,
        seen: &mut u64,
        cloud: &mut Cloud,
    ) -> Result<()> {
        if self.records == 0 {
            return Ok(());
        }
        if self.streams.iter().all(|s| s.width() == 0) {
            bail!("An E57 scan stores no point data (every field is constant).");
        }
        let start = Paged::logical(self.section)?;
        let header = paged.read(start, 32)?;
        if header.first() != Some(&1) {
            bail!("The binary section of an E57 scan is not a compressed vector section.");
        }
        let section_end = start
            .checked_add(u64_le(&header, 8)?)
            .filter(|end| *end <= paged.logical_len())
            .context("The binary section of an E57 scan is longer than the file.")?;
        let mut at = Paged::logical(u64_le(&header, 16)?)?;
        if at < start + 32 {
            bail!("The binary section of an E57 scan starts inside its header.");
        }
        let mut left = self.records;
        let mut packet = Vec::new();
        let mut values = [f64::NAN; ROLES];
        while left > 0 {
            if at.saturating_add(4) > section_end {
                bail!(
                    "An E57 scan ends before its {} points (the file is damaged).",
                    self.records
                );
            }
            packet.clear();
            paged.read_into(at, 4, &mut packet)?;
            let kind = packet.first().copied().unwrap_or(u8::MAX);
            let length = u64::from(u16_le(&packet, 2)?) + 1;
            if length % 4 != 0 || at + length > section_end {
                bail!("A packet of an E57 scan is damaged (offset {at}).");
            }
            match kind {
                // data packet: one buffer per byte stream
                1 => {
                    packet.clear();
                    paged.read_into(at, length as usize, &mut packet)?;
                    let count = usize::from(u16_le(&packet, 4)?);
                    if count != self.streams.len() {
                        bail!(
                            "An E57 data packet has {count} byte streams, its scan {} fields.",
                            self.streams.len()
                        );
                    }
                    let mut from = 6 + 2 * count;
                    for (k, stream) in self.streams.iter_mut().enumerate() {
                        let n = usize::from(u16_le(&packet, 6 + 2 * k)?);
                        stream
                            .bytes
                            .extend_from_slice(range(&packet, from, from + n)?);
                        from += n;
                    }
                    // the records every stream holds completely
                    let ready = self
                        .streams
                        .iter()
                        .filter(|s| s.width() > 0)
                        .map(|s| s.available() / s.width())
                        .min()
                        .unwrap_or(0) as u64;
                    let n = ready.min(left);
                    for _ in 0..n {
                        values.fill(f64::NAN);
                        for s in &mut self.streams {
                            let v = s.next();
                            if let Some(slot) = values.get_mut(s.role as usize) {
                                *slot = v;
                            }
                        }
                        if *seen % stride == 0 {
                            self.keep(&values, cloud);
                        }
                        *seen += 1;
                    }
                    left -= n;
                    for s in &mut self.streams {
                        s.compact();
                    }
                }
                // index and empty packets carry no points
                0 | 2 => {}
                other => bail!("An E57 scan has a packet of unknown type {other}."),
            }
            at += length;
        }
        Ok(())
    }

    /// Adds one record to the cloud unless it is marked invalid.
    fn keep(&self, values: &[f64; ROLES], cloud: &mut Cloud) {
        let get = |r: Role| values.get(r as usize).copied().unwrap_or(f64::NAN);
        // a missing state field is NaN: valid
        let invalid = |r: Role| {
            let state = get(r);
            state.is_finite() && state != 0.0
        };
        if invalid(Role::CartesianInvalid) || invalid(Role::SphericalInvalid) {
            return;
        }
        let local = if self.spherical {
            let (r, az, el) = (get(Role::Range), get(Role::Azimuth), get(Role::Elevation));
            [
                r * el.cos() * az.cos(),
                r * el.cos() * az.sin(),
                r * el.sin(),
            ]
        } else {
            [get(Role::X), get(Role::Y), get(Role::Z)]
        };
        let [r0, r1, r2] = self.rotation;
        let dot = |row: [f64; 3]| row[0] * local[0] + row[1] * local[1] + row[2] * local[2];
        let [tx, ty, tz] = self.translation;
        let p = [
            (dot(r0) + tx) * MM_PER_M,
            (dot(r1) + ty) * MM_PER_M,
            (dot(r2) + tz) * MM_PER_M,
        ];
        if p.iter()
            .any(|c| !c.is_finite() || c.abs() > f64::from(f32::MAX))
        {
            return;
        }
        let scaled = |v: f64, (lo, hi): (f64, f64)| {
            let span = if hi > lo { hi - lo } else { 1.0 };
            ((v - lo) / span).clamp(0.0, 1.0)
        };
        let intensity = if self.intensity && !invalid(Role::IntensityInvalid) {
            let v = scaled(get(Role::Intensity), self.intensity_limits);
            if v.is_finite() {
                v as f32
            } else {
                f32::NAN
            }
        } else {
            f32::NAN
        };
        let color = (self.color && !invalid(Role::ColorInvalid)).then(|| {
            let [lr, lg, lb] = self.color_limits;
            [
                (scaled(get(Role::Red), lr) * 255.0).round() as u8,
                (scaled(get(Role::Green), lg) * 255.0).round() as u8,
                (scaled(get(Role::Blue), lb) * 255.0).round() as u8,
            ]
        });
        cloud.push(p, color, intensity);
    }
}

/// The points read so far, over all scans.
#[derive(Default)]
struct Cloud {
    positions: Vec<f32>,
    colors: Vec<u8>,
    /// 0..=1 by the scan's intensity limits; NaN = none.
    intensity: Vec<f32>,
    any_color: bool,
    any_intensity: bool,
}

impl Cloud {
    fn push(&mut self, p: [f64; 3], color: Option<[u8; 3]>, intensity: f32) {
        self.positions
            .extend([p[0] as f32, p[1] as f32, p[2] as f32]);
        self.any_color |= color.is_some();
        self.any_intensity |= intensity.is_finite();
        // a point without colour in a coloured cloud: grey by its intensity
        let rgb = color.unwrap_or_else(|| {
            if intensity.is_finite() {
                let g = (intensity * 255.0).round() as u8;
                [g, g, g]
            } else {
                NEUTRAL
            }
        });
        self.colors.extend(rgb);
        self.intensity.push(intensity);
    }

    /// Colours when a scan has them, otherwise the intensity as a field
    /// (coloured with the colour map).
    fn into_mesh(self, scans: usize, thinned: Option<u64>) -> Mesh {
        let mut mesh = Mesh {
            positions: self.positions,
            parts: u32::try_from(scans).unwrap_or(u32::MAX),
            thinned,
            ..Mesh::default()
        };
        if self.any_color {
            mesh.colors = self.colors;
        } else if self.any_intensity {
            mesh.fields = vec![Field {
                name: "Intensity".into(),
                unit: String::new(),
                values: self.intensity,
            }];
            mesh.bake_colors();
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How a test field is stored.
    #[derive(Clone, Copy)]
    enum Kind {
        F32,
        F64,
        /// minimum, maximum (values are the integers)
        Int(i64, i64),
        /// raw minimum, raw maximum, scale, offset (values are raw)
        Scaled(i64, i64, f64, f64),
        /// a field of another type (no values written)
        Other(&'static str),
    }

    struct TestField {
        name: &'static str,
        kind: Kind,
        values: Vec<f64>,
    }

    fn field(name: &'static str, kind: Kind, values: &[f64]) -> TestField {
        TestField {
            name,
            kind,
            values: values.to_vec(),
        }
    }

    #[derive(Default)]
    struct TestScan {
        fields: Vec<TestField>,
        /// rotation (w, x, y, z) and translation
        pose: Option<([f64; 4], [f64; 3])>,
        /// more XML inside the scan (limits, names …)
        extra: String,
        /// recordCount when it should differ from the values
        records: Option<usize>,
    }

    fn width(kind: Kind) -> u32 {
        match kind {
            Kind::F32 => 32,
            Kind::F64 => 64,
            Kind::Other(_) => 0,
            Kind::Int(lo, hi) | Kind::Scaled(lo, hi, ..) => {
                128 - ((i128::from(hi) - i128::from(lo)) as u128).leading_zeros()
            }
        }
    }

    /// The byte stream of a field: bit-packed, least significant bit first.
    fn pack(f: &TestField) -> Vec<u8> {
        let mut out = Vec::new();
        match f.kind {
            Kind::F32 => f
                .values
                .iter()
                .for_each(|v| out.extend((*v as f32).to_le_bytes())),
            Kind::F64 => f.values.iter().for_each(|v| out.extend(v.to_le_bytes())),
            Kind::Other(_) => {}
            Kind::Int(lo, _) | Kind::Scaled(lo, ..) => {
                let bits = width(f.kind);
                let mut bit = 0usize;
                for v in &f.values {
                    let raw = (*v as i64 - lo) as u64;
                    for b in 0..bits {
                        if bit % 8 == 0 {
                            out.push(0);
                        }
                        if (raw >> b) & 1 == 1 {
                            *out.last_mut().unwrap() |= 1u8 << (bit % 8);
                        }
                        bit += 1;
                    }
                }
            }
        }
        out
    }

    fn physical(logical: usize) -> usize {
        logical / PAGE_DATA * PAGE + logical % PAGE_DATA
    }

    fn field_xml(f: &TestField) -> String {
        match f.kind {
            Kind::F32 => format!(r#"<{} type="Float" precision="single"/>"#, f.name),
            Kind::F64 => format!(r#"<{} type="Float"/>"#, f.name),
            Kind::Other(kind) => format!(r#"<{} type="{kind}"/>"#, f.name),
            Kind::Int(lo, hi) => {
                format!(
                    r#"<{} type="Integer" minimum="{lo}" maximum="{hi}"/>"#,
                    f.name
                )
            }
            Kind::Scaled(lo, hi, s, o) => format!(
                r#"<{} type="ScaledInteger" minimum="{lo}" maximum="{hi}" scale="{s}" offset="{o}"/>"#,
                f.name
            ),
        }
    }

    /// A minimal E57 writer: header, one binary section per scan (an empty
    /// packet first, then data packets of at most `chunk` bytes per stream,
    /// so values straddle packets), the XML, pages with their checksums.
    fn e57_with(scans: &[TestScan], chunk: usize, images: &str) -> Vec<u8> {
        let mut buf = vec![0u8; 48];
        let mut xml_scans = String::new();
        for scan in scans {
            while buf.len() % 4 != 0 {
                buf.push(0);
            }
            let start = buf.len();
            buf.extend([0u8; 32]);
            buf.extend([2, 0, 3, 0]); // an empty packet
            let streams: Vec<Vec<u8>> = scan.fields.iter().map(pack).collect();
            let packets = streams
                .iter()
                .map(|s| s.len().div_ceil(chunk))
                .max()
                .unwrap_or(0)
                .max(1);
            for p in 0..packets {
                let parts: Vec<&[u8]> = streams
                    .iter()
                    .map(|s| {
                        let from = (p * chunk).min(s.len());
                        &s[from..((p + 1) * chunk).min(s.len())]
                    })
                    .collect();
                let mut packet = vec![1u8, 0, 0, 0];
                packet.extend((parts.len() as u16).to_le_bytes());
                for part in &parts {
                    packet.extend((part.len() as u16).to_le_bytes());
                }
                for part in &parts {
                    packet.extend_from_slice(part);
                }
                while packet.len() % 4 != 0 {
                    packet.push(0);
                }
                let len = (packet.len() - 1) as u16;
                packet[2..4].copy_from_slice(&len.to_le_bytes());
                buf.extend(packet);
            }
            let section_len = (buf.len() - start) as u64;
            buf[start] = 1;
            buf[start + 8..start + 16].copy_from_slice(&section_len.to_le_bytes());
            buf[start + 16..start + 24]
                .copy_from_slice(&(physical(start + 32) as u64).to_le_bytes());
            let pose = scan
                .pose
                .map(|([w, x, y, z], [tx, ty, tz])| {
                    format!(
                        r#"<pose type="Structure"><rotation type="Structure"><w type="Float">{w}</w><x type="Float">{x}</x><y type="Float">{y}</y><z type="Float">{z}</z></rotation><translation type="Structure"><x type="Float">{tx}</x><y type="Float">{ty}</y><z type="Float">{tz}</z></translation></pose>"#
                    )
                })
                .unwrap_or_default();
            let records = scan
                .records
                .unwrap_or_else(|| scan.fields.first().map_or(0, |f| f.values.len()));
            let prototype: String = scan.fields.iter().map(field_xml).collect();
            xml_scans.push_str(&format!(
                r#"<vectorChild type="Structure"><guid type="String"><![CDATA[{{scan}}]]></guid>{pose}{}<points type="CompressedVector" fileOffset="{}" recordCount="{records}"><prototype type="Structure">{prototype}</prototype><codecs type="Vector" allowHeterogeneousChildren="1"/></points></vectorChild>"#,
                scan.extra,
                physical(start)
            ));
        }
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<e57Root type="Structure" xmlns="http://www.astm.org/COMMIT/E57/2010-e57-v1.0">
<formatName type="String"><![CDATA[ASTM E57 3D Imaging Data File]]></formatName>
<versionMajor type="Integer">1</versionMajor>
<data3D type="Vector" allowHeterogeneousChildren="1">{xml_scans}</data3D>
<images2D type="Vector" allowHeterogeneousChildren="1">{images}</images2D>
</e57Root>"#
        );
        let xml_start = buf.len();
        buf.extend(xml.as_bytes());
        let pages = buf.len().div_ceil(PAGE_DATA);
        buf[..8].copy_from_slice(b"ASTM-E57");
        buf[8..12].copy_from_slice(&1u32.to_le_bytes());
        buf[16..24].copy_from_slice(&((pages * PAGE) as u64).to_le_bytes());
        buf[24..32].copy_from_slice(&(physical(xml_start) as u64).to_le_bytes());
        buf[32..40].copy_from_slice(&(xml.len() as u64).to_le_bytes());
        buf[40..48].copy_from_slice(&(PAGE as u64).to_le_bytes());
        buf.resize(pages * PAGE_DATA, 0);
        let mut file = Vec::with_capacity(pages * PAGE);
        for page in buf.chunks(PAGE_DATA) {
            file.extend_from_slice(page);
            file.extend([0u8; 4]);
        }
        seal_pages(&mut file);
        file
    }

    fn e57(scans: &[TestScan]) -> Vec<u8> {
        e57_with(scans, 7, "")
    }

    fn xyz(kind: Kind, points: &[[f64; 3]]) -> Vec<TestField> {
        ["cartesianX", "cartesianY", "cartesianZ"]
            .iter()
            .enumerate()
            .map(|(i, &name)| field(name, kind, &points.iter().map(|p| p[i]).collect::<Vec<_>>()))
            .collect()
    }

    fn point(mesh: &Mesh, i: usize) -> [f32; 3] {
        [
            mesh.positions[3 * i],
            mesh.positions[3 * i + 1],
            mesh.positions[3 * i + 2],
        ]
    }

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-2)
    }

    #[test]
    fn the_fuzz_seed_is_a_valid_file() {
        // the seed the fuzzer starts from must pass every check of the reader
        // (page checksums included): 3 single-precision points with an 8-bit
        // intensity, moved 0.1 m along x by the scan's pose
        let seed = include_bytes!("../../fuzz/seeds/e57/three-points.e57");
        let mesh = crate::mesh::read("e57", seed).unwrap();
        assert!(mesh.is_point_cloud());
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.parts, 1);
        assert!(
            near(point(&mesh, 0), [100.0, 0.0, 0.0]),
            "{:?}",
            point(&mesh, 0)
        );
        assert!(
            near(point(&mesh, 1), [1100.0, 0.0, 250.0]),
            "{:?}",
            point(&mesh, 1)
        );
        assert!(
            near(point(&mesh, 2), [600.0, 1000.0, 500.0]),
            "{:?}",
            point(&mesh, 2)
        );
        assert_eq!(mesh.fields.len(), 1);
        assert_eq!(mesh.fields[0].name, "Intensity");
        assert_eq!(mesh.thinned, None);
    }

    #[test]
    fn crc32c_matches_the_standard_check_value() {
        assert_eq!(crc32c(&crc_table(), b"123456789"), 0xE306_9283);
        assert_eq!(crc32c(&crc_table(), b""), 0);
    }

    #[test]
    fn scans_are_moved_by_their_pose_and_converted_to_mm() {
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let file = e57(&[
            TestScan {
                fields: xyz(Kind::F32, &[[1.0, 0.0, 0.0], [0.0, 2.0, 0.0]]),
                pose: Some(([1.0, 0.0, 0.0, 0.0], [10.0, 0.0, 0.0])),
                ..TestScan::default()
            },
            TestScan {
                // 90° about z, then 5 m up
                fields: xyz(Kind::F64, &[[1.0, 0.0, 0.0]]),
                pose: Some(([half, 0.0, 0.0, half], [0.0, 0.0, 5.0])),
                ..TestScan::default()
            },
        ]);
        let mesh = crate::mesh::read("e57", &file).unwrap();
        assert!(mesh.is_point_cloud());
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.parts, 2);
        assert!(
            near(point(&mesh, 0), [11000.0, 0.0, 0.0]),
            "{:?}",
            point(&mesh, 0)
        );
        assert!(near(point(&mesh, 1), [10000.0, 2000.0, 0.0]));
        assert!(
            near(point(&mesh, 2), [0.0, 1000.0, 5000.0]),
            "{:?}",
            point(&mesh, 2)
        );
        assert!(mesh.colors.is_empty() && mesh.fields.is_empty());
        assert_eq!(mesh.thinned, None);
    }

    #[test]
    fn spherical_and_scaled_integer_coordinates() {
        let file = e57(&[TestScan {
            fields: vec![
                // 2 m and 1 m in millimetre steps
                field(
                    "sphericalRange",
                    Kind::Scaled(0, 10_000, 0.001, 0.0),
                    &[2000.0, 1000.0],
                ),
                field(
                    "sphericalAzimuth",
                    Kind::F64,
                    &[std::f64::consts::FRAC_PI_2, 0.0],
                ),
                field(
                    "sphericalElevation",
                    Kind::F32,
                    &[0.0, std::f64::consts::FRAC_PI_2 as f32 as f64],
                ),
            ],
            ..TestScan::default()
        }]);
        let mesh = read_e57(&file).unwrap();
        assert_eq!(mesh.vertex_count(), 2);
        assert!(
            near(point(&mesh, 0), [0.0, 2000.0, 0.0]),
            "{:?}",
            point(&mesh, 0)
        );
        assert!(
            near(point(&mesh, 1), [0.0, 0.0, 1000.0]),
            "{:?}",
            point(&mesh, 1)
        );

        // cartesian scaled integers with an offset: raw · 0.01 + 5
        let file = e57(&[TestScan {
            fields: xyz(
                Kind::Scaled(-1000, 1000, 0.01, 5.0),
                &[[100.0, -500.0, 0.0]],
            ),
            ..TestScan::default()
        }]);
        let mesh = read_e57(&file).unwrap();
        assert!(
            near(point(&mesh, 0), [6000.0, 0.0, 5000.0]),
            "{:?}",
            point(&mesh, 0)
        );
    }

    #[test]
    fn colours_are_scaled_by_their_limits() {
        let mut fields = xyz(Kind::F32, &[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);
        fields.push(field("colorRed", Kind::Int(0, 65535), &[65535.0, 0.0]));
        fields.push(field("colorGreen", Kind::Int(0, 65535), &[0.0, 32768.0]));
        fields.push(field("colorBlue", Kind::Int(0, 65535), &[0.0, 65535.0]));
        // without colorLimits: the prototype's minimum and maximum
        let file = e57(&[TestScan {
            fields,
            ..TestScan::default()
        }]);
        let mesh = read_e57(&file).unwrap();
        assert_eq!(mesh.colors, [255, 0, 0, 0, 128, 255]);
        assert!(mesh.fields.is_empty());

        // colorLimits narrower than the stored range: values above clamp
        let mut fields = xyz(Kind::F32, &[[0.0, 0.0, 0.0]]);
        fields.push(field("colorRed", Kind::Int(0, 255), &[100.0]));
        fields.push(field("colorGreen", Kind::Int(0, 255), &[50.0]));
        fields.push(field("colorBlue", Kind::Int(0, 255), &[0.0]));
        let limits = r#"<colorLimits type="Structure"><colorRedMinimum type="Integer">0</colorRedMinimum><colorRedMaximum type="Integer">100</colorRedMaximum><colorGreenMinimum type="Integer">0</colorGreenMinimum><colorGreenMaximum type="Integer">100</colorGreenMaximum><colorBlueMinimum type="Integer">0</colorBlueMinimum><colorBlueMaximum type="Integer">100</colorBlueMaximum></colorLimits>"#;
        let file = e57(&[TestScan {
            fields,
            extra: limits.into(),
            ..TestScan::default()
        }]);
        assert_eq!(read_e57(&file).unwrap().colors, [255, 128, 0]);
    }

    #[test]
    fn intensity_colours_a_cloud_without_colours() {
        let mut fields = xyz(
            Kind::F32,
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
        );
        fields.push(field(
            "intensity",
            Kind::Int(0, 2047),
            &[0.0, 1000.0, 2000.0],
        ));
        let limits = r#"<intensityLimits type="Structure"><intensityMinimum type="Integer">0</intensityMinimum><intensityMaximum type="Integer">2000</intensityMaximum></intensityLimits>"#;
        let file = e57(&[TestScan {
            fields,
            extra: limits.into(),
            ..TestScan::default()
        }]);
        let mesh = read_e57(&file).unwrap();
        assert_eq!(mesh.fields.len(), 1);
        assert_eq!(mesh.fields[0].name, "Intensity");
        assert_eq!(mesh.fields[0].values, [0.0, 0.5, 1.0]);
        assert_eq!(
            mesh.colors.len(),
            mesh.positions.len(),
            "baked for old viewers"
        );
        assert!(mesh.check().is_ok());
    }

    #[test]
    fn invalid_points_unknown_fields_and_images_are_left_out() {
        let mut fields = xyz(
            Kind::F64,
            &[
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
            ],
        );
        fields.insert(
            0,
            field("rowIndex", Kind::Int(0, 10), &[0.0, 1.0, 2.0, 3.0]),
        );
        fields.push(field(
            "cartesianInvalidState",
            Kind::Int(0, 2),
            &[0.0, 2.0, 1.0, 0.0],
        ));
        fields.push(field("timeStamp", Kind::F64, &[0.5, 0.6, 0.7, 0.8]));
        fields.push(field("returnCount", Kind::Int(1, 1), &[1.0; 4]));
        let images = r#"<vectorChild type="Structure"><pinholeRepresentation type="Structure"><jpegImage type="Blob" fileOffset="0" length="0"/></pinholeRepresentation></vectorChild>"#;
        let file = e57_with(
            &[TestScan {
                fields,
                ..TestScan::default()
            }],
            3,
            images,
        );
        let mesh = read_e57(&file).unwrap();
        assert_eq!(mesh.vertex_count(), 2);
        assert!(near(point(&mesh, 0), [1000.0, 0.0, 0.0]));
        assert!(near(point(&mesh, 1), [4000.0, 0.0, 0.0]));

        // every point invalid: no geometry
        let mut fields = xyz(Kind::F32, &[[1.0, 0.0, 0.0]]);
        fields.push(field("sphericalInvalidState", Kind::Int(0, 2), &[2.0]));
        let file = e57(&[TestScan {
            fields,
            ..TestScan::default()
        }]);
        let err = crate::mesh::read("e57", &file).unwrap_err().to_string();
        assert!(err.contains("no geometry"), "{err}");
    }

    #[test]
    fn large_clouds_are_thinned_evenly_over_all_scans() {
        let file = e57(&[
            TestScan {
                fields: xyz(
                    Kind::F32,
                    &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                ),
                ..TestScan::default()
            },
            TestScan {
                fields: xyz(Kind::F32, &[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]),
                ..TestScan::default()
            },
        ]);
        // 5 points, budget 2: every third point over both scans
        let mesh = read_within(&file, 2).unwrap();
        assert_eq!(mesh.vertex_count(), 2);
        assert!(near(point(&mesh, 0), [0.0, 0.0, 0.0]));
        assert!(near(point(&mesh, 1), [3000.0, 0.0, 0.0]));
        assert_eq!(mesh.thinned, Some(5));
        assert_eq!(read_within(&file, 5).unwrap().thinned, None);
        // the note survives the wire format
        assert_eq!(Mesh::decode(&mesh.encode()).unwrap().thinned, Some(5));
    }

    #[test]
    fn a_flipped_byte_is_refused() {
        let file = e57(&[TestScan {
            fields: xyz(Kind::F32, &[[1.0, 2.0, 3.0]]),
            ..TestScan::default()
        }]);
        assert!(read_e57(&file).is_ok());
        for at in [100, 600, file.len() - 10] {
            let mut damaged = file.clone();
            damaged[at] ^= 0x10;
            let err = read_e57(&damaged).unwrap_err().to_string();
            assert!(
                err.contains("damaged") && err.contains("checksum"),
                "{at}: {err}"
            );
        }
        // the page is named
        if file.len() > PAGE {
            let mut damaged = file.clone();
            damaged[PAGE + 5] ^= 1;
            let err = read_e57(&damaged).unwrap_err().to_string();
            assert!(err.contains("page 2"), "{err}");
        }
        // little-endian checksums are accepted as well
        let mut le = file.clone();
        for page in le.chunks_exact_mut(PAGE) {
            page[PAGE_DATA..].reverse();
        }
        assert!(read_e57(&le).is_ok());
    }

    #[test]
    fn damaged_structure_is_an_error_not_a_panic() {
        let file = e57(&[TestScan {
            fields: xyz(Kind::F32, &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]),
            ..TestScan::default()
        }]);
        assert!(read_e57(b"not an e57 file").is_err());
        assert!(read_e57(&file[..40]).is_err(), "header cut short");
        let err = read_e57(&file[..file.len() - PAGE.min(file.len() - 48)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("ends early"), "{err}");
        let mut version = file.clone();
        version[8] = 2;
        assert!(read_e57(&version)
            .unwrap_err()
            .to_string()
            .contains("version 2"));
        let mut pages = file.clone();
        pages[41] = 0x08; // 2048-byte pages
        assert!(read_e57(&pages)
            .unwrap_err()
            .to_string()
            .contains("pages of"));

        // more points claimed than stored
        let file = e57(&[TestScan {
            fields: xyz(Kind::F32, &[[1.0, 2.0, 3.0]]),
            records: Some(5),
            ..TestScan::default()
        }]);
        let err = read_e57(&file).unwrap_err().to_string();
        assert!(err.contains("ends before its 5 points"), "{err}");

        // no coordinates
        let file = e57(&[TestScan {
            fields: vec![field("intensity", Kind::F32, &[1.0])],
            ..TestScan::default()
        }]);
        let err = read_e57(&file).unwrap_err().to_string();
        assert!(err.contains("neither cartesian nor spherical"), "{err}");

        // no scans, only images
        let err = read_e57(&e57(&[])).unwrap_err().to_string();
        assert!(err.contains("no 3D scan"), "{err}");

        // a blob field in the prototype
        let mut fields = xyz(Kind::F32, &[[1.0, 2.0, 3.0]]);
        fields.push(field("label", Kind::Other("Blob"), &[]));
        let file = e57(&[TestScan {
            fields,
            ..TestScan::default()
        }]);
        let err = read_e57(&file).unwrap_err().to_string();
        assert!(err.contains("does not read"), "{err}");
    }

    #[test]
    fn the_bit_stream_continues_across_packets() {
        let mut s = Stream {
            role: Role::Other,
            codec: Codec::Int {
                min: -3,
                bits: 3,
                scale: 1.0,
                offset: 0.0,
            },
            bounds: None,
            bytes: vec![0b1010_1100],
            bit: 0,
        };
        // 100 = 4, 101 = 5, then two bits of the next value
        assert_eq!(s.next(), 1.0);
        assert_eq!(s.next(), 2.0);
        assert_eq!(s.available(), 2);
        s.compact();
        assert_eq!((s.bytes.len(), s.bit), (1, 6));
        s.bytes.push(0b0000_0001); // the third value's high bit
        assert_eq!(s.available(), 10);
        assert_eq!(s.next(), (0b110 - 3) as f64);
        assert_eq!(rotation([0.0; 4]), rotation([1.0, 0.0, 0.0, 0.0]));
    }
}
