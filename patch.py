import sys

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'use crate::types::{Coord3D, DeterminismOptions, Line3D, Polygon3D};',
    'use crate::types::{Coord3D, DeterminismOptions, Line3D, Polygon3D};\nuse crate::diagnostics::{DiagnosticsOptions, PolygonizerDiagnostics};'
)

content = content.replace(
    'pub determinism: DeterminismOptions,',
    'pub determinism: DeterminismOptions,\n    pub diagnostics_options: DiagnosticsOptions,'
)

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
