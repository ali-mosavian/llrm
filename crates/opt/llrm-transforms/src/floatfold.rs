//! Replaces floating values [`llrm_analysis::floatfacts`] proves exact by
//! their constants, and applies the algebraic freedoms an operation's
//! fast-math flags give.
//!
//! # How it works
//! `floatfacts` proves a result exactly representable in its format, so the
//! constant is right under any rounding mode. Three steps run in order over one
//! solve (the manager's, where it was made for the same call writes):
//!
//! 1. `discarded`: an exact float-to-integer conversion (`fptosi`,
//!    `llvm.lrint`, ...) has a known integer; every read of it reads that
//!    number.
//! 2. `stored`: every read of an exact float value, a store's included, reads
//!    its constant. Whatever stops being read is [`crate::dead`]'s.
//! 3. `freedoms`: rewrites that need no analysis, only the flags (`Facts`).
//!
//! ```text
//!   %a = fadd double 1.5, 2.0          %a = fadd double 1.5, 2.0   (dead)
//!   store double %a, ptr @x     ==>    store double 3.5, ptr @x
//!   %b = load double, ptr @x           %b = load double, ptr @x    (3.5 too)
//! ```
//!
//! The freedoms, per operation: `x + -0.0`, `x - 0.0` and `x * 1.0` are `x`;
//! `x + 0.0` is `x` with `nsz`; `x * 0` is 0 with `nnan` and `nsz`; `x - x` is
//! 0 and `x / x` is 1 with `nnan` and `ninf`. A constant moves to the right of
//! a sum or product. A division by a constant is a multiply by its reciprocal
//! with `arcp`, or without it when the reciprocal is exact (a normal power of
//! two whose reciprocal is normal). `(x op c1) op c2` is `x op (c1 op c2)` for
//! a sum or product when both carry `reassoc`; the new flags are the
//! intersection.
//!
//! # Example
//! Before:
//!
//! ```mir
//! @g = global float 0.0
//!
//! define i32 @f() {
//! b0:
//!   store float 1.440000e+02, ptr @g
//!   %x = load float, ptr @g
//!   %i = fptosi float %x to i32
//!   ret i32 %i
//! }
//! ```
//!
//! After, with the dead load and conversion removed:
//!
//! ```mir
//! define i32 @f() {
//! b0:
//!   store float 1.440000e+02, ptr @g
//!   ret i32 144
//! }
//! ```
//!
//! The exact conversion of the stored 144.0 reads as 144.
//!
//! From unit test `test_an_exact_conversion_reads_as_its_number` in
//! `floatfold_tests.rs`.
//!
//! # Runs
//! Inside [`crate::fold`], every time fold runs, which is the pass list's
//! second entry and after the header copy; there is no pass-list entry of its
//! own. On wherever `fold` is (-O1 and up).
//!
//! # Needs
//! [`llrm_analysis::floatfacts`] through `manager::FloatFacts`, and
//! `manager::writes` for what calls write.
//!
//! # See also
//! LLVM's `InstructionSimplify` and `InstCombine` floating rules.

use llrm_analysis::consts::{Calls, Known};
use llrm_analysis::floatfacts::{self, Finite, Format};
use llrm_analysis::manager;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::facts::Facts;
use llrm_mir::interpret::{self, Val};
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};
use llrm_mir::types::{FloatKind, Type, TypeId};
use llrm_support::hash::IndexMap;

pub struct FloatFold;

impl FunctionPass for FloatFold {
    fn name(&self) -> &'static str {
        "floatfold"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let calls = manager::writes(unit.context, unit.layout, unit.function, analyses);
        if _folded(unit.context, unit.layout, unit.function, analyses, &calls) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `discarded`, then `stored`, with what floatfacts knows of `function`;
/// `outer` is its module and target, `calls` what each call writes.
/// Whether anything changed.
pub fn folded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    calls: &Calls,
) -> bool {
    _folded(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())), calls)
}

/// `folded`, `analyses` holding what is known of `function`.
pub(crate) fn _folded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    calls: &Calls,
) -> bool {
    // With no float in the body there is nothing to fold: neither the solve
    // nor the facts under it are asked for (d_faces -O2: 28 of 48 float
    // solves, a third of what `fold` costs there).
    if !floatfacts::touches(context, function) {
        return false;
    }
    let (facts, conversions) = {
        // The manager's solve, where it was of these writes.
        let solved = (*calls == manager::writes(context, layout, function, analyses))
            .then(|| analyses.get::<manager::FloatFacts>(context, layout, function));
        let held = manager::Held::of(context, layout, function, analyses, true);
        let unit = held.unit(context, layout, function, analyses.outer());
        let facts = match solved {
            Some(solved) => solved.facts.clone(),
            None => floatfacts::known(&unit, calls),
        };
        let conversions = floatfacts::converted(&unit, calls, Some(&facts));
        (facts, conversions)
    };
    discarded(context, function, &conversions) | stored(context, function, &facts) | freedoms(context, function)
}

