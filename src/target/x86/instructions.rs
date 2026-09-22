//! Typed x86 instruction semantics used before encoding selection.
//!
//! These opcodes deliberately do not describe ModR/M, immediate, prefix, or
//! calling-convention forms.  Machine operands and later target constraints
//! carry those details so one semantic opcode does not grow a separate variant
//! for every byte encoding.

use crate::codegen::machine::TargetOpcode;

/// The operand width selected for an x86 integer instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum OperandSize {
    Byte = 8,
    Word = 16,
    Dword = 32,
}

/// The exact byte format of an x87 memory operand.
///
/// x87 instructions select an encoding from both their operation and the
/// bytes addressed by their memory operand.  The format is consequently an
/// explicit target operand, rather than a property inferred from an IR type
/// or an address's size.  Its numeric values are stable Machine/MC contract
/// values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum X87MemoryFormat {
    Float32 = 1,
    Float64 = 2,
    Float80 = 3,
    Signed16 = 4,
    Signed32 = 5,
    Signed64 = 6,
    Control16 = 7,
}

impl X87MemoryFormat {
    /// The stable immediate value carried by an x87 memory instruction.
    pub const fn raw(self) -> u8 {
        self as u8
    }

    /// Decodes one stable x87 memory-format immediate.
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Float32),
            2 => Some(Self::Float64),
            3 => Some(Self::Float80),
            4 => Some(Self::Signed16),
            5 => Some(Self::Signed32),
            6 => Some(Self::Signed64),
            7 => Some(Self::Control16),
            _ => None,
        }
    }

    /// Number of addressed bytes for this x87 memory format.
    pub(crate) const fn byte_width(self) -> u32 {
        match self {
            Self::Float32 | Self::Signed32 => 4,
            Self::Float64 | Self::Signed64 => 8,
            Self::Float80 => 10,
            Self::Signed16 | Self::Control16 => 2,
        }
    }
}

impl OperandSize {
    /// Width in bits.
    pub const fn bits(self) -> u8 {
        self as u8
    }
}

/// One condition tested by an x86 conditional branch.
///
/// The names state the flag relation rather than choosing an assembly
/// mnemonic, keeping signed and unsigned comparisons distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ConditionCode {
    Overflow = 1,
    NotOverflow = 2,
    Below = 3,
    AboveOrEqual = 4,
    Equal = 5,
    NotEqual = 6,
    BelowOrEqual = 7,
    Above = 8,
    Sign = 9,
    NotSign = 10,
    Parity = 11,
    NotParity = 12,
    Less = 13,
    GreaterOrEqual = 14,
    LessOrEqual = 15,
    Greater = 16,
}

/// The interpretation of an ordered integer comparison.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ComparisonKind {
    Signed,
    Unsigned,
}

impl ConditionCode {
    /// Every condition code in stable numeric order.
    pub const ALL: [Self; 16] = [
        Self::Overflow,
        Self::NotOverflow,
        Self::Below,
        Self::AboveOrEqual,
        Self::Equal,
        Self::NotEqual,
        Self::BelowOrEqual,
        Self::Above,
        Self::Sign,
        Self::NotSign,
        Self::Parity,
        Self::NotParity,
        Self::Less,
        Self::GreaterOrEqual,
        Self::LessOrEqual,
        Self::Greater,
    ];

    /// The condition with the opposite truth value.
    pub const fn inverted(self) -> Self {
        match self {
            Self::Overflow => Self::NotOverflow,
            Self::NotOverflow => Self::Overflow,
            Self::Below => Self::AboveOrEqual,
            Self::AboveOrEqual => Self::Below,
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::BelowOrEqual => Self::Above,
            Self::Above => Self::BelowOrEqual,
            Self::Sign => Self::NotSign,
            Self::NotSign => Self::Sign,
            Self::Parity => Self::NotParity,
            Self::NotParity => Self::Parity,
            Self::Less => Self::GreaterOrEqual,
            Self::GreaterOrEqual => Self::Less,
            Self::LessOrEqual => Self::Greater,
            Self::Greater => Self::LessOrEqual,
        }
    }

    /// Whether this code describes a signed or unsigned ordered comparison.
    ///
    /// Equality and individual-flag conditions have no signedness.
    pub const fn comparison_kind(self) -> Option<ComparisonKind> {
        match self {
            Self::Below | Self::AboveOrEqual | Self::BelowOrEqual | Self::Above => {
                Some(ComparisonKind::Unsigned)
            }
            Self::Less | Self::GreaterOrEqual | Self::LessOrEqual | Self::Greater => {
                Some(ComparisonKind::Signed)
            }
            Self::Overflow
            | Self::NotOverflow
            | Self::Equal
            | Self::NotEqual
            | Self::Sign
            | Self::NotSign
            | Self::Parity
            | Self::NotParity => None,
        }
    }
}

