import sys
with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'let mut d = PolygonizerDiagnostics::default();\n            d.input_segment_count = self.input_lines.len();',
    'let mut d = PolygonizerDiagnostics {\n                input_segment_count: self.input_lines.len(),\n                ..Default::default()\n            };'
)

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
