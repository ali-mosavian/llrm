#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// The instructions of each function in wasm assembly: `== name`, then its
/// lines without comments and directives.
pub fn bodies(assembly: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in assembly.lines() {
        let text = line.trim();
        if let Some(name) = text.strip_suffix(':').or_else(|| text.split_once(':').map(|(name, _)| name))
            && !text.starts_with('.')
            && !name.contains(char::is_whitespace)
            && !name.is_empty()
        {
            out.push_str(&format!("== {name}\n"));
            inside = true;
            continue;
        }
        if text.contains("end_function") {
            inside = false;
        } else if inside && !text.is_empty() && !text.starts_with('#') && !text.starts_with(".functype") {
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

/// The bodies `llrm-wasm-llc -O2 -S` makes of `tests/ir/NAME.ll`, and the
/// pin llc's (`tests/ir/NAME.expected`).
pub fn made_and_expected(name: &str) -> (String, String) {
    made_and_expected_at("-O2", name)
}

/// `made_and_expected` at the level `level`; the expected file is the pin llc's
/// at the same level.
pub fn made_and_expected_at(
    level: &str,
    name: &str,
) -> (String, String) {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args([level, "-S", "-o", "-"])
        .arg(tests.join(format!("{name}.ll")))
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let expected = std::fs::read_to_string(tests.join(format!("{name}.expected"))).unwrap();
    (bodies(&String::from_utf8_lossy(&made.stdout)), expected)
}
