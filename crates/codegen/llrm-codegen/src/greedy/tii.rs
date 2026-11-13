//! `TargetInstrInfo::foldMemoryOperand` for the forms LIR selects: the spill
//! slot put in place of a register an instruction names, where x86 has that
//! form. Translated from llvm/lib/CodeGen/TargetInstrInfo.cpp
//! (`foldMemoryOperand`) and llvm/lib/Target/X86/X86InstrInfo.cpp
//! (`foldMemoryOperandImpl`) at d1106deb71cc.
//!
//! Also here: the spill and reload instructions (`storeRegToStackSlot`,
//! `loadRegFromStackSlot`) that `InlineSpiller` inserts.
//!
//! DEVIATE 1: X86's fold tables are not translated; a form exists when the
//! selector prices it (`is_encodable`), the one place that knows the
//! m16 and m32 encodings.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::allocresult::Error;
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::FrameInfo;
use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

/// `frame.WORD` at the width type this module uses.
const WORD: u32 = crate::backend::frame::WORD as u32;

/// What the greedy allocator asks of the target's instructions
/// (`TargetInstrInfo`). `X86InstrInfo` answers for LIR's x86 forms; another
/// ISA answers with its own.
pub trait TargetInstrInfo {
    /// Whether `what` is a plain move that copies its source whole
    /// (`isCopyInstr` for X86 `MOV`, not `MOVZX` or `MOVSX`).
    fn is_plain_move(
        &self,
        what: &Semantics,
    ) -> bool;

    /// `BuildMI(COPY)`: a copy of `from` into `to`, `width` bytes.
    fn copy_semantics(
        &self,
        to: u32,
        from: u32,
        width: u32,
    ) -> Semantics;

    /// The register `one` copies, when it is a plain copy and nothing else
    /// (`isCopyInstr`).
    fn copy_source(
        &self,
        one: &Insn,
    ) -> Option<u32> {
        let w = one.what.as_ref()?;
        match (w.dests.as_slice(), w.sources.as_slice()) {
            ([Loc::Held(_)], [Loc::Held(from)])
                if self.is_plain_move(w)
                    && one.uses == [from.value]
                    && one.group.is_none()
                    && one.requires.is_empty()
                    && one.delivers.is_empty()
                    && one.clobbers.is_empty() =>
            {
                Some(from.value)
            }
            _ => None,
        }
    }

    /// Whether `one` is a definition `isTriviallyReMaterializable` takes
    /// in shape: one register written, nothing pinned or destroyed.
    fn remat_shape(
        &self,
        one: &Insn,
    ) -> bool;

    /// Whether a live range split makes `def` again, not copies it: `entering`
    /// is the value starting a new piece, else it returns to the complement.
    /// The pin asks `isAsCheapAsAMove` at both (SplitKit.cpp#L658), so the
    /// default says yes to a literal; the spiller's
    /// `isTriviallyReMaterializable` is another question.
    fn remake_at_split(
        &self,
        _def: &Insn,
        _entering: bool,
    ) -> bool {
        true
    }

    /// `isTriviallyReMaterializable`.
    fn is_trivially_re_materializable(
        &self,
        def: &Insn,
        invariant: bool,
    ) -> bool;

    /// `isLoadFromInvariantOrConstantMemory` over a body: the values loaded
    /// from an immutable stack object nothing writes.
    fn argument_loads(
        &self,
        body: &crate::model::lir::LirBody,
    ) -> BTreeSet<u32>;

    /// The incoming argument cells (frame displacement, width) `argument_loads`
    /// takes a load of, though other instructions also define the value loaded:
    /// the pin remakes a load per value number, not per register.
    fn argument_load_defs(
        &self,
        _body: &crate::model::lir::LirBody,
    ) -> BTreeSet<(i64, u32)> {
        BTreeSet::new()
    }

    /// The cell `one` loads when it is a load `argument_load_defs` names: the
    /// default is an incoming argument's.
    fn remat_load_cell(
        &self,
        one: &Insn,
    ) -> Option<(i64, u32)> {
        super::remat::argument_load_cell(one)
    }

    /// The registers each value `one` names may take there (the class of the
    /// operand, `MCInstrDesc`'s `RegClass`), as (value, registers in order). A
    /// target whose classes live on its virtual registers says it here, so a
    /// register a split made takes the class its own instructions give
    /// (`recomputeRegClass`) once a join has narrowed its parent's.
    fn operand_orders(
        &self,
        _one: &Insn,
    ) -> Vec<(u32, Vec<RegId>)> {
        Vec::new()
    }

    /// The registers of the smallest class that holds `order` and has more
    /// (`getLargestLegalSuperClass`), or none when `order` is the largest: a
    /// register narrowed by a join is a proper subclass (`isProperSubClass`),
    /// and `recomputeRegClass` starts from the larger class.
    fn larger_order(
        &self,
        _order: &[RegId],
    ) -> Option<Vec<RegId>> {
        None
    }

    /// DEVIATE SPILL-FOLD: `Tuning::spill_fold`; none for the pin.
    fn spill_fold_clocks(&self) -> Option<llrm_mir::target::SpillFoldClocks> {
        None
    }

    /// DEVIATE SPILL-FOLD: the clocks of one split copy of `value` as `one`
    /// names it; none for the pin.
    fn spill_fold_copy_clocks(
        &self,
        _one: &Insn,
        _value: u32,
    ) -> Option<i64> {
        None
    }

