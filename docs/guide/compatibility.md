# Compatibility profiles and divergences

Compatibility is defined by an exact input, options object, retained result
families, and reference implementation version. A profile name is shorthand for
that contract; it is not a promise that every GEOS, JTS, or Shapely release will
produce byte-identical geometry.

## Supported profiles

`PolygonizerOptions::default()` is the canonical already-noded, floating profile:
it does not insert intersections and uses deterministic output ordering. Callers
must supply already-noded linework or select validation/noding explicitly.

`PolygonizerOptions::cfb_robust_v1()` is the versioned CFB/CAD compatibility
profile. It enables noding, a `0.1` fixed grid, `0.5` reference-vertex pre-snap,
`GeosCompat` coordinate restoration, diagnostics, provenance, and the
`cfb_robust_v1` profile ID. The Python `cfb_robust_options()` helper emits the
same canonical options. Changing one of those values creates a different
profile; retain the versioned name in stored reports.

Public benchmark records use three workload lanes instead of implicit tuning:

- `already-noded` polygonization;
- floating noding plus polygonization; and
- certified fixed-precision noding plus polygonization.

Each workload manifest declares which lanes are permitted. A benchmark result is
valid only after its lane's correctness gate and reference classification pass.

## Classification contract

Compatibility fixtures use explicit classifications:

- `expected_parity` / `parity` means the retained comparable result agrees with
  the pinned reference within the fixture's stated contract;
- `expected_divergence` records a reviewed, reproducible difference with an
  exact witness; and
- `invalid_ambiguous` (or manifest `invalid` / `ambiguous`) means no single
  external topology result is asserted as ground truth.

Polygon count or total area alone is insufficient when both sides retain exact
topology. Use the versioned fingerprint or normalized error. Reduced GEOS/JTS
references compare only the fields they can represent and must not be promoted
to full Z, provenance, representative-edge, diagnostics, or error equality.

## Known intentional or recorded divergences

- The persisted `floating_microfaces` case retains two distinct `f64`
  intersection coordinates separated by a few bits, producing tiny faces that
  differ from the pinned GEOS reference. It remains an expected divergence, not
  a tolerance-based silent pass.
- `GeosCompat` restores one deterministic nearest source XY per snapped node. It
  targets Shapely-style snap plus full-precision noding, but it is not GEOS
  `set_precision` emulation and can diverge for degenerate or many-to-one snaps.
- Z reconstruction, Z conflicts, source provenance, representative edge IDs,
  diagnostics, and execution errors have no equivalent in polygon-only
  GEOS/Shapely references. Equality claims must exclude those fields or use a
  binding that retains the Rust contract.
- Dirty CAD gaps and overshoots can change topology when pre-snap tolerance or
  grid size changes. Results from different semantic profiles are not expected
  to compare equal merely because both are described as robust noding.

Novel differences are review candidates, not automatic golden fixtures. The
differential workflow retains exact input bits, source IDs, options, versions,
both outcomes, and a first witness before a human assigns a compatibility class.

## Reproducing comparisons

Use the checked-in compatibility corpus for application profiles and the public
workload manifest for benchmark lanes. Reference jobs pin Shapely with its GEOS
version and pin JTS for the certified lane. Record those dependency versions and
the repository commit with every result; a reference upgrade is new evidence and
must not silently rewrite an existing fixture classification.
