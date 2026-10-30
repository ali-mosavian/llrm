//! LoopStrengthReduce does not preserve GlobalsAA at the pin
//! (`LoopStrengthReducePass::run` returns `getLoopPassPreservedAnalyses()`): a
//! pointer induction variable over a global array makes the global's address
//! escape to the pass that reads it. Ours kept the module's `GlobalsModRef`
//! past `lsr` as if nothing escaped, and `LLRM_CHECK_STALE` failed on 12
//! front-end compiles after the call footprints came from it ("lsr added a
//! memory operation: the frozen globals-modref no longer covers the module").

use std::process::Command;

/// (program, -m, -O) of the compiles that failed.
const CASES: [(&str, &str, &str); 12] = [
    ("arridx", "-m16", "-O1"),
    ("idioms", "-m16", "-O1"),
    ("idioms", "-m16", "-O2"),
    ("matrix", "-m16", "-O1"),
    ("matrix", "-m16", "-O2"),
    ("matrix", "-m32", "-O1"),
    ("matrix", "-m32", "-O2"),
    ("nested", "-m16", "-O1"),
    ("nested", "-m32", "-O1"),
    ("pattern_fill", "-m16", "-O1"),
    ("pattern_fill", "-m16", "-O2"),
    ("stride", "-m16", "-O2"),
];

#[test]
fn lsr_leaves_no_stale_globals_modref_in_the_front_end_compiles() {
    for (program, target, level) in CASES {
        let scratch = tempfile::tempdir().unwrap();
        let source = format!("{}/tests/run/qb/{program}.bas", env!("CARGO_MANIFEST_DIR"));
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
            .current_dir(scratch.path())
            .args([source.as_str(), "--dialect", "qb45", level, target, "-o", "a.obj"])
            .env("LLRM_CHECK_STALE", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{program} {target} {level}: {}", String::from_utf8_lossy(&output.stderr));
    }
}
