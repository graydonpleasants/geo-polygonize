#!/usr/bin/env python3
"""Check the supported non-hidden crate-root export allowlist."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


PUB_USE = re.compile(r"(?ms)^pub use ([A-Za-z_][A-Za-z0-9_]*)::(\{.*?\}|[A-Za-z_][A-Za-z0-9_]*);")


def expected_entries(path: Path) -> set[str]:
    return {
        line.strip()
        for line in path.read_text().splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    }


def actual_entries(path: Path) -> set[str]:
    text = path.read_text()
    entries: set[str] = set()
    for match in PUB_USE.finditer(text):
        preceding = text[: match.start()].rstrip().splitlines()
        if preceding and preceding[-1].strip() == "#[doc(hidden)]":
            continue
        module, names = match.groups()
        if names.startswith("{"):
            names = names[1:-1]
            for name in names.split(","):
                item = name.strip().split(" as ", 1)[0].strip()
                if item:
                    entries.add(f"{module}::{item}")
        else:
            entries.add(f"{module}::{names}")
    return entries


def check(root: Path) -> tuple[set[str], set[str]]:
    expected = expected_entries(root / "release/stable-api-v1.txt")
    actual = actual_entries(root / "crates/geo-polygonize-core/src/lib.rs")
    return expected - actual, actual - expected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    missing, unexpected = check(args.root)
    if missing or unexpected:
        if missing:
            print("stable API exports missing from lib.rs:", file=sys.stderr)
            print("\n".join(f"  {entry}" for entry in sorted(missing)), file=sys.stderr)
        if unexpected:
            print("non-hidden lib.rs exports missing from stable-api-v1.txt:", file=sys.stderr)
            print("\n".join(f"  {entry}" for entry in sorted(unexpected)), file=sys.stderr)
        return 1
    print("stable API allowlist: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
