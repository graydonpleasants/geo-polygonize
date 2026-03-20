# Security Policy and Threat Model

## Reporting a Vulnerability

Please report security issues directly to the maintainers via email or through the GitHub security advisory reporting tool if available. Do not create public issues for undisclosed security vulnerabilities.

## Threat Model

`geo-polygonize` is a geospatial library designed to process linework into polygons. It is intended to be used as a backend kernel in various environments including Rust native servers, Python data science environments, and WebAssembly running in the browser.

### In Scope

The following vectors are considered within the scope of our threat model:
- **Panic Amplification:** Panicking across FFI, Wasm, or Python boundaries. The library must catch unwinding panics at the boundary and translate them to safe, structured errors to prevent host crashes.
- **Out of Bounds Memory Access:** When processing typed arrays, Arrow IPC streams, or FFI buffers, offsets and lengths must be strictly validated before processing to prevent out-of-bounds reads or writes.
- **Algorithmic Complexity Attacks (Denial of Service):** Inputs crafted to cause exponential or extreme combinatorial explosion during spatial indexing or intersection noding. While pathological cases might be slow, the library should ideally have safeguards or bounds.

### Out of Scope

- **Data Poisoning:** We assume the geometries provided to the API do not represent malicious payload outside the context of standard geometric evaluation (e.g., executing arbitrary code hidden in coordinate arrays).
- **Network Attacks:** The core library does not perform network operations. Security of the transport layer when downloading or transmitting geo-data is the responsibility of the host application.

## Panic Safety

We use `std::panic::catch_unwind` at language and memory boundaries (FFI, PyO3, wasm-bindgen). Expected invariants are that any internal logic bug resulting in a panic will yield a safe `Err` type to the caller rather than aborting the process.

## Unsafe Code

Usage of `unsafe` is minimized. Where used (e.g., for SIMD intrinsics or zero-copy FFI buffers), the safety invariants must be clearly documented above the `unsafe` block.
