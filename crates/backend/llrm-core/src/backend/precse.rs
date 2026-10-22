//! The same pure computation made twice, made once: `MachineCSE` before PHI
//! elimination, where a value has one definition. Translated from
//! [MachineCSE.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp)
//! (`ProcessBlockCSE`, `isCSECandidate`, `isProfitableToCSE`,
//! `PerformSimplePRE`).
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
//! An invariant load (a load of a constant global the module defines,
//! `isDereferenceableInvariantLoad`; selection marks it `Insn::invariant`) is
//! a candidate like any other, and a removed instruction no longer counts as a
//! reader of its operands.
//!
//! `PerformSimplePRE` runs first: an expression computed in two blocks, where
//! neither dominates the other and one reaches the other, is also computed
//! before the terminator of their nearest common dominator (when that block
//! runs no more often than the two together); the walk above then removes the
//! two. A copy no one reads is dropped again (the pin leaves it to
//! `DeadMachineInstructionElim`).
//!
//! DEVIATE CSE1 (owner funcattrs-port, exit 2026-11-13):
//! `isProfitableToHoistInto` reads the block frequencies selection estimated
//! (`LirBody::frequencies`, kept through rewrites), not
//! `MachineBlockFrequencyInfo` of the current CFG; a body with no table gets no
//! PRE. No `hasMinSize` shortcut (the selector has no minsize flag, isel.rs I2)
//! and no `isLegalToHoistInto` test (no block says it is a landing pad).
//!
//! DEVIATE CSE2 (owner funcattrs-port, exit 2026-11-13): an operation with no
//! register operand is not a candidate, a load of a constant global with a
//! fixed address among them (the pin shares and rematerializes it; ours cannot
//! once TwoAddress and the coalescer write over it in place: compare_to_int64.c
//! -Os 346 to 361 bytes, grep -O2 -m32 +21.9% instructions). Exit: the
//! allocator remakes a value with two defs. (This was E3.)
//!
//! DEVIATE CSE3 (owner funcattrs-port, exit 2026-11-13): only a load selected
//! by a pattern from a constant global is invariant. The pin also takes
//! `!invariant.load`, GOT and constant-pool loads and the immutable fixed stack
//! objects (incoming arguments); those loads are kept.
//!
//! DEVIATE CSE4 (owner funcattrs-port, exit 2026-11-13):
//! `PerformTrivialCopyPropagation` renames an operand in the table's key
//! without moving the instruction's read to the copy's source, so the readers
//! of a copy's source are counted from the body as it was. Heuristics 2 and 3
//! see the pin's lists except for that.
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
/// result, or a load of memory that never changes
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L398>).
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
    // CSE2: no register operand, so it can be made again where it is wanted.
    if one.uses.is_empty() {
        return false;
    }
    let held = |place: &Loc| matches!(place, Loc::Held(_) | Loc::Imm(_) | Loc::Address(_));
    // `MI->mayLoad()` is a candidate only if `isDereferenceableInvariantLoad`.
    let loads =
        matches!(what.op, Operation::Move | Operation::FloatLoad) && matches!(what.sources.as_slice(), [Loc::Mem(_)]);
    let loads_invariant = loads && one.invariant();
    let pure = pure && !loads || loads_invariant;
    pure && what.target.is_none()
        && !what.indirect
        && one.defines.len() == 1
        && matches!(what.dests.as_slice(), [Loc::Held(_)])
        && (loads_invariant || what.sources.iter().all(held))
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
    readers: HashMap<u32, HashSet<Reader>>,
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

    /// `isProfitableToCSE(CSReg, Reg, CSBB, MI)`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L435>).
    fn profitable(
        &self,
        shared: u32,
        old: u32,
        shared_block: usize,
        one: &Insn,
        block: usize,
    ) -> bool {
        let none = HashSet::default();
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
            let Some(effect) = liveness::effect(self.regs, self.body.bits, later) else {
                // A load through virtual address registers has no effect
                // table yet; a move leaves the flags alone.
                if later.what.as_ref().is_some_and(|what| what.op == Operation::Move) {
                    continue;
                }
                return true;
            };
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

/// Whether `one` writes the flags (an operation with no effect table is
/// taken to). A move and a load leave them alone.
fn sets_flags(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    one: &Insn,
) -> bool {
    if one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Move | Operation::FloatLoad)) {
        return false;
    }
    liveness::effect(regs, bits, one)
        .is_none_or(|effect| effect.writes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None))
}

