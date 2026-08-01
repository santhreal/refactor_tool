//! CLI for massive-scale pattern-based AST refactoring.
//!
//! The group tables below are surgec-specific one-offs, not a generic
//! refactoring vocabulary. If the tool is ever reused for a different
//! codebase, move them to a Tier-B data file the CLI reads instead of
//! hardcoding them here.

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use refactor_tool::distribute_items;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: refactor_tool <target_file.rs> <mode>");
        return ExitCode::FAILURE;
    }
    let target_path = Path::new(&args[1]);
    let mode = &args[2];

    let content = match fs::read_to_string(target_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "refactor_tool: failed to read target `{}`: {e}",
                target_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let mut ast = match refactor_tool::parse_rust_file(&content) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("refactor_tool: failed to parse `{}`: {e}", target_path.display());
            return ExitCode::FAILURE;
        }
    };

    let result = match mode.as_str() {
        "decode" => run_decode_refactor(&mut ast, target_path),
        "lower" => run_lower_refactor(&mut ast, target_path),
        "interp" => run_interp_refactor(&mut ast, target_path),
        "naga" => run_naga_refactor(&mut ast, target_path),
        other => {
            eprintln!("refactor_tool: unknown mode: {other}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = result {
        eprintln!("refactor_tool: failed to distribute items in `{}`: {e}", target_path.display());
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run_decode_refactor(ast: &mut syn::File, target_path: &Path) -> std::io::Result<()> {
    // surgec-specific decode extraction groups.
    let base64 = vec![
        "extract_encoded_regions",
        "extract_base64_regions",
        "base64_decoded_source_map",
        "is_b64_span_char",
        "is_b64_char_for",
        "base64_decode",
    ];
    let hex = vec![
        "extract_hex_regions",
        "extract_hex_escape_regions",
        "extract_raw_hex_runs",
        "extract_delimited_hex_runs",
        "decode_hex_bytes",
        "hex_token",
        "hex_nibble",
        "is_hex_separator",
    ];
    let unicode = vec![
        "extract_url_encoded_regions",
        "UnicodeMode",
        "extract_unicode_regions",
        "parse_unicode_escape",
        "parse_u4_escape",
        "parse_unicode_brace_escape",
        "push_codepoint_utf8",
        "is_high_surrogate",
        "is_low_surrogate",
        "decode_surrogate_pair",
        "extract_fullwidth_regions",
        "nfkc_candidate",
    ];
    let html = vec!["extract_html_entity_regions", "parse_html_entity"];
    let octal = vec!["extract_octal_regions", "parse_octal_escape"];
    let archive = vec![
        "extract_gzip_regions",
        "is_valid_gzip_header",
        "decode_gzip_member",
        "extract_zlib_regions",
        "zlib_candidate_offsets",
        "extract_deflate_regions",
        "deflate_candidate_offsets",
        "extract_zip_regions",
        "extract_tar_regions",
        "looks_like_tar_header",
        "decode_reader_bytes",
    ];
    let groups = vec![
        (base64, "base64"),
        (hex, "hex"),
        (unicode, "unicode"),
        (html, "html"),
        (octal, "octal"),
        (archive, "archive"),
    ];
    distribute_items(ast, target_path, &groups)
}

fn run_lower_refactor(ast: &mut syn::File, target_path: &Path) -> std::io::Result<()> {
    let call = vec!["lower_call", "lower_method"];
    let literal = vec![
        "lower_literal_regex",
        "lower_literal_bytes",
        "lower_literal_string",
    ];
    let dict = vec!["lower_dict", "lower_list", "lower_tuple"];
    let groups = vec![
        (call, "call_ops"),
        (literal, "literal_ops"),
        (dict, "collection_ops"),
    ];
    distribute_items(ast, target_path, &groups)
}

fn run_interp_refactor(ast: &mut syn::File, target_path: &Path) -> std::io::Result<()> {
    let memory = vec![
        "HashmapMemory",
        "workgroup_memory",
        "resolve_buffer",
        "buffer_mut",
        "atomic_buffer_mut",
        "output_value",
    ];
    let state = vec![
        "HashmapLocals",
        "HashmapInvocation",
        "HashmapResolvedCall",
        "HashmapInvocationSnapshot",
        "create_invocations",
        "run_invocations",
    ];
    let step = vec![
        "step_round_robin",
        "step",
        "step_nodes_frame",
        "step_loop_frame",
        "execute_node",
        "axis_value",
        "eval_expr_snapshot",
        "eval_to_index",
        "eval_call",
        "resolve_call",
        "capture_invocation_snapshots",
    ];
    let subgroup = vec![
        "subgroup_simulator",
        "subgroup_slice",
        "eval_subgroup_ballot",
        "eval_subgroup_shuffle",
        "eval_subgroup_add",
    ];
    let sync = vec![
        "release_barrier_if_ready",
        "live_waiting_count",
        "verify_uniform_control_flow",
        "contains_barrier",
        "node_contains_barrier",
        "node_id",
        "element_count",
    ];
    let groups = vec![
        (memory, "memory"),
        (state, "state"),
        (step, "step"),
        (subgroup, "subgroup"),
        (sync, "sync"),
    ];
    distribute_items(ast, target_path, &groups)
}

fn run_naga_refactor(ast: &mut syn::File, target_path: &Path) -> std::io::Result<()> {
    let arithmetic = vec!["emit_add", "emit_sub", "emit_mul", "emit_div", "emit_rem"];
    let atomic = vec![
        "emit_atomic_add",
        "emit_atomic_load",
        "emit_atomic_store",
        "emit_atomic_exchange",
        "emit_atomic_compare_exchange",
    ];
    let subgroup = vec![
        "emit_subgroup_ballot",
        "emit_subgroup_add",
        "emit_subgroup_shuffle",
    ];
    let groups = vec![
        (arithmetic, "arithmetic"),
        (atomic, "atomic"),
        (subgroup, "subgroup"),
    ];
    distribute_items(ast, target_path, &groups)
}
