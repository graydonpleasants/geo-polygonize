#!/bin/bash
set -e

export PATH="$HOME/.cargo/bin:$PATH"

# Configuration
WASM_BINDGEN_VERSION="0.2.114"
TARGET="wasm32-unknown-unknown"

# Ensure target is installed
echo "Checking for $TARGET..."
rustup target add $TARGET

# Install wasm-bindgen-cli if needed
if ! command -v wasm-bindgen &> /dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
    echo "Installing wasm-bindgen-cli $WASM_BINDGEN_VERSION..."
    if command -v cargo-binstall &> /dev/null; then
        cargo binstall -y wasm-bindgen-cli --version $WASM_BINDGEN_VERSION
    else
        cargo install wasm-bindgen-cli --version $WASM_BINDGEN_VERSION
    fi
fi

build_variant() {
    local VARIANT=$1
    local OUT_DIR="pkg-$VARIANT"
    local FLAGS=$2

    echo "Building $VARIANT version for docs site..."

    # 1. Cargo Build
    RUSTFLAGS="$FLAGS" cargo build -p geo-polygonize-wasm --target $TARGET --release --features console_error_panic_hook --lib

    # 2. Wasm Bindgen
    echo "Running wasm-bindgen for $VARIANT..."
    CRATE_NAME="geo_polygonize_wasm"
    WASM_PATH="target/$TARGET/release/$CRATE_NAME.wasm"

    if [ ! -f "$WASM_PATH" ]; then
        echo "Error: $WASM_PATH not found!"
        return 1
    fi

    rm -rf $OUT_DIR
    # Use full path to avoid PATH issues in CI environments
    ~/.cargo/bin/wasm-bindgen --target web --out-dir $OUT_DIR --out-name "geo_polygonize" "$WASM_PATH"

    # Remove .gitignore if generated
    rm -f $OUT_DIR/.gitignore
}

# Build Scalar
build_variant "scalar" ""

# Build SIMD
if [ "$SKIP_SIMD" = "1" ]; then
    echo "Skipping SIMD build as requested. Copying scalar to simd to satisfy dependencies..."
    cp -r pkg-scalar pkg-simd
else
    build_variant "simd" "-C target-feature=+simd128"
fi

# Export the ts-rs bindings so that TS type-checks succeed when imported via pkg-wrapper
echo "Exporting our TS-RS bindings into wasm-bindgen definitions..."
export TS_RS_EXPORT_DIR="crates/geo-polygonize-core/bindings"
cargo run -p geo-polygonize-core --bin export_bindings --release
mkdir -p pkg-wrapper/bindings
cp crates/geo-polygonize-core/bindings/* pkg-wrapper/bindings/

for DIR in pkg-scalar pkg-simd; do
  D_TS_FILE="${DIR}/geo_polygonize.d.ts"

  if [ -f "$D_TS_FILE" ]; then
    TEMP_FILE=$(mktemp)
    echo "import { PolygonizerOptions } from '../pkg-wrapper/bindings/PolygonizerOptions';" > "$TEMP_FILE"
    sed -e 's/options_val: any/options_val: Partial<PolygonizerOptions>/g' "$D_TS_FILE" >> "$TEMP_FILE"
    mv "$TEMP_FILE" "$D_TS_FILE"
  fi
done

# We mock threads folder to avoid rollup failure when doing slim build
mkdir -p pkg-threads
echo "export const threads = 'mocked for site build';" > pkg-threads/index.js
echo "export const threads = 'mocked for site build';" > pkg-threads/index.d.ts
echo "export const polygonizeWithOptions = () => {};" >> pkg-threads/index.js
echo "export const initThreadPool = () => {};" >> pkg-threads/index.js

# Install npm deps
if [ ! -d "node_modules" ]; then
    npm install
fi

# Bundle with Rollup
echo "Running rollup..."
npx rollup -c

# Prepare distribution files
echo "Preparing dist..."
cp pkg-scalar/geo_polygonize_bg.wasm dist/geo_polygonize.wasm
cp pkg-simd/geo_polygonize_bg.wasm dist/geo_polygonize_simd.wasm

echo "Site Wasm build complete! Artifacts are in dist/"
