//! Moves a constant a loop makes on every trip to the loop's entry.
//!
//! # How it works
//! The pin's post-allocation MachineLICM (`HoistRegionPostRA`, `ProcessMI`,
//! `HoistPostRA`). After allocation an instruction reads no value the loop
//! changes only if it reads no register at all, so what it moves is an
//! instruction that makes a constant (`mov r, imm`, the address of a symbol,
//! a zeroing `xorps x, x`) into a register that the loop defines nowhere else
//! and that no block of the loop is entered with a live value in. The
//! instruction goes to the end of the loop's one entry block, before its jump.
//!
//! The early MachineLICM leaves the same constant in the loop when a phi in
//! an exit block takes it (`HasLoopPHIUse`, which rejects every exit block):
//! `for (...) if (c) return 0; return 1;` makes the `0` in the loop and
//! returns it in `eax` from the exit. Allocation then names that phi's value
//! `eax` alone, and this pass hoists the `xor eax, eax`.
//!
//! Not ported: `getOrCreatePreheader` splitting an edge (a loop with several
//! entry blocks, or an entry that branches, is left as it is), and the stack
//! reloads, which `loopslots` moves.
//!
//! # Example
//! `safe` of queens -O1, the zero returned from inside the loop:
//!
//! ```text
//!   loop:  mov r8d, [rdi]                 pre:   xor eax, eax
//!          xor eax, eax            =>     loop:  mov r8d, [rdi]
//!          sub r8d, edx                          sub r8d, edx
//!          je  exit                              je  exit
//! ```
//!
//! # Runs
//! After `loopslots`, before `schedule`, at every -O level but -O0.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::loops;
use crate::backend::classes::RegisterClasses;
use crate::backend::lanes::Lanes;
use crate::backend::liveness;
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct PostRaLicm {
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for PostRaLicm {
    fn class_name(&self) -> &'static str {
        "PostRaLicm"
    }

    fn name(&self) -> &str {
        "postralicm"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(hoisted(&body, &self.classes))
    }
}

/// Whether `one` makes a constant into one register, and does nothing else:
/// `ProcessMI` takes an instruction with no register operand but its one
/// explicit definition (`HasNonInvariantUse` is false) that
/// `IsLICMCandidate` accepts.
fn constant(
    one: &Insn,
    classes: &RegisterClasses,
) -> bool {
    let Some(what) = one.what.as_ref() else { return false };
    let makes = what.op == Operation::Move && matches!(what.sources.as_slice(), [Loc::Imm(_)]) || zeroes(what);
    if !makes || what.target.is_some() || what.indirect {
        return false;
    }
    let [Loc::Reg(into)] = what.dests.as_slice() else { return false };
    one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && classes.available.contains(&ir::root(into.register))
}

/// `xor r, r`: the register zeroed by an instruction that reads only it (the
/// pin's `MOV32r0` and `V_SET0` read nothing).
fn zeroes(what: &crate::model::ir::Semantics) -> bool {
    matches!(
        (what.dests.as_slice(), what.name.as_deref()),
        ([Loc::Reg(into)], Some("xor" | "sub" | "pxor" | "xorps" | "xorpd"))
            if !what.sources.is_empty()
                && what.sources.iter().all(|source| matches!(source, Loc::Reg(from) if from.register == into.register))
    )
}

/// `body` with each constant a loop makes on every trip made before the loop.
pub fn hoisted(
    body: &LirBody,
    classes: &RegisterClasses,
) -> LirBody {
    let graph = &body.blocks;
    let mut found = loops::loops(graph, Some(body.entry));
    if found.is_empty() {
        return body.clone();
    }
    let regs = body.regs();
    let index =
        body.blocks.iter().enumerate().map(|(position, block)| (block.at, position)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    let (live_into, _, _) = liveness::live_into(body);
    // The pin visits a loop before the loops inside it.
    found.sort_by_key(|one| std::cmp::Reverse(one.body.len()));
    let mut blocks = body.blocks.clone();
    for one in &found {
        let entries = predecessors[&one.header].iter().copied().filter(|at| !one.body.contains(at)).collect::<Vec<_>>();
        let [entry] = entries.as_slice() else { continue };
        if blocks[index[entry]].succ != [one.header] {
            continue;
        }
        // `RUDefs`: what the blocks seen so far are entered with live, and
        // what their instructions write. `RUClobbers`: written twice.
        let mut defined = Lanes::default();
        let mut clobbered = Lanes::default();
        let mut candidates: Vec<(usize, usize, Lanes)> = Vec::new();
        let mut known = true;
        'scan: for at in &one.body {
            defined = defined.or(&live_into[at]);
            for (place, insn) in blocks[index[at]].insns.iter().enumerate() {
                let Some(effect) = liveness::effect(regs, body.bits, insn) else {
                    known = false;
                    break 'scan;
                };
                clobbered = clobbered.or(&defined.and(&effect.writes));
                defined = defined.or(&effect.writes);
                if constant(insn, classes) {
                    candidates.push((index[at], place, effect.writes));
                }
            }
        }
        if !known {
            continue;
        }
        // A candidate's lanes are written by it alone, and are live into no
        // block of the loop that comes before it (a block later in the loop
        // is entered with its value, made earlier on the trip).
        let moved = candidates
            .into_iter()
            .filter(|(block, place, writes)| {
                let flags = writes.flag_part();
                let made = writes.minus(&flags);
                !made.is_empty()
                    && made.is_disjoint(&clobbered)
                    && (flags.is_empty()
                        || flags.is_disjoint(&live_into[&one.header])
                            && flags_dead_after(&blocks, &index, &live_into, *block, *place, &flags, regs, body.bits))
            })
            .collect::<Vec<_>>();
        if moved.is_empty() {
            continue;
        }
        let mut taken: Vec<Arc<Insn>> = Vec::new();
        for (block, place, _) in &moved {
            taken.push(Arc::clone(&blocks[*block].insns[*place]));
        }
        let gone = moved.iter().map(|(block, place, _)| (*block, *place)).collect::<std::collections::BTreeSet<_>>();
        for (position, block) in blocks.iter_mut().enumerate() {
            if gone.iter().any(|(at, _)| *at == position) {
                let kept = block
                    .insns
                    .iter()
                    .enumerate()
                    .filter(|(place, _)| !gone.contains(&(position, *place)))
                    .map(|(_, insn)| Arc::clone(insn))
                    .collect();
                *block = block.with_insns(kept);
            }
        }
        let block = &blocks[index[entry]];
        let mut insns = block.insns.to_vec();
        let jumps = insns.last().and_then(|last| last.what.as_ref()).is_some_and(|what| what.op == Operation::Jump);
        for insn in taken {
            insns.insert(insns.len() - usize::from(jumps), insn);
        }
        blocks[index[entry]] = block.with_insns(insns);
    }
    body.with_blocks(blocks)
}

