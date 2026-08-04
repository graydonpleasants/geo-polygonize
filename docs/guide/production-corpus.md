# Production-scale corpus

`benchmarks/production-corpus-v1.json` registers redistributable source
artifacts for production-scale benchmark work. It is not a checked-in geometry
fixture, a runnable workload, or benchmark evidence. The current entry pins a
dated 1.32 GB California OpenStreetMap extract and the checksum published by
its distributor; no source geometry is stored in this repository.

The existing `tests/workloads/manifest-v1.json` remains the only input manifest
accepted by the benchmark runner. D2 is responsible for materializing a source
into a derived, checksum-pinned linework fixture before that changes.

## Acquisition and verification policy

Only a public artifact with an explicit redistribution license, attribution,
immutable download URL, publisher checksum URL, and byte length belongs in the
manifest. Do not register private data, `latest` URLs, or an artifact whose
checksum was calculated from an unreviewed local copy.

The registered California artifact is OpenStreetMap data under ODbL. Preserve
the stated attribution and comply with ODbL terms when sharing a derived
database. A derived benchmark input must retain its source ID and license
notice even when the raw PBF is not distributed.

Download only when a dedicated D2 materialization run is authorized. Keep the
PBF outside Git (for example, under `target/production-corpus/`) and verify it
before conversion; CI does not download it:

```bash
artifact=target/production-corpus/california-260802.osm.pbf
test "$(wc -c < "$artifact" | tr -d ' ')" = 1322385715
test "$(openssl dgst -md5 -r "$artifact" | awk '{print $1}')" = 5915ee72b206cc184e0940cde071a43a
```

The distributor publishes MD5 for this artifact, so it is a provenance and
corruption check rather than a substitute for a content-addressed source. D2
must record a SHA-256 for every derived linework artifact, the converter and
its exact version, its selection options, line/segment counts, and a topology
reference before the workload enters `tests/workloads/` or supports a timing
claim. Treat bridge/tunnel grade separation as input semantics to document,
not planar intersections to invent.

## Adding a source

Add a source only after independently reading its license and publisher
metadata. Use a dated object URL, copy the publisher checksum verbatim with
its checksum URL, and add a static manifest check. A source stays
`source-pinned` until the D2 materialization record supplies a derived
SHA-256 and the normal correctness gate accepts the resulting workload.
