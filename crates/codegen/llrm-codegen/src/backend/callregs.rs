//! Which registers a runtime call destroys or keeps, read from its contract, so
//! the allocator knows what may stay live across it.
//!
//! # How it works
//! A runtime routine's contract names the registers it disturbs. Each
//! allocatable register is matched by its 16- and 32-bit spellings:
//! `call_clobbers` is the registers the contract disturbs plus the selectors
//! it cannot be trusted with, and `call_keeps` is the general registers it
//! does not disturb.
//!
//! Selectors are the exception. A contract describes the 8086 and names no
//! FS or GS, so where it disturbs every register, or was written for the
//! 386 (`contract.i386`), the selectors it does not name are clobbered
//! too (`unnamed_selectors_clobbered`). For an i386 contract
//! `call_clobbered_high` is the registers the call keeps only the 16-bit half
//! of: every general register it does not clobber whole.
//! `call_clobbered_high_keeping` removes those the callee's convention saves
//! in full.
//!
//! # Example
//! No honest example for the contract tables: they are data in the runtime
//! description, and no unit test prints a set. The result is in the dump. In a
//! nib `main`, the first runtime call is `N$BGRW` (the dump's call at 0x0001,
//! the first `call far ptr` of the listing) and a bounds-failure call is
//! `N$EBND` (the dump's call in block 0x000f, label `L1_15` of the listing):
//!
//! ```lir
//! call   requires v1@AX,v2@DX,v3@CX; delivers v4@EAX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
//! call   clobbers EAX,ECX,EDX,EBX,ES,FS,GS
//! ```
//!
//! `FS` and `GS` are in the set though no 8086 contract names them.
//!
//! From `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions
//! -fno-inline-functions-called-once -fno-ipa-cp-clone
//! -march=i386` of `examples/pass-docs/calleefacts.nib`, stage
//! `FarIndirectCalls`.
//!
//! # Used by
//! `isel.rs`, `isel/wide.rs`, `isel/unwind.rs` (the clobber sets of a call
//! they select) and `abi/qb.rs` (how many registers a call keeps).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_lir::registers::RegId;

use super::target::Segments;
use crate::abi::runtime;
use crate::model::ir;
use crate::support::hash::{HashMap, IndexMap};

/// The registers a call under `contract` destroys.
pub fn call_clobbers(
    contract: &runtime::Contract,
    segments: &Segments,
    general: &[RegId],
) -> BTreeSet<RegId> {
    let names = _names(general);
    let mut out = unnamed_selectors_clobbered(contract, segments, general);
    out.extend(_named_clobbers(&names, &runtime::disturbs(contract)));
    out
}

/// The selectors a call under `contract` may change though the contract does
/// not name them. A contract is about the 8086 and names no FS or GS; one
/// reaching user code, or written for the 386, runs code that may use them.
pub fn unnamed_selectors_clobbered(
    contract: &runtime::Contract,
    segments: &Segments,
    general: &[RegId],
) -> BTreeSet<RegId> {
    let names = _names(general);
    if runtime::disturbs(contract) == *runtime::EVERY || contract.i386 {
        segments.selectors.iter().copied().filter(|register| !names.contains_key(register)).collect()
    } else {
        BTreeSet::new()
    }
}

/// The registers a value may be placed in that a call under `contract` keeps.
pub fn call_keeps(
    contract: &runtime::Contract,
    general: &[RegId],
) -> Vec<RegId> {
    let names = _names(general);
    let clobbered = _named_clobbers(&names, &runtime::disturbs(contract));
    general.iter().copied().filter(|register| !clobbered.contains(register)).collect()
}

fn _named_clobbers(
    names: &IndexMap<RegId, BTreeSet<String>>,
    disturbed: &BTreeSet<runtime::Reg>,
) -> BTreeSet<RegId> {
    let wanted: Vec<String> = disturbed
        .iter()
        .map(|named| {
            #[cfg(test)]
            LOWERED.with(|one| one.set(one.get() + 1));
            named.value().to_lowercase()
        })
        .collect();
    names
        .iter()
        .filter(|(_, spelled)| wanted.iter().any(|name| spelled.contains(name)))
        .map(|(register, _)| *register)
        .collect()
}

