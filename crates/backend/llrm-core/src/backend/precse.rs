//! The same pure computation made twice, made once: `MachineCSE` before PHI
//! elimination, where a value has one definition. Translated from
//! [MachineCSE.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp)
//! (`ProcessBlockCSE`, `isCSECandidate`, `isProfitableToCSE`).
//!
//! # How it works
//! The dominator tree is walked with a scoped table from an instruction's
//! expression (operation, destination width and sources) to the value it
//! defines: a block sees the entries of the blocks that dominate it and drops
//! its own when its subtree is done (`ScopedHashTable`, `EnterScope`,
//! `ExitScope`). An instruction that [`candidate`] admits (no store, call,
//! terminator, volatile access, fixed register or load) and finds its
//! expression in the table is removed, and every use of its value reads the
//! table's, when `profitable` agrees:
//!
//! - an operation as cheap as a move is shared only inside its block or with a
//!   block that dominates it by an edge (heuristic 1), so a constant does not
//!   stay live across a function;
//! - a value is shared only if every reader of the new one also reads the old
//!   one, or the old one is read in the block (heuristics 2 and 3).
//!
//! Before the table is asked, an operand made by a plain copy names the copy's
//! source (`PerformTrivialCopyPropagation`), and a commutative operation is
//! asked with its sources swapped if the first form is not there.
//!
//! An operation that sets flags is shared only where nothing after it in its
//! block reads them (the pin's `hasLivePhysRegDefUses`: EFLAGS).
//!
//! DEVIATE E1 (no number): no `PerformSimplePRE` (hoisting an expression
//! computed in sibling blocks to their dominator) and no load of an invariant
//! cell (`isDereferenceableInvariantLoad`). Exit: both translated.
//!
//! DEVIATE E2 (no number): the readers of a value are counted from the body as
//! it was, plus the readers merged in by earlier removals; the pin counts the
//! live use lists. Same decisions on a body where one removal does not change
//! another's readers.
//!
//! DEVIATE E3 (no number): an operation with no register operand (a constant,
//! the address of a global or a frame cell) is not a candidate. The pin shares
//! it and rematerializes it where it is wanted; ours cannot once TwoAddress
//! and the coalescer write over it in place: compare_to_int64.c -Os 346 to 361
//! bytes, grep -O2 -m32 +21.9% instructions. Exit: the allocator remakes a
//! value with two defs.
//!
//! # Example
//! `v3 := add v1, v2` in a block and `v7 := add v1, v2` in a block it
//! dominates: `v7` is gone and its readers read `v3`.
//!
//! # Runs
//! A machine phase before `machinesink` (`flow::machine_with`), at every -O
//! level but -O0.

use std::rc::Rc;
use std::sync::Arc;

use llrm_mir::domtree_address::AddressDominance;

use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap};

pub struct MachineCse {
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for MachineCse {
    fn class_name(&self) -> &'static str {
        "MachineCSE"
    }

    fn name(&self) -> &str {
        "precse"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(shared(&body, &self.classes))
    }
}

/// `isCSECandidate`: a pure operation on registers and constants with one
/// result.
fn candidate(
    one: &Insn,
    classes: &RegisterClasses,
) -> bool {
    let Some(what) = &one.what else { return false };
    // A pinned operand is a physical register the instruction defines or
    // reads (`cdq` writes `edx`): `hasLivePhysRegDefUses` keeps those apart.
    if !classes.requirements(what).is_empty() {
        return false;
    }
    let pure = matches!(
        what.op,
        Operation::Move
            | Operation::Address
            | Operation::Extend
            | Operation::Binary
            | Operation::Unary
            | Operation::Multiply
            | Operation::Funnel
    );
    // `MI->isCopyLike()`: a register copy is not a candidate.
    if what.op == Operation::Move && matches!(what.sources.as_slice(), [Loc::Held(_)]) {
        return false;
    }
    // E3: no register operand, so it can be made again where it is wanted.
    if one.uses.is_empty() {
        return false;
    }
    let held = |place: &Loc| matches!(place, Loc::Held(_) | Loc::Imm(_) | Loc::Address(_));
    pure && what.target.is_none()
        && !what.indirect
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
}

