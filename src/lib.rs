//! Pattern-based Rust AST splitting helpers used by surgec refactors.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use quote::ToTokens;
use syn::{Item, Type, UseTree};

/// Parse a Rust source file; returns `syn` diagnostics on failure.
pub fn parse_rust_file(content: &str) -> syn::Result<syn::File> {
    syn::parse_file(content)
}

/// Best-effort item name for module distribution.
#[must_use]
pub fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Fn(f) => Some(f.sig.ident.to_string()),
        Item::Struct(s) => Some(s.ident.to_string()),
        Item::Enum(e) => Some(e.ident.to_string()),
        Item::Const(c) => Some(c.ident.to_string()),
        Item::Static(s) => Some(s.ident.to_string()),
        Item::Type(t) => Some(t.ident.to_string()),
        Item::Trait(t) => Some(t.ident.to_string()),
        Item::TraitAlias(t) => Some(t.ident.to_string()),
        Item::Union(u) => Some(u.ident.to_string()),
        Item::Macro(m) => m.ident.as_ref().map(|id| id.to_string()),
        Item::Impl(i) => {
            if let Type::Path(p) = &*i.self_ty {
                p.path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}


/// Returns the identifiers of the methods/associated functions inside an
/// `impl` block, so `distribute_items` can move the whole block to the module
/// that owns one of its methods.
fn impl_method_names(item: &Item) -> Option<Vec<String>> {
    if let Item::Impl(i) = item {
        Some(
            i.items
                .iter()
                .filter_map(|it| match it {
                    syn::ImplItem::Fn(f) => Some(f.sig.ident.to_string()),
                    _ => None,
                })
                .collect(),
        )
    } else {
        None
    }
}

/// Returns true when `name` is a plain Rust identifier: a letter or
/// underscore followed by letters, digits, or underscores.
///
/// `distribute_items` turns a group filename into both a `pub mod` item and
/// an on-disk path, so anything that is not an identifier is rejected before
/// it can reach either.
fn is_rust_ident(name: &str) -> bool {
    !name.contains('#') && syn::parse_str::<syn::Ident>(name).is_ok()
}

/// Validates every group filename before any AST mutation or file write.
///
/// A filename becomes `<base_dir>/<filename>.rs` on disk, so a value like
/// `../../escape` would write outside the sibling module directory. It also
/// becomes `pub mod <filename>;`, so anything the identifier check accepts
/// still parses as a module declaration.
fn validate_group_filenames(groups: &[(Vec<&str>, &str)]) -> std::io::Result<()> {
    for (_, filename) in groups {
        if !is_rust_ident(filename) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "group filename `{filename}` is not a plain Rust identifier; it would produce an invalid module declaration or escape the module directory"
                ),
            ));
        }
    }
    Ok(())
}

/// True when the AST already declares `pub mod <filename>;`.
fn declares_module(items: &[Item], filename: &str) -> bool {
    items
        .iter()
        .any(|item| matches!(item, Item::Mod(m) if m.ident == filename))
}

/// True when the AST already re-exports `use <filename::*;` as a glob.
fn reexports_module(items: &[Item], filename: &str) -> bool {
    items.iter().any(|item| {
        matches!(item, Item::Use(u) if matches!(&u.tree,
            UseTree::Path(p) if p.ident == filename && matches!(&*p.tree, UseTree::Glob(_))))
    })
}

