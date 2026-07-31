# Noding and precision guarantees

Noding makes every topological intersection an endpoint on every incident
segment and normalizes collinear overlap before graph construction. Precision
controls which XY coordinates participate in that topology. They are separate
choices: a fixed grid can round coordinates without noding, and floating noding
can insert intersections without rounding to a grid.

## Choosing a guarantee

| Configuration | Work performed | Contract |
| --- | --- | --- |
| `node_input = false`, `Unchecked` | No intersection insertion. Fixed precision still rounds endpoints. | The caller asserts that the resulting linework is already noded. |
| `node_input = false`, `Validate` | No intersection insertion, followed by the independent validator. | Returns a typed error if the supplied or rounded linework is not fully noded. |
| `node_input = true`, `Unchecked` | Iterative floating or grid candidate noding. | Produces the noder's result without certifying its postcondition. |
| `node_input = true`, `Validate` | Ordinary noding, followed by the independent validator. | Succeeds only when the produced segments pass the full-noding checks. |
| `CertifiedFixedPrecision` | Hot-pixel snap rounding, followed by the independent validator. | Succeeds only for a validated fixed-grid result. |

`CertifiedFixedPrecision` requires all of the following:

- `node_input = true`;
- `PrecisionModel::FixedGrid` with a finite positive `grid_size`;
- the `Snap` backend; and
- `SnapStrategy::Grid`.

Other combinations return `UnsupportedOptionCombination` before geometry work.
Certification is a postcondition for the selected grid, not a claim that the
grid preserves the application's intended narrow features or distinct points.

## Validator postcondition

The independent validator rejects the first deterministic segment pair that
contains:

- a zero-length segment;
- an intersection that is not an endpoint of both segments; or
- an unnormalized collinear overlap.

Failures use the structured noding-validation error family and retain segment
indices plus the failure kind. Validation checks the segments produced by
noding, precision rounding, coordinate restoration, and Z reconciliation; it
does not merely re-check the original input.

## Floating and fixed precision

`PrecisionModel::Floating` preserves source endpoints and computes inserted
intersection coordinates in `f64`. It avoids intentional quantization, but it
does not make floating predicates or constructed coordinates exact real-number
arithmetic.

`PrecisionModel::FixedGrid { grid_size }` maps topology coordinates to integer
grid indices separated by `grid_size` in the input coordinate units. A smaller
grid retains more coordinate detail but may require larger exact grid indices;
a larger grid is more aggressive and can collapse short segments, narrow gaps,
or nearby vertices. Certified inputs must fit the supported integer grid range.

Use `SnapStrategy::Grid` when grid coordinates are the desired output contract.
`GeosCompat` uses the grid for topology and then restores one deterministic
nearest source XY per snapped node. That improves source-coordinate fidelity for
compatibility work, but it is not `set_precision` emulation and is intentionally
ineligible for `CertifiedFixedPrecision`.

## Pre-snap and execution controls

`pre_snap_tolerance` optionally moves endpoints to nearby reference vertices
before noding and therefore requires `node_input = true`. It changes semantic
geometry and should use a tolerance expressed in the same units as the input.

Execution budgets and cancellation are non-semantic controls. They can stop
noding or validation with a structured resource/cancellation error, but raising
a budget does not change the selected precision or guarantee. Diagnostics and
bounded traces report physical candidate, predicate, split, and validation work
without selecting a different algorithm through public semantic options.