/// The registers a call under `contract` keeps only the 16-bit half of, less
/// `whole`: those the callee's convention saves in full (`Target::callee_saved`
/// names the same register to keep and to push).
pub fn call_clobbered_high_keeping(
    contract: &runtime::Contract,
    segments: &Segments,
    general: &[RegId],
    whole: &BTreeSet<RegId>,
) -> BTreeSet<RegId> {
    call_clobbered_high(contract, segments, general)
        .into_iter()
        .filter(|register| !whole.contains(&ir::root(*register)))
        .collect()
}

/// The registers a call under `contract` keeps only the 16-bit half of.
pub fn call_clobbered_high(
    contract: &runtime::Contract,
    segments: &Segments,
    general: &[RegId],
) -> BTreeSet<RegId> {
    if !contract.i386 {
        return BTreeSet::new();
    }
    let whole: BTreeSet<RegId> = call_clobbers(contract, segments, general).into_iter().map(ir::root).collect();
    general.iter().copied().filter(|register| !whole.contains(&ir::root(*register))).collect()
}

type Names = IndexMap<RegId, BTreeSet<String>>;

thread_local! {
    /// `_names` by the `general` set it was built for: every call of a body
    /// asks for the same one.
    static NAMES: RefCell<HashMap<Vec<RegId>, Rc<Names>>> = RefCell::default();
    #[cfg(test)]
    static LOWERED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    #[cfg(test)]
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Each allocatable register by the names runtime.py's own Reg enum uses,
/// built once per `general` set.
fn _names(general: &[RegId]) -> Rc<Names> {
    if let Some(found) = NAMES.with(|cache| cache.borrow().get(general).cloned()) {
        return found;
    }
    #[cfg(test)]
    BUILT.with(|one| one.set(one.get() + 1));
    let built: Rc<Names> = Rc::new(
        general
            .iter()
            .copied()
            .chain([RegId::ES, ir::STACK])
            .map(|register| {
                (
                    register,
                    BTreeSet::from([
                        format!("{:?}", llrm_x86::registers::word_of(register).unwrap_or(register)).to_lowercase(),
                        format!("{register:?}").to_lowercase(),
                    ]),
                )
            })
            .collect(),
    );
    NAMES.with(|cache| cache.borrow_mut().insert(general.to_vec(), Rc::clone(&built)));
    built
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every call of a body asked for the table of register names and built
    /// it again: three string allocations a register a call. One per set.
    #[test]
    fn test_the_register_names_are_built_once_per_set() {
        let (small, large) = ([RegId::AX, RegId::BX], [RegId::AX, RegId::BX, RegId::CX]);
        let before = BUILT.with(std::cell::Cell::get);
        for _ in 0..5 {
            _names(&small);
            _names(&large);
        }
        assert_eq!(BUILT.with(std::cell::Cell::get) - before, 2);
    }

    /// `_named_clobbers` lowercased each disturbed register's name once per
    /// register of the table, a string allocation each (3.3 M Ir on d_faces):
    /// once per disturbed register.
    #[test]
    fn test_a_disturbed_name_is_lowercased_once_not_once_per_register() {
        let general = [RegId::AX, RegId::BX, RegId::CX, RegId::DX];
        let names = _names(&general);
        let disturbed = BTreeSet::from([runtime::Reg::Ax, runtime::Reg::Bx]);
        let before = LOWERED.with(std::cell::Cell::get);
        let clobbered = _named_clobbers(&names, &disturbed);
        assert_eq!(LOWERED.with(std::cell::Cell::get) - before, 2);
        assert_eq!(clobbered, BTreeSet::from([RegId::AX, RegId::BX]));
    }
}
