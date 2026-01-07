use clap::Parser;
use geo_polygonize::Polygonizer;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::convert::TryInto;

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
    let args = Args::parse();

    // Read Input
    println!("Reading input from {:?}", args.input);
    let file = File::open(&args.input)?;
    let reader = BufReader::new(file);
    let geojson: GeoJson = serde_json::from_reader(reader)?;

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = args.node;

    let mut line_count = 0;

    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    // Convert geojson::Geometry to geo_types::Geometry
                    // geojson crate provides conversion?
                    // geojson 0.24 has `try_into` for geo_types?
                    // Yes, `use std::convert::TryInto;`
                    let geo_geom: geo_types::Geometry<f64> = geom.try_into()?;
                    polygonizer.add_geometry(geo_geom);
                    line_count += 1;
                }
            }
        }
        GeoJson::Geometry(geom) => {
            let geo_geom: geo_types::Geometry<f64> = geom.try_into()?;
            polygonizer.add_geometry(geo_geom);
            line_count += 1;
        }
        GeoJson::Feature(feature) => {
             if let Some(geom) = feature.geometry {
                let geo_geom: geo_types::Geometry<f64> = geom.try_into()?;
                polygonizer.add_geometry(geo_geom);
                line_count += 1;
             }
        }
    }

    println!("Loaded {} features. Running polygonizer...", line_count);
    if args.node {
        println!("Robust noding enabled.");
    }

    let polygons = polygonizer.polygonize()?;

    println!("Found {} polygons.", polygons.len());

    // Write Output
    let features: Vec<Feature> = polygons.into_iter().map(|poly| {
        let geometry = Geometry::new(Value::from(&poly));
        Feature {
            bbox: None,
            geometry: Some(geometry),
            id: None,
            properties: None,
            foreign_members: None,
        }
    }).collect();

    let output_fc = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };

    let file = File::create(&args.output)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &GeoJson::FeatureCollection(output_fc))?;

    println!("Wrote output to {:?}", args.output);

    Ok(())
}
