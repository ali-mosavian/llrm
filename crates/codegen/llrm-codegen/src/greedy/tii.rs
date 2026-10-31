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

    /// `foldMemoryOperand`.
    fn fold_memory_operand(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
    ) -> Option<Arc<Insn>>;

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
            dests: vec![Loc::Held(Held { value: to, width })],
            sources: vec![Loc::Held(Held { value: from, width })],
            ..Semantics::new(Operation::Move)
        }
    }

    fn remat_shape(
        &self,
        one: &Insn,
    ) -> bool {
        super::inlinespiller::remat_shape(one)
    }

    fn is_trivially_re_materializable(
        &self,
        def: &Insn,
        invariant: bool,
    ) -> bool {
        super::remat::is_trivially_re_materializable(def, invariant)
    }

    fn argument_loads(
        &self,
        body: &crate::model::lir::LirBody,
    ) -> BTreeSet<u32> {
        super::remat::argument_loads(body)
    }

    fn fold_memory_operand(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
    ) -> Option<Arc<Insn>> {
        fold_memory_operand(one, reg, cell, self.bits, self.classes)
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
    let what = one.what.as_ref()?;
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
    let swap = |loc: &Loc| match loc {
        Loc::Held(held) if held.value == reg => {
            Loc::Mem(Mem { width: (held.width as u32).min(cell.width), ..cell.clone() })
        }
        _ => loc.clone(),
    };
    let narrow_imm = |loc: &Loc| match loc {
        Loc::Imm(imm) if narrow => Loc::Imm(Imm { width: imm.width.min(cell.width), ..imm.clone() }),
        _ => loc.clone(),
    };
    let made = Semantics {
        dests: what.dests.iter().map(swap).collect(),
        sources: what.sources.iter().map(swap).map(|loc| narrow_imm(&loc)).collect(),
        ..what.clone()
    };
    if !is_encodable(bits, &made, classes).ok()? {
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
        _mov(Loc::Held(Held { value: into, width: cell.width }), Loc::Mem(cell.clone())),
        vec![into],
        Vec::new(),
    );
    _with(&inserted, |made| made.spill_reload = true)
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
        _mov(Loc::Mem(cell.clone()), Loc::Held(Held { value: out_of, width: cell.width })),
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
) -> Result<bool, Error> {
    let regs = classes.registers;
    let mut taken: IndexMap<u32, RegId> = IndexMap::default();
    let rows: IndexMap<u32, Vec<RegId>> = [1_u32, 2, 4]
        .into_iter()
        .map(|width| {
            (
                width,
                classes
                    .available
                    .iter()
                    .copied()
                    .filter(|one| {
                        regs.width_of(regs.named(*one, i64::from(width)))
                            .filter(|_| regs.integer(regs.named(*one, i64::from(width))))
                            == Some(i64::from(width))
                    })
                    .collect(),
            )
        })
        .collect();

    let mut placed = |operand: &Loc| -> Loc {
        let Loc::Held(held) = operand else {
            return operand.clone();
        };
        if !taken.contains_key(&held.value) {
            let row = rows.get(&held.width).cloned().unwrap_or_default();
            if taken.len() >= row.len() {
                // refuses below, which is the safe answer
                return operand.clone();
            }
            taken.insert(held.value, row[taken.len()]);
        }
        Loc::Reg(Reg { register: regs.named(taken[&held.value], i64::from(held.width)), width: held.width })
    };

    let probe = Semantics {
        dests: what.dests.iter().map(&mut placed).collect(),
        sources: what.sources.iter().map(&mut placed).collect(),
        ..what.clone()
    };
    Ok(crate::backend::select::priced_in(bits, &probe, 0, None, false, false, None).is_some())
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
        let held = |value: u32| Loc::Held(Held { value, width: 2 });
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
}
