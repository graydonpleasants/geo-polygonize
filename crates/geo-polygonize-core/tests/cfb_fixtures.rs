mod common {
    pub mod cfb_fixture;
}

use common::cfb_fixture::{cfb_fixture_paths, fixture_lines, read_cfb_fixture, run_cfb_fixture};
use geo_polygonize_core::polygonize;
use geo_polygonize_core::{NodingGuarantee, PolygonizerOptions, SnapStrategy};

#[test]
fn cfb_fixtures_match_expected_output() {
    let paths = cfb_fixture_paths();
    assert!(!paths.is_empty(), "expected at least one CFB fixture");

    for path in paths {
        let fixture = read_cfb_fixture(&path);
        run_cfb_fixture(&fixture);
    }
}

#[test]
fn cfb_endpoint_reference_insertions_retain_reviewed_microfaces() {
    let fixture = cfb_fixture_paths()
        .into_iter()
        .map(|path| read_cfb_fixture(&path))
        .find(|fixture| fixture.case_id == "large_lot_block_anon_curb_snap_gap")
        .expect("the public curb fixture must exist");

    // These original source groups independently reproduce the three additional
    // faces. The first closes during pre-snap; the other two also depend on the
    // existing fixed-grid/source-coordinate-restoration policy.
    for (ids, expected_areas) in [
        (vec![207, 2803], vec![0.000185459140050504]),
        (
            vec![2262, 3038, 3057],
            vec![0.000002040355795070, 0.000015179968006478],
        ),
    ] {
        let lines = fixture_lines(&fixture)
            .into_iter()
            .filter(|line| ids.contains(&line.line_id));
        let result = polygonize(lines, &PolygonizerOptions::cfb_robust_v1()).unwrap();
        assert_eq!(result.polygons.len(), expected_areas.len());
        let mut areas: Vec<_> = result
            .polygons
            .iter()
            .map(|polygon| polygon.unsigned_area_2d())
            .collect();
        areas.sort_by(f64::total_cmp);
        for (actual, expected) in areas.into_iter().zip(expected_areas) {
            assert!((actual - expected).abs() < 1e-10);
        }
        let expected_ids: Vec<u64> = ids.into_iter().map(u64::from).collect();
        for polygon in result.polygons {
            assert_eq!(polygon.exterior.len(), 4);
            assert!(polygon.interiors.is_empty());
            assert_eq!(polygon.provenance.unwrap().boundary_line_ids, expected_ids);
        }
        assert!(result.invalid_rings.is_empty());
    }
}

#[test]
fn cfb_fixtures_pass_certified_fixed_precision_noding() {
    for path in cfb_fixture_paths() {
        let fixture = read_cfb_fixture(&path);
        let mut options = PolygonizerOptions::cfb_robust_v1();
        options.snap_strategy = SnapStrategy::Grid;
        options.noding.guarantee = NodingGuarantee::CertifiedFixedPrecision;

        polygonize(fixture_lines(&fixture), &options)
            .unwrap_or_else(|err| panic!("{} failed certification: {err}", fixture.case_id));
    }
}
