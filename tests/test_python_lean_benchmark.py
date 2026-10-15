"""The benchmark must reject corruption even when Python assertions are disabled."""
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize("flags", [[], ["-O"]], ids=["normal", "optimized"])
def test_benchmark_gate_rejects_corrupt_lean_source_ids(tmp_path, flags):
    output = tmp_path / "must_not_exist.json"
    script = '''
import sys
sys.path.insert(0, 'benchmarks')
import python_lean_report as benchmark
original = benchmark.call

def corrupted(*args, **kwargs):
    report, polygons, row = original(*args, **kwargs)
    if args[-1] == 'lean_report':
        report['flat_line_ids'][0] ^= 1
    return report, polygons, row

benchmark.call = corrupted
try:
    benchmark.main()
except RuntimeError as exc:
    if 'report mismatch' not in str(exc):
        raise
else:
    raise RuntimeError('Corruption passed correctness gate')
'''
    subprocess.run([sys.executable, *flags, "-c", script, "--sizes", "1", "--kinds", "grid",
                    "--routes", "packed", "--samples", "1", "--warmups", "0", "--output", str(output)],
                   cwd=ROOT, check=True, capture_output=True, text=True)
    assert not output.exists()


def run_prepared(tmp_path, manifest, names):
    import json

    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest))
    output = tmp_path / "must_not_exist.json"
    result = subprocess.run([sys.executable, "-O", "benchmarks/python_lean_prepared.py",
                             "--manifest", str(path), "--input-root", str(tmp_path),
                             "--names", *names, "--samples", "1", "--warmups", "0", "--output", str(output)],
                            cwd=ROOT, capture_output=True, text=True)
    assert result.returncode != 0
    assert not output.exists()
    return result.stderr


def test_prepared_gate_rejects_duplicate_manifest_names(tmp_path):
    stderr = run_prepared(tmp_path, [{"name": "A"}, {"name": "A"}], ["A", "B"])
    assert "Duplicate manifest names" in stderr


def test_prepared_gate_rejects_xym_coordinates(tmp_path):
    import gzip
    import hashlib
    import json
    import shapely

    line = shapely.from_wkt("LINESTRING M (0 0 1, 1 1 2)")
    data = {"wkbs": [shapely.to_wkb(line, hex=True)], "line_ids": [1]}
    path = tmp_path / "xym.json.gz"
    path.write_bytes(gzip.compress(json.dumps(data).encode()))
    manifest = [{"name": "xym", "file": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                 "line_count": 1, "coordinate_count": 2}]
    stderr = run_prepared(tmp_path, manifest, ["xym"])
    assert "XY-only prepared lane" in stderr