/// Split selected items from `ast` into sibling module files under `target_path`.
///
/// Items named in `groups` move to `<target_stem>/<group>.rs`; everything
/// else stays in the main file, which gains sorted `pub mod` / `pub use`
/// declarations for the groups that received items.
///
/// ```
/// let dir = tempfile::tempdir().unwrap();
/// let target = dir.path().join("main.rs");
/// std::fs::write(&target, "pub fn helper() {}\npub fn keep() {}\n").unwrap();
///
/// let mut ast = refactor_tool::parse_rust_file(
///     "pub fn helper() {}\npub fn keep() {}\n",
/// ).unwrap();
/// let groups: Vec<(Vec<&str>, &str)> = vec![(vec!["helper"], "helpers")];
/// refactor_tool::distribute_items(&mut ast, &target, &groups).unwrap();
///
/// let module = std::fs::read_to_string(dir.path().join("main").join("helpers.rs")).unwrap();
/// assert!(module.contains("helper"));
/// let main = std::fs::read_to_string(&target).unwrap();
/// assert!(main.contains("keep"));
/// ```
pub fn distribute_items(
    ast: &mut syn::File,
    target_path: &Path,
    groups: &[(Vec<&str>, &str)],
) -> std::io::Result<()> {
    let parent = target_path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "target_path `{}` has no parent directory; cannot create sibling modules",
                target_path.display()
            ),
        )
    })?;
    let stem = target_path
        .file_stem()
        .filter(|s| !s.is_empty())
        .zip(target_path.extension().filter(|e| *e == "rs"))
        .map(|(s, _)| s)
        .ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "target_path `{}` must be a `.rs` file with a non-empty stem; cannot name the sibling module directory",
                target_path.display()
            ),
        )
    })?;
    let base_dir = parent.join(stem);

    validate_group_filenames(groups)?;

    // BTreeMap keeps module file names in sorted order, so the injected
    // declarations and the module files are byte-identical across runs.
    let mut files: BTreeMap<&str, String> = BTreeMap::new();
    let mut retained_items = Vec::new();

    for item in std::mem::take(&mut ast.items) {
        let tokens = item.to_token_stream().to_string();
        let names: Vec<String> = impl_method_names(&item)
            .unwrap_or_else(|| item_name(&item).into_iter().collect());
        let mut moved = false;
        for n in names {
            for (items, filename) in groups {
                if items.contains(&n.as_str()) {
                    let buf: &mut String = files.entry(*filename).or_default();
                    buf.push_str(&tokens);
                    buf.push_str("\n\n");
                    moved = true;
                    break;
                }
            }
            if moved {
                break;
            }
        }
        if !moved {
            retained_items.push(item);
        }
    }

    ast.items = retained_items;

    // Inject the missing declarations into the AST. Each half is checked
    // separately on the parsed items: a source file that already has
    // `pub mod steps;` but no glob re-export gets only the re-export, never
    // a duplicate module declaration.
    let mut missing_mods = Vec::new();
    for filename in files.keys() {
        for needed in [
            (!declares_module(&ast.items, filename)).then(|| format!("pub mod {filename};")),
            (!reexports_module(&ast.items, filename)).then(|| format!("pub use {filename}::*;")),
        ]
        .into_iter()
        .flatten()
        {
            let parsed: syn::Item = syn::parse_str(&needed).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;
            missing_mods.push(parsed);
        }
    }
    let mut new_items = missing_mods;
    new_items.extend(std::mem::take(&mut ast.items));
    ast.items = new_items;

    let new_main = ast.to_token_stream().to_string();

    // Write the module files first and the main file last. If a module write
    // fails, the original main file still compiles; if the main write failed
    // first instead, the tree would declare modules that do not exist yet.
    fs::create_dir_all(&base_dir)?;
    for (filename, content) in &files {
        append_to_file(&base_dir.join(format!("{filename}.rs")), content)?;
    }
    fs::write(target_path, new_main)?;
    Ok(())
}

