# Canonical conformance

`TopologyFingerprintV1` is the repository's Rust-side conformance value. It is
versioned, structured, and diffable; adapters must compare it (or the subset of
it their format retains), never a digest alone. The Rust type is doc-hidden so
the stable polygonization API stays narrow while bindings gain one shared test
contract.

The fingerprint includes canonical polygons, exterior and interior rings,
representative edge IDs, complete boundary provenance sets, Z output, dangles,
cut edges, invalid rings, selected canonical options, and deterministic
topology diagnostics. It deliberately excludes timings, allocator data, work
counters, thread data, and architecture metadata.

Coordinates are finite IEEE-754 `f64` bit strings such as
`"0x3ff0000000000000"`. `-0.0` is normalized to positive zero. IDs use fixed
width hexadecimal strings, including `u64` provenance IDs, so JSON and
JavaScript never silently round them. The same encoding is used in normalized
error witnesses.

`NormalizedPolygonizeErrorV1` compares schema version, family, code, pipeline
stage, structured option values, and available witness IDs/coordinates. Human
messages remain useful to callers but are intentionally not equality input.
`TopologyFingerprintV1::diff` returns the first JSON-style field path for CI.

Direct `MultiPolygon` conversion is intentionally lossy: it discards dangles,
cut edges, invalid rings, diagnostics, representative IDs, provenance, and Z.
Adapter tests for such APIs compare only the polygon subset they can retain.

Future incompatible additions use `V2` rather than changing V1. Adapters must
declare the highest schema they consume and retain V1 comparisons during a
supported migration window.