    /// `getSpillWeightScaleFactor` of the class whose registers are `order`.
    fn spill_weight_scale(
        &self,
        _order: &[RegId],
    ) -> f32 {
        1.0
    }

    /// What one reference to `value` in `one` adds to its spill weight, as a
    /// multiple of LLVM's one (see `SpillWeightEnv::reference_weight`).
    fn reference_weight(
        &self,
        _one: &Insn,
        _value: u32,
    ) -> f32 {
        1.0
    }

    /// DEVIATE C5: the bytes `one` takes with `value` in `phys`, against a
    /// register that costs it nothing extra: a prefix, a longer form. Greedy
    /// takes the cheapest of the registers that are otherwise equal; the pin
    /// takes the first. 0: no difference between registers.
    fn encoding_bytes(
        &self,
        _one: &Insn,
        _value: u32,
        _phys: RegId,
    ) -> i64 {
        0
    }

    /// DEVIATE C5: registers of one class take the same `encoding_bytes` in
    /// every instruction, so greedy prices a class once. `None`: no sharing.
    fn encoding_class(
        &self,
        _phys: RegId,
    ) -> Option<u8> {
        None
    }

    /// The registers `one` is cheaper with, as (value, register): the target's
    /// `getRegAllocationHints` for the values `one` names.
    fn preferred_registers(
        &self,
        _one: &Insn,
    ) -> Vec<(u32, RegId)> {
        Vec::new()
    }

    /// `getInstrCost(COPY, {to, from})`: see `GreedyEnv::copy_register_cost`.
    fn copy_register_cost(
        &self,
        _to: RegId,
        _from: RegId,
        _size: bool,
    ) -> u32 {
        0
    }

    /// Whether writing `written` bytes of a `register`-byte register zeroes
    /// the rest of it (x86-64: a dword write zeroes bits 32..64). DEVIATE
    /// SPILL-ZEXT: such a value spills in a `written`-byte cell; the pin spills
    /// the whole register.
    fn write_zero_extends(
        &self,
        _written: u32,
        _register: u32,
    ) -> bool {
        false
    }

    /// `foldMemoryOperand`.
    fn fold_memory_operand(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
    ) -> Option<Arc<Insn>>;

    /// `foldMemoryOperand` with the register class of every value known: `xmm`
    /// says whether a value lives in an xmm register. The pin's opcodes carry
    /// the class (`MOVPQIto64rr` against `MOV64toPQIrr`); LIR's `movq` does
    /// not, so a target that folds a cross-class move needs it here.
    fn fold_memory_operand_classed(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
        _xmm: &dyn Fn(u32) -> bool,
    ) -> Option<Arc<Insn>> {
        self.fold_memory_operand(one, reg, cell)
    }

    /// `foldMemoryOperand` for one move of a parallel copy.
    fn fold_copy_member(
        &self,
        one: &Insn,
        values: &BTreeSet<u32>,
        frame: &mut dyn FrameInfo,
        home: &dyn Fn(u32) -> u32,
    ) -> Result<Option<Arc<Insn>>, Error>;

    /// `loadRegFromStackSlot`.
    fn load_reg_from_stack_slot(
        &self,
        beside: &Insn,
        into: u32,
        cell: &Mem,
    ) -> Arc<Insn>;

    /// `storeRegToStackSlot`.
    fn store_reg_to_stack_slot(
        &self,
        beside: &Insn,
        out_of: u32,
        cell: &Mem,
    ) -> Arc<Insn>;

    /// Whether the target stores one lane of a spilled register on its own: a
    /// store whose cell has an `offset` into the register's slot. The pin
    /// stores a sub-register with `storeRegToStackSlot(.., SubIdx)`.
    fn stores_lanes(&self) -> bool {
        false
    }

    /// Whether `one` reads the register every unprefixed access reads, so a
    /// split puts a reload of it ahead of `one`.
    fn needs_data_group(
        &self,
        one: &Insn,
    ) -> bool;
}

/// `X86InstrInfo` at the width and register classes of one compile.
pub struct X86InstrInfo<'a> {
    pub bits: u32,
    pub classes: &'a RegisterClasses,
    /// The module is BASIC's (`Target::basic`).
    pub basic_semantics: bool,
    /// `Tuning::address_reference_milliweight`.
    pub address_reference_milliweight: u32,
    /// `Tuning::spill_fold`.
    pub fold_spill: Option<llrm_mir::target::SpillFoldClocks>,
}

