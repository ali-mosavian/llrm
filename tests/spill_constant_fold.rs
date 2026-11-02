//! A spilled constant is stored from its immediate, not through a register.

use std::process::Command;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// bench/nbody_fixed at -O2 spilled each of its eleven 64-bit constants as
/// `mov rax, -4096` + `mov qword ptr [rsp+120], rax` (two instructions) where
/// the pin stores `mov qword ptr [rsp-80], -4096`: the fold probe addressed
/// the spill cell through the 16-bit frame register, which long mode refuses.
#[test]
fn a_spilled_constant_is_one_store_of_the_immediate() {
    let text = listing(include_str!("../bench/nbody_fixed/nbody_fixed.c"), "-O2");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<_> = lines
        .windows(2)
        .filter(|pair| {
            pair[0].strip_prefix("mov rax, ").is_some_and(|imm| imm.parse::<i32>().is_ok())
                && pair[1].starts_with("mov qword ptr [rsp")
                && pair[1].ends_with(", rax")
        })
        .collect();
    assert!(pairs.is_empty(), "{pairs:?}");
}

/// bench/nbody_fixed at -O2 reloaded each spilled loop-carried sum into one
/// register and copied it to another (`mov rax, [rsp+96]` + `mov rdx, rax`)
/// where the pin reloads straight into the register it uses: the fold probe
/// had no 8-byte register to stand for the copy's other side, so a fold with
/// a second 64-bit register operand was always refused.
#[test]
fn a_reload_goes_straight_into_the_register_the_copy_names() {
    let text = listing(include_str!("../bench/nbody_fixed/nbody_fixed.c"), "-O2");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<_> = lines
        .windows(2)
        .filter(|pair| {
            let Some((into, _)) = pair[0].strip_prefix("mov ").and_then(|rest| rest.split_once(", qword ptr [rsp"))
            else {
                return false;
            };
            pair[1].strip_prefix("mov ").is_some_and(|rest| rest.ends_with(&format!(", {into}")))
        })
        .collect();
    assert!(pairs.is_empty(), "{pairs:?}");
}
