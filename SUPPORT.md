# Support policy

This policy describes the compatibility commitments for published
`geo-polygonize` releases. It applies to the Rust crates, the
`geo-polygonize` npm package, and the `geo-polygonize-py` Python package.

## Supported API surface

For Rust, the supported facade is the non-hidden API exported from the
`geo-polygonize-core` crate root. Modules and exports marked `#[doc(hidden)]`
exist for repository tooling, benchmarks, diagnostics, or experimental use;
they are not stable API. In particular, graph, noding, containment, tiling,
trace, differential, utility, and mutable-builder interfaces may change before
`1.0`.

For JavaScript and Python, only documented package exports are supported.
Generated Wasm glue, generated TypeScript bindings, the native Python extension
module, and repository scripts are implementation details.

## Rust toolchain and targets

The minimum supported Rust version (MSRV) is the latest stable Rust release.
There is currently no fixed numeric MSRV and no `package.rust-version` value.
Pull-request CI uses the stable channel, so the supported compiler may advance
with a minor release before `1.0`. A fixed MSRV will be declared in Cargo
metadata only when that exact toolchain is continuously tested.

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
expand the native Rust CI guarantee. PyPy is not supported until it has an
import-and-call CI gate.

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

Until `1.0`, incompatible changes to the supported facade may ship in a minor
release. Patch releases are reserved for compatible fixes and documentation.
After `1.0`, the supported facade follows standard semantic versioning.
Hidden or explicitly experimental APIs are excluded from these guarantees.

When practical, a planned breaking rename or replacement is deprecated for at
least one minor release and includes migration guidance in the changelog.
Incorrect, unsafe, or unsupportable behavior may be removed immediately; the
release notes must identify the exception and replacement.

Source versions for the Rust crates, npm package, and Python package must match.
The required release-contract check enforces this before merge. Registry
publication is performed by separate tag-triggered workflows and is not atomic;
if one registry fails, maintainers repair that publication before starting a
new release.

## Errors, panics, and execution control

Supported Rust polygonization entrypoints return `Result` and use typed errors
for invalid inputs/options, topology failures, Z conflicts, noding validation,
resource limits, and cancellation. Resource limits are opt-in logical work and
output guards, not a process-memory sandbox.

Cancellation is cooperative. `CancellationToken` is safe to clone and cancel
from another thread, but cancellation is observed only at explicit polling
points; it is not preemptive. Python computation runs on a worker without
holding the GIL, polls Python signals, and maps an interrupt to cooperative
cancellation. Wasm does not currently expose the native cancellation token.

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
