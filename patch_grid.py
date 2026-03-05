import re

content = open("crates/geo-polygonize-core/src/noding/grid.rs").read()
if '#[cfg(feature = "parallel")]\n        const MAX_CELLS_PER_LINE: usize = 50;' in content:
    content = content.replace('#[cfg(feature = "parallel")]\n        const MAX_CELLS_PER_LINE: usize = 50;', 'const MAX_CELLS_PER_LINE: usize = 50;')
    open("crates/geo-polygonize-core/src/noding/grid.rs", "w").write(content)
    print("Replaced successfully")
else:
    print("Could not find line")
