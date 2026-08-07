# Changelog - refactor_tool

All notable changes to `refactor_tool` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.net/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.1.4] - 2026-08-07

### Added
- Multi-identifier `impl` item matching: `distribute_items` now extracts self-type names, trait names, and contained associated type/const/macro names alongside method names, preventing empty `impl` blocks or `impl` blocks matched by type/trait name from being silently left in main.
- Extended `item_name` to extract names for `Mod` and `ExternCrate` items.
- Support for extracting self-type identifiers from reference, paren, group, slice, array, and pointer types.

### Changed
- Standardized `Cargo.toml` authors to `["Santh <64453045+santhreal@users.noreply.github.com>"]`.
- Cleaned up whitespace formatting in `append_to_file` to maintain consistent double-newline separation.

### Fixed
- Pruned completed rows in `BACKLOG.md`.
## [0.1.3] - 2026-08-07

### Added
- Extended `item_name` support to cover `Static`, `Trait`, `TraitAlias`, `Union`, and named `Macro` items.
- Strict Rust keyword validation for group filenames via `syn::parse_str::<syn::Ident>` to prevent generating invalid module declarations.
- `SPEC.md` technical specification and `package.metadata.santh.status = "alpha"`.

### Fixed
- `is_rust_ident` now rejects Rust keywords (`fn`, `struct`, `mod`, etc.) that are syntactically invalid as module identifiers.

## [0.1.2] - 2026-07-14

### Added
- Property test suite (`tests/property/mod.rs`) verifying item conservation, determinism, and unmatched group handling.

### Fixed
- `append_to_file` error handling now distinguishes `NotFound` (seeding a new file) from unreadable/non-UTF8 IO errors, preventing silent data loss.
- Path traversal rejection: group filenames with invalid path characters or relative path components are rejected before any file or AST mutation.
- Mod-declaration ordering: injected `pub mod` statements are now sorted deterministically.

## [0.1.1] - 2026-07-14

### Fixed
- Impl block distribution: `distribute_items` matches `impl` blocks by contained method names so methods move alongside their group module.
- AST-based declaration injection: replaced string manipulation of generated source with AST-based `pub mod` and `pub use` insertion.
- Target path validation: returns clean errors for missing parents or empty stems instead of panicking.

## [0.1.0] - 2026-07-01

### Added
- Initial release of `refactor_tool` CLI and library for splitting large Rust modules in `surgec`.
