//! `TwoAddressInstructionPass`'s commuting: whether to swap the sources of a
//! commutative instruction so the destination can reuse the one that dies.
//! Translated from llvm/lib/CodeGen/TwoAddressInstructionPass.cpp at
//! d1106deb71cc (`isRevCopyChain`, `noUseAfterLastDef`, `isCopyToReg`,
//! `isPlainlyKilled`, `isKilled`, `isProfitableToCommute`,
//! `commuteInstruction`, `tryInstructionCommute`, `isProfitableToConv3Addr`,
//! `convertInstTo3Addr`).
//!
//! # How it works
//! A block is walked in order with a distance map (`DistanceMap`: the position
//! of each instruction passed). For `c := a + b` with `a` the tied source:
//! the sources swap when `b` dies here and `a` does not, or when
//! `isProfitableToCommute` says so: `b` dies here, a register `c` is to end in
//! agrees with `b`'s and not `a`'s, a use of `a` lies between its last
//! definition and here, a copy chain runs from `b` to `c`, or `b` was defined
//! after `a`. A value dies where the value's interval ends at the
//! instruction (`isPlainlyKilled`); looking through the copies that define it
//! gives `isKilled`.
//!
//! `SrcRegMap` and `DstRegMap` are made as the walk passes the copies from and
//! to registers (`processCopy`, `scanUses`, `removeClobberedSrcRegMap`). LIR
//! says which register a value arrives in (`delivers`) and must end in
//! (`requires`) on the instruction, where the pin has a COPY beside it (DEVIATE
//! TA-1: the copies are those fields, at the instruction carrying them).
//!
//! `hasCommutePreference` (X86InstrInfo.cpp#L3152): an `add` fed by an LEA that
//! this pass made takes the LEA's result as its second source, so X86FixupLEAs
//! (`optLEAALU`) turns the pair into two adds.
//!
//! DEVIATE TA-3 (exit: the translated pass owns the instruction list): the
//! pin sees a tied use through `isRegTiedToUseOperand`; here only the
//! commutative `Binary` `add`, `and`, `or`, `xor` and `Multiply` `imul`
//! of three values of one width are candidates, as `_swappable` finds them.
//!
//! A tied `add` or `sub` of a dword, whose first source lives on past it and
//! whose flags nothing reads, is made three-address: `d := lea [a + k]` or
//! `d := lea [a + b]` (`convertInstTo3Addr`, x86 `convertToThreeAddress`,
//! TwoAddressInstructionPass.cpp#L801, #L1402): no copy of `a` is made, so the
//! result can join a loop's register while `a` stays in its own.
//!
//! Where the target's dword address form is behind a prefix (m16;
//! `Profile::three_address`, from the target's address forms) a dword or 16-bit
//! result of a `lea` is the 32-bit prefixed form (`lea ecx, [eax-1]`), a byte
//! longer than the pin's `LEA32r`, and the 16-bit address registers (BX/BP,
//! SI/DI) are not used. There the adds and subs convert and the shifts stay the
//! peephole's, which fuses them with the add after them
//! (`ThreeAddress::AddSub`). `upperzero::wide_roots` does not count a `lea`
//! cell: it is arithmetic, not an address read.
//!
//! `isProfitableToConv3Addr`: a source that dies is also converted when the
//! sum is to end in a register other than the one the source arrives in
//! (`backend::twoaddr::profitable_to_convert`, from the maps of TA-1).
//!
//! `rescheduleMIBelowKill` moves a tied instruction below the one local reader
//! that kills its source (and the copies of its result), so the source dies at
//! the tied instruction and no copy is made
//! (TwoAddressInstructionPass.cpp#L928). A load never moves (the pin's latency
//! test refuses it, DEVIATE TA-7: no itineraries), and the pin's copy chain is
//! moved in order, where its splice reverses a chain of two.
//!
//! DEVIATE TA-8 (exit: loads folded into their reader before this pass, as isel
//! does in the pin): a value a load defines is not the operand that is tied
//! (`is_profitable_to_commute`).
//!
//! The kills are `isPlainlyKilled` on one `LiveIntervals` the pass edits
//! (`handleMove`, `rewrite_instr`, the intervals of a tied copy made afresh)
//! and hands to the coalescer (DEVIATE TA-6 retired: a backward scan of the
//! live-out rows stood in for it).
//!
//! # Example
//! `v3 := mov v1; v4 := add v1, v3` (the copy defines `v3`, `v1` lives on):
//! `v3` dies at the add and `v1` does not, so the add is `v4 := add v3, v1`
//! and the destination reuses `v3`.
//!
//! # Used by
//! `backend::twoaddr::tied`.

use std::sync::Arc;