/// Semantic x86 operations selected for the initial integer and control-flow
/// backend.
///
/// Integer width is an [`OperandSize`] carried by selection or instruction
/// constraints.  `JumpConditional` carries its [`ConditionCode`] the same
/// way, rather than multiplying opcodes by width and branch spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum X86Opcode {
    /// A target-independent register-to-register copy represented as a target
    /// opcode so it can survive selection and be removed after allocation.
    Copy = 1,
    /// A parallel-copy member created when lowering SSA block parameters.
    PhiCopy = 2,

    Mov = 3,
    Lea = 4,
    Add = 5,
    Sub = 6,
    Imul = 7,
    Idiv = 8,
    And = 9,
    Or = 10,
    Xor = 11,
    ShiftLeft = 12,
    ShiftRightLogical = 13,
    ShiftRightArithmetic = 14,
    Neg = 15,
    Not = 16,
    Cmp = 17,
    Test = 18,
    Push = 19,
    Pop = 20,

    CallNear = 21,
    CallFar = 22,
    ReturnNear = 23,
    ReturnFar = 24,
    Jump = 25,
    JumpConditional = 26,

    /// Load from a selected frame, global, or register-indirect address.
    Load = 27,
    /// Store to a selected frame, global, or register-indirect address.
    Store = 28,
    /// Join the low and high ABI words into one semantic dword value.
    MergeWords = 29,
    /// Extract the low ABI word from one semantic dword value.
    LowWord = 30,
    /// Extract the high ABI word from one semantic dword value.
    HighWord = 31,
    /// Restore SP and BP from a conventional frame in one instruction.
    Leave = 32,
    /// Sign-extend a 16-bit integer register into a 32-bit integer register.
    SignExtendWordToDword = 33,
    /// Funnel a dword source's upper word into a dword destination's low word.
    ShiftLeftDouble = 34,
    /// Unsigned division of the implicit DX:AX or EDX:EAX dividend.
    Div = 35,
    /// Sign-extend AX into DX or EAX into EDX for a following signed divide.
    CwdCdq = 36,
    /// Zero-extend a 16-bit integer register into a 32-bit integer register.
    ZeroExtendWordToDword = 37,

    X87Load = 38,
    X87Store = 39,
    X87StorePop = 40,
    X87IntegerLoad = 41,
    X87IntegerStore = 42,
    X87IntegerStorePop = 43,
    /// A target pseudo expanded to a control-word sequence before MC.
    X87IntegerStoreTrunc = 44,
    X87Add = 45,
    X87Subtract = 46,
    X87SubtractReverse = 47,
    X87Multiply = 48,
    X87Divide = 49,
    X87DivideReverse = 50,
    X87AddPop = 51,
    X87SubtractPop = 52,
    X87SubtractReversePop = 53,
    X87MultiplyPop = 54,
    X87DividePop = 55,
    X87DivideReversePop = 56,
    X87Compare = 57,
    X87ComparePop = 58,
    X87ComparePop2 = 59,
    X87StackLoad = 60,
    X87StackStorePop = 61,
    X87Exchange = 62,
    X87LoadZero = 63,
    X87LoadOne = 64,
    X87ChangeSign = 65,
    X87Absolute = 66,
    X87SquareRoot = 67,
    X87StoreStatusWord = 68,
    X87StoreControlWord = 69,
    X87LoadControlWord = 70,
    Wait = 71,
    Sahf = 72,
    /// A zero-byte Machine-IR ownership anchor.  It reaches MC as an empty
    /// data fragment, never as x86's one-byte NOP instruction.
    Nothing = 73,
}

