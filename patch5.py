import sys

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

# Make sure we add std::time::Instant
if 'use std::time::Instant;' not in content:
    content = content.replace('use geo::Contains;', 'use geo::Contains;\nuse std::time::Instant;')

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
