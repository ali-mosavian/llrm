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

use crate::backend::allocate::Error;
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::Frame;
use crate::model::ir::{self, Held, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::Insn;
use crate::support::hash::IndexMap;

/// `frame.WORD` at the width type this module uses.
const WORD: u32 = crate::backend::frame::WORD as u32;

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
    // A value wider than its cell (C-LANE keeps a value read as a word in a
    // word cell) writes or reads only the cell: a move stores its source's
    // low part, any other form keeps the value in a register.
    let wider = what
        .dests
        .iter()
        .chain(&what.sources)
        .any(
            |loc| matches!(
                loc,
                Loc::Held(held) if held.value == reg && held.width > cell.width
            ),
        );
    if wider && !(dests == 1 && sources == 0 && what.op == Operation::Move && what.sources.len() == 1) {
        return None;
    }
    let swap = |loc: &Loc| match loc {
        Loc::Held(held) if held.value == reg => Loc::Mem(Mem { width: held.width.min(cell.width), ..cell.clone() }),
        _ => loc.clone(),
    };
    let narrow = |loc: &Loc| match loc {
        Loc::Held(held) if wider && held.width > cell.width => Loc::Held(Held { width: cell.width, ..*held }),
        _ => loc.clone(),
    };
    let made = Semantics {
        dests: what.dests.iter().map(swap).collect(),
        sources: what.sources.iter().map(|loc| if wider { narrow(loc) } else { swap(loc) }).collect(),
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
    frame: &mut Frame,
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
        })));
    }
    let value = if into.is_empty() { outof[0] } else { into[0] };
    let cell = frame.cell(home(value), _width(one, value))?;
    if !into.is_empty() {
        return Ok(Some(_with(one, |made| {
            made.what = Some(Semantics { dests: vec![Loc::Mem(cell)], ..what.clone() });
            made.defines = one.defines.iter().copied().filter(|v| *v != value).collect();
        })));
    }
    Ok(Some(_with(one, |made| {
        made.what = Some(Semantics { sources: vec![Loc::Mem(cell)], ..what.clone() });
        made.uses = one.uses.iter().copied().filter(|v| *v != value).collect();
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
