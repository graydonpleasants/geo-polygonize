use geo_polygonize_core::{
    normalize_polygonize_error, polygonize, Coord3D, Line3D, NodingGuarantee, PolygonizerOptions,
};
use geojson::{GeoJson, Value as GeoJsonValue};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    workloads: Vec<Workload>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Workload {
    id: String,
    description: String,
    domain: String,
    source_url: String,
    license: String,
    attribution: String,
    artifact: Artifact,
    coordinate_reference: String,
    units: String,
    compatibility_class: CompatibilityClass,
    permitted_profiles: Vec<Profile>,
    options: Vec<PolygonizerOptions>,
    retained_result_families: Vec<ResultFamily>,
    size: Size,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    clip_path: Option<String>,
    download_url: Option<String>,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CompatibilityClass {
    Parity,
    ExpectedDivergence,
    Invalid,
    Ambiguous,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Profile {
    AlreadyNoded,
    Floating,
    IterativeGrid,
    CertifiedFixed,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ResultFamily {
    Polygons,
    Dangles,
    CutEdges,
    InvalidRings,
    Provenance,
    Diagnostics,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CandidateClass {
    Sparse,
    Moderate,
    Dense,
    Pathological,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Size {
    line_strings: usize,
    segments: usize,
    coordinates: usize,
    expected_candidate_class: Option<CandidateClass>,
}

fn workload_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/workloads")
}

fn validate(manifest: &Manifest) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err("unsupported manifest schema".into());
    }
    let mut ids = HashSet::new();
    for workload in &manifest.workloads {
        if !ids.insert(&workload.id) {
            return Err(format!("duplicate workload ID: {}", workload.id));
        }
        for (name, value) in [
            ("description", &workload.description),
            ("domain", &workload.domain),
            ("source_url", &workload.source_url),
            ("license", &workload.license),
            ("attribution", &workload.attribution),
            ("coordinate_reference", &workload.coordinate_reference),
            ("units", &workload.units),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{} has empty {name}", workload.id));
            }
        }
        if !workload.source_url.starts_with("https://") {
            return Err(format!("{} source URL must use HTTPS", workload.id));
        }
        if workload.permitted_profiles.is_empty()
            || workload.options.is_empty()
            || workload.retained_result_families.is_empty()
        {
            return Err(format!("{} has an empty contract list", workload.id));
        }
        for options in &workload.options {
            options
                .validate()
                .map_err(|error| format!("{} options: {error}", workload.id))?;
        }
        let _ = (
            &workload.compatibility_class,
            workload.size.line_strings,
            workload.size.segments,
            workload.size.coordinates,
            &workload.size.expected_candidate_class,
        );
        if workload.artifact.sha256.len() != 64
            || !workload
                .artifact
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(format!("{} has an invalid SHA-256", workload.id));
        }
        match (
            workload.artifact.clip_path.as_deref(),
            workload.artifact.download_url.as_deref(),
        ) {
            (Some(path), None) => {
                let bytes = std::fs::read(workload_root().join(path))
                    .map_err(|error| format!("{} clip: {error}", workload.id))?;
                let actual = format!("{:x}", Sha256::digest(bytes));
                if actual != workload.artifact.sha256 {
                    return Err(format!("{} checksum mismatch", workload.id));
                }
            }
            (None, Some(url)) if url.starts_with("https://") => {}
            _ => return Err(format!("{} must select one artifact source", workload.id)),
        }
    }
    Ok(())
}

#[test]
fn public_workload_manifest_is_valid() {
    let root = workload_root();
    let schema: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest-v1.schema.json")).unwrap())
            .unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );

    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(root.join("manifest-v1.json")).unwrap()).unwrap();
    validate(&manifest).unwrap();
}

#[test]
fn already_noded_workloads_pass_full_noding_validation() {
    let root = workload_root();
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(root.join("manifest-v1.json")).unwrap()).unwrap();
    for workload in manifest.workloads.iter().filter(|workload| {
        workload
            .permitted_profiles
            .iter()
            .any(|profile| matches!(profile, Profile::AlreadyNoded))
    }) {
        let path = workload.artifact.clip_path.as_ref().unwrap();
        let mut options = workload
            .options
            .iter()
            .find(|options| !options.node_input)
            .unwrap()
            .clone();
        options.noding.guarantee = NodingGuarantee::Validate;
        polygonize(load_segments(&root.join(path)), &options)
            .unwrap_or_else(|error| panic!("{} is not fully noded: {error}", workload.id));
    }
}

