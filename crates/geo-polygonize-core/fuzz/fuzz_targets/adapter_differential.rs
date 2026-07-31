#![no_main]

use geo_polygonize_core_fuzz::{replay_adapter_differential, ReplayOutcome};
use libfuzzer_sys::{fuzz_target, Corpus};

fuzz_target!(|data: &[u8]| -> Corpus {
    match replay_adapter_differential(data) {
        Ok(ReplayOutcome::Matched) => Corpus::Keep,
        Ok(ReplayOutcome::Ignored) => Corpus::Reject,
        Err(mismatch) => panic!("{mismatch}"),
    }
});
