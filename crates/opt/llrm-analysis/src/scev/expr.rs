//! The SCEV tree: `SCEV`, `SCEVUse` and the node kinds of
//! `ScalarEvolution.h` and `ScalarEvolutionExpressions.h` at `d1106deb71cc`.
//!
//! # How it works
//! A node is an index into the arena of [`super::ScalarEvolution`]; two equal
//! expressions are one index (`UniqueSCEVs`, the `FoldingSet`). A [`Use`] is an
//! index with the use-site `nuw`/`nsw` flags LLVM keeps in the low bits of the
//! operand pointer. The kinds are numbered as `SCEVTypes` is: the order is
//! "increasing complexity", which the folders sort by.

use llrm_mir::module::{BlockId, Operand};

use super::ap::Ap;

/// `SCEV *`: a node of the tree.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Scev(pub u32);

/// `SCEVNoWrapFlags`, as bits.
pub mod flags {
    pub const NONE: u8 = 0;
    /// `FlagNW`: no self-wrap.
    pub const NW: u8 = 1;
    /// `FlagNUW`: no unsigned wrap.
    pub const NUW: u8 = 2;
    /// `FlagNSW`: no signed wrap.
    pub const NSW: u8 = 4;
    pub const MASK: u8 = 7;
}

/// `SCEVUse`: an operand, with the `nuw` and `nsw` of this use (`NW` is
/// dropped).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Use {
    pub scev: Scev,
    /// The use-specific flags, `NUW | NSW` as `flags` has them.
    pub bits: u8,
}

impl Use {
    /// `SCEVUse(S)`: no use flags.
    pub fn of(scev: Scev) -> Self {
        Self { scev, bits: 0 }
    }
}

impl From<Scev> for Use {
    fn from(scev: Scev) -> Self {
        Self::of(scev)
    }
}

/// `SCEVTypes`' order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Type {
    Constant = 0,
    VScale = 1,
    Truncate = 2,
    ZeroExtend = 3,
    SignExtend = 4,
    Add = 5,
    Mul = 6,
    UDiv = 7,
    AddRec = 8,
    UMax = 9,
    SMax = 10,
    UMin = 11,
    SMin = 12,
    SequentialUMin = 13,
    PtrToAddr = 14,
    Unknown = 15,
    CouldNotCompute = 16,
}

/// `llvm::Type` of a SCEV: an integer, or a pointer in an address space.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Ty {
    Int(u32),
    Ptr(u32),
}

impl Ty {
    pub fn is_pointer(self) -> bool {
        matches!(self, Ty::Ptr(_))
    }

    pub fn is_integer(self) -> bool {
        matches!(self, Ty::Int(_))
    }
}

/// What a node is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Constant(Ap),
    Truncate(Use),
    ZeroExtend(Use),
    SignExtend(Use),
    PtrToAddr(Use),
    Add(Vec<Use>),
    Mul(Vec<Use>),
    UDiv(Use, Use),
    /// The operands, and the header of the loop.
    AddRec(Vec<Use>, BlockId),
    UMax(Vec<Use>),
    SMax(Vec<Use>),
    UMin(Vec<Use>),
    SMin(Vec<Use>),
    SequentialUMin(Vec<Use>),
    Unknown(Operand),
    CouldNotCompute,
}

impl Kind {
    pub fn tag(&self) -> Type {
        match self {
            Kind::Constant(_) => Type::Constant,
            Kind::Truncate(_) => Type::Truncate,
            Kind::ZeroExtend(_) => Type::ZeroExtend,
            Kind::SignExtend(_) => Type::SignExtend,
            Kind::PtrToAddr(_) => Type::PtrToAddr,
            Kind::Add(_) => Type::Add,
            Kind::Mul(_) => Type::Mul,
            Kind::UDiv(..) => Type::UDiv,
            Kind::AddRec(..) => Type::AddRec,
            Kind::UMax(_) => Type::UMax,
            Kind::SMax(_) => Type::SMax,
            Kind::UMin(_) => Type::UMin,
            Kind::SMin(_) => Type::SMin,
            Kind::SequentialUMin(_) => Type::SequentialUMin,
            Kind::Unknown(_) => Type::Unknown,
            Kind::CouldNotCompute => Type::CouldNotCompute,
        }
    }
}

/// One `SCEV` object.
#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Kind,
    pub ty: Option<Ty>,
    /// `ExpressionSize`.
    pub size: u16,
    /// `SubclassData`: the no-wrap flags of an n-ary expression.
    pub flags: u8,
    /// `CanonicalSCEV`.
    pub canonical: Scev,
}
