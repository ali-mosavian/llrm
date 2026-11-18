//! x86-32 -O1: the target's byte price at speed counts, as at -O2. x_iir kept
//! its constant multiplies as `imul` (24 clk each on the i486): 130,528 clk
//! against gcc -O1's 60,034 on a 1,500-trip run. Needs the Open Watcom tree
//! (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const PRIMES: &str = "long f(int n)\n{\n    int i, d, count = 0;\n    for (i = 2; i < n; ++i) { for (d = 2; d * d <= i; ++d) if (i % d == 0) break; if (d * d > i) ++count; }\n    return count;\n}\n";
const FIR: &str = "static short x[512], coef[16];\nlong f(long n)\n{\n    long i, k, sum = 0; unsigned long s = 17;\n    for (i = 0; i < 16; ++i) coef[i] = (short)(i * 100 - 700);\n    for (i = 0; i < n + 16; ++i) { s = s * 1103515245ul + 12345ul; x[i] = (short)((s >> 16) & 0xFFF); }\n    for (i = 0; i < n; ++i) { long acc = 0; for (k = 0; k < 16; ++k) acc += (long)x[i + k] * coef[k]; sum += acc >> 8; }\n    return sum;\n}\n";
const MASKED: &str = "long f(int n)\n{\n    int i, x = 1;\n    long s = 0;\n    for (i = 0; i < n; ++i) { x = (x * 5 + i) & 0xFFFF; s += x; }\n    return s;\n}\n";
const SOURCE: &str = "long f(long n)\n{\n    long i, y1 = 0, y2 = 0, sum = 0;\n    for (i = 0; i < n; ++i) {\n        long y = (i * 1024 + y1 * 1600 - y2 * 700) >> 10;\n        y2 = y1; y1 = y; sum += y;\n    }\n    return sum;\n}\n";

fn compile(
    source: &str,
    extra: &[&str],
) -> String {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "o1-speed-{}-{}-{}",
        std::process::id(),
        extra.len(),
        source.len()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-S", "-march=i486"])
        .args(extra)
        .arg("-o")
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

#[test]
fn at_o1_constant_multiplies_are_chains_not_imul() {
    let pin = compile(SOURCE, &["-fno-llrm-speed-rate"]);
    assert!(pin.contains("imul"), "premise: the pin's -O1 pricing keeps imul:\n{pin}");
    let text = compile(SOURCE, &[]);
    assert!(!text.contains("imul"), "an imul of 24 clk a trip is left at -O1:\n{text}");
}

/// x_primes -O1: gcc peels the `d = 2` trip (`i % 2` is `test bl, 1`); ours ran
/// a `div` for it, 1,196 of 6,733 `div`s of 40 clk: 437,480 clk against
/// gcc's 396,675.
#[test]
fn at_o1_the_first_trip_of_a_loop_with_constant_entry_values_is_peeled() {
    let text = compile(PRIMES, &[]);
    assert!(
        text.lines().any(|line| line.trim().starts_with("test") && line.trim().ends_with(", 1")),
        "no `i % 2` as a test:\n{text}"
    );
}

/// x_funcptr -O1: `x & 0xFFFF` was `movzx eax, ax`, 3 clk on the i486, 1,500
/// times: 34,525 clk against gcc's 31,529 (`X86_TUNE_ZERO_EXTEND_WITH_AND`).
#[test]
fn at_o1_a_word_mask_is_an_and_not_a_movzx() {
    let text = compile(MASKED, &[]);
    assert!(!text.contains("movzx"), "a movzx of 3 clk is left where `and` takes 1:\n{text}");
}

/// x_fir -O1: the 16-trip tap loop stayed rolled (its copies were larger than
/// the loop), so each trip paid an `imul` of 24 clk by a coefficient read from
/// memory: 220,085 clk against gcc's 212,662. Priced by its clocks, as at -O2,
/// the copies fold the coefficients to constants.
#[test]
fn at_o1_a_loop_of_sixteen_trips_is_unrolled_when_the_clocks_pay() {
    let pin = compile(FIR, &["-fno-llrm-speed-rate"]);
    assert!(pin.contains("imul"), "premise: the loop stays rolled without the rate:\n{pin}");
    let text = compile(FIR, &[]);
    assert!(!text.contains("imul"), "the rolled loop's imul is left at -O1:\n{text}");
}
