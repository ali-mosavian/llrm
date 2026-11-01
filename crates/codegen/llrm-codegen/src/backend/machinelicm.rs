//! Early `MachineLICM`: a pure computation of values defined outside a loop,
//! made in the loop, is made once in its preheader. Translated from
//! [MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp)
//! (`HoistOutOfLoop` L827, `IsLICMCandidate` L1098, `IsLoopInvariantInst`
//! L1133, `IsProfitableToHoist` L1273, `CanCauseHighRegPressure` L1235,
//! `Hoist` L1615).
//!
//! # How it works
//! The loops come inner first. A loop with one block outside it that enters
//! the header, and that block leaving only for the header (its preheader),
//! is walked in layout order, and again until nothing moves. An instruction
//! moves to the end of the preheader, before its jump, when:
//!
//! - it is a pure operation on registers and constants with one result, no
//!   pinned operand, no call, no x87 value (`candidate`), an `fld1`/`fldz` in a
//!   loop with no call and a free stack slot, in a function that never writes
//!   the control word (`constant_load`), or a load in a block that runs on
//!   every trip of a loop with no store, call or volatile access
//!   (`InitializeLoadsHoistableLoops` L1472, `IsGuaranteedToExecute`);
//! - every value it reads is defined outside the loop or was hoisted
//!   (`IsLoopInvariantInst`);
//! - it is not as cheap as a move (`IsProfitableToHoist`: a cheap instruction
//!   is hoisted only if it does not raise the pressure, and a constant cannot
//!   be made again where the allocator wants it, DEVIATE E3 of `precse`);
//! - it sets no flags a later instruction of its block reads;
//! - the loop does not already use as many values of its register class as the
//!   target has registers (`CanCauseHighRegPressure`, with the values named in
//!   the loop standing for the pressure: DEVIATE ML1, the pin tracks the live
//!   registers of each block of the loop).
//!
//! Not ported, priced at no yield (release/next 34d21c9f34, every C, nib and
//! BASIC program at -Os and -O2, counters in a throwaway build):
//! `getOrCreatePreheader` splitting an edge: the 6 to 32 loops per language
//! and level without a preheader hold 0 hoistable instructions;
//! `ExtractHoistableLoad`: 0 instructions, none of the 158 loop instructions
//! flagged invariant (`isDereferenceableInvariantLoad`) is a folded load;
//! `MayCSE`: only reached in the pin's high-pressure branch for an instruction
//! outside a block that runs every trip, where the `constant` path above
//! already hoists what the allocator can make again.
//!
//! # Example
//! A loop that adds `a[i] * k3` where `k3 := k + 3` is made inside it (the
//! sum is made by instruction selection, after the IR's LICM has run). The
//! sum is made once, ahead of the loop:
//!
//! ```text
//!   pre:                              pre:
//!     jmp loop                          v9 := add k, 0x3
//!   loop:                       =>      jmp loop
//!     v9 := add k, 0x3                loop:
//!     v10 := imul v8, v9                v10 := imul v8, v9
//! ```
//!
//! # Runs
//! A machine phase before `precse` (`flow::machine_with`), at every -O level
//! but -O0.

use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::loops as loopy;
use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

pub struct MachineLicm {
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for MachineLicm {
    fn class_name(&self) -> &'static str {
        "MachineLICM"
    }

    fn name(&self) -> &str {
        "machinelicm"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(hoisted(&body, &self.classes))
    }
}

/// The cell `one` loads into a value, where it is a plain load.
fn loaded(one: &Insn) -> Option<&Mem> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, [Loc::Held(_)], [Loc::Mem(cell)]) => Some(cell),
        _ => None,
    }
}

/// `isLoadFoldBarrier() || mayStore() || isCall() || hasOrderedMemoryRef()`:
/// what keeps every load of the loop in it (`InitializeLoadsHoistableLoops`).
pub(crate) fn keeps_loads(one: &Insn) -> bool {
    if one.is_meta() && one.what.as_ref().is_some_and(|what| what.op != Operation::Barrier) {
        return false;
    }
    crate::backend::machinesink::may_store(one)
        || one.what.as_ref().is_none_or(|what| {
            matches!(
                what.op,
                Operation::Push | Operation::Pop | Operation::Leave | Operation::Exchange | Operation::Nothing
            )
        })
}

