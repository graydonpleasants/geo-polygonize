import json
import subprocess
import tempfile
import time
import pytest
import numpy as np
import os
import random
from shapely.geometry import LineString, shape, mapping, Point
from shapely.ops import polygonize, unary_union

def generate_t_junction_soup(num_lines=50, seed=42):
    """
    Generates a soup of lines where new lines start from the interior of existing lines.
    """
    rng = random.Random(seed)
    lines = []

    # Start with a few base lines
    for _ in range(5):
        x1, y1 = rng.uniform(0, 100), rng.uniform(0, 100)
        x2, y2 = rng.uniform(0, 100), rng.uniform(0, 100)
        lines.append(LineString([(x1, y1), (x2, y2)]))

    # Add lines branching off existing ones
    for _ in range(num_lines - 5):
        base_line = rng.choice(lines)
        # Pick a point on the line
        point = base_line.interpolate(rng.uniform(0.1, 0.9), normalized=True)
        # Random angle and length
        angle = rng.uniform(0, 2 * np.pi)
        length = rng.uniform(5, 20)
        x2 = point.x + length * np.cos(angle)
        y2 = point.y + length * np.sin(angle)

        # Round coordinates to avoid extreme precision issues
        p1 = (round(point.x, 5), round(point.y, 5))
        p2 = (round(x2, 5), round(y2, 5))

        lines.append(LineString([p1, p2]))

    return lines

def generate_collinear_overlaps(num_lines=20, seed=42):
    """
    Generates heavily overlapping lines on the same axis.
    """
    rng = random.Random(seed)
    lines = []

    # Horizontal lines at different Y levels
    for y in range(0, 10, 2):
        for _ in range(num_lines // 5):
            x1 = rng.uniform(0, 10)
            x2 = rng.uniform(0, 10)
            if x1 > x2:
                x1, x2 = x2, x1
            # Ensure some length
            if x2 - x1 < 0.1:
                x2 += 1.0
            lines.append(LineString([(x1, y), (x2, y)]))

    return lines

def generate_grid(rows=10, cols=10, seed=42):
    """
    Generates a grid of intersecting lines.
    """
    lines = []
    # Horizontal
    for r in range(rows):
        lines.append(LineString([(0, r), (cols, r)]))
    # Vertical
    for c in range(cols):
        lines.append(LineString([(c, 0), (c, rows)]))
    return lines

def generate_self_intersecting_figure_8s(num_figures=10, seed=42):
    """
    Generates self-intersecting figure-8 shapes (bowties).
    """
    rng = random.Random(seed)
    lines = []

    for _ in range(num_figures):
        cx, cy = rng.uniform(0, 100), rng.uniform(0, 100)
        size = rng.uniform(5, 20)

        # Bowtie: (0,0) -> (1,1) -> (1,0) -> (0,1) -> (0,0)
        p1 = (cx, cy)
        p2 = (cx + size, cy + size)
        p3 = (cx + size, cy)
        p4 = (cx, cy + size)

        lines.append(LineString([p1, p2, p3, p4, p1]))

    return lines

def generate_spiderwebs(num_webs=5, rays_per_web=50, seed=42):
    """Multiple hubs where dozens of lines intersect at exactly one vertex.
    Uses integer coordinates to ensure exact intersection at the hub.
    """
    rng = random.Random(seed)
    lines = []
    for _ in range(num_webs):
        # Integer center
        cx, cy = rng.randint(20, 80), rng.randint(20, 80)
        for _ in range(rays_per_web):
            # Random integer vector (dx, dy)
            dx = rng.randint(-20, 20)
            dy = rng.randint(-20, 20)
            if dx == 0 and dy == 0:
                dx, dy = 1, 1

            # Line passing through (cx, cy)
            x1 = cx - dx
            y1 = cy - dy
            x2 = cx + dx
            y2 = cy + dy

            lines.append(LineString([(x1, y1), (x2, y2)]))
    return lines

def generate_concentric_nested_polygons(depth=10, center=(50,50)):
    """Shell -> Hole -> Shell -> Hole nesting."""
    lines = []
    cx, cy = center
    for i in range(1, depth + 1):
        radius = i * 5.0
        # Approximate a circle with an octagon or hexadecagon
        # Use endpoint=False and manual closure to ensure exact ring closure
        angles = np.linspace(0, 2*np.pi, 16, endpoint=False)
        coords = []
        for a in angles:
             x = cx + radius * np.cos(a)
             y = cy + radius * np.sin(a)
             coords.append((round(x, 5), round(y, 5)))

        # Explicitly close the ring by repeating the first point
        coords.append(coords[0])
        lines.append(LineString(coords))
    return lines

def generate_dumbbells_and_antennas():
    """Valid polygons connected by single-line bridges, covered in dangles."""
    lines = []
    # Square 1
    lines.append(LineString([(0,0), (10,0), (10,10), (0,10), (0,0)]))
    # Square 2
    lines.append(LineString([(20,0), (30,0), (30,10), (20,10), (20,0)]))
    # The Bridge (Cut-edge)
    lines.append(LineString([(10,5), (20,5)]))
    # The Antennas (Dangles)
    lines.append(LineString([(0,10), (-5, 15), (-10, 20)]))
    lines.append(LineString([(30,0), (35, -5)]))
    # Internal Cut-line (line projecting into the polygon but not closing)
    lines.append(LineString([(5,0), (5, 5)]))
    return lines

def generate_micro_gaps_and_overlaps(num_pairs=50, seed=42):
    """Lines separated by distances near the float64 epsilon limits."""
    rng = random.Random(seed)
    lines = []
    for _ in range(num_pairs):
        base_y = rng.uniform(0, 100)
        # Exactly horizontal line
        lines.append(LineString([(0, base_y), (10, base_y)]))

        # Line that overlaps by 1e-11 on the Y axis
        tiny_offset = rng.choice([1e-11, 1e-12, 1e-9])
        lines.append(LineString([(5, base_y + tiny_offset), (15, base_y + tiny_offset)]))

        # Line that almost touches the end
        lines.append(LineString([(10 + tiny_offset, base_y), (20, base_y)]))
    return lines

def generate_extreme_translation_grids(offset=1e9):
    """A standard grid but translated massively far away from origin."""
    lines = generate_grid(rows=5, cols=5) # reuse existing grid generator
    translated_lines = []
    for line in lines:
        coords = [(x + offset, y + offset) for x, y in line.coords]
        translated_lines.append(LineString(coords))
    return translated_lines

def generate_winding_chaos():
    """Identical boundaries drawn in opposite directions and fragmented."""
    lines = []
    # Square CW
    lines.append(LineString([(0,0), (0,10), (10,10), (10,0), (0,0)]))
    # Square CCW
    lines.append(LineString([(0,0), (10,0), (10,10), (0,10), (0,0)]))

    # Fragmented overlapping lines
    lines.append(LineString([(0,0), (5,0)]))
    lines.append(LineString([(10,0), (2,0)])) # Reversed and overlapping

    return lines

def run_shapely(lines):
    """The industry standard ground-truth GEOS recipe."""
    noded = unary_union(lines)
    return list(polygonize(noded))

def run_rust_cli(lines):
    """Executes the Rust engine via the CLI example."""
    with tempfile.NamedTemporaryFile(suffix='.geojson', mode='w', delete=False) as f_in, \
         tempfile.NamedTemporaryFile(suffix='.geojson', mode='r', delete=False) as f_out:

        # Write input
        fc = {
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "geometry": mapping(l), "properties": {}} for l in lines]
        }
        json.dump(fc, f_in)
        f_in.flush()
        f_in.close() # Close to ensure flush to disk

        # Check for pre-built release binary to avoid cargo overhead
        binary_path = os.path.join("target", "release", "examples", "polygonize")
        if os.path.exists(binary_path):
            cmd = [binary_path, "--input", f_in.name, "--output", f_out.name, "--node"]
        else:
            # Fallback to cargo run
            cmd = [
                "cargo", "run", "--release", "-p", "geo-polygonize-core", "--example", "polygonize",
                "--", "--input", f_in.name, "--output", f_out.name, "--node"
            ]

        # Using capture_output=True to suppress stdout unless error
        try:
            subprocess.run(cmd, check=True, capture_output=True, text=True)

            # Read output
            # Re-open f_out to read
            with open(f_out.name, 'r') as f_read:
                 # Check if file is empty
                content = f_read.read()
                if not content:
                    # If empty, maybe no polygons found or error?
                    return []
                f_read.seek(0)
                out_fc = json.load(f_read)

            return [shape(f["geometry"]) for f in out_fc["features"]]
        finally:
            if os.path.exists(f_in.name):
                os.unlink(f_in.name)
            if os.path.exists(f_out.name):
                os.unlink(f_out.name)

