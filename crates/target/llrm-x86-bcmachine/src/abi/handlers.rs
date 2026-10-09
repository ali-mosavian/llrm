//! Port of `qbopt/abi/handlers.py`: code entries passed to the runtime's
//! established far-address registrations.

use std::collections::{BTreeMap, BTreeSet};

use crate::objectfile::module::{self, Module, Space};

pub fn registered(
    found: &Module,
    routine: &str,
    families: &[&str],
) -> BTreeSet<i64> {
    registrations(found, routine, families).into_values().collect()
}

/// The code entry each call of `routine` registers, by the call's offset.
pub fn registrations(
    found: &Module,
    routine: &str,
    families: &[&str],
) -> BTreeMap<i64, i64> {
    if !families.contains(&module::family(&found.records).value())
        || module::defines(&found.records, found.seg).contains(routine)
    {
        return BTreeMap::new();
    }
    let code = |lo: i64, hi: i64| slice(&found.code, lo, hi);
    let mut entries = BTreeMap::new();
    for (&at, name) in &found.calls {
        if name != routine || at < 4 {
            continue;
        }
        let field;
        if code(0.max(at - 7), at) == b"\x0e\xb8\x00\x00\x68\x00\x00" {
            field = at - 2;
            if found.operands.get(&(at - 5)) != found.operands.get(&field) {
                continue;
            }
        } else if code(0.max(at - 7), at) == b"\x0e\x68\x00\x00\xb8\x00\x00" {
            // Constant propagation may materialize the stack offset before
            // the copy which establishes AX for the call. The stack is
            // already complete at that point and AX is established before
            // CALL, so this is the same far address ABI sequence. Require
            // both relocations to name the identical code entry: accepting a
            // pair of literal zeroes would instead invent a handler after
            // LINK has supplied unrelated offsets.
            field = at - 5;
            if found.operands.get(&(at - 2)) != found.operands.get(&field) {
                continue;
            }
        } else {
            match code(0.max(at - 5), at) {
                b"\xb8\x00\x00\x0e\x50" => field = at - 4,
                b"\x0e\xb8\x00\x00\x50" => field = at - 3,
                _ => {
                    if code(at - 4, at) != b"\x0e\x68\x00\x00" {
                        continue;
                    }
                    field = at - 2;
                }
            }
        }
        if let Some(entry) = code_entry(found, field) {
            entries.insert(at, entry);
        }
    }
    entries
}

/// The code offset the relocated word at `field` holds, if it is one.
fn code_entry(
    found: &Module,
    field: i64,
) -> Option<i64> {
    let reference = found.operands.get(&field)?;
    (reference.space == Space::Segment
        && reference.index == found.seg
        && found.start <= reference.disp
        && reference.disp < found.end)
        .then_some(reference.disp)
}

const ERROR_FAMILIES: [&str; 3] = ["qb45", "pds71", "vbdos"];

/// B$OEGA consumes parmD erradr; rt/error.asm installs it as OFD_ONERROR.
pub fn error_entries(found: &Module) -> BTreeSet<i64> {
    registered(found, "B$OEGA", &ERROR_FAMILIES)
}

/// The handler each ON ERROR GOTO registers, by its call of B$OEGA; ON
/// ERROR GOTO 0 registers none.
pub fn error_registrations(found: &Module) -> BTreeMap<i64, i64> {
    registrations(found, "B$OEGA", &ERROR_FAMILIES)
}

/// The label each RESUME label continues at, by its call of B$RESA, which
/// takes it in AX: `mov ax,label` just before the call.
pub fn resumptions(found: &Module) -> BTreeMap<i64, i64> {
    let mut labels = BTreeMap::new();
    for (&at, name) in &found.calls {
        if name == "B$RESA" && at >= 3 && found.code.get(at as usize - 3) == Some(&0xB8) {
            if let Some(label) = code_entry(found, at - 2) {
                labels.insert(at, label);
            }
        }
    }
    labels
}

/// `b[lo:hi]` for non-negative bounds: clamped, and empty where `hi < lo`.
fn slice(
    b: &[u8],
    lo: i64,
    hi: i64,
) -> &[u8] {
    let hi = (hi.max(0) as usize).min(b.len());
    let lo = lo.max(0) as usize;
    if lo >= hi { &[] } else { &b[lo..hi] }
}
