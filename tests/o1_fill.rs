//! -O1 turns a loop that copies or fills into a memmove, a memcpy or a memset:
//! clang's O1 pipeline has `LoopIdiomRecognize` (PassBuilderPipelines.cpp
//! L562), gcc's -O1 does not (`-ftree-loop-distribute-patterns` is -O2, opts.cc
//! L653). The user's decision of 2026-10-10, recorded in
//! docs/reference/levels.md; -Og keeps the loops.

mod common;

/// bench/scroll's four row loops: up a row, blank the last, down a row, copy to
/// a back buffer.
const SCROLL: &str = "short screen[2000];\nshort back[2000];\n\
long f(void)\n{\n    long total = 0;\n    short r, i;\n    for (r = 0; r < 50; ++r) {\n\
        for (i = 0; i < 1920; ++i) screen[i] = screen[i + 80];\n\
        for (i = 1920; i < 2000; ++i) screen[i] = 0x0720;\n\
        for (i = 1919; i >= 0; --i) screen[i + 80] = screen[i];\n\
        for (i = 0; i < 2000; ++i) back[i] = screen[i];\n    }\n    return total + back[7];\n}\n";

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), SCROLL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-march=i486"])
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// scroll at -O1 was 1,821,256 clocks against clang's 531,108 (3.43x), 4
/// instructions a trip in four loops where clang calls memmove and memcpy.
///
/// llrm-cc: `-Og` is clang's `-O1` (llrm-cc maps it so), which has
/// `LoopIdiomRecognize`; it forms the string operations as `-O1` does
/// (pin-shape; llrm-c kept the loops at `-Og`). The blank row stores `0x0720`,
/// which is no byte splat, so there is no memset: the pin keeps that loop.
#[test]
fn o1_and_og_form_memmoves_from_scrolls_loops() {
    // A copy is an inline `rep movs` or a call, as clang's (memmove, memcpy).
    let moves = |text: &str| text.matches("rep movs").count() + text.matches("call _mem").count();
    let o1 = listing(&["-O1"]);
    assert!(moves(&o1) >= 3, "-O1 left scroll's loops as loops:\n{o1}");
    let og = listing(&["-Og"]);
    assert!(moves(&og) >= 3, "-Og left scroll's loops as loops:\n{og}");
    let off = listing(&["-O1", "-fno-tree-loop-distribute-patterns"]);
    assert!(moves(&off) == 0, "-fno-tree-loop-distribute-patterns did not keep the loops:\n{off}");
}
