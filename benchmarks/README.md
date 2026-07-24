# Correctness-gated benchmark records

`benchmark-record-v1.schema.json` defines the only publishable timing record.
It keeps three lanes separate:

1. already-noded polygonization;
2. floating noding plus polygonization;
3. certified fixed-precision noding plus polygonization.

A runner must validate its promised noding postcondition and compare the
topology fingerprint before serializing a record. Expected divergence requires
both the manifest classification and a reason. Failed or unexplained runs are
test artifacts, not timing records.

Records pin the commit, compiler, operating system, architecture, implementation,
features, and dependency versions. Phase names and throughput units are explicit
strings so different runners can report their native phases without pretending
unlike pipelines are equivalent.
