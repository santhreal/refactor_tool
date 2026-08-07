# Technical Specification: refactor_tool

`refactor_tool` is an internal developer CLI and library that splits large Rust source modules into logically grouped sibling module files based on item identifiers.

## Architecture

- **`src/lib.rs`**: Core library API (`parse_rust_file`, `item_name`, `distribute_items`).
- **`src/main.rs`**: CLI wrapper providing pre-configured grouping rules for `surgec` refactoring passes (`decode`, `lower`, `interp`, `naga`).

## Core Invariants

1. **Item Conservation**: Every item in the source module must be either moved into exactly one target group module file or retained in the main module file. No item is dropped or duplicated.
2. **Determinism**: For identical inputs and group configurations, output module declarations and files are byte-identical across runs. Module names in injected `pub mod` statements are sorted alphabetically.
3. **Idempotence**: Running `distribute_items` repeatedly on the same source produces identical output without duplicating `pub mod` or `pub use` statements.
4. **Path & Identifier Safety**: Group filenames must be valid Rust identifiers (excluding raw identifiers and keywords). Any attempt to pass path traversal sequences (e.g. `../escape`) or invalid module names is rejected up front with `ErrorKind::InvalidInput`.
5. **Atomic-Like File Mutation**: Sibling module files are created/appended first; the main target file is rewritten last. If a module write fails, the main target file remains unchanged.

## Public Library API

### `parse_rust_file(content: &str) -> syn::Result<syn::File>`
Parses Rust source code into a `syn::File` AST representation.

### `item_name(item: &syn::Item) -> Option<String>`
Extracts the top-level identifier for `Fn`, `Struct`, `Enum`, `Const`, `Static`, `Type`, `Trait`, `TraitAlias`, `Union`, `Macro`, and `Impl` self-type.

### `distribute_items(ast: &mut syn::File, target_path: &Path, groups: &[(Vec<&str>, &str)]) -> std::io::Result<()>`
Splits matching items from `ast` into sibling module files relative to `target_path`. Injects sorted `pub mod` and `pub use` statements into `ast` for new modules.

## Command Line Interface

```bash
refactor_tool <target_file.rs> <mode>
```

Supported modes:
- `decode`: Base64, hex, unicode, html, octal, and archive decoding helpers.
- `lower`: Lowering passes (`call_ops`, `literal_ops`, `collection_ops`).
- `interp`: Interpreter state machine (`memory`, `state`, `step`, `subgroup`, `sync`).
- `naga`: WGSL code emission shards (`arithmetic`, `atomic`, `subgroup`).

## Maturity & Limitations

- **Maturity**: Alpha / Internal Utility (`package.metadata.santh.status = "internal-utility"`).
- **Scope**: Designed for splitting top-level items in trusted internal codebases.
- **Dependencies**: Relies on `syn` (full feature set) and `quote`.
