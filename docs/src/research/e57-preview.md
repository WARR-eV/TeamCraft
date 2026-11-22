# 3D preview of E57 point clouds

*Date: 2026-11-22 · Issue #373 · Decision: an own bounded reader (`src/mesh/e57.rs`), no new crate.*

## Question

E57 (`.e57`, ASTM E2807) is the exchange format most laser scanners and their
software export. TeamCraft recognised it (`src/cadformat.rs`) but showed no 3D
preview, although PTS, XYZ and LAS point clouds are previewed. What does a
reader need, and should it be the `e57` crate or an own reader?

## Sources

* **ASTM E2807-11** *Standard Specification for 3D Imaging Data Exchange,
  Version 1.0* – the public description of the file layout (header, pages,
  XML, binary sections), as summarised on the libE57 project pages.
* **libE57 format notes** (libe57.org, *E57 file format* and the reference
  implementation's documentation, Boost Software Licence): the checksum byte
  order, the packet layout of compressed vectors, the bit-pack codec and the
  element names of `data3D` (`pose`, `colorLimits`, `intensityLimits`,
  `cartesianInvalidState` …).
* **The `e57` crate** (github.com/cry-inc/e57, MIT/Apache-2.0, pure Rust) –
  read as documentation of the same public format to confirm three details:
  the checksum is stored big-endian (`crc.to_be_bytes()`), packet lengths are
  stored minus one and are multiples of four, and the bit-pack stream is read
  least significant bit first across packet boundaries. No code was copied.
* Synthetic files built by the tests' own minimal writer (below). No openly
  licensed real scan small enough for the repository was found yet; the libE57
  sample data has no clear licence for redistribution.

No proprietary scanner software was used or examined.

## The format, as far as the preview needs it

**File header** (48 bytes at offset 0, little endian): signature
`ASTM-E57`, major version `u32` (1), minor version `u32`, physical file
length `u64`, physical offset of the XML section `u64`, logical length of the
XML section `u64`, page size `u64` (1024).

**Pages.** The whole file – header included – is cut into physical pages of
1024 bytes: 1020 bytes of data and a CRC-32C (Castagnoli, reflected
polynomial `0x82F63B78`, initial value and final XOR `0xFFFFFFFF`) of those
1020 bytes. The standard says numbers are little endian, but libE57 writes the
checksum **big endian** and every reader follows it. Offsets in the XML are
*physical* (`page · 1024 + byte`), lengths are *logical* (counted without the
checksums): logical = `page · 1020 + byte`.

**XML section.** A tree of typed elements: `Structure`, `Vector`,
`CompressedVector`, `Integer` (attributes `minimum`, `maximum`; value as
text), `ScaledInteger` (also `scale`, `offset`; value = raw · scale +
offset), `Float` (`precision="single"|"double"`), `String`, `Blob`. The root
`e57Root` has `data3D` (one child per scan) and `images2D` (ignored). Per
scan:

* `pose` – `rotation` (unit quaternion `w x y z`) and `translation`
  (`x y z`); a point moves as `p' = R·p + t`, which lines up multi-station
  scans;
* `colorLimits` (`colorRedMinimum` … `colorBlueMaximum`) and
  `intensityLimits` (`intensityMinimum`, `intensityMaximum`) – the range the
  stored values are scaled from; missing limits fall back to the prototype's
  `minimum`/`maximum`;
* `points` – a `CompressedVector` with `fileOffset` (physical) and
  `recordCount`, whose `prototype` lists the fields of a record in order:
  `cartesianX/Y/Z` or `sphericalRange/Azimuth/Elevation` (radians, elevation
  from the XY plane), `cartesianInvalidState` / `sphericalInvalidState`
  (0 valid, 1 direction only, 2 invalid), `colorRed/Green/Blue`,
  `isColorInvalid`, `intensity`, `isIntensityInvalid`, and many others
  (`rowIndex`, `timeStamp`, `returnIndex`, extensions) that the preview skips.

**Binary section of a compressed vector.** A 32-byte section header (section
id `1`, 7 reserved bytes, logical length `u64`, physical offset of the first
data packet `u64`, physical offset of the index packet `u64`), then packets
(logical addresses, at most 64 KiB, length stored minus one, multiple of 4):

* data packet – type `1`, flags `u8`, length − 1 `u16`, byte stream count
  `u16`, one `u16` buffer length per stream, then the buffers;
* index packet – type `0`, 16-byte header (skipped);
* empty packet – type `2`, 4-byte header (skipped).

There is one byte stream per prototype field. Each stream continues from one
packet to the next. The only codec of E57 1.0 is **bit pack**: `Float`
values as 4 or 8 little-endian bytes; `Integer` and `ScaledInteger` as
`value − minimum` in `bits = ⌈log₂(maximum − minimum + 1)⌉` bits (0 bits
when minimum = maximum), least significant bit first in one continuous
bit stream.

## Crate or own reader?

The `e57` crate (MIT/Apache-2.0, pure Rust) reads the format well, but:

* it checks a page's checksum only when that page is read; the issue wants
  every page checked before anything is shown, so a damaged file never gives
  a wrong cloud;
* it brings `roxmltree` and its own allocation decisions (one `Vec` per
  record queue, sizes from the file), which we would have to audit and fuzz as
  a black box in the preview child;
* its API returns records point by point through an iterator; thinning evenly
  over all scans and dropping the unused fields is just as much code on top.

An own reader is about 700 lines on top of what TeamCraft already has:
`quick-xml` (already used for 3MF, AMF and VTK XML), the bounded byte helpers
of `src/mesh/helpers.rs`, `stride_for` and `MAX_POINTS`. A table-driven
CRC-32C is 20 lines. **Decision: own reader in `src/mesh/e57.rs`, no new
dependency.**

## Limits of the reader

* every page's CRC-32C is checked first (either byte order accepted – a
  random match is 2 in 2³²); a mismatch refuses the file and names the page;
* XML section at most 64 MiB, at most 1 000 000 elements, depth at most 64,
  text kept per element at most 256 characters;
* at most 4096 scans and 256 fields per prototype; packets checked against
  the section length and the stream count of the prototype; a scan that ends
  before its `recordCount` is refused;
* points thinned evenly with `stride_for` over the summed `recordCount` of
  all scans, so the preview never holds more than `MAX_POINTS`; the preview
  notes how many points the file has (flag 8 of `TCMESH01`, also set by the
  PTS/XYZ and LAS readers);
* metres converted to millimetres (the viewer's unit).

## Not now

* embedded images (`images2D`), colours from them, and E57 extension fields;
* `Blob`/`String` fields inside a compressed vector (not used by scanners;
  refused);
* per-scan grouping in the viewer (the scans become one cloud).
