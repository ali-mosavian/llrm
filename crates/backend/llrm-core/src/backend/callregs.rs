//! Which registers a runtime call destroys or keeps, read from its contract.
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
//! # Used by
//! `isel.rs`, `isel/wide.rs`, `isel/unwind.rs` (the clobber sets of a call
//! they select) and `abi/qb.rs` (how many registers a call keeps).

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use super::target::Segments;
use crate::abi::runtime;
use crate::model::ir;
use crate::support::hash::IndexMap;

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
    names
        .iter()
        .filter(|(_, spelled)| disturbed.iter().any(|named| spelled.contains(&named.value().to_lowercase())))
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

/// Each allocatable register by the names runtime.py's own Reg enum uses.
fn _names(general: &[RegId]) -> IndexMap<RegId, BTreeSet<String>> {
    general
        .iter()
        .copied()
        .chain([RegId::ES])
        .map(|register| {
            (
                register,
                BTreeSet::from([
                    format!("{:?}", llrm_x86::registers::word_of(register).unwrap_or(register)).to_lowercase(),
                    format!("{register:?}").to_lowercase(),
                ]),
            )
        })
        .collect()
}
