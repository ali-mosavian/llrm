//! Port of `qbopt/frontend/fppatches.py`: the FP emulator's own patch sites,
//! and the records without their fixups.

use std::collections::BTreeSet;

use iced_x86::Code;

use crate::frontends::bc::declen::decode;
use crate::objectfile::module::Module;
use crate::objectfile::omf::{self};

pub fn sites(module: &Module, starts: &BTreeSet<usize>) -> BTreeSet<usize> {
    let names = omf::externals(&module.records);
    let mut result: BTreeSet<usize> = BTreeSet::new();
    let overrides = |name: &str| match name {
        "FIERQQ" => Some(0x26u8),
        "FICRQQ" => Some(0x2E),
        "FISRQQ" => Some(0x36),
        "FIARQQ" => Some(0x3E),
        _ => None,
    };
    for (&at, fixup) in &module.fixup_at {
        let at = at as usize;
        if !starts.contains(&at)
            || fixup.loc != omf::LOC_OFF16
            || fixup.selfrel
            || fixup.target != "external"
            || fixup.disp != 0
            || !(0 < fixup.index && (fixup.index as usize) < names.len())
        {
            continue;
        }
        let name = names[fixup.index as usize].as_str();
        if name == "FIWRQQ" {
            if slice(&module.code, at, at + 2) == b"\x90\x9b" && starts.contains(&(at + 1)) {
                result.insert(at);
            }
            continue;
        }
        let mut opcode = at + 1;
        if let Some(prefix) = overrides(name) {
            if slice(&module.code, opcode, opcode + 1) != [prefix] {
                continue;
            }
            opcode += 1;
        } else if name != "FIDRQQ" {
            continue;
        }
        if slice(&module.code, at, at + 1) != b"\x9b"
            || !starts.contains(&(at + 1))
            || opcode as i64 >= module.end
            || !(0xD8..=0xDF).contains(&module.code[opcode])
        {
            continue;
        }
        let instruction = decode(&module.code, at + 1);
        if instruction.is_some_and(|instruction| instruction.code() != Code::INVALID) {
            result.insert(at);
        }
    }
    result
}

/// `b[lo:hi]` for non-negative bounds: clamped, and empty where `hi < lo`.
fn slice(b: &[u8], lo: usize, hi: usize) -> &[u8] {
    let hi = hi.min(b.len());
    if lo >= hi { &[] } else { &b[lo..hi] }
}