impl TargetInstrInfo for X86InstrInfo<'_> {
    fn is_plain_move(
        &self,
        what: &Semantics,
    ) -> bool {
        what.op == Operation::Move && what.name.as_deref() == Some("mov")
    }

    fn copy_semantics(
        &self,
        to: u32,
        from: u32,
        width: u32,
    ) -> Semantics {
        Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: to, width, offset: 0 })],
            sources: vec![Loc::Held(Held { value: from, width, offset: 0 })],
            ..Semantics::new(Operation::Move)
        }
    }

    fn remat_shape(
        &self,
        one: &Insn,
    ) -> bool {
        super::inlinespiller::remat_shape(one)
    }

    /// DEVIATE SPLIT-REMAT-ENTER (owner greedy-anyisa, exit 2026-12-08): a
    /// quadword symbol address (`LEA64r $rip`: `isReMaterializable` but not
    /// `isAsCheapAsAMove`, so the pin copies it) is made again where a piece
    /// starts and copied where it returns. Remade at both, as the code was,
    /// memory_expansion -O1 held 450 rip-leas to the pin's 318; copied at
    /// both, as the pin does, stft_fixed grew 37-61 B at every level (the
    /// pin itself is 49 B above the code that remade). Remade at the start
    /// only: stft_fixed, jpeg_fixed and memory_expansion are all at or under
    /// both. Exit: a priced remake (the copy's register against the lea's
    /// bytes) that reaches these without naming the entry and the exit
    /// apart.
    fn remake_at_split(
        &self,
        def: &Insn,
        entering: bool,
    ) -> bool {
        let address = def
            .what
            .as_ref()
            .is_some_and(
                |what| what.sources
                    .iter()
                    .any(
                        |source| matches!(
                            source,
                            Loc::Imm(imm) if imm.address.is_some() && imm.width == 8
                        ),
                    ),
            );
        entering || !address
    }

    /// DEVIATE RA-CLK (owner x86-m16, exit 2026-11-29; the A/B is on
    /// `Tuning::address_reference_milliweight`): a value that is an address
    /// component (the base, index or selector of a memory operand) weighs that
    /// many thousandths of a reference in the spill weight. Spilled, it must be
    /// reloaded into one of the few registers 16-bit addressing allows (bx,
    /// bp, si, di) before the access, where any other value is folded (`add
    /// r, [slot]`) or moves through any register: eviction leaves the
    /// others in memory first. The pin weighs every reference alike
    /// (`getSpillWeight`).
    fn reference_weight(
        &self,
        one: &Insn,
        value: u32,
    ) -> f32 {
        // Only where fewer registers can make an address than can hold a value.
        if self.classes.addressing.len() >= self.classes.available.len() {
            return 1.0;
        }
        let Some(what) = one.what.as_ref() else { return 1.0 };
        let held = |h: &Option<Held>| h.as_ref().is_some_and(|h| h.value == value);
        let component = what
            .sources
            .iter()
            .chain(what.dests.iter())
            .any(
                |loc| matches!(
                    loc,
                    Loc::Mem(m) if held(&m.base) || held(&m.index) || held(&m.selector)
                ),
            );
        if component { self.address_reference_milliweight as f32 / 1000.0 } else { 1.0 }
    }

    fn spill_fold_clocks(&self) -> Option<llrm_mir::target::SpillFoldClocks> {
        self.fold_spill
    }

    fn spill_fold_copy_clocks(
        &self,
        one: &Insn,
        value: u32,
    ) -> Option<i64> {
        let prices = self.fold_spill?;
        let wide = _width(one, value) * 8 > self.bits;
        Some(prices.copy + if wide { prices.wide_copy } else { 0 })
    }

    fn encoding_bytes(
        &self,
        one: &Insn,
        value: u32,
        phys: RegId,
    ) -> i64 {
        one.what.as_ref().map_or(0, |what| llrm_x86::register_cost::register_extra_bytes(what, value, phys))
    }

    fn encoding_class(
        &self,
        phys: RegId,
    ) -> Option<u8> {
        Some(llrm_x86::register_cost::encoding_class(phys))
    }

    fn is_trivially_re_materializable(
        &self,
        def: &Insn,
        invariant: bool,
    ) -> bool {
        super::remat::is_trivially_re_materializable(def, invariant)
    }

    fn argument_load_defs(
        &self,
        body: &crate::model::lir::LirBody,
    ) -> BTreeSet<(i64, u32)> {
        super::remat::argument_load_insns(body).into_iter().map(|(_, _, cell)| cell).collect()
    }

    fn argument_loads(
        &self,
        body: &crate::model::lir::LirBody,
    ) -> BTreeSet<u32> {
        let mut loads = super::remat::argument_loads(body);
        if !self.basic_semantics {
            loads.extend(x87_stable_loads(body));
        }
        loads
    }

    fn write_zero_extends(
        &self,
        written: u32,
        register: u32,
    ) -> bool {
        self.bits == 64 && written == 4 && register == 8
    }

    fn fold_memory_operand(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
    ) -> Option<Arc<Insn>> {
        fold_memory_operand(one, reg, cell, self.bits, self.classes)
    }

    fn fold_memory_operand_classed(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
        xmm: &dyn Fn(u32) -> bool,
    ) -> Option<Arc<Insn>> {
        fold_with_classes(one, reg, cell, self.bits, self.classes, Some(xmm))
    }

    fn fold_copy_member(
        &self,
        one: &Insn,
        values: &BTreeSet<u32>,
        frame: &mut dyn FrameInfo,
        home: &dyn Fn(u32) -> u32,
    ) -> Result<Option<Arc<Insn>>, Error> {
        fold_copy_member(one, values, frame, home)
    }

    fn load_reg_from_stack_slot(
        &self,
        beside: &Insn,
        into: u32,
        cell: &Mem,
    ) -> Arc<Insn> {
        load_reg_from_stack_slot(beside, into, cell)
    }

    fn store_reg_to_stack_slot(
        &self,
        beside: &Insn,
        out_of: u32,
        cell: &Mem,
    ) -> Arc<Insn> {
        store_reg_to_stack_slot(beside, out_of, cell)
    }

    fn needs_data_group(
        &self,
        one: &Insn,
    ) -> bool {
        crate::backend::target::needs_data_group(one)
    }
}

