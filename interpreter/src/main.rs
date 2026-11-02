use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use easyscript_compiler::compile_to_elf_with_std_library;
use easyscript_interpreter::{execute_with_std_library, library::validate_library_imports};
use easyscript_parser::{lex, parse_tokens, Program};

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let program_name = arguments.next().unwrap_or_else(|| "easyscript".into());
    let display_name = std::path::Path::new(&program_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("easyscript");
    let Some(source_path) = arguments.next() else {
        eprintln!("Usage: {display_name} <debug|build> <source.es>");
        return ExitCode::from(2);
    };
    if source_path != OsStr::new("debug") {
        let command = if source_path == OsStr::new("build") {
            "build"
        } else {
            eprintln!("Unknown command: {}", source_path.to_string_lossy());
            eprintln!("Usage: {display_name} debug <source.es>");
            eprintln!("       {display_name} build <source.es>");
            return ExitCode::from(2);
        };
        let Some(source_path) = arguments.next() else {
            eprintln!("Usage: {display_name} {command} <source.es>");
            return ExitCode::from(2);
        };
        if arguments.next().is_some() {
            eprintln!("Expected exactly one source file path after `{command}`.");
            eprintln!("Usage: {display_name} {command} <source.es>");
            return ExitCode::from(2);
        }
        return match read_source(&source_path) {
            Ok(source) => match parse_source(&source) {
                Ok(program) => {
                    let std_library_path =
                        match validate_library_imports(&program, Path::new(&source_path)) {
                            Ok(path) => path,
                            Err(error) => {
                                eprintln!("{error}");
                                return ExitCode::FAILURE;
                            }
                        };
                    let Some(output_name) = Path::new(&source_path).file_stem() else {
                        eprintln!("Build error: source path has no file name.");
                        return ExitCode::FAILURE;
                    };
                    let output_path = Path::new(".").join(output_name);
                    match compile_to_elf_with_std_library(
                        &program,
                        &output_path,
                        std_library_path.as_deref(),
                    ) {
                        Ok(()) => {
                            println!("Build success, output: ./{}", output_name.to_string_lossy());
                            ExitCode::SUCCESS
                        }
                        Err(error) => {
                            eprintln!("{error}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }

    let Some(source_path) = arguments.next() else {
        eprintln!("Usage: {display_name} debug <source.es>");
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("Expected exactly one source file path after `debug`.");
        eprintln!("Usage: {display_name} debug <source.es>");
        return ExitCode::from(2);
    }

    let source = match read_source(&source_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let program = match parse_source(&source) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let std_library_path = match validate_library_imports(&program, Path::new(&source_path)) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let result = match execute_with_std_library(&program, std_library_path.as_deref()) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    for (output, already_written) in result.output.into_iter().zip(result.output_already_written) {
        if !already_written {
            print!("{output}");
        }
    }
    ExitCode::SUCCESS
}

fn read_source(source_path: &OsStr) -> Result<String, String> {
    fs::read_to_string(source_path)
        .map_err(|error| format!("Could not read {}: {error}", source_path.to_string_lossy()))
}

fn parse_source(source: &str) -> Result<Program, String> {
    let tokens = lex(source).map_err(|error| error.to_string())?;
    parse_tokens(tokens).map_err(|error| error.to_string())
}
