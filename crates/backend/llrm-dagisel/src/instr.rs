//! The pin's `MCInstrDesc` of a machine opcode: the fields selection,
//! scheduling and emission read.
//!
//! Each target's tables are generated from the pin's `*GenInstrInfo.inc` for
//! the opcodes its matcher tables emit; the target's `desc` finds one by
//! opcode.

/// `MCID::*` flags: bit `n` is the pin's `n`th flag.
pub mod flag {
    pub const VARIADIC: u64 = 1 << 0;
    pub const PSEUDO: u64 = 1 << 1;
    pub const META: u64 = 1 << 2;
    pub const RETURN: u64 = 1 << 3;
    pub const EH_SCOPE_RETURN: u64 = 1 << 4;
    pub const CALL: u64 = 1 << 5;
    pub const BARRIER: u64 = 1 << 6;
    pub const TERMINATOR: u64 = 1 << 7;
    pub const BRANCH: u64 = 1 << 8;
    pub const INDIRECT_BRANCH: u64 = 1 << 9;
    pub const COMPARE: u64 = 1 << 10;
    pub const MOVE_IMM: u64 = 1 << 11;
    pub const MOVE_REG: u64 = 1 << 12;
    pub const BITCAST: u64 = 1 << 13;
    pub const SELECT: u64 = 1 << 14;
    pub const DELAY_SLOT: u64 = 1 << 15;
    pub const FOLDABLE_AS_LOAD: u64 = 1 << 16;
    pub const MAY_LOAD: u64 = 1 << 17;
    pub const MAY_STORE: u64 = 1 << 18;
    pub const MAY_RAISE_FP_EXCEPTION: u64 = 1 << 19;
    pub const PREDICABLE: u64 = 1 << 20;
    pub const NOT_DUPLICABLE: u64 = 1 << 21;
    pub const UNMODELED_SIDE_EFFECTS: u64 = 1 << 22;
    pub const COMMUTABLE: u64 = 1 << 23;
    pub const CONVERTIBLE_TO3_ADDR: u64 = 1 << 24;
    pub const USES_CUSTOM_INSERTER: u64 = 1 << 25;
    pub const HAS_POST_I_SEL_HOOK: u64 = 1 << 26;
    pub const REMATERIALIZABLE: u64 = 1 << 27;
    pub const CHEAP_AS_A_MOVE: u64 = 1 << 28;
    pub const EXTRA_SRC_REG_ALLOC_REQ: u64 = 1 << 29;
    pub const EXTRA_DEF_REG_ALLOC_REQ: u64 = 1 << 30;
    pub const REG_SEQUENCE: u64 = 1 << 31;
    pub const EXTRACT_SUBREG: u64 = 1 << 32;
    pub const INSERT_SUBREG: u64 = 1 << 33;
    pub const CONVERGENT: u64 = 1 << 34;
    pub const ADD: u64 = 1 << 35;
    pub const TRAP: u64 = 1 << 36;
    pub const SOFT_FAIL: u64 = 1 << 37;
    pub const AUTHENTICATED: u64 = 1 << 38;
    pub const PRE_I_SEL_OPCODE: u64 = 1 << 39;
}

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
