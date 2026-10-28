//! A spilled register's undef operand goes when the instruction is folded.

use std::process::Command;

/// `str$(42) <> " 42" or str$(-42) <> "-42"` under `--isel=dag` folded an
/// instruction whose `undefs` still named the spilled register: the allocator
/// stopped at "v23 is erased while an instruction names it" (rc 101).
#[test]
fn a_folded_instruction_does_not_keep_the_spilled_undef_operand() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("s.bas");
    std::fs::write(&source, "if str$(42) <> \" 42\" or str$(-42) <> \"-42\" then\n    print \"FAIL\"\nend if\n")
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .args(["-O2", "--isel=dag"])
        .arg(&source)
        .arg("-o")
        .arg(scratch.path().join("s.obj"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
