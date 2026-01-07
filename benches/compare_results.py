import re
import sys

def parse_rust_output(filename):
    # polygonize/grid/5       time:   [993.44 µs 1.0099 ms 1.0274 ms]
    results = {}
    with open(filename, 'r') as f:
        content = f.read()

    # Regex for "polygonize/grid/5 ... time: [low mid high] unit"
    # Note: Criterion output format can vary slightly but usually:
    # polygonize/grid/5       time:   [980.00 µs 1.0000 ms 1.0200 ms]

    # Simple regex to capture name and middle time estimate
    # We need to handle units: µs, ms, s

    # Matches: polygonize/grid/5   time:   [... val unit ...]
    pattern = re.compile(r'polygonize/([^/]+)/(\d+)\s+time:\s+\[[^\]]*\s([\d\.]+)\s([µms]+)\]')

    for match in pattern.finditer(content):
        cat = match.group(1) # grid or random
        size = int(match.group(2))
        val = float(match.group(3))
        unit = match.group(4)

        # Normalize to seconds
        if unit == 'µs':
            seconds = val / 1_000_000
        elif unit == 'ms':
            seconds = val / 1_000
        elif unit == 's':
            seconds = val
        else:
            seconds = val # default?

        results[(cat, size)] = seconds

    return results

def parse_python_output(filename):
    # Size       | Time (s)        | Polys
    # 5          | 0.001116        | 25
    results = {}
    current_cat = None

    with open(filename, 'r') as f:
        for line in f:
            line = line.strip()
            if "=== Grid Benchmark ===" in line:
                current_cat = "grid"
                continue
            if "=== Random Benchmark ===" in line:
                current_cat = "random"
                continue
            if line.startswith("Size") or line.startswith("Count") or line.startswith("-"):
                continue

            parts = [p.strip() for p in line.split('|')]
            if len(parts) >= 2:
                try:
                    size = int(parts[0])
                    time_s = float(parts[1])
                    if current_cat:
                        results[(current_cat, size)] = time_s
                except ValueError:
                    pass
    return results

def main():
    rust_results = parse_rust_output("rust_bench_output.txt")
    python_results = parse_python_output("python_bench_output.txt")

    all_keys = sorted(set(rust_results.keys()) | set(python_results.keys()))

    # Group by category
    categories = sorted(list(set(k[0] for k in all_keys)))

    print("# Benchmark Comparison (Rust vs Python/Shapely)")
    print("")

    for cat in categories:
        print(f"## Category: {cat}")
        print(f"| Input Size | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |")
        print(f"|---|---|---|---|")

        keys_in_cat = sorted([k for k in all_keys if k[0] == cat], key=lambda x: x[1])

        for k in keys_in_cat:
            size = k[1]
            r_time = rust_results.get(k, None)
            p_time = python_results.get(k, None)

            r_str = f"{r_time:.6f}" if r_time is not None else "-"
            p_str = f"{p_time:.6f}" if p_time is not None else "-"

            if r_time and p_time:
                ratio = p_time / r_time
                ratio_str = f"{ratio:.2f}x"
                # If Rust is faster, ratio > 1. If Python is faster, ratio < 1.
                # Usually we want "Speedup of Rust relative to Python" = P / R.
                # If P=2s, R=1s, Rust is 2x faster.
            else:
                ratio_str = "-"

            print(f"| {size} | {r_str} | {p_str} | {ratio_str} |")
        print("")

if __name__ == "__main__":
    main()