use crate::model::ir::{self, Held, Loc, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::HashMap;

/// An instruction by identity (`Insn::id`).
pub type InstrKey = u64;

/// `MaxDataFlowEdge` (`-dataflow-edge-limit`, 10): how many copies and tied
/// definitions `isRevCopyChain` walks through.
pub const MAX_DATA_FLOW_EDGE: usize = 10;

/// What the walks ask of a body, as the pin asks it of
/// `MachineRegisterInfo` and `LiveVariables`: who names each value, which block
/// each instruction is in, and where a value dies (`isPlainlyKilled`). The
/// pass edits the body through it, and the kills follow as the pin's calls
/// make them follow.
pub struct BodyFacts {
    state: std::cell::RefCell<State>,
}

struct State {
    insns: HashMap<InstrKey, Arc<Insn>>,
    /// The instructions of each block by order stamp: the stamps rise along the
    /// block, with gaps, so an instruction goes between two others (or moves)
    /// without renumbering the rest, as the pin's intrusive list does.
    blocks: Vec<std::collections::BTreeMap<u64, Arc<Insn>>>,
    /// The stamp of each instruction in its block.
    stamp_of: HashMap<InstrKey, u64>,
    block_of: HashMap<InstrKey, usize>,
    /// The instructions that read or define each value, and whether each
    /// defines it (`use_def_nodbg_operands`).
    operands: HashMap<u32, indexmap::IndexSet<(InstrKey, bool)>>,
    /// The instructions that define each value: a few, where a value read N
    /// times lists N operands.
    defs: HashMap<u32, indexmap::IndexSet<InstrKey>>,
    lv: crate::liveness::livevariables::LiveVariables,
}

// The instructions the reschedules looked at to find a source's kill, for a
// test that the search does not walk the rest of the block.
#[cfg(test)]
thread_local! {
    pub(crate) static KILL_SEARCHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// The values whose kills were made again from a block, for a test that an edit
// remakes only the ones whose naming changed.
#[cfg(test)]
thread_local! {
    static REFRESHED: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl State {
    /// The kills of `value` in `block` made again from the block as it is: the
    /// block's boundary is what the edits leave as it was, so what the
    /// instructions in it say is all that moved (DEVIATE LV3: the pin makes
    /// the same changes by `addVirtualRegisterKilled` and
    /// `removeVirtualRegisterKilled` where the instructions change).
    fn refresh_kills(
        &mut self,
        value: u32,
        block: usize,
    ) {
        use crate::liveness::livevariables::{Kill, Site};
        #[cfg(test)]
        REFRESHED.with(|seen| seen.borrow_mut().push(value));
        let live_in = self.lv.is_live_in_block(value, block);
        let live_out = self.lv.is_live_out_block(value, block);
        // Only the instructions naming the value can end a segment of it: they
        // are read from its operand list, not found by walking the block.
        let mut naming: Vec<(u64, &Arc<Insn>)> = self
            .operands
            .get(&value)
            .into_iter()
            .flatten()
            .filter(|(key, _)| self.block_of.get(key) == Some(&block))
            .map(|(key, _)| (self.stamp_of[key], &self.insns[key]))
            .collect();
        naming.sort_unstable_by_key(|(stamp, _)| *stamp);
        naming.dedup_by_key(|(stamp, _)| *stamp);
        let sites = crate::liveness::livevariables::segment_kills(
            value,
            naming.into_iter().map(|(_, one)| &**one),
            live_in,
            live_out,
        );
        let info = self.lv.get_var_info(value);
        info.kills.retain(|kill| kill.block != block);
        info.kills.extend(sites.into_iter().map(|site: Site| Kill { site, block }));
    }

    /// `count` stamps strictly between `lo` and `hi` (the block's ends where
    /// none), renumbering the block when the gap is too small.
    fn stamps_between(
        &mut self,
        block: usize,
        lo: Option<u64>,
        hi: Option<u64>,
        count: usize,
    ) -> (Vec<u64>, Option<u64>, Option<u64>) {
        const GAP: u64 = 1 << 20;
        let fits = |lo: Option<u64>, hi: Option<u64>| {
            let (low, high) = (lo.map_or(0, |one| one + 1), hi.unwrap_or(u64::MAX / 2));
            (high > low && (high - low) as u128 > count as u128).then_some((low, high))
        };
        let (mut lo, mut hi) = (lo, hi);
        let span = match fits(lo, hi) {
            Some(span) => span,
            None => {
                // Renumber the block evenly and look for the neighbours again.
                let (lo_key, hi_key) = (
                    lo.map(|one| Arc::clone(&self.blocks[block][&one])),
                    hi.map(|one| Arc::clone(&self.blocks[block][&one])),
                );
                let old = std::mem::take(&mut self.blocks[block]);
                for (n, (_, one)) in old.into_iter().enumerate() {
                    let stamp = (n as u64 + 1) * GAP;
                    self.stamp_of.insert(one.id(), stamp);
                    self.blocks[block].insert(stamp, one);
                }
                lo = lo_key.map(|one| self.stamp_of[&one.id()]);
                hi = hi_key.map(|one| self.stamp_of[&one.id()]);
                fits(lo, hi).expect("a renumbered block has room")
            }
        };
        let step = (span.1 - span.0) / (count as u64 + 1);
        ((1..=count as u64).map(|n| span.0 + n * step).collect(), lo, hi)
    }

    /// [`State::refresh_kills`] of every value `one` names.
    fn refresh_named(
        &mut self,
        one: &Insn,
        block: usize,
    ) {
        let mut values: Vec<u32> = one.uses.iter().chain(&one.defines).copied().collect();
        values.sort_unstable();
        values.dedup();
        for value in values {
            self.refresh_kills(value, block);
        }
    }

    /// `made` (one or more instructions, in order) takes the place of `old`.
    /// A value `old` only reads and none of `made` defines keeps its kill: the
    /// last of `made` to read it takes it (`replaceKillInstruction`, and the
    /// kill a tied pair's copy takes from the source). One instruction in the
    /// place of another that names a value the same way does the same. A value
    /// named otherwise is made again from the block, once (a full rescan of it
    /// for every value named was 1.18 M Ir of `replace`; one rescan per tied
    /// pair of a value read N times was quadratic in N).
    fn retarget_kills(
        &mut self,
        old: &Insn,
        made: &[Arc<Insn>],
        block: usize,
    ) {
        let mut values: Vec<u32> = old
            .uses
            .iter()
            .chain(&old.defines)
            .chain(made.iter().flat_map(|one| one.uses.iter().chain(&one.defines)))
            .copied()
            .collect();
        values.sort_unstable();
        values.dedup();
        for value in values {
            let role = |one: &Insn| (one.uses.contains(&value), one.defines.contains(&value));
            let heir = match made {
                [one] if role(old) == role(one) => Some(one),
                _ if role(old) == (true, false) && made.iter().all(|one| !one.defines.contains(&value)) => {
                    made.iter().rev().find(|one| one.uses.contains(&value))
                }
                _ => None,
            };
            match heir {
                Some(one) => self.lv.replace_kill_instruction(value, old.id(), one.id()),
                None => self.refresh_kills(value, block),
            }
        }
    }

    fn add_operands(
        &mut self,
        one: &Insn,
    ) {
        for &value in &one.uses {
            self.operands.entry(value).or_default().insert((one.id(), false));
        }
        for &value in &one.defines {
            self.operands.entry(value).or_default().insert((one.id(), true));
            self.defs.entry(value).or_default().insert(one.id());
        }
    }

    fn remove_operands(
        &mut self,
        one: &Insn,
    ) {
        for &value in &one.defines {
            if let Some(list) = self.defs.get_mut(&value) {
                list.swap_remove(&one.id());
            }
        }
        for &value in one.uses.iter().chain(&one.defines) {
            if let Some(list) = self.operands.get_mut(&value) {
                // Both roles: an instruction that reads and defines a value is
                // listed twice.
                list.swap_remove(&(one.id(), false));
                list.swap_remove(&(one.id(), true));
            }
        }
    }
}

impl BodyFacts {
    /// `LiveVariables` of `body`, filled by a dataflow (DEVIATE LV3) and kept
    /// as the pass edits.
    pub fn of(body: &LirBody) -> Self {
        let mut state = State {
            insns: HashMap::default(),
            blocks: Vec::new(),
            stamp_of: HashMap::default(),
            block_of: HashMap::default(),
            operands: HashMap::default(),
            defs: HashMap::default(),
            lv: crate::liveness::livevariables::LiveVariables::from_dataflow(body),
        };
        for (at, block) in body.blocks.iter().enumerate() {
            let mut kept = std::collections::BTreeMap::new();
            for (n, one) in block.insns.iter().enumerate() {
                let stamp = (n as u64 + 1) << 20;
                kept.insert(stamp, Arc::clone(one));
                state.stamp_of.insert(one.id(), stamp);
                state.insns.insert(one.id(), Arc::clone(one));
                state.block_of.insert(one.id(), at);
                state.add_operands(one);
            }
            state.blocks.push(kept);
        }
        Self { state: std::cell::RefCell::new(state) }
    }

    /// The instructions that define or read `value`, and whether each defines
    /// it.
    fn refs(
        &self,
        value: u32,
    ) -> Vec<(InstrKey, bool)> {
        self.state.borrow().operands.get(&value).map(|list| list.iter().copied().collect()).unwrap_or_default()
    }

    fn block_of(
        &self,
        key: InstrKey,
    ) -> Option<usize> {
        self.state.borrow().block_of.get(&key).copied()
    }

    fn insn(
        &self,
        key: InstrKey,
    ) -> Arc<Insn> {
        Arc::clone(&self.state.borrow().insns[&key])
    }

    /// The last reader of `value` in `block` after `after`, when no definition
    /// of it follows: the block's latest kill (`LiveVariables::findKill`, kept
    /// as the pass edits), where the readers of a value read N times are not
    /// walked. A reader that is no kill is no answer to the callers, which ask
    /// `killed` of it.
    pub fn last_reader_after(
        &self,
        block: usize,
        after: InstrKey,
        value: u32,
    ) -> Option<InstrKey> {
        use crate::liveness::livevariables::Site;
        let state = self.state.borrow();
        let floor = *state.stamp_of.get(&after)?;
        let (stamp, key) = state
            .lv
            .var_info(value)?
            .kills
            .iter()
            .filter(|kill| kill.block == block)
            .filter_map(|kill| match kill.site {
                Site::Insn(key) => state.stamp_of.get(&key).map(|stamp| (*stamp, key)),
                Site::Phi(_) => None,
            })
            .max()?;
        let reads = state.insns[&key].uses.contains(&value);
        let defined_after = state
            .defs
            .get(&value)
            .is_some_and(
                |defs| defs.iter().any(|def| state.block_of.get(def) == Some(&block) && state.stamp_of[def] > stamp),
            );
        (stamp > floor && reads && !defined_after).then_some(key)
    }

    /// `isPlainlyKilled(MI, Reg)`: `MI` is the kill of `value` in its block
    /// (`LV->getVarInfo(Reg).findKill(MI->getParent()) == MI`).
    pub fn killed(
        &self,
        key: InstrKey,
        value: u32,
    ) -> bool {
        use crate::liveness::livevariables::Site;
        let state = self.state.borrow();
        state.lv.var_info(value).is_some_and(|info| info.is_killed_by(Site::Insn(key)))
    }

    /// `replaceMachineInstrInMaps` and `LV->replaceKillInstruction`: `new`
    /// takes the place of `old` in `block`, and the kills `old` was.
    pub fn rewrite(
        &self,
        block: usize,
        old: InstrKey,
        new: Arc<Insn>,
    ) {
        let mut state = self.state.borrow_mut();
        let before = Arc::clone(&state.insns[&old]);
        state.remove_operands(&before);
        state.insns.remove(&old);
        state.block_of.remove(&old);
        let at = state.stamp_of.remove(&old).expect("an instruction of the block");
        state.blocks[block].insert(at, Arc::clone(&new));
        state.stamp_of.insert(new.id(), at);
        state.block_of.insert(new.id(), block);
        state.add_operands(&new);
        state.insns.insert(new.id(), Arc::clone(&new));
        state.retarget_kills(&before, std::slice::from_ref(&new), block);
    }

    /// The instruction `key` goes before `before` (the end of the block without
    /// one), and the kills of what it names follow its place
    /// (`removeVirtualRegisterKilled`/`addVirtualRegisterKilled` of
    /// `rescheduleMIBelowKill` and `rescheduleKillAboveMI`).
    pub fn move_before(
        &self,
        block: usize,
        key: InstrKey,
        before: Option<InstrKey>,
    ) {
        let mut state = self.state.borrow_mut();
        let from = state.stamp_of.remove(&key).expect("an instruction of the block");
        let moved = state.blocks[block].remove(&from).expect("an instruction of the block");
        let after = match before {
            Some(one) => state.stamp_of[&one],
            None => u64::MAX,
        };
        let lo = state.blocks[block].range(..after).next_back().map(|(stamp, _)| *stamp);
        let hi = (after != u64::MAX).then_some(after);
        let (stamps, ..) = state.stamps_between(block, lo, hi, 1);
        state.blocks[block].insert(stamps[0], Arc::clone(&moved));
        state.stamp_of.insert(key, stamps[0]);
        let moved = Arc::clone(&state.insns[&key]);
        state.refresh_named(&moved, block);
    }

    /// `one` replaced by `made`, in order (the copy of a tied pair and the
    /// instruction that reads it): the last takes the place of `one`, the
    /// others go before it, and the kills of every value they name are made
    /// again (`processTiedPairs`: the copy takes the source's kill when `one`
    /// no longer reads it).
    pub fn replace(
        &self,
        block: usize,
        one: &Insn,
        made: &[Arc<Insn>],
    ) {
        let (last, rest) = made.split_last().expect("an instruction replaced by none");
        let mut state = self.state.borrow_mut();
        let before = Arc::clone(&state.insns[&one.id()]);
        state.remove_operands(&before);
        state.insns.remove(&one.id());
        state.block_of.remove(&one.id());
        let at = state.stamp_of.remove(&one.id()).expect("an instruction of the block");
        let lo = state.blocks[block].range(..at).next_back().map(|(stamp, _)| *stamp);
        let (stamps, ..) = state.stamps_between(block, lo, Some(at), rest.len());
        // A renumbering moved `one` too: its stamp is read again.
        let at = if rest.is_empty() {
            at
        } else {
            state.blocks[block]
                .range(stamps[rest.len() - 1]..)
                .next()
                .map(|(stamp, _)| *stamp)
                .expect("the next of the block")
        };
        for (new, stamp) in rest.iter().zip(&stamps) {
            state.blocks[block].insert(*stamp, Arc::clone(new));
            state.stamp_of.insert(new.id(), *stamp);
            state.block_of.insert(new.id(), block);
            state.add_operands(new);
            state.insns.insert(new.id(), Arc::clone(new));
        }
        state.stamp_of.remove(&one.id());
        state.blocks[block].insert(at, Arc::clone(last));
        state.stamp_of.insert(last.id(), at);
        state.block_of.insert(last.id(), block);
        state.add_operands(last);
        state.insns.insert(last.id(), Arc::clone(last));
        state.retarget_kills(&before, made, block);
    }

    /// The function as edited.
    pub fn finish(
        self,
        body: &LirBody,
    ) -> LirBody {
        let done = self.body_like(body);
        if llrm_support::env_flag!("LLRM_CHECK_LIVEVARIABLES") {
            // The premise: the kills the edits left are the ones a fresh
            // dataflow finds.
            let fresh = crate::liveness::livevariables::LiveVariables::from_dataflow(&done);
            assert_eq!(
                self.state.borrow().lv.shape(),
                fresh.shape(),
                "kills after TwoAddress differ from a fresh dataflow"
            );
        }
        done
    }

    /// The function as edited so far.
    pub fn body_like(
        &self,
        body: &LirBody,
    ) -> LirBody {
        let state = self.state.borrow();
        let blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(at, block)| block.with_insns(state.blocks[at].values().cloned().collect()))
            .collect();
        body.with_blocks(blocks)
    }
}

/// An instruction nothing moves past: control flow, a call, a push or pop, a
/// string operation, a fixed register, a volatile access or an unmodeled
/// effect.
fn barrier(one: &Insn) -> bool {
    use crate::model::ir::Operation as Op;
    let Some(what) = one.what.as_ref() else { return true };
    matches!(
        what.op,
        Op::Jump
            | Op::Branch
            | Op::Call
            | Op::Return
            | Op::Escape
            | Op::Barrier
            | Op::Push
            | Op::Pop
            | Op::Leave
            | Op::Fill
            | Op::Copy
            | Op::Restore
            | Op::Data
            | Op::Exchange
    ) || one.call.is_some()
        || one.volatile
        || !one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || one.group.is_some()
}

/// `isSafeToMove` with a store seen: arithmetic on values alone, no memory, no
/// fixed register.
fn movable(one: &Insn) -> bool {
    use crate::model::ir::Operation as Op;
    let Some(what) = one.what.as_ref() else { return false };
    !barrier(one)
        && matches!(
            what.op,
            Op::Binary | Op::Unary | Op::Move | Op::Extend | Op::Address | Op::Funnel | Op::Select
        )
        && what.dests.iter().chain(&what.sources).all(|place| {
            // A `lea` names a cell and reads none.
            matches!(place, Loc::Held(_) | Loc::Imm(_)) || (what.op == Op::Address && matches!(place, Loc::Mem(_)))
        })
}

fn flag_lanes(
    one: &Insn,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> Option<(bool, bool)> {
    use crate::backend::liveness;
    let flags = |lanes: &crate::backend::lanes::Lanes| {
        lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
    };
    let effect = liveness::effect(regs, bits, one)?;
    Some((flags(&effect.all_reads()), flags(&effect.writes)))
}

/// Whether `one` reads or writes the flags (an unknown effect does both).
fn flags_touched(
    one: &Insn,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> bool {
    flag_lanes(one, regs, bits).is_none_or(|(reads, writes)| reads || writes)
}

/// Whether `one` neither reads nor writes the flags (an unknown effect does
/// both).
fn flags_untouched(
    one: &Insn,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> bool {
    flag_lanes(one, regs, bits).is_some_and(|(reads, writes)| !reads && !writes)
}

/// Which tied instructions become a `lea`: none; the adds and subs (a target
/// that reads addresses through a prefix, where the fusion of a shift with the
/// add that follows stays the peephole's); or those and the shifts by 1 to 3.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThreeAddress {
    #[default]
    Off,
    AddSub,
    /// `AddSub`, and the add or sub of a word and an immediate whose source
    /// takes a 16-bit address form: `lea ax, [bx + k]`.
    AddSubWords,
    All,
    /// `All` on a target that addresses with 64-bit registers: `lea eax, [rdi +
    /// k]` (`LEA64_32r`), the base read at its full width.
    Wide,
}

/// The `lea` of `d := a + k` or `d := a + b`, three-address, or None where the
/// instruction is not such an add or sub of dwords: the instruction
/// `convertToThreeAddress` makes of an `ADD32ri`, `ADD32rr` or `SUB32ri`.
pub fn three_address(
    one: &Insn,
    mode: ThreeAddress,
) -> Option<Arc<Insn>> {
    use crate::model::ir::{Imm, Mem, Operation};
    if mode == ThreeAddress::Off {
        return None;
    }
    let what = one.what.as_ref()?;
    // `INC32r` and `DEC32r`: `lea [a + 1]`, `lea [a - 1]`.
    if what.op == Operation::Unary
        && let Some(name @ ("inc" | "dec")) = what.name.as_deref()
        && what.dests.len() == 1
        && what.sources.len() == 1
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && let (Loc::Held(into), Loc::Held(base)) = (&what.dests[0], &what.sources[0])
        && into.width == 4
        && base.width == 4
        && into.value != base.value
    {
        let offset = if name == "inc" { 1 } else { -1 };
        let mut made = one.clone();
        made.what = Some(Semantics {
            op: Operation::Address,
            name: Some("lea".into()),
            dests: vec![Loc::Held(*into)],
            sources: vec![Loc::Mem(Mem {
                base: Some(if mode == ThreeAddress::Wide { Held { width: 8, ..*base } } else { *base }),
                offset,
                ..Mem::new(None, 4)
            })],
            ..Semantics::new(Operation::Address)
        });
        return Some(Arc::new(made));
    }
    if what.op != Operation::Binary
        || !matches!(what.name.as_deref(), Some("add" | "sub" | "shl"))
        || what.dests.len() != 1
        || what.sources.len() != 2
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let (Loc::Held(into), Loc::Held(base)) = (&what.dests[0], &what.sources[0]) else { return None };
    let width = into.width;
    let word = width == 2
        && mode == ThreeAddress::AddSubWords
        && matches!(what.sources[1], Loc::Imm(Imm { address: None, .. }));
    if !(width == 4 || word) || base.width != width || into.value == base.value {
        return None;
    }
    let wide = |held: &Held| if mode == ThreeAddress::Wide { Held { width: 8, ..*held } } else { *held };
    let cell = match (&what.sources[1], what.name.as_deref()) {
        // `SHL32ri` by 1 to 3: `[a + a]` for 1, `[a*4]`, `[a*8]` for the rest
        // (the index alone takes a 32-bit displacement, as the pin's does).
        (Loc::Imm(Imm { value: shift @ 1..=3, address: None, .. }), Some("shl"))
            if matches!(mode, ThreeAddress::All | ThreeAddress::Wide) =>
        {
            Mem {
                base: (*shift == 1).then_some(wide(base)),
                index: Some(wide(base)),
                scale: if *shift == 1 { 1 } else { 1 << shift },
                offset: 0,
                disp_width: if *shift == 1 { 0 } else { 4 },
                ..Mem::new(Some(ir::Addr::new(ir::Space::Literal, 0)), 4)
            }
        }
        (_, Some("shl")) => return None,
        (Loc::Imm(Imm { value, address: None, .. }), name) => {
            let step = if name == Some("sub") { value.checked_neg()? } else { *value };
            // A word unit step stays `inc`: with the source in an address
            // register the `lea` is three bytes against the INC's one, and
            // with it elsewhere the copy it needs makes five against three.
            if word && step.abs() == 1 {
                return None;
            }
            Mem { base: Some(wide(base)), offset: step, ..Mem::new(None, width) }
        }
        (Loc::Held(index), Some("add")) if index.width == 4 && index.value != into.value => {
            Mem { base: Some(wide(base)), index: Some(wide(index)), scale: 1, offset: 0, ..Mem::new(None, 4) }
        }
        _ => return None,
    };
    let mut made = one.clone();
    made.what = Some(Semantics {
        op: Operation::Address,
        name: Some("lea".into()),
        dests: vec![Loc::Held(*into)],
        sources: vec![Loc::Mem(cell)],
        ..Semantics::new(Operation::Address)
    });
    Some(Arc::new(made))
}

/// Whether the flags `insns[index]` writes are written again, in its block,
/// before anything reads them, or are dead past its end (`dead_out`: no block
/// after reads them first; the pin reads the instruction's
/// `implicit dead $eflags`).
pub fn flags_dead_after(
    insns: &[Arc<Insn>],
    index: usize,
    regs: llrm_lir::registers::Regs,
    bits: u32,
    dead_out: bool,
) -> bool {
    use crate::backend::precse::{Flags, flags_after};
    match flags_after(regs, bits, insns, index, false) {
        Flags::Read => false,
        Flags::Dead => true,
        Flags::Out => dead_out,
    }
}

/// What `SrcRegMap` and `DstRegMap` map a value to: another value, or a
/// register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mapped {
    Value(u32),
    Register(llrm_lir::registers::RegId),
}

/// The state of one block's walk: the intervals, the maps of values to the
/// registers they may end up in (`SrcRegMap`, `DstRegMap`, made as the walk
/// passes the copies, `processCopy`) and the instructions passed so far.
pub struct Commuter<'a> {
    facts: &'a BodyFacts,
    block: usize,
    /// `DistanceMap`: the position of each instruction passed in this block.
    distance: HashMap<InstrKey, u32>,
    dist: u32,
    /// `SrcRegMap`: a value to the register it arrives in, or the value it is
    /// tied to a use of.
    src: HashMap<u32, Mapped>,
    /// `DstRegMap`: a value to the register it ends in, or the value it dies
    /// into.
    dst: HashMap<u32, Mapped>,
    /// `Processed`: the instructions whose copies are in the maps.
    processed: crate::support::hash::HashSet<InstrKey>,
    /// The values `convertInstTo3Addr` made a two-register `lea` of in this
    /// block (`isConvertibleLEA`).
    leas: llrm_support::hash::HashSet<u32>,
}

impl<'a> Commuter<'a> {
    /// The walk of `block`, at its first instruction.
    pub fn new(
        facts: &'a BodyFacts,
        block: usize,
    ) -> Self {
        Self::sized(facts, block, 0)
    }

    /// [`Commuter::new`] for a block of `size` instructions: the distance map
    /// is made at its length, not grown by doubling.
    pub fn sized(
        facts: &'a BodyFacts,
        block: usize,
        size: usize,
    ) -> Self {
        Self {
            facts,
            block,
            distance: HashMap::with_capacity_and_hasher(size, Default::default()),
            dist: 0,
            src: HashMap::default(),
            dst: HashMap::default(),
            processed: Default::default(),
            leas: Default::default(),
        }
    }

    /// `getMappedReg`: the register `value` may be mapped to, following the
    /// values.
    fn mapped(
        map: &HashMap<u32, Mapped>,
        value: u32,
    ) -> Option<llrm_lir::registers::RegId> {
        let mut at = Mapped::Value(value);
        // A cycle of values maps to no register.
        for _ in 0..=map.len() {
            match at {
                Mapped::Register(register) => return Some(register),
                Mapped::Value(value) => at = *map.get(&value)?,
            }
        }
        None
    }

    /// `getMappedReg(value, SrcRegMap)`: the register `value` arrives in.
    pub fn src_register(
        &self,
        value: u32,
    ) -> Option<llrm_lir::registers::RegId> {
        Self::mapped(&self.src, value)
    }

    /// `getMappedReg(value, DstRegMap)`: the register `value` ends in.
    pub fn dst_register(
        &self,
        value: u32,
    ) -> Option<llrm_lir::registers::RegId> {
        Self::mapped(&self.dst, value)
    }

    /// `processCopy`: the copies of `one` from and to a register (`delivers`
    /// and `requires`), once.
    pub fn process_copy(
        &mut self,
        one: &Insn,
    ) {
        // Only an instruction that carries a copy is read, and once.
        if one.requires.is_empty() && one.delivers.is_empty() || !self.processed.insert(one.id()) {
            return;
        }
        // The copy to a register, before the instruction.
        for (held, register) in &one.requires {
            self.dst.entry(held.value).or_insert(Mapped::Register(*register));
        }
        // The copy from a register, after it: the value's uses are followed.
        for (held, register) in &one.delivers {
            self.src.entry(held.value).or_insert(Mapped::Register(*register));
            self.scan_uses(held.value);
        }
    }

    /// `findOnlyInterestingUse`: where all of `reg`'s uses are in this block,
    /// the one that kills it when it is a copy to a register or another
    /// value, or a tied use: that instruction, whether it is a copy, and
    /// where `reg` goes.
    fn find_only_interesting_use(
        &self,
        reg: u32,
    ) -> Option<(Arc<Insn>, bool, Mapped)> {
        let mut killing = None;
        for (key, is_def) in self.facts.refs(reg) {
            if is_def {
                continue;
            }
            if self.facts.block_of(key) != Some(self.block) {
                return None;
            }
            if self.facts.killed(key, reg) {
                killing = Some(key);
            }
        }
        let one = self.facts.insn(killing?);
        // A copy to a register (the requirement that `reg` is in it).
        if let Some(register) = one.requires.iter().find(|(held, _)| held.value == reg).map(|(_, register)| *register) {
            return Some((one, true, Mapped::Register(register)));
        }
        // A copy to another value.
        if let Some((dst, src)) = crate::coalescer::pair::full_copy(&one)
            && src.value == reg
        {
            return Some((one, true, Mapped::Value(dst.value)));
        }
        // A tied use: the result is the first source's register.
        if let Some(what) = one.what.as_ref().filter(|what| crate::backend::twoaddr::ties(what))
            && let (Some(Loc::Held(into)), Some(Loc::Held(first))) = (what.dests.first(), what.sources.first())
            && first.value == reg
        {
            let into = into.value;
            return Some((one, false, Mapped::Value(into)));
        }
        // The other operand of a commutative instruction is the tied one.
        if let Some((into, first, second)) = crate::backend::twoaddr::swappable_held(&one)
            && second.value == reg
            && first.value != reg
        {
            let into = into.value;
            return Some((one, false, Mapped::Value(into)));
        }
        None
    }

    /// `scanUses`: the chain of copies and tied uses `dst_reg` goes through,
    /// into `SrcRegMap` (each to the value before it) and `DstRegMap` (each
    /// to where the chain ends).
    fn scan_uses(
        &mut self,
        dst_reg: u32,
    ) {
        let mut pairs: Vec<Mapped> = Vec::new();
        let mut reg = dst_reg;
        while let Some((one, is_copy, next)) = self.find_only_interesting_use(reg) {
            if is_copy && !self.processed.insert(one.id()) {
                break;
            }
            // Earlier in this block, reached by a back edge.
            if self.distance.contains_key(&one.id()) {
                break;
            }
            let Mapped::Value(next_value) = next else {
                pairs.push(next);
                break;
            };
            self.src.insert(next_value, Mapped::Value(reg));
            pairs.push(next);
            reg = next_value;
        }
        let Some(mut to) = pairs.pop() else { return };
        while let Some(from) = pairs.pop() {
            let Mapped::Value(from_value) = from else { break };
            self.dst.entry(from_value).or_insert(to);
            to = from;
        }
        self.dst.entry(dst_reg).or_insert(to);
    }

    /// `convertInstTo3Addr`, after `three` replaced `old`: it has the position
    /// `old` had (`DistanceMap`), and `SrcRegMap[regA]` and
    /// `DstRegMap[regB]` are gone.
    pub fn converted_to_three_address(
        &mut self,
        old: InstrKey,
        three: InstrKey,
        into: u32,
        base: u32,
    ) {
        if let Some(&at) = self.distance.get(&old) {
            self.distance.insert(three, at);
        }
        self.src.remove(&into);
        self.dst.remove(&base);
    }

    /// `tryInstructionTransform`'s `scanUses(regA)`: where `value`, the result
    /// of a tied instruction, goes.
    pub fn scan_uses_of(
        &mut self,
        value: u32,
    ) {
        self.scan_uses(value);
    }

    /// `processTiedPairs`, after the copy `into := mov first` was put before
    /// `mi`: the copy takes the distance `mi` had and `mi` the next
    /// (`DistanceMap`), and where `first` died at `mi` and `mi` reads it no
    /// more, `into` arrives as `first` did (`SrcRegMap[LastCopiedReg] =
    /// RegB`).
    pub fn tied_pair_processed(
        &mut self,
        copy: InstrKey,
        mi: InstrKey,
        into: u32,
        first: u32,
        first_dies_here: bool,
    ) {
        self.distance.insert(copy, self.dist);
        self.dist += 1;
        self.distance.insert(mi, self.dist);
        if first_dies_here {
            self.src.insert(into, Mapped::Value(first));
        }
    }

    /// `DistanceMap[key]`.
    pub fn distance_of(
        &self,
        key: InstrKey,
    ) -> Option<u32> {
        self.distance.get(&key).copied()
    }

    /// `removeClobberedSrcRegMap`: after `one`, a register it writes no longer
    /// holds what the entries mapped to it arrived as.
    pub fn remove_clobbered_src_reg_map(
        &mut self,
        one: &Insn,
    ) {
        if one.clobbers.is_empty() && one.clobbers_high.is_empty() && one.requires.is_empty() {
            return;
        }
        // A copy from a register into a value writes no register (the pin
        // returns early for it); one to a register writes it, unless
        // the value was mapped to it already (`%100 = COPY $r8 ... $r8
        // = COPY %100` keeps the entry).
        let mut written: Vec<llrm_lir::registers::RegId> =
            one.clobbers.iter().chain(&one.clobbers_high).copied().collect();
        for (held, register) in &one.requires {
            if !self.src_register(held.value).is_some_and(|mapped| Self::compatible(*register, mapped)) {
                written.push(*register);
            }
        }
        if written.is_empty() {
            return;
        }
        self.src.retain(|_, to| match to {
            Mapped::Register(register) => !written.iter().any(|clobbered| Self::compatible(*clobbered, *register)),
            Mapped::Value(_) => true,
        });
    }

    /// `DistanceMap.insert(MI, ++Dist)`: the instruction is passed; its
    /// position.
    pub fn pass(
        &mut self,
        one: &Insn,
    ) -> u32 {
        self.dist += 1;
        self.distance.insert(one.id(), self.dist);
        self.dist
    }

    /// `getSingleDef(Reg, MBB)`: the one instruction of this block that
    /// defines `reg`; none if it has two here. Definitions in other blocks are
    /// not counted.
    fn single_def(
        &self,
        reg: u32,
    ) -> Option<InstrKey> {
        let mut found = None;
        for (instr, is_def) in self.facts.refs(reg) {
            // A definition in another block is not counted (`DefMI.getParent()
            // != BB`).
            if !is_def || self.facts.block_of(instr) != Some(self.block) {
                continue;
            }
            if found.is_some_and(|key| key != instr) {
                return None;
            }
            found = Some(instr);
        }
        found
    }

    /// `isCopyToReg`: the (source, destination) of a copy between values.
    fn copy_to_reg(
        &self,
        key: InstrKey,
    ) -> Option<(u32, u32)> {
        crate::coalescer::pair::full_copy(&self.facts.insn(key)).map(|(dst, src)| (src.value, dst.value))
    }

    /// `getTiedUse`: the value the instruction's definition of `def` is tied
    /// to (its first source) where it is one of the tied operations.
    fn tied_use(
        &self,
        key: InstrKey,
        def: u32,
    ) -> Option<u32> {
        let one = self.facts.insn(key);
        let what = one.what.as_ref().filter(|what| crate::backend::twoaddr::ties(what))?;
        match (&what.dests[0], &what.sources[0]) {
            (Loc::Held(dst), Loc::Held(src)) if dst.value == def => Some(src.value),
            _ => None,
        }
    }

    /// Whether `reg` is defined in this block by one load (`mov r, [m]`, not
    /// volatile).
    fn is_load_def(
        &self,
        reg: u32,
    ) -> bool {
        let Some(key) = self.single_def(reg) else { return false };
        let one = self.facts.insn(key);
        !one.volatile
            && one.what.as_ref().is_some_and(|what| {
                what.op == crate::model::ir::Operation::Move
                    && matches!(
                        (what.dests.as_slice(), what.sources.as_slice()),
                        ([Loc::Held(_)], [Loc::Mem(_)])
                    )
            })
    }

    /// `isRevCopyChain`: a reversed copy chain from `from` to `to`:
    /// `tmp1 = copy tmp2; from = copy tmp1; to = add from ...; tmp2 = copy to`.
    /// The walk goes up through copies and, as `AnalyzeRevCopyTied` (on by
    /// default) says, through the tied source of a definition.
    pub fn is_rev_copy_chain(
        &self,
        from: u32,
        to: u32,
        max_len: usize,
    ) -> bool {
        let mut tmp = from;
        for _ in 0..max_len {
            let Some(def) = self.single_def(tmp) else { return false };
            if let Some((source, _)) = self.copy_to_reg(def) {
                tmp = source;
            } else if let Some(source) = self.tied_use(def, tmp) {
                // Tied use register matches def register: no copy chain, and
                // no progress to make.
                if source == tmp {
                    return false;
                }
                tmp = source;
            } else {
                return false;
            }
            if tmp == to {
                return true;
            }
        }
        false
    }

    /// `noUseAfterLastDef`: whether no instruction passed in this block uses
    /// `reg` between its last definition and `dist`; and where that
    /// definition is (0 where there is none).
    pub fn no_use_after_last_def(
        &self,
        reg: u32,
        dist: u32,
    ) -> (bool, u32) {
        let mut last_def = 0;
        let mut last_use = dist;
        for (instr, is_def) in self.facts.refs(reg) {
            let Some(&at) = self.distance.get(&instr) else { continue };
            if !is_def && at < last_use {
                last_use = at;
            }
            if is_def && at > last_def {
                last_def = at;
            }
        }
        (!(last_use > last_def && last_use < dist), last_def)
    }

    /// `isPlainlyKilled`: `reg`'s interval ends at the instruction.
    pub fn is_plainly_killed(
        &self,
        key: InstrKey,
        reg: u32,
    ) -> bool {
        self.facts.killed(key, reg)
    }

    /// `isKilled`: whether `reg`, used by the instruction, dies there, looking
    /// through the copies that define it: a copy of a value that lives on does
    /// not count as dying, so the sources commute and the copy joins.
    pub fn is_killed(
        &self,
        key: InstrKey,
        reg: u32,
    ) -> bool {
        let (mut at, mut reg) = (key, reg);
        loop {
            if !self.is_plainly_killed(at, reg) {
                return false;
            }
            // Several definitions: go with what the kill says.
            let defs: Vec<_> = self.facts.refs(reg).into_iter().filter(|(_, is_def)| *is_def).collect();
            let [(def, _)] = defs[..] else { return true };
            at = def;
            // A definition that is no copy is not going to be coalesced.
            let Some((source, _)) = self.copy_to_reg(at) else { return true };
            reg = source;
        }
    }

    /// `regsAreCompatible` (values here are fixed to registers of one root).
    fn compatible(
        left: llrm_lir::registers::RegId,
        right: llrm_lir::registers::RegId,
    ) -> bool {
        ir::root(left) == ir::root(right)
    }

    /// `isProfitableToCommute`: whether to swap `b` (the tied source) and `c`
    /// of `a := b op c` at distance `dist`.
    pub fn is_profitable_to_commute(
        &self,
        a: u32,
        b: u32,
        c: u32,
        key: InstrKey,
        dist: u32,
        prefers: Option<bool>,
    ) -> bool {
        if !self.is_plainly_killed(key, c) {
            return false;
        }
        // DEVIATE TA-8 (exit: loads folded into their reader before this pass,
        // as isel does in the pin): a value a load defines is no
        // operand to tie, the peephole folds it into the instruction
        // (`add eax, [esp+16]`). Tying the other one keeps that form; tying the
        // load gave `mov eax, [a]; add eax, ecx` and an extra register
        // (by_value in c32/regs.c: 5 instructions with `push ecx`
        // against the pin's 3).
        match (self.is_load_def(b), self.is_load_def(c)) {
            (false, true) => return false,
            (true, false) => return true,
            _ => {}
        }
        if let Some(to_a) = self.dst_register(a) {
            let from_b = self.src_register(b);
            let from_c = self.src_register(c);
            let comp_b = from_b.is_some_and(|register| Self::compatible(register, to_a));
            let comp_c = from_c.is_some_and(|register| Self::compatible(register, to_a));
            // B is not tied to a register and C agrees with A, or B is tied to
            // the wrong one and C is to the right one or to none.
            if (from_b.is_none() && comp_c) || (from_b.is_some() && !comp_b && (from_c.is_none() || comp_c)) {
                return true;
            }
            // And the other way about.
            if (from_c.is_none() && comp_b) || (from_c.is_some() && !comp_c && (from_b.is_none() || comp_b)) {
                return false;
            }
        }
        // A use of C between its last definition and here: leave it.
        let (clear_c, last_def_c) = self.no_use_after_last_def(c, dist);
        if !clear_c {
            return false;
        }
        // A use of B between its last definition and here: commute.
        let (clear_b, last_def_b) = self.no_use_after_last_def(b, dist);
        if !clear_b {
            return true;
        }
        if self.is_rev_copy_chain(c, a, MAX_DATA_FLOW_EDGE) {
            return true;
        }
        if self.is_rev_copy_chain(b, a, MAX_DATA_FLOW_EDGE) {
            return false;
        }
        // Another target preference (`hasCommutePreference`).
        if let Some(commute) = prefers {
            return commute;
        }
        // Neither is used in between: the one defined later has the shorter
        // interval.
        last_def_b != 0 && last_def_c != 0 && last_def_c > last_def_b
    }

    /// `a := lea [x + y]` was made in this block.
    pub fn note_lea(
        &mut self,
        value: u32,
    ) {
        self.leas.insert(value);
    }

    /// `X86InstrInfo::hasCommutePreference` (X86InstrInfo.cpp#L3152): `r3 = lea
    /// r1, r2; r5 = add r3, r4` is better as `add r4, r3`, which
    /// X86FixupLEAs then splits into two adds. `Some(true)` commutes.
    fn commute_preference(
        &self,
        what: &Semantics,
        first: u32,
        second: u32,
    ) -> Option<bool> {
        if what.name.as_deref() != Some("add") {
            return None;
        }
        if self.leas.contains(&first) {
            return Some(true);
        }
        self.leas.contains(&second).then_some(false)
    }

    /// Whether `isKilled` says `reg` dies at the instruction.
    pub fn killed_at(
        &self,
        one: &Insn,
        reg: u32,
    ) -> bool {
        self.is_killed(one.id(), reg)
    }

    /// `rescheduleMIBelowKill` for the tied instruction `work[at]` (its source
    /// `reg`): move it, with the copies that follow it, below the
    /// instruction that kills `reg`, where nothing in between objects. True
    /// when the list changed; `at` then holds what came after the instruction.
    pub fn reschedule_mi_below_kill(
        &mut self,
        work: &mut Vec<Arc<Insn>>,
        at: usize,
        regs: llrm_lir::registers::Regs,
        bits: u32,
    ) -> bool {
        let mi = Arc::clone(&work[at]);
        // Only an instruction the pass would untie: tied, and writing another
        // value than it reads.
        let Some(what) = mi.what.as_ref().filter(|what| crate::backend::twoaddr::ties(what)) else { return false };
        let (Some(Loc::Held(into)), Some(Loc::Held(source))) = (what.dests.first(), what.sources.first()) else {
            return false;
        };
        if into.value == source.value {
            return false;
        }
        let source = *source;
        let reg = source.value;
        if !self.distance.contains_key(&mi.id()) || !movable(&mi) {
            return false;
        }
        // The last reader of `reg` in the block, which must be the kill and no
        // later definition.
        let Some(kill_key) = self.facts.last_reader_after(self.block, mi.id(), reg) else { return false };
        if !self.facts.killed(kill_key, reg) {
            return false;
        }
        let kill = self.facts.insn(kill_key);
        if crate::coalescer::pair::full_copy(&kill).is_some() || barrier(&kill) {
            return false;
        }
        // `isTwoAddrUse`: a kill that ties `reg` itself stays.
        if let Some(what) = kill.what.as_ref()
            && crate::backend::twoaddr::ties(what)
            && matches!(
                what.sources.first(),
                Some(Loc::Held(held)) if held.value == reg
            )
        {
            return false;
        }
        let uses: Vec<u32> = mi.uses.clone();
        let mut defs: Vec<u32> = mi.defines.clone();
        let kills: Vec<u32> =
            mi.uses.iter().copied().filter(|&value| value != reg && self.killed_at(&mi, value)).collect();
        // The copies connected to it move down as well.
        let mut end = at + 1;
        while end < work.len()
            && let Some((dst, src)) = crate::coalescer::pair::full_copy(&work[end])
            && defs.contains(&src.value)
        {
            defs.push(dst.value);
            end += 1;
        }
        // The flags it reads or writes are not to be read or written between.
        let writes_flags = flags_touched(&mi, regs, bits);
        // The kill is within the eleven instructions the walk below allows.
        let Some(found) = work[end.min(work.len())..].iter().take(11).position(|one| {
            #[cfg(test)]
            KILL_SEARCHED.with(|n| n.set(n.get() + 1));
            one.id() == kill_key
        }) else {
            return false;
        };
        let last = end + found;
        let mut visited = 0;
        for other in &work[end..=last] {
            if visited > 10 {
                return false;
            }
            visited += 1;
            if barrier(other) {
                return false;
            }
            if writes_flags && !flags_untouched(other, regs, bits) {
                return false;
            }
            if other.defines.iter().any(|value| uses.contains(value) || defs.contains(value)) {
                return false;
            }
            for &value in &other.uses {
                if defs.contains(&value) {
                    return false;
                }
                let kills_here = self.facts.killed(other.id(), value);
                if value != reg && ((kills_here && uses.contains(&value)) || kills.contains(&value)) {
                    return false;
                }
                if value == reg && !kills_here {
                    return false;
                }
            }
        }
        let moved: Vec<Arc<Insn>> = work.drain(at..end).collect();
        let after = last - (end - at) + 1;
        for (place, one) in moved.into_iter().enumerate() {
            work.insert(after + place, one);
        }
        // `LIS->handleMove` of the copies and then the instruction, each
        // below what it reads: the last copy first, before what follows them,
        // so the block is well-formed (a def moved below its reader) at each
        // move (TwoAddressInstructionPass.cpp#L1066).
        let mut before = work.get(after + (end - at)).map(|one| one.id());
        for one in work[after..after + (end - at)].iter().rev() {
            self.facts.move_before(self.block, one.id(), before);
            before = Some(one.id());
        }
        self.distance.remove(&mi.id());
        true
    }

    /// `isDefTooClose`: whether the definition of `reg` that `work[at]` follows
    /// is too near to move a reader of `reg` above `work[at]`: `work[at]`
    /// itself defines it, a definition sits below it, or a load defines it
    /// one instruction above (a load takes 2, anything else 1:
    /// `getInstrLatency` without itineraries, TA-7).
    fn def_too_close(
        &self,
        reg: u32,
        dist: u32,
        mi: &Insn,
    ) -> bool {
        for (instr, is_def) in self.facts.refs(reg) {
            if !is_def || self.facts.block_of(instr) != Some(self.block) {
                continue;
            }
            let def = self.facts.insn(instr);
            if crate::coalescer::pair::full_copy(&def).is_some() {
                continue;
            }
            if instr == mi.id() {
                return true;
            }
            let Some(&at) = self.distance.get(&instr) else { return true };
            let latency =
                if def.what.as_ref().is_some_and(|what| what.sources.iter().any(|one| matches!(one, Loc::Mem(_)))) {
                    2
                } else {
                    1
                };
            if latency > dist - at {
                return true;
            }
        }
        false
    }

    /// `rescheduleKillAboveMI` for the tied instruction `work[at]`: the one
    /// later reader that kills its source goes above it, so the source dies
    /// at the tied instruction. True when the list changed; `at` then holds
    /// the moved reader.
    pub fn reschedule_kill_above_mi(
        &mut self,
        work: &mut Vec<Arc<Insn>>,
        at: usize,
        regs: llrm_lir::registers::Regs,
        bits: u32,
    ) -> bool {
        let mi = Arc::clone(&work[at]);
        let Some(what) = mi.what.as_ref().filter(|what| crate::backend::twoaddr::ties(what)) else { return false };
        let (Some(Loc::Held(into)), Some(Loc::Held(source))) = (what.dests.first(), what.sources.first()) else {
            return false;
        };
        if into.value == source.value {
            return false;
        }
        let reg = source.value;
        let Some(&dist) = self.distance.get(&mi.id()) else { return false };
        let Some(kill_key) = self.facts.last_reader_after(self.block, mi.id(), reg) else { return false };
        if !self.facts.killed(kill_key, reg) {
            return false;
        }
        let kill = self.facts.insn(kill_key);
        // A copy is left to the coalescer (the pin moves one only for a folded
        // load, which has no case here).
        if crate::coalescer::pair::full_copy(&kill).is_some() || !movable(&kill) {
            return false;
        }
        if let Some(what) = kill.what.as_ref()
            && crate::backend::twoaddr::ties(what)
            && matches!(
                what.sources.first(),
                Some(Loc::Held(held)) if held.value == reg
            )
        {
            return false;
        }
        let mut uses: Vec<u32> = Vec::new();
        let mut kills: Vec<u32> = Vec::new();
        for &value in &kill.uses {
            if self.def_too_close(value, dist, &mi) {
                return false;
            }
            let killed = self.facts.killed(kill.id(), value);
            if value == reg && !killed {
                return false;
            }
            uses.push(value);
            if killed && value != reg {
                kills.push(value);
            }
        }
        let touches_flags = flags_touched(&kill, regs, bits);
        // The kill is within the eleven instructions the walk below allows.
        let Some(found) = work[at + 1..].iter().take(11).position(|one| {
            #[cfg(test)]
            KILL_SEARCHED.with(|n| n.set(n.get() + 1));
            one.id() == kill_key
        }) else {
            return false;
        };
        let last = at + 1 + found;
        let mut visited = 0;
        for other in &work[at..last] {
            if visited > 10 {
                return false;
            }
            visited += 1;
            if barrier(other) {
                return false;
            }
            if touches_flags && !flags_untouched(other, regs, bits) {
                return false;
            }
            for &value in &other.uses {
                if kills.contains(&value) {
                    return false;
                }
                if other.id() != mi.id() && value == reg && !self.facts.killed(other.id(), value) {
                    return false;
                }
            }
            if other.defines.iter().any(|value| uses.contains(value)) {
                return false;
            }
        }
        let moved = work.remove(last);
        work.insert(at, moved);
        // `LIS->handleMove(*KillMI)`: above the tied instruction.
        self.facts.move_before(self.block, kill.id(), Some(mi.id()));
        self.distance.remove(&mi.id());
        true
    }

    /// `tryInstructionCommute` for the instruction `one` (at `dist`): the
    /// instruction with its two sources swapped, or none where it stays.
    /// `base_killed` is `isKilled` of its tied source.
    pub fn try_instruction_commute(
        &self,
        one: &Arc<Insn>,
        dist: u32,
    ) -> Option<Arc<Insn>> {
        let what = one.what.as_ref()?;
        let (into, first, second) = crate::backend::twoaddr::swappable_held(one)?;
        let key = one.id();
        let base_killed = self.is_killed(key, first.value);
        let other_killed = self.is_killed(key, second.value);
        let mut commute = !base_killed && other_killed;
        if !commute {
            commute = self.is_profitable_to_commute(
                into.value,
                first.value,
                second.value,
                key,
                dist,
                self.commute_preference(what, first.value, second.value),
            );
        }
        if !commute {
            return None;
        }
        // `commuteInstruction`: the sources swap.
        let mut made = (**one).clone();
        // A `cmov` also takes the opposite condition.
        let name = what.name.as_deref().and_then(|name| {
            (what.op == ir::Operation::Select).then(|| crate::backend::flagscopy::opposite_cmov(name)).flatten()
        });
        made.what = Some(Semantics {
            sources: vec![Loc::Held(*second), Loc::Held(*first)],
            name: name.map(Into::into).or_else(|| what.name.clone()),
            ..what.clone()
        });
        Some(Arc::new(made))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Held, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    /// `process_copy` entered every instruction of a block in a std `HashSet`
    /// (SipHash, 409 Ir an insert, 0.86 M of d_faces' 11.7 M in TwoAddress)
    /// though only an instruction that carries a copy has anything to read.
    #[test]
    fn test_process_copy_remembers_only_an_instruction_that_carries_a_copy() {
        let held = |value: u32| Held { value, width: 4 };
        let plain = {
            let what = Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Held(held(2))],
                sources: vec![Loc::Imm(crate::model::ir::Imm { value: 1, width: 4, address: None })],
                ..Semantics::new(Operation::Move)
            };
            Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![2], vec![]))
        };
        let arriving = {
            let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
            let mut one = Insn::new(1, Some((1, 1)), Some(what), vec![1], vec![]);
            one.delivers = vec![(held(1), llrm_lir::registers::RegId::EAX)];
            Arc::new(one)
        };
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![Arc::clone(&plain), Arc::clone(&arriving)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = BodyFacts::of(&body);
        let mut walk = Commuter::new(&facts, 0);
        walk.process_copy(&plain);
        assert_eq!(walk.processed.len(), 0, "a plain instruction is not remembered");
        walk.process_copy(&arriving);
        assert_eq!(walk.processed.len(), 1, "the premise: one that carries a copy is");
    }

    /// The distance map grew by doubling as a block was passed (2,409 inserts
    /// on d_faces at 150 Ir, rehashes among them); a block's walk starts with
    /// room for its instructions.
    #[test]
    fn test_a_blocks_distance_map_starts_with_room_for_its_instructions() {
        let body =
            LirBody::new("t", 0, vec![LirBlock::new(0, vec![]).into()], IndexMap::default(), IndexMap::default());
        let facts = BodyFacts::of(&body);
        assert!(Commuter::sized(&facts, 0, 100).distance.capacity() >= 100);
    }

    /// `replace` remade the kills of every value `one` and the new instructions
    /// name, scanning the block for each (1.18 M Ir on d_faces): a value named
    /// the same way before and after keeps its kill at the new instruction.
    #[test]
    fn test_replacing_a_tied_pair_remakes_only_the_kills_whose_naming_changed() {
        let held = |value: u32| Held { value, width: 4 };
        let make = |at: i64, op: Operation, name: &str, defines: Vec<u32>, uses: Vec<u32>| {
            let what = Semantics {
                name: Some(name.into()),
                dests: defines.iter().map(|&value| Loc::Held(held(value))).collect(),
                sources: uses.iter().map(|&value| Loc::Held(held(value))).collect(),
                ..Semantics::new(op)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
        };
        let one = make(1, Operation::Binary, "add", vec![3], vec![1, 2]);
        let body = LirBody::new(
            "t",
            0,
            vec![LirBlock::new(0, vec![make(0, Operation::Move, "mov", vec![1], vec![]), Arc::clone(&one)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let facts = BodyFacts::of(&body);
        let copy = make(2, Operation::Move, "mov", vec![3], vec![1]);
        let last = make(3, Operation::Binary, "add", vec![3], vec![3, 2]);
        REFRESHED.with(|seen| seen.borrow_mut().clear());
        facts.replace(0, &one, &[copy, last]);
        let seen = REFRESHED.with(|seen| seen.borrow().clone());
        assert!(!seen.is_empty(), "the premise: the copy's values are remade");
        assert!(!seen.contains(&2), "v2 is read by `one` and by the new add alike: {seen:?}");
    }

    /// A tied pair replaced in a long block walked the whole block for every
    /// value it named (quadratic in the block; the pin's lists are O(1)): the
    /// walk is now the instructions naming the value, whatever the block holds.
    #[test]
    fn test_replacing_tied_pairs_walks_the_instructions_naming_the_value_not_the_block() {
        use crate::liveness::livevariables::WALKED;
        let held = |value: u32| Held { value, width: 4 };
        let make = |at: i64, op: Operation, name: &str, defines: Vec<u32>, uses: Vec<u32>| {
            let what = Semantics {
                name: Some(name.into()),
                dests: defines.iter().map(|&value| Loc::Held(held(value))).collect(),
                sources: uses.iter().map(|&value| Loc::Held(held(value))).collect(),
                ..Semantics::new(op)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
        };
        let walked_in = |filler: i64| {
            let mut insns: Vec<Arc<Insn>> =
                (0..filler).map(|n| make(n, Operation::Move, "mov", vec![100 + n as u32], vec![])).collect();
            let adds: Vec<Arc<Insn>> =
                (0..20).map(|n| make(filler + n, Operation::Binary, "add", vec![10 + n as u32], vec![1, 2])).collect();
            insns.extend(adds.iter().cloned());
            let body =
                LirBody::new("t", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
            let facts = BodyFacts::of(&body);
            WALKED.with(|walked| walked.set(0));
            for (n, one) in adds.iter().enumerate() {
                let n = n as i64;
                let copy = make(10_000 + 2 * n, Operation::Move, "mov", vec![1000 + n as u32], vec![1]);
                let last =
                    make(10_001 + 2 * n, Operation::Binary, "add", vec![10 + n as u32], vec![1000 + n as u32, 2]);
                facts.replace(0, one, &[copy, last]);
            }
            WALKED.with(std::cell::Cell::get)
        };
        let (small, large) = (walked_in(250), walked_in(1000));
        assert!(small > 0, "the premise: the kills are remade");
        assert!(large <= 2 * small, "4x the block walked {large} against {small}: the whole block is walked");
    }

    fn tied_pairs_sharing_a_source(count: i64) -> (BodyFacts, Vec<Arc<Insn>>) {
        let held = |value: u32| Held { value, width: 4 };
        let make = |at: i64, op: Operation, name: &str, defines: Vec<u32>, uses: Vec<u32>| {
            let what = Semantics {
                name: Some(name.into()),
                dests: defines.iter().map(|&value| Loc::Held(held(value))).collect(),
                sources: uses.iter().map(|&value| Loc::Held(held(value))).collect(),
                ..Semantics::new(op)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
        };
        let adds: Vec<Arc<Insn>> =
            (0..count).map(|n| make(n, Operation::Binary, "add", vec![10 + n as u32], vec![1, 2])).collect();
        let body =
            LirBody::new("t", 0, vec![LirBlock::new(0, adds.clone()).into()], IndexMap::default(), IndexMap::default());
        let facts = BodyFacts::of(&body);
        let mut made = Vec::new();
        for (n, one) in adds.iter().enumerate() {
            let n = n as i64;
            let copy = make(10_000 + 2 * n, Operation::Move, "mov", vec![1000 + n as u32], vec![1]);
            let last = make(10_001 + 2 * n, Operation::Binary, "add", vec![10 + n as u32], vec![1000 + n as u32, 2]);
            facts.replace(0, one, &[Arc::clone(&copy), Arc::clone(&last)]);
            made.extend([copy, last]);
        }
        (facts, made)
    }

    /// N tied pairs read one source (the DAG's shifts and leas of one value,
    /// `equal` at 4096): every replace made the source's kills again from its
    /// N readers, 5.7 G Ir of twoaddr, x40 at 2N. The source keeps its kill by
    /// moving it to the copy that reads it last.
    #[test]
    fn test_n_tied_pairs_reading_one_source_never_remake_its_kills() {
        REFRESHED.with(|seen| seen.borrow_mut().clear());
        let (facts, made) = tied_pairs_sharing_a_source(20);
        let seen = REFRESHED.with(|seen| seen.borrow().clone());
        assert!(!seen.is_empty(), "the premise: the copies' own values are remade");
        assert!(!seen.contains(&1), "v1 is read by every copy: {seen:?}");
        assert!(facts.killed(made[2 * 19].id(), 1), "the last copy is v1's kill");
        assert!(!facts.killed(made[0].id(), 1), "the first is not");
    }

    /// `last_reader_after` walked every operand of a value read N times for
    /// each tied pair; the answer is the block's latest kill (the pin's
    /// `findKill`), and none once the value is defined after it.
    #[test]
    fn test_the_last_reader_is_the_latest_kill_unless_a_definition_follows() {
        let (facts, made) = tied_pairs_sharing_a_source(6);
        let first = made[0].id();
        assert_eq!(facts.last_reader_after(0, first, 1), Some(made[2 * 5].id()));
        assert_eq!(facts.last_reader_after(0, made[2 * 5].id(), 1), None, "nothing reads it after the last");
    }
}
