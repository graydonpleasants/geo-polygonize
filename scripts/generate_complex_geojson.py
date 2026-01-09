import json
import numpy as np
from shapely.geometry import Point, LineString, mapping
import os

def create_circle(x, y, r, points=100):
    angles = np.linspace(0, 2*np.pi, points)
    coords = []
    for a in angles:
        coords.append((x + r * np.cos(a), y + r * np.sin(a)))
    return LineString(coords)

def main():
    os.makedirs("examples/data", exist_ok=True)

    # 1. Overlapping Circles
    # Three circles: (0,0), (10,0), (5, 8.66)
    c1 = create_circle(30, 30, 30)
    c2 = create_circle(60, 30, 30)
    c3 = create_circle(45, 55, 30)

    features = []
    for geom in [c1, c2, c3]:
        features.append({
            "type": "Feature",
            "properties": {},
            "geometry": mapping(geom)
        })

    with open("examples/data/overlapping_circles.geojson", "w") as f:
        json.dump({"type": "FeatureCollection", "features": features}, f)

    # 2. Curved Holes (Swiss Cheese)
    # Large outer circle
    outer = create_circle(50, 50, 50, points=200)

    # Random holes
    holes = []
    holes.append(create_circle(30, 30, 10))
    holes.append(create_circle(70, 30, 10))
    holes.append(create_circle(50, 70, 15))
    holes.append(create_circle(50, 40, 5)) # Central small one

    features = []
    features.append({
        "type": "Feature",
        "properties": {},
        "geometry": mapping(outer)
    })
    for h in holes:
         features.append({
            "type": "Feature",
            "properties": {},
            "geometry": mapping(h)
        })

    with open("examples/data/curved_holes.geojson", "w") as f:
        json.dump({"type": "FeatureCollection", "features": features}, f)

    print("Generated examples/data/overlapping_circles.geojson and examples/data/curved_holes.geojson")

if __name__ == "__main__":
    main()
