//! A loop whose phis start at constants runs its first trip folded when the
//! clocks saved repay the copy (PL6): mandel's `x = y = 0` trip. Needs
//! the Open Watcom tree (tools/ow-fetch.sh) and the m16 clang
//! (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

/// bench/mandel's kernel: three multiplies a trip.
const SOURCE: &str = "long f(long cx, long cy)\n{\n    long x = 0, y = 0;\n    short i;\n    for (i = 0; i < 32; ++i) {\n        long xx = (x * x) >> 8;\n        long yy = (y * y) >> 8;\n        if (xx + yy > 1024) break;\n        y = ((x * y) >> 7) + cy;\n        x = xx - yy + cx;\n    }\n    return i;\n}\n";

fn asm(
    level: &str,
    tag: &str,
) -> String {
    asm_of(SOURCE, level, tag)
}

fn asm_of(
    source: &str,
    level: &str,
    tag: &str,
) -> String {
    assert!(
        llrm_cc::ow_present(),
        "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip"
    );
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("peelentry-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
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

/// The unroll pass's log at `level`: it says "peeling 1 iterations" for a loop
/// whose first trip it runs folded.
fn peels(level: &str) -> bool {
    peels_source(SOURCE, level)
}

fn peels_source(
    source: &str,
    level: &str,
) -> bool {
    assert!(
        llrm_cc::ow_present(),
        "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip"
    );
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "peelentry-log-{}-{}-{}",
        level.trim_start_matches('-'),
        source.len(),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_DEBUG", "unroll")
        .args([level, "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    String::from_utf8_lossy(&run.stderr).contains("peel the first iteration")
        && String::from_utf8_lossy(&run.stderr).contains("peeling 1 iterations")
}

/// At speed the first trip (x = y = 0: three multiplies fold) is peeled; for
/// size it is not (mandel -O2: 660898 -> 596494 clocks).
#[test]
fn at_speed_the_first_trip_of_a_folding_loop_is_peeled_and_for_size_it_is_not() {
    assert!(peels("-O2"), "-O2 did not peel the first trip");
    assert!(!peels("-Os"), "-Os peeled it");
}

/// x_primes: `i % d` at `d = 2` is a test of the low bit, not a divide, and
/// the divide per outer trip (40 clocks of the 486, 28k of 430k) is gone. The
/// folded remainder is priced as the `and` the backend makes of it.
#[test]
fn at_speed_the_first_trip_of_a_remainder_loop_tests_the_low_bit() {
    let source = "long f(int n)\n{\n    int i, d, count = 0;\n    for (i = 2; i < n; ++i) { for (d = 2; d * d <= i; ++d) if (i % d == 0) break; if (d * d > i) ++count; }\n    return count;\n}\n";
    let text = asm_of(source, "-O2", "primes");
    assert!(
        text.lines().any(|line| line.trim_start().starts_with("test") && line.trim_end().ends_with(", 1")),
        "{text}"
    );
}

/// frames: `rest % 10` has a constant divisor in the rolled loop already.
/// Priced as a divide on that side and an `and` after the peel, it read as 60
/// clocks folded and peeled digits() (+2.1M of 3.1M clocks). It is not peeled.
#[test]
fn at_speed_a_constant_divisor_the_loop_already_has_does_not_pay_for_a_peel() {
    let source = "int digits(long v)\n{\n    int total = 0;\n    long rest = v;\n    while (rest > 0) {\n        total += (int)(rest % 10);\n        rest /= 10;\n    }\n    return total;\n}\n";
    assert!(!peels_source(source, "-O2"), "the digit loop was peeled");
}

/// nbody_fixed: the inner loop's divides and multiplies read their operands
/// from arrays, so the entry values fold nothing. A two-trip count that took
/// the next trip's work as folded peeled it (-O2: +8999 clocks, +122 B). It is
/// not peeled.
#[test]
fn at_speed_a_loop_whose_multiplies_read_memory_does_not_peel() {
    let source = "static long px[6], py[6];\nlong f(void)\n{\n    long ax = 0, ay = 0, dx, dy, d2, fo;\n    int b, o;\n    for (b = 0; b < 6; ++b) {\n        ax = ay = 0;\n        for (o = 0; o < 6; ++o) {\n            if (o != b) {\n                dx = px[o] - px[b];\n                dy = py[o] - py[b];\n                d2 = dx * dx + dy * dy + 262144L;\n                fo = 512L / (d2 / 262144L + 1);\n                ax += (dx * fo) / 512L;\n                ay += (dy * fo) / 512L;\n            }\n        }\n        px[b] += ax;\n        py[b] += ay;\n    }\n    return px[0] + py[0];\n}\n";
    assert!(!peels_source(source, "-O2"), "the n-body inner loop was peeled");
}
