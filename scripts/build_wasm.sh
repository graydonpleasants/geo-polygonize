#!/bin/bash
set -e

# Configuration
WASM_BINDGEN_VERSION="0.2.106"
TARGET="wasm32-unknown-unknown"

# Ensure target is installed
echo "Checking for $TARGET..."
rustup target add $TARGET

# Install wasm-bindgen-cli if needed
if ! command -v wasm-bindgen &> /dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
    echo "Installing wasm-bindgen-cli $WASM_BINDGEN_VERSION..."
    cargo install wasm-bindgen-cli --version $WASM_BINDGEN_VERSION
fi

# Install binaryen (wasm-opt) if needed
if ! command -v wasm-opt &> /dev/null; then
    echo "wasm-opt not found. Attempting to install via npm..."
    npm install -g wasm-opt
    if ! command -v wasm-opt &> /dev/null; then
        echo "Warning: wasm-opt could not be installed. Build will proceed without optimization."
    else
        echo "Successfully installed wasm-opt: $(wasm-opt --version)"
    fi
else
    echo "Found wasm-opt: $(wasm-opt --version)"
fi

build_variant() {
    local VARIANT=$1
    local OUT_DIR="pkg-$VARIANT"
    local FLAGS=$2

    echo "Building $VARIANT version..."

    # 1. Cargo Build
    # We explicitly specify the wasm crate
    RUSTFLAGS="$FLAGS" cargo build -p geo-polygonize-wasm --target $TARGET --release --features console_error_panic_hook --lib

    # 2. Wasm Bindgen
    echo "Running wasm-bindgen for $VARIANT..."
    # With the new crate name, the output is geo_polygonize_wasm.wasm
    CRATE_NAME="geo_polygonize_wasm"
    WASM_PATH="target/$TARGET/release/$CRATE_NAME.wasm"

    if [ ! -f "$WASM_PATH" ]; then
        echo "Error: $WASM_PATH not found!"
        exit 1
    fi

    rm -rf $OUT_DIR
    # We want the output JS/WASM file to still be named nicely for consumption,
    # or match what rollup expects.
    # The original script output name was "geo_polygonize".
    # pkg-wrapper/index.ts imports from pkg-scalar/geo_polygonize.js usually, or whatever wasm-bindgen outputs.
    # Let's check rollup config or pkg-wrapper import.
    # The previous script used --out-name "geo_polygonize". We should stick to that if possible
    # to avoid breaking downstream consumers.

    wasm-bindgen --target web --out-dir $OUT_DIR --out-name "geo_polygonize" "$WASM_PATH"

    # 3. Optimization
    if command -v wasm-opt &> /dev/null; then
        echo "Optimizing $VARIANT..."
        # The file name comes from --out-name "geo_polygonize" -> "geo_polygonize_bg.wasm"
        wasm-opt -O3 -o "$OUT_DIR/geo_polygonize_bg.wasm" "$OUT_DIR/geo_polygonize_bg.wasm"
    fi

    # Remove .gitignore if generated
    rm -f $OUT_DIR/.gitignore
}

# Build Scalar
build_variant "scalar" ""

# Build SIMD
build_variant "simd" "-C target-feature=+simd128"

# Build Threads
build_variant_threads() {
    local OUT_DIR="pkg-threads"
    # Enable atomics and shared memory
    local FLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals"

    echo "Building Threads version..."

    # Check for nightly
    if ! rustup toolchain list | grep -q "nightly"; then
        echo "Installing nightly toolchain..."
        rustup toolchain install nightly
    fi

    # Ensure rust-src component is installed (required for build-std)
    echo "Installing rust-src for nightly..."
    rustup component add rust-src --toolchain nightly

    # 1. Cargo Build with Nightly and build-std
    RUSTFLAGS="$FLAGS" cargo +nightly build -p geo-polygonize-wasm \
        --target $TARGET \
        --release \
        --features "console_error_panic_hook threads" \
        --lib \
        -Z build-std=std,panic_abort

    # 2. Wasm Bindgen
    echo "Running wasm-bindgen for threads..."
    CRATE_NAME="geo_polygonize_wasm"
    WASM_PATH="target/$TARGET/release/$CRATE_NAME.wasm"

    if [ ! -f "$WASM_PATH" ]; then
        echo "Error: $WASM_PATH not found!"
        exit 1
    fi

    rm -rf $OUT_DIR
    # Use --target web to ensure correct loading behavior for threads
    wasm-bindgen --target web --out-dir $OUT_DIR --out-name "geo_polygonize" "$WASM_PATH"

    # 3. Optimization
    if command -v wasm-opt &> /dev/null; then
        echo "Optimizing Threads..."
        # wasm-opt generally supports atomics if present in the binary
        wasm-opt -O3 -o "$OUT_DIR/geo_polygonize_bg.wasm" "$OUT_DIR/geo_polygonize_bg.wasm"
    fi

    rm -f $OUT_DIR/.gitignore
}

build_variant_threads

# Export the ts-rs bindings so that TS type-checks succeed when imported via pkg-wrapper
echo "Exporting our TS-RS bindings into wasm-bindgen definitions..."
export TS_RS_EXPORT_DIR="bindings"
cargo test -p geo-polygonize-core
mkdir -p pkg-wrapper/bindings
cp crates/geo-polygonize-core/bindings/* pkg-wrapper/bindings/

for DIR in pkg-scalar pkg-simd pkg-threads; do
  D_TS_FILE="${DIR}/geo_polygonize.d.ts"

  if [ -f "$D_TS_FILE" ]; then
    TEMP_FILE=$(mktemp)
    # 1. Write the imports
    echo "import { PolygonizerOptions } from '../pkg-wrapper/bindings/PolygonizerOptions';" > "$TEMP_FILE"

    # 2. Process the original file and replace `any` with `Partial<PolygonizerOptions>` for the options_val argument
    sed -e 's/options_val: any/options_val: Partial<PolygonizerOptions>/g' "$D_TS_FILE" >> "$TEMP_FILE"

    # 3. Replace original file
    mv "$TEMP_FILE" "$D_TS_FILE"
  fi
done

# Ensure wrapper exists
if [ ! -d "pkg-wrapper" ]; then
    echo "pkg-wrapper directory missing!"
    exit 1
fi

# Install npm deps
if [ ! -d "node_modules" ]; then
    npm install
fi

# Bundle with Rollup
echo "Running rollup..."
npx rollup -c

# Prepare distribution files
echo "Preparing dist..."
# Copy the WASM files to dist for external consumption (Slim build)
cp pkg-scalar/geo_polygonize_bg.wasm dist/geo_polygonize.wasm
cp pkg-simd/geo_polygonize_bg.wasm dist/geo_polygonize_simd.wasm

echo "Build complete! Artifacts are in dist/"
