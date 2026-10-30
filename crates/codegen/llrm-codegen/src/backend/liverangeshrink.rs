//! Moves an instruction up to just after the last definition of its operands,
//! so the operands' live ranges end sooner. Translated from
//! [LiveRangeShrink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeShrink.cpp)
//! (`LiveRangeShrink`, x86 `addPreRegAlloc`, before PHI elimination, on SSA).
//!
//! # How it works
//! Each block is walked once from its first instruction, with an order number
//! for every instruction between two code motion barriers (`BuildInstOrderMap`;
//! a barrier is `Barrier` or `Escape`, here the pin's unmodeled side effect).
//! An instruction that is safe to move ([`safe_to_move`]), defines one value
//! and reads no register by name is a candidate. Each value it reads must be
//! read once in the function and defined once, in the class of its own value;
//! a value read that is the result of a copy does not count. The instruction
//! goes to just after the latest definition among them that is in the block
//! (`FindDominatedInstruction`), if more than one value read is counted: the
//! values it reads then end where it is, and the one it defines starts later.
//!
//! ```text
//!  before                      after
//!  a = load [p]                a = load [p]
//!  b = load [q]                b = load [q]
//!  x = ...                     s = add a, b    (a and b end here)
//!  y = ...                     x = ...
//!  s = add a, b                y = ...
//! ```
//!
//! MIRROR: the order map and its rebuild after a barrier, the single-use,
//! single-def and same-class test of each operand, the count of more than one
//! shortened range, the copy exemption, the landing after the latest
//! definition, and `isSafeToMove`'s store rule (a load stays below the stores
//! seen before it in its barrier-free stretch).
//!
//! DEVIATE LRS1: the pin asks `MRI.getRegClass`; here a value's class is the
//! set that [`crate::backend::regclass`] confines it to and its width, and the
//! x87 stack. An instruction that writes the flags moves only where no
//! instruction between the landing and its old place reads them before one
//! writes them (the pin: `EFLAGS` is a physical register, so an instruction
//! with a live flags def does not move), and one that reads them does not
//! move. Exit: none; the register and flag model of the LIR.
//!
//! DEVIATE LRS2 (owner w-8, review 2026-11-14): the pin's `COPY` from a
//! physical register is a value the entry delivers; a call's result is a `COPY`
//! there too, and counted here as a definition that shortens a range. Not
//! counting it grew recchop -O3 by 16 bytes. Exit: the allocation that makes
//! the call result's copy coalesce.
//!
//! # Example
//! ```text
//! b1: v1 = load [p]      b1: v1 = load [p]
//!     v2 = load [q]          v2 = load [q]
//!     v3 = mov 7             v4 = add v1, v2
//!     v4 = add v1, v2        v3 = mov 7
//! ```
//!
//! # Runs
//! A machine phase after `machinesink` and before `phielim`
//! (`flow::machine_with`), in every optimized machine.
//!
//! # Needs
//! Classes from [`crate::backend::regclass`], effects from
//! [`crate::backend::liveness`].

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::backend::regclass;
use crate::backend::target::Segments;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct LiveRangeShrink {
    pub segments: Segments,
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for LiveRangeShrink {
    fn class_name(&self) -> &'static str {
        "LiveRangeShrink"
    }

    fn name(&self) -> &str {
        "liverangeshrink"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(shrunk(&body, &self.segments, &self.classes))
    }
}

/// `isCodeMotionBarrier`: an instruction after which nothing may move above.
fn code_motion_barrier(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Barrier | Operation::Escape))
}

/// `MachineInstr::isSafeToMove`, as the LIR has it. A store, a call or an
/// effect seen sets `saw_store`; a load moves only while none has been.
fn safe_to_move(
    one: &Insn,
    saw_store: &mut bool,
) -> bool {
    let Some(what) = &one.what else { return false };
    let writes_memory = what.dests.iter().any(|place| matches!(place, Loc::Mem(_)));
    let reads_memory = what.sources.iter().any(|place| matches!(place, Loc::Mem(_)));
    let effects = writes_memory
        || one.volatile()
        || one.call.is_some()
        || matches!(
            what.op,
            Operation::Call
                | Operation::Branch
                | Operation::Jump
                | Operation::Return
                | Operation::Barrier
                | Operation::Escape
                | Operation::Fill
                | Operation::Copy
                | Operation::Push
                | Operation::Pop
                | Operation::Leave
                | Operation::Nothing
        );
    if effects {
        *saw_store |= writes_memory || one.call.is_some() || what.op != Operation::Nothing;
        return false;
    }
    !(reads_memory && *saw_store)
        && one.group.is_none()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
}

