import re
import sys
import argparse
import os

def parse_rust_output(filename):
    results = {}
    if not os.path.exists(filename):
        print(f"Warning: {filename} not found.")
        return results

    with open(filename, 'r') as f:
        content = f.read()

    # Matches: polygonize/grid/5   time:   [... val unit ...]
    pattern = re.compile(r'polygonize/([^/]+)/(\d+)\s+time:\s+\[[^\]]*\s([\d\.]+)\s([µms]+)\]')

    for match in pattern.finditer(content):
        cat = match.group(1)
        size = int(match.group(2))
        val = float(match.group(3))
        unit = match.group(4)

        if unit == 'µs':
            seconds = val / 1_000_000
        elif unit == 'ms':
            seconds = val / 1_000
        elif unit == 's':
            seconds = val
        else:
            seconds = val

        results[(cat, size)] = seconds

    return results

def parse_python_output(filename):
    results = {}
    current_cat = None
    if not os.path.exists(filename):
        print(f"Warning: {filename} not found.")
        return results

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

def parse_wasm_output(filename):
    results = {}
    if not os.path.exists(filename):
        print(f"Warning: {filename} not found.")
        return results

    with open(filename, 'r') as f:
        current_cat = None
        for line in f:
            line = line.strip()
            if "=== Grid Benchmark ===" in line:
                current_cat = "grid"
                continue
            if "=== Bowtie Grid Benchmark ===" in line:
                current_cat = "bowtie_grid_auto"
                continue
            if "=== Random Benchmark ===" in line:
                current_cat = "random"
                continue
            if "=== Large Parallel Benchmark ===" in line:
                current_cat = "large_parallel_10k"
                continue
            if "=== Planar Graph Benchmark ===" in line:
                current_cat = "planar_graph"
                continue
            if "=== Planar Graph Dangles Benchmark ===" in line:
                current_cat = "planar_graph_dangles"
                continue

            if line.startswith("|") and not line.startswith("|---") and not "Polygonize" in line and not "Bowtie" in line and not "Robust" in line and not "Get Edge Rings" in line:
                parts = [p.strip() for p in line.split('|') if p.strip()]
                if len(parts) >= 2:
                    try:
                        size_str = parts[0]
                        if "x" in size_str:
                            size = int(size_str.split('x')[0])
                        else:
                            size = int(size_str)

                        poly_ms = float(parts[1])
                        # The JS benchmark outputs ms. Convert to seconds.
                        if current_cat == "bowtie_grid_auto":
                            results[("bowtie_grid_auto", size)] = float(parts[1]) / 1000.0
                            results[("bowtie_grid_force_grid", size)] = float(parts[2]) / 1000.0
                            if parts[3] != '-':
                                results[("bowtie_grid_force_simd", size)] = float(parts[3]) / 1000.0
                        elif current_cat == "grid":
                            results[("grid", size)] = float(parts[1]) / 1000.0
                            if parts[2] != '-':
                                results[("grid_tiled", size)] = float(parts[2]) / 1000.0
                        else:
                            if current_cat:
                                results[(current_cat, size)] = poly_ms / 1000.0
                    except ValueError:
                        pass
    return results

def skip_table_lines(lines, i):
    """Skips the existing table in the markdown lines starting from index i."""
    i += 1
    # Skip blank lines
    while i < len(lines) and lines[i].strip() == "":
        i += 1
    # Skip header
    if i < len(lines) and "|" in lines[i]:
        i += 1
    # Skip separator
    if i < len(lines) and "|---" in lines[i]:
        i += 1
    # Skip rows
    while i < len(lines) and "|" in lines[i]:
        i += 1
    return i