/// `foldMemoryOperand`: `one` with `cell` for every operand naming `reg`, or
/// `None` where that form does not exist. A narrower operand reads or writes
/// the low part of the cell.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetInstrInfo.cpp#L718>
pub fn fold_memory_operand(
    one: &Arc<Insn>,
    reg: u32,
    cell: &Mem,
    bits: u32,
    classes: &RegisterClasses,
) -> Option<Arc<Insn>> {
    fold_with_classes(one, reg, cell, bits, classes, None)
}

/// [`fold_memory_operand`] with `xmm`, when the caller knows it, saying which
/// values live in xmm registers: the folded form is then priced with every
/// operand in its own class, not in all-general or all-xmm registers (a
/// `movq r64, xmm` whose xmm source became memory was priced as the
/// `movq xmm, [mem]` it is not).
fn fold_with_classes(
    one: &Arc<Insn>,
    reg: u32,
    cell: &Mem,
    bits: u32,
    classes: &RegisterClasses,
    xmm: Option<&dyn Fn(u32) -> bool>,
) -> Option<Arc<Insn>> {
    let original = one.what.as_ref()?;
    // X86InstrInfo::foldMemoryOperandImpl, Ops = {0, 1}: `test r, r` becomes
    // `cmp r, 0` first, and that folds.
    let zero_compare = test_as_compare_with_zero(original, reg);
    let what = zero_compare.as_ref().unwrap_or(original);
    // A register a fixed place requires, a register inside an address, and a
    // second memory operand all refuse the fold.
    if one.requires.iter().chain(&one.delivers).any(|(held, _)| held.value == reg) {
        return None;
    }
    for (place, _) in classes.requirements(what) {
        let side = if place.side == "dest" { &what.dests } else { &what.sources };
        if matches!(
            side.get(place.index),
            Some(Loc::Held(held)) if held.value == reg
        ) {
            return None;
        }
    }
    let direct = |loc: &Loc| matches!(loc, Loc::Held(held) if held.value == reg);
    for loc in what.dests.iter().chain(&what.sources) {
        if !direct(loc)
            && (matches!(loc, Loc::Mem(_)) || crate::model::ir::values(loc).iter().any(|held| held.value == reg))
        {
            return None;
        }
    }
    let dests = what.dests.iter().filter(|loc| direct(loc)).count();
    let sources = what.sources.iter().filter(|loc| direct(loc)).count();
    // A copy group's member moves one place; it may not move memory to memory.
    if one.group.is_some() && dests + sources != 1 {
        return None;
    }
    // One operand, or the tied pair of a two-address form.
    let tied = dests == 1 && sources == 1 && direct(&what.dests[0]) && what.sources.first().is_some_and(direct);
    if dests + sources != 1 && !tied {
        return None;
    }
    // The cell is the width of the value's slot. An operand named wider than
    // it (a dword zero defining a word value) would write the next slot too
    // (grep.c: `mov dword ptr [bp-20], 0` zeroed a neighbour): a move of a
    // constant stores the cell's width of it, anything else is not folded.
    let narrow = what
        .dests
        .iter()
        .chain(&what.sources)
        .any(
            |loc| matches!(
                loc,
                Loc::Held(held) if held.value == reg && held.width as u32 > cell.width
            ),
        );
    if narrow && !(what.op == Operation::Move && matches!(what.sources.as_slice(), [Loc::Imm(_)])) {
        return None;
    }
    // A dword write in long mode also zeroes the register's upper half, which
    // a dword store into a qword cell leaves stale for the qword reload. A
    // constant is stored zero-extended at the cell's width; any other def is
    // not folded.
    let zeroes_upper = bits == 64
        && what.dests.iter().any(|loc| {
            matches!(
                loc,
                Loc::Held(held) if held.value == reg && held.width == 4 && cell.width == 8
            )
        });
    let constant_move = what.op == Operation::Move
        && matches!(
            what.sources.as_slice(),
            [Loc::Imm(imm)] if imm.address.is_none()
        );
    if zeroes_upper && !constant_move {
        return None;
    }
    let swap = |loc: &Loc| match loc {
        Loc::Held(held) if held.value == reg => {
            let width = if zeroes_upper { cell.width } else { (held.width as u32).min(cell.width) };
            Loc::Mem(Mem { width, ..cell.clone() })
        }
        _ => loc.clone(),
    };
    let narrow_imm = |loc: &Loc| match loc {
        Loc::Imm(imm) if narrow => Loc::Imm(Imm { width: imm.width.min(cell.width), ..imm.clone() }),
        Loc::Imm(imm) if zeroes_upper => {
            Loc::Imm(Imm { value: imm.value as u32 as i64, width: cell.width, ..imm.clone() })
        }
        _ => loc.clone(),
    };
    let made = Semantics {
        dests: what.dests.iter().map(swap).collect(),
        sources: what.sources.iter().map(swap).map(|loc| narrow_imm(&loc)).collect(),
        ..what.clone()
    };
    // Long mode builds an address with a rip-relative `lea`; a store of the
    // address to a cell has no encoding (the object writer dropped the
    // displacement and used a PC32 fixup, or refused the instruction).
    if bits == 64
        && made.dests.iter().any(|loc| matches!(loc, Loc::Mem(_)))
        && made.sources.iter().any(|loc| matches!(loc, Loc::Imm(imm) if imm.address.is_some()))
    {
        return None;
    }
    if !is_encodable(bits, &made, classes, xmm).ok()? {
        return None;
    }
    let mut out = (**one).clone();
    // The copy that defined a spilled value, folded: it is the spill store, and
    // `postrasink` may move it into the one successor that reads the cell.
    if dests == 1 && sources == 0 && made.op == Operation::Move {
        out.spill_store = true;
    }
    out.what = Some(made);
    out.defines.retain(|value| *value != reg);
    out.uses.retain(|value| *value != reg);
    out.undefs.retain(|value| *value != reg);
    Some(Arc::new(out))
}

