use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_std_library(directory: &Path) -> PathBuf {
    let library_directory = directory.join("library");
    fs::create_dir_all(&library_directory).expect("create test library directory");
    fs::write(
        library_directory.join("std.esh"),
        include_str!("../../../library/std.esh"),
    )
    .expect("install std declaration");
    let package_path = library_directory.join("std.ess");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stdlib/src/lib.rs");
    let native_filename = if cfg!(windows) {
        "es_std.dll"
    } else {
        "libes_std.so"
    };
    let native_path = library_directory.join(native_filename);
    let output = Command::new("rustc")
        .arg("--crate-name")
        .arg("es_std")
        .arg("--crate-type")
        .arg("cdylib")
        .arg("--edition=2021")
        .arg("-o")
        .arg(&native_path)
        .arg(source)
        .output()
        .expect("compile standalone std test library");
    assert!(
        output.status.success(),
        "std library compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let archive_path = library_directory.join("std.ess.tmp");
    let native_library = fs::read(&native_path).expect("read native std library");
    let (so, dll) = if cfg!(windows) {
        (
            b"other platform binary".as_slice(),
            native_library.as_slice(),
        )
    } else {
        (
            native_library.as_slice(),
            b"other platform binary".as_slice(),
        )
    };
    easyscript_archive::write_tar(&archive_path, &[("libes_std.so", so), ("es_std.dll", dll)])
        .expect("write std tar package");
    fs::rename(&archive_path, &package_path).expect("install std package");
    native_path
}
