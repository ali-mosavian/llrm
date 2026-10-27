//! What the MIR expansions of an intrinsic share: finding its calls, and the
//! instructions made before one.
//!
//! # How it works
//! [`expand_calls`] walks every function of a module and hands each call of an
//! intrinsic the caller picks to a rewrite; an [`Emitter`] places the
//! instructions the rewrite makes right before the call, in one integer type.
//! The bit counts, `bswap` and the three-way compares are each a rewrite.
//!
//! # Example
//! A `call i32 @llvm.bswap.i32(i32 %x)` is replaced, in front of the call, by
//! the shifts and masks that move each byte, and the call's uses read the
//! result.
//!
//! # Runs
//! Under codegen's `bitcounts`, `bytes` and `compares`, from
//! `driver::optimized`.
//!
//! # See also
//! LLVM's `TargetLowering::expand*` family, which these follow
//! ([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)).

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, GlobalKind, GlobalValue, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::types::TypeId;

/// Every call in `module` of an intrinsic `pick` returns something for, given
/// to `rewrite` with what `pick` returned.
pub fn expand_calls<T>(
    module: &mut Module,
    pick: impl Fn(Intrinsic) -> Option<T>,
    mut rewrite: impl FnMut(&mut Context, &mut Function, InstId, T) -> Result<(), String>,
) -> Result<(), String> {
    let intrinsics: Vec<Option<Intrinsic>> = module.globals.iter().map(GlobalValue::intrinsic).collect();
    let context = &mut module.context;
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        let found: Vec<(InstId, T)> = function
            .walk()
            .filter_map(|(_, inst)| {
                let callee = llrm_mir::memory::callee(context, function, inst)?;
                Some((inst, pick(intrinsics[callee.0 as usize]?)?))
            })
            .collect();
        for (inst, what) in found {
            rewrite(context, function, inst, what)?;
        }
    }
    Ok(())
}

/// The instructions of one expansion, made before the call they replace.
pub struct Emitter<'a> {
    pub context: &'a Context,
    pub function: &'a mut Function,
    pub at: InstId,
    /// The type the expansion computes in, `bits` wide.
    pub ty: TypeId,
    pub bits: u32,
}

impl Emitter<'_> {
    pub fn emit(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
    ) -> Result<Operand, String> {
        let made = self.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.function.insert(made, Position::Before(self.at))?;
        Ok(Operand::Value(self.function.instruction(made).result.expect("a value")))
    }

    pub fn binary(
        &mut self,
        op: BinaryOp,
        a: Operand,
        b: Operand,
    ) -> Result<Operand, String> {
        self.emit(Opcode::Binary(op), self.ty, vec![a, b])
    }

    /// `value` in the computing type.
    pub fn constant(
        &self,
        value: u128,
    ) -> Operand {
        Operand::Constant(self.context.int(self.ty, value as i128))
    }

    pub fn shift_right(
        &mut self,
        x: Operand,
        by: u32,
    ) -> Result<Operand, String> {
        let by = self.constant(u128::from(by));
        self.binary(BinaryOp::LShr, x, by)
    }

    pub fn shift_right_arithmetic(
        &mut self,
        x: Operand,
        by: u32,
    ) -> Result<Operand, String> {
        let by = self.constant(u128::from(by));
        self.binary(BinaryOp::AShr, x, by)
    }

    pub fn shift_left(
        &mut self,
        x: Operand,
        by: u32,
    ) -> Result<Operand, String> {
        let by = self.constant(u128::from(by));
        self.binary(BinaryOp::Shl, x, by)
    }

    pub fn mask(
        &mut self,
        x: Operand,
        bits: u128,
    ) -> Result<Operand, String> {
        let bits = self.constant(bits);
        self.binary(BinaryOp::And, x, bits)
    }
}
