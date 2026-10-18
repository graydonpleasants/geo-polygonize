"""Exact projection equivalence; the independent analytical expectations are deliberate."""
import numpy as np
import pytest
from shapely.geometry import Polygon

from geo_polygonize import cfb_robust_options, polygonize, polygonize_with_options


# Core fixes #1562 and #1561 are included, so independent point-touch and
# endpoint correctness assertions run unconditionally alongside full/lean parity.


SQUARE = [(0., 0.), (10., 0.), (10., 10.), (0., 10.), (0., 0.)]
INNER = [(2., 2.), (4., 2.), (4., 4.), (2., 4.), (2., 2.)]
POINT_TOUCH = [
    [(-10., 10.), (0., 0.)], [(0., 0.), (0., -10.)],
    [(0., -10.), (-10., 10.)], [(0., 0.), (-2., 1.)],
    [(-2., 1.), (-2., -1.)], [(-2., -1.), (0., 0.)],
]
CASES = {
    "empty": [],
    "square": [SQUARE],
    "nested_hole": [SQUARE, INNER],
    "point_touch": POINT_TOUCH,
    "endpoint_region": [[(10., -10.), (0., 0.)], [(-10., 1.), (0., -0.09)], [(10., -10.), (-10., 1.)]],
    "dangle": [SQUARE, [(10., 10.), (12., 12.), (14., 12.)]],
    "cut": [SQUARE, INNER, [(0., 0.), (2., 2.)]],
    "bowtie": [[(0., 0.), (10., 10.), (10., 0.), (0., 10.), (0., 0.)]],
    "duplicate_alias": [SQUARE, [(0., 0.), (10., 0.)], [(3., 0.), (7., 0.)]],
    "zero_length": [[(0., 0.), (0., 0.)], SQUARE],
}


def arguments(name, stride=2, reverse=False, options=None):
    lines = CASES[name]
    ids = list(range(101, 101 + len(lines)))
    if reverse:
        lines = [list(reversed(line)) for line in reversed(lines)]
        ids.reverse()
    if stride == 3:
        lines = [[(x, y, x + 2 * y) for x, y in line] for line in lines]
    coords = np.asarray([p for line in lines for p in line], dtype=np.float64).reshape(-1, stride)
    offsets = np.asarray([sum(map(len, lines[:i])) for i in range(len(lines))], dtype=np.uint32)
    return dict(coords=coords, offsets=offsets, stride=stride,
                line_ids=np.asarray(ids, dtype=np.uint32), options=options)


