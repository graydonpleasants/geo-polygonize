use geo_polygonize_core_fuzz::{replay_adapter_differential, ReplayOutcome};
use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: replay_adapter_differential ARTIFACT...");
        return ExitCode::FAILURE;
    }

    for path in paths {
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(error) => {
                eprintln!("{}: {error}", path.to_string_lossy());
                return ExitCode::FAILURE;
            }
        };
        match replay_adapter_differential(&data) {
            Ok(ReplayOutcome::Matched) => println!("{}: matched", path.to_string_lossy()),
            Ok(ReplayOutcome::Ignored) => println!("{}: ignored", path.to_string_lossy()),
            Err(mismatch) => {
                eprintln!("{}: {mismatch}", path.to_string_lossy());
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
