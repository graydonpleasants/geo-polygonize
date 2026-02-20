
import timeit
import numpy as np
from shapely.geometry import LineString

def create_circle_original(x, y, r, points=100):
    angles = np.linspace(0, 2*np.pi, points)
    coords = []
    for a in angles:
        coords.append((x + r * np.cos(a), y + r * np.sin(a)))
    return LineString(coords)

def create_circle_optimized(x, y, r, points=100):
    angles = np.linspace(0, 2*np.pi, points)
    # Vectorized calculation
    xs = x + r * np.cos(angles)
    ys = y + r * np.sin(angles)
    coords = np.column_stack((xs, ys))
    return LineString(coords)

def benchmark():
    points = 10000
    loops = 100

    t_orig = timeit.timeit(lambda: create_circle_original(0, 0, 10, points), number=loops)
    t_opt = timeit.timeit(lambda: create_circle_optimized(0, 0, 10, points), number=loops)

    print(f"Original: {t_orig:.4f} s")
    print(f"Optimized: {t_opt:.4f} s")
    print(f"Speedup: {t_orig / t_opt:.2f}x")

if __name__ == "__main__":
    benchmark()
