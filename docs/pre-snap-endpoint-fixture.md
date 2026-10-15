# Reviewed endpoint-reference fixture changes

The endpoint-region insertion correction changes one existing public CFB fixture,
`large_lot_block_anon_curb_snap_gap`. Its original source coordinates, line IDs,
`cfb_robust_v1` options, and area tolerance are unchanged. Only the reviewed
expected polygon count, summed area, dangle count, and explanation are updated.

| Metric | Previous output | Corrected output |
| --- | ---: | ---: |
| Polygons | 1,253 | 1,256 |
| Summed polygon area | 8,797,796.79145574 | 8,797,796.791472947 |
| Dangles | 2,570 | 2,569 |
| Cuts | 49 | 49 |
| Invalid rings | 0 | 0 |
| Owned holes | 10 | 10 |

All 1,256 polygons were independently checked with Shapely 2.1.2 / GEOS 3.13.1:
they are valid, and there are no positive-area overlapping polygon pairs. The
union gains approximately 0.00001722032380155 square input units and loses no
area. The other three public CFB fixtures retain their geometry and all asserted
metrics, including the Z-conflict and boundary-source-ID checks.

## Small exact reductions

Two subsets of the original public fixture reproduce the three added faces:

- Source IDs `207, 2803`: one triangle of area 0.000185459140050504. It is split
  from an existing face, and its short edge accounts for the removed dangle.
  The old pre-snap result has an open boundary; corrected pre-snap contains all
  three sides. GEOS independently polygonizes that same prepared face exactly.
- Source IDs `2262, 3038, 3057`: two triangles of areas 0.000015179968006478 and
  0.000002040355795070. These depend on both the corrected pre-snap insertion
  and the existing grid/source-coordinate-restoration policy. They are valid
  native boundaries, but are not claimed to match GEOS whole-LineString snapping.

The new `cfb_endpoint_reference_insertions_retain_reviewed_microfaces` regression
uses those original source groups and asserts triangle counts, positive areas,
simple triangular structure, and exact boundary source IDs. Both groups produce
zero faces before the endpoint correction. Run it together with the complete
fixture gates:

```sh
cargo test --locked -p geo-polygonize-core --test cfb_fixtures
```

## Interpretation limits

The profile returns raw bounded faces without an application minimum-area filter.
These are three microfaces, not three recovered lot-sized regions. The historical
comparison with 1,253 area-filtered GEOS polygons is no longer a claim of equal
raw counts. Source attribution can also gain IDs on retained faces when inserted
connectors overlap their boundaries; unchanged geometry does not imply unchanged
provenance.

This is a deliberately reviewed geometry expectation change, not a blanket golden
regeneration or proof of GEOS topology parity. Application filtering, source
priority, grading, and the versioned profile/release contract remain adoption
decisions outside this correctness patch.