/// What the table is keyed by: the instruction with its result named by
/// nothing.
fn expression(what: &Semantics) -> Semantics {
    let mut key = what.clone();
    if let Some(Loc::Held(dest)) = key.dests.first_mut() {
        *dest = Held { value: 0, width: dest.width };
    }
    key
}

/// `TII->isAsCheapAsAMove`.
fn cheap(one: &Insn) -> bool {
    crate::backend::machinesink::as_cheap_as_a_move(one)
}

/// The commutative operations: `add`, `and`, `or`, `xor`, `imul`.
fn commuted(what: &Semantics) -> Option<Semantics> {
    let commutative = (what.op == Operation::Binary
        && matches!(what.name.as_deref(), Some("add" | "and" | "or" | "xor")))
        || (what.op == Operation::Multiply && what.name.as_deref() == Some("imul"));
    if !commutative || what.sources.len() != 2 || what.sources[0] == what.sources[1] {
        return None;
    }
    let mut swapped = what.clone();
    swapped.sources.swap(0, 1);
    Some(swapped)
}

/// Where a value is read: the block of an instruction, or of a phi's edge.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Reader {
    Insn(u64),
    Phi(usize),
}

struct Walk<'a> {
    body: &'a LirBody,
    block_of: HashMap<u64, usize>,
    readers: HashMap<u32, Vec<Reader>>,
    by_at: IndexMap<i64, usize>,
    /// The value each removed one is read as.
    rename: HashMap<u32, u32>,
    removed: HashSet<u64>,
    /// `expression -> (value, block, instruction)`.
    table: HashMap<Semantics, (u32, usize)>,
    regs: llrm_lir::registers::Regs,
}

impl Walk<'_> {
    fn name(
        &self,
        mut value: u32,
    ) -> u32 {
        while let Some(&next) = self.rename.get(&value) {
            value = next;
        }
        value
    }

    /// `isProfitableToCSE(CSReg, Reg, CSBB, MI)`.
    fn profitable(
        &self,
        shared: u32,
        old: u32,
        shared_block: usize,
        one: &Insn,
        block: usize,
    ) -> bool {
        let none = Vec::new();
        let shared_readers = self.readers.get(&shared).unwrap_or(&none);
        let old_readers = self.readers.get(&old).unwrap_or(&none);
        // Every reader of the new value reads the shared one too: its range
        // does not grow.
        if old_readers.iter().all(|reader| shared_readers.contains(reader)) {
            return true;
        }
        // Heuristic 1: a cheap computation is shared only when the definition
        // is local or in an immediate predecessor.
        if cheap(one) && shared_block != block {
            return false;
        }
        // Heuristic 2: no register operand and only copies read it: no use.
        let reads_value = !one.uses.is_empty();
        if !reads_value
            && old_readers.iter().all(|reader| match reader {
                Reader::Insn(id) => self.copies(*id),
                Reader::Phi(_) => false,
            })
        {
            return false;
        }
        // Heuristic 3: a value a phi reads is not shared unless the block
        // reads it already.
        let phi = shared_readers.iter().any(|reader| matches!(reader, Reader::Phi(_)));
        let local = shared_readers
            .iter()
            .any(
                |reader| match reader {
                    Reader::Insn(id) => self.block_of.get(id) == Some(&block),
                    Reader::Phi(_) => false,
                },
            );
        local || !phi
    }

    fn copies(
        &self,
        id: u64,
    ) -> bool {
        self.block_of
            .get(&id)
            .and_then(|&block| self.body.blocks[block].insns.iter().find(|one| one.id() == id))
            .and_then(|one| one.what.as_ref())
            .is_some_and(|what| what.op == Operation::Move)
    }

    /// Whether the flags `one` sets are read before anything sets them again,
    /// in its block.
    fn flags_read_after(
        &self,
        block: usize,
        index: usize,
    ) -> bool {
        for later in &self.body.blocks[block].insns[index + 1..] {
            let Some(effect) = liveness::effect(self.regs, self.body.bits, later) else { return true };
            let flags = |lanes: &crate::backend::lanes::Lanes| {
                lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
            };
            if flags(&effect.all_reads()) {
                return true;
            }
            if flags(&effect.writes) {
                return false;
            }
        }
        false
    }
}

