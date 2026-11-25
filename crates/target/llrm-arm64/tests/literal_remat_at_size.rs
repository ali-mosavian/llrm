//! At -Os a constant that stays read is made again where it is the zero
//! register's move.

use llrm_arm64::lir_view::Arm64;
use llrm_codegen::model::ir::{Held, Imm, Loc, Operation, Semantics};
use llrm_codegen::model::lir::Insn;
use llrm_lir::registers::RegId;
use llrm_spike_mf::tolir::literal_remat_wins;

fn literal(name: &str) -> Insn {
    let what = Semantics {
        name: Some(name.into()),
        dests: vec![Loc::Held(Held { value: 1, width: 8, offset: 0 })],
        sources: vec![Loc::Imm(Imm { value: 4961950020807581558, width: 8, address: None })],
        ..Semantics::new(Operation::Move)
    };
    Insn::new(0, None, Some(what), vec![1], vec![])
}

/// jpeg_single -Os copied a zero through x20 (`orr x20,xzr,xzr; orr
/// x24,xzr,x20; orr x10,xzr,x20`; 61 reg-reg moves against the
/// pin's 19) because C10 kept every constant that stays read as a copy; the pin
/// makes the zero again. `mul_constants` -Os grew 124 B when every constant was
/// made again, so another constant stays a copy.
#[test]
fn a_zero_that_stays_read_is_made_again_at_size_but_another_constant_is_not() {
    let (from, to) = (Vec::<RegId>::new(), Vec::<RegId>::new());
    assert!(literal_remat_wins::<Arm64>(&literal("Zero"), &from, &to, true, false));
    assert!(!literal_remat_wins::<Arm64>(&literal("Imm"), &from, &to, true, false));
}

/// A `movz` is one instruction, the size of the copy it replaces.
#[test]
fn a_constant_one_instruction_makes_is_made_again_at_size() {
    let (from, to) = (Vec::<RegId>::new(), Vec::<RegId>::new());
    assert!(literal_remat_wins::<Arm64>(&literal("Imm1"), &from, &to, true, false));
}

/// jpeg_fixed -Os held `movz w24,#1` in a callee-saved register and copied it
/// to x0 at ten call sites (`orr x0,xzr,x24`; 35 reg-reg copies against the
/// pin's 19): the constant also feeds `1 << n`, so it stays read. The pin makes
/// it again at each call (`mov w0, #1`).
#[test]
fn a_constant_one_that_feeds_a_shift_is_not_copied_from_a_saved_register_at_each_call() {
    use llrm_arm64::driver::{self, Level};
    use llrm_arm64::machine::Platform;
    let text = driver::assembly(include_str!("data/one_constant_across_calls_os.ll"), Platform::Linux, Level::Os)
        .expect("lowers");
    assert!(text.matches("bl w").count() >= 5, "premise: the calls are there:\n{text}");
    let copies: Vec<&str> = text
        .lines()
        .filter(|line| (19..=28).any(|n| line.trim() == format!("orr x0, xzr, x{n}, lsl #0")))
        .collect();
    assert!(copies.is_empty(), "the constant is copied out of a saved register:\n{text}");
}

/// queens `place` -Os (248 B against clang's 240): the `-row` that starts the inner loop's counter stayed
/// hoisted out of the column loop and lived across the recursive call in a seventh callee-saved register
/// (one more save and restore, 8 B). The pin remakes it at the phi copy (`reMaterializeDef`: AArch64 flags
/// every `ADD`/`SUB` `isReMaterializable` and `isAsCheapAsAMove`; the reads need only be live there).
#[test]
fn a_negated_loop_invariant_is_made_again_in_the_loop_instead_of_held_in_a_saved_register() {
    use llrm_arm64::driver::{self, Level};
    use llrm_arm64::machine::Platform;
    let text = driver::assembly(include_str!("data/negated_row_in_col_loop_os.ll"), Platform::Linux, Level::Os)
        .expect("lowers");
    assert!(text.contains("bl place"), "premise: the recursive call is there:\n{text}");
    let saved: std::collections::BTreeSet<u32> = (19..=28).filter(|n| text.contains(&format!("x{n}"))).collect();
    assert!(saved.len() <= 6, "{} callee-saved registers used:\n{text}", saved.len());
}