fn is_terminator(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(
                what.op,
                Operation::Branch | Operation::Jump | Operation::Return
            ),
        )
}

/// `isPotentiallyReachable(from, to)` over the blocks' edges: a search that
/// gives up after 32 blocks and answers yes (CFG.cpp,
/// `DefaultMaxBBsToExplore`).
fn reaches(
    body: &LirBody,
    by_at: &IndexMap<i64, usize>,
    from: usize,
    to: usize,
) -> bool {
    let mut seen: HashSet<usize> = HashSet::default();
    let mut work = vec![from];
    let mut budget = 32;
    while let Some(at) = work.pop() {
        for succ in body.blocks[at].succ.iter() {
            let Some(&next) = by_at.get(succ) else { continue };
            if next == to {
                return true;
            }
            if seen.insert(next) {
                budget -= 1;
                if budget == 0 {
                    return true;
                }
                work.push(next);
            }
        }
    }
    false
}

/// `isPRECandidate`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L799>):
/// a candidate that is not a load, not as cheap as a move, defines one value
/// and leaves the flags alone (the pin refuses an operation that defines a
/// physical register: EFLAGS).
fn pre_candidate(
    one: &Insn,
    classes: &RegisterClasses,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> bool {
    candidate(one, classes)
        && !one.what.as_ref().is_some_and(|what| what.sources.iter().any(|place| matches!(place, Loc::Mem(_))))
        && !cheap(one)
        && !sets_flags(regs, bits, one)
}

/// `PerformSimplePRE`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L901>),
/// with `ProcessBlockPRE`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L821>)
/// and
/// `isProfitableToHoistInto`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L919>):
/// `body` with a copy of each partially redundant expression before the
/// terminator of the nearest common dominator of the blocks that compute it,
/// and the values made. None if nothing was hoisted.
///
/// The pin's `isProfitableToCSE` of the new value is true of every candidate
/// here (not cheap, a register operand, no reader yet), so it is not asked.
fn hoisted(
    body: &LirBody,
    classes: &RegisterClasses,
    dominance: &AddressDominance,
    children: &HashMap<i64, Vec<i64>>,
    by_at: &IndexMap<i64, usize>,
) -> Option<(LirBody, HashSet<u64>)> {
    let frequencies = body.frequencies.as_ref()?;
    let regs = body.regs();
    // `PREMap`: expression -> the block it was last seen in or hoisted to.
    let mut seen: HashMap<Semantics, usize> = HashMap::default();
    let mut put: HashMap<usize, Vec<Arc<Insn>>> = HashMap::default();
    let mut next = crate::backend::splitkit::_next_value_following(body);
    let mut like: Vec<(u32, u32)> = Vec::new();
    let mut stack = vec![body.entry];
    while let Some(at) = stack.pop() {
        stack.extend(children.get(&at).into_iter().flatten().copied());
        let number = by_at[&at];
        for one in body.blocks[number].insns.iter() {
            if !pre_candidate(one, classes, regs, body.bits) {
                continue;
            }
            let what = one.what.as_ref().expect("a candidate has semantics");
            let key = expression(what);
            let Some(&other) = seen.get(&key) else {
                seen.insert(key, number);
                continue;
            };
            let (other_at, here) = (body.blocks[other].at, body.blocks[number].at);
            // The nearest common dominator.
            let (mut up, mut down) = (other_at, here);
            while up != down {
                let order = |at: i64| dominance.preorder(at);
                let climbing = if order(up) > order(down) { &mut up } else { &mut down };
                let Some(parent) = dominance.immediate(*climbing) else { break };
                *climbing = parent;
            }
            if up != down || up == other_at {
                continue;
            }
            let common = by_at[&up];
            let freq = |at: i64| frequencies.0.get(&at).copied();
            let (Some(common_freq), Some(a), Some(b)) = (freq(up), freq(here), freq(other_at)) else { continue };
            if common_freq > a + b {
                continue;
            }
            if !(reaches(body, by_at, other, number) || reaches(body, by_at, number, other)) {
                continue;
            }
            let value = next;
            next += 1;
            let mut made = what.clone();
            if let Some(Loc::Held(dest)) = made.dests.first_mut() {
                dest.value = value;
            }
            let at_end = body.blocks[common].insns.iter().find(|one| is_terminator(one)).map_or(here, |one| one.at);
            let copy = Insn {
                what: Some(made),
                defines: vec![value],
                at: at_end,
                covers: Some((at_end, at_end)),
                line: None,
                debug: Default::default(),
                ..(**one).clone()
            };
            like.push((value, one.defines[0]));
            put.entry(common).or_default().push(Arc::new(copy));
            seen.insert(key, common);
        }
    }
    if put.is_empty() {
        return None;
    }
    let mut made_ids = HashSet::default();
    let mut out = body.clone();
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .enumerate()
        .map(|(number, block)| match put.remove(&number) {
            None => block.clone(),
            Some(copies) => {
                let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
                let end = insns.iter().position(|one| is_terminator(one)).unwrap_or(insns.len());
                made_ids.extend(copies.iter().map(|one| one.id()));
                insns.splice(end..end, copies);
                block.with_insns(insns)
            }
        })
        .collect();
    out = out.with_blocks(blocks);
    let table = Arc::make_mut(&mut out.class_like);
    for (value, old) in like {
        table.insert(value, old);
    }
    Some((out, made_ids))
}

/// `body` with every repeated pure computation shared.
pub fn shared(
    body: &LirBody,
    classes: &RegisterClasses,
) -> LirBody {
    let by_at: IndexMap<i64, usize> =
        body.blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
    let dominance = AddressDominance::of(body.blocks.iter().map(|block| (block.at, &block.succ[..])), Some(body.entry));
    let mut children: HashMap<i64, Vec<i64>> = HashMap::default();
    for block in &body.blocks {
        if let Some(parent) = dominance.immediate(block.at) {
            children.entry(parent).or_default().push(block.at);
        }
    }
    let (body, made) = match hoisted(body, classes, &dominance, &children, &by_at) {
        Some((body, made)) => (std::borrow::Cow::Owned(body), made),
        None => (std::borrow::Cow::Borrowed(body), HashSet::default()),
    };
    let body: &LirBody = &body;
    let regs = body.regs();
    let mut block_of = HashMap::default();
    let mut readers: HashMap<u32, HashSet<Reader>> = HashMap::default();
    for (number, block) in body.blocks.iter().enumerate() {
        for one in block.insns.iter() {
            block_of.insert(one.id(), number);
            for value in &one.uses {
                readers.entry(*value).or_default().insert(Reader::Insn(one.id()));
            }
        }
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                readers.entry(value).or_default().insert(Reader::Phi(number));
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
            let sets_flags = sets_flags(regs, body.bits, one);
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
            // The erased instruction no longer reads its operands.
            for &used in &one.uses {
                let used = walk.name(used);
                if let Some(readers) = walk.readers.get_mut(&used) {
                    readers.remove(&Reader::Insn(one.id()));
                }
            }
            let merged = walk.readers.remove(&old).unwrap_or_default();
            walk.readers.entry(shared_value).or_default().extend(merged);
        }
        stack.push((at, Some(mine)));
        for child in children.get(&at).into_iter().flatten() {
            stack.push((*child, None));
        }
    }
    // A hoisted copy nothing reads is dead.
    for block in &body.blocks {
        for one in block.insns.iter().filter(|one| made.contains(&one.id())) {
            if walk.readers.get(&one.defines[0]).is_none_or(|readers| readers.is_empty()) {
                walk.removed.insert(one.id());
            }
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
