//! The pin's `MCInstrDesc` of a machine opcode: the fields selection,
//! scheduling and emission read.
//!
//! The tables are generated (`generated/instrinfo.rs`) from the pin's
//! `X86GenInstrInfo.inc` for the opcodes the matcher tables emit; [`desc`]
//! finds one by opcode.

pub use crate::generated::instrinfo::flag;

/// `MCOI` operand types, the ones every target shares; a target's own are `>=
/// FIRST_TARGET`.
pub mod operand_type {
    pub const UNKNOWN: u16 = 0;
    pub const IMMEDIATE: u16 = 1;
    pub const REGISTER: u16 = 2;
    pub const MEMORY: u16 = 3;
    pub const PCREL: u16 = 4;
    pub const FIRST_TARGET: u16 = 15;
    /// `X86::OPERAND_COND_CODE` (`OPERAND_ROUNDING_CONTROL` is the first target
    /// type).
    pub const X86_COND_CODE: u16 = FIRST_TARGET + 1;
}

/// One operand of an instruction (`MCOperandInfo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperandDesc {
    /// The register class (`X86::GR32RegClassID`), or `-1` for none.
    pub reg_class: i16,
    /// `MCOI::OPERAND_*`, or a target operand type.
    pub kind: u16,
    /// The operand this one is tied to, or `-1`.
    pub tied_to: i8,
    pub early_clobber: bool,
    /// `reg_class` names a pointer class chosen by the hardware mode
    /// (`LookupRegClassByHwMode`).
    pub pointer_class: bool,
    pub branch_target: bool,
}

/// `MCInstrDesc`.
#[derive(Clone, Copy, Debug)]
pub struct InstrDesc {
    pub num_operands: u8,
    pub num_defs: u8,
    /// Bytes in the encoding, 0 when it varies.
    pub size: u8,
    /// `flag::*`.
    pub flags: u64,
    /// The pin's register numbers.
    pub implicit_uses: &'static [u16],
    pub implicit_defs: &'static [u16],
    pub operands: &'static [OperandDesc],
}

impl InstrDesc {
    pub fn has(
        &self,
        flag: u64,
    ) -> bool {
        self.flags & flag != 0
    }
    pub fn may_load(&self) -> bool {
        self.has(flag::MAY_LOAD)
    }
    pub fn may_store(&self) -> bool {
        self.has(flag::MAY_STORE)
    }
    pub fn is_call(&self) -> bool {
        self.has(flag::CALL)
    }
    pub fn is_commutable(&self) -> bool {
        self.has(flag::COMMUTABLE)
    }
    pub fn is_pseudo(&self) -> bool {
        self.has(flag::PSEUDO)
    }
}

/// The descriptor of a machine opcode the matcher tables emit.
pub fn desc(opcode: u16) -> Option<&'static InstrDesc> {
    let table: &'static [(u16, InstrDesc)] = &crate::generated::instrinfo::INSTRS;
    table.binary_search_by_key(&opcode, |&(o, _)| o).ok().map(|at| &table[at].1)
}