/// `fld1` / `fldz`: an x87 constant, made again where it is read. The pin
/// hoists it (`LD_Fp164` is not as cheap as a move); the value then stays on
/// the stack across the loop. It reads `$fpcw`, which `IsLoopInvariantInst`
/// accepts only where nothing in the function writes it: a `fistp` that
/// truncates (`fisttp` here) writes it with `FLDCW`.
fn constant_load(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        what.op == Operation::FloatLoad
            && matches!(what.name.as_deref(), Some("fld1" | "fldz"))
            && what.sources.is_empty()
            && matches!(what.dests.as_slice(), [Loc::Held(_)])
    }) && one.defines.len() == 1
        && one.uses.is_empty()
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
}

/// `IsLICMCandidate` for what this pass moves. `loads`: the loop has nothing
/// that keeps a load in it, and the load's block runs on every trip.
fn candidate(
    one: &Insn,
    classes: &RegisterClasses,
    loads: bool,
) -> bool {
    let Some(what) = &one.what else { return false };
    if !classes.requirements(what).is_empty() {
        return false;
    }
    let load = loads && loaded(one).is_some();
    let pure = load
        || matches!(
            what.op,
            Operation::Address
                | Operation::Extend
                | Operation::Binary
                | Operation::Unary
                | Operation::Multiply
                | Operation::Funnel
        );
    // `lea` names a cell and reads none (LEA32r is not `mayLoad`).
    let cell = |cell: &Mem| (what.op == Operation::Address || load) && cell.selector.is_none() && !cell.stack_argument;
    let held = |place: &Loc| match place {
        Loc::Held(_) | Loc::Imm(_) | Loc::Address(_) => true,
        Loc::Mem(one) => cell(one),
        _ => false,
    };
    pure && what.target.is_none()
        && !what.indirect
        && !one.uses.is_empty()
        && one.defines.len() == 1
        && matches!(what.dests.as_slice(), [Loc::Held(_)])
        && what.sources.iter().all(held)
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && !crate::backend::machinesink::as_cheap_as_a_move(one)
}

