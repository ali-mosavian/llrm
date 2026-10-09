//! The module summaries are made once, at the end of the interprocedural step,
//! and the passes after it read them without making them again after each edit
//! (`PassManager::freeze`). Each of those passes says it adds no memory
//! operation; a compile with `LLRM_CHECK_STALE=1` makes the summaries afresh
//! after each and fails on a pass that did. The gate runs this so that a later
//! pass which speculates a load, and does not say so, cannot leave a frozen
//! summary unsound unseen.

use std::process::Command;

const PROGRAMS: [&str; 8] = ["crc", "bintree", "matmul", "nbody", "sieve", "queens", "grep", "huge"];

#[test]
fn the_passes_after_the_summaries_add_no_memory_operation() {
    for program in PROGRAMS {
        for flags in [&["-m32", "-march=i486"][..], &[][..]] {
            for level in ["-O1", "-O2", "-Os"] {
                let scratch = tempfile::tempdir().unwrap();
                let source = format!("{}/bench/{program}/{program}.c", env!("CARGO_MANIFEST_DIR"));
                let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
                    .current_dir(scratch.path())
                    .args(flags)
                    .args([level, "-o", "a.obj", &source])
                    .env("LLRM_CHECK_STALE", "1")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{program} {flags:?} {level}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}
