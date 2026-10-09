//! The bodies a runtime's description gives its routines: each routine a
//! module declares and the description states is defined `available_externally`,
//! for the inliner to price against the call. A copy never reaches the
//! object; a call it does not replace is the runtime's own.

use llrm_mir::BinaryOp;
use llrm_mir::build::Builder;
use llrm_mir::facts::Fact;
use llrm_mir::{CastOp, Flags, GlobalId, IntPredicate, Linkage, Module, Operand, Type, TypeId};

use super::{RUNTIME, convention, mark_cold};
use crate::meaning::{Arithmetic, Descriptor, Expr, Meaning, Parameter, Returns};
use crate::model;

/// Each routine of `promises` that `module` declares, defined by its meaning.
pub(super) fn defined(
    module: &mut Module,
    spaces: &llrm_target::layout::AddressSpaces,
    promises: &model::RuntimePromises,
) {
    let Some(descriptor) = promises.descriptor else { return };
    for meaning in &promises.routines {
        let Some(global) = module.named(&format!("{RUNTIME}{}", meaning.routine)) else { continue };
        if !declares(module, global, meaning) {
            continue;
        }
        let raise = if promises.checked && meaning.check.is_some() { raiser(module, spaces) } else { None };
        define(module, global, meaning, descriptor, raise);
    }
}

/// Whether `global` is a declaration of the type `meaning` gives.
fn declares(
    module: &mut Module,
    global: GlobalId,
    meaning: &Meaning,
) -> bool {
    let Some(function) = module.global(global).function().filter(|one| one.is_declaration()) else { return false };
    let ty = function.ty;
    let (ptr, word) = (module.context.types.ptr(0), module.context.types.int(16));
    let returns = match meaning.result {
        Returns::Value(_) => word,
        Returns::View { .. } => ptr,
    };
    let parameters: Vec<TypeId> =
        meaning.parameters.iter().map(|one| if *one == Parameter::String { ptr } else { word }).collect();
    matches!(
        module.context.types.get(ty),
        Type::Function { returns: r, parameters: p, variadic: false } if *r == returns && *p == parameters
    )
}

/// The runtime's error routine, `B$SERR(number)`, as the module calls it, declared where it is not.
fn raiser(
    module: &mut Module,
    spaces: &llrm_target::layout::AddressSpaces,
) -> Option<(GlobalId, TypeId, u32)> {
    let (conv, space) = convention(spaces, model::StackCleanup::Callee, model::CallDistance::Far, None).ok()?;
    let name = format!("{RUNTIME}B$SERR");
    let global = match module.named(&name) {
        Some(one) => one,
        None => {
            let types = &mut module.context.types;
            let (void, word) = (types.void(), types.int(16));
            let ty = super::function_type(types, void, vec![word]);
            let global = module.add_function(&name, ty, Linkage::External).ok()?;
            super::place_function(module, global, (conv, space));
            global
        }
    };
    Some((global, module.global(global).function()?.ty, conv))
}

fn define(
    module: &mut Module,
    global: GlobalId,
    meaning: &Meaning,
    descriptor: Descriptor,
    raise: Option<(GlobalId, TypeId, u32)>,
) {
    let raise = raise.map(|(routine, ty, conv)| (module.reference(routine), ty, conv));
    let mut builder = module.builder(global);
    let entry = builder.block("entry");
    builder.position(entry);
    let mut body = Body { b: builder, descriptor, parameters: &meaning.parameters };
    if let (Some((callee, ty, conv)), Some((when, number))) = (raise, &meaning.check) {
        let condition = body.condition(when);
        let (fail, go) = (body.b.block("raise"), body.b.block("go"));
        body.b.cond_br(condition, fail, go);
        body.b.position(fail);
        let number = body.b.int(16, i128::from(*number));
        body.b.call_as(conv, ty, Operand::Constant(callee), &[number], "");
        body.b.unreachable();
        mark_cold(body.b.function, fail);
        body.b.position(go);
    }
    let result = match &meaning.result {
        Returns::Value(value) => {
            let value = body.expr(value);
            body.word(value)
        }
        Returns::View { length, data } => {
            let (length, data) = (body.expr(length), body.expr(data));
            let length = body.word(length);
            let bytes = body.b.context.types.int(8);
            let space = body.b.context.types.intern(Type::Array { element: bytes, count: descriptor.size as u64 });
            let made = body.b.alloca(space, "view");
            let at = body.field(made, descriptor.length);
            body.b.store(length, at, false);
            let at = body.field(made, descriptor.data);
            body.b.store(data, at, false);
            made
        }
    };
    body.b.ret(Some(result));
    body.b.function.take_changes();
    for &one in &meaning.releases {
        module.builder(global).function.parameter_attrs[one].push(Fact::Releases.carrier());
    }
    module.globals[global.0 as usize].linkage = Linkage::AvailableExternally;
}

