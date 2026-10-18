//! The translated greedy allocator writes its spills as it goes (`InlineSpiller`).

use std::process::Command;

/// Eight values rotated in a loop, each started from a constant: the copy at the back edge reads eight
/// spilled sources at once, a constant read by a copy is a literal in place, and the instructions the
/// spiller rewrites keep the live intervals of every other value (`LLRM_CHECK_LIVEINTERVALS`).
const ROTATE: &str = "void rot(int *p, int n)\n{\n    int a = 1, b = 2, c = 3, d = 4, e = 5, f = 6, g = 7, h = 8;\n    int i;\n\
    for (i = 0; i < n; ++i) {\n        int t = a;\n        a = b + p[0];\n        b = c + p[1];\n        c = d + p[2];\n\
        d = e + p[3];\n        e = f + p[4];\n        f = g + p[5];\n        g = h + p[6];\n        h = t + p[7];\n    }\n\
    p[0] = a; p[1] = b; p[2] = c; p[3] = d; p[4] = e; p[5] = f; p[6] = g; p[7] = h;\n}\n";

/// The inline allocation of `rot` fell back to the rounds at -Os (two spill products of the copy needed a
/// register no one could give), and at -O1 and -O2 left an interval that the instructions rewritten no
/// longer matched. Each level must finish inline, with the intervals equal to ones made afresh.
#[test]
fn a_rotation_of_constants_is_allocated_inline_at_every_level() {
    for level in ["-O1", "-O2", "-Os"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), ROTATE).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
            .current_dir(scratch.path())
            .env("LLRM_GREEDY", "1")
            .env("LLRM_CHECK_LIVEINTERVALS", "1")
            .env("LLRM_GREEDY_CHECK", "1")
            .env("LLRM_GREEDY_TRACE", "1")
            .args(["-m32", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{level}: {said}");
        assert!(said.contains("intervals equal afresh"), "{level}: the check compared nothing: {said}");
        assert!(!said.contains("left for the rounds"), "{level}: {said}");
        assert!(!said.contains("greedy-check"), "{level}: {said}");
    }
}

/// tests/run/c/es_across_copy.c at -O2: spilling a far pointer's offset put an index add before its use, which
/// redefined the base register; the base kept its old interval and `LLRM_CHECK_LIVEINTERVALS` found it stale.
#[test]
fn a_register_an_inline_spill_redefines_gets_its_interval_again() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/c/es_across_copy.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .env("LLRM_GREEDY", "1")
        .env("LLRM_CHECK_LIVEINTERVALS", "1")
        .env("LLRM_GREEDY_CHECK", "1")
        .args(["-O2", "-march=i486", "-S", "-o", "a.s"])
        .arg(source)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
    assert!(!said.contains("greedy-check"), "{said}");
}
