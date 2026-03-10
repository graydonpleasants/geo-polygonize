import sys

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'determinism: DeterminismOptions::default(),',
    'determinism: DeterminismOptions::default(),\n            diagnostics_options: DiagnosticsOptions::default(),'
)

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
