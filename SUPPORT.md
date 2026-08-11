# Support policy

This policy describes the compatibility commitments for published
`geo-polygonize` releases. It applies to the Rust crates, the
`geo-polygonize` npm package, and the `geo-polygonize-py` Python package.

## Supported API surface

For Rust, the supported facade is the non-hidden API exported from the
`geo-polygonize-core` crate root. The checked-in
[`stable-api-v1.txt`](release/stable-api-v1.txt) allowlist is the compatibility
gate for that facade. Modules and exports marked `#[doc(hidden)]` exist for
repository tooling, benchmarks, diagnostics, or experimental use; they are
not stable API. In particular, graph, noding, containment, tiling, trace,
differential, utility, and mutable-builder interfaces remain unsupported 1.x
research surfaces.

For JavaScript and Python, only documented package exports are supported.
Generated Wasm glue, generated TypeScript bindings, the native Python extension
module, and repository scripts are implementation details.

## Rust toolchain and targets

The minimum supported Rust version (MSRV) is Rust `1.87.0`. Published Cargo
packages declare `rust-version = "1.87"`, and CI continuously checks the
workspace with that exact toolchain. The MSRV may advance only in a documented
minor release with migration guidance; patch releases never raise it.

The required target gates are:

| Surface | Required gate |
| --- | --- |
| Native Rust | `x86_64-unknown-linux-gnu` on stable Rust |
| Wasm scalar | `wasm32-unknown-unknown` on stable Rust |
| Wasm SIMD | `wasm32-unknown-unknown` with `simd128` on stable Rust |
| Wasm threads | `wasm32-unknown-unknown` with atomics and `build-std` on the nightly selected by the build workflow |
| Python | CPython 3.8 or newer through PyO3 `abi3-py38` |

Other native Rust targets are best-effort unless a release workflow emits and
tests an artifact for that target. Release-time Python wheel builds cover the
platforms listed in `.github/workflows/publish-python.yml`; those builds do not
expand the native Rust CI guarantee. PyPy is unsupported and is intentionally
not advertised as a package classifier.

Wasm SIMD requires host SIMD support. Wasm threads additionally require shared
memory, atomics, and the browser isolation headers described in the Wasm guide.
The scalar package is the compatibility fallback.

## Feature combinations

Supported Cargo feature combinations are intentionally small:

| Package | Combination | Meaning |
| --- | --- | --- |
| `geo-polygonize-core` | `--no-default-features` | Serial core |
| `geo-polygonize-core` | default or `--all-features` | Parallel core; `parallel` is currently the only optional feature |
| `geo-polygonize-arrow` | default | Arrow and GeoArrow adapters |
| `geo-polygonize-arrow` | `geoparquet` | Optional GeoParquet adapter |
| `geo-polygonize-wasm` | default | Scalar/SIMD-compatible binding with the console panic hook |
| `geo-polygonize-wasm` | `threads` | Thread-pool binding; also enables core parallelism |
| `geo-polygonize-python` | `extension-module` | Published PyO3 extension |

Combinations outside this table are not release gates. SIMD is selected with a
target feature rather than a Cargo feature.

## Versions, semver, and deprecation

The supported facade follows standard semantic versioning. Patch releases are
reserved for compatible fixes and documentation; minor releases may add
supported functionality and may raise the documented MSRV. Hidden or
explicitly experimental APIs are excluded from these guarantees.

When practical, a planned breaking rename or replacement is deprecated for at
least one minor release and includes migration guidance in the changelog.
Incorrect, unsafe, or unsupportable behavior may be removed immediately; the
release notes must identify the exception and replacement.

The retired `NodingBackend::Advanced` compatibility alias was removed before
the 1.0 release; use `NodingBackend::Snap` with `PrecisionModel::Floating` for
the same exact snap-noding behavior.

The retired `TileOwnershipPolicy::CanonicalBoundaryHash` compatibility alias
was removed before the 1.0 release; use `RepresentativePointInsidePolygon`,
which already provided the same ownership behavior.

Source versions for the Rust crates, npm package, and Python package must match.
The required release-contract check enforces this before merge. Registry
publication is performed by separate tag-triggered workflows and is not atomic;
the post-publication report gate requires maintainers to repair a failed
publication before starting a new release.

The root facade is the only supported Rust compatibility surface. A
`#[doc(hidden)]` item is still compiler-public, so hiding it from rustdoc does
not stabilize it or make it private. Research changes remain in the current
crate for 1.x compatibility with experimental consumers, but they must not be
added to the stable allowlist or re-exported from a supported binding. The
longer-term isolation path is a separate research crate or explicit unstable
feature in the next planned major release.

Release-please cannot infer public API impact from Rust implementation details.
The repository therefore keeps the conservative rule that internal
`feat(core)` commits may produce a minor release; agents must not relabel them
as patch changes merely because the touched item is doc-hidden. The changelog
and stable allowlist remain the review gates for public impact.

For the 0.x to 1.x migration, see the
[0.x to 1.0 migration guide](docs/guide/migration-1-0.md).

## Errors, panics, and execution control

Supported Rust polygonization entrypoints return `Result` and use typed errors
for invalid inputs/options, topology failures, Z conflicts, noding validation,
resource limits, and cancellation. Resource limits are opt-in logical work and
output guards, not a process-memory sandbox.

Cancellation is cooperative. `CancellationToken` is safe to clone and cancel
from another thread: any clone may cancel, and cancellation is monotonic for
the token lifetime. Tokens are single-run, so each new operation needs a fresh
token. Cancellation is observed only at explicit polling points; it is not
preemptive. Python computation runs on a worker without holding the GIL, polls
Python signals, and maps an interrupt to cooperative cancellation. Wasm does
not currently expose the native cancellation token.

A panic from a supported Rust entrypoint is a bug, not part of the error
contract. Native Arrow C ABI and Python boundaries catch unwinding panics and
translate them to boundary errors. `wasm32-unknown-unknown` uses aborting panics,
so an unexpected panic traps and the caller must replace the Wasm instance. See
[the security policy](SECURITY.md#panic-safety) for the boundary details.

## Thread safety

Independent core calls do not share mutable geometry state. The `parallel`
feature uses Rayon internally. A `PolygonizerWorkspace` or mutable
`Polygonizer` must not be used concurrently; callers may create one instance
per concurrent operation. `CancellationToken` is the supported cross-thread
control handle.

Python calls copy inputs into owned Rust data before starting worker computation
and may run concurrently as independent calls. The Wasm threads build owns a
Rayon worker pool behind its generated package API; callers must follow that
package's initialization and browser-isolation requirements.
