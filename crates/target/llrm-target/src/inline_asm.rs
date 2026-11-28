//! What a target tells the translation of GCC inline assembly, as clang's
//! `TargetInfo` does: the registers a constraint may name, the assembler's
//! mode, the constants an immediate constraint takes. The translation is the
//! C driver's; each target crate states one [`InlineAsm`].

use llrm_lir::registers::RegId;

/// A target's facts for GCC inline assembly.
#[derive(Clone, Debug)]
pub struct InlineAsm {
    /// The block's mode, the prefix of its intrinsic's name
    /// (`llrm_mir::intrinsics::asm_name`).
    pub bits: u8,
    /// A block's operands sit in the registers its constraints name
    /// (`llrm.ia32.asm.*`); otherwise its memory operands are frame places
    /// (`llrm.ia16.code.*`).
    pub flat: bool,
    /// The register file (`registers.regs`): its `@asm` line lists what a
    /// constraint may name.
    pub registers: String,
    /// The directive that puts the assembler in the block's mode, where the
    /// target has more than one.
    pub mode: Option<&'static str>,
    /// Other spellings of a register a constraint names, and the register.
    pub aliases: &'static [(&'static str, &'static str)],
    pub immediates: Option<Immediates>,
    pub dead_stack_input: Option<StackPop>,
}

/// The constraint letters that take a constant, and whether a constant of
/// `width` bits, zero-extended, is one `letter` takes.
#[derive(Clone, Copy, Debug)]
pub struct Immediates {
    pub letters: &'static [u8],
    pub fits: fn(letter: u8, zero_extended: i128, width: u32) -> bool,
}

/// A stack register an input sits in that the block leaves on the stack unless
/// it clobbers or returns it, and the instruction that pops it after the
/// block, in AT&T and Intel syntax.
#[derive(Clone, Copy, Debug)]
pub struct StackPop {
    pub register: &'static str,
    pub att: &'static str,
    pub intel: &'static str,
}

/// Where a register an inline block names lives
/// (`TargetLowering::getRegForInlineAsmConstraint`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsmRegister {
    Register(RegId),
    /// A position on the x87 stack, 0 its top: an operand of the block's node,
    /// an `f80`.
    Stack(usize),
}
