//! A loop-invariant load whose index is a register copy is made before the
//! loop (the pin's EarlyMachineLICM hoists the `SUBREG_TO_REG` that
//! zero-extends a 32-bit `lea`, then the load). bintree -O1 `insert` read
//! `t[3 * node]` on every trip: 45,071 instructions against the pin's 43,938.

use std::process::Command;

const SOURCE: &str = include_str!("../bench/bintree/bintree.c");

/// The lines of the loop: from a label to the jump back to it.
fn loop_lines(asm: &str) -> Vec<&str> {
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(target) = line.split_whitespace().nth(1).filter(|_| line.starts_with('j')) else { continue };
        if let Some(label) = lines[..at].iter().position(|earlier| earlier.strip_suffix(':') == Some(target)) {
            return lines[label..=at].to_vec();
        }
    }
    panic!("premise: the function has a loop\n{asm}")
}

#[test]
fn a_load_indexed_by_a_loop_invariant_zero_extended_lea_is_made_before_the_loop() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O1", "-fno-inline-functions", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let from = listing.find("insert proc").expect("premise: `insert` is a function");
    let asm = &listing[from..from + listing[from..].find("insert endp").expect("its end")];
    let reads = loop_lines(asm).into_iter().filter(|line| line.contains("dword ptr [")).count();
    assert_eq!(reads, 2, "the loop reads t[3 * at] and the child, not t[3 * node] too:\n{asm}");
}