/// `TII.isCopyInstr`: a register-to-register move.
fn is_copy(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.op == Operation::Move
                && matches!(
                    (what.dests.as_slice(), what.sources.as_slice()),
                    ([Loc::Held(_)], [Loc::Held(_)])
                ),
        )
}

/// What a value is, for the same-class test: the set it is confined to, its
/// width, and whether it lives on the x87 stack.
type Class = (Option<BTreeSet<llrm_lir::registers::RegId>>, Option<u32>);

/// `body` with each instruction that shortens more than one live range made
/// just after the last definition of what it reads.
pub fn shrunk(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
) -> LirBody {
    let regs = body.regs();
    // Worked out when a candidate first asks: most bodies have none.
    let confined = std::cell::OnceCell::new();
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    let mut defined: IndexMap<u32, usize> = IndexMap::default();
    let mut read: IndexMap<u32, usize> = IndexMap::default();
    // The instruction of a block that defines a value, by the position it had.
    let mut def_site: IndexMap<u32, (usize, usize)> = IndexMap::default();
    // The phi results of each block: defined before its first instruction.
    let mut phi_site: IndexMap<u32, usize> = IndexMap::default();
    for (at, block) in body.blocks.iter().enumerate() {
        for phi in block.phis.iter() {
            *defined.entry(phi.result).or_default() += 1;
            phi_site.insert(phi.result, at);
            for &(_, value) in &phi.incoming {
                *read.entry(value).or_default() += 1;
            }
        }
        for (index, one) in block.insns.iter().enumerate() {
            for value in &one.defines {
                *defined.entry(*value).or_default() += 1;
                def_site.insert(*value, (at, index));
            }
            for value in &one.uses {
                *read.entry(*value).or_default() += 1;
            }
            if let Some(what) = &one.what {
                for place in what.dests.iter().chain(&what.sources) {
                    if let Loc::Held(held) = place {
                        widths.insert(held.value, held.width);
                    }
                }
            }
        }
    }
    let class_of = |value: u32| -> Class {
        let confined = confined.get_or_init(|| regclass::classes(body, &BTreeSet::new(), segments, registers));
        (confined.get(&value).cloned(), widths.get(&value).copied())
    };
    let mut changed = false;
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .enumerate()
        .map(|(block_at, block)| {
            let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
            if insns.is_empty() {
                return block.clone();
            }
            // `ident[position]` is the index an instruction had at the start.
            let mut ident: Vec<usize> = (0..insns.len()).collect();
            let mut position: Vec<usize> = (0..insns.len()).collect();
            let mut order: Vec<Option<u32>> = vec![None; insns.len()];
            let build = |from: usize, ident: &[usize], insns: &[Arc<Insn>], order: &mut Vec<Option<u32>>| {
                order.iter_mut().for_each(|slot| *slot = None);
                let mut next = 0;
                for at in from..insns.len() {
                    if code_motion_barrier(&insns[at]) {
                        break;
                    }
                    order[ident[at]] = Some(next);
                    next += 1;
                }
            };
            build(0, &ident, &insns, &mut order);
            let mut saw_store = false;
            let mut at = 0;
            while at < insns.len() {
                let me = ident[at];
                let one = Arc::clone(&insns[at]);
                if !safe_to_move(&one, &mut saw_store) {
                    // A barrier: later instructions must not move above it.
                    if code_motion_barrier(&one) && at + 1 < insns.len() {
                        build(at + 1, &ident, &insns, &mut order);
                        saw_store = false;
                    }
                    at += 1;
                    continue;
                }
                let target =
                    landing(&one, body, &position, &order, block_at, &def_site, &phi_site, &defined, &read, &class_of);
                at += 1;
                let Some(after) = target else { continue };
                let place = if after == PHI { 0 } else { position[after] + 1 };
                if place >= position[me]
                    || !flags_allow(regs, body.bits, &insns, place, position[me], !block.succ.is_empty())
                {
                    continue;
                }
                // The instruction takes the position of the one it lands
                // before: the stretch keeps its order numbers non-decreasing.
                let mut moved = (*insns.remove(position[me])).clone();
                moved.at = insns[place].at;
                insns.insert(place, Arc::new(moved));
                let carried = ident.remove(position[me]);
                ident.insert(place, carried);
                for (index, id) in ident.iter().enumerate() {
                    position[*id] = index;
                }
                order[me] = order[ident[place + 1]];
                changed = true;
            }
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The instruction (by its first index) a candidate goes after, if it should
/// move: the latest definition in the stretch of what it reads, when more than
/// one value read is counted.
#[allow(clippy::too_many_arguments)]
fn landing(
    one: &Insn,
    body: &LirBody,
    position: &[usize],
    order: &[Option<u32>],
    block_at: usize,
    def_site: &IndexMap<u32, (usize, usize)>,
    phi_site: &IndexMap<u32, usize>,
    defined: &IndexMap<u32, usize>,
    read: &IndexMap<u32, usize>,
    class_of: &dyn Fn(u32) -> Class,
) -> Option<usize> {
    let [result] = one.defines.as_slice() else { return None };
    if defined.get(result) != Some(&1) {
        return None;
    }
    let mut insert: Option<usize> = None;
    let mut eligible = 0;
    for value in &one.uses {
        if read.get(value) != Some(&1) || defined.get(value) != Some(&1) || class_of(*value) != class_of(*result) {
            return None;
        }
        // A phi of this block is the first thing in its order map.
        if phi_site.get(value) == Some(&block_at) {
            eligible += 1;
            insert = insert.or(Some(PHI));
            continue;
        }
        // A value with no definition here (an input) is defined outside any
        // stretch.
        let Some(&(block, def)) = def_site.get(value) else {
            eligible += 1;
            continue;
        };
        // A value the entry delivers in a register is the pin's `COPY` from
        // that physical register. A call's result is the pin's
        // `COPY` too, and counted here: not counting it grew recchop -O3 by 16
        // bytes (DEVIATE LRS2).
        let site = &body.blocks[block].insns[def];
        if !is_copy(site) && (site.call.is_some() || !site.delivers.iter().any(|(held, _)| held.value == *value)) {
            eligible += 1;
        }
        // The pin's `COPY` of the result follows the call frame's close, which
        // is a barrier: it is in the stretch after it, not before. A call
        // behind the barrier is defined at the stretch's first instruction.
        let def = match order.get(def) {
            Some(None) if block == block_at && site.call.is_some() => {
                order.iter().position(|slot| *slot == Some(0)).unwrap_or(def)
            }
            _ => def,
        };
        insert = dominated(block, block_at, def, insert, position, order);
    }
    (eligible > 1).then_some(insert?)
}

/// Stands for the phis of a block as the instruction a landing follows.
const PHI: usize = usize::MAX;

/// `FindDominatedInstruction`: `old`, or the definition `def` of this block if
/// it comes later in the order.
fn dominated(
    block: usize,
    block_at: usize,
    def: usize,
    old: Option<usize>,
    position: &[usize],
    order: &[Option<u32>],
) -> Option<usize> {
    if block != block_at {
        return old;
    }
    let Some(new_order) = order[def] else { return old };
    let Some(old) = old else { return Some(def) };
    if old == PHI {
        return Some(def);
    }
    let old_order = order[old].expect("a chosen definition is in the order map");
    if old_order != new_order {
        return Some(if old_order < new_order { def } else { old });
    }
    // The same number: `def` is dominated by `old` when it follows it.
    if position[def] > position[old] { Some(def) } else { Some(old) }
}

/// Whether an instruction that goes to `place` from `from` may. It does not
/// read the flags. If it writes them, no instruction after it reads them before
/// one writes them (the block's exits read them when none does and `exits`),
/// and none from `place` to it reads them before one writes them.
fn flags_allow(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    insns: &[Arc<Insn>],
    place: usize,
    from: usize,
    exits: bool,
) -> bool {
    let flags = |lanes: &crate::backend::lanes::Lanes| {
        lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
    };
    let Some(effect) = liveness::effect(regs, bits, &insns[from]) else { return false };
    if flags(&effect.all_reads()) {
        return false;
    }
    if !flags(&effect.writes) {
        return true;
    }
    for later in &insns[from + 1..] {
        // No flag is live into a return.
        if later.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
            break;
        }
        let Some(effect) = liveness::effect(regs, bits, later) else { return false };
        if flags(&effect.all_reads()) {
            return false;
        }
        if flags(&effect.writes) {
            break;
        }
        if std::ptr::eq(later, insns.last().expect("a block with a later instruction")) && exits {
            return false;
        }
    }
    for between in &insns[place..from] {
        let Some(effect) = liveness::effect(regs, bits, between) else { return false };
        if flags(&effect.all_reads()) {
            return false;
        }
        if flags(&effect.writes) {
            return true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::*;
    use crate::model::ir::{Held, Imm, Semantics};
    use crate::model::lir::{Insn, LirBlock};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    fn constant(
        at: i64,
        into: u32,
    ) -> Arc<Insn> {
        insn(
            at,
            Operation::Move,
            "mov",
            vec![held(into)],
            vec![Loc::Imm(Imm { value: i64::from(into), width: 4, address: None })],
            vec![into],
            vec![],
        )
    }

    fn add(
        at: i64,
        into: u32,
        left: u32,
        right: u32,
    ) -> Arc<Insn> {
        insn(
            at,
            Operation::Binary,
            "add",
            vec![held(into)],
            vec![held(left), held(right)],
            vec![into],
            vec![left, right],
        )
    }

    fn ret(at: i64) -> Arc<Insn> {
        insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![])
    }

    fn body(insns: Vec<Arc<Insn>>) -> LirBody {
        let mut body =
            LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    fn defines(body: &LirBody) -> Vec<u32> {
        body.blocks[0].insns.iter().flat_map(|one| one.defines.clone()).collect()
    }

    fn shrink(body: &LirBody) -> LirBody {
        let registers = RegisterClasses::of(&llrm_x86_m16::M16);
        shrunk(body, &crate::backend::target::BUILT_IN, &registers)
    }

    /// `s = a + b` after two unrelated definitions held a and b across them:
    /// the sum goes up to just after b (LiveRangeShrink.cpp, the whole pass).
    #[test]
    fn test_a_sum_of_two_single_use_values_goes_up_to_the_later_one() {
        let before = body(vec![constant(0, 10), constant(1, 11), constant(2, 12), add(3, 13, 10, 11), ret(4)]);
        assert_eq!(defines(&shrink(&before)), [10, 11, 13, 12]);
    }

    /// A value that is a copy does not count: one counted range is not more
    /// than one, and nothing moves.
    #[test]
    fn test_a_copy_does_not_count_as_a_shortened_range() {
        let copy = insn(0, Operation::Move, "mov", vec![held(10)], vec![held(1)], vec![10], vec![1]);
        let before = body(vec![copy, constant(1, 11), constant(2, 12), add(3, 13, 10, 11), ret(4)]);
        assert_eq!(defines(&shrink(&before)), [10, 11, 12, 13]);
    }

    /// A value the entry delivers in a register is the pin's `COPY` from a
    /// physical register: it does not count either. `a + b` of two
    /// arguments was hoisted above the sum of the other two, which made the
    /// two-address pass see the `lea` first and left `lea ecx,[ebx+ecx]` and
    /// `add` apart in six_.
    #[test]
    fn test_a_value_delivered_in_a_register_does_not_count_as_a_shortened_range() {
        let entry = Arc::new(Insn {
            delivers: vec![(Held { value: 10, width: 4 }, RegId::EAX), (Held { value: 11, width: 4 }, RegId::EDX)],
            ..Insn::new(0, None, None, vec![10, 11], vec![])
        });
        let before = body(vec![entry, constant(1, 12), add(2, 13, 10, 11), ret(3)]);
        assert_eq!(defines(&shrink(&before)), [10, 11, 12, 13]);
    }

    /// A value read twice is not shortened by the move.
    #[test]
    fn test_a_value_read_twice_keeps_the_instruction_where_it_is() {
        let before = body(vec![
            constant(0, 10),
            constant(1, 11),
            constant(2, 12),
            add(3, 13, 10, 11),
            add(4, 14, 10, 12),
            ret(5),
        ]);
        assert_eq!(defines(&shrink(&before)), [10, 11, 12, 13, 14]);
    }

    /// Nothing moves above a barrier: the definitions before it are not in the
    /// order map after it.
    #[test]
    fn test_nothing_moves_above_a_barrier() {
        let barrier = insn(2, Operation::Barrier, "asm", vec![], vec![], vec![], vec![]);
        let before = body(vec![constant(0, 10), constant(1, 11), barrier, constant(3, 12), add(4, 13, 10, 11), ret(5)]);
        assert_eq!(defines(&shrink(&before)), [10, 11, 12, 13]);
    }

    /// The `add` after a `cmp` whose branch reads the `add`'s flags stays below
    /// the `cmp`: the branch must read the flags of the last writer.
    #[test]
    fn test_an_instruction_whose_flags_a_branch_reads_does_not_cross_a_compare() {
        let cmp = insn(2, Operation::Compare, "cmp", vec![], vec![held(1), held(2)], vec![], vec![1, 2]);
        let branch = insn(4, Operation::Branch, "je", vec![], vec![], vec![], vec![]);
        let before = body(vec![constant(0, 10), constant(1, 11), cmp, add(3, 13, 10, 11), branch, ret(5)]);
        let names: Vec<_> = shrink(&before).blocks[0]
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            .collect();
        assert_eq!(names, ["mov", "mov", "cmp", "add", "je", "ret"]);
    }
}