def assert_parity(shapely_polys, rust_polys):
    """Compares topological equivalence using geometric symmetric difference."""
    # Filter empty polygons if any (though shapely/rust shouldn't produce them usually)
    shapely_polys = [p for p in shapely_polys if not p.is_empty and p.area > 1e-9]
    rust_polys = [p for p in rust_polys if not p.is_empty and p.area > 1e-9]

    # Compute the unary_union of the Shapely polygons and the unary_union of the Rust polygons
    shapely_union = unary_union(shapely_polys)
    rust_union = unary_union(rust_polys)

    # Assert that the symmetric_difference between these two unions has an area of < 1e-5
    diff = shapely_union.symmetric_difference(rust_union)
    assert diff.area < 1e-5, f"Spatial parity failed. Symmetric difference area: {diff.area}"

@pytest.mark.parametrize("generator", [
    generate_t_junction_soup,
    generate_collinear_overlaps,
    generate_grid,
    generate_self_intersecting_figure_8s,
    generate_spiderwebs,
    generate_concentric_nested_polygons,
    generate_dumbbells_and_antennas,
    generate_micro_gaps_and_overlaps,
    generate_extreme_translation_grids,
    generate_winding_chaos,
])
def test_differential_parity(generator):
    lines = generator()

    start_shapely = time.perf_counter()
    shapely_result = run_shapely(lines)
    shapely_time = time.perf_counter() - start_shapely

    start_rust = time.perf_counter()
    rust_result = run_rust_cli(lines)
    rust_time = time.perf_counter() - start_rust

    assert_parity(shapely_result, rust_result)

    # Soft assertion for time complexity (with overhead buffer for small inputs)
    # Using a constant floor of 1.0s to account for CLI startup and IO overhead
    assert rust_time < max(shapely_time * 10.0, 1.0), \
        f"Rust performance regression: Rust {rust_time:.4f}s > max(Shapely {shapely_time:.4f}s * 10, 1.0s)"
