//! The `dot` example compiles at both word sizes and every level it is built
//! at.

use std::process::Command;

/// `ends_program` (cb93feed2f) turned every `Return` named `jmp` in the main
/// into a call; once `Dot` was inlined, a second sibling `jmp` in a minted
/// block had no callee, and `dot.bas` stopped compiling: m16 "$QB$MAIN at
/// 8589934593: a call with no callee", m32 "the frame register was given to
/// a value" (gate run 40780286803: oracle-2, 8 of 470 compiles).
#[test]
fn test_dot_bas_compiles_at_m16_and_m32_at_every_level() {
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/dot/dot.bas");
    let scratch = tempfile::tempdir().unwrap();
    let mut failed = Vec::new();
    for mode in ["-m16", "-m32"] {
        for level in ["-O1", "-O2", "-O3"] {
            let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
                .current_dir(scratch.path())
                .args([level, mode, "-o", "dot.o", source])
                .output()
                .unwrap();
            if !output.status.success() {
                let said = String::from_utf8_lossy(&output.stderr);
                failed.push(format!("{mode} {level}: {}", said.trim().lines().last().unwrap_or("")));
            }
        }
    }
    assert!(failed.is_empty(), "dot.bas did not compile: {failed:#?}");
}
