//! x86-64: a local function that is the same code as another is dropped at -O1
//! to -O3, as gcc's `-fipa-icf` does. signbit_idioms -O2 kept `ge_*` beside
//! `bit_*` (8 of 24 functions were copies): 75,560 instructions against gcc's
//! 74,628, 74,888 with one body each. Needs `LLRM_LLC`.

use std::process::Command;

fn defined(level: &str) -> Vec<&'static str> {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o", "-"])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/merge_local_functions.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let asm = String::from_utf8_lossy(&run.stdout);
    ["ge_a", "ge_b"].into_iter().filter(|name| asm.contains(&format!("{name} proc"))).collect()
}

#[test]
fn two_local_functions_of_one_body_are_one_at_the_speed_levels() {
    for level in ["-O1", "-O2", "-O3"] {
        assert_eq!(defined(level).len(), 1, "{level}");
    }
}

#[test]
fn at_o0_both_stay() {
    assert_eq!(defined("-O0").len(), 2);
}
