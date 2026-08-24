mod cli;
mod cli_commands;
mod cli_protocol;
mod cli_query;
mod cli_sync;
mod cli_sync_state;
mod cli_update;
mod cli_vectors;
mod repository_state;

#[cfg(test)]
mod cli_sync_tests;
#[cfg(test)]
mod cli_tests;
#[cfg(test)]
mod cli_update_tests;

use std::env;
use std::fs::{self, File};
use std::process::ExitCode;

use arcana::benchmark::{
    BenchmarkCommand, BenchmarkConfig, benchmark_usage, run_mutation_benchmark,
};
use arcana::{PROJECT_NAME, PROJECT_VERSION, about};

fn main() -> ExitCode {
    match cli::parse(env::args().skip(1)) {
        Ok(cli::Command::Help) => {
            println!("{PROJECT_NAME} — {}", about());
            println!("{}", cli::USAGE);
            ExitCode::SUCCESS
        }
        Ok(cli::Command::Version) => {
            println!("{PROJECT_NAME} {PROJECT_VERSION}");
            ExitCode::SUCCESS
        }
        Ok(cli::Command::Benchmark(arguments)) => run_benchmark_command(arguments),
        Ok(cli::Command::ImportFacts(command)) => match cli_commands::run_import_facts(&command) {
            Ok(summary) => {
                print!("{summary}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("arcana import-facts: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(cli::Command::Sync(command)) => match cli_sync::run_sync(&command) {
            Ok(summary) => {
                print!("{summary}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("arcana sync: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(cli::Command::UpdateFacts(command)) => match cli_update::run_update_facts(&command) {
            Ok(summary) => {
                print!("{summary}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("arcana update-facts: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(cli::Command::Query(command)) => match cli_query::run_query(&command) {
            Ok(output) => {
                print!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("arcana query: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(cli::Command::Vectorize(command)) => match cli_vectors::run_vectorize(&command) {
            Ok(summary) => {
                print!("{summary}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("arcana vectorize: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(cli::Command::SemanticQuery(command)) => match cli_vectors::run_semantic_query(&command)
        {
            Ok(output) => {
                print!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("arcana semantic-query: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(cli::Command::Protocol(command)) => match cli_protocol::run_protocol(&command) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("arcana protocol: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("arcana: {error}\n\n{}", cli::USAGE);
            ExitCode::from(2)
        }
    }
}

fn run_benchmark_command(arguments: Vec<String>) -> ExitCode {
    if matches!(arguments.as_slice(), [argument] if argument == "-h" || argument == "--help") {
        println!("{}", benchmark_usage());
        return ExitCode::SUCCESS;
    }

    let command = match BenchmarkCommand::parse(arguments.iter().map(String::as_str)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("arcana benchmark: {error}\n\n{}", benchmark_usage());
            return ExitCode::from(2);
        }
    };
    let config = BenchmarkConfig::new(
        command.graph,
        command.query_count,
        command.sample_count as usize,
        &command.work_dir,
        command.keep_files,
    );
    let report = match run_mutation_benchmark(&config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("arcana benchmark: {error}");
            return ExitCode::FAILURE;
        }
    };

    print!("{}", report.human_summary());
    if let Some(path) = command.csv_path {
        if let Err(error) = write_csv(&report, &path) {
            eprintln!("arcana benchmark: write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
        println!("raw samples: {}", path.display());
    }
    ExitCode::SUCCESS
}

fn write_csv(
    report: &arcana::benchmark::BenchmarkReport,
    path: &std::path::Path,
) -> Result<(), arcana::benchmark::BenchmarkError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    report.write_csv(File::create(path)?)
}