/// A floating constant's value and format.
pub(crate) fn float(
    context: &Context,
    operand: Operand,
) -> Option<(FloatKind, f64)> {
    let Operand::Constant(id) = operand else { return None };
    let constant = context.get(id);
    match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Float(bits), Type::Float(FloatKind::Float)) => {
            Some((FloatKind::Float, f64::from(f32::from_bits(*bits as u32))))
        }
        (ConstantKind::Float(bits), Type::Float(kind)) => Some((*kind, f64::from_bits(*bits))),
        _ => None,
    }
}

/// A floating constant of `ty`.
pub(crate) fn float_constant(
    context: &Context,
    ty: TypeId,
    kind: FloatKind,
    number: f64,
) -> Operand {
    let bits = if kind == FloatKind::Float { u64::from((number as f32).to_bits()) } else { number.to_bits() };
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Float(bits) }))
}

/// What the language lets a pass do to a floating operation (`Facts` of its
/// flags): LLVM's InstSimplify and InstCombine floating rules. A division by
/// a constant is a multiply by its reciprocal (`arcp`); two constants of a
/// chain of sums, or of products, are one (`reassoc` of both); `x * 0` is 0
/// (`nnan`, `nsz`), `x - x` is 0 and `x / x` is 1 (`nnan`, `ninf`), `x + 0.0`
/// is `x` (`nsz`). `x + -0.0`, `x - 0.0` and `x * 1.0` need no flag. Whether
/// anything changed.
pub fn freedoms(
    context: &Context,
    function: &mut Function,
) -> bool {
    let mut changed = false;
    for inst in function.walk().map(|(_, inst)| inst).collect::<Vec<_>>() {
        if function.is_erased(inst) {
            continue;
        }
        if let Some(simpler) = _simplified(context, function, inst) {
            let result = function.instruction(inst).result.expect("an operation's value");
            function.replace_all_uses_with(result, simpler);
            function.erase(inst).expect("its uses were replaced");
            changed = true;
        } else {
            changed |= _combined(context, function, inst);
        }
    }
    changed
}

/// The operand `inst` equals, or a number it is, as its flags let it.
fn _simplified(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    let instruction = function.instruction(inst);
    // A constant widened is the wider constant, exactly: a long double literal
    // came as the double's `fpext`, and its division by `-8.0L` never saw a
    // constant divisor (no multiply by the reciprocal).
    if let (Opcode::Cast(CastOp::FPExt), Some(&source), Type::Float(kind)) =
        (&instruction.opcode, instruction.operands.first(), context.types.get(instruction.ty))
    {
        if let Some((_, number)) = float(context, source) {
            return Some(float_constant(context, instruction.ty, *kind, number));
        }
    }
    let Opcode::Binary(op @ (BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)) = instruction.opcode
    else {
        return None;
    };
    let (a, b, ty) = (instruction.operands[0], instruction.operands[1], instruction.ty);
    let facts = Facts::of_flags(instruction.flags);
    let right = float(context, b);
    let Type::Float(kind) = *context.types.get(ty) else { return None };
    let (zero, negative_zero) = (
        |number: f64| number == 0.0 && number.is_sign_positive(),
        |number: f64| number == 0.0 && number.is_sign_negative(),
    );
    match (op, right) {
        // x + -0.0 and x - +0.0 are x; the other zero turns -0.0 into +0.0.
        (BinaryOp::FAdd, Some((_, number))) if negative_zero(number) || (zero(number) && facts.no_signed_zeros()) => {
            Some(a)
        }
        (BinaryOp::FSub, Some((_, number))) if zero(number) || (negative_zero(number) && facts.no_signed_zeros()) => {
            Some(a)
        }
        (BinaryOp::FMul | BinaryOp::FDiv, Some((_, number))) if number == 1.0 => Some(a),
        // 0 * x is NaN for a NaN or an infinity, and -0 for a negative x.
        (BinaryOp::FMul, Some((_, number))) if number == 0.0 && facts.no_nans() && facts.no_signed_zeros() => {
            Some(float_constant(context, ty, kind, 0.0))
        }
        (BinaryOp::FSub, _) if a == b && facts.no_nans() && facts.no_infs() => {
            Some(float_constant(context, ty, kind, 0.0))
        }
        (BinaryOp::FDiv, _) if a == b && facts.no_nans() && facts.no_infs() => {
            Some(float_constant(context, ty, kind, 1.0))
        }
        _ => None,
    }
}

/// Whether `1 / divisor` is exact in `kind`: a normal power of two whose
/// reciprocal is normal too.
fn exact_reciprocal(
    kind: FloatKind,
    divisor: f64,
) -> bool {
    match kind {
        FloatKind::Float => {
            let divisor = divisor as f32;
            divisor.is_normal() && divisor.to_bits() & 0x7f_ffff == 0 && (1.0 / divisor).is_normal()
        }
        FloatKind::Double | FloatKind::X86Fp80 => {
            divisor.is_normal() && divisor.to_bits() & 0xf_ffff_ffff_ffff == 0 && (1.0 / divisor).is_normal()
        }
    }
}

