//! What a size build does with a register that wants a hinted register.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// An `optsize` function must not split a register around its hint
/// (`RAGreedy::trySplitAroundHintReg` returns at `hasOptSize`), and the arm64
/// allocator was told the function was not for size: stft_fixed -Os remade
/// addresses, 262 instructions where 259 are enough (+12 B).
#[test]
fn an_optsize_function_does_not_split_around_a_hint() {
    let ir = include_str!("data/stft_fixed_os.ll");
    // Premise: the kernel is an optsize function.
    assert!(ir.contains("optsize"), "the fixture is a size build");
    let asm = driver::assembly(ir, Platform::Linux, false).expect("lowers");
    let body = asm.split("bench_stft_fixed:").nth(1).expect("the kernel").split("\n\n").next().unwrap();
    let instructions = body.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(instructions <= 259, "{instructions} instructions (262 when the allocator is told the function is not for size)");
}
