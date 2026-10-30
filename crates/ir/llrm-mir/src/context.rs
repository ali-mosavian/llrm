//! LLVM's context: the uniqued types and constants a module's code refers to.

use std::cell::RefCell;

use crate::arena::Arena;
use crate::hash::HashMap;
use crate::opcode::{BinaryOp, CastOp};
use crate::types::{TypeId, Types};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantId(u32);

impl ConstantId {
    /// Its place among the interned constants: later ids are made later.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A global value -- variable or function -- of the module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GlobalId(pub u32);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConstantKind {
    /// Bits, modulo the type's width.
    Int(u128),
    /// IEEE bits: a `float`'s in the low 32. An `x86_fp80` whose value a
    /// double holds exactly is held as that double.
    Float(u64),
    /// An `x86_fp80` no double holds exactly: the 80 bits, the sign and 15-bit
    /// exponent above a 64-bit significand with its integer bit.
    Float80(u128),
    Null,
    /// `UndefValue`: any value, chosen per use.
    Undef,
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
    /// `nusw` includes what `inbounds` implies.
    GetElementPtr {
        source: TypeId,
        inbounds: bool,
        nusw: bool,
        nuw: bool,
        operands: Vec<ConstantId>,
    },
    Cast {
        op: CastOp,
        value: ConstantId,
    },
    /// `add`, `sub` and `xor` are the binary operators LLVM still has as
    /// constant expressions (`add (i32 ptrtoint (ptr @a to i32), i32 3)`).
    Binary {
        op: BinaryOp,
        lhs: ConstantId,
        rhs: ConstantId,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Constant {
    pub ty: TypeId,
    pub kind: ConstantKind,
}

#[derive(Debug, Default)]
pub struct Context {
    pub types: Types,
    constants: Arena<Constant>,
    interned: Box<RefCell<HashMap<Constant, ConstantId>>>,
    extensions: Extensions,
}

impl Clone for Context {
    fn clone(&self) -> Self {
        crate::counts::bump(crate::counts::Event::ContextClones);
        Self {
            types: self.types.clone(),
            constants: self.constants.clone(),
            interned: self.interned.clone(),
            extensions: self.extensions.clone(),
        }
    }
}

/// What a layer above MIR keeps in the context, one value per type, dropped
/// with it: LLVMContext's uniqued tables (an analysis's interned objects live
/// and die with the module, not in a global).
#[derive(Clone, Default)]
struct Extensions(std::cell::RefCell<crate::hash::HashMap<std::any::TypeId, std::rc::Rc<dyn std::any::Any>>>);

impl std::fmt::Debug for Extensions {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(formatter, "Extensions({})", self.0.borrow().len())
    }
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    /// This context's `T`, made by `Default` on the first ask. A clone of the
    /// context shares it.
    pub fn extension<T: std::any::Any + Default>(&self) -> std::rc::Rc<T> {
        let mut held = self.extensions.0.borrow_mut();
        let found = held.entry(std::any::TypeId::of::<T>()).or_insert_with(|| std::rc::Rc::new(T::default()));
        std::rc::Rc::clone(found).downcast::<T>().unwrap_or_else(|_| unreachable!("keyed by its type"))
    }

    /// LLVM's `ConstantInt::get` and kin: `constant`, made on the first ask.
    /// The table is borrowed for the lookup and again for the insert, never
    /// across a call.
    #[inline]
    pub fn constant(
        &self,
        constant: Constant,
    ) -> ConstantId {
        let found = self.interned.borrow().get(&constant).copied();
        if let Some(id) = found {
            return id;
        }
        let id = ConstantId(self.constants.push(constant.clone()) as u32);
        self.interned.borrow_mut().insert(constant, id);
        id
    }

    #[inline]
    pub fn get(
        &self,
        id: ConstantId,
    ) -> &Constant {
        self.constants.get(id.0 as usize)
    }

    /// `constant` of `from`, interned here: none for one that names a
    /// global or a type only `from` defines.
    pub fn imported(
        &self,
        from: &Context,
        constant: ConstantId,
    ) -> Option<ConstantId> {
        let Constant { ty, kind } = from.get(constant).clone();
        let kind = match kind {
            ConstantKind::Global(_) | ConstantKind::Expr(_) => return None,
            ConstantKind::Aggregate(members) => {
                ConstantKind::Aggregate(members.iter().map(|&one| self.imported(from, one)).collect::<Option<_>>()?)
            }
            other => other,
        };
        let ty = self.types.imported(&from.types, ty)?;
        Some(self.constant(Constant { ty, kind }))
    }

    /// Each global's constant naming the global `renumbered` gives, or
    /// poison where it gives none: a global gone, as LLVM's RAUW with
    /// poison before erasing it.
    pub fn renumber_globals(
        &mut self,
        renumbered: &dyn Fn(GlobalId) -> Option<GlobalId>,
    ) {
        for constant in self.constants.iter_mut() {
            if let ConstantKind::Global(global) = constant.kind {
                constant.kind = renumbered(global).map_or(ConstantKind::Poison, ConstantKind::Global);
            }
        }
        let interned =
            self.constants.iter().enumerate().rev().map(|(at, one)| (one.clone(), ConstantId(at as u32))).collect();
        self.interned = Box::new(RefCell::new(interned));
    }

    /// `value` modulo the width of the integer type `ty`.
    #[inline]
    pub fn int(
        &self,
        ty: TypeId,
        value: i128,
    ) -> ConstantId {
        let bits = self.types.int_bits(ty).expect("an integer constant has an integer type");
        self.constant(Constant { ty, kind: ConstantKind::Int(value as u128 & mask(bits)) })
    }
}

/// The low `bits` bits set.
pub fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `bits` as a two's-complement number `width` bits wide.
pub fn signed(
    bits: u128,
    width: u32,
) -> i128 {
    let shift = 128 - width.min(128);
    ((bits << shift) as i128) >> shift
}
