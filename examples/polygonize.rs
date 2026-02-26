use clap::Parser;
use geo_polygonize_core::Polygonizer;
use geo_types::Geometry as GeoGeometry;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
use std::convert::TryInto;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Read Input
    if !args.input.exists() {
        eprintln!("Input file does not exist: {:?}", args.input);
        return Ok(());
    }

    println!("Reading input from {:?}", args.input);
    let file = File::open(&args.input)?;
    let reader = BufReader::new(file);
    let geojson: GeoJson = serde_json::from_reader(reader)?;

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = args.node;

    let mut count = 0;

    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    if let Ok(geo_geom) = geom.try_into() {
                        add_geometry(&mut polygonizer, geo_geom);
                        count += 1;
                    }
                }
            }
        }
        GeoJson::Geometry(geom) => {
            if let Ok(geo_geom) = geom.try_into() {
                add_geometry(&mut polygonizer, geo_geom);
                count += 1;
            }
        }
        GeoJson::Feature(feature) => {
            if let Some(geom) = feature.geometry {
                if let Ok(geo_geom) = geom.try_into() {
                    add_geometry(&mut polygonizer, geo_geom);
                    count += 1;
                }
            }
        }
    }

    println!("Added {} geometries. Polygonizing...", count);

    let result = polygonizer.polygonize()?;
    println!("Found {} polygons.", result.polygons.len());
    println!("Found {} dangles.", result.dangles.len());

    // Convert back to GeoJSON (handling 3D)
    let features: Vec<Feature> = result
        .polygons
        .into_iter()
        .map(|poly| {
            // poly is Polygon3D
            let exterior_ring: Vec<Vec<f64>> =
                poly.exterior.iter().map(|c| vec![c.x, c.y, c.z]).collect();

            let mut rings: Vec<Vec<Vec<f64>>> = vec![exterior_ring];

            for interior in poly.interiors {
                let interior_ring: Vec<Vec<f64>> =
                    interior.iter().map(|c| vec![c.x, c.y, c.z]).collect();
                rings.push(interior_ring);
            }

            let geometry = Geometry::new(Value::Polygon(rings));

            Feature {
                bbox: None,
                geometry: Some(geometry),
                id: None,
                properties: None,
                foreign_members: None,
            }
        })
        .collect();

    let output_fc = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };

    let output_geojson = GeoJson::FeatureCollection(output_fc);

    println!("Writing output to {:?}", args.output);
    let file = File::create(&args.output)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &output_geojson)?;

    Ok(())
}

fn add_geometry(polygonizer: &mut Polygonizer, geom: GeoGeometry<f64>) {
    match geom {
        GeoGeometry::LineString(ls) => {
            polygonizer.add_geometry(GeoGeometry::LineString(ls));
        }
        GeoGeometry::MultiLineString(mls) => {
            for ls in mls {
                polygonizer.add_geometry(GeoGeometry::LineString(ls));
            }
        }
        GeoGeometry::GeometryCollection(gc) => {
            for g in gc {
                add_geometry(polygonizer, g);
            }
        }
        _ => {
            // Ignore other types or try to add them if Polygonizer supports them?
            // Polygonizer::add_geometry takes Geometry, so we can just pass it.
            // But usually we want LineStrings.
            polygonizer.add_geometry(geom);
        }
    }
}
