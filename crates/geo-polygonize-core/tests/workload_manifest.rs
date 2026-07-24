use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
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
        if workload.permitted_profiles.is_empty() || workload.retained_result_families.is_empty() {
            return Err(format!("{} has an empty contract list", workload.id));
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
fn validator_rejects_duplicate_ids_missing_licenses_bad_checksums_and_profiles() {
    let workload = |id: &str, license: &str, sha256: &str, profile: &str| {
        format!(
            r#"{{"id":"{id}","description":"clip","domain":"test","source_url":"https://example.com","license":"{license}","attribution":"test","artifact":{{"download_url":"https://example.com/clip","sha256":"{sha256}"}},"coordinate_reference":"EPSG:4326","units":"degrees","compatibility_class":"parity","permitted_profiles":["{profile}"],"retained_result_families":["polygons"],"size":{{"line_strings":1,"segments":1,"coordinates":2}}}}"#
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
