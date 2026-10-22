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
//! A tied `add` or `sub` of a dword, whose first source lives on past it and
//! whose flags nothing reads, is made three-address: `d := lea [a + k]` or
//! `d := lea [a + b]` (`convertInstTo3Addr`, x86 `convertToThreeAddress`,
//! TwoAddressInstructionPass.cpp#L801, #L1402): no copy of `a` is made, so the
//! result can join a loop's register while `a` stays in its own.
//!
//! DEVIATE TA-4 (no exit while the target reads addresses through a prefix): a
//! dword or 16-bit result of a `lea` at m16 is the 32-bit prefixed form
//! (`lea ecx, [eax-1]`), which costs a prefix byte where the pin's `LEA32r` has
//! none, and the 16-bit address registers (BX/BP, SI/DI) are not used; there
//! the adds and subs convert and the shifts stay the peephole's, which fuses
//! them with the add after them (`ThreeAddress::AddSub`).
//! `upperzero::wide_roots` does not count a `lea` cell: it is arithmetic, not
//! an address read.
//!
//! DEVIATE TA-5 (exit: `isProfitableToConv3Addr` needs the register maps of
//! piece 4): a source that dies is never converted; the pin also converts one
//! that dies when the destination is mapped to another register.
//!
//! DEVIATE TA-6 (exit: `LiveIntervals` shared across phases, TwoAddress to
//! Coalescer, then revert to the pin's `isPlainlyKilled` on it; review
//! 2026-11-14): the kills come from one backward scan of each block over the
//! live-out rows (`BodyFacts`), not from `LiveIntervals`. The pin builds them
//! once and shares them with the coalescer; ours built a second set here (+3.0
//! M Ir on d_faces -O2 at m32, `LiveIntervals::new`), and the scan answers the
//! same question for a use: no later read, or a redefinition.
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
use crate::model::ir::{self, Loc, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::{HashMap, HashSet};

/// An instruction by identity (`Insn::id`).
pub type InstrKey = u64;

/// `MaxDataFlowEdge` (`-dataflow-edge-limit`, 10): how many copies and tied
/// definitions `isRevCopyChain` walks through.
pub const MAX_DATA_FLOW_EDGE: usize = 10;

/// What the walks ask of a body: who reads and writes each value, which block
/// each instruction is in, and where a value dies (`isPlainlyKilled`). The pin
/// reads the kills off `LiveIntervals`; here one backward scan of each block
/// from the live-out rows gives them, without the intervals, the slot indexes
/// and the trees that `LiveIntervals::new` builds (DEVIATE TA-6: the same
/// answer for a use, from less).
pub struct BodyFacts {
    /// By value: each instruction that names it and whether it defines it.
    refs: HashMap<u32, Vec<(InstrKey, bool)>>,
    block_of: HashMap<InstrKey, usize>,
    insn: HashMap<InstrKey, Arc<Insn>>,
    /// (instruction, value): the use is the value's last, nothing after reads
    /// it.
    killed: HashSet<(InstrKey, u32)>,
}

impl BodyFacts {
    pub fn of(body: &LirBody) -> Self {
        use crate::backend::allocate::LiveAt;
        let rows = crate::backend::allocate::live_rows(body);
        let mut facts = Self {
            refs: HashMap::default(),
            block_of: HashMap::default(),
            insn: HashMap::default(),
            killed: HashSet::default(),
        };
        for (at, block) in body.blocks.iter().enumerate() {
            // Whether each value read in this block is live after the point the
            // scan has reached: asked of the rows the first time,
            // then kept up.
            let mut live: HashMap<u32, bool> = HashMap::default();
            for one in block.insns.iter().rev() {
                let key = one.id();
                facts.block_of.insert(key, at);
                facts.insn.insert(key, Arc::clone(one));
                for &value in &one.defines {
                    facts.refs.entry(value).or_default().push((key, true));
                    live.insert(value, false);
                }
                for &value in &one.uses {
                    facts.refs.entry(value).or_default().push((key, false));
                    let after = *live.entry(value).or_insert_with(|| rows.live_out(block.at, value));
                    if !after {
                        facts.killed.insert((key, value));
                    }
                    live.insert(value, true);
                }
            }
        }
        facts
    }

    fn refs(
        &self,
        value: u32,
    ) -> &[(InstrKey, bool)] {
        self.refs.get(&value).map_or(&[], Vec::as_slice)
    }
}

/// Which tied instructions become a `lea`: none; the adds and subs (a target
/// that reads addresses through a prefix, where the fusion of a shift with the
/// add that follows stays the peephole's); or those and the shifts by 1 to 3.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThreeAddress {
    #[default]
    Off,
    AddSub,
    All,
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
    if into.width != 4 || base.width != 4 || into.value == base.value {
        return None;
    }
    let cell = match (&what.sources[1], what.name.as_deref()) {
        // `SHL32ri` by 1 to 3: `[a + a]` for 1, `[a*4]`, `[a*8]` for the rest
        // (the index alone takes a 32-bit displacement, as the pin's does).
        (Loc::Imm(Imm { value: shift @ 1..=3, address: None, .. }), Some("shl")) if mode == ThreeAddress::All => Mem {
            base: (*shift == 1).then_some(*base),
            index: Some(*base),
            scale: if *shift == 1 { 1 } else { 1 << shift },
            offset: 0,
            disp_width: if *shift == 1 { 0 } else { 4 },
            ..Mem::new(Some(ir::Addr::new(ir::Space::Literal, 0)), 4)
        },
        (_, Some("shl")) => return None,
        (Loc::Imm(Imm { value, address: None, .. }), name) => {
            let step = if name == Some("sub") { value.checked_neg()? } else { *value };
            Mem { base: Some(*base), offset: step, ..Mem::new(None, 4) }
        }
        (Loc::Held(index), Some("add")) if index.width == 4 && index.value != into.value => {
            Mem { base: Some(*base), index: Some(*index), scale: 1, offset: 0, ..Mem::new(None, 4) }
        }
        _ => return None,
    };
    let mut made = one.clone();
    made.what = Some(Semantics {
        op: Operation::Address,
        name: Some("lea".to_owned()),
        dests: vec![Loc::Held(*into)],
        sources: vec![Loc::Mem(cell)],
        ..Semantics::new(Operation::Address)
    });
    Some(Arc::new(made))
}

