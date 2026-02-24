import json
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PathCollection
from matplotlib.path import Path
from shapely.geometry import shape
import sys
import argparse
import numpy as np

def polygon_to_path(polygon):
    """Converts a Shapely Polygon to a matplotlib Path."""
    if polygon.is_empty:
        return None

    vertices = []
    codes = []

    # Exterior ring
    ext = np.array(polygon.exterior.coords)
    if len(ext) > 0:
        vertices.extend(ext)
        codes.append(Path.MOVETO)
        codes.extend([Path.LINETO] * (len(ext) - 2))
        codes.append(Path.CLOSEPOLY)

    # Interior rings (holes)
    for interior in polygon.interiors:
        inte = np.array(interior.coords)
        if len(inte) > 0:
            vertices.extend(inte)
            codes.append(Path.MOVETO)
            codes.extend([Path.LINETO] * (len(inte) - 2))
            codes.append(Path.CLOSEPOLY)

    if not vertices:
        return None

    return Path(vertices, codes)

def plot_geojson(filepath, ax, color, title, is_polygon=False):
    with open(filepath, 'r') as f:
        data = json.load(f)

    geoms = []
    if data['type'] == 'FeatureCollection':
        for feature in data['features']:
            if feature['geometry']:
                geoms.append(shape(feature['geometry']))
    elif data['type'] == 'GeometryCollection':
        for geom in data['geometries']:
            geoms.append(shape(geom))
    else:
        # Single geometry or Feature
        if 'geometry' in data:
            geoms.append(shape(data['geometry']))
        else:
            geoms.append(shape(data))

    count = 0
    if is_polygon:
        paths = []
        for geom in geoms:
            if geom.geom_type == 'Polygon':
                path = polygon_to_path(geom)
                if path:
                    paths.append(path)
                count += 1
            elif geom.geom_type == 'MultiPolygon':
                for poly in geom.geoms:
                    path = polygon_to_path(poly)
                    if path:
                        paths.append(path)
                count += 1

        if paths:
            # Optimized: Use PathCollection with Paths directly
            p = PathCollection(paths, facecolors=color, edgecolors='black', alpha=0.5)
            ax.add_collection(p)
            ax.autoscale()
    else:
        lines = []
        for geom in geoms:
            if geom.geom_type == 'LineString':
                if not geom.is_empty:
                    lines.append(np.array(geom.coords))
                count += 1
            elif geom.geom_type == 'MultiLineString':
                for line in geom.geoms:
                    if not line.is_empty:
                        lines.append(np.array(line.coords))
                count += 1

        if lines:
            lc = LineCollection(lines, colors=color, linewidths=1, alpha=0.7)
            ax.add_collection(lc)
            ax.autoscale()

    ax.set_title(f"{title} ({count} items)")

def main():
    parser = argparse.ArgumentParser(description="Visualize Polygonization Results")
    parser.add_argument("--input", required=True, help="Input GeoJSON (Lines)")
    parser.add_argument("--output", required=True, help="Output GeoJSON (Polygons)")
    parser.add_argument("--save", help="Save plot to file")
    args = parser.parse_args()

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(12, 6))

    try:
        plot_geojson(args.input, ax1, 'blue', "Input Lines", is_polygon=False)
        plot_geojson(args.output, ax2, 'green', "Output Polygons", is_polygon=True)

        plt.tight_layout()

        if args.save:
            plt.savefig(args.save)
            print(f"Saved visualization to {args.save}")
        else:
            plt.show()

    except Exception as e:
        print(f"Error visualizing: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