/// `TEST16rr r, r` -> `CMP16ri r, 0` (and the other widths): the form the
/// fold takes when both operands are the spilled register.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L8004>
fn test_as_compare_with_zero(
    what: &Semantics,
    reg: u32,
) -> Option<Semantics> {
    let [Loc::Held(first), Loc::Held(second)] = what.sources.as_slice() else {
        return None;
    };
    if what.op != Operation::Compare
        || what.name.as_deref() != Some("test")
        || !what.dests.is_empty()
        || first != second
        || first.value != reg
    {
        return None;
    }
    Some(Semantics {
        name: Some("cmp".into()),
        sources: vec![Loc::Held(first.clone()), Loc::Imm(Imm { value: 0, width: first.width as u32, address: None })],
        ..what.clone()
    })
}

pub(crate) fn _with<F: FnOnce(&mut Insn)>(
    one: &Insn,
    change: F,
) -> Arc<Insn> {
    let mut made = one.clone();
    change(&mut made);
    Arc::new(made)
}

/// How wide this instruction reads or writes the value, defaulting to a word.
pub fn _width(
    one: &Insn,
    value: u32,
) -> u32 {
    for (held, _register) in one.requires.iter().chain(&one.delivers) {
        if held.value == value {
            return held.width;
        }
    }
    for (named, width) in &one.widths {
        if *named == value {
            return *width;
        }
    }
    let Some(what) = &one.what else {
        return WORD;
    };
    // A cell's base and index too.
    for place in what.dests.iter().chain(&what.sources) {
        for held in ir::values(place) {
            if held.value == value {
                return held.width;
            }
        }
    }
    WORD
}

pub(crate) fn _mov(
    into: Loc,
    out_of: Loc,
) -> Semantics {
    Semantics { name: Some("mov".into()), dests: vec![into], sources: vec![out_of], ..Semantics::new(Operation::Move) }
}

/// `X86InstrInfo::loadRegFromStackSlot`, as `InlineSpiller::insertReload` calls
/// it: the load that puts a spilled value back for one instruction.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L4840>
pub fn load_reg_from_stack_slot(
    beside: &Insn,
    into: u32,
    cell: &Mem,
) -> Arc<Insn> {
    let inserted = _inserted(
        beside,
        _mov(Loc::Held(Held { value: into, width: cell.width, offset: 0 }), Loc::Mem(cell.clone())),
        vec![into],
        Vec::new(),
    );
    _with(&inserted, |made| made.spill_reload = true)
}

/// The cell of the `width` bytes `offset` up in a spill slot: the same slot,
/// its `offset` one more than the lane's so that no lane has the offset a
/// whole cell has (0). [`lane_offset`] reads it back.
pub fn lane_cell(
    cell: &Mem,
    offset: u32,
    width: u32,
) -> Mem {
    Mem { offset: i64::from(offset) + 1, width, ..cell.clone() }
}

/// The bytes above the start of the slot a [`lane_cell`] names; None for a
/// cell that is a whole slot.
pub fn lane_offset(cell: &Mem) -> Option<u32> {
    (cell.addr.is_some() && cell.offset > 0).then(|| (cell.offset - 1) as u32)
}

/// `X86InstrInfo::storeRegToStackSlot`, as `InlineSpiller::insertSpill` calls
/// it: the store that puts a spilled value away as soon as it is written.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L4816>
pub fn store_reg_to_stack_slot(
    beside: &Insn,
    out_of: u32,
    cell: &Mem,
) -> Arc<Insn> {
    let inserted = _inserted(
        beside,
        _mov(Loc::Mem(cell.clone()), Loc::Held(Held { value: out_of, width: cell.width, offset: 0 })),
        Vec::new(),
        vec![out_of],
    );
    _with(&inserted, |made| made.spill_store = true)
}

/// An instruction that stands beside another and claims none of its bytes.
pub(crate) fn _inserted(
    beside: &Insn,
    what: Semantics,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = Insn::new(beside.at, Some((at, at)), Some(what), defines, uses);
    made.call = beside.call.clone();
    Arc::new(made)
}

