//! x86-64 -O3: ipa-cp clones a recursive function seven deep, so every row of
//! `place(q, row + 1, n)` of the 7-row search is a clone of its own. Four
//! clones left rows 4.. generic: queens ran 4.3% more clk than gcc's.
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static int safe(const int *q, int row, int col)\n{\n    int r;\n    for (r = 0; r < row; ++r) {\n        int d = q[r] - col;\n        if (d == 0 || d == row - r || d == r - row) return 0;\n    }\n    return 1;\n}\nstatic int place(int *q, int row, int n)\n{\n    int col, found = 0;\n    if (row == n) return 1;\n    for (col = 0; col < n; ++col)\n        if (safe(q, row, col)) {\n            q[row] = col;\n            found += place(q, row + 1, n);\n        }\n    return found;\n}\nlong f(void)\n{\n    int q[12];\n    return place(q, 0, 7);\n}\n";

#[test]
fn a_recursion_called_with_constants_is_cloned_seven_deep_at_o3_on_m64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ipacp-depth-m64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-O3", "-w", "-S", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let lines = text.lines().count();
    assert!(lines > 1200, "four clones of `place` leave the later rows generic ({lines} lines):\n{text}");
}

/// gcc runs `tailr1` before ipa-cp: `fib(n - 2)` is a loop, so `fib(n - 1)`
/// passes a phi and nothing is cloned. Seven clones were made for 18, 17 and 16
/// down a chain that folds nothing: fib -O2 +52% clk (3106 -> 4709).
#[test]
fn a_tree_recursion_called_with_a_constant_is_not_cloned_on_m64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ipacp-fib-m64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("f.c"),
        "extern void report(long v);\nstatic int fib(int n)\n{\n    if (n < 2) return n;\n    return fib(n - 1) + fib(n - 2);\n}\nlong bench_fib(int n)\n{\n    return fib(n);\n}\nint bench_main_(void)\n{\n    report(bench_fib(20));\n    return 0;\n}\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-O2", "-w", "-S", "-o"])
        .arg(dir.join("f.s"))
        .arg(dir.join("f.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("f.s")).unwrap();
    assert!(!text.contains("constprop"), "fib was cloned for the constants of its own calls:\n{text}");
}
