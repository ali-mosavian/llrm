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
//! (`crate::backend::bodyfacts::_stable_loads_by`), so the load is passed in as
//! a fact.
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
        // `LEA32r` of registers: the
        // generic `isReallyTriviallyReMaterializable` takes a virtual-register
        // use; `allUsesAvailableAt` asks at the use whether the registers still
        // hold.
        (Operation::Address, Some("lea"), [Loc::Mem(cell)]) if !def.uses.is_empty() => {
            into.width == 4 && cell.width == 4 && def.spread.is_empty() && !def.volatile
        }
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
        // `LD_Fp064` / `LD_Fp164`
        // (X86InstrFPStack.td#L526): `isReMaterializable`.
        (Operation::FloatLoad, Some("fld1" | "fldz"), []) => def.uses.is_empty() && !def.volatile,
        _ => false,
    }
}

/// The values one definition of which is a load the instruction itself calls
/// invariant (`Insn::invariant`, from `!invariant.load` or a read of constant
/// memory): the pin's `isLoadFromInvariantOrConstantMemory` on the load's
/// memory operand. This is the exit of D-I; `values` still asks the shape of
/// the load.
pub fn flagged_loads(body: &LirBody) -> BTreeSet<u32> {
    let mut definitions: IndexMap<u32, usize> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for value in &one.defines {
            *definitions.entry(*value).or_default() += 1;
        }
    }
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.invariant())
        .filter_map(|one| match one.defines.as_slice() {
            [value] if definitions.get(value) == Some(&1) => Some(*value),
            _ => None,
        })
        .collect()
}

/// The loads of an incoming stack argument that nothing in `body` writes or
/// takes the address of. X86 lowers a stack argument to an immutable fixed
/// stack object, and `isLoadFromInvariantOrConstantMemory` takes a load of one
/// (`MachineFrameInfo::isImmutableObjectIndex`): the pin's own rule, which
/// needs no per-live-range proof. Incoming arguments are the frame cells above
/// the frame pointer. An x87 `fld` of one is such a load too.
pub fn argument_loads(body: &LirBody) -> BTreeSet<u32> {
    let incoming = |cell: &crate::model::ir::Mem| {
        cell.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0)
            && cell.base.is_none()
            && cell.index.is_none()
    };
    let mut written: Vec<(i64, i64)> = Vec::new();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        let Some(what) = one.what.as_ref() else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            let span = |addr: crate::model::ir::Addr, width: u32| (addr.disp, addr.disp + i64::from(width));
            match place {
                // Its address is taken: any callee may write it.
                Loc::Address(a) if a.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0) => {
                    return BTreeSet::new();
                }
                Loc::Imm(imm) if imm.address.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0) => {
                    return BTreeSet::new();
                }
                _ => {}
            }
            if what.op == Operation::Address {
                if let Loc::Mem(cell) = place {
                    if cell.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0) {
                        return BTreeSet::new();
                    }
                }
            }
            if let Loc::Mem(cell) = place {
                let is_dest = what.dests.iter().any(|d| matches!(d, Loc::Mem(c) if c == cell));
                if is_dest && cell.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0) {
                    written.push(span(cell.addr.expect("an address"), cell.width));
                }
            }
        }
    }
    let mut definitions: IndexMap<u32, usize> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for value in &one.defines {
            *definitions.entry(*value).or_default() += 1;
        }
    }
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter_map(|one| {
            let what = one.what.as_ref()?;
            let ([Loc::Held(into)], [Loc::Mem(cell)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
                return None;
            };
            let addr = cell.addr?;
            let (from, to) = (addr.disp, addr.disp + i64::from(cell.width));
            let integer = what.op == Operation::Move && what.name.as_deref() == Some("mov") && into.width == cell.width;
            let float = what.op == Operation::FloatLoad && what.name.as_deref() == Some("fld");
            ((integer || float)
                && incoming(cell)
                && one.defines == [into.value]
                && definitions.get(&into.value) == Some(&1)
                && !written.iter().any(|(start, end)| *start < to && from < *end))
            .then_some(into.value)
        })
        .collect()
}

