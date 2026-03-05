# AI Agent Instructions

## Pull Request and Commit Standards

### PR Titles
The PR title **must** follow the Conventional Commits specification. This ensures that `release-please` functions correctly and changelogs are generated accurately.

When creating a Pull Request, the title must be in the following format:
`<type>(<scope>): <short description>`

#### Allowed Scopes
The scope is **required** for every Pull Request. You must use one of the following scopes depending on what part of the repository you are modifying:

*   `core`: Modifications to the Rust core implementation (`crates/geo-polygonize-core`).
*   `python`: Modifications to the Python bindings (`python/`, `crates/geo-polygonize-core/src/python.rs`).
*   `wasm`: Modifications to the WASM bindings (`pkg-threads/`, `pkg-wrapper/`, `crates/geo-polygonize-wasm`).
*   `fleet`: Modifications to the fleet automation tools (`scripts/fleet/`).
*   `deps`: Dependency updates or modifications.

If a PR spans multiple areas, try to pick the most significant one or consider if the PR should be split.

#### Allowed Types
Standard Conventional Commit types apply:
*   `feat`: A new feature.
*   `fix`: A bug fix.
*   `docs`: Documentation only changes.
*   `style`: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc).
*   `refactor`: A code change that neither fixes a bug nor adds a feature.
*   `perf`: A code change that improves performance.
*   `test`: Adding missing tests or correcting existing tests.
*   `build`: Changes that affect the build system or external dependencies (example scopes: gulp, broccoli, npm).
*   `ci`: Changes to our CI configuration files and scripts.
*   `chore`: Other changes that don't modify src or test files.
*   `revert`: Reverts a previous commit.

#### Examples
*   `feat(core): implement SIMD intersection checks`
*   `fix(python): resolve memory leak in FFI translation`
*   `docs(wasm): update README for threads package`
*   `chore(deps): update rayon to 1.10.0`
