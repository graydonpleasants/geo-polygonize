import shapely
from shapely.geometry import LineString
from shapely.ops import polygonize, unary_union
import time
import timeit
import sys
import random

def generate_grid(n):
    lines = []
    for i in range(n + 1):
        # Horizontal
        lines.append(LineString([(0.0, float(i)), (float(n), float(i))]))
        # Vertical
        lines.append(LineString([(float(i), 0.0), (float(i), float(n))]))
    return lines

def generate_random_lines(n, seed=42):
    random.seed(seed)
    lines = []
    for _ in range(n):
        x1 = random.uniform(0.0, 100.0)
        y1 = random.uniform(0.0, 100.0)
        x2 = random.uniform(0.0, 100.0)
        y2 = random.uniform(0.0, 100.0)
        lines.append(LineString([(x1, y1), (x2, y2)]))
    return lines

def run_polygonize(lines):
    # Noding + Polygonization
    noded = unary_union(lines)
    polys = list(polygonize(noded))
    return polys

def benchmark():
    # Grid
    grid_sizes = [5, 10, 20, 50, 100]
    print(f"=== Grid Benchmark ===")
    print(f"{'Size':<10} | {'Time (s)':<15} | {'Polys':<10}")
    print("-" * 40)

    for size in grid_sizes:
        lines = generate_grid(size)

        t = timeit.Timer(lambda: run_polygonize(lines))
        try:
            t.timeit(number=1) # Warmup
        except Exception as e:
            print(f"Error at size {size}: {e}")
            continue

        loops = 10
        total_time = t.timeit(number=loops)
        avg_time = total_time / loops

        polys = run_polygonize(lines)
        print(f"{size:<10} | {avg_time:<15.6f} | {len(polys):<10}")

    # Random
    # Matched to Rust bench max
    random_counts = [50, 100, 200]
    print(f"\n=== Random Benchmark ===")
    print(f"{'Count':<10} | {'Time (s)':<15} | {'Polys':<10}")
    print("-" * 40)

    for count in random_counts:
        lines = generate_random_lines(count)

        t = timeit.Timer(lambda: run_polygonize(lines))
        try:
            t.timeit(number=1) # Warmup
        except Exception as e:
            print(f"Error at size {count}: {e}")
            continue

        loops = 10
        total_time = t.timeit(number=loops)
        avg_time = total_time / loops

        polys = run_polygonize(lines)
        print(f"{count:<10} | {avg_time:<15.6f} | {len(polys):<10}")

if __name__ == "__main__":
    benchmark()
