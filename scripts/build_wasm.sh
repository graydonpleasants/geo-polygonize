#!/bin/bash
set -e

# Build the scalar version (no SIMD)
echo "Building scalar version..."
wasm-pack build --target web --out-dir pkg-scalar --release --features console_error_panic_hook
# Remove the .gitignore file so it doesn't mess with npm publishing if we were publishing the pkg dir directly (we aren't, but good practice)
rm -f pkg-scalar/.gitignore

# Build the SIMD version
echo "Building SIMD version..."
# We need to set RUSTFLAGS to enable simd128
export RUSTFLAGS="-C target-feature=+simd128"
wasm-pack build --target web --out-dir pkg-simd --release --features console_error_panic_hook
rm -f pkg-simd/.gitignore

# Ensure wrapper exists (it is part of the repo now, but let's be safe)
if [ ! -d "pkg-wrapper" ]; then
    echo "pkg-wrapper directory missing!"
    exit 1
fi

# Install dependencies if node_modules doesn't exist
if [ ! -d "node_modules" ]; then
    npm install
fi

# Run rollup
echo "Running rollup..."
npx rollup -c

# Copy the WASM file to dist for external consumption
# We copy the scalar version as the default 'geo_polygonize.wasm'
cp pkg-scalar/geo_polygonize_bg.wasm dist/geo_polygonize.wasm

echo "Build complete! Artifacts are in dist/"
