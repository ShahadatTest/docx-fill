# Contributing

Put one change in each pull request. Without write access, open it from a fork.

## Checks

CI runs these on every pull request:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --target wasm32-unknown-unknown
```

The wasm build needs `rustup target add wasm32-unknown-unknown` once.

CI does not check formatting. The code doesn't follow rustfmt's defaults, so don't run `cargo fmt`: it rewrites hundreds of unrelated lines. Match the surrounding code.

## Sources

Build behaviour from the WordprocessingML spec (ECMA-376), docxtemplater's public tag syntax and documentation, and black-box runs of the MIT-licensed docxtemplater core. Extension tags follow the paid modules' public documentation.

Don't copy code from docxtemplater, and don't tune anything against the paid modules' output. Name your source in the pull request.

## Adding a feature

- Keep `serde_json` as the only dependency, so the crate keeps building for wasm.
- A new tag kind gets a flag in `Extensions` and a key in `read_extensions` in `src/json.rs`.
- A new option goes in `FillOptions::read_json`, which refuses unknown keys, and in `reads_every_option` in `tests/json.rs`.
- Tests go in `tests/<feature>.rs`. `tests/common/mod.rs` has helpers for building and reading document XML.
- Add the tag or option to the README.
