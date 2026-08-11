# Release Process

Release publication is deliberately split across crates.io, npm, and PyPI.
The tag-triggered publish workflows are not atomic, so a release is not
complete until the post-publication verifier has installed the exact public
artifacts and attached a complete report to the GitHub release.

## Checklist

1.  **Open the release PR**:
    - Let `release-please` update all synchronized package versions.
    - Keep the PR draft until the previous release publication gate passes.

2.  **Update Changelog**:
    - Update `CHANGELOG.md` with all notable changes.
    - Change `[Unreleased]` to the new version number and date.

3.  **Run Tests**:
    - Run `cargo test` to verify Rust tests.
    - Run `./scripts/build_wasm.sh` and `npm test` to verify WASM build.

4.  **Create Release Commit**:
    - Commit changes: `git commit -am "chore: release vX.Y.Z"`.

5.  **Tag Release**:
    - Create a git tag: `git tag vX.Y.Z`.
    - Push changes and tag: `git push origin main --tags`.

6.  **Wait for publication verification**:
    - `Publish to crates.io`, `Publish to npm`, and `Publish Python Package` run
      from the tag.
    - `Verify release publication` polls all registries with bounded retries,
      runs the Rust, npm standard/slim, and Python install-smoke fixtures, and
      uploads `release-publication-report.json`.
    - A complete report is attached to the GitHub release as
      `publication-report.json`.

## Repair and rerun procedure

If verification fails, use the report artifact to identify the registry,
package, platform, or smoke fixture. Repair only the failed publication using
the registry's supported rerun procedure; never force-move the release tag or
overwrite a different version. Then manually dispatch **Verify release
publication** with the exact version and tag. A later release remains gated
until the prior tag has a complete report.

For a local audit against public artifacts, use bounded retries explicitly:

```bash
GH_TOKEN="$(gh auth token)" python3 scripts/verify_release.py \
  --version 1.0.0 \
  --tag geo-polygonize-v1.0.0 \
  --repository graydonpleasants/geo-polygonize \
  --github-token "$GH_TOKEN" \
  --attempts 1 \
  --delay-seconds 0 \
  --report target/release-publication-report.json
```

The report is evidence, not a replacement for the registry-specific publish
workflows. A failed or missing report must be repaired before starting another
release.
