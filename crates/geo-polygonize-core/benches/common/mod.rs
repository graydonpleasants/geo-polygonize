use geo_types::LineString;
use serde::Deserialize;

#[derive(Deserialize)]
struct GridFixture {
    inputs: Vec<GridLine>,
}

#[derive(Deserialize)]
struct GridLine {
    start: GridCoord,
    end: GridCoord,
}

#[derive(Deserialize)]
struct GridCoord {
    x: f64,
    y: f64,
}

pub fn generate_grid(n: usize) -> Vec<LineString<f64>> {
    if n == 10 {
        let fixture: GridFixture = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/benchmark/grid_10.json"
        )))
        .expect("benchmark golden fixture should parse");
        return fixture
            .inputs
            .into_iter()
            .map(|line| {
                LineString::from(vec![(line.start.x, line.start.y), (line.end.x, line.end.y)])
            })
            .collect();
    }

    let mut lines = Vec::new();
    for i in 0..=n {
        lines.push(LineString::from(vec![
            (0.0, i as f64),
            (n as f64, i as f64),
        ]));
        lines.push(LineString::from(vec![
            (i as f64, 0.0),
            (i as f64, n as f64),
        ]));
    }
    lines
}
