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
//! When the table has no entry, an operand made by a plain copy is moved to the
//! copy's source and a copy nothing else reads is erased
//! (`PerformTrivialCopyPropagation`); a commutative operation is then asked
//! with its sources swapped if the first form is still not there.
//!
//! An operation that sets flags is shared where nothing after it in its block
//! reads them, or where the flags it would set are the shared one's still
//! (`PhysRegDefsReach`: the same block or the sole predecessor, no flag write
//! or call between, five instructions at most).
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
//! `hasMinSize` is `LirBody::min_size` (the function's `minsize` attribute;
//! CSE1 retired). `isLegalToHoistInto` is translated: a return block is no
//! target; the pin's other two conditions cannot hold on this LIR (see
//! `legal_to_hoist_into`).
//!
//! Row CSE2 (was DEVIATE CSE2, retired 2026-10-25): a constant with no register
//! operand is a candidate again. The address of a symbol stays out: the pin
//! shares it and ours cannot make it again. 71 programs -O2 net 0 (lru -1,
//! x_ll_div +1), -Os and m16 unchanged; the skip-edge zero of x_life waits on
//! phi operands without a copy (DAG isel).
//!
//! DEVIATE CSE3 (owner w-1, exit 2026-11-08): both selectors mark a load
//! invariant when it reads a constant global or says so (`!invariant.load`, a
//! `noalias readonly` argument), and the DAG selector the load of an incoming
//! stack argument (an immutable fixed object). The pin also takes GOT and
//! constant-pool loads, which ours lacks; the old selector reads an incoming
//! argument as a memory operand of its reader, not a load of its own.
//!
//! Row CSE5 (was DEVIATE CSE5, retired 2026-10-25): `machinecse.rs`, the same
//! elimination after allocation, was ours alone (the pin runs MachineCSE once,
//! before allocation). Deleted: d_faces -3.60 M Ir (dist). 7 of 568
//! objects grow and none shrinks (x_mergesort +34 B at m32 -Os and m16 -O1 to
//! -Os, shellsort m16 +22 at -O1/-O2), QCport (tools/rows/qcport.py) +0.030%
//! -O1, +0.080% -O2, +0.116% -O3, +0.025% -Os, x_mergesort -Os +0.43% clocks.
//! Exit: CSE2 and rematerialization of multi-def values in RA (w13/w14).
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
use crate::liveness::blockfrequency::MachineBlockFrequencyInfo;
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
    // An instruction that reads the flags is not a candidate: `sbb v, v` after
    // two different compares is two values (the pin: an operation that uses a
    // physical register, EFLAGS, is not a CSE candidate).
    if what.name.as_deref().is_some_and(|name| {
        matches!(name, "adc" | "sbb" | "rcl" | "rcr") || name.starts_with("set") || name.starts_with("cmov")
    }) {
        return false;
    }
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
    // A symbol's address stays out: it is a load of memory the pin shares and
    // ours cannot make again (CSE2).
    if one.uses.is_empty()
        && !matches!(
            what.sources.as_slice(),
            [Loc::Imm(imm)] if imm.address.is_none()
        )
    {
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
    /// The copy that defines a value, and the value it copies.
    copy_of: HashMap<u32, (u64, u32)>,
    /// The operands `PerformTrivialCopyPropagation` moved to a copy's source,
    /// by instruction: `(value read, value read now)`.
    moved: HashMap<u64, Vec<(u32, u32)>>,
    /// `expression -> (value, block, instruction)`.
    table: HashMap<Semantics, (u32, usize)>,
    regs: llrm_lir::registers::Regs,
    /// Each block's predecessors, as block numbers.
    preds: Vec<Vec<usize>>,
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

    /// `value` as `one` reads it now: renamed by the removals, then moved to a
    /// copy's source.
    fn read_as(
        &self,
        one: u64,
        value: u32,
    ) -> u32 {
        let named = self.name(value);
        match self.moved.get(&one).and_then(|pairs| pairs.iter().find(|pair| pair.0 == named)) {
            Some(&(_, to)) => self.name(to),
            None => named,
        }
    }

    /// `PerformTrivialCopyPropagation`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L170>):
    /// an operand made by a plain copy reads the copy's source, and a copy
    /// nothing else reads is erased. Whether an operand moved.
    fn propagate_copies(
        &mut self,
        one: &Insn,
    ) -> bool {
        let id = one.id();
        let mut changed = false;
        for &used in &one.uses {
            let used = self.read_as(id, used);
            let Some(&(copy, source)) = self.copy_of.get(&used) else { continue };
            if self.removed.contains(&copy) {
                continue;
            }
            let source = self.name(source);
            let reads = one
                .what
                .as_ref()
                .map_or(
                    0,
                    |what| what.sources
                        .iter()
                        .filter(|place| matches!(place, Loc::Held(held) if self.name(held.value) == used))
                        .count(),
                );
            let only_one = reads == 1 && self.readers.get(&used).is_some_and(|readers| readers.len() == 1);
            self.moved.entry(id).or_default().push((used, source));
            if let Some(readers) = self.readers.get_mut(&used) {
                readers.remove(&Reader::Insn(id));
            }
            self.readers.entry(source).or_default().insert(Reader::Insn(id));
            if only_one {
                self.removed.insert(copy);
                if let Some(readers) = self.readers.get_mut(&source) {
                    readers.remove(&Reader::Insn(copy));
                }
            }
            changed = true;
        }
        changed
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
        // is local or in an immediate predecessor (`CSBB->isSuccessor(BB)`).
        if cheap(one)
            && shared_block != block
            && !self.body.blocks[shared_block].succ.contains(&self.body.blocks[block].at)
        {
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
            .is_some_and(|what| {
                what.op == Operation::Move
                    && matches!(
                        (what.dests.as_slice(), what.sources.as_slice()),
                        ([Loc::Held(_)], [Loc::Held(_)])
                    )
            })
    }

    /// `PhysRegDefsReach`: the flags the instruction that defines `value` in
    /// `shared` set are still there at the instruction `index` of `block`,
    /// which is in `shared` or has it as its only predecessor, within
    /// `LookAheadLimit` instructions that neither write the flags nor call.
    fn flags_reach(
        &self,
        value: u32,
        shared: usize,
        block: usize,
        index: usize,
    ) -> bool {
        const LOOK_AHEAD_LIMIT: usize = 5;
        let first = &self.body.blocks[shared].insns;
        let Some(def) = first.iter().position(|one| one.defines.first() == Some(&value)) else { return false };
        let (rest, until) = if shared == block {
            (&first[def + 1..index], None)
        } else if self.preds[block] == [shared] {
            (&first[def + 1..], Some(&self.body.blocks[block].insns[..index]))
        } else {
            return false;
        };
        let mut left = LOOK_AHEAD_LIMIT;
        for one in rest.iter().chain(until.into_iter().flatten()) {
            if self.removed.contains(&one.id()) || one.is_meta() {
                continue;
            }
            if left == 0 {
                return false;
            }
            let calls = one.call.is_some() || one.what.as_ref().is_some_and(|what| what.op == Operation::Call);
            if calls || (!is_terminator(one) && sets_flags(self.regs, self.body.bits, one)) {
                return false;
            }
            left -= 1;
        }
        true
    }

    /// Whether the flags `one` sets are read before anything sets them again,
    /// in its block.
    fn flags_read_after(
        &self,
        block: usize,
        index: usize,
    ) -> bool {
        flags_read_after(self.regs, self.body.bits, &self.body.blocks[block].insns, index)
    }
}

/// The flag lanes of `lanes`, but AF: nothing emitted reads it. An x87
/// instruction names lanes past the six of EFLAGS (its status word, which
/// `fnstsw; sahf` moves there): it counts as reading and writing all six.
pub(crate) fn flag_lanes(lanes: &crate::backend::lanes::Lanes) -> crate::backend::lanes::Lanes {
    use llrm_lir::flag::Flag;
    let six = Flag::READ.bits();
    let mask = lanes.mask() as u32;
    let beyond = mask & !(six | Flag::AF.bits()) != 0;
    crate::backend::lanes::Lanes::flags(if beyond { six } else { mask & six })
}

/// What becomes of the flags `insns[index]` sets, walking `insns` after it.
pub(crate) enum Flags {
    /// One of them is read before every one is written again.
    Read,
    /// Every one is written again before any is read.
    Dead,
    /// Some are neither read nor written by the end of `insns`.
    Out,
}

/// `Flags` of the instruction at `index`, per lane: `inc` writes every flag but
/// CF, so a carry read after it is still the one `index` set. An instruction
/// with no effect table reads them, unless it is a move and `moves_pass`.
pub(crate) fn flags_after(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    insns: &[Arc<Insn>],
    index: usize,
    moves_pass: bool,
) -> Flags {
    let mut pending = match liveness::effect(regs, bits, &insns[index]) {
        Some(effect) => flag_lanes(&effect.writes),
        None => flag_lanes(&crate::backend::lanes::Lanes::flags(u32::MAX)),
    };
    for later in &insns[index + 1..] {
        let Some(effect) = liveness::effect(regs, bits, later) else {
            if moves_pass && later.what.as_ref().is_some_and(|what| what.op == Operation::Move) {
                continue;
            }
            return Flags::Read;
        };
        if !flag_lanes(&effect.all_reads()).is_disjoint(&pending) {
            return Flags::Read;
        }
        pending = pending.minus(&flag_lanes(&effect.writes));
        if pending.is_empty() {
            return Flags::Dead;
        }
    }
    Flags::Out
}

/// Whether the flags the instruction at `index` sets are read before anything
/// sets them again, in `insns`.
pub(crate) fn flags_read_after(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    insns: &[Arc<Insn>],
    index: usize,
) -> bool {
    matches!(flags_after(regs, bits, insns, index, true), Flags::Read)
}

/// Whether `one` writes the flags (an operation with no effect table is
/// taken to). A move and a load leave them alone.
pub(crate) fn sets_flags(
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

/// `MachineBasicBlock::isLegalToHoistInto`
/// ([MachineBasicBlock.cpp#L316](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBasicBlock.cpp#L316)):
/// no return block. The other two conditions cannot hold here: the QB landing
/// pad is entered by the runtime and reached by no edge (`ehprepare`), so no
/// block has it as a successor and no block is a candidate that it dominates;
/// and no instruction branches out of inline assembly.
fn legal_to_hoist_into(block: &crate::model::lir::Block) -> bool {
    !block.insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Return))
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
    // `MBFI`: asked the first time two blocks compute an expression.
    let frequencies = std::cell::OnceCell::new();
    let regs = body.regs();
    // `PREMap`: expression -> the block it was last seen in or hoisted to.
    let mut seen: HashMap<Semantics, usize> = HashMap::default();
    let mut put: HashMap<usize, Vec<Arc<Insn>>> = HashMap::default();
    let mut next = crate::backend::postings::next_value_following(body);
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
            // `isLegalToHoistInto`.
            if !legal_to_hoist_into(&body.blocks[common]) {
                continue;
            }
            // `isProfitableToHoistInto`
            // ([#L922](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp#L922)):
            // always under `hasMinSize`, else by `BlockFrequency`, which adds
            // saturating.
            if !body.min_size {
                let mbfi = frequencies.get_or_init(|| MachineBlockFrequencyInfo::of(body));
                let freq = |block: usize| mbfi.get_block_freq(block).get_frequency();
                if freq(common) > freq(number).saturating_add(freq(other)) {
                    continue;
                }
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
    let mut copy_of: HashMap<u32, (u64, u32)> = HashMap::default();
    for block in &body.blocks {
        for one in block.insns.iter() {
            if let Some(what) = one.what.as_ref()
                && what.op == Operation::Move
                && let ([Loc::Held(from)], [Loc::Held(to)]) = (what.sources.as_slice(), what.dests.as_slice())
                && from.width == to.width
                && one.defines == [to.value]
                && one.call.is_none()
                && one.group.is_none()
                && !one.volatile()
                && !one.spill_reload
                && !one.spill_store
                && classes.requirements(what).is_empty()
                && one.clobbers.is_empty()
                && one.requires.is_empty()
                && one.delivers.is_empty()
            {
                copy_of.insert(to.value, (one.id(), from.value));
            }
        }
    }
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); body.blocks.len()];
    for (from, block) in body.blocks.iter().enumerate() {
        for to in block.succ.iter().filter_map(|at| by_at.get(at)) {
            preds[*to].push(from);
        }
    }
    let mut walk = Walk {
        preds,
        body,
        block_of,
        readers,
        by_at,
        rename: HashMap::default(),
        removed: HashSet::default(),
        copy_of,
        moved: HashMap::default(),
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
            let keyed = |walk: &Walk| {
                let mut named = what.clone();
                for place in &mut named.sources {
                    *place = llrm_lir::mapped(place, |held| Held {
                        value: walk.read_as(one.id(), held.value),
                        width: held.width,
                    });
                }
                expression(&named)
            };
            let mut key = keyed(&walk);
            let mut found = walk.table.get(&key).copied();
            // Copy propagation finds more of them (`ProcessBlockCSE`).
            if found.is_none() && walk.propagate_copies(one) {
                key = keyed(&walk);
                found = walk.table.get(&key).copied();
            }
            let found = found.or_else(|| commuted(&key).and_then(|swapped| walk.table.get(&swapped).copied()));
            let Some((shared_value, shared_block)) = found else {
                walk.table.insert(key.clone(), (one.defines[0], number));
                mine.push(key);
                continue;
            };
            // An operation that sets flags is shared where nothing reads them.
            let sets_flags = sets_flags(regs, body.bits, one);
            if sets_flags
                && walk.flags_read_after(number, index)
                && !walk.flags_reach(shared_value, shared_block, number, index)
            {
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
                let used = walk.read_as(one.id(), used);
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
                .map(|one| one.renamed(&|value| walk.read_as(one.id(), value)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lir::LirBlock;

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

    /// `v12 = add v10, v2` where `v10 = mov v1` read `add v10, v2` as a new
    /// expression, so the `add v1, v2` of a dominating block was made again
    /// (`PerformTrivialCopyPropagation` was missing), and the copy stayed.
    #[test]
    fn test_an_add_of_a_copy_is_the_add_of_its_source_and_the_copy_goes() {
        let entry = LirBlock {
            succ: vec![1].into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Move, "mov", vec![held(10)], vec![held(1)], vec![10], vec![1]),
                    insn(1, Operation::Binary, "add", vec![held(11)], vec![held(1), held(2)], vec![11], vec![1, 2]),
                ],
            )
        };
        let next = LirBlock::new(
            1,
            vec![
                insn(2, Operation::Binary, "add", vec![held(12)], vec![held(10), held(2)], vec![12], vec![10, 2]),
                insn(3, Operation::Move, "mov", vec![held(13)], vec![held(12)], vec![13], vec![12]),
            ],
        );
        let mut body =
            LirBody::new("copy", 0, vec![entry.into(), next.into()], IndexMap::default(), IndexMap::default());
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let count = |body: &LirBody, name: &str| {
            body.blocks
                .iter()
                .flat_map(|block| block.insns.iter())
                .filter(|one| one.what.as_ref().and_then(|what| what.name.as_deref()) == Some(name))
                .count()
        };
        assert_eq!(count(&body, "add"), 2, "the premise: two adds");
        let shared = shared(&body, &RegisterClasses::m16());
        assert_eq!(count(&shared, "add"), 1);
        let copy_of_v1 = shared.blocks.iter().flat_map(|block| block.insns.iter()).any(|one| one.defines == [10]);
        assert!(!copy_of_v1, "the copy nothing reads is erased");
    }

    /// `PerformSimplePRE` hoisted an expression into a block that ends in `ret`
    /// (a conditional-return form), where the pin's `isLegalToHoistInto`
    /// refuses a return block. Two blocks compute `not v1` (no flags: the
    /// pin does not hoist an operation that defines EFLAGS), one reaching
    /// the other; their nearest common dominator is block 0.
    #[test]
    fn test_nothing_is_hoisted_into_a_return_block() {
        let add = |at: i64, result: u32| {
            insn(at, Operation::Unary, "not", vec![held(result)], vec![held(1)], vec![result], vec![1])
        };
        let jump = |at: i64| insn(at, Operation::Branch, "jle", vec![], vec![], vec![], vec![]);
        let ret = |at: i64| insn(at, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
        let build = |ends: Vec<Arc<Insn>>| {
            let entry = LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(0, ends) };
            let read = |at: i64, value: u32| {
                insn(at, Operation::Compare, "cmp", vec![], vec![held(value), held(value)], vec![], vec![value])
            };
            let left = LirBlock { succ: vec![2].into(), ..LirBlock::new(1, vec![add(1, 10), read(3, 10)]) };
            let right = LirBlock::new(2, vec![add(2, 11), read(4, 11)]);
            let mut body = LirBody::new(
                "pre",
                0,
                vec![entry.into(), left.into(), right.into()],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1, 2].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            body
        };
        let adds = |body: &LirBody| {
            body.blocks
                .iter()
                .flat_map(|block| block.insns.iter())
                .filter(|one| one.what.as_ref().and_then(|what| what.name.as_deref()) == Some("not"))
                .count()
        };
        let plain = shared(&build(vec![jump(0)]), &RegisterClasses::m16());
        assert!(
            plain.blocks[0]
                .insns
                .iter()
                .any(|one| one.defines.len() == 1 && one.what.as_ref().and_then(|w| w.name.as_deref()) == Some("not")),
            "the premise: hoisted into block 0 without the return"
        );
        let returning = shared(&build(vec![jump(0), ret(0)]), &RegisterClasses::m16());
        assert!(
            !returning.blocks[0]
                .insns
                .iter()
                .any(|one| one.what.as_ref().and_then(|w| w.name.as_deref()) == Some("not")),
            "{returning:?}"
        );
        assert_eq!(adds(&returning), 2);
    }

    /// `isProfitableToHoistInto` answers yes at once under `hasMinSize`
    /// (MachineCSE.cpp#L922): the shortcut was missing (DEVIATE CSE1), so a
    /// minsize function kept two copies of an expression computed in two cold
    /// arms because their common dominator, run by every call, was hotter than
    /// the two together.
    #[test]
    fn test_a_minsize_function_hoists_an_expression_into_a_hotter_block() {
        let make = |at: i64, result: u32| {
            insn(at, Operation::Unary, "not", vec![held(result)], vec![held(1)], vec![result], vec![1])
        };
        let jump = |at: i64| insn(at, Operation::Branch, "jle", vec![], vec![], vec![], vec![]);
        let read = |at: i64, value: u32| {
            insn(at, Operation::Compare, "cmp", vec![], vec![held(value), held(value)], vec![], vec![value])
        };
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let build = |min_size: bool| {
            // 0 -> left (a tenth) | mid; left -> right; mid -> right (a tenth)
            // | exit.
            let mut body = LirBody::new(
                "cold",
                0,
                vec![
                    block(0, vec![1, 2], vec![jump(0)]).into(),
                    block(1, vec![3], vec![make(1, 10), read(5, 10)]).into(),
                    block(2, vec![3, 4], vec![jump(2)]).into(),
                    block(3, vec![], vec![make(3, 11), read(6, 11)]).into(),
                    block(4, vec![], vec![read(7, 1)]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            let tenth = (0.1 * crate::model::lir::BlockOdds::CERTAIN) as u32;
            let rest = crate::model::lir::BlockOdds::CERTAIN as u32 - tenth;
            for (edge, odds) in [((0, 1), tenth), ((0, 2), rest), ((2, 3), tenth), ((2, 4), rest)] {
                body.odds.taken.insert(edge, odds);
            }
            body.inputs = [1, 2].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            body.min_size = min_size;
            body
        };
        let in_entry = |body: &LirBody| {
            body.blocks[0].insns.iter().any(|one| one.what.as_ref().and_then(|w| w.name.as_deref()) == Some("not"))
        };
        assert!(!in_entry(&shared(&build(false), &RegisterClasses::m16())), "the premise: hotter, so not hoisted");
        assert!(in_entry(&shared(&build(true), &RegisterClasses::m16())), "minsize hoists it");
    }

    /// `bt a, 1; sbb m1 := x, x` and later `bt b, 1; sbb m2 := x, x`: the
    /// second read another carry, and sharing them gave every step of a CRC
    /// the first step's mask (bench crc under `--isel=dag` returned
    /// -46801).
    #[test]
    fn test_two_sbbs_of_the_same_values_after_different_compares_are_two_values() {
        let entry = LirBlock::new(
            0,
            vec![
                insn(0, Operation::Compare, "bt", vec![], vec![held(1), held(2)], vec![], vec![1, 2]),
                insn(1, Operation::Binary, "sbb", vec![held(10)], vec![held(3), held(3)], vec![10], vec![3]),
                insn(2, Operation::Compare, "bt", vec![], vec![held(4), held(2)], vec![], vec![4, 2]),
                insn(3, Operation::Binary, "sbb", vec![held(11)], vec![held(3), held(3)], vec![11], vec![3]),
                insn(4, Operation::Binary, "xor", vec![held(12)], vec![held(10), held(11)], vec![12], vec![10, 11]),
            ],
        );
        let mut body = LirBody::new("sbbs", 0, vec![entry.into()], IndexMap::default(), IndexMap::default());
        body.inputs = [1, 2, 3, 4].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let shared = shared(&body, &RegisterClasses::m16());
        let sbbs = shared
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| one.what.as_ref().and_then(|what| what.name.as_deref()) == Some("sbb"))
            .count();
        assert_eq!(sbbs, 2);
    }

    /// A constant made in a block and again in its successor stayed two
    /// instructions: `candidate` refused any operation with no register operand
    /// (CSE2) and heuristic 1 shared a cheap operation only inside one block.
    /// The pin shares it with a block that precedes by an edge (`x_life`'s
    /// skip-edge zero and loop zero, a second return block); not with a block
    /// two edges away.
    #[test]
    fn test_a_constant_is_shared_with_the_block_before_it_by_an_edge() {
        let zero = |at: i64, result: u32| {
            let imm = Loc::Imm(llrm_lir::Imm { value: 0, width: 4, address: None });
            insn(at, Operation::Move, "mov", vec![held(result)], vec![imm], vec![result], vec![])
        };
        let read = |at: i64, value: u32, result: u32| {
            insn(
                at,
                Operation::Binary,
                "add",
                vec![held(result)],
                vec![held(value), held(1)],
                vec![result],
                vec![value, 1],
            )
        };
        let build = |hops: usize| {
            let mut blocks =
                vec![LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![zero(0, 10), read(1, 10, 11)]) }];
            for hop in 1..=hops {
                let last = hop == hops;
                let insns =
                    if last { vec![zero(10 * hop as i64, 20), read(10 * hop as i64 + 1, 20, 21)] } else { vec![] };
                blocks.push(LirBlock {
                    succ: if last { vec![].into() } else { vec![hop as i64 + 1].into() },
                    ..LirBlock::new(hop as i64, insns)
                });
            }
            let mut body = LirBody::new(
                "zeros",
                0,
                blocks.into_iter().map(Into::<Block>::into).collect::<Vec<Block>>(),
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            body
        };
        let zeros = |body: &LirBody| {
            body.blocks
                .iter()
                .flat_map(|block| block.insns.iter())
                .filter(|one| one.what.as_ref().and_then(|what| what.name.as_deref()) == Some("mov"))
                .count()
        };
        let next = build(1);
        assert_eq!(zeros(&next), 2, "the premise: two zeros");
        assert_eq!(zeros(&shared(&next, &RegisterClasses::m16())), 1, "the next block reads the first");
        let far = build(2);
        assert_eq!(zeros(&shared(&far, &RegisterClasses::m16())), 2, "two edges away stays two");
    }

    /// A store of a constant is not a copy: heuristic 2 took any `mov` that
    /// reads the value for one, so the segment constant a hoisted poke loads
    /// twice (`pop es`, `pop fs`) stayed two values (qb textfill's Fill loop).
    #[test]
    fn test_a_constant_only_stores_read_is_made_once() {
        let make = |at: i64, result: u32| {
            let imm = Loc::Imm(llrm_lir::Imm { value: 47104, width: 2, address: None });
            insn(at, Operation::Move, "mov", vec![held(result)], vec![imm], vec![result], vec![])
        };
        let store = |at: i64, value: u32, disp: i64| {
            let cell = Loc::Mem(llrm_lir::Mem::new(Some(llrm_lir::Addr::new(llrm_lir::Space::Frame, disp)), 2));
            insn(at, Operation::Move, "mov", vec![cell], vec![held(value)], vec![], vec![value])
        };
        let mut body = LirBody::new(
            "stores",
            0,
            vec![LirBlock::new(0, vec![make(0, 10), make(1, 11), store(2, 10, 4), store(3, 11, 6)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let constants = |body: &LirBody| {
            body.blocks
                .iter()
                .flat_map(|block| block.insns.iter())
                .filter(|one| one.uses.is_empty() && !one.defines.is_empty())
                .count()
        };
        assert_eq!(constants(&body), 2, "the premise: two constants");
        assert_eq!(constants(&shared(&body, &RegisterClasses::m16())), 1);
    }

    /// Two blocks: `first` ends in a branch on the flags of `sub v1, v2`;
    /// `second` follows it (its only predecessor when `lone`) and compares the
    /// same operands before its own branch.
    fn compare_twice(
        between: Vec<Arc<Insn>>,
        lone: bool,
    ) -> LirBody {
        let first = LirBlock {
            succ: if lone { vec![1, 2] } else { vec![1, 3] }.into(),
            ..LirBlock::new(
                0,
                vec![
                    insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
                    insn(1, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
                ],
            )
        };
        let mut insns = between;
        insns.push(insn(4, Operation::Binary, "sub", vec![held(4)], vec![held(1), held(2)], vec![4], vec![1, 2]));
        insns.push(insn(5, Operation::Branch, "jle", vec![], vec![], vec![], vec![]));
        let second = LirBlock { succ: vec![2].into(), ..LirBlock::new(1, insns) };
        let third = LirBlock::new(2, vec![insn(6, Operation::Return, "ret", vec![], vec![], vec![], vec![])]);
        // with a second way into `second`: block 3, reached from the first
        let side = LirBlock { succ: vec![1].into(), ..LirBlock::new(3, vec![]) };
        let blocks = if lone {
            vec![first.into(), second.into(), third.into()]
        } else {
            vec![first.into(), second.into(), third.into(), side.into()]
        };
        let mut body = LirBody::new("twice", 0, blocks, IndexMap::default(), IndexMap::default());
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    fn subs(body: &LirBody) -> usize {
        body.blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| one.what.as_ref().and_then(|what| what.name.as_deref()) == Some("sub"))
            .count()
    }

    /// The pin's `PhysRegDefsReach`: the compare in the only successor of a
    /// block that made the same flags is the first one's, and `jle` reads
    /// them. x_binsearch -O2: `a[mid] < key` then `a[mid] > key` kept two
    /// compares, so the load had two readers and `cmp [m], r` was not made
    /// (+9.5% executed instructions).
    #[test]
    fn test_a_compare_repeated_in_the_sole_successor_reads_the_first_flags() {
        let body = compare_twice(vec![], true);
        assert_eq!(subs(&body), 2, "the premise: two compares");
        assert_eq!(subs(&shared(&body, &RegisterClasses::m16())), 1);
    }

    /// Something that writes the flags between the two compares breaks the
    /// reach.
    #[test]
    fn test_a_flag_write_between_the_two_compares_keeps_both() {
        let between =
            vec![insn(7, Operation::Binary, "add", vec![held(9)], vec![held(1), held(5)], vec![9], vec![1, 5])];
        let body = compare_twice(between, true);
        assert_eq!(subs(&shared(&body, &RegisterClasses::m16())), 2);
    }

    /// A block with a second way in is not the sole successor.
    #[test]
    fn test_a_block_with_two_predecessors_keeps_its_compare() {
        let body = compare_twice(vec![], false);
        assert_eq!(subs(&shared(&body, &RegisterClasses::m16())), 2);
    }
}
