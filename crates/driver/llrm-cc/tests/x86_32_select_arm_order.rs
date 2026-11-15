//! x86-32 -Os: a select with even odds, expanded to a branch (the i486 has no
//! `cmov`), falls through to the arm of the false value, as
//! `EmitLoweredSelect`'s `copy0MBB` does. x_collatz had the true arm (`shr`)
//! fall through: the hot even step paid `jne` + `jmp` and the odd one a taken
//! `jne`, 4,833 clk over the pin's layout on a 168,668-clk run. Needs the Open
//! Watcom tree (tools/ow-fetch.sh) and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned long f(int n)\n{\n    unsigned long x = n, steps = 0;\n    while (x != 1) { x = (x & 1) ? 3 * x + 1 : x >> 1; ++steps; }\n    return steps;\n}\n";

#[test]
fn a_select_of_even_odds_falls_through_to_the_arm_of_the_false_value() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("select-arm-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = |what: &str| lines.iter().position(|line| line.starts_with(what));
    let (lea, shr) = (at("lea").expect("the odd step"), at("shr").expect("the even step"));
    assert!(lea < shr, "the even step (shr) falls through to the test, the odd one (lea) is a taken branch:\n{text}");
}
