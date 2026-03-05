import re

content = open("crates/geo-polygonize-core/src/types.rs").read()

if "assert!((centroid.x() - expected_x).abs() < 1e-10);" in content:
    content = content.replace("assert!((centroid.x() - expected_x).abs() < 1e-10);", 'println!("centroid = {:?}, expected = {:?}", centroid, (expected_x, expected_y));\n        assert!((centroid.x() - expected_x).abs() < 1e-10);')
    open("crates/geo-polygonize-core/src/types.rs", "w").write(content)
    print("Replaced successfully")
else:
    print("Could not append tests")
