//! Port of `qbopt/backend/liveness.py`: which physical register and flag
//! lanes are dead on exit from each block.
//!
//! A backward walk inside one block starts by assuming everything is live, so a
//! copy written as the last instruction of a block always survives it -- and a
//! parallel copy for a phi is written exactly there. deedlines' plasmablobs ends
//! its inner loop with `mov di,bx` whose destination no path reads before writing
//! it again.

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::backend::peephole::{_branch_reads, _flag_lanes, _lanes, _register_effects, Lanes};
use crate::backend::target;
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};

pub fn _terminator(what: Option<&Semantics>) -> bool {
    what.is_some_and(|what| {
        [Operation::Branch, Operation::Jump].contains(&what.op)
            && what.dests.is_empty()
            && what.sources.is_empty()
            && what.target.is_some()
    })
}

// Machine state a generated return itself needs beside its explicit results.
// SI and DI are callee-saved at the ABI boundary, but their incoming values
// live in the prologue's stack saves, not in transient body registers.  The
// MASM emitter derives push/pop preservation from surviving body uses, so a
// dead final write to either register must remain removable.  Treating them as
// semantic return inputs retained one-use loads and other dead computations
// immediately before an epilogue.
pub const _RETURN_STATE: [Register; 5] = [Register::EBP, Register::ESP, Register::DS, Register::SS, Register::CS];

/// Every lane a body can name. "Dead" here means every lane but the live ones.
pub fn _universe() -> Lanes {
    let mut lanes = _flag_lanes(0xFFFF_FFFF);
    for register in target::WIDTHS.keys().chain(target::SEGMENTS.iter()) {
        lanes.extend(_lanes(*register));
    }
    lanes
}

/// The lanes live before `block`, given those live after it.
pub fn _backwards(block: &LirBlock, live: Lanes, universe: &Lanes) -> Lanes {
    let (read, written) = _transfer(block, universe);
    live.minus(&written).or(&read)
}

/// What `block` reads before writing, and what it writes: the lanes live
/// before it are the first and those live after it less the second.
/// Decoding an instruction is the cost, so a fixed point asks this once.
fn _transfer(block: &LirBlock, universe: &Lanes) -> (Lanes, Lanes) {
    let (mut read, mut written) = (Lanes::new(), Lanes::new());
    for one in block.insns.iter().rev() {
        if _terminator(one.what.as_ref()) {
            // Its flag read is not in `_register_effects`, which answers only
            // for instructions that fall through. It writes nothing.
            let what = one.what.as_ref().expect("a terminator has semantics");
            if what.op == Operation::Branch {
                read = read.or(&_branch_reads(what));
            }
            continue;
        }
        let mut effects = _register_effects(one, false, true);
        if effects.is_none() {
            effects = _declared(one);
        }
        let Some((reads, writes)) = effects else {
            // It may read anything, but what the block writes before it is still written first.
            read = universe.clone();
            written = universe.clone();
            continue;
        };
        read = read.minus(&writes).or(&reads);
        written = written.or(&writes);
    }
    (read, written)
}

/// What a call says it reads and writes, for an instruction no decoder covers.
///
/// `requires` and `clobbers` are the contract the allocation is already built
/// on, so reading a call as touching every register only makes a register the
/// callee never names look live -- which kept every value a phi copies alive
/// across the whole loop.
pub fn _declared(one: &Insn) -> Option<(Lanes, Lanes)> {
    let held_lanes = |held: &Held, register: Register| _lanes(target::named(register, i64::from(held.width)));

    if one.what.as_ref().is_some_and(|what| what.op == Operation::Return)
        && one.reads_complete()
    {
        // Nothing runs after it: it reads explicit results and only the
        // architectural state its generated epilogue itself needs.
        let mut reads: Lanes = one.requires.iter().flat_map(|(held, register)| held_lanes(held, *register)).collect();
        for register in _RETURN_STATE {
            reads.extend(_lanes(register));
        }
        let writes = _universe().minus(&reads);
        return Some((reads, writes));
    }
    if one.clobbers.is_empty() || one.symbol == Some(true) {
        return None;
    }
    let mut reads: Lanes = one.requires.iter().flat_map(|(held, register)| held_lanes(held, *register)).collect();
    // A transfer's decoded effects are unavailable, but its explicit operands
    // are still real reads. In particular an indirect `call bx` reads BX
    // before the calling convention clobbers it.
    for source in one.what.as_ref().map_or(&[][..], |what| what.sources.as_slice()) {
        match source {
            Loc::Reg(source) => reads.extend(_lanes(source.register)),
            Loc::Mem(source) => {
                reads.extend(_lanes(source.through));
                reads.extend(_lanes(source.index_through));
                // `selector` is a `Held`, never an `ir.Reg`.
            }
            Loc::Address(source) => {
                reads.extend(_lanes(source.through));
                reads.extend(_lanes(source.index));
            }
            _ => {}
        }
    }
    let mut writes: Lanes = one.delivers.iter().flat_map(|(held, register)| held_lanes(held, *register)).collect();
    for register in &one.clobbers {
        writes.extend(_lanes(*register));
    }
    for register in &one.clobbers_high {
        writes.extend(_lanes(*register).into_iter().filter(|lane| lane.1 >= 2));
    }
    writes.extend(_flag_lanes(0xFFFF_FFFF));
    Some((reads, writes))
}

