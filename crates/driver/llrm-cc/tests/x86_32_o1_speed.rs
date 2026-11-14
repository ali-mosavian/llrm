//! x86-32 -O1: the target's byte price at speed counts, as at -O2. x_iir kept
//! its constant multiplies as `imul` (24 clk each on the i486): 130,528 clk
//! against gcc -O1's 60,034 on a 1,500-trip run. Needs the Open Watcom tree
//! (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "long f(long n)\n{\n    long i, y1 = 0, y2 = 0, sum = 0;\n    for (i = 0; i < n; ++i) {\n        long y = (i * 1024 + y1 * 1600 - y2 * 700) >> 10;\n        y2 = y1; y1 = y; sum += y;\n    }\n    return sum;\n}\n";

fn compile(extra: &[&str]) -> String {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("o1-speed-{}-{}", std::process::id(), extra.len()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
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
    let pin = compile(&["-fno-llrm-speed-rate"]);
    assert!(pin.contains("imul"), "premise: the pin's -O1 pricing keeps imul:\n{pin}");
    let text = compile(&[]);
    assert!(!text.contains("imul"), "an imul of 24 clk a trip is left at -O1:\n{text}");
}
