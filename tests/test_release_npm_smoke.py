import importlib.util
import json
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "verify_release_npm", ROOT / "scripts/verify_release.py"
)
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


@pytest.mark.parametrize("version", ["1.0.0", "1.1.0", "1.2.0", "1.2.0-rc.1", "2.0.0"])
def test_installed_npm_smoke_requires_packed_exports_from_1_2(monkeypatch, version):
    real_run = VERIFY.run

    def install_fixture(command, cwd, env=None):
        if command[0] == "node":
            return real_run(command, cwd, env)
        if command[1] == "install":
            assert command[-1] == f"geo-polygonize@{version}"
            package = cwd / "node_modules/geo-polygonize"
            package.mkdir(parents=True)
            (package / "package.json").write_text(json.dumps({
                "version": version,
                "type": "module",
                "exports": {
                    name: {"import": "./index.js", "require": "./missing-commonjs.cjs"}
                    for name in (".", "./slim")
                },
            }))
            (package / "dist").mkdir()
            (package / "dist/geo_polygonize.wasm").write_bytes(b"")
            (package / "index.js").write_text('''
export default async function init() {}
export async function initBest() {}
export function polygonizeReportWithOptions(square, options) {
  if (options.pre_snap_tolerance) {
    throw Object.assign(new Error("invalid options"), { normalized: {
      family: "invalid_argument", code: "unsupported_option_combination"
    }});
  }
  return JSON.stringify({ schema_version: 1, polygons: [{}] });
}
''')
        return ""

    monkeypatch.setattr(VERIFY, "run", install_fixture)
    if version in ["1.0.0", "1.1.0"]:
        assert VERIFY.smoke_npm(version)["status"] == "passed"
    else:
        with pytest.raises(VERIFY.VerificationError, match="missing packed export"):
            VERIFY.smoke_npm(version)
