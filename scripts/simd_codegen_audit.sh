#!/bin/bash
# Emit release assembly for the point-in-ring kernel and summarize the vector
# width each build actually executes. See "Code-generation audit" in
# docs/SIMD_OPTIMIZATION.md for how to read the output.
#
# Usage: scripts/simd_codegen_audit.sh [OUT_DIR]
#
# Requires the x86_64-unknown-linux-gnu target (`rustup target add ...`); no
# linker is needed because only assembly is emitted.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/target/simd-codegen-audit}"
mkdir -p "$OUT"

emit() {
    local name="$1" target="$2" flags="$3"
    local target_dir="$OUT/target-$name"
    local target_args=()
    local deps="$target_dir/release/deps"
    if [ -n "$target" ]; then
        target_args=(--target "$target")
        deps="$target_dir/$target/release/deps"
    fi
    rm -f "$deps"/geo_polygonize_core-*.s
    CARGO_TARGET_DIR="$target_dir" RUSTFLAGS="$flags" cargo rustc -q \
        --manifest-path "$ROOT/Cargo.toml" -p geo-polygonize-core --lib --release \
        --no-default-features ${target_args[@]+"${target_args[@]}"} \
        -- --emit asm -C codegen-units=1
    cp "$deps"/geo_polygonize_core-*.s "$OUT/$name.s"
}

emit x86_64-generic x86_64-unknown-linux-gnu ""
emit x86_64-v3 x86_64-unknown-linux-gnu "-C target-cpu=x86-64-v3"
if [ "$(uname -m)" = "arm64" ] || [ "$(uname -m)" = "aarch64" ]; then
    emit aarch64-host "" ""
fi

python3 - "$OUT" <<'EOF'
import pathlib, re, sys

out = pathlib.Path(sys.argv[1])
LOCAL = re.compile(r"^(\.L|L|l_|ltmp)")
# Functions that hold the kernel body in some build: the multiversion clones,
# or SimdRing::contains when static dispatch inlines the kernel.
PATTERN = re.compile(r"contains_simd_\w+_version|SimdRing8contains")


def functions(path):
    name, body = None, []
    for line in path.read_text().splitlines():
        label = re.match(r"^([^\s:]+):\s*$", line)
        if label and not LOCAL.match(label.group(1)):
            if name:
                yield name, body
            name, body = label.group(1), []
        elif name and line.startswith("\t") and not line.strip().startswith("."):
            body.append(line)
    if name:
        yield name, body


def short(symbol):
    m = re.search(r"(contains_simd_\w+?_version|SimdRing8contains)", symbol)
    return m.group(1).replace("SimdRing8contains", "SimdRing::contains")


METRICS = (
    r"%ymm",
    r"%xmm",
    r"\.2d\s",
    r"(v?divpd|fdiv\.2d)",
    r"\((%rsp|%rbp)\)|\[sp",
)


def count(body, pattern):
    return sum(1 for line in body if re.search(pattern, line))


print(f"{'build':<16} {'function':<58} {'insns':>5} {'ymm':>4} {'xmm':>4} "
      f"{'.2d':>4} {'vdiv':>4} {'stack':>5}")
for asm in sorted(out.glob("*.s")):
    for symbol, body in functions(asm):
        if not PATTERN.search(symbol):
            continue
        cols = [len(body)] + [count(body, p) for p in METRICS]
        print(f"{asm.stem:<16} {short(symbol):<58} " + " ".join(
            f"{c:>{w}}" for c, w in zip(cols, (5, 4, 4, 4, 4, 5))))
EOF