/// Whether the flags `flags` that the instruction at (`block`, `place`) sets
/// are dead after it (`MO.isDead()` of the pin's implicit definition): the
/// instructions after it write them before any reads them, or the block's
/// successors are not entered with them live.
#[allow(clippy::too_many_arguments)]
fn flags_dead_after(
    blocks: &[crate::model::lir::Block],
    index: &BTreeMap<i64, usize>,
    live_into: &crate::support::hash::IndexMap<i64, Lanes>,
    block: usize,
    place: usize,
    flags: &Lanes,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> bool {
    for later in &blocks[block].insns[place + 1..] {
        let Some(effect) = liveness::effect(regs, bits, later) else { return false };
        if !effect.all_reads().is_disjoint(flags) {
            return false;
        }
        if flags.is_subset(&effect.writes) {
            return true;
        }
    }
    blocks[block].succ.iter().all(|to| index.contains_key(to) && live_into[to].is_disjoint(flags))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn reg(register: RegId) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn made(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new()))
    }

    fn set(
        at: i64,
        register: RegId,
        value: i64,
    ) -> Arc<Insn> {
        made(at, Operation::Move, "mov", vec![reg(register)], vec![Loc::Imm(Imm { value, width: 2, address: None })])
    }

    /// `entry: jmp loop; loop: <body> jne loop; ret`.
    fn looping(body: Vec<Arc<Insn>>) -> LirBody {
        let branch = Arc::new(Insn::new(
            0x40,
            Some((0x40, 0x41)),
            Some(Semantics { name: Some("jne".into()), target: Some(0x10), ..Semantics::new(Operation::Branch) }),
            vec![],
            vec![],
        ));
        let leave =
            Arc::new(Insn { reads_complete: true, ..(*made(0x20, Operation::Return, "ret", vec![], vec![])).clone() });
        let block =
            |at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![made(0, Operation::Jump, "jmp", vec![], vec![])], vec![0x10]).into(),
                block(0x10, [body, vec![branch]].concat(), vec![0x10, 0x20]).into(),
                block(0x20, vec![leave], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    /// The registers the instructions of block `at` set.
    fn sets(
        body: &LirBody,
        at: i64,
    ) -> Vec<RegId> {
        let block = body.blocks.iter().find(|one| one.at == at).expect("the block");
        block
            .insns
            .iter()
            .filter_map(|one| match one.what.as_ref()?.dests.as_slice() {
                [Loc::Reg(reg)] => Some(reg.register),
                _ => None,
            })
            .collect()
    }

    fn run(body: &LirBody) -> LirBody {
        super::hoisted(body, &crate::backend::classes::RegisterClasses::m16())
    }

    /// `HoistRegionPostRA`: a constant made once in the loop into a register
    /// nothing else in it writes is made ahead of the loop (queens -O1 `safe`
    /// made `xor eax, eax` on every trip: 5,720 of the 6,232 extra
    /// instructions against the pin).
    #[test]
    fn test_a_constant_made_once_in_a_loop_is_made_before_it() {
        let out = run(&looping(vec![set(0x11, RegId::BX, 7)]));
        assert_eq!(sets(&out, 0x10), Vec::<RegId>::new(), "the loop no longer makes it");
        assert_eq!(sets(&out, 0), [RegId::BX], "the entry block does");
    }

    /// A register the loop makes twice is not invariant (`RUClobbers`): the
    /// value the second leaves is read on the next trip after the first
    /// wrote the other.
    #[test]
    fn test_a_register_the_loop_writes_twice_stays_in_it() {
        let out = run(&looping(vec![set(0x11, RegId::CX, 1), set(0x12, RegId::CX, 2)]));
        assert_eq!(sets(&out, 0x10), [RegId::CX, RegId::CX]);
        assert_eq!(sets(&out, 0), Vec::<RegId>::new());
    }
}
