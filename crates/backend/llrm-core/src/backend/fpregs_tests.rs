use std::sync::Arc;

use super::*;
use crate::backend::allocate::RFP;
use crate::model::ir::{Mem, Operation};
use crate::model::lir::LirBlock;

fn fp(at: usize) -> Loc {
    Loc::Reg(Reg { register: RFP[at], width: 10 })
}

fn insn(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

fn cell() -> Loc {
    Loc::Mem(Mem::new(Some(crate::model::ir::Addr::new(crate::model::ir::Space::Frame, -8)), 8))
}

fn body(blocks: Vec<LirBlock>) -> LirBody {
    let mut made = LirBody::new("fp", 0, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
    made.registers = Some(Regs(&llrm_x86_m16::REGISTER_INFO));
    made
}

fn value(place: &Loc) -> u32 {
    match place {
        Loc::Held(held) => held.value,
        other => panic!("not a value: {other:?}"),
    }
}

/// A register read is the value last written to it: the stackifier follows
/// values, and a register rewritten while it is read would name one value
/// twice.
#[test]
fn test_a_register_read_is_the_value_last_written_to_it() {
    let mut first = LirBlock::new(
        0,
        vec![
            insn(0, Operation::FloatLoad, "fld", vec![fp(1)], vec![cell()]),
            insn(1, Operation::FloatArith, "fadd", vec![fp(1)], vec![fp(1), fp(0)]),
            insn(2, Operation::FloatStore, "fstp", vec![cell()], vec![fp(1)]),
        ],
    );
    first.succ = Vec::new().into();
    let made = valued(&body(vec![first])).expect("registers named");
    let what = |at: usize| made.blocks[0].insns[at].what.clone().unwrap();
    let loaded = value(&what(0).dests[0]);
    let sum = value(&what(1).dests[0]);
    assert_eq!(value(&what(1).sources[0]), loaded);
    assert_ne!(sum, loaded);
    assert_eq!(value(&what(2).sources[0]), sum);
    // fp0 is read with nothing written: its border value.
    assert_ne!(value(&what(1).sources[1]), loaded);
    assert_eq!(made.blocks[0].insns[1].uses, vec![loaded, value(&what(1).sources[1])]);
    assert_eq!(made.blocks[0].insns[1].defines, vec![sum]);
}

/// A register live out of a block ends it as one value, whatever wrote it: the
/// block it falls to reads that value, so the stack bookkeeping agrees at the
/// border.
#[test]
fn test_a_register_live_out_ends_the_block_as_its_border_value() {
    let mut first = LirBlock::new(0, vec![insn(0, Operation::FloatLoad, "fld", vec![fp(2)], vec![cell()])]);
    first.succ = vec![1].into();
    let second = LirBlock::new(1, vec![insn(1, Operation::FloatStore, "fstp", vec![cell()], vec![fp(2)])]);
    let made = valued(&body(vec![first, second])).expect("registers named");
    let written = value(&made.blocks[0].insns[0].what.as_ref().unwrap().dests[0]);
    let read = value(&made.blocks[1].insns[0].what.as_ref().unwrap().sources[0]);
    assert_eq!(written, read);
}

/// Nothing to rename where no x87 register is named.
#[test]
fn test_a_body_with_no_x87_register_is_left_alone() {
    let first = LirBlock::new(0, vec![insn(0, Operation::FloatLoad, "fld", vec![], vec![cell()])]);
    assert!(valued(&body(vec![first])).is_none());
}
