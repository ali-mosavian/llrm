//! x86-64 -O2: a zero vector (`V_SET0`, `xorps r, r`) is remade where a copy
//! of it was wanted, as the pin's `isReMaterializable` zero idioms are. The
//! remat predicate had no zero idiom, so each accumulator a loop starts at
//! zero took `movaps xmm2, xmm1` of one shared zero
//! (mem_length_boundaries_m32 `main`, twice).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn assembly(source: &Path) -> String {
    let llc = target_dir::llc(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."));
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zero-remat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// The register copies of a zeroed register: `movaps x, r` after `xorps r, r`,
/// before `r` is written again.
fn copies_of_a_zero(text: &str) -> Vec<String> {
    let mut zero: Option<String> = None;
    let mut found = Vec::new();
    for line in text.lines().map(str::trim) {
        let mut words = line.split([' ', ',']).filter(|word| !word.is_empty());
        let (Some(op), Some(dest)) = (words.next(), words.next()) else { continue };
        let source = words.next();
        if op == "xorps" && source == Some(dest) {
            zero = Some(dest.to_owned());
        } else if op == "movaps" && source.is_some() && source == zero.as_deref() {
            found.push(line.to_owned());
        } else if zero.as_deref() == Some(dest) || line.ends_with(':') {
            zero = None;
        }
    }
    found
}

#[test]
fn a_loop_accumulator_starting_at_zero_takes_its_own_xorps() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/mem_length_boundaries_m32.c");
    let text = assembly(&source);
    assert!(text.contains("xorps"), "premise: the loops start their sums at a zero vector");
    let copies = copies_of_a_zero(&text);
    assert!(copies.is_empty(), "a zero copied, not remade: {copies:?}");
}
