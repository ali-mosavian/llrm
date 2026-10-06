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

use crate::backend::peephole::{_branch_reads, _flag_lanes, _lanes, _moved_lanes, _register_effects, Lane, Lanes};
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

/// What one instruction does to register and flag lanes.
#[derive(Clone, Debug)]
pub struct Effect {
    /// Read whatever is live after it.
    pub reads: Lanes,
    pub writes: Lanes,
    /// A constant shift's bytes, `(written, source)`: a source is read only
    /// where its written byte, or a flag the shift sets, is live after it.
    moved: Vec<(Lane, Lane)>,
}

impl Effect {
    /// The lanes live before it, given those live after.
    pub fn live_before(&self, live: &Lanes) -> Lanes {
        live.minus(&self.writes).or(&self.read(|lane| live.contains(lane)))
    }

    /// The lanes dead before it, given those dead after.
    pub fn dead_before(&self, dead: &Lanes) -> Lanes {
        dead.or(&self.writes).minus(&self.read(|lane| !dead.contains(lane)))
    }

    fn read(&self, live: impl Fn(&Lane) -> bool) -> Lanes {
        let mut reads = self.reads;
        // A shift's flags come from the bits it moves.
        let flagged = self.writes.iter().any(|lane| lane.0 == Register::None && live(lane));
        for (written, source) in &self.moved {
            if flagged || live(written) {
                reads.insert(*source);
            }
        }
        reads
    }
}

/// `one`'s effect as it decodes, else as its contract declares; None when unknown.
pub fn effect(bits: u32, one: &Insn) -> Option<Effect> {
    if _terminator(one.what.as_ref()) {
        // A jump or branch writes nothing; a branch reads its flags.
        let what = one.what.as_ref().expect("a terminator has semantics");
        let reads = if what.op == Operation::Branch { _branch_reads(what) } else { Lanes::new() };
        return Some(Effect { reads, writes: Lanes::new(), moved: Vec::new() });
    }
    let (reads, writes) = _register_effects(bits, one, false, true).or_else(|| _declared(one))?;
    Some(match _moved_lanes(bits, one) {
        Some((moved, operands)) => Effect { reads: reads.minus(&operands), writes, moved },
        None => Effect { reads, writes, moved: Vec::new() },
    })
}

/// Each instruction's effect in `block`, decoded once for a fixed point to reuse.
fn _effects(bits: u32, block: &LirBlock) -> Vec<Option<Effect>> {
    block.insns.iter().map(|one| effect(bits, one)).collect()
}

/// The lanes live before `effects`, given those live after them. An unknown
/// instruction may read anything.
fn _before(effects: &[Option<Effect>], live: Lanes, universe: &Lanes) -> Lanes {
    effects.iter().rev().fold(live, |live, one| one.as_ref().map_or_else(|| *universe, |one| one.live_before(&live)))
}

/// The lanes live before `block`, given those live after it.
pub fn _backwards(bits: u32, block: &LirBlock, live: Lanes, universe: &Lanes) -> Lanes {
    _before(&_effects(bits, block), live, universe)
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
    // A callee runs on the caller's frame chain, stack and data group.
    if one.what.as_ref().is_some_and(|what| what.op == Operation::Call) {
        for register in _RETURN_STATE {
            reads.extend(_lanes(register));
        }
    }
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

thread_local! {
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out a block's lanes, for a test that a chain of blocks does not
/// take a round of the body for each.
pub fn visits() -> usize {
    VISITS.with(std::cell::Cell::get)
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
    let effects: IndexMap<i64, Vec<Option<Effect>>> = blocks.iter().map(|(at, block)| (*at, _effects(body.bits, block))).collect();
    // The least fixed point of a backward problem, found by a worklist that starts from the last block: a
    // block is recomputed when a successor's lanes changed. Taken round the layout from the first block, each
    // round carried a change one block back, and a chain of n blocks took n rounds.
    let mut predecessors: IndexMap<i64, Vec<i64>> = blocks.keys().map(|at| (*at, Vec::new())).collect();
    for (at, to) in &successors {
        for next in to {
            predecessors[next].push(*at);
        }
    }
    let mut pending: Vec<i64> = blocks.keys().copied().collect();
    let mut queued: crate::support::hash::HashSet<i64> = pending.iter().copied().collect();
    while let Some(at) = pending.pop() {
        VISITS.with(|visits| visits.set(visits.get() + 1));
        queued.remove(&at);
        let after = if successors[&at].is_empty() {
            universe.clone()
        } else {
            successors[&at].iter().flat_map(|to| into[to].iter().copied()).collect()
        };
        let before = _before(&effects[&at], after, &universe);
        if before != into[&at] {
            into.insert(at, before);
            for &pred in &predecessors[&at] {
                if queued.insert(pred) {
                    pending.push(pred);
                }
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

    /// A chain of blocks, the last reading AX and each before it writing a register: the lanes live into the
    /// first.
    fn chain(blocks: i64) -> LirBody {
        let bx = Reg { register: Register::BX, width: 2 };
        let blocks: Vec<LirBlock> = (1..=blocks)
            .map(|at| {
                let insns = if at == blocks { vec![_insn(at, Operation::Move, "mov", vec![Loc::Reg(DX)], vec![Loc::Reg(AX)])] } else { vec![_insn(at, Operation::Move, "mov", vec![Loc::Reg(bx)], vec![Loc::Reg(DX)])] };
                LirBlock { succ: if at == blocks { vec![] } else { vec![at + 1] }, ..LirBlock::new(at, insns) }
            })
            .collect();
        LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default())
    }

    /// Taken in layout order from the first block, each round of the fixed point carried a change one block
    /// back: a chain of n blocks was n rounds of n blocks, and peephole's `dead_at_exit` was 18% of compiling
    /// 800 blocks (#560). Starting from the last, each block is worked out about once.
    #[test]
    fn test_a_chain_of_blocks_is_not_a_round_of_the_body_for_each_block() {
        let body = chain(200);
        let before = super::visits();
        let (into, _successors, _universe) = live_into(&body);
        assert!(super::visits() - before <= 3 * 200, "{} block visits for 200 blocks", super::visits() - before);
        assert!(_lanes(Register::AX).is_subset(&into[&1]), "AX is read at the end of the chain and written nowhere before");
    }

    #[test]
    fn test_a_return_whose_reads_are_complete_reads_only_results_and_return_state() {
        // Every return read every register; one the raise wrote reads its
        // results and the registers the return itself needs.
        let cx = Reg { register: Register::CX, width: 2 };
        let mut ret = Insn::new(
            3,
            Some((3, 3)),
            Some(Semantics { name: Some(String::new()), ..Semantics::new(Operation::Return) }),
            vec![],
            vec![1],
        );
        ret.requires = vec![(Held { value: 1, width: 2 }, Register::AX)];
        ret.reads_complete = true;
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
