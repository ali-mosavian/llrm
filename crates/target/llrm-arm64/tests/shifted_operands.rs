//! Shifts folded into the instruction that reads them.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/shifted_operands.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `a + (b << 3)` was `ubfm`/`lsl` then `add`; clang makes `add x0, x0, x1, lsl
/// #3`. The same for `sub` of a right shift and for the logical operations.
#[test]
fn a_shift_read_by_an_arithmetic_or_logical_instruction_is_its_shifted_operand() {
    for (function, folded) in [("add_shl", "add x"), ("sub_lshr", "sub w"), ("eor_ashr", "eor x")] {
        let asm = body(function);
        let lines = asm.lines().filter(|line| line.starts_with('\t')).count();
        assert!(asm.contains(folded) && lines <= 2, "{function}:\n{asm}");
    }
}

/// A shift read twice, by a variable amount, or on the subtracted side stays
/// its own instruction.
#[test]
fn a_shift_that_cannot_fold_stays_a_shift() {
    for function in ["shift_read_twice", "variable_amount", "subtracted_from_the_shift"] {
        let asm = body(function);
        assert!(
            asm.contains("lsl x") || asm.contains("lsl\tx") || asm.contains("ubfm") || asm.contains("lslv"),
            "{function}:\n{asm}"
        );
    }
}
