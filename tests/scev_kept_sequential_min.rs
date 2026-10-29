//! `LLRM_CHECK_PRESERVED` compares a kept SE with a fresh one.

mod common;

/// queens at -O3: LSR keeps the expression of the or of two exit tests as
/// `umin_seq`; a fresh SE proves the second test not poison and says `umin`.
/// The two are equal, the kept one the weaker, but the oracle read it as stale
/// ("lsr left scev held but stale") on every bench with such a test.
#[test]
fn a_kept_sequential_min_that_a_fresh_se_makes_plain_is_not_stale() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    for flags in [&[][..], &["-m32", "-mabi=sysv", "-march=i486"][..]] {
        let output = common::cc::llrm_cc()
            .env("LLRM_CHECK_PRESERVED", "scev")
            .current_dir(scratch.path())
            .arg("-c")
            .arg("-O3")
            .args(flags)
            .arg("-I")
            .arg(root.join("bench"))
            .arg("-o")
            .arg("q.obj")
            .arg(root.join("bench/queens/queens.c"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{flags:?}: {}", String::from_utf8_lossy(&output.stderr));
    }
}
