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
    rules! write -> a
    rules! output -> a
}

int main() {
    println("Hello, EasyScript!");
    return code -> 0;
}
stop;
```

Import the standard library with `from library import std;`. Prefer the
explicit calls `std.write(value)` and `std.output()`: `std.write` stores a
value in the shared output buffer without printing, while `std.output` prints
the current buffer. `println` is not part of `std`; define it as an EasyScript
`def` function that calls `std.write` followed by `std.output`. The shorter
`write(value)` and `output()` aliases are supported, but
using them without the `std.` prefix is not recommended; the prefix makes the
source of these functions explicit.

`rules! write -> value` and `rules! output -> a` are syntax sugar for calls to
`std.write(value)` and `std.output(a)`. Use write and output as a pair; calling
output before writing a value leaves the buffer uninitialized and reports a
runtime error.

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
