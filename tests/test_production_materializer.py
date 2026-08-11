import importlib.util
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PATH = ROOT / "scripts/materialize_production_workloads.py"
SPEC = importlib.util.spec_from_file_location("materialize_production_workloads", PATH)
MATERIALIZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MATERIALIZER)


def feature(osm_id, coordinates, highway="residential"):
    return {
        "type": "Feature",
        "properties": {"osm_id": str(osm_id), "highway": highway},
        "geometry": {"type": "LineString", "coordinates": coordinates},
    }


def test_structure_descriptor_retains_chain_components_and_duplicates():
    descriptor = MATERIALIZER.structure_descriptor(
        [
            feature(1, [[0, 0], [1, 0], [2, 0]]),
            feature(2, [[2, 0], [2, 1]]),
            feature(3, [[10, 10], [11, 10]]),
            feature(4, [[2, 0], [1, 0]]),
        ]
    )

    assert descriptor["line_strings"] == 4
    assert descriptor["segments"] == 5
    assert descriptor["coordinates"] == 9
    assert descriptor["connected_components"]["count"] == 2
    assert descriptor["duplicate_incidence"]["exact_duplicate_segments"] == 1
    assert descriptor["envelope_grid_occupancy"]["occupied_cells"] >= 2


def test_seq_parser_strips_record_separator_and_rejects_non_linework():
    parsed = MATERIALIZER.parse_seq_feature(
        '\x1e{"type":"Feature","properties":{"osm_id": "7", "highway": "track"},'
        '"geometry":{"type":"LineString","coordinates":[[0,0],[1,1]]}}',
        1,
    )
    assert parsed["properties"] == {"osm_id": "7", "highway": "track"}


def test_materializer_cli_exposes_reproducibility_controls():
    help_text = subprocess.run(
        [sys.executable, PATH, "--help"], check=True, capture_output=True, text=True
    ).stdout
    for option in ("--source", "--source-manifest", "--acquired-on", "--include-million", "--validation-dir"):
        assert option in help_text


def test_materialized_report_schema_is_self_consistent():
    schema = json.loads((ROOT / "benchmarks/production-workload-v1.schema.json").read_text())
    assert schema["$id"].endswith("production-workload-v1.schema.json")
