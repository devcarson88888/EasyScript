# EasyScript language notes

EasyScript source files use the `.es` extension. Programs are parsed with the
shared lexer/parser crates and run through the `easyscript` CLI.

## Program structure

The top level may contain platform declarations, library imports, `def`
functions, type declarations, and the required `int main()` entry point.
Runtime statements belong inside `main` or another function. An optional
`stop;` marks the end of the source. Statement semicolons are optional when
line breaks make the statement boundary clear.

```es
active system linux;
from library import std;

def println(a=std) {
    rules! std::write -> a
    rules! std::output -> a
}

int main() {
    println("Hello, EasyScript!");
    return code -> 0;
}
stop;
```

Import the standard library with `from library import std;`. Prefer the
qualified forms `rules! std::write -> value`, `rules! std::output -> a`, and
`rules! std::input -> name`; direct member calls are written
`std.write(value)` and `std.output()` in the current expression syntax.
`std.write` stores a
value in the shared output buffer without printing, while `std.output` prints
the current buffer. `println` is not part of `std`; define it as an EasyScript
`def` function that calls `std.write` followed by `std.output`. The shorter
`write(value)` and `output()` aliases are supported, but
using them without the `std.` prefix is not recommended; the prefix makes the
source of these functions explicit.

The standard library is a separate Rust `cdylib` crate in `stdlib/`. Build it
independently, then package the platform libraries using the in-process
libarchive tool:

```sh
cargo build --manifest-path stdlib/Cargo.toml --release
cargo run --manifest-path tools/std-package/Cargo.toml -- \
  stdlib/target/release/libes_std.so path/to/es_std.dll \
  examples/library/std.ess
```

The `.ess` is an uncompressed tar archive whose root contains exactly
`libes_std.so` and `es_std.dll`, with no subdirectories. The interpreter extracts
only the current platform library and caches it; it does not invoke an external
tar command. The host must provide a loadable libarchive library (on Windows,
`archive.dll`). The interpreter and AOT-generated program do not contain the IO
implementation.

The standard library is declared in the UTF-8 header file
`library/std.esh`, located in a `library` directory beside the `.es` script:

```es
// ===== EasyScript STD Standard Library ===== //
// Namespace : std
namespace std {
    extern std write -> es_std_write
    extern std input -> es_std_input
    extern std output -> es_std_output
}
```

These are symbol declarations only, with no implementations. EasyScript does
not create the directory or files automatically: a missing directory reports
`missing library directory`, and a missing header reports
`library std header file (std.esh) not found`. `examples/hello.es` uses the
header at `examples/library/std.esh`; other scripts need a sibling `library/`
containing `std.esh` and `std.ess`.

`rules! std::write -> value` and `rules! std::output -> a` are syntax sugar for calls to
`std.write(value)` and `std.output(a)`. Use write and output as a pair; calling
output before writing a value leaves the buffer uninitialized and reports a
runtime error.

`rules! std::input -> name` blocks until one line is entered in the terminal,
stores that line as a string in `name`, and returns it from the containing
function. This supports a small wrapper whose output parameter is omitted at
the call site:

```es
def input(d) {
    rules! std::input -> d
}

int main() {
    d = auto -> input()
}
```

The trailing line ending is removed from the returned string.

## Commands

From the repository root:

```sh
cargo run --manifest-path interpreter/Cargo.toml -- debug examples/hello.es
cargo run --release --manifest-path interpreter/Cargo.toml -- build examples/hello.es
./hello
```

The `build` command generates Rust source in memory and invokes `rustc` for a
Linux x86-64 ELF. A working `rustc` installation and that target's standard
library are required.

## Crate responsibilities

- `lexer/`: tokenization and source locations.
- `parser/`: AST definitions, parsing, and mutability validation.
- `interpreter/`: runtime library, debug CLI, and integration tests.
- `compiler/`: Rust code generation and `rustc` invocation for AOT builds.
- `examples/`: runnable `.es` samples.
- `docs/`: language and development documentation.

`examples/bugs/missing-std-import.es` is an intentional debug-mode failure
example showing that `rules!` IO requires `from library import std;`.

Run crate tests with `cargo test --manifest-path <crate>/Cargo.toml` for
`lexer`, `parser`, and `interpreter`.
