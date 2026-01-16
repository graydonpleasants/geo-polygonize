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

# Install binaryen (wasm-opt) if needed - assuming it's not present or managing manual install is hard in this env,
# we'll try to use the system one or skip if not found, but since we are "deconstructing", we should try to ensure it exists.
# For this environment, we'll check if it's available. If not, we might assume it's pre-installed or we skip optimization
# if we can't easily fetch it (since we can't use apt-get/brew).
# However, `cargo install wasm-opt` allows installing a wrapper.
if ! command -v wasm-opt &> /dev/null; then
    echo "wasm-opt not found. Attempting to install via cargo..."
    # There isn't a direct official cargo crate for wasm-opt binary, usually provided by system or npm.
    # We will skip explicit installation logic for wasm-opt here to avoid breaking the build environment
    # if it's complex, but we'll try to use it if present.
    echo "Warning: wasm-opt not found. Build will proceed without optimization."
else
    echo "Found wasm-opt: $(wasm-opt --version)"
fi

build_variant() {
    local VARIANT=$1
    local OUT_DIR="pkg-$VARIANT"
    local FLAGS=$2

    echo "Building $VARIANT version..."

    # 1. Cargo Build
    # We use --release and the specified flags
    # Note: We need to handle the fact that cargo build outputs to target/wasm32-unknown-unknown/release
    # and filenames are based on crate name.

    # Clean previous build artifacts for this target to ensure flags apply?
    # Cargo handles rebuilds, but changing RUSTFLAGS might require a clean or careful handling.
    # To be safe, we touch the source or rely on cargo detecting flag changes.

    RUSTFLAGS="$FLAGS" cargo build --target $TARGET --release --features console_error_panic_hook --lib

    # 2. Wasm Bindgen
    echo "Running wasm-bindgen for $VARIANT..."
    # The output filename is usually geo_polygonize.wasm based on Cargo.toml name
    CRATE_NAME="geo_polygonize"
    WASM_PATH="target/$TARGET/release/$CRATE_NAME.wasm"

    if [ ! -f "$WASM_PATH" ]; then
        echo "Error: $WASM_PATH not found!"
        exit 1
    fi

    rm -rf $OUT_DIR
    wasm-bindgen --target web --out-dir $OUT_DIR --out-name $CRATE_NAME "$WASM_PATH"

    # 3. Optimization
    if command -v wasm-opt &> /dev/null; then
        echo "Optimizing $VARIANT..."
        wasm-opt -O3 -o "$OUT_DIR/${CRATE_NAME}_bg.wasm" "$OUT_DIR/${CRATE_NAME}_bg.wasm"
    fi

    # Remove .gitignore if generated
    rm -f $OUT_DIR/.gitignore
}

# Build Scalar
build_variant "scalar" ""

# Build SIMD
build_variant "simd" "-C target-feature=+simd128"

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
# We rename them to be explicit as per the "Wasm is not an implementation detail" advice
cp pkg-scalar/geo_polygonize_bg.wasm dist/geo_polygonize.wasm
cp pkg-simd/geo_polygonize_bg.wasm dist/geo_polygonize_simd.wasm

echo "Build complete! Artifacts are in dist/"
