//! The registers a runtime call destroys or keeps, read from its contract.

use std::collections::BTreeSet;

use iced_x86::Register;

use super::target::{self, Segments};
use crate::abi::runtime;
use crate::model::ir;
use crate::support::hash::IndexMap;

/// The registers a call under `contract` destroys.
pub fn call_clobbers(contract: &runtime::Contract, segments: &Segments) -> BTreeSet<Register> {
    let names = _names();
    let mut out = unnamed_selectors_clobbered(contract, segments);
    out.extend(_named_clobbers(&names, &runtime::disturbs(contract)));
    out
}

/// The selectors a call under `contract` may change though the contract does not name them. A contract is about the 8086 and
/// names no FS or GS; one reaching user code, or written for the 386, runs code that may use them.
pub fn unnamed_selectors_clobbered(contract: &runtime::Contract, segments: &Segments) -> BTreeSet<Register> {
    let names = _names();
    if runtime::disturbs(contract) == *runtime::EVERY || contract.i386 {
        segments.selectors.iter().copied().filter(|register| !names.contains_key(register)).collect()
    } else {
        BTreeSet::new()
    }
}

/// The registers a value may be placed in that a call under `contract` keeps.
pub fn call_keeps(contract: &runtime::Contract) -> Vec<Register> {
    let names = _names();
    let clobbered = _named_clobbers(&names, &runtime::disturbs(contract));
    llrm_x86_m16::GENERAL.into_iter().filter(|register| !clobbered.contains(register)).collect()
}

fn _named_clobbers(names: &IndexMap<Register, BTreeSet<String>>, disturbed: &BTreeSet<runtime::Reg>) -> BTreeSet<Register> {
    names.iter().filter(|(_, spelled)| disturbed.iter().any(|named| spelled.contains(&named.value().to_lowercase()))).map(|(register, _)| *register).collect()
}

/// The registers a call under `contract` keeps only the 16-bit half of, less `whole`: those the callee's
/// convention saves in full (`Target::callee_saved` names the same register to keep and to push).
pub fn call_clobbered_high_keeping(contract: &runtime::Contract, segments: &Segments, whole: &BTreeSet<Register>) -> BTreeSet<Register> {
    call_clobbered_high(contract, segments).into_iter().filter(|register| !whole.contains(&ir::root(*register))).collect()
}

/// The registers a call under `contract` keeps only the 16-bit half of.
pub fn call_clobbered_high(contract: &runtime::Contract, segments: &Segments) -> BTreeSet<Register> {
    if !contract.i386 {
        return BTreeSet::new();
    }
    let whole: BTreeSet<Register> = call_clobbers(contract, segments).into_iter().map(ir::root).collect();
    llrm_x86_m16::GENERAL.into_iter().filter(|register| !whole.contains(&ir::root(*register))).collect()
}

/// Each allocatable register by the names runtime.py's own Reg enum uses.
fn _names() -> IndexMap<Register, BTreeSet<String>> {
    llrm_x86_m16::GENERAL
        .into_iter()
        .chain([Register::ES])
        .map(|register| {
            (
                register,
                BTreeSet::from([
                    target::name_of(target::named(register, 2)),
                    target::name_of(target::named(register, 4)),
                ]),
            )
        })
        .collect()
}
