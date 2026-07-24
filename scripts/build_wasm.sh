#!/bin/bash
set -e

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

WASM_BINDGEN_VERSION="0.2.114"
TARGET="wasm32-unknown-unknown"
SITE_BUILD="${SITE_BUILD:-${BUILD_WASM_SITE:-0}}"
VARIANT=""
NO_BUNDLE=0
BUNDLE_ONLY=0

while [ "$#" -gt 0 ]; do
    case "$1" in
        --variant)
            VARIANT="$2"
            shift 2
            ;;
        --no-bundle)
            NO_BUNDLE=1
            shift
            ;;
        --bundle-only)
            BUNDLE_ONLY=1
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            exit 1
            ;;
    esac
done

setup_wasm_tools() {
    echo "Checking for $TARGET..."
    rustup target add $TARGET

    if ! command -v wasm-bindgen &> /dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
        echo "Installing wasm-bindgen-cli $WASM_BINDGEN_VERSION..."
        if command -v cargo-binstall &> /dev/null; then
            cargo binstall -y wasm-bindgen-cli --version $WASM_BINDGEN_VERSION
        else
            cargo install wasm-bindgen-cli --version $WASM_BINDGEN_VERSION
        fi
    fi
    if ! command -v wasm-bindgen &> /dev/null; then
        cargo install --force wasm-bindgen-cli --version $WASM_BINDGEN_VERSION
    fi
    WASM_BINDGEN_BIN="$(command -v wasm-bindgen)"

    if [ "$SITE_BUILD" != "1" ] && ! command -v wasm-opt &> /dev/null; then
        echo "wasm-opt not found. Attempting to install via npm..."
        npm install -g --allow-scripts=wasm-opt wasm-opt
        if ! command -v wasm-opt &> /dev/null; then
            echo "Warning: wasm-opt could not be installed. Build will proceed without optimization."
        else
            echo "Successfully installed wasm-opt: $(wasm-opt --version)"
        fi
    elif [ "$SITE_BUILD" != "1" ]; then
        echo "Found wasm-opt: $(wasm-opt --version)"
    fi
}

build_variant() {
    local variant=$1
    local out_dir="pkg-$variant"
    local flags=$2

    echo "Building $variant version..."
    RUSTFLAGS="$flags" cargo build --locked -p geo-polygonize-wasm --target $TARGET --release --features console_error_panic_hook --lib

    echo "Running wasm-bindgen for $variant..."
    local wasm_path="target/$TARGET/release/geo_polygonize_wasm.wasm"
    if [ ! -f "$wasm_path" ]; then
        echo "Error: $wasm_path not found!"
        exit 1
    fi

    rm -rf "$out_dir"
    "$WASM_BINDGEN_BIN" --target web --out-dir "$out_dir" --out-name "geo_polygonize" "$wasm_path"

    if [ "$SITE_BUILD" != "1" ] && command -v wasm-opt &> /dev/null; then
        echo "Optimizing $variant..."
        wasm-opt -O3 -o "$out_dir/geo_polygonize_bg.wasm" "$out_dir/geo_polygonize_bg.wasm"
    fi

    rm -f "$out_dir/.gitignore"
}

patch_threads_worker() {
    echo "Patching wasm-bindgen-rayon workerHelpers.js..."
    for worker_helpers in $(find pkg-threads/snippets -name "workerHelpers.js" 2>/dev/null); do
        temp_file=$(mktemp)
        sed -e "s/new URL('.\/workerHelpers.js', import.meta.url)/import.meta.resolve('.\/workerHelpers.js')/g" "$worker_helpers" > "$temp_file"
        mv "$temp_file" "$worker_helpers"
    done
}

build_variant_threads() {
    local out_dir="pkg-threads"
    local flags="-C target-feature=+atomics,+bulk-memory,+mutable-globals -C link-arg=--shared-memory -C link-arg=--max-memory=1073741824 -C link-arg=--import-memory -C link-arg=--export=__heap_base -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base"

    echo "Building Threads version..."
    echo "Installing rust-src for nightly..."
    rustup component add rust-src --toolchain nightly

    RUSTFLAGS="$flags" cargo +nightly build -p geo-polygonize-wasm \
        --locked \
        --target $TARGET \
        --release \
        --features "console_error_panic_hook threads" \
        --lib \
        -Z build-std=std,panic_abort

    echo "Running wasm-bindgen for threads..."
    local wasm_path="target/$TARGET/release/geo_polygonize_wasm.wasm"
    if [ ! -f "$wasm_path" ]; then
        echo "Error: $wasm_path not found!"
        exit 1
    fi

    rm -rf "$out_dir"
    "$WASM_BINDGEN_BIN" --target web --out-dir "$out_dir" --out-name "geo_polygonize" "$wasm_path"

    if command -v wasm-opt &> /dev/null; then
        echo "Optimizing Threads..."
        wasm-opt -O3 --enable-threads --enable-bulk-memory -o "$out_dir/geo_polygonize_bg.wasm" "$out_dir/geo_polygonize_bg.wasm"
    fi

    rm -f "$out_dir/.gitignore"
    patch_threads_worker
}

