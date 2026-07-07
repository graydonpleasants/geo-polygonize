mod common {
    pub mod cfb_fixture;
}

use common::cfb_fixture::{cfb_fixture_paths, read_cfb_fixture, run_cfb_fixture};

#[test]
fn cfb_fixtures_match_expected_output() {
    let paths = cfb_fixture_paths();
    assert!(!paths.is_empty(), "expected at least one CFB fixture");

    for path in paths {
        let fixture = read_cfb_fixture(&path);
        run_cfb_fixture(&fixture);
    }
}
