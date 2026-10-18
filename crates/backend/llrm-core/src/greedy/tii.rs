//! `TargetInstrInfo::foldMemoryOperand` for the forms LIR selects: the spill
//! slot put in place of a register an instruction names, where x86 has that
//! form. Translated from llvm/lib/CodeGen/TargetInstrInfo.cpp
//! (`foldMemoryOperand`) and llvm/lib/Target/X86/X86InstrInfo.cpp
//! (`foldMemoryOperandImpl`) at d1106deb71cc.
//!
//! DEVIATE 1: X86's fold tables are not translated; a form exists when the
//! selector prices it (`spiller::_encodable`), the one place that knows the
//! m16 and m32 encodings.

use std::sync::Arc;

use crate::backend::classes::RegisterClasses;
use crate::model::ir::{Loc, Mem, Semantics};
use crate::model::lir::Insn;

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
        if matches!(side.get(place.index), Some(Loc::Held(held)) if held.value == reg) {
            return None;
        }
    }
    let direct = |loc: &Loc| matches!(loc, Loc::Held(held) if held.value == reg);
    for loc in what.dests.iter().chain(&what.sources) {
        if !direct(loc) && (matches!(loc, Loc::Mem(_)) || crate::model::ir::values(loc).iter().any(|held| held.value == reg)) {
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
    let swap = |loc: &Loc| match loc {
        Loc::Held(held) if held.value == reg => Loc::Mem(Mem { width: held.width, ..cell.clone() }),
        _ => loc.clone(),
    };
    let made =
        Semantics { dests: what.dests.iter().map(swap).collect(), sources: what.sources.iter().map(swap).collect(), ..what.clone() };
    if !crate::backend::spiller::_encodable(bits, &made, classes).ok()? {
        return None;
    }
    let mut out = (**one).clone();
    out.what = Some(made);
    out.defines.retain(|value| *value != reg);
    out.uses.retain(|value| *value != reg);
    Some(Arc::new(out))
}