/// `foldMemoryOperand` for one move of a parallel copy, with its spilled end
/// read or written where it lives. DEVIATE 2 (no number): LLVM has no
/// parallel copies; a member moves one place, and both ends spilled make a
/// memory-to-memory move.
pub(crate) fn fold_copy_member(
    one: &Insn,
    values: &BTreeSet<u32>,
    frame: &mut dyn FrameInfo,
    home: &dyn Fn(u32) -> u32,
) -> Result<Option<Arc<Insn>>, Error> {
    let Some(what) = &one.what else {
        return Ok(None);
    };
    if one.group.is_none() || what.op != Operation::Move {
        return Ok(None);
    }
    if what.dests.len() != 1 || what.sources.len() != 1 {
        return Ok(None);
    }
    let into: Vec<u32> = one.defines.iter().copied().filter(|v| values.contains(v)).collect();
    let outof: Vec<u32> = one.uses.iter().copied().filter(|v| values.contains(v)).collect();
    if into.is_empty() && outof.is_empty() {
        return Ok(None);
    }
    if !into.is_empty() && !outof.is_empty() {
        let dest = frame.cell(home(into[0]), _width(one, into[0]))?;
        let source = frame.cell(home(outof[0]), _width(one, outof[0]))?;
        return Ok(Some(_with(one, |made| {
            made.what =
                Some(Semantics { dests: vec![Loc::Mem(dest)], sources: vec![Loc::Mem(source)], ..what.clone() });
            made.defines = one.defines.iter().copied().filter(|value| !into.contains(value)).collect();
            made.uses = one.uses.iter().copied().filter(|value| !outof.contains(value)).collect();
            made.undefs.retain(|value| !into.contains(value) && !outof.contains(value));
        })));
    }
    let value = if into.is_empty() { outof[0] } else { into[0] };
    let cell = frame.cell(home(value), _width(one, value))?;
    if !into.is_empty() {
        return Ok(Some(_with(one, |made| {
            made.what = Some(Semantics { dests: vec![Loc::Mem(cell)], ..what.clone() });
            made.defines = one.defines.iter().copied().filter(|v| *v != value).collect();
            made.undefs.retain(|v| *v != value);
        })));
    }
    Ok(Some(_with(one, |made| {
        made.what = Some(Semantics { sources: vec![Loc::Mem(cell)], ..what.clone() });
        made.uses = one.uses.iter().copied().filter(|v| *v != value).collect();
        made.undefs.retain(|v| *v != value);
    })))
}

/// Whether this form exists, asked of the one place that knows.
pub(crate) fn is_encodable(
    bits: u32,
    what: &Semantics,
    classes: &RegisterClasses,
    xmm_of: Option<&dyn Fn(u32) -> bool>,
) -> Result<bool, Error> {
    let regs = classes.registers;
    // A scalar float operand (4 or 8 bytes) is an xmm register: the probe is
    // priced with every operand in the integer registers first, then in the
    // xmm ones. Vector widths are not probed (a legacy SSE memory operand
    // must be 16-byte aligned, which a spill cell is not).
    let pool = |xmm: bool, width: u32| -> Vec<RegId> {
        if xmm {
            if width == 4 || width == 8 { classes.xmm.clone() } else { Vec::new() }
        } else {
            classes
                .available
                .iter()
                .copied()
                .filter(|one| {
                    regs.width_of(regs.named(*one, i64::from(width)))
                        .filter(|_| regs.integer(regs.named(*one, i64::from(width))))
                        == Some(i64::from(width))
                })
                .collect()
        }
    };
    // A spill cell is made through the 16-bit frame register; a target whose
    // frame register is wider addresses it through its own.
    let wide_frame = regs.width_of(classes.frame).is_some_and(|width| width > 2);
    let framed = |operand: Loc| match operand {
        Loc::Mem(cell) if wide_frame && cell.through == crate::model::ir::FRAME => {
            Loc::Mem(Mem { through: classes.frame, ..cell })
        }
        other => other,
    };
    let priced = |xmm: bool| -> bool {
        let mut taken: IndexMap<u32, RegId> = IndexMap::default();
        let mut placed = |operand: &Loc| -> Loc {
            let Loc::Held(held) = operand else {
                return operand.clone();
            };
            if !taken.contains_key(&held.value) {
                let row = pool(xmm_of.map_or(xmm, |class| class(held.value)), u32::from(held.width));
                if taken.len() >= row.len() {
                    // refuses below, which is the safe answer
                    return operand.clone();
                }
                taken.insert(held.value, row[taken.len()]);
            }
            Loc::Reg(Reg { register: regs.named(taken[&held.value], i64::from(held.width)), width: held.width })
        };
        let probe = Semantics {
            dests: what.dests.iter().map(&mut placed).map(framed).collect(),
            sources: what.sources.iter().map(&mut placed).map(framed).collect(),
            ..what.clone()
        };
        crate::backend::select::priced_in(bits, &probe, 0, None, false, false, None).is_some()
    };
    // Every value in its own class when the caller knows them; else
    // all-general, then all-xmm.
    if xmm_of.is_some() {
        return Ok(priced(false));
    }
    Ok(priced(false) || (!classes.xmm.is_empty() && priced(true)))
}

