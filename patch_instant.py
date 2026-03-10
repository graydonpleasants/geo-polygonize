import sys
with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

# Replace Instant::now with platform-independent implementation, or use conditional compilation for Wasm,
# since Instant::now() panics on wasm32-unknown-unknown target without WASI.
# Actually, the easiest is to just check if the platform is wasm, and optionally use `web_time` or just conditional compile it to skip timings if not supported.

patch_content = """
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
fn get_time() -> Option<Instant> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn get_time() -> Option<()> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn get_elapsed(start: Option<Instant>) -> std::time::Duration {
    start.map(|s| s.elapsed()).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn get_elapsed(_start: Option<()>) -> std::time::Duration {
    std::time::Duration::default()
}
"""

if 'use std::time::Instant;' in content:
    content = content.replace('use std::time::Instant;', patch_content)

content = content.replace('let t_ingest_start = Instant::now();', 'let t_ingest_start = get_time();')
content = content.replace('d.phase_times.ingest_and_node = t_ingest_start.elapsed();', 'd.phase_times.ingest_and_node = get_elapsed(t_ingest_start);')

content = content.replace('let t_graph_build_start = Instant::now();', 'let t_graph_build_start = get_time();')
content = content.replace('d.phase_times.graph_build = t_graph_build_start.elapsed();', 'd.phase_times.graph_build = get_elapsed(t_graph_build_start);')

content = content.replace('let t_ring_extraction_start = Instant::now();', 'let t_ring_extraction_start = get_time();')
content = content.replace('d.phase_times.ring_extraction = t_ring_extraction_start.elapsed();', 'd.phase_times.ring_extraction = get_elapsed(t_ring_extraction_start);')

content = content.replace('let t_containment_start = Instant::now();', 'let t_containment_start = get_time();')
content = content.replace('d.phase_times.containment = t_containment_start.elapsed();', 'd.phase_times.containment = get_elapsed(t_containment_start);')


with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
