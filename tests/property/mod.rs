//! Property tests for refactor_tool: item-conservation invariants of
//! `distribute_items`, driven by a fixed-seed generator so every run
//! exercises the exact same cases (rung 5: reproducible, no flakiness).

use refactor_tool::{distribute_items, parse_rust_file};

/// xorshift64* with a fixed seed: deterministic pseudo-randomness without
/// pulling in a proptest dependency for one suite.
struct FixedRng(u64);

impl FixedRng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// One generated case: unique function names and a group assignment for each.
struct Case {
    fns: Vec<String>,
    /// group index per function; `groups.len()` means "retained in main".
    assignment: Vec<usize>,
    groups: Vec<String>,
}

/// Generate `cases` deterministic cases, seeded at 0x5EED so a failure
/// reproduces byte-for-byte on every machine.
fn generate(cases: usize) -> Vec<Case> {
    let mut rng = FixedRng(0x5EED);
    (0..cases)
        .map(|case_idx| {
            let group_count = 1 + rng.below(4);
            let groups: Vec<String> = (0..group_count).map(|g| format!("grp{g}")).collect();
            let fn_count = 1 + rng.below(12);
            let fns: Vec<String> = (0..fn_count)
                .map(|f| format!("case{case_idx}_fn{f}"))
                .collect();
            let assignment = (0..fn_count).map(|_| rng.below(group_count + 1)).collect();
            Case {
                fns,
                assignment,
                groups,
            }
        })
        .collect()
}

/// Why: the whole point of `distribute_items` is a lossless split. For any
/// generated source and grouping, every function must survive exactly once:
/// either in the main file (retained) or in exactly one group module file.
/// A drop or duplication would silently delete or double user code.
#[test]
fn distribution_conserves_every_item_exactly_once() {
    for case in generate(64) {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("main.rs");

        let source = case
            .fns
            .iter()
            .map(|name| format!("pub fn {name}() {{}}\n"))
            .collect::<String>();
        std::fs::write(&target, &source).unwrap();

        let mut ast = parse_rust_file(&source).unwrap();
        let named: Vec<&str> = case
            .fns
            .iter()
            .zip(&case.assignment)
            .filter(|(_, g)| **g < case.groups.len())
            .map(|(n, _)| n.as_str())
            .collect();
        // Build groups: group g gets every function assigned to it.
        let mut group_items: Vec<Vec<&str>> = vec![Vec::new(); case.groups.len()];
        for (name, &g) in case.fns.iter().zip(&case.assignment) {
            if g < case.groups.len() {
                group_items[g].push(name);
            }
        }
        let group_spec: Vec<(Vec<&str>, &str)> = group_items
            .iter()
            .zip(&case.groups)
            .map(|(items, g)| (items.clone(), g.as_str()))
            .collect();

        distribute_items(&mut ast, &target, &group_spec).unwrap();

        // Count each function's occurrences across main + module files by
        // re-parsing the written tree.
        let main_src = std::fs::read_to_string(&target).unwrap();
        let mut occurrences: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut count_in = |src: &str| {
            let file = parse_rust_file(src).unwrap();
            for item in &file.items {
                if let Some(name) = refactor_tool::item_name(item) {
                    if case.fns.contains(&name) {
                        *occurrences.entry(name).or_insert(0) += 1;
                    }
                }
            }
        };
        count_in(&main_src);
        for g in &case.groups {
            let module = dir.path().join("main").join(format!("{g}.rs"));
            if module.exists() {
                count_in(&std::fs::read_to_string(&module).unwrap());
            }
        }

        for name in &case.fns {
            assert_eq!(
                occurrences.get(name).copied().unwrap_or(0),
                1,
                "function {name} must survive exactly once (groups: {named:?})"
            );
        }
    }
}

/// Why: injected `pub mod` declarations must be byte-identical across runs
/// (the tool runs in CI diffs), which holds only if module names come out in
/// sorted order regardless of the input group order.
#[test]
fn injected_module_declarations_are_sorted() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("main.rs");
    let source = "pub fn a() {}\npub fn b() {}\npub fn c() {}\n";
    std::fs::write(&target, source).unwrap();

    let mut ast = parse_rust_file(source).unwrap();
    // Deliberately unsorted group order.
    let groups: Vec<(Vec<&str>, &str)> = vec![
        (vec!["c"], "zeta"),
        (vec!["a"], "alpha"),
        (vec!["b"], "mid"),
    ];
    distribute_items(&mut ast, &target, &groups).unwrap();

    let main_src = std::fs::read_to_string(&target).unwrap();
    // The main file is re-rendered from a token stream, so punctuation is
    // spaced (`pub mod alpha ;`). Match on the stable prefix instead.
    let alpha = main_src.find("pub mod alpha").unwrap();
    let mid = main_src.find("pub mod mid").unwrap();
    let zeta = main_src.find("pub mod zeta").unwrap();
    assert!(
        alpha < mid && mid < zeta,
        "module declarations must be injected in sorted order, got:\n{main_src}"
    );
}

/// Why: a group that names no present item must not create an empty module
/// file or a dangling `pub mod` declaration; that boundary is where stale
/// refactor configs would otherwise litter the tree with dead modules.
#[test]
fn unmatched_groups_produce_no_files_or_declarations() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("main.rs");
    let source = "pub fn present() {}\n";
    std::fs::write(&target, source).unwrap();

    let mut ast = parse_rust_file(source).unwrap();
    let groups: Vec<(Vec<&str>, &str)> = vec![(vec!["absent"], "ghost")];
    distribute_items(&mut ast, &target, &groups).unwrap();

    assert!(
        !dir.path().join("main").join("ghost.rs").exists(),
        "unmatched group must not create a module file"
    );
    let main_src = std::fs::read_to_string(&target).unwrap();
    assert!(
        !main_src.contains("pub mod ghost;"),
        "unmatched group must not inject a module declaration"
    );
    assert!(
        main_src.contains("present"),
        "ungrouped item must be retained in main"
    );
}
