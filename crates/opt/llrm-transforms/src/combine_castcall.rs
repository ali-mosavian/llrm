//! InstCombine's call through a mismatched prototype: a call whose function
//! type is not the callee's, to a function defined here, takes the callee's
//! type. Translated from `InstCombinerImpl::transformConstExprCastCall`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L5313)).
//!
//! # How it works
//! K&R `int none() {..}` called as `none(t)` is `call i16 @none(ptr byval(%S)
//! %t)`: the front end passes what the call site wrote. The callee takes
//! nothing, so the arguments it does not declare are dropped (the struct copy
//! feeding the `byval` goes dead with them), and a parameter the call lacks is
//! passed as null.
//!
//! # Example
//! ```text
//! call i16 (ptr) @none(ptr byval(%S) %tmp)   ; define i16 @none()
//!   ==>  call i16 @none()
//! ```
//!
//! MIRROR: the guards (a definition, no `thunk` or `naked` callee, no
//! `musttail` call, no `inalloca`/`preallocated` callee, an argument type for
//! each common parameter, `byval` agreeing on both sides, no `sret` among the
//! dropped arguments of a variadic callee), dropping extra arguments, null for
//! the missing ones, the call's calling convention and tail kind kept.
//!
//! DEVIATE 1: the return type must be the callee's already (no cast of the
//! result), and a common argument's type must be the parameter's (no cast of
//! the argument): the pin bit-casts where the types are castable. DEVIATE 2:
//! calls only, not invokes (the combiner visits `Call`). DEVIATE 3: a variadic
//! callee keeps its extra arguments unpromoted, so one that would be promoted
//! (an integer narrower than `i32`) refuses.
//!
//! # Used by
//! `combine_memintrinsics` (`visit_call`).

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::InstId;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{Attribute, Opcode, Tail};
use llrm_mir::types::Type;

use crate::combine::{Combiner, Visit};

impl Combiner<'_> {
    /// `transformConstExprCastCall`.
    pub(crate) fn transform_const_expr_cast_call(
        &mut self,
        call: InstId,
    ) -> Visit {
        let Opcode::Call(info) = &self.function.instruction(call).opcode else { return Visit::None };
        // The bail-outs below are the common path: the call's info is cloned
        // only once one of them has not been taken.
        let (tail, function_type) = (info.tail, info.function_type);
        let Some(global) = llrm_mir::memory::callee(self.context, self.function, call) else { return Visit::None };
        let Some(callee) = self.env.globals.get(global.0 as usize).and_then(|global| global.function()) else {
            return Visit::None;
        };
        // a declaration may not be accurate: `void @foo()` is the usual
        // placeholder for an unknown prototype
        if !self.env.callees.get(&global).is_some_and(|summary| summary.defined) || tail == Tail::MustTail {
            return Visit::None;
        }
        let named = |attrs: &[Attribute], name: &str| {
            attrs.iter().any(|attr| match attr {
                Attribute::Flag(one) | Attribute::Str(one, _) | Attribute::Int(one, _) | Attribute::Type(one, _) => {
                    one == name
                }
                _ => false,
            })
        };
        if named(&callee.attrs, "thunk") || named(&callee.attrs, "naked") {
            return Visit::None;
        }
        if callee.ty == function_type {
            return Visit::None;
        }
        let (Type::Function { returns: callee_returns, parameters, variadic }, Type::Function { returns, .. }) =
            (self.context.types.get(callee.ty).clone(), self.context.types.get(function_type).clone())
        else {
            return Visit::None;
        };
        if callee_returns != returns {
            return Visit::None;
        }
        let Opcode::Call(info) = &self.function.instruction(call).opcode else { return Visit::None };
        let info = info.clone();
        // `takes_i32_inalloca` must not lose its pointer, nor a byval call its
        // copy
        let somewhere = |name: &str| {
            callee.parameter_attrs.iter().any(|attrs| named(attrs, name)) || named(&callee.return_attrs, name)
        };
        if somewhere("inalloca") || somewhere("preallocated") {
            return Visit::None;
        }
        let operands = self.function.instruction(call).operands.clone();
        let actual = operands.len() - 1;
        let common = parameters.len().min(actual);
        for index in 0..common {
            let attrs = info.argument_attrs.get(index).map_or(&[][..], Vec::as_slice);
            if self.type_of(operands[index]) != parameters[index]
                || named(attrs, "inalloca")
                || named(attrs, "preallocated")
                || named(attrs, "swifterror")
                || named(attrs, "byval") != callee.parameter_attrs.get(index).is_some_and(|attrs| named(attrs, "byval"))
            {
                return Visit::None;
            }
        }
        if variadic && parameters.len() < actual {
            // extra arguments of a variadic callee stay, and must already be
            // what `va_arg` reads
            for &operand in &operands[parameters.len()..actual] {
                let ty = self.type_of(operand);
                if self.context.types.int_bits(ty).is_some_and(|width| width < 32) {
                    return Visit::None;
                }
            }
            if info
                .argument_attrs
                .iter()
                .enumerate()
                .any(|(index, attrs)| index >= parameters.len() && named(attrs, "sret"))
            {
                return Visit::None;
            }
        }
        let mut new_operands = operands[..common].to_vec();
        let mut argument_attrs: Vec<Vec<Attribute>> =
            (0..common).map(|index| info.argument_attrs.get(index).cloned().unwrap_or_default()).collect();
        for &ty in &parameters[common..] {
            let kind = match self.context.types.get(ty) {
                Type::Pointer(_) => ConstantKind::Null,
                Type::Int(_) => ConstantKind::Int(0),
                Type::Float(_) => ConstantKind::Float(0),
                _ => ConstantKind::Zero,
            };
            new_operands.push(Operand::Constant(self.context.constant(Constant { ty, kind })));
            argument_attrs.push(Vec::new());
        }
        if variadic {
            for index in parameters.len()..actual {
                new_operands.push(operands[index]);
                argument_attrs.push(info.argument_attrs.get(index).cloned().unwrap_or_default());
            }
        }
        new_operands.push(*operands.last().expect("a callee"));
        let mut new_info = (*info).clone();
        new_info.function_type = callee.ty;
        new_info.argument_attrs = argument_attrs;
        self.function.set_operands(call, new_operands);
        self.function.set_opcode(call, Opcode::Call(Box::new(new_info)));
        for operand in operands {
            if let Operand::Value(value) = operand
                && let llrm_mir::module::ValueDef::Instruction(inst) = self.function.value(value).def
            {
                self.add_to_worklist(inst);
            }
        }
        Visit::Changed
    }
}

#[cfg(test)]
#[path = "combine_castcall_tests.rs"]
mod tests;
