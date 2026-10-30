//! What instruction selection knows of a MIR value: which register holds it,
//! and what is known of the bits beyond its type's width. LLVM's DAG types
//! legalize `iN` to a 32- or 64-bit register whose extra bits are `any`; the
//! `Known` record is how this selector avoids extending a value twice.

use crate::mc::inst::{Extend, FpSize, Gpr, Vreg, Width};

/// What is known of a register's bits above the value's width `bits`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Known {
    /// Nothing, not even of the bits above the 32nd.
    Nothing,
    /// Bits 32 and above are zero (every `w` register write leaves them so);
    /// bits `bits..32` are unknown.
    Hi32Zero,
    /// Bits `bits` and above are zero.
    Zero,
    /// Bits `bits` and above are copies of bit `bits - 1`.
    Sign,
}

/// An integer value of 1 to 64 bits in a register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct G {
    pub reg: Gpr,
    pub bits: u32,
    pub known: Known,
}

impl G {
    /// The width of the instructions that work on it.
    pub fn width(self) -> Width {
        width_of(self.bits)
    }
}

/// The register width an `iN` is legalized to.
pub fn width_of(bits: u32) -> Width {
    if bits <= 32 { Width::W } else { Width::X }
}

/// Where an address is based.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Base {
    Reg(Gpr),
    /// A frame object (an `alloca`).
    Slot(u32),
    /// A global, reached through the page of its symbol; `got` where the
    /// symbol may be defined elsewhere.
    Global {
        name: String,
        got: bool,
    },
}

/// A pointer held as an expression, so a load or store can fold it into its
/// addressing mode: `base + offset + (index << shift)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddrExpr {
    pub base: Base,
    pub offset: i64,
    pub index: Option<Index>,
}

/// A register index of an address, scaled by a left shift, extended if it is a
/// 32-bit value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Index {
    pub reg: Gpr,
    pub shift: u8,
    pub extend: Extend,
}

impl AddrExpr {
    pub fn of(base: Base) -> Self {
        Self { base, offset: 0, index: None }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Val {
    Int(G),
    Float {
        reg: Vreg,
        size: FpSize,
    },
    Ptr(AddrExpr),
    /// An array or struct held member by member, each in registers of its own.
    Agg(Vec<Val>),
}

impl Val {
    /// The scalars of the value in order, an aggregate's members flattened.
    pub fn leaves(&self) -> Vec<&Val> {
        match self {
            Val::Agg(members) => members.iter().flat_map(Val::leaves).collect(),
            scalar => vec![scalar],
        }
    }

    pub fn leaves_mut(&mut self) -> Vec<&mut Val> {
        match self {
            Val::Agg(members) => members.iter_mut().flat_map(Val::leaves_mut).collect(),
            scalar => vec![scalar],
        }
    }
}