impl X86Opcode {
    /// Every opcode in stable numeric order.
    pub const ALL: [Self; 73] = [
        Self::Copy,
        Self::PhiCopy,
        Self::Mov,
        Self::Lea,
        Self::Add,
        Self::Sub,
        Self::Imul,
        Self::Idiv,
        Self::And,
        Self::Or,
        Self::Xor,
        Self::ShiftLeft,
        Self::ShiftRightLogical,
        Self::ShiftRightArithmetic,
        Self::Neg,
        Self::Not,
        Self::Cmp,
        Self::Test,
        Self::Push,
        Self::Pop,
        Self::CallNear,
        Self::CallFar,
        Self::ReturnNear,
        Self::ReturnFar,
        Self::Jump,
        Self::JumpConditional,
        Self::Load,
        Self::Store,
        Self::MergeWords,
        Self::LowWord,
        Self::HighWord,
        Self::Leave,
        Self::SignExtendWordToDword,
        Self::ShiftLeftDouble,
        Self::Div,
        Self::CwdCdq,
        Self::ZeroExtendWordToDword,
        Self::X87Load,
        Self::X87Store,
        Self::X87StorePop,
        Self::X87IntegerLoad,
        Self::X87IntegerStore,
        Self::X87IntegerStorePop,
        Self::X87IntegerStoreTrunc,
        Self::X87Add,
        Self::X87Subtract,
        Self::X87SubtractReverse,
        Self::X87Multiply,
        Self::X87Divide,
        Self::X87DivideReverse,
        Self::X87AddPop,
        Self::X87SubtractPop,
        Self::X87SubtractReversePop,
        Self::X87MultiplyPop,
        Self::X87DividePop,
        Self::X87DivideReversePop,
        Self::X87Compare,
        Self::X87ComparePop,
        Self::X87ComparePop2,
        Self::X87StackLoad,
        Self::X87StackStorePop,
        Self::X87Exchange,
        Self::X87LoadZero,
        Self::X87LoadOne,
        Self::X87ChangeSign,
        Self::X87Absolute,
        Self::X87SquareRoot,
        Self::X87StoreStatusWord,
        Self::X87StoreControlWord,
        Self::X87LoadControlWord,
        Self::Wait,
        Self::Sahf,
        Self::Nothing,
    ];

    /// The opaque target-independent Machine IR opcode identifier.
    pub const fn machine_opcode(self) -> TargetOpcode {
        TargetOpcode::new(self as u32)
    }

    /// Recovers an x86 opcode from its stable numeric identity.
    pub fn from_raw(raw: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| *candidate as u32 == raw)
    }

    /// Recovers an x86 opcode from its Machine IR identity.
    pub fn from_machine_opcode(opcode: TargetOpcode) -> Option<Self> {
        Self::from_raw(opcode.get())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{ComparisonKind, ConditionCode, X86Opcode, X87MemoryFormat};

    #[test]
    fn opcode_ids_are_unique_and_stable() {
        let mut ids = BTreeSet::new();
        for opcode in X86Opcode::ALL {
            let id = opcode.machine_opcode().get();
            assert_eq!(id, opcode as u32);
            assert!(ids.insert(id), "duplicate opcode id {id}");
        }
        assert_eq!(X86Opcode::Copy.machine_opcode().get(), 1);
        assert_eq!(X86Opcode::JumpConditional.machine_opcode().get(), 26);
        assert_eq!(X86Opcode::ShiftLeftDouble.machine_opcode().get(), 34);
        assert_eq!(X86Opcode::ZeroExtendWordToDword.machine_opcode().get(), 37);
        assert_eq!(X86Opcode::X87Load.machine_opcode().get(), 38);
        assert_eq!(X86Opcode::Sahf.machine_opcode().get(), 72);
    }

    #[test]
    fn x87_memory_format_ids_are_stable_and_exhaustive() {
        for format in [
            X87MemoryFormat::Float32,
            X87MemoryFormat::Float64,
            X87MemoryFormat::Float80,
            X87MemoryFormat::Signed16,
            X87MemoryFormat::Signed32,
            X87MemoryFormat::Signed64,
            X87MemoryFormat::Control16,
        ] {
            assert_eq!(X87MemoryFormat::from_raw(format.raw()), Some(format));
        }
        assert_eq!(X87MemoryFormat::from_raw(0), None);
        assert_eq!(X87MemoryFormat::from_raw(8), None);
    }

    #[test]
    fn condition_inversions_are_complete_pairs() {
        for condition in ConditionCode::ALL {
            assert_ne!(condition, condition.inverted());
            assert_eq!(condition.inverted().inverted(), condition);
        }
        assert_eq!(
            ConditionCode::Below.comparison_kind(),
            Some(ComparisonKind::Unsigned)
        );
        assert_eq!(
            ConditionCode::Less.comparison_kind(),
            Some(ComparisonKind::Signed)
        );
        assert_eq!(ConditionCode::Equal.comparison_kind(), None);
    }
}