/// `inst` rewritten where a flag lets the language's freedom be used.
fn _combined(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst).clone();
    let ty = instruction.ty;
    let facts = Facts::of_flags(instruction.flags);
    match instruction.opcode {
        // A constant goes to the right of a sum or a product, which commute.
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FMul)
            if float(context, instruction.operands[0]).is_some()
                && float(context, instruction.operands[1]).is_none() =>
        {
            function.set_operand(inst, 0, instruction.operands[1]);
            function.set_operand(inst, 1, instruction.operands[0]);
            true
        }
        // A division by a constant is a multiply by its reciprocal, as `arcp`
        // lets, or whenever the reciprocal is exact (LLVM's InstCombine,
        // GCC's `fold_binary` for `RDIV_EXPR`): the product rounds as the
        // quotient did.
        Opcode::Binary(BinaryOp::FDiv) => {
            let Some((kind, divisor)) = float(context, instruction.operands[1]).filter(|(kind, divisor)| {
                divisor.is_finite()
                    && *divisor != 0.0
                    && (facts.allow_reciprocal() || exact_reciprocal(*kind, *divisor))
            }) else {
                return false;
            };
            let reciprocal = float_constant(context, ty, kind, 1.0 / divisor);
            let multiply = function.create_instruction(
                Opcode::Binary(BinaryOp::FMul),
                ty,
                vec![instruction.operands[0], reciprocal],
                instruction.flags,
                None,
            );
            _replaced(function, inst, multiply)
        }
        // (x op c1) op c2 is x op (c1 op c2), where sums and products may
        // regroup.
        Opcode::Binary(op @ (BinaryOp::FAdd | BinaryOp::FMul))
            if facts.reassoc() && float(context, instruction.operands[1]).is_some() =>
        {
            let Operand::Value(inner) = instruction.operands[0] else { return false };
            let ValueDef::Instruction(def) = function.value(inner).def else { return false };
            let inner = function.instruction(def).clone();
            if inner.opcode != Opcode::Binary(op)
                || float(context, inner.operands[1]).is_none()
                || !Facts::of_flags(inner.flags).reassoc()
            {
                return false;
            }
            let (Some(c1), Some(c2)) = (_value(context, inner.operands[1]), _value(context, instruction.operands[1]))
            else {
                return false;
            };
            let Ok(Val::Float(_, bits)) = interpret::binary(op, instruction.flags, c1, c2) else { return false };
            let folded = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Float(bits) }));
            function.set_operand(inst, 0, inner.operands[0]);
            function.set_operand(inst, 1, folded);
            function.set_flags(inst, instruction.flags.intersect(inner.flags));
            true
        }
        _ => false,
    }
}

/// A floating constant as the interpreter holds values.
fn _value(
    context: &Context,
    operand: Operand,
) -> Option<Val> {
    let Operand::Constant(id) = operand else { return None };
    let constant = context.get(id);
    match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Float(bits), Type::Float(kind)) => Some(Val::Float(*kind, *bits)),
        _ => None,
    }
}

/// `inst` replaced by `new`, placed where it was.
fn _replaced(
    function: &mut Function,
    inst: InstId,
    new: InstId,
) -> bool {
    function.insert(new, Position::Before(inst)).expect("a placed instruction");
    let (old, value) =
        (function.instruction(inst).result.expect("a value"), function.instruction(new).result.expect("a value"));
    function.replace_all_uses_with(old, Operand::Value(value));
    function.erase(inst).expect("its uses were replaced");
    true
}

/// Every read of a known float value reads its constant.
pub fn stored(
    context: &Context,
    function: &mut Function,
    facts: &IndexMap<ValueId, Finite>,
) -> bool {
    let mut changed = false;
    for (&value, fact) in facts {
        let ty = function.value(value).ty;
        if function.users(value).is_empty() {
            continue;
        }
        let format = Format::of(&context.types, ty).expect("a float");
        let bits = floatfacts::encoded(fact, format).expect("a fact fits its format");
        let constant =
            context.constant(Constant { ty, kind: ConstantKind::Float(u64::try_from(bits).expect("64 bits")) });
        function.replace_all_uses_with(value, Operand::Constant(constant));
        changed = true;
    }
    changed
}

/// Every read of a known conversion result reads its number.
pub fn discarded(
    context: &Context,
    function: &mut Function,
    converted: &IndexMap<ValueId, Known>,
) -> bool {
    let mut changed = false;
    for (&value, fact) in converted {
        if function.users(value).is_empty() {
            continue;
        }
        let bits = u128::try_from(&fact.n).expect("a masked number") as i128;
        let constant = context.int(function.value(value).ty, bits);
        function.replace_all_uses_with(value, Operand::Constant(constant));
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "floatfold_tests.rs"]
pub(crate) mod tests;
