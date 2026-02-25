import json
import subprocess
import tempfile
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

        # Run Rust process
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
    """Compares topological equivalence without relying on vertex order."""
    # Filter empty polygons if any (though shapely/rust shouldn't produce them usually)
    shapely_polys = [p for p in shapely_polys if not p.is_empty and p.area > 1e-9]
    rust_polys = [p for p in rust_polys if not p.is_empty and p.area > 1e-9]

    assert len(shapely_polys) == len(rust_polys), f"Count mismatch: Shapely {len(shapely_polys)} vs Rust {len(rust_polys)}"

    shapely_areas = sorted([p.area for p in shapely_polys])
    rust_areas = sorted([p.area for p in rust_polys])

    # Use a looser tolerance for areas because of different FP arithmetic
    np.testing.assert_allclose(
        rust_areas, shapely_areas,
        rtol=1e-6, atol=1e-6,
        err_msg="Multiset of polygon areas do not match."
    )

@pytest.mark.parametrize("generator", [
    generate_t_junction_soup,
    generate_collinear_overlaps,
    generate_grid,
    generate_self_intersecting_figure_8s,
])
def test_differential_parity(generator):
    lines = generator()
    shapely_result = run_shapely(lines)
    rust_result = run_rust_cli(lines)
    assert_parity(shapely_result, rust_result)
