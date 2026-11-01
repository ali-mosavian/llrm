//! The Z80's register file as the allocator sees it.
//!
//! The classes are `GR8` (the bytes `b c d e h l a`, `a` last: the 8-bit ALU
//! works on it), `ACC` (`a`), `GR16` (the pairs `bc de hl`), `HLR` (`hl`) and
//! `IY`. `ix` is the frame and `sp` the stack. A call keeps nothing but
//! `ix` and `sp` (`calling.toml`), so every allocatable register is
//! caller-saved.

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::{REGISTER_INFO, reg};

/// A virtual register's class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    Gr8,
    Acc,
    Gr16,
    Hlr,
    Iy,
    /// Unconstrained: any allocatable register.
    Any,
}

pub type MFunction = llrm_spike_mf::MFunction<Class>;

const BYTES: [RegId; 7] = [reg::B, reg::C, reg::D, reg::E, reg::H, reg::L, reg::A];
const PAIRS: [RegId; 3] = [reg::BC, reg::DE, reg::HL];

impl Class {
    fn is_byte(self) -> bool {
        matches!(self, Class::Gr8 | Class::Acc)
    }
}

/// The Z80 behind the allocator.
pub struct Z80Registers;

impl Target for Z80Registers {
    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        match (one, other) {
            (one, two) if one == two => Some(one),
            (Class::Any, one) | (one, Class::Any) => Some(one),
            (Class::Gr8, Class::Acc) | (Class::Acc, Class::Gr8) => Some(Class::Acc),
            (Class::Gr16, Class::Hlr) | (Class::Hlr, Class::Gr16) => Some(Class::Hlr),
            _ => None,
        }
    }

    fn any() -> Class {
        Class::Any
    }

    fn candidates(class: Class) -> Vec<RegId> {
        match class {
            Class::Gr8 => BYTES.to_vec(),
            Class::Acc => vec![reg::A],
            Class::Gr16 => PAIRS.to_vec(),
            Class::Hlr => vec![reg::HL],
            Class::Iy => vec![reg::IY],
            Class::Any => BYTES.iter().chain(&PAIRS).chain(&[reg::IY]).copied().collect(),
        }
    }

    /// The register of `class`'s width in `root`: a byte class takes the low
    /// byte of a pair.
    fn view(
        root: RegId,
        class: Class,
    ) -> RegId {
        if class.is_byte() && REGISTER_INFO.bytes(root) == Some(2) {
            return REGISTER_INFO.view(root, 8).unwrap_or(root);
        }
        root
    }

    fn root(register: RegId) -> RegId {
        REGISTER_INFO.root(register)
    }

    fn reserved(root: RegId) -> bool {
        matches!(root, r if r == reg::SP || r == reg::IX || r == reg::FLAGS)
    }

    fn caller_saved() -> Vec<RegId> {
        vec![reg::A, reg::BC, reg::DE, reg::HL, reg::IY]
    }

    /// A pair and its bytes share storage; two bytes of one pair do not.
    fn overlaps(
        one: RegId,
        other: RegId,
    ) -> bool {
        REGISTER_INFO.root(one) == REGISTER_INFO.root(other)
            && REGISTER_INFO.lanes(one) & REGISTER_INFO.lanes(other) != 0
    }

    /// `ld (ix+d),r` for a byte, two of them for a pair: `ld` only, so a spill
    /// never touches the flags.
    fn spill_store(
        class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let name = if class.is_byte() { "SPILL8" } else { "SPILL16" };
        Some(Inst { may_store: true, ..Inst::new(Kind::Op(name), vec![Operand::Frame(slot), Operand::Use(register)]) })
    }

    fn spill_load(
        class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        let name = if class.is_byte() { "RELOAD8" } else { "RELOAD16" };
        Some(Inst { may_load: true, ..Inst::new(Kind::Op(name), vec![Operand::Def(register), Operand::Frame(slot)]) })
    }
}
