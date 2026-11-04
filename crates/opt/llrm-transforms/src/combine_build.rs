//! `InstCombiner::Builder`: the `IRBuilder` over `InstSimplifyFolder` that
//! InstCombine's `visit*` functions build new instructions with.
//!
//! Translations at d1106deb71cc:
//! [`InstSimplifyFolder`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/InstSimplifyFolder.h#L25),
//! whose `Fold*` return `simplify*` of the operands or nothing, and the
//! `Create*` of
//! [`IRBuilder`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/IRBuilder.h)
//! that ask the folder first and otherwise insert (`InsertNewInstWith`, which
//! queues the instruction on the worklist, is `Combiner::insert_before`).
//!
//! MIRROR: a `create_*` returns the value `simplify*` finds, else a new
//! instruction before `at`. The folder's query has no dominator tree, no
//! assumptions and no context instruction (`InstSimplifyFolder(DL)`), as here.
//!
//! DEVIATE 1: no instruction names (`Twine`): MIR names are not read by any
//! pass.
//!
//! DEVIATE 2: `CreateIntrinsic` ([`Combiner::create_intrinsic`]) builds a call
//! to the declaration the pass makes through `Unit::declared`
//! (`Intrinsic::getOrInsertDeclaration`); the declaration reaches the module
//! after the pass. It is for intrinsics overloaded on one integer type, named
//! `llvm.<base>.i<width>`, with the argument types as the parameters.
//!
//! # How it works
//! Each `create_*` asks `simplify*` of its operands first (the folder) and
//! inserts a new instruction only when nothing simpler exists; the instruction
//! is queued on the worklist. `replace_with_new` is what `visit*` returns for
//! a `BinaryOperator::Create`: insert, replace the visited instruction, erase.
//!
//! # Used by
//! `combine_addsub` and the families after it.
//!
//! # Example
//! `create_binop(at, Add, x, 0)` is `x`, found by `simplifyAddInst`;
//! `create_binop(at, Add, x, y)` is a new `add` before `at`.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode, Tail};
use llrm_mir::simplify::{
    simplify_binop_operands, simplify_cast_operand, simplify_icmp_operands, simplify_select_operands,
};
use llrm_mir::types::{Type, TypeId};

use crate::combine::{Combiner, Visit};

