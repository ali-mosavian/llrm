//! A loop whose phis start at constants runs its first trip folded when the
//! clocks saved repay the copy (PL6): mandel's `x = y = 0` trip. Needs
//! `LLRM_OW` and the m16 clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

/// bench/mandel's kernel: three multiplies a trip.
const SOURCE: &str = "long f(long cx, long cy)\n{\n    long x = 0, y = 0;\n    short i;\n    for (i = 0; i < 32; ++i) {\n        long xx = (x * x) >> 8;\n        long yy = (y * y) >> 8;\n        if (xx + yy > 1024) break;\n        y = ((x * y) >> 7) + cy;\n        x = xx - yy + cx;\n    }\n    return i;\n}\n";

fn asm(
    level: &str,
    tag: &str,
) -> String {
    for name in ["LLRM_OW", "LLRM_CLANG_M16"] {
        assert!(std::env::var_os(name).is_some(), "{name} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("peelentry-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args([level, "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

/// At speed the first trip is peeled; the loop's three multiplies are kept
/// and the peeled copy's that fold are gone.
#[test]
fn at_speed_the_first_trip_of_a_folding_loop_is_peeled() {
    let text = asm("-O2", "speed");
    let loops = text.matches("imul").count();
    let size = asm("-Os", "size").matches("imul").count();
    assert_eq!(size, 3, "-Os keeps the loop alone");
    assert!(loops > size, "{loops} imul at -O2 against {size} at -Os: the first trip was not peeled\n{text}");
}
