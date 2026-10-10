//! Inline assembly: the x86 assembler is `llrm_x86::asm`; what is here is the
//! QB runtime's naming of registers around it (`Reg`, the registers a runtime
//! routine's contract names).

use llrm_lir::registers::RegId;
use llrm_x86::asm::REGISTERS;
pub use llrm_x86::asm::{Mode, Refusal, assembled};

use crate::abi::runtime::Reg;

/// The part of its 16-bit register an operand register is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    Word,
    Low,
    High,
}

/// Registers an input or output may name: the ones a call's argument can be
/// pinned to.
const OPERANDS: &[(&str, Reg, Part)] = &[
    ("ax", Reg::Ax, Part::Word),
    ("al", Reg::Ax, Part::Low),
    ("ah", Reg::Ax, Part::High),
    ("bx", Reg::Bx, Part::Word),
    ("bl", Reg::Bx, Part::Low),
    ("bh", Reg::Bx, Part::High),
    ("cx", Reg::Cx, Part::Word),
    ("cl", Reg::Cx, Part::Low),
    ("ch", Reg::Cx, Part::High),
    ("dx", Reg::Dx, Part::Word),
    ("dl", Reg::Dx, Part::Low),
    ("dh", Reg::Dx, Part::High),
    ("si", Reg::Si, Part::Word),
    ("di", Reg::Di, Part::Word),
];

/// An input or output register: its 16-bit register and the part named.
pub fn operand_register(name: &str) -> Option<(Reg, Part)> {
    OPERANDS.iter().find(|(spelled, ..)| spelled.eq_ignore_ascii_case(name)).map(|(_, reg, part)| (*reg, *part))
}

/// A register a block may declare it changes. The rest -- sp, bp, ds, ss,
/// cs -- the program depends on, and a block restores any it changes.
pub fn clobbered(name: &str) -> Option<Reg> {
    match name.to_ascii_lowercase().as_str() {
        "es" => Some(Reg::Es),
        "flags" => Some(Reg::Flags),
        _ => operand_register(name).map(|(reg, _)| reg).or_else(|| view(name).map(|(reg, _)| reg)),
    }
}

/// A 16-bit register by the name the HIR spells it with.
pub fn named(name: &str) -> Option<Reg> {
    Reg::ALL.into_iter().find(|reg| reg.name().eq_ignore_ascii_case(name))
}

/// A general register by the name the HIR spells it with: its root `Reg` (the
/// 16-bit register whose 32-bit view a flat target has) and the bits of the
/// view named (`ax` is (Ax, 16), `eax` (Ax, 32), `al` (Ax, 8)).
pub fn view(name: &str) -> Option<(Reg, u32)> {
    // A register the runtime names whole: ax..di, es, flags.
    if let Some(reg) = named(name) {
        return Some((reg, 16));
    }
    let register = *REGISTERS.get(&name.to_lowercase())?;
    if !(register.is_gpr8() || register.is_gpr16() || register.is_gpr32()) {
        return None;
    }
    // iced numbers every view of a general register as the register itself: the
    // root is the 16-bit register of that number.
    let sixteen = crate::backend::registerinfo::view(crate::backend::registerinfo::root(register), 16)?;
    let bits = if register.is_gpr8() {
        8
    } else if register.is_gpr16() {
        16
    } else {
        32
    };
    named(&format!("{sixteen:?}")).map(|reg| (reg, bits))
}

/// The machine register for the view of `bits` of the root `reg`: `Ax` of 32
/// bits is EAX.
pub fn machine(
    reg: Reg,
    bits: u32,
) -> Option<RegId> {
    let sixteen = REGISTERS.get(&reg.name().to_lowercase()).copied()?;
    Some(if bits == 32 { sixteen.full_register32() } else { sixteen })
}
