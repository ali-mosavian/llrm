//! `hasFP` reads `MachineFrameInfo`: what pins a frame register is one named
//! fact each. Before, `without_frame_register` said only "no" for all of them
//! (a `sub esp, reg` and an `Escape` were the same bare `false`).

use std::sync::Arc;

use llrm_core::backend::framefree::{Pinned, has_fp, pinned_by};
use llrm_core::model::ir::{Loc, Operation, Semantics};
use llrm_core::model::lir::{Block, Insn, LirBlock, LirBody};
use llrm_core::support::hash::IndexMap;
use llrm_target::{FrameRegisters, Target};

fn body_of(what: Semantics) -> LirBody {
    let insns = vec![Arc::new(Insn::new(0, Some((0, 1)), Some(what), vec![], vec![]))];
    LirBody {
        bits: 32,
        registers: Some(llrm_core::backend::registerinfo::Regs(llrm_x86_m32::M32.registers())),
        ..LirBody::new("f", 0, vec![Block::from(LirBlock::new(0, insns))], IndexMap::default(), IndexMap::default())
    }
}

fn pinned(
    what: Semantics,
    registers: &FrameRegisters,
) -> Option<Pinned> {
    pinned_by(&body_of(what), registers, &Default::default(), false)
}

#[test]
fn test_each_fact_that_keeps_the_frame_register_is_named() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let reg = |register| Loc::Reg(llrm_core::model::ir::Reg { register, width: 4 });
    let sub_sp = |amount| Semantics {
        name: Some("sub".into()),
        dests: vec![reg(registers.stack)],
        sources: vec![reg(registers.stack), amount],
        ..Semantics::new(Operation::Binary)
    };
    let (other, _) = registers.saved[0];

    assert_eq!(pinned(Semantics::new(Operation::Nothing), &registers), None, "premise: a nop pins nothing");
    assert_eq!(pinned(Semantics::new(Operation::Leave), &registers), Some(Pinned::FrameAddressTaken));
    assert_eq!(pinned(Semantics::new(Operation::Escape), &registers), Some(Pinned::OpaqueSpAdjustment));
    assert_eq!(
        pinned(Semantics { indirect: true, ..Semantics::new(Operation::Call) }, &registers),
        Some(Pinned::OpaqueSpAdjustment)
    );
    assert_eq!(pinned(sub_sp(reg(other)), &registers), Some(Pinned::VarSizedObjects), "sub esp, reg");
    let word = Loc::Imm(llrm_core::model::ir::Imm { value: 8, width: 4, address: None });
    assert_eq!(pinned(sub_sp(word), &registers), None, "sub esp, 8 is a depth the walk follows");
}

#[test]
fn test_inline_code_keeps_the_frame_register() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let body = body_of(Semantics::new(Operation::Nothing));
    assert!(!has_fp(&body, &registers, &Default::default(), false, false, false, false));
    assert!(has_fp(&body, &registers, &Default::default(), true, false, false, false));
}
