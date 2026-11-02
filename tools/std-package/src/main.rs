use easyscript_archive::write_tar;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match package() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("package-std: {error}");
            ExitCode::FAILURE
        }
    }
}

fn package() -> Result<(), String> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: package-std <libes_std.so> <es_std.dll> <std.ess>".to_owned());
    }

    let so = fs::read(&args[1])
        .map_err(|error| format!("could not read {}: {error}", Path::new(&args[1]).display()))?;
    let dll = fs::read(&args[2])
        .map_err(|error| format!("could not read {}: {error}", Path::new(&args[2]).display()))?;
    let output = Path::new(&args[3]);
    let temporary_output = output.with_extension(format!("ess.tmp-{}", std::process::id()));
    write_tar(
        &temporary_output,
        &[("libes_std.so", &so), ("es_std.dll", &dll)],
    )?;
    fs::rename(&temporary_output, output)
        .map_err(|error| format!("could not install {}: {error}", output.display()))?;
    Ok(())
}
