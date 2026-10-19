//! The shared mid-end on a program of a 64-bit, little-endian, Darwin
//! datalayout (clang's own for `arm64-apple-macos`): it reads the module and
//! optimizes it.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LOOP_CALL: &str = include_str!("data/loop_call.ll");

#[test]
fn the_mid_end_removes_the_frame_slots_of_a_loop() {
    let mut program = driver::program(LOOP_CALL, Platform::Darwin).expect("the module reads");
    let before = llrm_mir::print::module(&program.modules[0]);
    // Premise: clang's -O0-shaped IR has the slots the pipeline is to remove.
    assert!(before.contains("alloca i32"), "{before}");
    driver::optimized(&mut program).expect("the pipeline runs");
    let after = llrm_mir::print::module(&program.modules[0]);
    assert!(!after.contains("alloca"), "{after}");
}

/// A constant needing several instructions (`movz` then `movk`s) was built
/// again inside the loop that uses it, because the chain was not one
/// hoistable definition: nbody ran 4 extra instructions per use.
#[test]
fn a_wide_constant_is_built_outside_the_loop_that_uses_it() {
    let text = include_str!("data/wide_constant_loop.ll");
    let asm = driver::assembly(text, Platform::Darwin, true).expect("the function lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let back = lines.iter().rposition(|l| l.starts_with("b.")).expect("a loop branch");
    let target = lines[back].rsplit(' ').next().unwrap();
    let start = lines.iter().position(|l| l.strip_suffix(':') == Some(target)).expect("the loop label");
    // Premise: the constant is built somewhere (not folded away).
    assert!(lines.iter().any(|l| l.starts_with("movk")), "{asm}");
    let inside: Vec<&&str> = lines[start..=back].iter().filter(|l| l.starts_with("movk") || l.starts_with("movz")).collect();
    assert!(inside.is_empty(), "constant rebuilt in the loop: {inside:?}\n{asm}");
}
