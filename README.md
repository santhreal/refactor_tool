# refactor_tool

Internal CLI that splits large `surgec` / `vyre` Rust modules into focused submodules by item name.

## Usage

```bash
refactor_tool path/to/lib.rs decode   # encoding helpers → base64/, hex/, …
refactor_tool path/to/lib.rs lower    # lowering helpers
refactor_tool path/to/lib.rs interp   # interpreter state machine
refactor_tool path/to/lib.rs naga     # WGSL emit shards
```

## Library API

- `parse_rust_file`: infallible-hostile `syn::parse_file` wrapper for tests/fuzz.
- `item_name`: resolve top-level item name (`Fn`, `Struct`, `Enum`, `Const`, `Static`, `Type`, `Trait`, `TraitAlias`, `Union`, `Macro`, `Impl`) for grouping.
- `distribute_items`: write grouped items into `stem/{group}.rs` siblings.

See [SPEC.md](SPEC.md) for technical specification and invariants.

## Tests

```bash
cargo test -p refactor_tool --test adversarial_tests
```

## Fuzz

```bash
cd fuzz && cargo fuzz run fuzz_parse_rust -runs=1000
```
