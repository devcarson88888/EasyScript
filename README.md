# EasyScript

## Project layout

The source is divided into separate `lexer/`, `parser/`, `interpreter/`, and
`compiler/` crates. Runnable `.es` samples are in `examples/`; language notes
are in [`docs/language.md`](docs/language.md).

Run the interpreter against the Hello World example:

```sh
cargo run --manifest-path interpreter/Cargo.toml -- debug examples/hello.es
cargo run --release --manifest-path interpreter/Cargo.toml -- build examples/hello.es
./hello
```

Crate test commands and the language overview are documented in
[`docs/language.md`](docs/language.md).