import sys

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'pub invalid_rings: Vec<Vec<Coord3D>>,',
    'pub invalid_rings: Vec<Vec<Coord3D>>,\n    pub diagnostics: Option<PolygonizerDiagnostics>,'
)

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