def generate_table(category, display_name, col1_name, rust_results, python_results, wasm_results):
    lines = []
    lines.append(f"### {display_name}")
    lines.append("")
    lines.append(f"| {col1_name} | Rust Time (s) | Python Time (s) | Wasm Time (s) | Speedup (Py/Rs) | Speedup (Py/Wasm) | Speedup (Wasm/Rs) |")
    lines.append(f"|---|---|---|---|---|---|---|")

    all_keys = set(rust_results.keys()) | set(python_results.keys()) | set(wasm_results.keys())
    keys_in_cat = sorted([k for k in all_keys if k[0] == category], key=lambda x: x[1])

    for k in keys_in_cat:
        size = k[1]
        r_time = rust_results.get(k, None)
        p_time = python_results.get(k, None)
        w_time = wasm_results.get(k, None)

        r_str = f"{r_time:.6f}" if r_time is not None else "-"
        p_str = f"{p_time:.6f}" if p_time is not None else "-"
        w_str = f"{w_time:.6f}" if w_time is not None else "-"

        if r_time and p_time:
            ratio_py_rs = f"{(p_time / r_time):.2f}x"
        else:
            ratio_py_rs = "-"

        if w_time and p_time:
            ratio_py_w = f"{(p_time / w_time):.2f}x"
        else:
            ratio_py_w = "-"

        if r_time and w_time:
            ratio_w_rs = f"{(w_time / r_time):.2f}x"
        else:
            ratio_w_rs = "-"

        lines.append(f"| {size} | {r_str} | {p_str} | {w_str} | {ratio_py_rs} | {ratio_py_w} | {ratio_w_rs} |")

    return lines

def update_markdown(filename, rust_results, python_results, wasm_results):
    if not os.path.exists(filename):
        print(f"Error: {filename} not found.")
        return

    with open(filename, 'r') as f:
        lines = f.readlines()

    new_lines = []
    i = 0
    while i < len(lines):
        line = lines[i]

        # Detect Grid Table
        if "### Grid Topology" in line:
            table_lines = generate_table("grid", "Grid Topology (Intersecting Lines)", "Input Size (NxN)", rust_results, python_results, wasm_results)
            for l in table_lines:
                new_lines.append(l + "\n")

            i = skip_table_lines(lines, i)
            continue

        # Detect Random Table
        if "### Random Lines" in line:
            table_lines = generate_table("random", "Random Lines", "Count", rust_results, python_results, wasm_results)
            for l in table_lines:
                new_lines.append(l + "\n")

            i = skip_table_lines(lines, i)
            continue

        new_lines.append(line)
        i += 1

    with open(filename, 'w') as f:
        f.writelines(new_lines)

def print_original_summary(rust_results, python_results, wasm_results):
    all_keys = sorted(set(rust_results.keys()) | set(python_results.keys()) | set(wasm_results.keys()))

    # Group by category
    categories = sorted(list(set(k[0] for k in all_keys)))

    print("# Benchmark Comparison (Rust vs Python/Shapely vs Wasm)")
    print("")

    for cat in categories:
        print(f"## Category: {cat}")
        print(f"| Input Size | Rust Time (s) | Python Time (s) | Wasm Time (s) | Speedup (Py/Rs) |")
        print(f"|---|---|---|---|---|")

        keys_in_cat = sorted([k for k in all_keys if k[0] == cat], key=lambda x: x[1])

        for k in keys_in_cat:
            size = k[1]
            r_time = rust_results.get(k, None)
            p_time = python_results.get(k, None)
            w_time = wasm_results.get(k, None)

            r_str = f"{r_time:.6f}" if r_time is not None else "-"
            p_str = f"{p_time:.6f}" if p_time is not None else "-"
            w_str = f"{w_time:.6f}" if w_time is not None else "-"

            if r_time and p_time:
                ratio = p_time / r_time
                ratio_str = f"{ratio:.2f}x"
            else:
                ratio_str = "-"

            print(f"| {size} | {r_str} | {p_str} | {w_str} | {ratio_str} |")
        print("")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--update", action="store_true", help="Update BENCHMARKS.md")
    args = parser.parse_args()

    rust_results = parse_rust_output("rust_bench_output.txt")
    python_results = parse_python_output("python_bench_output.txt")
    wasm_results = parse_wasm_output("wasm_bench_output.txt")

    if args.update:
        print("Updating BENCHMARKS.md...")
        update_markdown("BENCHMARKS.md", rust_results, python_results, wasm_results)
    else:
        print_original_summary(rust_results, python_results, wasm_results)

if __name__ == "__main__":
    main()