/// `body` with the loop-invariant computations in the preheaders.
pub fn hoisted(
    body: &LirBody,
    classes: &RegisterClasses,
) -> LirBody {
    let Some(regs) = body.registers else { return body.clone() };
    let found = loopy::loops(&body.blocks, Some(body.entry));
    if found.is_empty() {
        return body.clone();
    }
    let floating = crate::backend::regclass::floating(body);
    let control_word_written = body
        .insns()
        .iter()
        .any(
            |one| one.what
                .as_ref()
                .is_some_and(
                    |what| what.op == Operation::FloatStore && matches!(what.name.as_deref(), Some("fisttp" | "fistt")),
                ),
        );
    let dominance = loopy::dominance(&body.blocks, Some(body.entry));
    let mut blocks: Vec<crate::model::lir::Block> = body.blocks.clone();
    let place: HashMap<i64, usize> = blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
    // Where each value is defined: the block, by position.
    let mut defined: HashMap<u32, i64> = HashMap::default();
    for block in &blocks {
        for phi in block.phis.iter() {
            defined.insert(phi.result, block.at);
        }
        for one in block.insns.iter() {
            for &value in &one.defines {
                defined.insert(value, block.at);
            }
        }
    }
    // The values one instruction defines: a constant made twice is not hoisted.
    let mut defs: HashMap<u32, usize> = HashMap::default();
    for block in &blocks {
        for one in block.insns.iter() {
            for &value in &one.defines {
                *defs.entry(value).or_default() += 1;
            }
        }
    }
    let mut order: Vec<&loopy::Loop> = found.iter().collect();
    order.sort_by_key(|one| one.body.to_set().len());
    let mut changed = false;
    for lp in order {
        let inside = lp.body.to_set();
        let outside_entries: Vec<i64> = blocks
            .iter()
            .filter(|block| !inside.contains(&block.at) && block.succ.contains(&lp.header))
            .map(|block| block.at)
            .collect();
        let [pre] = outside_entries[..] else { continue };
        let pre_at = place[&pre];
        if blocks[pre_at].succ.len() != 1 {
            continue;
        }
        // The values the loop reads from outside, and its phis: the registers
        // it holds across its trips, which the hoisted one joins.
        let named: HashSet<u32> = inside
            .iter()
            .flat_map(|at| {
                let block = &blocks[place[at]];
                block
                    .insns
                    .iter()
                    .flat_map(|one| one.uses.iter().copied())
                    .filter(|value| defined.get(value).is_none_or(|home| !inside.contains(home)))
                    .chain(block.phis.iter().map(|phi| phi.result))
                    .collect::<Vec<_>>()
            })
            .filter(|value| !floating.contains(value))
            .collect();
        // `HasLoopPHIUse`: the values a copy in the loop reads; a cheap
        // instruction whose value a copy takes would make one more copy.
        let copied: HashSet<u32> = inside
            .iter()
            .flat_map(|at| blocks[place[at]].insns.iter())
            .filter(|one| {
                one.what
                    .as_ref()
                    .is_some_and(
                        |what| what.op == Operation::Move
                            && matches!(
                                (what.dests.as_slice(), what.sources.as_slice()),
                                ([Loc::Held(_)], [Loc::Held(_)])
                            ),
                    )
            })
            .flat_map(|one| one.uses.iter().copied())
            .collect();
        let copied = {
            // `HasLoopPHIUse`: a def a phi of the loop (or of an exit block,
            // for any of its predecessors) takes, directly or through copies
            // in the loop: the join of a state machine takes a constant from
            // each case, and 262 of them hoisted were live across all the
            // loop.
            let mut taken = copied;
            let exits = inside
                .iter()
                .flat_map(|at| blocks[place[at]].succ.iter().copied())
                .filter(|to| !inside.contains(to) && place.contains_key(to));
            for at in inside.iter().copied().chain(exits) {
                for phi in blocks[place[&at]].phis.iter() {
                    taken.extend(phi.incoming.iter().map(|(_, value)| *value));
                }
            }
            let copies: Vec<(u32, Vec<u32>)> = inside
                .iter()
                .flat_map(|at| blocks[place[at]].insns.iter())
                .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Move))
                .filter_map(|one| Some((*one.defines.first()?, one.uses.to_vec())))
                .collect();
            while copies
                .iter()
                .any(|(result, uses)| taken.contains(result) && uses.iter().any(|value| taken.insert(*value)))
            {
            }
            taken
        };
        let limit = classes.available.len();
        // An x87 value cannot be live across a call (`floatassign` does not
        // spill across one), and the stack holds `float_stack` values.
        let calls = inside
            .iter()
            .any(
                |at| blocks[place[at]].insns.iter().any(|one| {
                    one.call.is_some()
                        || one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Call | Operation::Barrier))
                }),
            );
        let floats_named = inside
            .iter()
            .flat_map(|at| {
                let block = &blocks[place[at]];
                block
                    .insns
                    .iter()
                    .flat_map(|one| one.uses.iter().copied())
                    .filter(|value| defined.get(value).is_none_or(|home| !inside.contains(home)))
                    .chain(block.phis.iter().map(|phi| phi.result))
                    .collect::<Vec<_>>()
            })
            .filter(|value| floating.contains(value))
            .collect::<HashSet<u32>>()
            .len();
        // `InitializeLoadsHoistableLoops`: no store, call or ordered load in
        // the loop, its inner loops among it.
        let loads = !inside.iter().any(|at| blocks[place[at]].insns.iter().any(|one| keeps_loads(one)));
        // `IsGuaranteedToExecute`: the header, or a block over every exiting
        // block.
        let exiting: Vec<i64> =
            inside.iter().copied().filter(|at| blocks[place[at]].succ.iter().any(|to| !inside.contains(to))).collect();
        let every_trip = |at: i64| at == lp.header || exiting.iter().all(|&out| dominance.dominates(at, out));
        let mut moved: Vec<Arc<Insn>> = Vec::new();
        let mut hoisted_values: HashSet<u32> = HashSet::default();
        loop {
            let mut any = false;
            for at in blocks.iter().map(|block| block.at).filter(|at| inside.contains(at)).collect::<Vec<_>>() {
                let number = place[&at];
                let loads_here = loads && every_trip(at);
                let mut keep: Vec<Arc<Insn>> = Vec::new();
                let insns: Vec<Arc<Insn>> = blocks[number].insns.iter().cloned().collect();
                for (index, one) in insns.iter().enumerate() {
                    let value = one.defines.first().copied();
                    let invariant = one.uses.iter().all(|used| {
                        hoisted_values.contains(used) || defined.get(used).is_none_or(|home| !inside.contains(home))
                    });
                    let flags = |lanes: &crate::backend::lanes::Lanes| {
                        lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
                    };
                    // An x87 constant is hoisted where the control word is
                    // constant and the stack has room (`constant_load`).
                    if constant_load(one) {
                        if !control_word_written && !calls && floats_named < body.float_stack {
                            hoisted_values.extend(value);
                            moved.push(Arc::clone(one));
                            any = true;
                        } else {
                            keep.push(Arc::clone(one));
                        }
                        continue;
                    }
                    let mut movable = candidate(one, classes, loads_here)
                        && invariant
                        && value.is_some_and(|value| !floating.contains(&value))
                        && named.len() < limit;
                    // `IsProfitableToHoist`: "Trivially rematerializable
                    // instructions should always be hoisted providing the
                    // register allocator can just pull them down again", unless
                    // a copy in the loop takes the value.
                    // An immediate or symbol address goes to a general
                    // register, whatever its width:
                    // `floating` takes 8 bytes for a float.
                    let immediate = one
                        .what
                        .as_ref()
                        .is_some_and(
                            |what| what.op == Operation::Move && matches!(what.sources.as_slice(), [Loc::Imm(_)]),
                        );
                    let constant = value.is_some_and(|value| {
                        one.uses.is_empty()
                            && defs.get(&value) == Some(&1)
                            && !copied.contains(&value)
                            && (immediate || !floating.contains(&value))
                            && one.defines.len() == 1
                            && one.call.is_none()
                            && one.group.is_none()
                            && one.clobbers.is_empty()
                            && one.requires.is_empty()
                            && one.delivers.is_empty()
                            && crate::greedy::remat::is_trivially_re_materializable(one, false)
                    });
                    if constant && !movable {
                        hoisted_values.extend(value);
                        moved.push(Arc::clone(one));
                        any = true;
                        continue;
                    }
                    if movable {
                        let effect = liveness::effect(regs, body.bits, one);
                        let name = one.what.as_ref().and_then(|what| what.name.as_deref()).unwrap_or("");
                        let reads = matches!(name, "adc" | "sbb" | "rcl" | "rcr")
                            || effect.as_ref().is_some_and(|effect| flags(&effect.all_reads()));
                        let sets = effect.as_ref().is_none_or(|effect| flags(&effect.writes));
                        if reads {
                            movable = false;
                        } else if sets {
                            // Flags set here must not be read below, in the
                            // block.
                            for later in &insns[index + 1..] {
                                match liveness::effect(regs, body.bits, later) {
                                    Some(next) if flags(&next.all_reads()) => {
                                        movable = false;
                                        break;
                                    }
                                    Some(next) if flags(&next.writes) => break,
                                    Some(_) => {}
                                    None if later.what.as_ref().is_some_and(|what| {
                                        what.op == Operation::Move
                                            || what.op == Operation::Jump
                                            || matches!(what.name.as_deref(), Some("jcxz" | "jecxz" | "loop"))
                                    }) => {}
                                    None => {
                                        movable = false;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if movable {
                        hoisted_values.extend(value);
                        moved.push(Arc::clone(one));
                        any = true;
                    } else {
                        keep.push(Arc::clone(one));
                    }
                }
                if keep.len() != blocks[number].insns.len() {
                    blocks[number] = blocks[number].with_insns(keep);
                }
            }
            if !any {
                break;
            }
        }
        if moved.is_empty() {
            continue;
        }
        changed = true;
        for one in &moved {
            for &value in &one.defines {
                defined.insert(value, pre);
            }
        }
        let mut insns: Vec<Arc<Insn>> = blocks[pre_at].insns.iter().cloned().collect();
        let end = insns.iter().rposition(|one| !one.is_meta()).filter(|&last| {
            insns[last].what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch))
        });
        let at = end.unwrap_or(insns.len());
        for (offset, one) in moved.into_iter().enumerate() {
            insns.insert(at + offset, one);
        }
        blocks[pre_at] = blocks[pre_at].with_insns(insns);
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Held, Semantics};
    use crate::model::lir::LirBlock;
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

    /// `x = a + b` made on every trip of a loop that changes neither (the pin's
    /// EarlyMachineLICM runs ahead of MachineCSE): it is made once, before the
    /// loop.
    #[test]
    fn test_a_sum_of_values_the_loop_does_not_change_is_made_before_the_loop() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let add = insn(10, Operation::Binary, "add", vec![held(10)], vec![held(1), held(2)], vec![10], vec![1, 2]);
        let copy = insn(11, Operation::Move, "mov", vec![held(11)], vec![held(3)], vec![11], vec![3]);
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], vec![add, copy, branch(12)]).into(),
                block(2, vec![], vec![insn(13, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2, 3].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let classes = RegisterClasses::m16();
        let done = hoisted(&body, &classes);
        let names = |at: usize| -> Vec<String> {
            done.blocks[at]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(names(0), ["add", "jmp"]);
        assert_eq!(names(1), ["mov", "jcxz"]);
    }

    /// bintree's `insert` at -O1, a loop after `tailrecurse`: `t[3*node]` is
    /// read on every trip of a loop that stores nothing. The pin's
    /// EarlyMachineLICM hoists its `lea` and load (`AllowedToHoistLoads`);
    /// both stayed in the loop, 96366 clocks against 93475 hoisted. A store in
    /// the loop keeps the load in it.
    #[test]
    fn test_a_load_of_a_cell_a_loop_never_stores_is_made_before_the_loop() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let cell = |base: u32, index: u32, scale: i64| {
            Loc::Mem(Mem {
                base: Some(Held { value: base, width: 4 }),
                index: Some(Held { value: index, width: 4 }),
                scale,
                ..Mem::new(None, 4)
            })
        };
        let run = |stores: bool| {
            let lea = insn(10, Operation::Address, "lea", vec![held(10)], vec![cell(2, 2, 2)], vec![10], vec![2, 2]);
            let load = insn(11, Operation::Move, "mov", vec![held(11)], vec![cell(1, 10, 4)], vec![11], vec![1, 10]);
            let mut inside = vec![lea, load];
            if stores {
                inside.push(insn(
                    12,
                    Operation::Move,
                    "mov",
                    vec![cell(1, 3, 4)],
                    vec![held(11)],
                    vec![],
                    vec![1, 3, 11],
                ));
            }
            inside.push(branch(13));
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    block(1, vec![1, 2], inside).into(),
                    block(2, vec![], vec![insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1, 2, 3].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted(&body, &RegisterClasses::m16());
            let names = |at: usize| -> Vec<String> {
                done.blocks[at]
                    .insns
                    .iter()
                    .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                    .collect()
            };
            (names(0), names(1))
        };
        assert_eq!(run(false), (vec!["lea".to_owned(), "mov".into(), "jmp".into()], vec!["jcxz".to_owned()]));
        assert_eq!(
            run(true),
            (vec!["lea".to_owned(), "jmp".into()], vec!["mov".to_owned(), "mov".into(), "jcxz".into()])
        );
    }

    /// recfloat's `decay` at -O1: `fld1` was made on every trip of the loop
    /// (4 clk and an `fxch` a trip); the pin's EarlyMachineLICM hoists it, the
    /// value then stays on the x87 stack. A call in the loop keeps it there.
    #[test]
    fn test_an_x87_constant_load_is_made_before_a_loop_without_a_call() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 10 });
        let run = |call: bool, truncating: bool| {
            let one = insn(10, Operation::FloatLoad, "fld1", vec![wide(10)], vec![], vec![10], vec![]);
            let add =
                insn(11, Operation::Binary, "fadd", vec![wide(11)], vec![wide(1), wide(10)], vec![11], vec![1, 10]);
            let mut inside = vec![one, add];
            if call {
                inside.push(insn(12, Operation::Call, "call", vec![], vec![], vec![], vec![]));
            }
            inside.push(branch(13));
            let mut exit = vec![insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![])];
            if truncating {
                exit.insert(0, insn(15, Operation::FloatStore, "fisttp", vec![], vec![wide(11)], vec![], vec![11]));
            }
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    block(1, vec![1, 2], inside).into(),
                    block(2, vec![], exit).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().collect();
            body.float_stack = 8;
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted(&body, &RegisterClasses::m16());
            done.blocks[0]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(false, false), ["fld1", "jmp"]);
        assert_eq!(run(true, false), ["jmp"]);
        // recfloat -O2: the pin's `FLDCW` for the `fptosi` after the loop
        // makes `$fpcw` non-constant, so `fld1` stays in the loop.
        assert_eq!(run(false, true), ["jmp"]);
    }

    /// A constant made in the loop that a phi of the loop takes (the join of a
    /// state machine takes one from each case) stays where it is: the pin's
    /// `HasLoopPHIUse` keeps a cheap instruction from making a copy in the
    /// loop. Hoisted it was live across the whole loop, and the 262 of them in
    /// a 256-case state machine cost 29 times the pin's segment work in
    /// `LiveRangeCalc::extend` (206550 adds against 7056 at N=256).
    #[test]
    fn test_a_constant_a_loop_phi_takes_stays_in_the_loop() {
        use crate::model::lir::Phi;
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let imm = Loc::Imm(llrm_lir::Imm { value: 7, width: 4, address: None });
        let run = |phi_takes_it: bool| {
            let constant = insn(10, Operation::Move, "mov", vec![held(10)], vec![imm.clone()], vec![10], vec![]);
            let mut header = block(1, vec![1, 2], vec![constant, branch(11)]);
            let taken = if phi_takes_it { 10 } else { 1 };
            header.phis = vec![Phi { result: 20, incoming: vec![(0, 1), (1, taken)] }].into();
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    header.into(),
                    block(2, vec![], vec![insn(12, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted(&body, &RegisterClasses::m16());
            (done.blocks[0].insns.len(), done.blocks[1].insns.len())
        };
        assert_eq!(run(true), (1, 2), "the phi's copy stays");
        assert_eq!(run(false), (2, 1), "no phi takes it: hoisted");
    }

    /// m64 nbody_single at -O1/-O2: `mov r64, offset sym` made on every trip of
    /// a loop was kept there, three per trip, against the pin's one before the
    /// loop (+18% executed): `floating` took every value wider than 4 bytes
    /// for a float, an 8-byte address among them.
    #[test]
    fn test_a_symbol_address_of_eight_bytes_is_made_before_the_loop() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 8 });
        let address = Loc::Imm(llrm_lir::Imm {
            value: 0,
            width: 8,
            address: Some(llrm_lir::Addr::new(llrm_lir::Space::Segment, 0)),
        });
        let constant = insn(10, Operation::Move, "mov", vec![wide(10)], vec![address], vec![10], vec![]);
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], vec![constant, branch(11)]).into(),
                block(2, vec![], vec![insn(12, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::of(&llrm_x86_m64::M64));
        assert_eq!((done.blocks[0].insns.len(), done.blocks[1].insns.len()), (2, 1));
    }
}
