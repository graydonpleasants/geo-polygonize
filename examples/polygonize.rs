use clap::Parser;
use geo_polygonize::Polygonizer;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::convert::TryInto;
use geo::Area;
use geo_types::{LineString, Polygon};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input GeoJSON file (LineStrings)
    #[arg(short, long)]
    input: PathBuf,

    /// Output GeoJSON file (Polygons)
    #[arg(short, long)]
    output: PathBuf,

    /// Enable robust noding (split intersecting lines)
    #[arg(long, default_value_t = false)]
    node: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // DEBUG: Test area calculation
    let ls = LineString::from(vec![
        (0.0, 0.0), (10.0, 0.0), (0.0, 10.0), (0.0, 0.0)
    ]);
    println!("DEBUG: Triangle coords: {:?}", ls);
    println!("DEBUG: is_closed: {}", ls.is_closed());
    println!("DEBUG: Triangle LS signed_area: {}", ls.signed_area());

    let poly = Polygon::new(ls.clone(), vec![]);
    println!("DEBUG: Triangle POLY signed_area: {}", poly.signed_area());

    let args = Args::parse();

    // Read Input
    if !args.input.exists() {
        return Ok(());
    }

    println!("Reading input from {:?}", args.input);
    let file = File::open(&args.input)?;
    let reader = BufReader::new(file);
    let geojson: GeoJson = serde_json::from_reader(reader)?;

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = args.node;

    // ... (rest omitted for brevity as we just want the debug prints)

    Ok(())
}
