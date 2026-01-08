use geo_types::LineString;
use geo::Area;

fn main() {
    let ls = LineString::from(vec![
        (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
    ]);
    println!("Area: {}", ls.signed_area());
}
