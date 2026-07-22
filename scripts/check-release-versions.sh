#!/usr/bin/env bash
set -euo pipefail

package_version() {
  awk '/^\[package\]$/ { in_package = 1; next } in_package && /^version = / { gsub(/"/, "", $3); print $3; exit }' "$1"
}

expected="$(package_version crates/geo-polygonize-core/Cargo.toml)"
for manifest in crates/geo-polygonize-{arrow,flatgeobuf,python,wasm}/Cargo.toml; do
  actual="$(package_version "$manifest")"
  [ "$actual" = "$expected" ] || {
    echo "$manifest has version $actual; expected $expected" >&2
    exit 1
  }
done

package_version_json="$(sed -nE 's/^[[:space:]]*"version":[[:space:]]*"([^"]+)".*/\1/p' package.json | head -n1)"
pyproject_version="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/p' pyproject.toml | head -n1)"
for actual in "$package_version_json" "$pyproject_version"; do
  [ "$actual" = "$expected" ] || {
    echo "release version $actual; expected $expected" >&2
    exit 1
  }
done