mock_threads_package() {
    mkdir -p pkg-threads
    cat > pkg-threads/geo_polygonize.js <<'EOF'
export const polygonizeWithOptions = () => {};
export const initThreadPool = () => {};
export default async function init() {}
EOF
    cat > pkg-threads/geo_polygonize.d.ts <<'EOF'
export declare const polygonizeWithOptions: () => void;
export declare const initThreadPool: () => void;
export default function init(): Promise<void>;
EOF
}

build_requested_wasm() {
    setup_wasm_tools

    case "$VARIANT" in
        scalar)
            build_variant "scalar" ""
            ;;
        simd)
            build_variant "simd" "-C target-feature=+simd128"
            ;;
        threads)
            build_variant_threads
            ;;
        "")
            build_variant "scalar" ""
            if [ "$SKIP_SIMD" = "1" ]; then
                echo "Skipping SIMD build as requested. Copying scalar to simd to satisfy dependencies..."
                rm -rf pkg-simd
                cp -r pkg-scalar pkg-simd
            else
                build_variant "simd" "-C target-feature=+simd128"
            fi

            if [ "$SITE_BUILD" = "1" ]; then
                mock_threads_package
            else
                build_variant_threads
            fi
            ;;
        *)
            echo "Unknown WASM variant: $VARIANT"
            exit 1
            ;;
    esac
}

patch_vite_urls() {
    echo "Patching wasm-bindgen fallback URLs for Vite..."
    for bindgen_js in pkg-scalar/geo_polygonize.js pkg-simd/geo_polygonize.js pkg-threads/geo_polygonize.js; do
        if [ -f "$bindgen_js" ]; then
            temp_file=$(mktemp)
            sed -e "s/new URL('geo_polygonize_bg.wasm', import.meta.url)/new URL(\/\\* @vite-ignore \\*\/ 'geo_polygonize_bg.wasm', import.meta.url)/g" "$bindgen_js" > "$temp_file"
            mv "$temp_file" "$bindgen_js"
        fi
    done
}

export_bindings() {
    echo "Exporting our TS-RS bindings into wasm-bindgen definitions..."
    export TS_RS_EXPORT_DIR="crates/geo-polygonize-core/bindings"
    cargo run --locked -p geo-polygonize-core --bin export_bindings
    mkdir -p pkg-wrapper/bindings
    cp crates/geo-polygonize-core/bindings/* pkg-wrapper/bindings/

    for dir in pkg-scalar pkg-simd pkg-threads; do
        d_ts_file="${dir}/geo_polygonize.d.ts"
        if [ -f "$d_ts_file" ]; then
            temp_file=$(mktemp)
            echo "import { PolygonizerOptions } from '../pkg-wrapper/bindings/PolygonizerOptions';" > "$temp_file"
            sed -e 's/options_val: any/options_val: Partial<PolygonizerOptions>/g' "$d_ts_file" >> "$temp_file"
            mv "$temp_file" "$d_ts_file"
        fi
    done
}

copy_wasm_bindgen_types() {
    for env in standard slim threads; do
        if [ -d "dist/$env/es/bindings" ]; then
            mkdir -p "dist/$env/pkg-wrapper"
            cp -r "dist/$env/es/bindings" "dist/$env/pkg-wrapper/"
        fi
    done

    mkdir -p dist/standard/pkg-scalar dist/slim/pkg-scalar dist/threads/pkg-threads
    cp pkg-scalar/*.d.ts dist/standard/pkg-scalar/
    cp pkg-scalar/*.d.ts dist/slim/pkg-scalar/
    cp pkg-threads/*.d.ts dist/threads/pkg-threads/
}

bundle_package() {
    if [ ! -d "pkg-wrapper" ]; then
        echo "pkg-wrapper directory missing!"
        exit 1
    fi

    if [ ! -d "node_modules" ]; then
        npm install
    fi

    patch_vite_urls
    export_bindings

    echo "Running rollup..."
    npx rollup -c
    copy_wasm_bindgen_types

    echo "Preparing dist..."
    cp pkg-scalar/geo_polygonize_bg.wasm dist/geo_polygonize.wasm
    cp pkg-simd/geo_polygonize_bg.wasm dist/geo_polygonize_simd.wasm
}

if [ "$BUNDLE_ONLY" != "1" ]; then
    build_requested_wasm
fi

if [ "$NO_BUNDLE" = "1" ]; then
    exit 0
fi

bundle_package
echo "Build complete! Artifacts are in dist/"
