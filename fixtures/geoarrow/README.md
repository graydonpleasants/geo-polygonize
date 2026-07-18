# GeoArrow conformance fixtures

The shared test helper builds the official GeoArrow toy LineString corpus from
the MIT/Apache-2.0 licensed `geoarrow-test` 0.7 crate in both supported layouts:

- [separated coordinates](https://geoarrow.org/data.html)
- [interleaved coordinates](https://geoarrow.org/data.html)

Both contain two LineStrings, one null, and one empty geometry. Tests generate
the Arrow IPC in memory; no network access is required.
