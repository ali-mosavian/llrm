//! `TwoAddressInstructionPass`'s commuting: whether to swap the sources of a
//! commutative instruction so the destination can reuse the one that dies.
//! Translated from llvm/lib/CodeGen/TwoAddressInstructionPass.cpp at
//! d1106deb71cc (`isRevCopyChain`, `noUseAfterLastDef`, `isCopyToReg`,
//! `isPlainlyKilled`, `isKilled`, `isProfitableToCommute`, `commuteInstruction`,
//! `tryInstructionCommute`).
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
//! DEVIATE TA-1 (exit: `processCopy`/`scanUses` translated, piece 4 of
//! docs/working/twoaddress-translation-plan.md): `SrcRegMap` and `DstRegMap`
//! are not built by walking copies from and to physical registers. LIR says
//! which register a value arrives in (`delivers`) and must end in (`requires`)
//! on the instruction; `backend::twoaddr::Fixed` reads them, and a value read
//! once by a sum that ends in a register ends in it too.
//!
//! DEVIATE TA-2 (no exit while the target needs it): `hasCommutePreference`
//! (X86InstrInfo.cpp#L3152) concerns an `add` fed by an LEA that
//! X86FixupLEAs turns back into an `add`; no LEA is made before this pass
//! here, so there is no preference to give.
//!
//! DEVIATE TA-3 (exit: the translated pass owns the instruction list): the
//! pin sees a tied use through `isRegTiedToUseOperand`; here only the
//! commutative `Binary` `add`, `and`, `or`, `xor` and `Multiply` `imul`
//! of three values of one width are candidates, as `_swappable` finds them.
//!
//! # Example
//! `v3 := mov v1; v4 := add v1, v3` (the copy defines `v3`, `v1` lives on):
//! `v3` dies at the add and `v1` does not, so the add is `v4 := add v3, v1`
//! and the destination reuses `v3`.
//!
//! # Used by
//! `backend::twoaddr::tied`.

use std::sync::Arc;

use crate::backend::twoaddr::Fixed;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
use crate::model::ir::{self, Loc, Semantics};
use crate::model::lir::Insn;
use crate::support::hash::HashMap;

/// `MaxDataFlowEdge` (`-dataflow-edge-limit`, 10): how many copies and tied
/// definitions `isRevCopyChain` walks through.
pub const MAX_DATA_FLOW_EDGE: usize = 10;

/// The intervals the commuting asks of: the sources of each commutative instruction and, for
/// `isKilled`, the sources of the copies that define them. The pin has every interval at hand;
/// computing these alone is what keeps the pass off the cost of all of them.
pub fn prepare(
    lis: &mut LiveIntervals,
    body: &crate::model::lir::LirBody,
) {
    let mut todo: Vec<u32> = Vec::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        if let Some((_, first, second)) = crate::backend::twoaddr::swappable_held(one) {
            todo.extend([first.value, second.value]);
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    while let Some(value) = todo.pop() {
        if !seen.insert(value) {
            continue;
        }
        lis.interval_rc(value);
        let mut defs = lis.operands(value).iter().filter(|operand| operand.is_def);
        if let (Some(def), None) = (defs.next(), defs.next())
            && let Some((dst, src)) = crate::coalescer::pair::full_copy(lis.instr(def.instr))
            && dst.value == value
        {
            todo.push(src.value);
        }
    }
}

/// The state of one block's walk: the intervals, the registers values are
/// fixed to and the instructions passed so far.
pub struct Commuter<'a> {
    lis: &'a LiveIntervals,
    fixed: &'a Fixed,
    block: usize,
    /// `DistanceMap`: the position of each instruction passed in this block.
    distance: HashMap<InstrKey, u32>,
    dist: u32,
}

impl<'a> Commuter<'a> {
    /// The walk of `block`, at its first instruction. Every value named in the
    /// block must have its interval (`LiveIntervals::interval_rc`).
    pub fn new(
        lis: &'a LiveIntervals,
        fixed: &'a Fixed,
        block: usize,
    ) -> Self {
        Self { lis, fixed, block, distance: HashMap::default(), dist: 0 }
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
        for operand in self.lis.operands(reg).iter().filter(|operand| operand.is_def) {
            let at = self.lis.indexes.get_instruction_index(operand.instr);
            // A definition in another block is not counted (`DefMI.getParent() != BB`).
            if self.lis.indexes.get_mbb_from_index(&at) != self.block {
                continue;
            }
            if found.is_some_and(|key| key != operand.instr) {
                return None;
            }
            found = Some(operand.instr);
        }
        found
    }

    /// `isCopyToReg`: the (source, destination) of a copy between values.
    fn copy_to_reg(
        &self,
        key: InstrKey,
    ) -> Option<(u32, u32)> {
        crate::coalescer::pair::full_copy(self.lis.instr(key)).map(|(dst, src)| (src.value, dst.value))
    }

    /// `getTiedUse`: the value the instruction's definition of `def` is tied
    /// to (its first source) where it is one of the tied operations.
    fn tied_use(
        &self,
        key: InstrKey,
        def: u32,
    ) -> Option<u32> {
        let what = self.lis.instr(key).what.as_ref().filter(|what| crate::backend::twoaddr::ties(what))?;
        match (&what.dests[0], &what.sources[0]) {
            (Loc::Held(dst), Loc::Held(src)) if dst.value == def => Some(src.value),
            _ => None,
        }
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
        for operand in self.lis.operands(reg) {
            let Some(&at) = self.distance.get(&operand.instr) else { continue };
            if !operand.is_def && at < last_use {
                last_use = at;
            }
            if operand.is_def && at > last_def {
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
        let interval = self.lis.get_interval(reg);
        if !interval.has_at_least_one_value() {
            return false;
        }
        let at = self.lis.indexes.get_instruction_index(key);
        interval.query(&at).is_kill()
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
            // Several definitions: go with what the interval says.
            let mut defs = self.lis.operands(reg).iter().filter(|operand| operand.is_def);
            let (Some(def), None) = (defs.next(), defs.next()) else { return true };
            at = def.instr;
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
    ) -> bool {
        if !self.is_plainly_killed(key, c) {
            return false;
        }
        if let Some(&to_a) = self.fixed.to.get(&a) {
            let from_b = self.fixed.from.get(&b).copied();
            let from_c = self.fixed.from.get(&c).copied();
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
        // Neither is used in between: the one defined later has the shorter
        // interval.
        last_def_b != 0 && last_def_c != 0 && last_def_c > last_def_b
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
            commute = self.is_profitable_to_commute(into.value, first.value, second.value, key, dist);
        }
        if !commute {
            return None;
        }
        // `commuteInstruction`: the sources swap.
        let mut made = (**one).clone();
        made.what = Some(Semantics { sources: vec![Loc::Held(*second), Loc::Held(*first)], ..what.clone() });
        Some(Arc::new(made))
    }
}
