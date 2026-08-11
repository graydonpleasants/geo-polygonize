import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "check_stable_api", ROOT / "scripts/check_stable_api.py"
)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def test_supported_root_exports_match_the_v1_allowlist():
    missing, unexpected = CHECK.check(ROOT)
    assert not missing, sorted(missing)
    assert not unexpected, sorted(unexpected)