impl Combiner<'_> {
    /// The type of `v`.
    pub fn type_of(
        &self,
        v: Operand,
    ) -> TypeId {
        self.function.operand_type(self.context, v).expect("a value or a constant")
    }

    /// `ConstantInt::get(Ty, Bits)`: `bits` modulo the width of `ty`.
    pub fn int(
        &self,
        ty: TypeId,
        bits: u128,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, bits as i128))
    }

    /// `Constant::getAllOnesValue`.
    pub fn all_ones(
        &self,
        ty: TypeId,
    ) -> Operand {
        self.int(ty, u128::MAX)
    }

    /// The value of the instruction `inst` made.
    pub(crate) fn value_of(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    /// `Builder.CreateBinOp(op, a, b)` with `flags` (`CreateNUWAdd` and the
    /// like): what `simplifyBinOp` finds, else a new instruction before `at`.
    pub fn create_binop(
        &mut self,
        at: InstId,
        op: BinaryOp,
        a: Operand,
        b: Operand,
        flags: Flags,
    ) -> Operand {
        if let Some(found) =
            simplify_binop_operands(self.context, self.env.layout, self.function, op, Flags::default(), a, b)
        {
            return found;
        }
        let ty = self.type_of(a);
        let new = self.insert_before(at, Opcode::Binary(op), ty, vec![a, b], flags);
        self.value_of(new)
    }

    /// `Builder.CreateIntrinsic(ty, ID, args)` before `at`: a call of
    /// `llvm.<base>.i<width of ty>` returning `ty`, its declaration made
    /// where the module has none.
    pub fn create_intrinsic(
        &mut self,
        at: InstId,
        base: &str,
        ty: TypeId,
        args: &[Operand],
    ) -> Operand {
        let width = self.context.types.int_bits(ty).expect("an integer");
        self.create_intrinsic_named(at, &format!("llvm.{base}.i{width}"), ty, args)
    }

    /// `Builder.CreateIntrinsic` of an intrinsic overloaded on more than one
    /// type (`llvm.scmp.i8.i16`): the call of the declaration `name`.
    pub fn create_intrinsic_named(
        &mut self,
        at: InstId,
        name: &str,
        ty: TypeId,
        args: &[Operand],
    ) -> Operand {
        let parameters: Vec<TypeId> = args.iter().map(|&arg| self.type_of(arg)).collect();
        let function_type = self.context.types.intern(Type::Function { returns: ty, parameters, variadic: false });
        let callee = self.declared.declare(name, function_type);
        let pointer = self.context.types.ptr(0);
        let callee =
            Operand::Constant(self.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); args.len()],
            attrs: Vec::new(),
            tail: Tail::default(),
        };
        let mut operands = args.to_vec();
        operands.push(callee);
        let new = self.insert_before(at, Opcode::Call(Box::new(info)), ty, operands, Flags::default());
        self.value_of(new)
    }

    /// `Builder.CreateIntrinsic(umul/smul.with.overflow, {a, b})`: the call,
    /// of the type `{ iN, i1 }`.
    pub fn create_mul_with_overflow(
        &mut self,
        at: InstId,
        signed: bool,
        a: Operand,
        b: Operand,
    ) -> Operand {
        let ty = self.type_of(a);
        let i1 = self.context.types.int(1);
        let pair = self.context.types.intern(Type::Struct { fields: vec![ty, i1], packed: false });
        let parameters = vec![ty, ty];
        let function_type = self.context.types.intern(Type::Function { returns: pair, parameters, variadic: false });
        let width = self.context.types.int_bits(ty).expect("an integer");
        let base = if signed { "smul" } else { "umul" };
        let callee = self.declared.declare(&format!("llvm.{base}.with.overflow.i{width}"), function_type);
        let pointer = self.context.types.ptr(0);
        let callee =
            Operand::Constant(self.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 2],
            attrs: Vec::new(),
            tail: Tail::default(),
        };
        let new = self.insert_before(at, Opcode::Call(Box::new(info)), pair, vec![a, b, callee], Flags::default());
        self.value_of(new)
    }

    /// `Builder.CreateExtractValue(aggregate, {index})` of a member of type
    /// `ty`.
    pub fn create_extract_value(
        &mut self,
        at: InstId,
        aggregate: Operand,
        index: u32,
        ty: TypeId,
    ) -> Operand {
        let new = self.insert_before(at, Opcode::ExtractValue(vec![index]), ty, vec![aggregate], Flags::default());
        self.value_of(new)
    }

    /// `Builder.CreateAdd(a, b)`.
    pub fn create_add(
        &mut self,
        at: InstId,
        a: Operand,
        b: Operand,
    ) -> Operand {
        self.create_binop(at, BinaryOp::Add, a, b, Flags::default())
    }

    /// `Builder.CreateSub(a, b)`.
    pub fn create_sub(
        &mut self,
        at: InstId,
        a: Operand,
        b: Operand,
    ) -> Operand {
        self.create_binop(at, BinaryOp::Sub, a, b, Flags::default())
    }

    /// `Builder.CreateShl(a, b)`.
    pub fn create_shl(
        &mut self,
        at: InstId,
        a: Operand,
        b: Operand,
    ) -> Operand {
        self.create_binop(at, BinaryOp::Shl, a, b, Flags::default())
    }

    /// `Builder.CreateNot(v)`: `xor v, -1`.
    pub fn create_not(
        &mut self,
        at: InstId,
        v: Operand,
    ) -> Operand {
        let ones = self.all_ones(self.type_of(v));
        self.create_binop(at, BinaryOp::Xor, v, ones, Flags::default())
    }

    /// `Builder.CreateNeg(v)`: `sub 0, v`.
    pub fn create_neg(
        &mut self,
        at: InstId,
        v: Operand,
    ) -> Operand {
        let zero = self.int(self.type_of(v), 0);
        self.create_binop(at, BinaryOp::Sub, zero, v, Flags::default())
    }

    /// `Builder.CreateCast(op, v, ty)`.
    pub fn create_cast(
        &mut self,
        at: InstId,
        op: CastOp,
        v: Operand,
        ty: TypeId,
        flags: Flags,
    ) -> Operand {
        if let Some(found) = simplify_cast_operand(self.context, self.env.layout, self.function, op, v, ty) {
            return found;
        }
        let new = self.insert_before(at, Opcode::Cast(op), ty, vec![v], flags);
        self.value_of(new)
    }

    /// `Builder.CreateICmp(pred, a, b)`.
    pub fn create_icmp(
        &mut self,
        at: InstId,
        pred: IntPredicate,
        a: Operand,
        b: Operand,
    ) -> Operand {
        if let Some(found) = simplify_icmp_operands(self.context, self.env.layout, self.function, pred, a, b) {
            return found;
        }
        let ty = self.context.types.int(1);
        let new = self.insert_before(at, Opcode::ICmp(pred), ty, vec![a, b], Flags::default());
        self.value_of(new)
    }

    /// `Builder.CreateSelect(cond, if_true, if_false)`.
    pub fn create_select(
        &mut self,
        at: InstId,
        cond: Operand,
        if_true: Operand,
        if_false: Operand,
    ) -> Operand {
        if let Some(found) =
            simplify_select_operands(self.context, self.env.layout, self.function, cond, if_true, if_false)
        {
            return found;
        }
        let ty = self.type_of(if_true);
        let new = self.insert_before(at, Opcode::Select, ty, vec![cond, if_true, if_false], Flags::default());
        self.value_of(new)
    }

    /// What a `visit*` returns for `BinaryOperator::Create(op, a, b)` (or any
    /// new instruction): InstCombine inserts it before `at`, replaces `at` by
    /// it and erases `at`.
    pub fn replace_with_new(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> Visit {
        let new = self.insert_before(at, opcode, ty, operands, flags);
        Visit::Replace(self.value_of(new))
    }

    /// `replace_with_new` of a binary operator.
    pub fn replace_with_binop(
        &mut self,
        at: InstId,
        op: BinaryOp,
        a: Operand,
        b: Operand,
        flags: Flags,
    ) -> Visit {
        let ty = self.type_of(a);
        self.replace_with_new(at, Opcode::Binary(op), ty, vec![a, b], flags)
    }
}

#[cfg(test)]
#[path = "combine_build_tests.rs"]
mod tests;