/// `body` with every repeated pure computation shared.
pub fn shared(
    body: &LirBody,
    classes: &RegisterClasses,
) -> LirBody {
    let regs = body.regs();
    let by_at: IndexMap<i64, usize> =
        body.blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
    let mut block_of = HashMap::default();
    let mut readers: HashMap<u32, Vec<Reader>> = HashMap::default();
    for (number, block) in body.blocks.iter().enumerate() {
        for one in block.insns.iter() {
            block_of.insert(one.id(), number);
            for value in &one.uses {
                readers.entry(*value).or_default().push(Reader::Insn(one.id()));
            }
        }
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                readers.entry(value).or_default().push(Reader::Phi(number));
            }
        }
    }
    let mut walk = Walk {
        body,
        block_of,
        readers,
        by_at,
        rename: HashMap::default(),
        removed: HashSet::default(),
        table: HashMap::default(),
        regs,
    };
    let dominance = AddressDominance::of(body.blocks.iter().map(|block| (block.at, &block.succ[..])), Some(body.entry));
    let mut children: HashMap<i64, Vec<i64>> = HashMap::default();
    for block in &body.blocks {
        if let Some(parent) = dominance.immediate(block.at) {
            children.entry(parent).or_default().push(block.at);
        }
    }
    // Depth first over the dominator tree; an entry is `(block, keys to drop)`
    // once its subtree is done.
    let mut stack: Vec<(i64, Option<Vec<Semantics>>)> = vec![(body.entry, None)];
    while let Some((at, added)) = stack.pop() {
        if let Some(added) = added {
            for key in added {
                walk.table.remove(&key);
            }
            continue;
        }
        let number = walk.by_at[&at];
        let mut mine: Vec<Semantics> = Vec::new();
        for (index, one) in body.blocks[number].insns.iter().enumerate() {
            if !candidate(one, classes) {
                continue;
            }
            let Some(what) = one.what.as_ref() else { continue };
            // The operands as the values they have become.
            let mut named = what.clone();
            for place in &mut named.sources {
                *place = llrm_lir::mapped(place, |held| Held { value: walk.name(held.value), width: held.width });
            }
            let key = expression(&named);
            let found = walk
                .table
                .get(&key)
                .copied()
                .or_else(|| commuted(&key).and_then(|swapped| walk.table.get(&swapped).copied()));
            let Some((shared_value, shared_block)) = found else {
                walk.table.insert(key.clone(), (one.defines[0], number));
                mine.push(key);
                continue;
            };
            // An operation that sets flags is shared where nothing reads them.
            let sets_flags = liveness::effect(regs, body.bits, one).is_none_or(|effect| {
                effect.writes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
            });
            if sets_flags && walk.flags_read_after(number, index) {
                walk.table.insert(key.clone(), (one.defines[0], number));
                mine.push(key);
                continue;
            }
            let old = one.defines[0];
            if !walk.profitable(shared_value, old, shared_block, one, number) {
                walk.table.insert(key.clone(), (old, number));
                mine.push(key);
                continue;
            }
            walk.rename.insert(old, shared_value);
            walk.removed.insert(one.id());
            let merged = walk.readers.remove(&old).unwrap_or_default();
            walk.readers.entry(shared_value).or_default().extend(merged);
        }
        stack.push((at, Some(mine)));
        for child in children.get(&at).into_iter().flatten() {
            stack.push((*child, None));
        }
    }
    if walk.removed.is_empty() {
        return body.clone();
    }
    let swap = |value: u32| walk.name(value);
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .filter(|one| !walk.removed.contains(&one.id()))
                .map(|one| one.renamed(&swap))
                .collect();
            let mut out = block.with_insns(insns);
            for phi in out.phis.iter_mut() {
                for (_, value) in &mut phi.incoming {
                    *value = swap(*value);
                }
            }
            out
        })
        .collect();
    body.with_blocks(blocks)
}
