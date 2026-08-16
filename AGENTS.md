# AI Agent Instructions

## Native Stacked Pull Requests

GitHub supports native stacked pull requests in public preview. A stack is a
same-repository chain where each pull request targets the head branch of the
pull request below it. Matching base branches creates the eligible chain, but
the GitHub stack metadata and stack map must also be created with `gh stack`
or the website's **Create stack** action.

Use the GitHub CLI stack extension for new work:

1. Start from the trunk with `gh stack init BRANCH-NAME`.
2. Add dependent layers with `gh stack add BRANCH-NAME`.
3. Commit each focused layer, then run `gh stack submit` to push branches and
   create/link the draft pull requests.

For existing branches or pull requests, use `gh stack link` with the complete
bottom-to-top sequence, for example `gh stack link 1376 1377`. Use
`gh stack init BRANCH1 BRANCH2` followed by `gh stack submit` when local stack
tracking is also needed. Creating pull requests through an API or connector
with a dependent base branch alone is not sufficient to guarantee the stack
object or stack map exists.

Maintain stacks with `gh stack view`, `gh stack rebase`, `gh stack push`, and
`gh stack sync --prune` after the bottom layer merges. Make lower-layer fixes
on that branch, then rebase the upstack; merge from the bottom upward. Keep
the working tree clean before restructuring with `gh stack modify`. All stack
branches must live in this repository; cross-fork stacks are unsupported.

Check the installed extension with `gh stack --help`; the current GitHub
documentation requires GitHub CLI 2.90.0 or later and Git 2.20 or later.
Reference: <https://docs.github.com/en/pull-requests/how-tos/create-pull-requests/creating-stacked-pull-requests>.

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
*   `deps`: Dependency updates or modifications.
*   `main`: Used for release PRs (e.g. `chore(main): release 0.1.2`).
*   `github`: Modifications to GitHub Actions workflows and configuration (`.github/`).

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
