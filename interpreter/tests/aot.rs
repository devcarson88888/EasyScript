use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_directory() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "easyscript-aot-test-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create isolated test directory");
    path
}

#[test]
fn build_emits_runnable_elf_with_rules_write_and_output_semantics() {
    let directory = temporary_directory();
    let source_path = directory.join("hello.es");
    fs::write(
        &source_path,
        r#"active system linux;
from library import std;
def println(a=std) {
    rules! write -> a
    rules! output -> a
}
int main() {
    value = int -> 1 + 2;
    if (value == 3) {
        println("A+B");
    } else {
        println("unexpected");
    }
    return code -> 0;
}
stop;
"#,
    )
    .expect("write source");

    let debug = Command::new(env!("CARGO_BIN_EXE_easyscript"))
        .arg("debug")
        .arg(&source_path)
        .output()
        .expect("run easyscript debug");
    assert!(debug.status.success());
    assert_eq!(String::from_utf8_lossy(&debug.stdout), "A+B\n");

    let build = Command::new(env!("CARGO_BIN_EXE_easyscript"))
        .arg("build")
        .arg(&source_path)
        .current_dir(&directory)
        .output()
        .expect("run easyscript build");
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&build.stdout),
        "Build success, output: ./hello\n"
    );

    let executable = directory.join("hello");
    assert!(executable.is_file());
    let executable_bytes = fs::read(&executable).expect("read generated ELF");
    assert!(executable_bytes.starts_with(b"\x7fELF"));
    assert_eq!(&executable_bytes[18..20], &[0x3e, 0x00]);
    let run = Command::new(&executable)
        .output()
        .expect("run generated ELF");
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout), "A+B\n");

    fs::remove_dir_all(directory).expect("clean isolated test directory");
}

#[test]
fn debug_and_build_report_the_same_parser_error() {
    let directory = temporary_directory();
    let source_path = directory.join("invalid.es");
    fs::write(&source_path, "int main( {\n").expect("write invalid source");
    let binary = env!("CARGO_BIN_EXE_easyscript");

    let debug = Command::new(binary)
        .arg("debug")
        .arg(&source_path)
        .output()
        .expect("run debug");
    let build = Command::new(binary)
        .arg("build")
        .arg(&source_path)
        .output()
        .expect("run build");
    assert!(!debug.status.success());
    assert!(!build.status.success());
    assert_eq!(debug.stderr, build.stderr);

    fs::remove_dir_all(directory).expect("clean isolated test directory");
}

#[test]
fn build_distinguishes_rustc_failures_from_easyscript_errors() {
    let directory = temporary_directory();
    let source_path = directory.join("hello.es");
    let executable = directory.join("hello");
    fs::write(&source_path, "int main() { return code -> 0; }\n").expect("write source");
    fs::write(&executable, "preserve existing output").expect("write sentinel output");

    let build = Command::new(env!("CARGO_BIN_EXE_easyscript"))
        .arg("build")
        .arg(&source_path)
        .env("PATH", &directory)
        .current_dir(&directory)
        .output()
        .expect("run easyscript build without rustc in PATH");
    assert!(!build.status.success());
    assert!(String::from_utf8_lossy(&build.stderr).starts_with("Rust compiler error:"));
    assert_eq!(
        fs::read_to_string(&executable).expect("existing output remains"),
        "preserve existing output"
    );

    fs::remove_dir_all(directory).expect("clean isolated test directory");
}
