//! `x & 255` widened is `movzx r32, r8`: BASIC, C and Nib each compiled
//! `and r16, 0FFh` after a register copy, then `movzx r32, r16`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn compiled(tool: &str, source: &str, arguments: &[&str]) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("out.asm");
    let done = Command::new(bin.join(tool)).arg(root.join("tests/inputs/masked").join(source)).args(arguments).args(["-O2", "--cpu", "486", "-S", "-o"]).arg(&out).output().unwrap();
    assert!(done.status.success(), "{tool} {source}: {}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// The mask is gone and the extension reads the byte register.
fn assert_byte_extension(language: &str, asm: &str) {
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    assert!(lines.iter().any(|line| line.starts_with("movzx e") && line.ends_with(['l', 'h'])), "premise: {language} extends a byte register:\n{asm}");
    let masks: Vec<&&str> = lines.iter().filter(|line| line.starts_with("and ") && (line.ends_with("0FFh") || line.ends_with("255"))).collect();
    assert!(masks.is_empty(), "{language} masks before extending: {masks:?}\n{asm}");
    let words: Vec<&&str> = lines.iter().filter(|line| line.starts_with("movzx e") && !line.ends_with(['l', 'h'])).collect();
    assert!(words.is_empty(), "{language} extends a word that the mask cleared: {words:?}\n{asm}");
}

#[test]
fn test_a_masked_byte_widened_is_one_movzx_in_basic() {
    assert_byte_extension("bas", &compiled("llrm-qb", "mix.bas", &["--dialect", "pds71", "--runtime", "pds71"]));
}

#[test]
fn test_a_masked_byte_widened_is_one_movzx_in_c() {
    assert_byte_extension("c", &compiled("llrm-c", "mix.c", &["-fno-inline-functions"]));
}

#[test]
fn test_a_masked_byte_widened_is_one_movzx_in_nib() {
    assert_byte_extension("nib", &compiled("llrm-nib", "mix.nib", &[]));
}
