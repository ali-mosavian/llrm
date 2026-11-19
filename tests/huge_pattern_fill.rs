//! A word fill over a huge array is a loop, not a `memset.pattern` the DAG
//! refuses.

use std::process::Command;

/// tests/run/huge/fillw.bas: 40000 words past 64K. The m16 target keeps a word
/// fill as `memset.pattern` (535c331947), whose count over a huge pointer is an
/// `i32`: `--isel=dag` stopped the compile ("a pattern fill of other than a
/// word or dword cell, or counted past an address") and the run test went red.
#[test]
fn a_word_fill_over_a_huge_array_compiles() {
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .args(["--dialect", "pds71", "-O2", "-march=i486", "--huge-arrays", "-S", "-o"])
        .arg(scratch.path().join("a.s"))
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run/huge/fillw.bas"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