def freeze(value):
    """Preserve float bits (including signed zero), array shape/dtype, all IDs."""
    if isinstance(value, np.ndarray):
        return value.dtype.str, value.shape, value.tobytes()
    if isinstance(value, float):
        return value.hex()
    if hasattr(value, "shell"):
        return freeze(vars(value))
    if isinstance(value, dict):
        return {k: freeze(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return tuple(freeze(v) for v in value)
    return value


def semantic_report(report):
    result = {k: v for k, v in report.items()
              if k not in ("binding_timings", "binding_profile", "topology_fingerprint")}
    if "diagnostics" in result:
        result["diagnostics"] = {k: v for k, v in result["diagnostics"].items() if k != "phase_times"}
    return freeze(result)


@pytest.mark.parametrize("name", CASES)
@pytest.mark.parametrize("stride", [2, 3])
@pytest.mark.parametrize("reverse", [False, True])
@pytest.mark.parametrize("policy", ["floating", "cfb", "certified"])
def test_lean_report_matches_every_semantic_report_field(name, stride, reverse, policy):
    options = cfb_robust_options()
    if policy == "floating":
        options.update(precision_model={"type": "floating"}, pre_snap_tolerance=0.)
    elif policy == "certified":
        options.update(precision_model={"type": "fixed_grid", "grid_size": 0.01},
                       pre_snap_tolerance=0., snap_strategy="Grid")
        options["noding"]["guarantee"] = "CertifiedFixedPrecision"
    kwargs = arguments(name, stride, reverse, options)
    original = [kwargs[k].copy() for k in ("coords", "offsets", "line_ids")]
    full = polygonize_with_options(**kwargs)
    lean = polygonize_with_options(**kwargs, output="lean_report")
    assert set(lean) == set(full) - {"topology_fingerprint"}
    assert semantic_report(lean) == semantic_report(full)
    assert lean["binding_timings"]["fingerprint_ms"] == 0.
    assert "binding_profile" not in lean
    for key, prior in zip(("coords", "offsets", "line_ids"), original):
        np.testing.assert_array_equal(kwargs[key], prior)


@pytest.mark.parametrize("stride", [2, 3])
def test_correctness_baseline_preserves_point_touch_owned_hole_and_sources(stride):
    result = polygonize_with_options(**arguments("point_touch", stride, options=cfb_robust_options()), output="lean_report")
    polygons = sorted(result["polygons"], key=lambda p: Polygon(p.shell, p.holes).area)
    assert [Polygon(p.shell, p.holes).area for p in polygons] == [2., 48.]
    assert [len(p.holes) for p in polygons] == [0, 1]
    assert all(Polygon(p.shell, p.holes).is_valid for p in polygons)
    assert [p.provenance["boundary_line_ids"] for p in polygons] == [(104, 105, 106), (101, 102, 103, 104, 105, 106)]
    assert result["dangles"] == result["cut_edges"] == result["invalid_rings"] == []


def test_correctness_baseline_retains_endpoint_region_closure():
    result = polygonize_with_options(**arguments("endpoint_region", options=cfb_robust_options()), output="lean_report")
    assert len(result["polygons"]) == 1
    p = result["polygons"][0]
    assert Polygon(p.shell, p.holes).area == pytest.approx(44.55)
    assert p.provenance["boundary_line_ids"] == (101, 102, 103)


def test_complete_alias_provenance_survives_lean_projection():
    result = polygonize_with_options(**arguments("duplicate_alias", options=cfb_robust_options()), output="lean_report")
    assert len(result["polygons"]) == 1
    assert result["polygons"][0].provenance == {
        "boundary_line_ids": (101, 102, 103), "input_profile_id": "cfb_robust_v1"}


@pytest.mark.parametrize("output", ["buffers", "objects", "report", "lean_report"])
def test_binding_profiling_is_explicit_and_nonsemantic(output):
    kwargs = arguments("nested_hole", options=cfb_robust_options())
    plain = polygonize_with_options(**kwargs, output=output)
    profiled = polygonize_with_options(**kwargs, output=output, profile_binding=True)
    assert "binding_profile" not in plain
    assert semantic_report(plain) == semantic_report(profiled)
    profile = profiled["binding_profile"]
    assert set(profile) == {"input_conversion_ms", "native_polygonize_ms", "thread_spawn_ms", "buffer_conversion_ms", "buffer_export_ms", "fingerprint_build_ms", "fingerprint_json_ms", "python_objects_ms", "diagnostics_json_ms", "binding_body_ms"}
    assert all(value >= 0 for value in profile.values())
    if output != "report":
        assert profile["fingerprint_build_ms"] == profile["fingerprint_json_ms"] == 0.
    else:
        assert plain["topology_fingerprint"] == profiled["topology_fingerprint"]


def test_legacy_and_shapely_projections_accept_lean_report():
    kwargs = arguments("nested_hole")
    kwargs.pop("options")
    full = polygonize(**kwargs)
    lean = polygonize(**kwargs, output="lean_report", profile_binding=True)
    assert semantic_report(full) == semantic_report(lean)
    full_shapes = polygonize_with_options(**kwargs, return_polygons=True)
    lean_shapes = polygonize_with_options(**kwargs, return_polygons=True, output="lean_report")
    assert [p.wkb for p in full_shapes] == [p.wkb for p in lean_shapes]


@pytest.mark.parametrize("failure", ["limit", "nonfinite", "z_conflict", "invalid_options"])
def test_lean_errors_match_report(failure):
    kwargs = arguments("square", options=cfb_robust_options())
    if failure == "limit":
        kwargs["execution_limits"] = {"max_input_segments": 0}
    elif failure == "nonfinite":
        kwargs["coords"][0, 0] = np.nan
    elif failure == "z_conflict":
        kwargs = arguments("point_touch", stride=3, options=cfb_robust_options())
        kwargs["coords"][1, 2] += 10.
        kwargs["options"]["z"]["policy"] = "ErrorOnConflict"
    else:
        kwargs["options"]["precision_model"]["grid_size"] = -1.
    errors = []
    for output in ("report", "lean_report"):
        with pytest.raises(Exception) as caught:
            polygonize_with_options(**kwargs, output=output)
        errors.append((type(caught.value), str(caught.value), getattr(caught.value, "normalized", None)))
    assert errors[0] == errors[1]


@pytest.mark.parametrize("stride", [2, 3])
def test_all_nonpolygon_geometry_families_remain_complete(stride):
    import json
    from pathlib import Path

    path = Path(__file__).resolve().parents[1] / "crates/geo-polygonize-core/tests/fixtures/topology/reported_outputs.json"
    fixture = json.loads(path.read_text())
    coords = [[line[end][axis] for axis in ("x", "y", "z")[:stride]]
              for line in fixture["inputs"] for end in ("start", "end")]
    options = cfb_robust_options()
    options.update(node_input=False, pre_snap_tolerance=0., precision_model={"type": "floating"})
    kwargs = dict(coords=coords, offsets=np.arange(0, len(coords), 2, dtype=np.uint32),
                  stride=stride, line_ids=[line["id"] for line in fixture["inputs"]], options=options)
    full = polygonize_with_options(**kwargs)
    lean = polygonize_with_options(**kwargs, output="lean_report")
    assert semantic_report(full) == semantic_report(lean)
    assert len(lean["polygons"]) == 3
    assert len(lean["dangles"]) == len(lean["cut_edges"]) == 1
    assert len(lean["invalid_rings"]) == 2
    for key in ("dangles", "cut_edges", "invalid_rings"):
        expected = [[tuple(point[axis] for axis in ("x", "y", "z")[:stride]) for point in line]
                    for line in fixture["expected"][key]]
        assert freeze(lean[key]) == freeze(expected)


@pytest.mark.parametrize("ids", [None, [0, 0, 0], [7, 7, 7]])
def test_unknown_and_repeated_source_ids_match_report(ids):
    kwargs = arguments("duplicate_alias", options=cfb_robust_options())
    kwargs["line_ids"] = ids
    full = polygonize_with_options(**kwargs)
    lean = polygonize_with_options(**kwargs, output="lean_report")
    assert semantic_report(lean) == semantic_report(full)
    assert lean["polygons"][0].provenance["boundary_line_ids"] == (() if ids is None or ids[0] == 0 else (7,))


def test_lean_report_does_not_enable_optional_diagnostics_or_provenance():
    kwargs = arguments("square", options={"diagnostics": {"enabled": False}, "provenance": {"enabled": False}})
    full = polygonize_with_options(**kwargs)
    lean = polygonize_with_options(**kwargs, output="lean_report")
    assert semantic_report(lean) == semantic_report(full)
    assert "diagnostics" not in lean
    assert lean["polygons"][0].provenance is None


def test_enabled_core_phase_timings_remain_present_in_lean_report():
    options = cfb_robust_options()
    options["diagnostics"]["timings"] = True
    kwargs = arguments("nested_hole", options=options)
    full = polygonize_with_options(**kwargs)
    lean = polygonize_with_options(**kwargs, output="lean_report")
    assert semantic_report(lean) == semantic_report(full)
    for report in (full, lean):
        phases = report["diagnostics"]["phase_times"]
        assert set(phases) == {"ingest_and_node", "graph_build", "ring_extraction", "containment", "output_flatten"}
        for value in phases.values():
            assert set(value) == {"secs", "nanos"}
            assert value["secs"] >= 0 and 0 <= value["nanos"] < 1_000_000_000


def test_direct_native_legacy_entrypoint_accepts_lean_and_profile():
    from geo_polygonize.geo_polygonize_core import polygonize as native_polygonize
    kwargs = arguments("nested_hole")
    kwargs.pop("options")
    kwargs["coords"] = kwargs["coords"].ravel()
    kwargs["report_mode"] = True
    full = native_polygonize(**kwargs)
    lean = native_polygonize(**kwargs, output="lean_report", profile_binding=True)
    assert semantic_report(lean) == semantic_report(full)
    assert "diagnostics" in lean
    assert lean["binding_profile"]["fingerprint_build_ms"] == 0.


def core_fixtures():
    import json
    from pathlib import Path

    root = Path(__file__).resolve().parents[1] / "crates/geo-polygonize-core/tests/fixtures"
    fixtures = []
    for path in sorted(root.rglob("*.json")):
        value = json.loads(path.read_text())
        if isinstance(value, dict) and "inputs" in value:
            fixtures.append((str(path.relative_to(root)), value))
    return fixtures


@pytest.mark.parametrize("name,fixture", core_fixtures(), ids=lambda value: value if isinstance(value, str) else "fixture")
@pytest.mark.parametrize("stride", [2, 3])
def test_existing_core_fixtures_preserve_exact_projection(name, fixture, stride):
    coords = [[line[end][axis] for axis in ("x", "y", "z")[:stride]]
              for line in fixture["inputs"] for end in ("start", "end")]
    kwargs = dict(coords=np.asarray(coords, dtype=np.float64).reshape(-1, stride),
                  offsets=np.arange(0, len(coords), 2, dtype=np.uint32), stride=stride,
                  line_ids=[line["id"] for line in fixture["inputs"]], options=fixture.get("options"))
    full = polygonize_with_options(**kwargs)
    lean = polygonize_with_options(**kwargs, output="lean_report")
    assert semantic_report(lean) == semantic_report(full), name
