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

`rules! write -> value` stores the value in variable `a` without printing.
`rules! output -> a` evaluates and prints `a`. Both require importing `std`.

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
