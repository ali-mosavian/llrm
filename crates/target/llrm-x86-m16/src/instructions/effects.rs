//! The registers an instruction reads and writes, from its row in `x86.instr`: its operands as `Semantics` has
//! them, and the row's `reads` and `writes` for the registers it uses without naming. LLVM's `MCInstrDesc` operand
//! defs and `ImplicitUses`/`ImplicitDefs`. Flags are not here yet.

use iced_x86::Register;
use llrm_lir::{Loc, Operation, Semantics};
use llrm_x86::select;

use super::parse::Form;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub reads: Vec<Register>,
    pub writes: Vec<Register>,
}

/// The register `name` is at the address size `bits`.
pub fn root(name: &str, bits: u32) -> Register {
    let wide = bits == 32;
    match (name, wide) {
        ("ax", true) => Register::EAX,
        ("bx", true) => Register::EBX,
        ("cx", true) => Register::ECX,
        ("dx", true) => Register::EDX,
        ("si", true) => Register::ESI,
        ("di", true) => Register::EDI,
        ("bp", true) => Register::EBP,
        ("sp", true) => Register::ESP,
        ("ax", _) => Register::AX,
        ("bx", _) => Register::BX,
        ("cx", _) => Register::CX,
        ("dx", _) => Register::DX,
        ("si", _) => Register::SI,
        ("di", _) => Register::DI,
        ("bp", _) => Register::BP,
        ("sp", _) => Register::SP,
        ("es", _) => Register::ES,
        ("ds", _) => Register::DS,
        ("fs", _) => Register::FS,
        ("ss", _) => Register::SS,
        ("ah", _) => Register::AH,
        _ => Register::GS,
    }
}

/// The registers `place` reads or writes as an operand: the register itself, or the ones its address is encoded with.
fn used(places: &[Loc], bits: u32, reads: &mut Vec<Register>, mut writes: Option<&mut Vec<Register>>) {
    let mut add = |into: &mut Vec<Register>, register: Register| {
        if register != Register::None && !into.contains(&register) {
            into.push(register);
        }
    };
    for place in places {
        let address = match place {
            Loc::Reg(one) => {
                match writes.as_deref_mut() {
                    Some(into) => add(into, one.register),
                    None => add(reads, one.register),
                }
                continue;
            }
            Loc::Mem(one) => select::operand_of(one, bits),
            Loc::Address(one) => select::address_operand(one, bits, bits / 8),
            Loc::Imm(_) | Loc::St(_) | Loc::Held(_) => continue,
        };
        if let Some((operand, _)) = address {
            add(reads, operand.base);
            add(reads, operand.index);
            if matches!(place, Loc::Mem(_)) {
                // The segment it names, else the default: SS behind the stack's registers, DS otherwise.
                let stack = [Register::BP, Register::EBP, Register::SP, Register::ESP].contains(&operand.base);
                add(reads, if operand.segment_prefix != Register::None { operand.segment_prefix } else if stack { Register::SS } else { Register::DS });
            }
        }
    }
}

/// The bytes `place` is, where it says.
fn bytes_of(place: &Loc) -> Option<u32> {
    match place {
        Loc::Reg(one) => Some(one.width),
        Loc::Mem(one) => Some(one.width),
        Loc::Imm(one) => Some(one.width),
        _ => None,
    }
}

/// The row of `what`: its mnemonic, operation and operand counts.
fn row<'a>(forms: &'a [Form], what: &Semantics) -> Option<&'a Form> {
    // The machine instruction is the mnemonic's, whatever operation LIR lowered it as; a comparison with none is `cmp`.
    let name = match what.name.as_deref() {
        Some(name) if !name.is_empty() => name,
        _ if what.op == Operation::Compare => "cmp",
        _ => return None,
    };
    forms.iter().find(|form| {
        form.name == name
            && form.dests.len() == what.dests.len()
            && form.sources.len() == what.sources.len()
    })
}

/// What `what` reads and writes in `bits`-bit code, or None where its row does not say: a transfer of control, or no row.
pub fn effects(forms: &[Form], bits: u32, what: &Semantics) -> Option<Effects> {
    if matches!(what.op, Operation::Branch | Operation::Jump | Operation::Call | Operation::Return | Operation::Escape) {
        return None;
    }
    let form = row(forms, what)?;
    let width = what.sources.iter().filter_map(bytes_of).min().map_or(0, |bytes| bytes * 8);
    let mut found = Effects::default();
    used(&what.sources, bits, &mut found.reads, None);
    let mut writes = Vec::new();
    used(&what.dests, bits, &mut found.reads, Some(&mut writes));
    found.writes = writes;
    for (list, into) in [(&form.reads, &mut found.reads), (&form.writes, &mut found.writes)] {
        for entry in list {
            let (dropped, entry) = entry.strip_prefix('-').map_or((false, entry.as_str()), |rest| (true, rest));
            let (name, only) = entry.split_once('@').map_or((entry, None), |(name, width)| (name, width.parse::<u32>().ok()));
            if only.is_some_and(|only| only != width) {
                continue;
            }
            let register = root(name, bits);
            if dropped {
                into.retain(|one| one.full_register() != register.full_register());
            } else if !into.contains(&register) {
                into.push(register);
            }
        }
    }
    Some(found)
}