#[test]
fn certified_fixed_workloads_have_zero_residual_noding_failures() {
    let root = workload_root();
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(root.join("manifest-v1.json")).unwrap()).unwrap();
    let mut certified_workloads = 0;
    let mut failures = BTreeMap::new();
    for workload in manifest.workloads.iter().filter(|workload| {
        workload
            .permitted_profiles
            .iter()
            .any(|profile| matches!(profile, Profile::CertifiedFixed))
    }) {
        certified_workloads += 1;
        let options = workload
            .options
            .iter()
            .find(|options| {
                matches!(
                    options.noding.guarantee,
                    NodingGuarantee::CertifiedFixedPrecision
                )
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} permits certified-fixed but has no explicit certified options",
                    workload.id
                )
            });
        let path = workload.artifact.clip_path.as_ref().unwrap_or_else(|| {
            panic!(
                "{} certified workload must have a checked-in clip",
                workload.id
            )
        });
        if let Err(error) = polygonize(load_segments(&root.join(path)), options) {
            failures.insert(workload.id.clone(), normalize_polygonize_error(&error));
        }
    }
    assert!(
        certified_workloads > 0,
        "public manifest must retain at least one certified-fixed workload"
    );
    assert!(
        failures.is_empty(),
        "certified-fixed public workloads retained residual noding failures:\n{}",
        serde_json::to_string_pretty(&failures).unwrap()
    );
}

#[test]
fn checked_in_size_descriptors_match_geojson() {
    let root = workload_root();
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(root.join("manifest-v1.json")).unwrap()).unwrap();
    for workload in manifest.workloads {
        let Some(path) = workload.artifact.clip_path else {
            continue;
        };
        let geojson: GeoJson = std::fs::read_to_string(root.join(path))
            .unwrap()
            .parse()
            .unwrap();
        let GeoJson::FeatureCollection(collection) = geojson else {
            panic!("{} must be a FeatureCollection", workload.id);
        };
        let mut actual = (0usize, 0usize, 0usize);
        for feature in collection.features {
            let lines = match feature.geometry.unwrap().value {
                GeoJsonValue::LineString(line) => vec![line],
                GeoJsonValue::MultiLineString(lines) => lines,
                _ => panic!("{} must contain line strings", workload.id),
            };
            for line in lines {
                actual.0 += 1;
                actual.1 += line.len().saturating_sub(1);
                actual.2 += line.len();
            }
        }
        assert_eq!(
            actual,
            (
                workload.size.line_strings,
                workload.size.segments,
                workload.size.coordinates,
            ),
            "{} size descriptor",
            workload.id
        );
    }
}

fn load_segments(path: &Path) -> Vec<Line3D> {
    let geojson: GeoJson = std::fs::read_to_string(path).unwrap().parse().unwrap();
    let GeoJson::FeatureCollection(collection) = geojson else {
        panic!("workload must be a FeatureCollection");
    };
    let mut lines = Vec::new();
    for feature in collection.features {
        match feature.geometry.unwrap().value {
            GeoJsonValue::LineString(line) => lines.push(line),
            GeoJsonValue::MultiLineString(feature_lines) => lines.extend(feature_lines),
            _ => panic!("workload must contain line strings"),
        }
    }
    lines
        .into_iter()
        .enumerate()
        .flat_map(|(index, line)| {
            line.windows(2)
                .map(move |pair| {
                    let coordinate = |position: &[f64]| {
                        Coord3D::new(
                            position[0],
                            position[1],
                            position.get(2).copied().unwrap_or_default(),
                        )
                    };
                    Line3D::new(
                        coordinate(&pair[0]),
                        coordinate(&pair[1]),
                        u32::try_from(index + 1).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn validator_rejects_duplicate_ids_missing_licenses_bad_checksums_and_profiles() {
    let workload = |id: &str, license: &str, sha256: &str, profile: &str| {
        format!(
            r#"{{"id":"{id}","description":"clip","domain":"test","source_url":"https://example.com","license":"{license}","attribution":"test","artifact":{{"download_url":"https://example.com/clip","sha256":"{sha256}"}},"coordinate_reference":"EPSG:4326","units":"degrees","compatibility_class":"parity","permitted_profiles":["{profile}"],"options":[{{}}],"retained_result_families":["polygons"],"size":{{"line_strings":1,"segments":1,"coordinates":2}}}}"#
        )
    };
    let hash = "0".repeat(64);
    let duplicate: Manifest = serde_json::from_str(&format!(
        r#"{{"schema_version":1,"workloads":[{},{}]}}"#,
        workload("same", "MIT", &hash, "floating"),
        workload("same", "MIT", &hash, "floating")
    ))
    .unwrap();
    assert!(validate(&duplicate).unwrap_err().contains("duplicate"));

    let missing_license: Manifest = serde_json::from_str(&format!(
        r#"{{"schema_version":1,"workloads":[{}]}}"#,
        workload("missing-license", "", &hash, "floating")
    ))
    .unwrap();
    assert!(validate(&missing_license).unwrap_err().contains("license"));

    let bad_checksum: Manifest = serde_json::from_str(&format!(
        r#"{{"schema_version":1,"workloads":[{}]}}"#,
        workload("bad-hash", "MIT", "xyz", "floating")
    ))
    .unwrap();
    assert!(validate(&bad_checksum).unwrap_err().contains("SHA-256"));

    let unsupported = format!(
        r#"{{"schema_version":1,"workloads":[{}]}}"#,
        workload("bad-profile", "MIT", &hash, "gpu")
    );
    assert!(serde_json::from_str::<Manifest>(&unsupported).is_err());
}
