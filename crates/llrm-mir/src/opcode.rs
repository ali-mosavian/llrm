//! Opcodes and the schema every consumer reads them through: spelling, the
//! role of each operand, and the type rule.

use crate::types::{MirContext, TypeId};

/// What an integer operation does when its result does not fit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Overflow {
    Wrap,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Predicate {
    Equal,
    NotEqual,
    SignedLess,
    SignedLessEqual,
    SignedGreater,
    SignedGreaterEqual,
    UnsignedLess,
    UnsignedLessEqual,
    UnsignedGreater,
    UnsignedGreaterEqual,
}

impl Predicate {
    pub const ALL: [Predicate; 10] = [
        Self::Equal,
        Self::NotEqual,
        Self::SignedLess,
        Self::SignedLessEqual,
        Self::SignedGreater,
        Self::SignedGreaterEqual,
        Self::UnsignedLess,
        Self::UnsignedLessEqual,
        Self::UnsignedGreater,
        Self::UnsignedGreaterEqual,
    ];

    /// The mnemonic and the infix operator: `compare.signed a > b`.
    pub fn spelling(self) -> (&'static str, &'static str) {
        match self {
            Self::Equal => ("compare", "=="),
            Self::NotEqual => ("compare", "!="),
            Self::SignedLess => ("compare.signed", "<"),
            Self::SignedLessEqual => ("compare.signed", "<="),
            Self::SignedGreater => ("compare.signed", ">"),
            Self::SignedGreaterEqual => ("compare.signed", ">="),
            Self::UnsignedLess => ("compare.unsigned", "<"),
            Self::UnsignedLessEqual => ("compare.unsigned", "<="),
            Self::UnsignedGreater => ("compare.unsigned", ">"),
            Self::UnsignedGreaterEqual => ("compare.unsigned", ">="),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Opcode {
    Copy,
    Phi,
    Select,
    Add(Overflow),
    Sub(Overflow),
    Mul(Overflow),
    And,
    Or,
    Xor,
    Compare(Predicate),
    Truncate,
    ZeroExtend,
    SignExtend,
    Goto,
    If,
    Return,
    Unreachable,
}

/// The role an operand plays, which fixes its type and so a constant's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    /// `i1`.
    Bool,
    /// The type of the result, shared by every `Result` operand.
    Result,
    /// One integer type shared by every `Shared` operand, not the result's.
    Shared,
    /// Any integer; a constant here states its type.
    Free,
    /// The function's `n`th return type.
    Return(usize),
    Edge,
}

impl Opcode {
    /// The opcodes written `result = mnemonic operands`: all but phis,
    /// comparisons and terminators, which have syntax of their own.
    pub const ORDINARY: [Opcode; 11] = [
        Self::Copy,
        Self::Select,
        Self::Add(Overflow::Wrap),
        Self::Sub(Overflow::Wrap),
        Self::Mul(Overflow::Wrap),
        Self::And,
        Self::Or,
        Self::Xor,
        Self::Truncate,
        Self::ZeroExtend,
        Self::SignExtend,
    ];

    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Phi => "phi",
            Self::Select => "select",
            Self::Add(Overflow::Wrap) => "add.wrap",
            Self::Sub(Overflow::Wrap) => "sub.wrap",
            Self::Mul(Overflow::Wrap) => "mul.wrap",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::Compare(predicate) => predicate.spelling().0,
            Self::Truncate => "trunc",
            Self::ZeroExtend => "zext",
            Self::SignExtend => "sext",
            Self::Goto => "goto",
            Self::If => "if",
            Self::Return => "return",
            Self::Unreachable => "unreachable",
        }
    }

    pub fn is_terminator(self) -> bool {
        matches!(self, Self::Goto | Self::If | Self::Return | Self::Unreachable)
    }

    pub fn result_count(self) -> usize {
        usize::from(!self.is_terminator())
    }

    /// Each operand's role, for an instruction with `count` operands.
    pub fn slots(self, count: usize) -> Vec<Slot> {
        match self {
            Self::Copy => vec![Slot::Result],
            Self::Phi => (0..count).map(|at| if at % 2 == 0 { Slot::Edge } else { Slot::Result }).collect(),
            Self::Select => vec![Slot::Bool, Slot::Result, Slot::Result],
            Self::Add(_) | Self::Sub(_) | Self::Mul(_) | Self::And | Self::Or | Self::Xor => {
                vec![Slot::Result, Slot::Result]
            }
            Self::Compare(_) => vec![Slot::Shared, Slot::Shared],
            Self::Truncate | Self::ZeroExtend | Self::SignExtend => vec![Slot::Free],
            Self::Goto => vec![Slot::Edge],
            Self::If => vec![Slot::Bool, Slot::Edge, Slot::Edge],
            Self::Return => (0..count).map(Slot::Return).collect(),
            Self::Unreachable => vec![],
        }
    }

    /// Whether `count` operands is a shape this opcode takes.
    pub fn takes(self, count: usize) -> bool {
        match self {
            Self::Phi => count.is_multiple_of(2),
            Self::Return => true,
            _ => self.slots(count).len() == count,
        }
    }

    /// The result type when the operands fix it: `known` is each operand's
    /// type where known. `None` means the definition must state it.
    pub fn infer(self, context: &MirContext, known: &[Option<TypeId>]) -> Option<TypeId> {
        match self {
            Self::Compare(_) => Some(context.bool()),
            Self::Phi | Self::Truncate | Self::ZeroExtend | Self::SignExtend => None,
            _ => self.slots(known.len()).iter().zip(known).find_map(|(slot, ty)| ty.filter(|_| *slot == Slot::Result)),
        }
    }

    /// The opcode-specific type rule over the slots' agreement, which the
    /// verifier checks first: an error names what is wrong.
    pub fn check(self, context: &MirContext, operands: &[TypeId], result: Option<TypeId>) -> Result<(), String> {
        let int = |ty: TypeId| context.int_bits(ty).ok_or_else(|| format!("{} is not an integer", context.display(ty)));
        match self {
            Self::Add(_) | Self::Sub(_) | Self::Mul(_) | Self::And | Self::Or | Self::Xor | Self::Compare(_) => {
                int(operands[0]).map(|_| ())
            }
            Self::Truncate | Self::ZeroExtend | Self::SignExtend => {
                let (from, to) = (int(operands[0])?, int(result.expect("a conversion has a result"))?);
                let fits = if self == Self::Truncate { to < from } else { to > from };
                if fits { Ok(()) } else { Err(format!("{} cannot take i{from} to i{to}", self.mnemonic())) }
            }
            _ => Ok(()),
        }
    }
}