fn append_to_file(path: &Path, content: &str) -> std::io::Result<()> {
    if content.is_empty() {
        return Ok(());
    }
    let existing = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "use super::*;\n".to_string(),
        Err(e) => return Err(e),
    };
    fs::write(path, existing + "\n\n" + content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn append_to_file_creates_missing_file_with_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("new.rs");
        append_to_file(&path, "fn foo() {}").expect("append should succeed");
        let content = fs::read_to_string(&path).expect("read");
        assert!(content.starts_with("use super::*;"));
        assert!(content.contains("fn foo() {}"));
    }

    #[test]
    fn append_to_file_preserves_existing_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("existing.rs");
        fs::write(&path, "use super::*;\n\nfn old() {}").expect("setup");
        append_to_file(&path, "fn new() {}").expect("append should succeed");
        let content = fs::read_to_string(&path).expect("read");
        assert!(content.contains("fn old() {}"));
        assert!(content.contains("fn new() {}"));
    }

    #[test]
    fn append_to_file_propagates_permission_error() {
        // Simulates a file that cannot be read by making it non-UTF-8 content,
        // which `fs::read_to_string` rejects as an error rather than silently
        // falling back to a default.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("binary.rs");
        {
            let mut f = fs::File::create(&path).expect("create");
            f.write_all(&[0xff, 0xfe]).expect("write");
        }
        let result = append_to_file(&path, "fn x() {}");
        assert!(result.is_err(), "reading a non-UTF-8 file should fail, not silently overwrite");
    }


    #[test]
    fn distribute_items_moves_impl_block_by_method_name() {
        let mut ast = syn::parse_file(r#"
impl Foo {
    fn step() {}
    fn emit() {}
}
"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        distribute_items(&mut ast, &main, groups).expect("distribute");

        let main_content = fs::read_to_string(&main).unwrap();
        assert!(!main_content.contains("impl Foo"), "impl block should be moved out of main");
        assert!(main_content.contains("pub mod steps"), "mod decl should be injected");

        let steps_content = fs::read_to_string(dir.path().join("main/steps.rs")).unwrap();
        assert!(steps_content.contains("impl Foo"), "impl block should land in steps.rs");
        assert!(steps_content.contains("fn step"), "method should be in steps.rs");
    }

    #[test]
    fn distribute_items_retains_impl_without_matching_method() {
        let mut ast = syn::parse_file(r#"
impl Bar {
    fn keep() {}
}
"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        distribute_items(&mut ast, &main, groups).expect("distribute");

        let main_content = fs::read_to_string(&main).unwrap();
        assert!(main_content.contains("impl Bar"), "impl with no matching method should stay in main");
    }

    #[test]
    fn distribute_items_moves_named_items_and_retains_unmatched() {
        let mut ast = syn::parse_file(r#"
fn keep() {}
fn step() {}
struct Unused;
"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        distribute_items(&mut ast, &main, groups).expect("distribute");

        let main_content = fs::read_to_string(&main).unwrap();
        assert!(main_content.contains("fn keep"), "unmatched item stays in main");
        assert!(main_content.contains("struct Unused"), "unmatched struct stays in main");
        assert!(!main_content.contains("fn step"), "matched function leaves main");
        assert!(main_content.contains("pub mod steps"), "mod decl injected");

        let steps_content = fs::read_to_string(dir.path().join("main/steps.rs")).unwrap();
        assert!(steps_content.contains("fn step"), "matched function lands in steps.rs");
    }

    #[test]
    fn distribute_items_is_idempotent() {
        let mut ast = syn::parse_file(r#"fn step() {}"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];

        distribute_items(&mut ast, &main, groups).expect("first");
        let first = fs::read_to_string(&main).unwrap();
        assert_eq!(first.matches("pub mod steps").count(), 1, "single mod decl after first run");

        distribute_items(&mut ast, &main, groups).expect("second");
        let second = fs::read_to_string(&main).unwrap();
        assert_eq!(second.matches("pub mod steps").count(), 1, "no duplicate mod decl after second run");
        assert_eq!(first, second, "main file is stable after re-run");
    }

    #[test]
    fn distribute_items_rejects_target_without_parent() {
        let mut ast = syn::parse_file(r#"fn step() {}"#).unwrap();
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        let result = distribute_items(&mut ast, Path::new("main.rs"), groups);
        assert!(result.is_err(), "target with no parent must error, not panic");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("parent directory"), "error explains the missing parent");
    }

    #[test]
    fn distribute_items_rejects_target_without_stem() {
        let mut ast = syn::parse_file(r#"fn step() {}"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        let result = distribute_items(&mut ast, &dir.path().join(".rs"), groups);
        eprintln!("STEM_RESULT: {:?}", result.as_ref().map_err(|e| e.to_string()));
        assert!(result.is_err(), "target with no stem must error, not panic");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("non-empty stem"), "error explains the missing stem");
    }

    /// Regression: a group filename like `../../escape` used to flow straight
    /// into `base_dir.join(format!("{filename}.rs"))`, writing the moved items
    /// outside the sibling module directory. Filenames are now validated as
    /// plain Rust identifiers before any AST mutation or file write, and the
    /// tool must refuse traversal instead of following it.
    #[test]
    fn distribute_items_rejects_path_traversal_filename() {
        let mut ast = syn::parse_file(r#"fn step() {}"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "../escape")];
        let result = distribute_items(&mut ast, &main, groups);
        let err = result.expect_err("traversal filename must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("../escape"), "error names the offending filename");
        assert!(
            !dir.path().join("escape.rs").exists(),
            "no file may be written outside the module directory"
        );
        assert!(
            !main.exists(),
            "main file must not be rewritten when validation fails"
        );
        assert_eq!(ast.items.len(), 1, "AST is left unmodified when validation fails");
    }

    /// Regression: filenames with characters that are legal in paths but not
    /// in Rust identifiers (`my-mod`, empty string) used to fail later with a
    /// syn parse error, or worse, write a file the injected `pub mod` could
    /// never name. Validation now rejects them up front with the filename in
    /// the message.
    #[test]
    fn distribute_items_rejects_non_identifier_filenames() {
        for bad in ["my-mod", "", "9lives", "a b", "r#raw"] {
            let mut ast = syn::parse_file(r#"fn step() {}"#).unwrap();
            let dir = tempfile::tempdir().expect("tempdir");
            let main = dir.path().join("main.rs");
            let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], bad)];
            let result = distribute_items(&mut ast, &main, groups);
            let err = result.expect_err("filename `{bad}` must be rejected");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "filename `{bad}`");
            assert!(err.to_string().contains(bad), "error names the offending filename `{bad}`");
        }
    }

    /// Regression: module file names were stored in a HashMap, so the order of
    /// the injected `pub mod` / `pub use` declarations changed from run to run
    /// and produced noisy diffs in the rewritten main file. Storage is now
    /// sorted, and two runs over the same input must produce byte-identical
    /// output.
    #[test]
    fn distribute_items_output_is_deterministic_across_runs() {
        let source = r#"
fn keep() {}
fn step() {}
fn emit() {}
fn parse() {}
"#;
        let groups: &[(Vec<&str>, &str)] = &[
            (vec!["step"], "steps"),
            (vec!["emit"], "emitters"),
            (vec!["parse"], "parsing"),
        ];
        let mut mains = Vec::new();
        for _ in 0..2 {
            let mut ast = syn::parse_file(source).unwrap();
            let dir = tempfile::tempdir().expect("tempdir");
            let main = dir.path().join("main.rs");
            distribute_items(&mut ast, &main, groups).expect("distribute");
            mains.push(fs::read_to_string(&main).unwrap());
        }
        assert_eq!(mains[0], mains[1], "rewritten main file must be deterministic");
        let mod_pos = mains[0].find("pub mod").expect("mod decls present");
        let decls = &mains[0][mod_pos..];
        let emitters = decls.find("pub mod emitters").unwrap();
        let parsing = decls.find("pub mod parsing").unwrap();
        let steps = decls.find("pub mod steps").unwrap();
        assert!(emitters < parsing && parsing < steps, "mod decls are injected in sorted order");
    }

    /// Regression: when the source already declared `pub mod steps;` without
    /// the glob re-export, the old substring check counted the pair as present
    /// and skipped BOTH injections, so the moved items were never re-exported
    /// and the rewritten crate failed to build. Each declaration is now
    /// checked on the parsed AST and only the missing half is injected.
    #[test]
    fn distribute_items_completes_partial_existing_declarations() {
        let mut ast = syn::parse_file(r#"
pub mod steps;
fn step() {}
"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        distribute_items(&mut ast, &main, groups).expect("distribute");

        let main_content = fs::read_to_string(&main).unwrap();
        assert_eq!(
            main_content.matches("pub mod steps").count(),
            1,
            "no duplicate mod declaration"
        );
        assert_eq!(
            main_content.matches("pub use steps :: *").count(),
            1,
            "missing glob re-export is injected"
        );
    }

    /// Regression: the main file used to be rewritten BEFORE the module files
    /// were created, so a failed module write left main declaring modules that
    /// did not exist. Module files are now written first; when the module
    /// directory cannot be created, the main file must be left untouched.
    #[test]
    fn distribute_items_leaves_main_untouched_when_module_dir_fails() {
        let mut ast = syn::parse_file(r#"fn step() {}"#).unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let original_main = "fn step() {}\n";
        fs::write(&main, original_main).expect("seed main");
        // A regular file where the module directory must go makes
        // create_dir_all fail.
        fs::write(dir.path().join("main"), b"blocker").expect("seed blocker");

        let groups: &[(Vec<&str>, &str)] = &[(vec!["step"], "steps")];
        let result = distribute_items(&mut ast, &main, groups);
        assert!(result.is_err(), "module directory creation must fail");
        assert_eq!(
            fs::read_to_string(&main).unwrap(),
            original_main,
            "main file must not be rewritten when module writes cannot happen"
        );
    }
}