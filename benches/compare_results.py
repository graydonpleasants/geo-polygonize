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

def generate_table(category, display_name, col1_name, rust_results, python_results):
    lines = []
    lines.append(f"### {display_name}")
    lines.append("")
    lines.append(f"| {col1_name} | Rust Time (s) | Python Time (s) | Speedup (Py/Rs) |")
    lines.append(f"|---|---|---|---|")

    all_keys = set(rust_results.keys()) | set(python_results.keys())
    keys_in_cat = sorted([k for k in all_keys if k[0] == category], key=lambda x: x[1])

    for k in keys_in_cat:
        size = k[1]
        r_time = rust_results.get(k, None)
        p_time = python_results.get(k, None)

        r_str = f"{r_time:.6f}" if r_time is not None else "-"
        p_str = f"{p_time:.6f}" if p_time is not None else "-"

        if r_time and p_time:
            ratio = p_time / r_time
            ratio_str = f"{ratio:.2f}x"
        else:
            ratio_str = "-"

        lines.append(f"| {size} | {r_str} | {p_str} | {ratio_str} |")

    return lines

def update_markdown(filename, rust_results, python_results):
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
            table_lines = generate_table("grid", "Grid Topology (Intersecting Lines)", "Input Size (NxN)", rust_results, python_results)
            for l in table_lines:
                new_lines.append(l + "\n")

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
            continue

        # Detect Random Table
        if "### Random Lines" in line:
            table_lines = generate_table("random", "Random Lines", "Count", rust_results, python_results)
            for l in table_lines:
                new_lines.append(l + "\n")

            i += 1
            while i < len(lines) and lines[i].strip() == "":
                i += 1
            if i < len(lines) and "|" in lines[i]:
                 i += 1
            if i < len(lines) and "|---" in lines[i]:
                 i += 1
            while i < len(lines) and "|" in lines[i]:
                i += 1
            continue

        new_lines.append(line)
        i += 1

    with open(filename, 'w') as f:
        f.writelines(new_lines)

def print_original_summary(rust_results, python_results):
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
            else:
                ratio_str = "-"

            print(f"| {size} | {r_str} | {p_str} | {ratio_str} |")
        print("")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--update", action="store_true", help="Update BENCHMARKS.md")
    args = parser.parse_args()

    rust_results = parse_rust_output("rust_bench_output.txt")
    python_results = parse_python_output("python_bench_output.txt")

    if args.update:
        print("Updating BENCHMARKS.md...")
        update_markdown("BENCHMARKS.md", rust_results, python_results)
    else:
        print_original_summary(rust_results, python_results)

if __name__ == "__main__":
    main()