/// Per block, the lanes live on entry -- with its successors and the universe.
pub fn live_into(body: &LirBody) -> (IndexMap<i64, Lanes>, IndexMap<i64, Vec<i64>>, Lanes) {
    let universe = _universe();
    let at_of: crate::support::hash::HashSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    let successors: IndexMap<i64, Vec<i64>> = body
        .blocks
        .iter()
        .map(|block| (block.at, block.succ.iter().copied().filter(|at| at_of.contains(at)).collect()))
        .collect();
    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut into: IndexMap<i64, Lanes> = blocks.keys().map(|at| (*at, Lanes::new())).collect();
    let transfer: IndexMap<i64, (Lanes, Lanes)> = blocks.iter().map(|(at, block)| (*at, _transfer(block, &universe))).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for at in blocks.keys() {
            let after = if successors[at].is_empty() {
                universe.clone()
            } else {
                successors[at].iter().flat_map(|to| into[to].iter().copied()).collect()
            };
            let (read, written) = &transfer[at];
            let before = after.minus(written).or(read);
            if before != into[at] {
                into.insert(*at, before);
                changing = true;
            }
        }
    }
    (into, successors, universe)
}

/// Per block, the lanes nothing reads again after it.
pub fn dead_at_exit(body: &LirBody) -> IndexMap<i64, Lanes> {
    let (into, successors, universe) = live_into(body);
    into.keys()
        .map(|at| {
            let live: Lanes = if successors[at].is_empty() {
                universe.clone()
            } else {
                successors[at].iter().flat_map(|to| into[to].iter().copied()).collect()
            };
            (*at, universe.minus(&live))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::live_into;
    use crate::backend::peephole::_lanes;
    use crate::model::ir::{Held, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::model::mir::{self, Kind, OpCode};

    const AX: Reg = Reg { register: Register::AX, width: 2 };
    const DX: Reg = Reg { register: Register::DX, width: 2 };

    fn _insn(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }),
            vec![],
            vec![],
        ))
    }

    #[test]
    fn test_a_register_written_before_an_unknown_instruction_is_not_live_into_its_block() {
        // A return, which nothing decodes, made its whole block live-in for
        // every lane: the `mov edx,eax` before it did not count.
        let block = LirBlock::new(
            1,
            vec![
                _insn(1, Operation::Move, "mov", vec![Loc::Reg(DX)], vec![Loc::Reg(AX)]),
                _insn(2, Operation::Return, "ret", vec![], vec![]),
            ],
        );
        let body = LirBody::new("f", 1, vec![block], IndexMap::default(), IndexMap::default());
        let (into, _successors, _universe) = live_into(&body);
        assert!(_lanes(Register::DX).is_disjoint(&into[&1]));
        assert!(_lanes(Register::AX).is_subset(&into[&1]));
    }

    #[test]
    fn test_a_return_whose_reads_are_complete_reads_only_results_and_return_state() {
        // Every return read every register; one the raise wrote reads its
        // results and the registers the return itself needs.
        let cx = Reg { register: Register::CX, width: 2 };
        let mut returned = mir::Op::new(3, OpCode::Operation(Operation::Return), "", vec![], vec![]);
        returned.kind = Kind::Return;
        returned.reads_complete = true;
        let mut ret = Insn::new(
            3,
            Some((3, 3)),
            Some(Semantics { name: Some(String::new()), ..Semantics::new(Operation::Return) }),
            vec![],
            vec![1],
        );
        ret.requires = vec![(Held { value: 1, width: 2 }, Register::AX)];
        ret.op = Some(Arc::new(returned));
        let block = LirBlock::new(
            1,
            vec![_insn(1, Operation::Move, "mov", vec![Loc::Reg(cx)], vec![Loc::Reg(AX)]), Arc::new(ret)],
        );
        let body = LirBody::new("f", 1, vec![block], IndexMap::default(), IndexMap::default());
        let (into, _successors, _universe) = live_into(&body);
        assert!(_lanes(Register::AX).is_subset(&into[&1]));
        assert!(_lanes(Register::SI).is_disjoint(&into[&1]));
        assert!(_lanes(Register::BP).is_subset(&into[&1]));
        assert!(_lanes(Register::DX).is_disjoint(&into[&1]));
    }
}
