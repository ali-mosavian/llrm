//! LLVM's context: the uniqued types and constants a module's code refers to.

use std::collections::HashMap;

use crate::opcode::CastOp;
use crate::types::{TypeId, Types};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantId(u32);

/// A global value -- variable or function -- of the module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GlobalId(pub u32);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConstantKind {
    /// Bits, modulo the type's width.
    Int(u128),
    /// IEEE bits: a `float`'s in the low 32.
    Float(u64),
    Null,
    Poison,
    /// `zeroinitializer`.
    Zero,
    /// An array, struct or vector, member by member.
    Aggregate(Vec<ConstantId>),
    /// `c"..."`: an `[n x i8]`.
    Bytes(Vec<u8>),
    Global(GlobalId),
    Expr(ConstantExpr),
}

/// The constant expressions MIR keeps, for global initializers only.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConstantExpr {
    GetElementPtr { source: TypeId, inbounds: bool, operands: Vec<ConstantId> },
    Cast { op: CastOp, value: ConstantId },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Constant {
    pub ty: TypeId,
    pub kind: ConstantKind,
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    pub types: Types,
    constants: Vec<Constant>,
    interned: HashMap<Constant, ConstantId>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn constant(&mut self, constant: Constant) -> ConstantId {
        if let Some(&id) = self.interned.get(&constant) {
            return id;
        }
        let id = ConstantId(self.constants.len() as u32);
        self.constants.push(constant.clone());
        self.interned.insert(constant, id);
        id
    }

    pub fn get(&self, id: ConstantId) -> &Constant {
        &self.constants[id.0 as usize]
    }

    /// `value` modulo the width of the integer type `ty`.
    pub fn int(&mut self, ty: TypeId, value: i128) -> ConstantId {
        let bits = self.types.int_bits(ty).expect("an integer constant has an integer type");
        self.constant(Constant { ty, kind: ConstantKind::Int(value as u128 & mask(bits)) })
    }
}

/// The low `bits` bits set.
pub fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `bits` as a two's-complement number `width` bits wide.
pub fn signed(bits: u128, width: u32) -> i128 {
    let shift = 128 - width.min(128);
    ((bits << shift) as i128) >> shift
}
