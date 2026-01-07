import json
import matplotlib.pyplot as plt
from shapely.geometry import shape
from shapely.plotting import plot_line, plot_polygon
import sys
import argparse

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
    for geom in geoms:
        if is_polygon:
            if geom.geom_type in ['Polygon', 'MultiPolygon']:
                plot_polygon(geom, ax=ax, facecolor=color, edgecolor='black', alpha=0.5)
                count += 1
        else:
            if geom.geom_type in ['LineString', 'MultiLineString']:
                plot_line(geom, ax=ax, color=color, linewidth=1, alpha=0.7)
                count += 1

    ax.set_title(f"{title} ({count} items)")
    ax.autoscale()

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
