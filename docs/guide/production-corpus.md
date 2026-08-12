# Production-scale corpus

`benchmarks/production-corpus-v1.json` registers redistributable source
artifacts for production-scale benchmark work. It is not a checked-in geometry
fixture, a runnable workload, or benchmark evidence. The current entry pins a
dated 1.32 GB California OpenStreetMap extract and the checksum published by
its distributor; no source geometry is stored in this repository.

The checked-in `tests/workloads/manifest-v1.json` remains the default input
manifest. The runner and GEOS reference helper also accept `--manifest`, so a
materialized manifest can stay outside Git while using the same correctness
gate.

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
artifact=target/production-corpus/california-260801.osm.pbf
test "$(wc -c < "$artifact" | tr -d ' ')" = 1322245000
test "$(openssl dgst -md5 -r "$artifact" | awk '{print $1}')" = 3be0a7bdf02572622c791b89063638a0
```

The distributor publishes MD5 for this artifact, so it is a provenance and
corruption check rather than a substitute for a content-addressed source. D2
must record a SHA-256 for every derived linework artifact, the converter and
its exact version, its selection options, line/segment counts, and a topology
reference before the workload enters `tests/workloads/` or supports a timing
claim. Treat bridge/tunnel grade separation as input semantics to document,
not planar intersections to invent.

## Reproducible materialization

`materialize_production_workloads.py` converts the pinned `lines` layer with
the highway filter, verifies the raw byte length and publisher MD5, requires
monotonic OSM way IDs, retains complete source ways, and emits deterministic
FeatureCollections. It records the converter version and command, source and
derived SHA-256 values, geographic extent, chain/component/grid descriptors,
exact duplicate incidence, and the compatibility contract. The generated
manifest and geometry remain out of tree:

```bash
python3 scripts/materialize_production_workloads.py \
  --source target/production-corpus/california-260801.osm.pbf \
  --output-dir target/production-corpus/materialized \
  --acquired-on 2026-08-11
```

The default run emits approximately 1k, 10k, and 100k segment tiers. Add
`--include-million` only when the local disk and correctness runner can handle
the larger derived artifact. The authorized run also emitted an out-of-tree
`1,000,009`-segment tier. It is not yet correctness-gated or a timing
publication: a dedicated runner/reference gate is not currently provisioned
for a decision-quality 1m run, which is the concrete blocker recorded in the
roadmap.

After generating references and running the correctness gate, rerun with
`--validation-dir target/production-corpus/materialized/validation` to attach
candidate, exact-predicate, split, topology, and fingerprint evidence to the
materialization report. The external manifest uses the existing runner:

```bash
python3 benchmarks/check_geos_references.py \
  --manifest target/production-corpus/materialized/runner-manifest-v1.json \
  --output-dir target/production-corpus/materialized/references \
  --validation-output-dir target/production-corpus/materialized/validation \
  --repeat
```

The production run completed locally for `1,003`, `10,093`, and `100,013`
segments. GEOS/Shapely parity, repeated-run determinism, and serial/parallel
equality passed for all three tiers. Those local checks are correctness
evidence, not dedicated-runner timing publications.

## Dedicated baseline publication

Stage the generated `runner-manifest-v1.json` and its relative clip directory
on the dedicated runner, then dispatch the publication workflow with its
absolute manifest path:

```bash
gh workflow run benchmark-publication.yml --ref main \
  -f workload=osm-california-highways-10k-v1 \
  -f lane=floating \
  -f manifest_path=/mnt/geo-polygonize/runner-manifest-v1.json
```

The workflow keeps the staged bundle out of Git, resolves clips relative to the
manifest, and verifies each declared SHA-256 before generating a reference or
running the Rust correctness gate. The five independent timing processes then
publish only through the existing dedicated-runner policy. A certified-fixed
manifest follows the same path through the pinned JTS container and must
explicitly declare the certified-fixed profile; the materializer's current OSM
manifest intentionally permits only the floating profile.

The derived road tiers cover the network category. Coverage, hydrographic, and
CAD categories remain represented by the existing public/procedural fixtures;
the materialization report records those choices and reasons. No CFB or
customer geometry is used.

The required cross-workload baseline matrix is
`benchmarks/production-baseline-suite-v1.json`. Repeat the dedicated publication
workflow for each matrix entry, download each resulting `publication.json` into
one directory on the dedicated runner, then validate the complete suite:

```bash
gh workflow run benchmark-baseline-suite.yml --ref main \
  -f publication_dir=/mnt/geo-polygonize/publications
```

The suite workflow refuses incomplete or mixed-commit collections and retains
only the checksum-linked `production-baseline-evidence-v1.json` summary. This
does not close P1.2 until all seven required dedicated-runner publications
exist; local or hosted correctness runs remain diagnostic.

## Adding a source

Add a source only after independently reading its license and publisher
metadata. Use a dated object URL, copy the publisher checksum verbatim with
its checksum URL, and add a static manifest check. A source remains
`source-pinned` in the checked-in source manifest. The out-of-tree
materialization report upgrades the derived evidence only after it supplies
SHA-256 values and the normal correctness gate accepts each workload.
