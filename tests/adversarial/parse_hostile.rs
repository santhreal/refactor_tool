//! Hostile Rust snippets for `parse_rust_file`: must not panic.

use refactor_tool::{item_name, parse_rust_file};

fn parse_snippet(name: &str, content: &str) {
    let _ = parse_rust_file(content);
    if let Ok(file) = parse_rust_file(content) {
        for item in &file.items {
            let _ = item_name(item);
        }
    }
    let _ = name;
}

macro_rules! parse_case {
    ($name:ident, $content:expr) => {
        #[test]
        fn $name() {
            parse_snippet(stringify!($name), $content);
        }
    };
}

parse_case!(empty, "");
parse_case!(whitespace, "   \n\t\n");
parse_case!(fn_minimal, "fn main() {}");
parse_case!(fn_with_args, "fn f(a: i32, b: &str) -> bool { a > 0 }");
parse_case!(struct_def, "struct S { x: i32 }");
parse_case!(enum_def, "enum E { A, B(i32) }");
parse_case!(impl_block, "struct S;\nimpl S { fn f(&self) {} }");
parse_case!(const_item, "const X: u8 = 1;");
parse_case!(type_alias, "type T = Vec<i32>;");
parse_case!(use_tree, "use std::collections::{HashMap, BTreeMap};");
parse_case!(mod_decl, "mod inner { pub fn g() {} }");
parse_case!(attr_fn, "#[inline]\nfn hot() {}");
parse_case!(macro_rules, "macro_rules! m { ($x:expr) => { $x }; }");
parse_case!(unsafe_fn, "unsafe fn u() {}");
parse_case!(async_fn, "async fn a() {}");
parse_case!(gen_fn, "fn g<T: Clone>(x: T) -> T { x.clone() }");
parse_case!(where_clause, "fn w<T>(x: T) -> T where T: Send {}");
parse_case!(lifetime, "fn l<'a>(x: &'a str) -> &'a str { x }");
parse_case!(raw_identifier, "fn r#type() {}");
parse_case!(union_def, "union U { a: u32, b: f32 }");
parse_case!(trait_def, "trait T { fn f(&self); }");
parse_case!(extern_crate, "extern crate std;");
parse_case!(foreign_mod, "extern \"C\" { fn c(); }");
parse_case!(unclosed_brace, "fn bad() {");
parse_case!(unclosed_paren, "fn bad( {}");
parse_case!(double_colon_noise, "::::");
parse_case!(only_semicolons, ";;;");
parse_case!(hash_bang, "#!/usr/bin/env rust");
parse_case!(nested_comments, "/* /* */ still");
parse_case!(line_comment_eof, "// comment only\n");
parse_case!(string_with_escape, r#"fn s() { let _ = "a\nb\tc"; }"#);
parse_case!(byte_string, r#"fn b() { let _ = b"\xff"; }"#);
parse_case!(c_string, r#"fn c() { let _ = c"hi"; }"#);
parse_case!(utf8_ident, "fn café() {}");
parse_case!(null_in_comment, "fn x() {} // \0 note");
parse_case!(very_long_fn_name, &format!("fn {}() {{}}", "f".repeat(200)));
parse_case!(many_items, "fn a() {}\nfn b() {}\nfn c() {}\nstruct S;\nenum E{A,B};");
parse_case!(path_types, "fn p(x: std::sync::Arc<Vec<u8>>) {}");
parse_case!(tuple_struct, "struct T(i32, i32);");
parse_case!(unit_struct, "struct U;");
parse_case!(const_generics, "struct A<const N: usize>;");
parse_case!(async_trait_like, "trait AsyncT { async fn f(); }");
parse_case!(quote_in_attr, "#[doc=\"\\\"\"]\\nfn d() {}");

#[test]
fn item_name_on_parsed_items() {
    let src = "fn alpha() {}\nstruct Beta;\nenum Gamma { X }\nstatic DELTA: u32 = 1;\ntrait Epsilon {}\nunion Zeta { a: u32 }\nmacro_rules! eta { () => {} }";
    let file = parse_rust_file(src).expect("valid");
    let names: Vec<_> = file.items.iter().filter_map(item_name).collect();
    assert!(names.contains(&"alpha".to_string()));
    assert!(names.contains(&"Beta".to_string()));
    assert!(names.contains(&"Gamma".to_string()));
    assert!(names.contains(&"DELTA".to_string()));
    assert!(names.contains(&"Epsilon".to_string()));
    assert!(names.contains(&"Zeta".to_string()));
    assert!(names.contains(&"eta".to_string()));
}
#[test]
fn parse_err_is_syn_error() {
    assert!(parse_rust_file("fn (").is_err());
}
#[test]
fn distribute_items_rejects_keyword_filenames() {
    for kw in ["fn", "struct", "type", "mod", "impl", "enum", "match", "crate", "super", "self", "trait", "where", "use", "pub"] {
        let mut ast = parse_rust_file("fn step() {}").unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("main.rs");
        let groups = &[(vec!["step"], kw)];
        let result = refactor_tool::distribute_items(&mut ast, &main, groups);
        assert!(result.is_err(), "keyword filename `{kw}` must be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
