//! LSR's end-biased formulae (DEVIATE LG3) add a symbol base for each address
//! (`@sym+24`: an `adrp` + `add` on arm64). nbody_single -O2 (6 trips) took 8
//! `adrp` where the pin's solution needs 4: 12 B for two loop instructions
//! saved over six trips.

use std::process::Command;

fn compile(fixture: &str) -> (String, String) {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_DEBUG", "lsr")
        .env_remove("LLRM_ARM64_VECTORS")
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(format!("{}/tests/data/{fixture}.ll", env!("CARGO_MANIFEST_DIR")))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    (std::fs::read_to_string(&output).unwrap(), String::from_utf8_lossy(&said.stderr).into_owned())
}

fn pages(text: &str) -> usize {
    text.lines().filter(|line| line.contains("adrp")).count()
}

#[test]
fn nbody_single_keeps_the_plain_symbol_bases() {
    let (text, _) = compile("nbody_single_o2");
    assert_eq!(pages(&text), 4, "{text}");
}

/// Control: where the trips are many (`n` is any positive `i32`) the end-biased
/// solution still wins. A `u32` product of trips and insns wrapped here once.
#[test]
fn a_loop_of_many_trips_keeps_the_end_biased_solution() {
    let (text, trace) = compile("lg3_many_trips");
    assert!(trace.contains("pin's space solved: kept false"), "{trace}");
    assert_eq!(pages(&text), 3, "{text}");
}
