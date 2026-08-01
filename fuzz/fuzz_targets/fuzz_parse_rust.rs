#![no_main]

use libfuzzer_sys::fuzz_target;
use refactor_tool::{item_name, parse_rust_file};

fuzz_target!(|data: &[u8]| {
    let content = String::from_utf8_lossy(data);
    if let Ok(file) = parse_rust_file(&content) {
        for item in &file.items {
            let _ = item_name(item);
        }
    }
});
