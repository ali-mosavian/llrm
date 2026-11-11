//! Post-RA tail duplication and backward copy propagation.

use llrm_arm64::frame::Lowered;
use llrm_arm64::layout::fold;
use llrm_arm64::mc::inst::{Gpr, Inst, Logic, Shift, Target, Width};
use llrm_arm64::mc::visit::Reg;

fn label(name: &str) -> Target {
    Target::Label(name.to_owned())
}

fn block(
    name: &str,
    insts: Vec<Inst>,
) -> Lowered {
    let locs = vec![None; insts.len()];
    Lowered { label: name.to_owned(), insts, locs }
}

fn mov(
    rd: u32,
    rm: u32,
) -> Inst {
    Inst::LogicShifted {
        op: Logic::Orr,
        invert: false,
        width: Width::X,
        rd: Gpr(rd),
        rn: Gpr::R31,
        rm: Gpr(rm),
        shift: Shift::Lsl,
        amount: 0,
    }
}

/// `recgcd`'s zero-trip path ran `mov x9, xzr; b exit` into `mov x0, x9; ret`:
/// an extra `b` and, once the exit is copied, a move that the pin folds into
/// `mov x0, xzr`. Only the loop's own exit keeps the `mov x0, x9`.
#[test]
fn a_two_instruction_return_is_copied_and_its_move_folded() {
    let entry = block(
        ".Lf.0",
        vec![
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(1), target: label(".Lf.1") },
            Inst::Branch { link: false, target: label(".Lf.2") },
        ],
    );
    let early = block(".Lf.1", vec![mov(9, 31), Inst::Branch { link: false, target: label(".Lf.3") }]);
    let work = block(".Lf.2", vec![Inst::Nop, mov(9, 8), Inst::Branch { link: false, target: label(".Lf.3") }]);
    let exit = block(".Lf.3", vec![mov(0, 9), Inst::Ret { rn: Gpr(30) }]);
    let out = fold(vec![entry, early, work, exit], &mut [], &Default::default(), false, &[Reg::Gpr(Gpr(0))]);
    let jumps = out.iter().flat_map(|b| &b.insts).filter(|i| matches!(i, Inst::Branch { link: false, .. })).count();
    assert_eq!(jumps, 0, "{out:?}");
    let early = out.iter().find(|b| b.label == ".Lf.1").expect("the early block stays");
    assert_eq!(early.insts, vec![mov(0, 31), Inst::Ret { rn: Gpr(30) }], "{out:?}");
}

/// A block that falls into the short block keeps falling: copying into it
/// grew `recmany` by 8 B (the pin skips a fall-through predecessor).
#[test]
fn a_block_that_falls_into_the_return_does_not_take_a_copy() {
    let entry = block(
        ".Lf.0",
        vec![
            Inst::CompareBranch { nonzero: false, width: Width::X, rt: Gpr(1), target: label(".Lf.2") },
            Inst::Branch { link: false, target: label(".Lf.1") },
        ],
    );
    let work = block(".Lf.1", vec![Inst::Nop, Inst::Branch { link: false, target: label(".Lf.2") }]);
    let exit = block(".Lf.2", vec![mov(0, 9), Inst::Ret { rn: Gpr(30) }]);
    let out = fold(vec![entry, work, exit], &mut [], &Default::default(), false, &[Reg::Gpr(Gpr(0))]);
    let rets = out.iter().flat_map(|b| &b.insts).filter(|i| matches!(i, Inst::Ret { .. })).count();
    assert_eq!(rets, 1, "{out:?}");
}
