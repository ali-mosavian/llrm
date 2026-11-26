//! x86-32 -O3 with `-fipa-cp-clone`: ipa-cp clones a recursive function to gcc's
//! depth, 8, so every row of a search that recurses on `row + 1` is a clone of
//! its own and the row constants fold. By default -O3 makes no clone, as the
//! pin. Needs the Open Watcom tree (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static int safe(const int *q, int row, int col)\n{\n    int r;\n    for (r = 0; r < row; ++r) {\n        int d = q[r] - col;\n        if (d == 0 || d == row - r || d == r - row) return 0;\n    }\n    return 1;\n}\nstatic int place(int *q, int row, int n)\n{\n    int col, found = 0;\n    if (row == n) return 1;\n    for (col = 0; col < n; ++col)\n        if (safe(q, row, col)) {\n            q[row] = col;\n            found += place(q, row + 1, n);\n        }\n    return found;\n}\nlong f(void)\n{\n    int q[12];\n    return place(q, 0, 7);\n}\n";

fn lines(extra: &[&str]) -> usize {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ipacp-depth-{}-{}", std::process::id(), extra.len()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-O3", "-w", "-S", "-march=i486"])
        .args(extra)
        .arg("-o")
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap().lines().count()
}

/// -O3 makes no clone by default, as the pin (IPACP-O3 ended on m32);
/// `-fipa-cp-clone` clones to gcc's depth.
#[test]
fn a_recursion_called_with_constants_is_cloned_to_gccs_depth_at_o3_only_when_asked() {
    let (default, asked) = (lines(&[]), lines(&["-fipa-cp-clone"]));
    assert!(default < 400, "-O3 cloned by default ({default} lines)");
    assert!(asked > 1200, "four clones of `place` leave the later rows generic ({asked} lines)");
}
