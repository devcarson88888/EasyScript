use std::env;
use std::fs;
use std::process::ExitCode;

use easyscript::{lex, parse_tokens};
use easyscript_interpreter::execute;

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let program_name = arguments
        .next()
        .unwrap_or_else(|| "easyscript-interpreter".into());
    let Some(source_path) = arguments.next() else {
        eprintln!(
            "Usage: {} <source.es>",
            std::path::Path::new(&program_name)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("easyscript-interpreter")
        );
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("Expected exactly one EasyScript source file path.");
        return ExitCode::from(2);
    }

    let source = match fs::read_to_string(&source_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Could not read {}: {error}", source_path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };

    let tokens = match lex(&source) {
        Ok(tokens) => tokens,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let program = match parse_tokens(tokens) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let result = match execute(&program) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    for output in result.output {
        print!("{output}");
    }
    ExitCode::SUCCESS
}
