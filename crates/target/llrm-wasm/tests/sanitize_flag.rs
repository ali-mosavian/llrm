//! `-fsanitize=` on this target's compiler (executed sweep, 2026-11-05).

use std::process::{Command, Output};

fn llc(flag: &str) -> Output {
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ir/debug_lines.ll");
    Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc")).args(["-S", "-o", "-", flag, ir]).output().unwrap()
}

/// `llrm-cc -fsanitize=bounds` on wasm32 failed with `unrecognized argument`,
/// though the check is nothing in IR: the program must compile to what it does
/// without the flag.
#[test]
fn fsanitize_bounds_is_accepted_and_changes_nothing() {
    let with = llc("-fsanitize=bounds");
    assert!(with.status.success(), "{}", String::from_utf8_lossy(&with.stderr));
    assert_eq!(with.stdout, llc("-fno-sanitize=bounds").stdout);
}

/// The stack check needs a runtime wasm lacks: refused by name, never silently
/// dropped (a program that expects "Stack Overflow!" would run off its stack)
/// and not as a typo.
#[test]
fn fsanitize_stack_is_refused_by_name() {
    let said = llc("-fsanitize=stack");
    let stderr = String::from_utf8_lossy(&said.stderr);
    assert_eq!(said.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("-fsanitize=stack: wasm has no C runtime stack check"), "{stderr}");
}
