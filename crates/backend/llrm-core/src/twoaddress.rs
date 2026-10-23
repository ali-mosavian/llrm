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
//! DEVIATE TA-4 (no exit while the target reads addresses through a prefix): a
//! dword or 16-bit result of a `lea` at m16 is the 32-bit prefixed form
//! (`lea ecx, [eax-1]`), which costs a prefix byte where the pin's `LEA32r` has
//! none, and the 16-bit address registers (BX/BP, SI/DI) are not used; there
//! the adds and subs convert and the shifts stay the peephole's, which fuses
//! them with the add after them (`ThreeAddress::AddSub`).
//! `upperzero::wide_roots` does not count a `lea` cell: it is arithmetic, not
//! an address read.
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

use crate::model::ir::{self, Loc, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::HashMap;

/// An instruction by identity (`Insn::id`).
pub type InstrKey = u64;

/// `MaxDataFlowEdge` (`-dataflow-edge-limit`, 10): how many copies and tied
/// definitions `isRevCopyChain` walks through.
pub const MAX_DATA_FLOW_EDGE: usize = 10;

/// What the walks ask of a body, as the pin asks it of `LiveIntervals`: who
/// names each value, which block each instruction is in, and where a value dies
/// (`isPlainlyKilled`). The one `LiveIntervals` of the function, which the
/// pass edits as it moves, rewrites and inserts instructions.
pub struct BodyFacts {
    lis: std::cell::RefCell<crate::liveness::liveintervals::LiveIntervals>,
}

impl BodyFacts {
    /// `LiveIntervals` of `body`, every value's interval computed
    /// (`LiveIntervals::runOnMachineFunction`).
    pub fn of(body: &LirBody) -> Self {
        // An interval is made when it is first asked, as the coalescer's was:
        // the pin computes every one, which no value that is never a
        // source or a copy needs.
        Self { lis: std::cell::RefCell::new(crate::liveness::liveintervals::LiveIntervals::new(body)) }
    }

    /// The instructions that define or read `value`, and whether each defines
    /// it (an operand read as undef is no reference).
    fn refs(
        &self,
        value: u32,
    ) -> Vec<(InstrKey, bool)> {
        let lis = self.lis.borrow();
        lis.operands(value).iter().filter(|op| op.is_def || op.reads_reg).map(|op| (op.instr, op.is_def)).collect()
    }

    fn block_of(
        &self,
        key: InstrKey,
    ) -> Option<usize> {
        let lis = self.lis.borrow();
        let at = lis.indexes.get_instruction_index(key);
        Some(lis.indexes.get_mbb_from_index(&at))
    }

    fn insn(
        &self,
        key: InstrKey,
    ) -> Arc<Insn> {
        Arc::clone(self.lis.borrow().instr(key))
    }

    /// `isPlainlyKilled(MI, Reg)`: the interval of `value` ends at the
    /// instruction (TwoAddressInstructionPass.cpp#L394-L483).
    pub fn killed(
        &self,
        key: InstrKey,
        value: u32,
    ) -> bool {
        let mut lis = self.lis.borrow_mut();
        let li = lis.interval_rc(value);
        // An undef read has no value to kill.
        if !li.has_at_least_one_value() {
            return false;
        }
        let at = lis.indexes.get_instruction_index(key);
        let i = li.find(&at);
        i != li.segments.len()
            && !li.segments[i].end.is_block()
            && crate::liveness::slotindex::SlotIndex::is_same_instr(&li.segments[i].end, &at)
    }

    /// `replaceMachineInstrInMaps`: `new` takes the place of `old` in `block`.
    pub fn rewrite(
        &self,
        block: usize,
        old: InstrKey,
        new: Arc<Insn>,
    ) {
        self.lis.borrow_mut().rewrite_instr(block, old, new);
    }

    /// `LiveIntervals::handleMove`: `key` goes before `before` (the end of the
    /// block without one).
    pub fn move_before(
        &self,
        block: usize,
        key: InstrKey,
        before: Option<InstrKey>,
    ) {
        self.lis.borrow_mut().handle_move(block, key, before);
    }

    /// `one` replaced by `made`, in order (the copy of a tied pair and the
    /// instruction that reads it): the last takes the place and the index of
    /// `one`, the others go before it. Only the intervals of the values the
    /// others name, and of those `one` and the last name differently, are made
    /// afresh (`processTiedPairs`: `removeInterval`,
    /// `createAndComputeVirtRegInterval`).
    pub fn replace(
        &self,
        block: usize,
        one: &Insn,
        made: &[Arc<Insn>],
    ) {
        use std::collections::BTreeSet;
        let named =
            |one: &Insn| -> BTreeSet<u32> { one.defines.iter().chain(&one.uses).chain(&one.undefs).copied().collect() };
        let (last, rest) = made.split_last().expect("an instruction replaced by none");
        let mut lis = self.lis.borrow_mut();
        lis.rewrite_instr(block, one.id(), Arc::clone(last));
        let mut changed: BTreeSet<u32> = named(one).symmetric_difference(&named(last)).copied().collect();
        for new in rest {
            lis.insert_instr_before(block, Some(last.id()), Arc::clone(new));
            changed.extend(named(new));
        }
        // Only an interval already made is redone; the others are made from the
        // body as it is.
        let held: Vec<u32> = changed.into_iter().filter(|value| lis.has_interval(*value)).collect();
        for value in held {
            lis.remove_interval(value);
            lis.interval(value);
        }
    }

    /// The function as edited, and its intervals left for the coalescer.
    pub fn finish(
        self,
        body: &LirBody,
    ) -> LirBody {
        let lis = self.lis.into_inner();
        let done = lis.body_like(body);
        if llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS") {
            // The premise: the intervals the edits left are the ones made
            // afresh.
            lis.assert_intervals_equal_afresh(&done);
        }
        crate::liveness::liveintervals::hand_over(lis, &done);
        done
    }

    /// The function as edited so far.
    pub fn body_like(
        &self,
        body: &LirBody,
    ) -> LirBody {
        self.lis.borrow().body_like(body)
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
        name: Some("lea".into()),
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
    processed: std::collections::HashSet<InstrKey>,
    /// The values `convertInstTo3Addr` made a two-register `lea` of in this
    /// block (`isConvertibleLEA`).
    leas: std::collections::HashSet<u32>,
}

impl<'a> Commuter<'a> {
    /// The walk of `block`, at its first instruction.
    pub fn new(
        facts: &'a BodyFacts,
        block: usize,
    ) -> Self {
        Self {
            facts,
            block,
            distance: HashMap::default(),
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
        if !self.processed.insert(one.id()) {
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
        let Some(last) = (at + 1..work.len()).rev().find(|&place| work[place].uses.contains(&reg)) else {
            return false;
        };
        if work[last + 1..].iter().any(|one| one.defines.contains(&reg)) || !self.facts.killed(work[last].id(), reg) {
            return false;
        }
        let kill = Arc::clone(&work[last]);
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
        if end > last {
            return false;
        }
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
        let Some(last) = (at + 1..work.len()).rev().find(|&place| work[place].uses.contains(&reg)) else {
            return false;
        };
        if work[last + 1..].iter().any(|one| one.defines.contains(&reg)) || !self.facts.killed(work[last].id(), reg) {
            return false;
        }
        let kill = Arc::clone(&work[last]);
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
        made.what = Some(Semantics { sources: vec![Loc::Held(*second), Loc::Held(*first)], ..what.clone() });
        Some(Arc::new(made))
    }
}
