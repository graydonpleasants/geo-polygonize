const fs = require('fs');
let file = fs.readFileSync('crates/geo-polygonize-wasm/Cargo.toml', 'utf8');
if (!file.includes('wasm-bindgen-test')) {
    file += '\n[dev-dependencies]\nwasm-bindgen-test = "0.3.64"\nweb-sys = { version = "0.3.91", features = ["console"] }\n';
    fs.writeFileSync('crates/geo-polygonize-wasm/Cargo.toml', file);
}
