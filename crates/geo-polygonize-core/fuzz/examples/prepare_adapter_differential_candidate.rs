use geo_polygonize_core_fuzz::{prepare_adapter_differential_candidate, CandidatePreparation};
use std::{env, fs, fs::OpenOptions, io::Write, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: prepare_adapter_differential_candidate ARTIFACT OUTPUT");
        return ExitCode::FAILURE;
    }
    let data = match fs::read(&args[0]) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("{}: {error}", args[0].to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    let prepared = match prepare_adapter_differential_candidate(&data) {
        Ok(CandidatePreparation::Candidate(prepared)) => prepared,
        Ok(CandidatePreparation::Ignored) => {
            eprintln!("artifact is not a decodable adapter_differential input");
            return ExitCode::FAILURE;
        }
        Ok(CandidatePreparation::Matched) => {
            eprintln!("artifact does not reproduce an adapter mismatch");
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let json = prepared.candidate.to_pretty_json().unwrap();
    let mut output = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("{}: {error}", args[1].to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = writeln!(output, "{json}") {
        eprintln!("{}: {error}", args[1].to_string_lossy());
        return ExitCode::FAILURE;
    }
    println!(
        "{}: prepared {} -> {} lines for review; not admitted",
        args[1].to_string_lossy(),
        prepared.original_line_count,
        prepared.minimized_line_count
    );
    ExitCode::SUCCESS
}
