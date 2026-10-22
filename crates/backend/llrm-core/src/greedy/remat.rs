//! `TargetInstrInfo::isTriviallyReMaterializable` over the definitions of a
//! body: the values whose one definition can be made again where it is read.
//! Translated from llvm/lib/CodeGen/TargetInstrInfo.cpp
//! (`isReallyTriviallyReMaterializable`) and
//! llvm/lib/Target/X86/X86InstrInfo.cpp (`isReallyTriviallyReMaterializable`)
//! at d1106deb71cc.
//!
//! # How it works
//! `is_trivially_re_materializable` answers for one definition, as the pin
//! does for one `MachineInstr`: a move of a literal, or an address of a frame
//! cell or a symbol (the `LEA` the pin takes when no register is in it), is;
//! a load is only when the cell holds. `values` asks it of every value with
//! one definition, and hands it the loads that hold.
//!
//! DEVIATE D-I (exit: an invariant-load flag on the instruction): the pin takes
//! a load when its memory operand is a dereferenceable invariant load
//! (`isLoadFromInvariantOrConstantMemory`); llrm proves it per live range
//! (`spiller::_stable_loads_by`), so the load is passed in as a fact.
//!
//! DEVIATE R1 (no number): a literal read wider than the move that makes it is
//! not made again, and a use inside a parallel copy that cannot take a literal
//! in place stops it. The fact: a value here is one width and a copy group's
//! member moves registers, not immediates.
//!
//! # Used by
//! `shadow`.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::model::ir::{Held, Loc, Operation, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// `isTriviallyReMaterializable` for the definition `def` of one value. A load
/// is taken only if it is `invariant`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetInstrInfo.cpp#L1673>
pub fn is_trivially_re_materializable(
    def: &Insn,
    invariant: bool,
) -> bool {
    let [value] = def.defines.as_slice() else { return false };
    let Some(what) = def.what.as_ref() else { return false };
    if def.group.is_some()
        || !def.requires.is_empty()
        || !def.delivers.is_empty()
        || !def.clobbers.is_empty()
        || def.symbol == Some(true)
    {
        return false;
    }
    let [Loc::Held(into)] = what.dests.as_slice() else { return false };
    if into.value != *value {
        return false;
    }
    match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        // `MOV32ri`: a literal and nothing else.
        (Operation::Move, Some("mov"), [Loc::Imm(imm)]) => def.uses.is_empty() && into.width == imm.width,
        // `LEA32r` of a frame index or a global, no register in it.
        (Operation::Address, Some("lea"), [source]) => {
            let (addr, through, index) = match source {
                Loc::Address(a) => (a.addr, a.through, a.index_through),
                Loc::Mem(cell) if cell.base.is_none() && cell.index.is_none() => (cell.addr, cell.through, RegId::None),
                _ => return false,
            };
            matches!(into.width, 2 | 4)
                && def.uses.is_empty()
                && index == RegId::None
                && addr.is_some_and(|addr| {
                    addr.space == Space::Frame && through == RegId::BP
                        || matches!(addr.space, Space::Segment | Space::External) && through == RegId::None
                })
        }
        // A load from a cell: `mayLoad` and invariant.
        (Operation::Move, Some("mov"), [Loc::Mem(cell)]) => {
            invariant
                && into.width == cell.width
                && cell.addr.is_some()
                && def.uses.is_empty()
                && def.spread.is_empty()
                && !def.volatile
        }
        (Operation::FloatLoad, Some("fld"), [Loc::Mem(_)]) => {
            invariant && def.uses.is_empty() && def.spread.is_empty() && !def.volatile
        }
        _ => false,
    }
}

/// The values of `body` with one definition that
/// `is_trivially_re_materializable` takes. `invariant` names the loads that
/// hold; a value read wider than its definition writes, or inside a parallel
/// copy that cannot take it in place, is left out (DEVIATE R1).
pub fn values(
    body: &LirBody,
    invariant: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    let mut definitions: IndexMap<u32, Vec<&Arc<Insn>>> = IndexMap::default();
    let mut widest: IndexMap<u32, u32> = IndexMap::default();
    let mut blocked: BTreeSet<u32> = BTreeSet::new();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in &one.defines {
            definitions.entry(*value).or_default().push(one);
        }
        for value in &one.uses {
            let had = widest.get(value).copied().unwrap_or(0);
            widest.insert(*value, had.max(crate::greedy::tii::_width(one, *value)));
        }
        if one.group.is_some() {
            blocked.extend(one.defines.iter().copied());
            if group_source(one).is_none() {
                blocked.extend(one.uses.iter().copied());
            }
        }
    }
    definitions
        .iter()
        .filter(|(value, _)| !blocked.contains(*value))
        .filter_map(|(value, defining)| {
            // A split remakes a value where each piece ends: definitions that
            // are one count as one.
            let first = defining.first()?;
            let alike = defining
                .iter()
                .all(
                    |one| one.what == first.what
                        && one.defines == first.defines
                        && one.uses == first.uses
                        && one.clobbers == first.clobbers
                        && one.requires == first.requires
                        && one.delivers == first.delivers
                        && one.group == first.group,
                );
            let literal = first.what.as_ref().is_some_and(|what| matches!(what.sources.as_slice(), [Loc::Imm(_)]));
            let narrow = !literal || widest.get(value).copied().unwrap_or(0) <= first_width(first);
            (alike && narrow && is_trivially_re_materializable(first, invariant.contains(value))).then_some(*value)
        })
        .collect()
}

/// How wide the definition writes its value.
fn first_width(def: &Insn) -> u32 {
    match def.what.as_ref().map(|what| what.dests.as_slice()) {
        Some([Loc::Held(into)]) => into.width,
        _ => 0,
    }
}

/// An unconstrained full-width parallel copy can take a literal in place.
pub(crate) fn group_source(one: &Insn) -> Option<Held> {
    if one.group.is_none() || !one.clobbers.is_empty() || !one.requires.is_empty() || !one.delivers.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
        match (what.dests.as_slice(), what.sources.as_slice()) {
            ([Loc::Held(dest)], [Loc::Held(source)])
                if dest.width == source.width && one.uses == [source.value] && one.defines == [dest.value] =>
            {
                return Some(*source);
            }
            // Into a spilled value's slot.
            ([Loc::Mem(dest)], [Loc::Held(source)])
                if dest.width == source.width && one.uses == [source.value] && one.defines.is_empty() =>
            {
                return Some(*source);
            }
            _ => {}
        }
    }
    None
}
