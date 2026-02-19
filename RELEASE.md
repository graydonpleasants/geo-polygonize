# Release Process

This document outlines the steps for releasing a new version of `geo-polygonize`.

## Checklist

1.  **Update Changelog**:
    - Update `CHANGELOG.md` with all notable changes.
    - Change `[Unreleased]` to the new version number and date.

2.  **Bump Version**:
    - Update version in `Cargo.toml`.
    - Update version in `package.json`.
    - Run `cargo check` to ensure `Cargo.lock` is updated.

3.  **Run Tests**:
    - Run `cargo test` to verify Rust tests.
    - Run `./scripts/build_wasm.sh` and `npm test` to verify WASM build.

4.  **Create Release Commit**:
    - Commit changes: `git commit -am "chore: release vX.Y.Z"`.

5.  **Tag Release**:
    - Create a git tag: `git tag vX.Y.Z`.
    - Push changes and tag: `git push origin main --tags`.

6.  **Publish to Crates.io**:
    - `cargo publish`.

7.  **Publish to NPM**:
    - `npm publish`.

## Automation

Currently, releases are manual. Future improvements may include GitHub Actions for automated publishing on tag push.
