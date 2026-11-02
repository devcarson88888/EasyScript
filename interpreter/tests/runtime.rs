use easyscript_interpreter::{run_source, run_source_with_std_library, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

mod common;

fn isolated_std_library() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "easyscript-runtime-std-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create isolated std directory");
    common::build_std_library(&directory)
}

fn run_with_std_binary(
    source: &str,
) -> Result<easyscript_interpreter::RunResult, easyscript_interpreter::RunError> {
    let library_path = isolated_std_library();
    let result = run_source_with_std_library(source, &library_path);
    fs::remove_dir_all(library_path.parent().expect("std library parent"))
        .expect("clean isolated std directory");
    result
}

fn run_with_std(
    source: &str,
) -> Result<easyscript_interpreter::RunResult, easyscript_interpreter::RunError> {
    let prelude = r#"
from library import std;
def println(a=std) {
    rules! write -> a;
    rules! output -> a;
}
"#;
    let source = if let Some(stop_index) = source.rfind("stop;") {
        format!(
            "{}{}{}",
            &source[..stop_index],
            prelude,
            &source[stop_index..]
        )
    } else {
        format!("{source}{prelude}")
    };
    run_with_std_binary(&source)
}

#[test]
fn evaluates_arithmetic_conditionals_and_while_loops_with_dynamic_variables() {
    let result = run_with_std(
        r#"int main() {
    value = int -> 0;
    while (value < 3) {
        value = int -> value + 1;
        println(value);
    }
    value = auto -> "finished";
    if (value == "finished") {
        println(value);
    } else {
        println("unexpected");
    }
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["1\n", "2\n", "3\n", "finished\n"]);
    assert!(matches!(result.value, Some(Value::Null)));
}

#[test]
fn calls_functions_lambdas_and_builtin_functions() {
    let result = run_with_std(
        r#"def add(a=int, b=int) {
    return code -> a + b
}
int main() {
    twice = auto -> lambda(x=auto) -> x * 2;
    println(add(2, 3));
    println(twice(4));
    println(len("abcd"));
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["5\n", "8\n", "4\n"]);
}

#[test]
fn handles_lists_tuples_dictionaries_comprehensions_and_slices() {
    let result = run_with_std(
        r#"int main() {
    values = auto -> [1, 2, 3];
    values.append(4);
    values[0] = int -> 10;
    println(values[-1]);
    println(values[::-1]);
    pair = auto -> (5, 6);
    println(pair[-1]);
    record = auto -> {"name": "easy", "version": 1};
    record["version"] = int -> 2;
    println(record["name"]);
    println(len(record));
    doubled = auto -> [x * 2 for x in range(4) if x > 1];
    println(doubled);
}
"#,
    )
    .unwrap();
    assert_eq!(
        result.output,
        ["4\n", "[4, 3, 2, 10]\n", "6\n", "easy\n", "2\n", "[4, 6]\n"]
    );
}

#[test]
fn iterates_ranges_lists_strings_and_supports_break_continue() {
    let result = run_with_std(
        r#"int main() {
    total = int -> 0;
    for value in range(1, 6) {
        if (value == 2) {
            continue;
        }
        if (value == 5) {
            break;
        }
        total = int -> total + value;
    }
    println(total);
    for ch in "ab" {
        println(ch);
    }
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["8\n", "a\n", "b\n"]);
}

#[test]
fn constructs_and_mutates_structs_and_class_instances() {
    let result = run_with_std(
        r#"struct Point {
    x: i32;
    y: i32;
}
class Counter {
    def init(start=int) {
        self.value = auto -> start;
    }
    def increment() {
        self.value = int -> self.value + 1;
        return code -> self.value;
    }
}
int main() {
    point = auto -> Point(3, 4);
    println(point.x);
    point.x = int -> 7;
    println(point.x);
    counter = auto -> Counter(5);
    println(counter.increment());
    println(counter.value);
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["3\n", "7\n", "6\n", "6\n"]);
}

#[test]
fn registers_all_functions_before_calling_main_and_does_not_run_def_bodies_early() {
    let result = run_with_std(
        r#"int main() {
    println(later());
}
def later() {
    println("called");
    return code -> 42;
}
stop;
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["called\n", "42\n"]);
}

#[test]
fn executes_rules_write_and_output_commands() {
    let result = run_with_std(
        r#"int main() {
    rules! write -> "hello";
    rules! output -> a;
    rules! write -> 42;
    rules! output -> a;
}
stop;
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["hello\n", "42\n"]);
}

#[test]
fn std_namespace_calls_require_an_explicit_import() {
    let error = run_source(
        r#"int main() {
    std.write("must not be implicitly available");
}
"#,
    )
    .unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert_eq!(error.message, "namespace \"std\" is not imported");
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }

    let error = run_source(
        r#"int main() {
    rules! std::write -> "must not be implicitly available"
}
"#,
    )
    .unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert_eq!(error.message, "namespace \"std\" is not imported");
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }
}

#[test]
fn rules_write_stores_without_printing_until_output_reads_a() {
    let result = run_with_std_binary(
        r#"from library import std
int main() {
    rules! write -> "stored only"
}
"#,
    )
    .unwrap();
    assert!(result.output.is_empty());
}

#[test]
fn std_io_functions_and_compatible_short_aliases_use_the_shared_buffer() {
    let result = run_with_std_binary(
        r#"from library import std;
def println(value=auto) {
    std.write(value);
    std.output();
}
int main() {
    std.write("buffered");
    std.output();
    std.write(42);
    std.output(42);
    write("short alias");
    output();
    println("line");
}
"#,
    )
    .unwrap();
    assert_eq!(
        result.output,
        ["buffered\n", "42\n", "short alias\n", "line\n"]
    );
}

