mod common {
    pub mod cfb_fixture;
}

use common::cfb_fixture::{cfb_fixture_paths, fixture_lines, read_cfb_fixture, run_cfb_fixture};
use geo_polygonize_core::options::{NodingGuarantee, PolygonizerOptions, SnapStrategy};
use geo_polygonize_core::polygonize;

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
