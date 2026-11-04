//! What a size build does with a register that wants a hinted register.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

/// An `optsize` function must not split a register around its hint
/// (`RAGreedy::trySplitAroundHintReg` returns at `hasOptSize`), and the arm64
/// allocator was told the function was not for size: stft_fixed -Os remade
/// three addresses, 260 instructions where 257 are enough (+12 B).
#[test]
fn an_optsize_function_does_not_split_around_a_hint() {
    let ir = include_str!("data/stft_fixed_os.ll");
    // Premise: the kernel is an optsize function.
    assert!(ir.contains("optsize"), "the fixture is a size build");
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Linux, false).expect("lowers");
    let body = asm.split("bench_stft_fixed:").nth(1).expect("the kernel").split("\n\n").next().unwrap();
    let instructions = body.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(instructions <= 257, "{instructions} instructions (260 when the allocator is told the function is not for size)");
}
