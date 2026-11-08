use std::path::Path;
use std::process::Command;

/// The instructions of each function in wasm assembly: `== name`, then its
/// lines without comments and directives.
fn bodies(assembly: &str) -> String {
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

/// `llvm.{s,u}mul.fix[.sat]` and the narrow `{s,u}{add,sub}.sat`, `ushl.sat` at
/// i8 and i16 reached "promote result" and was refused ("Unsupported { op:
/// SMulFixSat, ty: I8 }"), so `-ffixed-point` code (tests/run/c/fixed_point.c)
/// did not compile on wasm32. The expected file is the pin's `llc -O2` output
/// for the same IR (`PromoteIntRes_MULFIX`).
#[test]
fn a_narrow_fixed_point_multiply_promotes_as_the_pin_does() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-S", "-o", "-"])
        .arg(tests.join("mulfix_promote.ll"))
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let expected = std::fs::read_to_string(tests.join("mulfix_promote.expected")).unwrap();
    assert_eq!(bodies(&String::from_utf8_lossy(&made.stdout)), expected);
}
