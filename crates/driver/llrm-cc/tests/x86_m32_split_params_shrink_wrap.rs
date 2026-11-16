//! A recursion that returns on its base case before any call pays no frame
//! there: the argument it keeps across the calls is copied to a kept register
//! where the calls begin, not at the entry. gcc 15 does (IRA's
//! `split_live_ranges_for_shrink_wrap`); ours pushed four registers and
//! reserved the frame first for every leaf call (x_mergesort -O2: 1,598 pops
//! against 797, 57,252 clk against gcc's 54,411).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static int a[256], b[256];\n\
    static void msort(int lo, int hi) {\n\
        int mid, i, j, k;\n\
        if (hi - lo < 2) return;\n\
        mid = (lo + hi) / 2;\n\
        msort(lo, mid); msort(mid, hi);\n\
        i = lo; j = mid; k = lo;\n\
        while (i < mid && j < hi) b[k++] = a[i] <= a[j] ? a[i++] : a[j++];\n\
        while (i < mid) b[k++] = a[i++];\n\
        while (j < hi) b[k++] = a[j++];\n\
        for (k = lo; k < hi; ++k) a[k] = b[k];\n\
    }\n\
    void sort(int n) { msort(0, n); }\n";

/// The listing's lines of `msort` up to its first `push`, which must come
/// after the base-case test.
fn lines_before_first_push() -> Vec<String> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("split-params-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-m32", "-march=i486", "-O2", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    listing
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != "msort proc near")
        .take_while(|line| !line.starts_with("push"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn test_m32_o2_recursion_tests_its_base_case_before_it_pushes() {
    let before = lines_before_first_push();
    assert!(
        before.iter().any(|line| line.starts_with("cmp ")) && before.iter().any(|line| line.starts_with("jl ")),
        "the first push comes before the base-case test:\n{before:#?}"
    );
}