#[test]
fn std_output_before_write_reports_the_uninitialized_buffer() {
    let error = run_with_std_binary(
        r#"from library import std;
int main() {
    std.output();
}
"#,
    )
    .unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert!(error.message.contains("before std.write()"));
            assert_eq!(error.line, 3);
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }
}

#[test]
fn std_input_rules_read_one_line_and_input_wrapper_returns_it() {
    let source = r#"active system linux
from library import std
def input(d) {
    rules! std::input -> d
}
def println(a) {
    rules! std::write -> a
    rules! std::output -> a
}
int main() {
    d = auto -> input()
    println("Hi" + d)
}
stop
"#;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "easyscript-input-test-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create isolated test directory");
    common::build_std_library(&directory);
    let source_path = directory.join("input.es");
    fs::write(&source_path, source).expect("write script");
    let mut child = Command::new(env!("CARGO_BIN_EXE_easyscript"))
        .arg("debug")
        .arg(&source_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start easyscript process");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"Ada Lovelace\r\n")
        .expect("write input line");

    let output = child.wait_with_output().expect("wait for interpreter");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "HiAda Lovelace\n");
    fs::remove_dir_all(directory).expect("clean isolated test directory");
}

#[test]
fn std_short_aliases_do_not_replace_user_defined_functions() {
    let result = run_with_std_binary(
        r#"def write(value=auto) {
    return code -> value;
}
from library import std;
def println(value=auto) {
    std.write(value);
    std.output();
}
int main() {
    println(write("custom write"));
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["custom write\n"]);
}

#[test]
fn println_is_not_a_std_member_function() {
    let error = run_with_std_binary(
        r#"from library import std;
int main() {
    std.println("not part of std");
}
"#,
    )
    .unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert!(error
                .message
                .contains("std library has no function `println`"));
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }
}

#[test]
fn runs_the_documented_hello_world_with_either_terminator_style() {
    let semicolon_source = r#"active system linux;
from library import std;
def println(a=std) {
    rules! write -> a;
    rules! output -> a;
}
int main() {
    println("Hello World!\n");
    return code -> 0;
}
stop;
"#;
    let newline_source = r#"active system linux
from library import std
def println(a=std) {
    rules! write -> a
    rules! output -> a
}
int main() {
    println("Hello World!\n")
    return code -> 0
}
stop
"#;

    for source in [semicolon_source, newline_source] {
        let result = run_with_std_binary(source).unwrap();
        assert_eq!(result.output, ["Hello World!\n\n"]);
        assert!(matches!(result.value, Some(Value::Number(0))));
    }
}

#[test]
fn std_io_requires_an_explicit_import_and_println_is_not_builtin() {
    let error = run_source("int main() {\n    rules! output -> 1;\n}\n").unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert!(error.message.contains("namespace \"std\" is not imported"));
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }

    let error = run_source("int main() {\n    println(\"not implicit\");\n}\n").unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert!(error.message.contains("undefined name `println`"));
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }
}

#[test]
fn constructs_enum_unit_tuple_and_named_payload_variants() {
    let result = run_with_std(
        r#"enum Color {
    Red,
    Green(u8),
    Blue { r: u8, g: u8, b: u8 }
}
int main() {
    red = auto -> Color.Red;
    green = auto -> Color.Green(5);
    blue = auto -> Color.Blue({"r": 1, "g": 2, "b": 3});
    println(red.variant);
    println(green[0]);
    println(blue.g);
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["Red\n", "5\n", "2\n"]);
}

#[test]
fn rejects_executable_and_unlisted_declarations_at_top_level() {
    let error = run_source("println(\"outside main\");\nint main() {}\n").unwrap_err();
    assert!(matches!(error, easyscript_interpreter::RunError::Parse(_)));

    let error = run_source("extern \"C\" fn host_call() -> int;\nint main() {}\n").unwrap_err();
    assert!(matches!(error, easyscript_interpreter::RunError::Parse(_)));
}

#[test]
fn executes_try_handlers_and_finally_and_reports_runtime_locations() {
    let result = run_with_std(
        r#"int main() {
    try {
        value = int -> 1 / 0;
    } except ZeroDivisionError {
        println("caught");
    } finally {
        println("finally");
    }
}
"#,
    )
    .unwrap();
    assert_eq!(result.output, ["caught\n", "finally\n"]);

    let error = run_source("int main() {\n    value = int -> 1 / 0;\n}\n").unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert_eq!((error.line, error.column), (2, 22));
            assert!(error.message.contains("division by zero"));
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }
}

#[test]
fn rejects_runtime_type_errors_and_out_of_range_indexes_with_source_positions() {
    let error = run_source("int main() {\n    value = int -> \"text\" - 1;\n}\n").unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert_eq!((error.line, error.column), (2, 27));
            assert!(error.message.contains("require numbers"));
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }

    let error =
        run_with_std("int main() {\n    values = auto -> [1];\n    println(values[3]);\n}\n")
            .unwrap_err();
    match error {
        easyscript_interpreter::RunError::Runtime(error) => {
            assert_eq!(error.line, 3);
            assert!(error.message.contains("out of range"));
        }
        easyscript_interpreter::RunError::Parse(error) => panic!("unexpected parse error: {error}"),
    }
}
