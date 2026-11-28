//! A block that ends in a call that never returns runs into nothing.

use llrm_arm64::cfi::rows;
use llrm_arm64::frame::Lowered;
use llrm_arm64::mc::inst::{Cond, Gpr, Inst, Target};
use llrm_object::assembler::Cfi;

fn block(
    label: &str,
    insts: Vec<Inst>,
) -> Lowered {
    let locs = vec![None; insts.len()];
    Lowered { label: label.into(), insts, locs }
}

/// A frameless return placed after a cold `bl fault` took the frame the call
/// block had pushed: its unwind row said CFA = sp + 16 where the frame was not
/// made.
#[test]
fn a_block_after_a_noreturn_call_does_not_inherit_its_frame() {
    let entry = block("entry", vec![Inst::BranchCond { cond: Cond::Hs, target: Target::Label("mid".into()) }]);
    let cold = block(
        "cold",
        vec![
            Inst::Cfi(Cfi::DefCfaOffset { offset: 16 }),
            Inst::Branch { link: true, target: Target::Symbol { name: "fault".into(), addend: 0 } },
        ],
    );
    let done = block("done", vec![Inst::Ret { rn: Gpr(30) }]);
    let mid = block("mid", vec![Inst::Branch { link: false, target: Target::Label("done".into()) }]);
    let rows_of = |dead_ends: &[String]| {
        rows(&[entry.clone(), cold.clone(), done.clone(), mid.clone()], dead_ends)
            .into_iter()
            .map(|row| (row.offset, row.cfa_offset))
            .collect::<Vec<_>>()
    };
    // The frame is made at 4 and `done`, at 8, is outside it.
    assert_eq!(rows_of(&["cold".into()]), [(4, 16), (8, 0)]);
    // The control: without the fact `done` takes the call block's frame, as
    // far as `mid`.
    assert_eq!(rows_of(&[]), [(4, 16), (12, 0)]);
}
