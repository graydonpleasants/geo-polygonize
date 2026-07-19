# Contributing to Geo Polygonize

Thank you for your interest in contributing!

## Development Setup

1.  **Install Rust**: Ensure you have the latest stable Rust toolchain installed.
    ```bash
    rustup update stable
    ```

2.  **Install Node.js**: Required for building and testing WebAssembly artifacts.
    - Recommended: Node.js v18+.

3.  **Install Dependencies**:
    ```bash
    npm install
    ```

## Running Tests

### Rust Tests
Run unit and integration tests:
```bash
cargo test
```

### WebAssembly Tests
Build and run JS tests:
```bash
# Build WASM
./scripts/build_wasm.sh

# Run JS tests
npm test
```

## Benchmarks

Run the benchmark suite:
```bash
cargo bench
```

## Code Style

- Format code with `cargo fmt`.
- Lint code with `cargo clippy`.
- Ensure no warnings are introduced.

## Pull Requests

1.  Fork the repository.
2.  Create a feature branch.
3.  Add tests for your changes.
4.  Submit a Pull Request.

### Maintainer Codex tasks

The `Codex Task` GitHub workflow can implement an open issue as a draft pull
request. Add `OPENAI_API_KEY` as a repository secret, dispatch the workflow with
an issue number, then review the generated diff and CI results before marking
the pull request ready.

## License

By contributing, you agree that your contributions will be licensed under the project's MIT/Apache-2.0 license.
