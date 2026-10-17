# Nested point-touch ring correction

Six grid-aligned edges expose a ring-extraction defect independently of snapping:

- Outer triangle: (-10,10), (0,0), (0,-10), (-10,10).
- Inner triangle: (0,0), (-2,1), (-2,-1), (0,0).

The two bounded faces have areas 48 and 2. The larger face has one triangular hole touching its shell only at (0,0). Before this correction, the larger output embeds the inner loop in its shell and repeats (0,0), producing an invalid self-touching ring despite unchanged counts and areas. This occurs with floating/fixed precision, with/without noding, and under `cfb_robust_v1`.

Outgoing edges are sorted counter-clockwise. The maximal-to-minimal conversion must pair each incoming edge of a ring label with the next outgoing edge in that order. Reversing that traversal preserves the maximal repeated-vertex walk instead of splitting its minimal boundary rings.

Partition physical witnesses intentionally model maximal topological face cycles for Euler/adjacency checks, rather than the minimal rings of polygon output. Their preparation now states that distinction explicitly. Existing partition eligibility and conservative rejection tests remain unchanged.

## Reviewed snapshot change

Only the `bowtie` fingerprint in `serial_parallel_outcomes_v1.json` changes. Exact polygon coordinates, edge IDs, source IDs, dangles, cuts and invalid-ring sidecars remain unchanged. Correct splitting changes diagnostics:

- ring_count: 3 -> 4
- hole_count: 1 -> 2
- unassigned_hole_count: 1 -> 2
- unassigned_hole_area remains 50

The new bowtie regression explicitly checks these semantics. This is a manually reviewed diagnostic expectation change, not blanket golden regeneration.