/// The values of `body` with one definition that
/// `is_trivially_re_materializable` takes. `invariant` names the loads that
/// hold; a value read wider than its definition writes, or inside a parallel
/// copy that cannot take it in place, is left out (DEVIATE R1).
pub fn values(
    body: &LirBody,
    invariant: &BTreeSet<u32>,
    tii: &dyn super::tii::TargetInstrInfo,
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
            (alike && narrow && tii.is_trivially_re_materializable(first, invariant.contains(value))).then_some(*value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::classes::RegisterClasses;
    use crate::model::ir::{Held, Imm, Mem, Semantics};

    fn def(
        sources: Vec<Loc>,
        op: Operation,
        name: &str,
    ) -> Insn {
        let what = Semantics {
            name: Some(name.into()),
            dests: vec![Loc::Held(Held { value: 1, width: 4 })],
            sources,
            ..Semantics::new(op)
        };
        Insn::new(0, None, Some(what), vec![1], vec![])
    }

    /// A literal move is made again; a load only when the cell holds. Without
    /// the second rule a load of a cell a store changes between two reads was
    /// made again and read the new value.
    #[test]
    fn test_a_load_is_made_again_only_when_the_cell_holds() {
        let literal = def(vec![Loc::Imm(Imm { value: 7, width: 4, address: None })], Operation::Move, "mov");
        assert!(is_trivially_re_materializable(&literal, false));
        let cell = Mem::new(Some(crate::model::ir::Addr::new(Space::Segment, 0)), 4);
        let load = def(vec![Loc::Mem(cell)], Operation::Move, "mov");
        assert!(!is_trivially_re_materializable(&load, false), "a load nothing proves invariant stays");
        assert!(is_trivially_re_materializable(&load, true));
    }

    /// nbody_single -Os: a hoisted `fld1` with the x87 stack full was stored
    /// to a frame cell and reloaded in the loop (+18 B); the pin remakes
    /// `LD_Fp164` where it is read.
    #[test]
    fn test_an_x87_constant_load_is_made_again() {
        let mut one = def(vec![], Operation::FloatLoad, "fld1");
        assert!(is_trivially_re_materializable(&one, false));
        one.what = Some(Semantics { name: Some("fldz".into()), ..one.what.clone().unwrap() });
        assert!(is_trivially_re_materializable(&one, false));
    }

    /// queens `_place`: `lea t, [row+1]` was spilled and reloaded across the
    /// call, because only a `lea` that reads no register was made again; the
    /// pin makes it again where `row` still holds (`allUsesAvailableAt`).
    #[test]
    fn test_a_lea_of_a_register_is_made_again() {
        let cell = Mem { base: Some(Held { value: 2, width: 4 }), offset: 1, ..Mem::new(None, 4) };
        let what = Semantics {
            name: Some("lea".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 4 })],
            sources: vec![Loc::Mem(cell)],
            ..Semantics::new(Operation::Address)
        };
        let lea = Insn::new(0, None, Some(what), vec![1], vec![2]);
        assert!(is_trivially_re_materializable(&lea, false));
    }

    /// `values` leaves out a literal read wider than the move that makes it
    /// (DEVIATE R1) and takes the one read at its width.
    #[test]
    fn test_a_literal_read_wider_than_its_move_is_not_made_again() {
        let literal = Arc::new(def(vec![Loc::Imm(Imm { value: 7, width: 4, address: None })], Operation::Move, "mov"));
        let narrow = Arc::new(Insn::new(1, None, None, vec![], vec![1]));
        let body = LirBody::new(
            "r",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(
                0,
                vec![Arc::clone(&literal), narrow],
            ))],
            Default::default(),
            Default::default(),
        );
        assert_eq!(
            values(
                &body,
                &BTreeSet::new(),
                &super::super::tii::X86InstrInfo { bits: 16, classes: &RegisterClasses::m16() }
            ),
            BTreeSet::from([1])
        );
    }

    fn one_block(insns: Vec<Arc<Insn>>) -> LirBody {
        LirBody::new(
            "args",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(0, insns))],
            Default::default(),
            Default::default(),
        )
    }

    fn move_of(
        at: i64,
        dest: Loc,
        source: Loc,
        defines: Vec<u32>,
    ) -> Arc<Insn> {
        let what = crate::model::ir::Semantics {
            name: Some("mov".into()),
            dests: vec![dest],
            sources: vec![source],
            ..crate::model::ir::Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(at, None, Some(what), defines, vec![]))
    }

    fn argument(disp: i64) -> Loc {
        Loc::Mem(Mem::new(Some(crate::model::ir::Addr::new(Space::Frame, disp)), 2))
    }

    fn word(value: u32) -> Loc {
        Loc::Held(Held { value, width: 2 })
    }

    /// The pin takes a load of an incoming stack argument as invariant (an
    /// immutable fixed stack object): hanoi's three parameters were stored to
    /// slots and read back (`mov [bp-6], cx`), not read again from `[bp+6]`
    /// (m16 BASIC memory operands +15.8% with the per-range proof off).
    #[test]
    fn test_a_load_of_an_unwritten_incoming_argument_is_invariant() {
        let body =
            one_block(vec![move_of(0, word(1), argument(6), vec![1]), move_of(1, word(2), argument(8), vec![2])]);
        assert_eq!(argument_loads(&body), BTreeSet::from([1, 2]));
    }

    /// A parameter the function assigns, or whose address it takes, is not
    /// immutable: the loads of it stay.
    #[test]
    fn test_a_written_or_address_taken_argument_is_not_invariant() {
        let body = one_block(vec![
            move_of(0, word(1), argument(6), vec![1]),
            move_of(1, word(2), argument(8), vec![2]),
            move_of(2, argument(8), word(3), vec![]),
        ]);
        assert_eq!(argument_loads(&body), BTreeSet::from([1]), "only the cell nobody writes");
        let lea = {
            let what = crate::model::ir::Semantics {
                name: Some("lea".into()),
                dests: vec![word(4)],
                sources: vec![argument(6)],
                ..crate::model::ir::Semantics::new(Operation::Address)
            };
            Arc::new(Insn::new(3, None, Some(what), vec![4], vec![]))
        };
        let taken = one_block(vec![move_of(0, word(1), argument(6), vec![1]), lea]);
        assert!(argument_loads(&taken).is_empty(), "an address taken: any call may write");
    }

    /// A load the instruction calls invariant is rematerialised and the same
    /// load without the flag is not: the consumer of `!invariant.load` that the
    /// spiller ignored (it took only the per-range proof and stack arguments).
    #[test]
    fn test_a_load_flagged_invariant_is_rematerialised_and_an_unflagged_one_is_not() {
        let cell = || Loc::Mem(Mem::new(Some(crate::model::ir::Addr::new(Space::Segment, 0)), 2));
        let flagged = |at: i64, value: u32| {
            let one = move_of(at, word(value), cell(), vec![value]);
            Arc::new(Insn { invariant: true, ..(*one).clone() })
        };
        let body = one_block(vec![flagged(0, 1), move_of(1, word(2), cell(), vec![2])]);
        assert_eq!(flagged_loads(&body), BTreeSet::from([1]));
        assert_eq!(
            values(
                &body,
                &flagged_loads(&body),
                &super::super::tii::X86InstrInfo { bits: 16, classes: &RegisterClasses::m16() }
            ),
            BTreeSet::from([1])
        );
        assert!(
            values(
                &body,
                &BTreeSet::new(),
                &super::super::tii::X86InstrInfo { bits: 16, classes: &RegisterClasses::m16() }
            )
            .is_empty(),
            "the flag is what takes the load"
        );
    }
}
