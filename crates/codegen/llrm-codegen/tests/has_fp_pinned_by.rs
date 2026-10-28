//! `hasFP` reads `MachineFrameInfo`: what pins a frame register is one named
//! fact each. Before, `without_frame_register` said only "no" for all of them
//! (a `sub esp, reg` and an `Escape` were the same bare `false`).

use std::sync::Arc;

use llrm_codegen::backend::framefree::{Pinned, has_fp, pinned_by};
use llrm_codegen::model::ir::{Loc, Operation, Semantics};
use llrm_codegen::model::lir::{Block, Insn, LirBlock, LirBody};
use llrm_codegen::support::hash::IndexMap;
use llrm_target::{FrameRegisters, Target};

fn body_of(what: Semantics) -> LirBody {
    let insns = vec![Arc::new(Insn::new(0, Some((0, 1)), Some(what), vec![], vec![]))];
    LirBody {
        bits: 32,
        registers: Some(llrm_codegen::backend::registerinfo::Regs(llrm_x86_m32::M32.registers())),
        ..LirBody::new("f", 0, vec![Block::from(LirBlock::new(0, insns))], IndexMap::default(), IndexMap::default())
    }
}

fn pinned(
    what: Semantics,
    registers: &FrameRegisters,
) -> Option<Pinned> {
    pinned_by(&body_of(what), registers, &Default::default())
}

#[test]
fn test_each_fact_that_keeps_the_frame_register_is_named() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let reg = |register| Loc::Reg(llrm_codegen::model::ir::Reg { register, width: 4 });
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
    let word = Loc::Imm(llrm_codegen::model::ir::Imm { value: 8, width: 4, address: None });
    assert_eq!(pinned(sub_sp(word), &registers), None, "sub esp, 8 is a depth the walk follows");
}

#[test]
fn test_inline_code_keeps_the_frame_register() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let body = body_of(Semantics::new(Operation::Nothing));
    assert!(!has_fp(&body, &registers, &Default::default(), false, false, false));
    assert!(has_fp(&body, &registers, &Default::default(), true, false, false));
}

/// A call frame the machine passes have not lowered yet (`call_frame_setup`
/// and `call_frame_destroy`) was read as an opaque stack adjustment: every
/// m32 function with a call kept `ebp` from the allocator (QCport m32 -O2
/// +0.70%).
#[test]
fn test_call_frame_pseudos_are_a_depth_the_walk_follows() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let imm = |value| Loc::Imm(llrm_codegen::model::ir::Imm { value, width: 4, address: None });
    let pseudo =
        |name: &str, sources| Semantics { name: Some(name.into()), sources, ..Semantics::new(Operation::Barrier) };
    let insns =
        [pseudo("call_frame_setup", vec![imm(8), imm(0), imm(0)]), pseudo("call_frame_destroy", vec![imm(8), imm(0)])]
            .into_iter()
            .enumerate()
            .map(|(at, what)| {
                Arc::new(Insn::new(at as i64, Some((at as i64, at as i64 + 1)), Some(what), vec![], vec![]))
            })
            .collect();
    let body = LirBody {
        bits: 32,
        registers: Some(llrm_codegen::backend::registerinfo::Regs(llrm_x86_m32::M32.registers())),
        ..LirBody::new("f", 0, vec![Block::from(LirBlock::new(0, insns))], IndexMap::default(), IndexMap::default())
    };
    assert_eq!(pinned_by(&body, &registers, &Default::default()), None);
}

/// Tuned for size, a pushed value kept the frame register (`pinned_by` took a
/// `size` flag); the pin's `hasFP` has no such rule, so m32 -Os kept `ebp`
/// from the allocator in every function that pushed an argument (QCport -Os
/// 262,607 B against 267,607 B without the rule, a size row).
#[test]
fn test_a_pushed_value_does_not_keep_the_frame_register() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let held = Loc::Held(llrm_codegen::model::ir::Held { value: 1, width: 4 });
    let push = Semantics { name: Some("push".into()), sources: vec![held], ..Semantics::new(Operation::Push) };
    assert_eq!(pinned(push, &registers), None);
}

/// An indirect jump through a jump table was read as an opaque stack
/// adjustment: a switch-ending function kept `ebp` (QCport -Os weapons.c
/// `_pl_select_weapon`, +52 instructions over the pin).
#[test]
fn test_a_jump_table_is_not_an_opaque_sp_adjustment() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let jump = |table| Semantics { indirect: true, table, ..Semantics::new(Operation::Jump) };

    assert_eq!(pinned(jump(vec![]), &registers), Some(Pinned::OpaqueSpAdjustment), "premise: a bare indirect jump");
    assert_eq!(pinned(jump(vec![1, 2]), &registers), None);
}

/// An indirect call inside a call frame was read as an opaque stack adjustment
/// ("the callee may pop"): `long through(long x) { return chosen(x + 1); }`
/// kept `ebp` (tail_calls_edges, 3 instructions over the pin). The frame's
/// pseudos name the bytes the callee pops.
#[test]
fn test_an_indirect_call_in_a_call_frame_is_a_depth_the_walk_follows() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let imm = |value| Loc::Imm(llrm_codegen::model::ir::Imm { value, width: 4, address: None });
    let pseudo =
        |name: &str, sources| Semantics { name: Some(name.into()), sources, ..Semantics::new(Operation::Barrier) };
    let call = Semantics { indirect: true, ..Semantics::new(Operation::Call) };
    let body = |whats: Vec<Semantics>| {
        let insns = whats
            .into_iter()
            .enumerate()
            .map(|(at, what)| {
                Arc::new(Insn::new(at as i64, Some((at as i64, at as i64 + 1)), Some(what), vec![], vec![]))
            })
            .collect();
        LirBody {
            bits: 32,
            registers: Some(llrm_codegen::backend::registerinfo::Regs(llrm_x86_m32::M32.registers())),
            ..LirBody::new("f", 0, vec![Block::from(LirBlock::new(0, insns))], IndexMap::default(), IndexMap::default())
        }
    };
    let framed = body(vec![
        pseudo("call_frame_setup", vec![imm(4), imm(0), imm(0)]),
        call.clone(),
        pseudo("call_frame_destroy", vec![imm(4), imm(0)]),
    ]);
    assert_eq!(
        pinned_by(&body(vec![call]), &registers, &Default::default()),
        Some(Pinned::OpaqueSpAdjustment),
        "premise: no frame"
    );
    assert_eq!(pinned_by(&framed, &registers, &Default::default()), None);
}