struct Body<'a, 'm> {
    b: Builder<'m>,
    descriptor: Descriptor,
    parameters: &'a [Parameter],
}

impl Body<'_, '_> {
    /// `operand` as a word: a byte zero-extended.
    fn word(
        &mut self,
        operand: Operand,
    ) -> Operand {
        let ty = self.b.type_of(operand);
        if self.b.context.types.int_bits(ty) == Some(8) {
            let word = self.b.context.types.int(16);
            return self.b.cast(CastOp::ZExt, operand, word, "");
        }
        operand
    }

    /// The address `offset` bytes into `pointer`.
    fn field(
        &mut self,
        pointer: Operand,
        offset: i64,
    ) -> Operand {
        let (byte, index) = (self.b.context.types.int(8), self.b.int(16, i128::from(offset)));
        self.b.gep(byte, pointer, &[index], Flags::default(), "")
    }

    /// `expr` as a condition.
    fn condition(
        &mut self,
        expr: &Expr,
    ) -> Operand {
        self.expr(expr)
    }

    fn expr(
        &mut self,
        expr: &Expr,
    ) -> Operand {
        let word = self.b.context.types.int(16);
        match expr {
            Expr::Int(value) => self.b.int(16, i128::from(*value)),
            Expr::Param(at) => {
                debug_assert_eq!(self.parameters[*at], Parameter::Int);
                self.b.parameter(*at)
            }
            Expr::Length(at) => {
                let address = self.field(self.b.parameter(*at), self.descriptor.length);
                self.b.load(word, address, false, "length")
            }
            Expr::Data(at) => {
                let (pointer, address) =
                    (self.b.context.types.ptr(0), self.field(self.b.parameter(*at), self.descriptor.data));
                self.b.load(pointer, address, false, "data")
            }
            Expr::Slot(value) => {
                let value = self.expr(value);
                let value = self.word(value);
                let byte = self.b.context.types.int(8);
                let low = self.b.cast(CastOp::Trunc, value, byte, "");
                let slot = self.b.alloca(byte, "slot");
                self.b.store(low, slot, false);
                slot
            }
            Expr::Byte(address) => {
                let (address, byte) = (self.expr(address), self.b.context.types.int(8));
                self.b.load(byte, address, false, "byte")
            }
            Expr::Binary(op, left, right) => {
                let (left, right) = (self.expr(left), self.expr(right));
                let (left_ty, _) = (self.b.type_of(left), ());
                if matches!(self.b.context.types.get(left_ty), Type::Pointer(_)) {
                    let right = self.word(right);
                    let byte = self.b.context.types.int(8);
                    return self.b.gep(byte, left, &[right], Flags::default(), "");
                }
                let (left, right) = if self.b.context.types.int_bits(left_ty) == Some(1) {
                    (left, right)
                } else {
                    (self.word(left), self.word(right))
                };
                match op {
                    Arithmetic::Add => self.b.binary(BinaryOp::Add, left, right, Flags::default(), ""),
                    Arithmetic::Sub => self.b.binary(BinaryOp::Sub, left, right, Flags::default(), ""),
                    Arithmetic::Or => self.b.binary(BinaryOp::Or, left, right, Flags::default(), ""),
                    Arithmetic::Eq => self.b.icmp(IntPredicate::Eq, left, right, ""),
                    Arithmetic::Lt => self.b.icmp(IntPredicate::Slt, left, right, ""),
                    Arithmetic::Min => {
                        let less = self.b.icmp(IntPredicate::Slt, left, right, "");
                        self.b.select(less, left, right, "")
                    }
                    Arithmetic::Max => {
                        let more = self.b.icmp(IntPredicate::Sgt, left, right, "");
                        self.b.select(more, left, right, "")
                    }
                }
            }
        }
    }
}
