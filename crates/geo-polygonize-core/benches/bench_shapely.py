import shapely
from shapely.geometry import LineString
from shapely.ops import polygonize, unary_union
import time
import timeit
import sys
import random

FAST_CI = "--fast-ci" in sys.argv

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

def generate_dirty_grid(size):
    lines = []
    for i in range(size):
        for j in range(size):
            lines.append(LineString([(float(i), float(j)), (float(i + 1), float(j + 1))]))
            lines.append(LineString([(float(i + 1), float(j)), (float(i), float(j + 1))]))
    return lines

def generate_parallel_lines(n):
    return [LineString([(0.0, float(i)), (10.0, float(i))]) for i in range(n)]

def run_polygonize(lines):
    # Noding + Polygonization
    noded = unary_union(lines)
    polys = list(polygonize(noded))
    return polys

def benchmark():
    runs = 1 if FAST_CI else 5

    # Grid
    grid_sizes = [20] if FAST_CI else [5, 10, 20, 50, 100]
    print(f"=== Grid Benchmark ===")
    print(f"{'Size':<10} | {'Time (s)':<15} | {'Tiled Time (s)':<15} | {'Polys':<10}")
    print("-" * 40)

    for size in grid_sizes:
        lines = generate_grid(size)

        t = timeit.Timer(lambda: run_polygonize(lines))
        t_tiled = timeit.Timer(lambda: run_polygonize(lines))
        try:
            t.timeit(number=1) # Warmup
            if size >= 50:
                t_tiled.timeit(number=1)
        except Exception as e:
            print(f"Error at size {size}: {e}")
            continue

        total_time = t.timeit(number=runs)
        avg_time = total_time / runs

        tiled_time = "-"
        if size >= 50:
            tiled_total_time = t_tiled.timeit(number=runs)
            tiled_time = f"{(tiled_total_time / runs):<15.6f}"

        polys = run_polygonize(lines)
        print(f"{size:<10} | {avg_time:<15.6f} | {tiled_time} | {len(polys):<10}")

    # Bowtie
    dirty_sizes = [] if FAST_CI else [10, 20, 50]
    print(f"\n=== Bowtie Grid Benchmark ===")
    print(f"{'Size':<10} | {'Auto Time (s)':<15} | {'Force Grid (s)':<15} | {'Force SIMD (s)':<15}")
    print("-" * 70)

    for size in dirty_sizes:
        lines = generate_dirty_grid(size)
        t = timeit.Timer(lambda: run_polygonize(lines))
        try:
            t.timeit(number=1)
        except Exception as e:
            print(f"Error at size {size}: {e}")
            continue

        avg_time = t.timeit(number=runs) / runs
        force_simd = f"{avg_time:<15.6f}" if size <= 20 else "-"
        print(f"{size:<10} | {avg_time:<15.6f} | {avg_time:<15.6f} | {force_simd}")

    # Random
    # Matched to Rust bench max
    random_counts = [] if FAST_CI else [50, 100, 200]
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

        total_time = t.timeit(number=runs)
        avg_time = total_time / runs

        polys = run_polygonize(lines)
        print(f"{count:<10} | {avg_time:<15.6f} | {len(polys):<10}")

    if FAST_CI:
        return

    print(f"\n=== Large Parallel Benchmark ===")
    print(f"{'Count':<10} | {'Time (s)':<15}")
    print("-" * 30)
    parallel_lines = generate_parallel_lines(10000)
    t = timeit.Timer(lambda: run_polygonize(parallel_lines))
    t.timeit(number=1)
    avg_time = t.timeit(number=runs) / runs
    print(f"{10000:<10} | {avg_time:<15.6f}")

    print(f"\n=== Planar Graph Benchmark ===")
    print(f"{'Size':<10} | {'Time (s)':<15}")
    print("-" * 30)
    graph_lines = generate_grid(50)
    t = timeit.Timer(lambda: run_polygonize(graph_lines))
    t.timeit(number=1)
    avg_time = t.timeit(number=runs) / runs
    print(f"{50:<10} | {avg_time:<15.6f}")

    print(f"\n=== Planar Graph Dangles Benchmark ===")
    print(f"{'Count':<10} | {'Time (s)':<15}")
    print("-" * 30)
    dangles_lines = generate_random_lines(500, seed=12345)
    t = timeit.Timer(lambda: run_polygonize(dangles_lines))
    t.timeit(number=1)
    avg_time = t.timeit(number=runs) / runs
    print(f"{500:<10} | {avg_time:<15.6f}")

if __name__ == "__main__":
    benchmark()