/// DEVIATE D-RFP4 (beyond LLVM: gcc's `REG_EQUIV`, 854's `floatassign`): the
/// x87 `fld` of a global, or of an incoming argument's cell, that nothing may
/// write while the value lives is made again where it is read, as the pin does
/// for an invariant load. The pin takes only a load marked invariant, so it
/// spills the rest: nbody_single -O3 m32 spills 9 loaded globals (clang 50017
/// frame stores against gcc's 17011).
///
/// Left out of a BASIC module (`basic_semantics`): its handler sees a fault in
/// the statement that raised it, and reading the cell later moves where a
/// signalling NaN raises.
/// Complexity: one scan of the body (`_stable_loads_through`).
fn x87_stable_loads(body: &crate::model::lir::LirBody) -> BTreeSet<u32> {
    let mut loads = BTreeSet::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        let Some(what) = one.what.as_ref() else { continue };
        if what.op == Operation::FloatLoad && what.name.as_deref() == Some("fld") {
            loads.extend(one.defines.iter().copied());
        }
    }
    if loads.is_empty() {
        return loads;
    }
    crate::backend::bodyfacts::_stable_loads_through(body, &loads, &IndexMap::default()).into_keys().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Addr, Space};

    /// gfxdev.c at m16 -Os panicked in the spiller ("v48 is erased while an
    /// instruction names it"): a folded `add [cell], v` still listed the
    /// spilled register as undef.
    #[test]
    fn test_a_fold_drops_the_spilled_register_from_the_undefs() {
        let held = |value: u32| Loc::Held(Held { value, width: 2, offset: 0 });
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(1)],
            sources: vec![held(1), held(2)],
            ..Semantics::new(Operation::Binary)
        };
        let mut add = Insn::new(0, None, Some(what), vec![1], vec![1, 2]);
        add.undefs = vec![1];
        let cell = Mem::new(Some(Addr::new(Space::Frame, -2)), 2);
        let folded = fold_memory_operand(&Arc::new(add), 1, &cell, 16, &RegisterClasses::m16()).expect("folds");
        assert!(folded.undefs.is_empty(), "the folded add still names v1: {folded:?}");
    }

    /// rectwo -O1 m16 (88 B against main's 79): the loop's `test v, v` of a
    /// spilled word stayed `mov bx, [bp-6]; test bx, bx`. The pin folds the
    /// reload as `cmp word ptr [bp-6], 0` (`TEST16rr` is rewritten to `CMP16ri
    /// r, 0` first).
    #[test]
    fn test_a_test_of_a_spilled_register_folds_into_a_compare_with_zero() {
        let held = Loc::Held(Held { value: 1, width: 2, offset: 0 });
        let what = Semantics {
            name: Some("test".into()),
            sources: vec![held.clone(), held],
            ..Semantics::new(Operation::Compare)
        };
        let test = Insn::new(0, None, Some(what), vec![], vec![1]);
        let cell = Mem::new(Some(Addr::new(Space::Frame, -6)), 2);
        let folded = fold_memory_operand(&Arc::new(test), 1, &cell, 16, &RegisterClasses::m16()).expect("folds");
        let what = folded.what.as_ref().unwrap();
        assert_eq!(what.name.as_deref(), Some("cmp"), "{folded:?}");
        assert!(
            matches!(what.sources.as_slice(), [Loc::Mem(_), Loc::Imm(imm)] if imm.value == 0 && imm.width == 2),
            "{folded:?}"
        );
        assert!(folded.uses.is_empty(), "the folded compare still reads v1: {folded:?}");
    }

    /// nbody -O3 m64: the probe placed every operand in an integer register, so
    /// no float arithmetic priced and a spilled double was reloaded with a
    /// `movsd` of its own (146 instructions a trip against the pin's 132 on the
    /// same IR); `mulsd xmm, [cell]` is the pin's fold.
    #[test]
    fn test_a_spilled_double_folds_into_the_scalar_float_arithmetic_that_reads_it() {
        let held = |value: u32| Loc::Held(Held { value, width: 8, offset: 0 });
        let what = Semantics {
            name: Some("mulsd".into()),
            dests: vec![held(1)],
            sources: vec![held(1), held(2)],
            ..Semantics::new(Operation::Binary)
        };
        let mulsd = Arc::new(Insn::new(0, None, Some(what), vec![1], vec![1, 2]));
        let cell = Mem { through: RegId::RBP, ..Mem::new(Some(Addr::new(Space::Frame, -8)), 8) };
        let classes = RegisterClasses::of(&llrm_x86_m64::M64);
        let folded = fold_memory_operand(&mulsd, 2, &cell, 64, &classes).expect("folds");
        assert!(matches!(folded.what.as_ref().unwrap().sources.as_slice(), [Loc::Held(_), Loc::Mem(_)]), "{folded:?}");
    }

    /// recchop c -O1 (m16) 184 B, grep c -O1 +24.7% clocks: eviction spilled a
    /// value that is the base of a memory operand as readily as one that
    /// only an ALU op reads, so every access through it reloaded it into
    /// bx/si/di first. An address component weighs more.
    #[test]
    fn test_an_address_component_weighs_more_in_the_spill_weight_at_m16_only() {
        let held = |value: u32| Held { value, width: 2, offset: 0 };
        let cell = Mem { base: Some(held(7)), ..Mem::new(Some(Addr::new(Space::Frame, 0)), 2) };
        let load = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(held(8))],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Move)
        };
        let insn = Insn::new(0, None, Some(load), vec![8], vec![7]);
        let m16 = X86InstrInfo {
            bits: 16,
            classes: &RegisterClasses::m16(),
            basic_semantics: false,
            address_reference_milliweight: 3000,
            fold_spill: None,
        };
        let flat = X86InstrInfo {
            bits: 32,
            classes: &RegisterClasses::of(&llrm_x86_m32::M32),
            basic_semantics: false,
            address_reference_milliweight: 3000,
            fold_spill: None,
        };
        assert!(m16.reference_weight(&insn, 7) > 1.0, "the base of the cell");
        assert_eq!(m16.reference_weight(&insn, 8), 1.0, "the loaded value");
        assert_eq!(flat.reference_weight(&insn, 7), 1.0, "flat code weighs the pin's way");
        let long = X86InstrInfo {
            bits: 64,
            classes: &RegisterClasses::of(&llrm_x86_m64::M64),
            basic_semantics: false,
            address_reference_milliweight: 3000,
            fold_spill: None,
        };
        assert_eq!(long.reference_weight(&insn, 7), 1.0, "long mode weighs the pin's way");
    }

    /// The m16 weight of an address component was a literal in this file; it
    /// is the target table's `address_reference_milliweight`. With the pin's
    /// 1000 every reference weighs alike, whatever a literal says.
    #[test]
    fn test_an_address_component_weighs_the_tables_milliweight() {
        let held = |value: u32| Held { value, width: 2, offset: 0 };
        let cell = Mem { base: Some(held(7)), ..Mem::new(Some(Addr::new(Space::Frame, 0)), 2) };
        let load = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(held(8))],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Move)
        };
        let insn = Insn::new(0, None, Some(load), vec![8], vec![7]);
        let weight = |milli| {
            X86InstrInfo {
                bits: 16,
                classes: &RegisterClasses::m16(),
                basic_semantics: false,
                address_reference_milliweight: milli,
                fold_spill: None,
            }
            .reference_weight(&insn, 7)
        };
        assert_eq!((weight(1000), weight(3000), weight(4500)), (1.0, 3.0, 4.5));
    }

    fn x87_body(insns: Vec<Arc<Insn>>) -> crate::model::lir::LirBody {
        crate::model::lir::LirBody::new(
            "x87",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(0, insns))],
            Default::default(),
            Default::default(),
        )
    }

    fn fld(
        at: i64,
        value: u32,
        cell: Mem,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("fld".into()),
            dests: vec![Loc::Held(Held { value, width: 10, offset: 0 })],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::FloatLoad)
        };
        Arc::new(Insn::new(at, None, Some(what), vec![value], vec![]))
    }

    fn fstp(
        at: i64,
        value: u32,
        cell: Mem,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("fstp".into()),
            dests: vec![Loc::Mem(cell)],
            sources: vec![Loc::Held(Held { value, width: 10, offset: 0 })],
            ..Semantics::new(Operation::FloatStore)
        };
        Arc::new(Insn::new(at, None, Some(what), vec![], vec![value]))
    }

    fn global(
        symbol: i64,
        disp: i64,
    ) -> Mem {
        Mem::new(Some(Addr { index: symbol, ..Addr::new(Space::External, disp) }), 4)
    }

    fn frame(disp: i64) -> Mem {
        Mem::new(Some(Addr::new(Space::Frame, disp)), 4)
    }

    /// nbody_single -O3 m32 stored 9 loaded globals to the frame and loaded
    /// them again (`fld _vel_y+8; fstp [esp+244]` ... `fld [esp+244]`); gcc
    /// reads the global again. A global nothing writes while the value
    /// lives is a load the allocator makes again.
    #[test]
    fn test_an_x87_load_of_a_global_nothing_writes_is_made_again() {
        let tii = X86InstrInfo {
            bits: 32,
            classes: &RegisterClasses::m16(),
            basic_semantics: false,
            address_reference_milliweight: 3000,
            fold_spill: None,
        };
        let body = x87_body(vec![
            fld(0, 1, global(0, 0)),
            fld(1, 2, global(1, 0)),
            fstp(2, 2, frame(-8)),
            fstp(3, 1, frame(-4)),
        ]);
        assert_eq!(tii.argument_loads(&body), BTreeSet::from([1, 2]));
    }

    /// A store to the cell between the load and the last reader changes the
    /// value: the load stays.
    #[test]
    fn test_an_x87_load_of_a_global_written_while_it_lives_is_kept() {
        let tii = X86InstrInfo {
            bits: 32,
            classes: &RegisterClasses::m16(),
            basic_semantics: false,
            address_reference_milliweight: 3000,
            fold_spill: None,
        };
        let body = x87_body(vec![
            fld(0, 1, global(0, 0)),
            fld(1, 2, global(1, 0)),
            fstp(2, 2, global(0, 0)),
            fstp(3, 1, frame(-4)),
        ]);
        assert_eq!(tii.argument_loads(&body), BTreeSet::from([2]));
    }

    /// A BASIC module observes fault order: reading the cell later would move
    /// where a signalling NaN raises. Without the flag the same body re-reads.
    #[test]
    fn test_a_basic_module_keeps_its_x87_loads() {
        let body = x87_body(vec![fld(0, 1, global(0, 0)), fstp(2, 1, frame(-4))]);
        let plain = X86InstrInfo {
            bits: 16,
            classes: &RegisterClasses::m16(),
            basic_semantics: false,
            address_reference_milliweight: 3000,
            fold_spill: None,
        };
        let basic = X86InstrInfo { basic_semantics: true, ..plain };
        assert_eq!(plain.argument_loads(&body), BTreeSet::from([1]));
        assert!(basic.argument_loads(&body).is_empty());
    }
}
