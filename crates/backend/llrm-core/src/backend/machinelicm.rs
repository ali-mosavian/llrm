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
//!   pinned operand, no memory, no call, no x87 value (`candidate`);
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
//! DEVIATE ML2: no preheader is made where a loop has none; no load of an
//! invariant cell (`ExtractHoistableLoad`); no CSE of the hoisted value
//! (`MayCSE`).
//!
//! # Runs
//! A machine phase before `precse` (`flow::machine_with`), at every -O level
//! but -O0.

use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::loops as loopy;
use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::model::ir::{Loc, Operation};
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

/// `IsLICMCandidate` for what this pass moves.
fn candidate(
    one: &Insn,
    classes: &RegisterClasses,
) -> bool {
    let Some(what) = &one.what else { return false };
    if !classes.requirements(what).is_empty() {
        return false;
    }
    let pure = matches!(
        what.op,
        Operation::Address
            | Operation::Extend
            | Operation::Binary
            | Operation::Unary
            | Operation::Multiply
            | Operation::Funnel
    );
    let held = |place: &Loc| matches!(place, Loc::Held(_) | Loc::Imm(_) | Loc::Address(_));
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
        // The values named in the loop: the pressure the hoisted one adds to.
        let named: HashSet<u32> = inside
            .iter()
            .flat_map(|at| {
                let block = &blocks[place[at]];
                block
                    .insns
                    .iter()
                    .flat_map(|one| one.uses.iter().chain(&one.defines).copied())
                    .chain(block.phis.iter().map(|phi| phi.result))
                    .collect::<Vec<_>>()
            })
            .filter(|value| !floating.contains(value))
            .collect();
        let limit = classes.available.len();
        let mut moved: Vec<Arc<Insn>> = Vec::new();
        let mut hoisted_values: HashSet<u32> = HashSet::default();
        loop {
            let mut any = false;
            for at in blocks.iter().map(|block| block.at).filter(|at| inside.contains(at)).collect::<Vec<_>>() {
                let number = place[&at];
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
                    let mut movable = candidate(one, classes)
                        && invariant
                        && value.is_some_and(|value| !floating.contains(&value))
                        && named.len() < limit;
                    if movable && let Some(effect) = liveness::effect(regs, body.bits, one) {
                        if flags(&effect.all_reads()) {
                            movable = false;
                        } else if flags(&effect.writes) {
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
                                    None => {
                                        movable = false;
                                        break;
                                    }
                                }
                            }
                        }
                    } else if movable {
                        movable = false;
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
