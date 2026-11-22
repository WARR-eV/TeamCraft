# 3D preview of SolidCraft designs without an export

*2026-11-22* – issue #371

## Question

Can the server show a SolidCraft design (`.sc`) in 3D from the file alone –
pure Rust in `mesh.rs`, like the other native readers – when no STEP or 3MF
export sits next to it?

## Sources

SolidCraft 0.46.1 (commit `18c9760e3`), MIT OR Apache-2.0, read as a
specification: `docs/design-container.md` (sections *Provisional saved
display*, *Model cache*, *Portable thumbnail*) and `docs/native-brep.md`
(geometry schema 4, body envelope `SCBREP05`, *Bounded provisional display*).
See also [SolidCraft designs](solidcraft-design.md).

## Findings

What a design file can hold that shows the model:

| Data | Written | What it is | Effort to read |
|------|---------|-----------|----------------|
| `cache/preview.json` + `cache/preview.bin` | only saves with `preview: true` – **not** the default; never with an active sketch, feature errors or pending moves | display meshes per body: `SCMESH01`, u64 counts, f64 coordinates, u32 indices, names/colours in JSON; ≤ 8 MiB, ≤ 100,000 triangles | small: a bounded reader like the other mesh formats, → TCMESH01 |
| `cache/model.json` + `cache/bodies.bin` | desktop Save/Save As by default (`cache: true`); not plain JSON, autosave or browser downloads; left out with feature errors | the evaluated model: per body the exact native B-rep (`SCBREP05` envelope, tagged binary values, geometry schema 4), mesh bodies as `SCMESH01` slices; read back by SolidCraft only when a build digest matches | large: decode the tagged envelope, then tessellate B-rep faces |
| `blobs/<sha>.step`, `blobs/<sha>.mesh` | when the design imports STEP or meshes | the *inputs* of import features, not the modelled result | small, but shows the wrong thing for any design with features after the import |
| `cache/thumbnail.png` | warm desktop saves | a 240 × 150 picture | done (#364) |

Native B-rep, what tessellation would need:

* Curves: line, circle, ellipse, polyline, (rational) B-spline and
  `surface_intersection` – a *procedural* curve traced between two surfaces
  from a leader polyline/spline (the leader is a usable approximation for
  display).
* Surfaces: plane (two forms), cylinder, cone, sphere, torus, (rational)
  B-spline, `placed_revolution` (a profile revolved and placed by a matrix),
  `ball_patch` and `blend` (both always carry their NURBS net). Analytic
  faces may carry a NURBS form whose parameters the trims use.
* Faces are trimmed by loops of oriented edges with optional pcurves; the
  same problem TeamCraft already solves for Creo (`mesh/creo/`: sampled
  edges, ear-clipping in the parameter plane) and STEP (truck).
* SolidCraft itself has a bounded display mesher for this data (≤ 32 points
  per curved edge, a fixed ≤ 16 × 16 grid per face, procedural revolutions
  refused) – a size reference, not code to take.
* The envelope is versioned (`SCBREP05`, schema 4) and changed five times in
  development; the cache is keyed to a build digest, so a reader outside
  SolidCraft must treat any unknown version as "no preview".

How often the data is there: the preview mesh almost never (opt-in); the
model cache on most designs saved by the desktop app without errors, never
on plain-JSON designs or browser downloads.

## Decision

1. **Now:** keep the export route – the desktop app writes the STEP with
   `solidcraft-cli` at check-in (#368), and the thumbnail covers the picture
   (#364).
2. **Next, small:** read `cache/preview.bin` when present (`SCMESH01` →
   TCMESH01, bounded, unknown versions = no preview). Cheap, but rarely
   present until SolidCraft writes it by default – a request to the
   SolidCraft project, for the maintainer to decide.
3. **Later, only if 1–2 are not enough:** an `SCBREP05` reader with a
   display tessellator for planes, cylinders, cones, spheres, tori and NURBS
   nets (procedural curves from their leaders), sharing the trimming of
   `mesh/creo/`. Estimated as large as the Creo B-rep reader; it has to
   follow every envelope version SolidCraft ships.
4. **Not:** the imported `blobs/` as a stand-in for the model – it would show
   outdated geometry.