/// Whether the flags `insns[index]` writes are written again, in its block,
/// before anything reads them; a block that ends first leaves them live (the
/// pin reads the instruction's `implicit dead $eflags`).
pub fn flags_dead_after(
    insns: &[Arc<Insn>],
    index: usize,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> bool {
    use crate::backend::liveness;
    let flags = |lanes: &crate::backend::lanes::Lanes| {
        lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
    };
    for later in &insns[index + 1..] {
        let Some(effect) = liveness::effect(regs, bits, later) else { return false };
        if flags(&effect.all_reads()) {
            return false;
        }
        if flags(&effect.writes) {
            return true;
        }
    }
    false
}

/// The state of one block's walk: the intervals, the registers values are
/// fixed to and the instructions passed so far.
pub struct Commuter<'a> {
    facts: &'a BodyFacts,
    fixed: &'a Fixed,
    block: usize,
    /// `DistanceMap`: the position of each instruction passed in this block.
    distance: HashMap<InstrKey, u32>,
    dist: u32,
}

impl<'a> Commuter<'a> {
    /// The walk of `block`, at its first instruction.
    pub fn new(
        facts: &'a BodyFacts,
        fixed: &'a Fixed,
        block: usize,
    ) -> Self {
        Self { facts, fixed, block, distance: HashMap::default(), dist: 0 }
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
        for &(instr, is_def) in self.facts.refs(reg) {
            // A definition in another block is not counted (`DefMI.getParent()
            // != BB`).
            if !is_def || self.facts.block_of.get(&instr) != Some(&self.block) {
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
        crate::coalescer::pair::full_copy(&self.facts.insn[&key]).map(|(dst, src)| (src.value, dst.value))
    }

    /// `getTiedUse`: the value the instruction's definition of `def` is tied
    /// to (its first source) where it is one of the tied operations.
    fn tied_use(
        &self,
        key: InstrKey,
        def: u32,
    ) -> Option<u32> {
        let what = self.facts.insn[&key].what.as_ref().filter(|what| crate::backend::twoaddr::ties(what))?;
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
        for &(instr, is_def) in self.facts.refs(reg) {
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
        self.facts.killed.contains(&(key, reg))
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
            let mut defs = self.facts.refs(reg).iter().filter(|(_, is_def)| *is_def);
            let (Some(&(def, _)), None) = (defs.next(), defs.next()) else { return true };
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

    /// Whether `isKilled` says `reg` dies at the instruction.
    pub fn killed_at(
        &self,
        one: &Insn,
        reg: u32,
    ) -> bool {
        self.is_killed(one.id(), reg)
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
