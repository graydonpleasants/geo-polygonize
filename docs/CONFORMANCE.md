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

## Stable entrypoint matrix

Conformance fixtures are exercised at the strongest contract each supported
entrypoint retains:

| Surface | Stable entrypoints | Compared contract |
| --- | --- | --- |
| Rust | `polygonize`, `polygonize_with_execution_policy`, `polygonize_with_workspace`, `polygonize_with_workspace_and_execution_policy`, `polygonize_line_strings`, `polygonize_line_strings_with_execution_policy` | Exact `TopologyFingerprintV1` |
| Rust XY projection | `polygonize_to_multi_polygon` | Polygon structure only; non-polygon outputs, IDs, provenance, diagnostics, and Z are discarded |
| Python | `polygonize_with_options`, legacy `polygonize` with `output="report"` | Exact `topology_fingerprint` serialized by Rust |
| Python projections | `output="buffers"`, `output="objects"`, and `return_polygons=True` | Only their exposed buffers or objects; these modes do not carry a topology fingerprint |
| Wasm full-result | `polygonizeFingerprintWithOptions`, `polygonizeReportWithOptions`, `polygonizeTraceWithOptions`, `polygonizeWithOptionsBuffer`, legacy `polygonize_buffers`, and the async report/trace wrappers | Exact fingerprint, or the exact `topology` member for traces |
| Wasm polygon projections | `polygonizeWithOptions`, legacy `polygonize`, and the async polygon wrapper | Polygon GeoJSON only |
| Wasm Arrow IPC | `polygonizeGeoArrowWithOptions`, legacy `polygonize_geoarrow` | XY polygon structure only |
| Arrow Rust API | `polygonize_arrow` | XY polygon structure only |
| Arrow C Data Interface | `polygonize_with_options_ffi`, legacy `polygonize_ffi` | XY polygon structure only |

Python and Wasm core failures expose the complete
`NormalizedPolygonizeErrorV1`. The Rust Arrow API returns the core error, which
is normalized by the conformance test. The version-1 C ABI last-error record is
more limited: tests compare its stable status, family, stage, and witness and
deliberately ignore `message`; it does not expose the normalized code or schema
version. Adapter parsing failures that never reach the core are likewise
binding-specific rather than cross-binding equality candidates.

Future incompatible additions use `V2` rather than changing V1. Adapters must
declare the highest schema they consume and retain V1 comparisons during a
supported migration window.

The Python canonical-options result exposes this value as
`topology_fingerprint`. The Wasm typed-buffer result exposes the same value as
`topology_fingerprint`; both are serialized directly by Rust rather than
reconstructed in the adapter.

## Error construction matrix

`PolygonizeErrorKind` is the structural error-family contract. The exhaustive
match in `error_family_contract.rs` makes adding a family a compile failure
until its construction class is chosen. Construction tests compare structural
variants, never message wording.

| Error kind | Supported construction path | Regression |
| --- | --- | --- |
| `InvalidArgumentType` | Invalid `PolygonizerOptions` through `validate` or polygonization | `supported_core_entrypoints_construct_every_core_error_family` |
| `InvalidGeometry` | Non-finite input through `polygonize` | `supported_core_entrypoints_construct_every_core_error_family` |
| `InvalidBufferShape` | Malformed Arrow list buffers through `polygonize_arrow` | `test_polygonize_arrow_invalid_buffer_shape_error_path` |
| `ResourceLimitExceeded` | Any exceeded `ExecutionPolicy` budget | `supported_core_entrypoints_construct_every_core_error_family` |
| `Cancelled` | A cancelled `ExecutionPolicy` token | `supported_core_entrypoints_construct_every_core_error_family` |
| `UnsupportedOptionCombination` | Incompatible semantic options through `validate` or polygonization | `supported_core_entrypoints_construct_every_core_error_family` |
| `ZConflict` | Conflicting source Z values with `ErrorOnConflict` | `supported_core_entrypoints_construct_every_core_error_family` |
| `NodingValidationFailure` | Residual intersection with the `Validate` guarantee | `supported_core_entrypoints_construct_every_core_error_family` |
| `ArrowError` | Incompatible Arrow array through `polygonize_arrow` | `test_polygonize_arrow_invalid_type_error_path` |

`InternalInvariantViolation` is a bug sentinel exercised through internal
invariant regressions, not an input-validation path. `Panic` is emitted only
when a binding catches an unexpected unwind; a deterministic public input must
never be added merely to exercise it. The C ABI reports null pointers directly
as `InvalidArgument`; it does not route them through a core error variant.

## Serialized report and compatibility policy

Topology fingerprints, normalized errors, and serialized diagnostics expose a
`schema_version`. The repository does not yet emit a topology trace; P1.5 must
define its versioned trace schema before exposing one.

The canonical options APIs are the forward-compatible contract. Through 1.x,
the legacy positional APIs remain supported: Python `polygonize`, Wasm
`polygonize`, `polygonize_buffers`, and `polygonize_geoarrow`, plus the legacy
Arrow C `polygonize_ffi` request struct. They remain polygon-only where they
already are polygon-only. New options and result fields are added only through
the canonical JSON/options APIs; legacy calls are not extended.
