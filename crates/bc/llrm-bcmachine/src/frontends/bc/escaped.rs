//! The addresses a BC object hands out: pushed, or loaded and then pushed.

use std::collections::BTreeSet;

use iced_x86::{FlowControl, Mnemonic, OpKind, Register};

use crate::frontends::bc::declen::{self, Insn};
use crate::objectfile::module::{Module, defines};
use crate::objectfile::omf::{self, Fixup};
use crate::support::hash::IndexMap;

/// Every (segment, displacement) this object hands out the address of.
///
/// A relocated immediate inside a `push`, a register `mov` subsequently
/// pushed without being overwritten, or a `lea` is what handing one over
/// looks like. The object it names, not the byte: an escaped address poisons
/// the whole landmark object.
pub fn escaped(found: &Module) -> BTreeSet<(i64, i64)> {
    let fields: Vec<Fixup> =
        omf::fixups(&found.records).into_iter().filter(|one| one.seg == Some(found.seg)).collect();
    if fields.is_empty() {
        return BTreeSet::new();
    }
    let instructions = _instructions(found);
    let Some(instructions) = instructions else {
        return fields.iter().map(|one| (one.index, one.disp)).collect();
    };
    let mut out = BTreeSet::new();
    let values = _numeric_arguments(found, Some(&instructions));
    for (at, end, pushed) in _pushes(found, Some(&instructions)) {
        if values.contains(&at) || pushed.is_some_and(|pushed| values.contains(&pushed)) {
            continue;
        }
        for one in &fields {
            if at <= one.offset && one.offset < end {
                out.insert((one.index, one.disp));
            }
        }
    }
    out
}

/// The code map's instructions in address order, or None where there is no map.
///
/// Not a sweep from the segment's start: that decodes the module header as
/// code.
pub fn _instructions(found: &Module) -> Option<Vec<Insn>> {
    use crate::frontends::bc::blocks;

    let mapped = blocks::code_map(found).ok()?;
    Some(mapped.starts.iter().filter_map(|&at| declen::decode(&found.code, at)).collect())
}

/// Numeric argument pushes, including nested long-arithmetic call frames.
pub fn _numeric_arguments(found: &Module, instructions: Option<&[Insn]>) -> BTreeSet<i64> {
    use crate::abi::runtime;
    use crate::frontends::bc::{blocks, stack};

    let local = defines(&found.records, found.seg);
    let calls: IndexMap<i64, String> =
        found.calls.iter().filter(|(_, name)| !local.contains(*name)).map(|(&at, name)| (at, name.clone())).collect();
    let (mut pending, mut values, mut end): (Vec<&Insn>, BTreeSet<i64>, Option<i64>) =
        (Vec::new(), BTreeSet::new(), None);
    let owned;
    let instructions = match instructions {
        Some(instructions) => instructions,
        None => {
            owned = _instructions(found).unwrap_or_default();
            &owned
        }
    };
    for insn in instructions {
        let at = insn.at as i64;
        if Some(at) != end {
            pending = Vec::new();
        }
        end = Some(insn.end() as i64);
        if insn.insn.mnemonic() == Mnemonic::Push {
            pending.push(insn);
        } else {
            let width = runtime::numeric_stack_arguments(calls.get(&at).map_or("", String::as_str));
            if let Some(width) = width {
                let (mut consumed, mut total) = (Vec::new(), 0i64);
                for one in pending.iter().rev() {
                    total -= one.insn.stack_pointer_increment() as i64;
                    consumed.push(one.at as i64);
                    if total >= width {
                        if total == width {
                            values.extend(consumed.iter().copied());
                        }
                        break;
                    }
                }
            }
            pending = Vec::new();
        }
    }

    let long_arity = |name: &str| if runtime::numeric_stack_arguments(name) == Some(8) { Some(2) } else { None };

    if calls.values().any(|name| long_arity(name).is_some()) {
        let mapped = match blocks::code_map(found) {
            Ok(mapped) => mapped,
            Err(_) => panic!("AttributeError: 'str' object has no attribute 'starts'"),
        };
        for block in blocks::partition(found, &mapped) {
            for frame in stack::frames(&block, &calls, &long_arity) {
                values.extend(frame.pushed.iter().map(|one| one.at as i64));
            }
        }
    }
    values
}

/// Address-bearing spans and the push consuming a materialized address.
pub fn _pushes(found: &Module, instructions: Option<&[Insn]>) -> Vec<(i64, i64, Option<i64>)> {
    let owned;
    let instructions = match instructions {
        Some(instructions) => instructions,
        None => {
            owned = _instructions(found).unwrap_or_default();
            &owned
        }
    };
    let mut out = Vec::new();
    for insn in instructions {
        let at = insn.at as i64;
        // `str(insn.insn).lower()`'s first word is the mnemonic: measured
        // equal on every decode of every fixture's code segment.
        let text = format!("{:?}", insn.insn.mnemonic()).to_lowercase();
        // A `mov [x],ax` also carries a relocated field, but that is the
        // store's own displacement -- the address of the cell being written,
        // not an address being handed to anybody.
        let mut materialized = insn.insn.mnemonic() == Mnemonic::Mov
            && insn.insn.op0_kind() == OpKind::Register
            && matches!(insn.insn.op1_kind(), OpKind::Immediate16 | OpKind::Immediate32);
        let pushed = if materialized {
            _pushed_before_write(found, insn.end() as i64, insn.insn.op0_register())
        } else {
            None
        };
        materialized = pushed.is_some();
        if text.starts_with("push") || text.starts_with("lea") || materialized {
            out.push((at, insn.end() as i64, pushed));
        }
    }
    out
}

pub fn _pushed_before_write(found: &Module, mut at: i64, register: Register) -> Option<i64> {
    let root = register.full_register32();
    let mut info = declen::instruction_info_factory();
    while at < found.end {
        let one = declen::decode(&found.code, at as usize)?;
        if one.insn.flow_control() != FlowControl::Next {
            return None;
        }
        if one.insn.mnemonic() == Mnemonic::Push
            && one.insn.op0_kind() == OpKind::Register
            && one.insn.op0_register().full_register32() == root
        {
            return Some(at);
        }
        if info
            .info(&one.insn)
            .used_registers()
            .iter()
            .any(|access| declen::WRITES.contains(&access.access()) && access.register().full_register32() == root)
        {
            return None;
        }
        at = one.end() as i64;
    }
    None
}
