//! The remainder idiom `x - (x / c) * c` read back as `x % c`, where the target
//! keeps the divide.
//!
//! # How it works
//! Loop strength reduction expands a remainder by a constant, as the pin's
//! does: ScalarEvolution models `urem x, c` as `x - (x /u c) * c`, and LSR
//! writes that out, narrowed to the counter it chose (`sub i16 %n, mul (trunc
//! (udiv i32 %x, 251)), 251`). A target that lowers the division to a
//! multiply-high is served; one that keeps the `div` (a word, or any size at
//! -Os) pays a divide, a multiply and a subtract where the divide's own
//! remainder was one instruction. Here the idiom is found after LSR and
//! replaced by `urem`/`srem` (and a `trunc`), whose selection reads the
//! divide's remainder.
//!
//! The quotient, its `trunc` and the product must have no other user (a second
//! user is `DivRemPairs`' case, and a remainder beside a live quotient
//! is a second divide). The subtrahend's left operand is `x` itself, or a
//! counter ScalarEvolution proves is `trunc x + k` for a constant `k` (`x % c +
//! 1` becomes `(x + 1) - (x / c) * c` once LSR picks the counter `x + 1`); the
//! fold then adds `k` to the remainder.
//!
//! DEVIATE REM1: the pin has this fold in the DAG (`foldRemainderIdiom`,
//! [DAGCombiner.cpp#L4215](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4215)),
//! on a `UDIVREM` node, and not through a `trunc`. Ours is an IR fold so both
//! selectors take it, and it looks through the narrowing LSR makes.
//! Exit: the DAG selector's `foldRemainderIdiom`, and the old selector gone.
//!
//! # Example
//! What LSR leaves of `trunc (urem i32 %x, 251)` at -Os, and what this pass
//! makes of it (schematic: names are the printer's):
//!
//! ```text
//!  before                                  after
//!  %q = udiv i32 %x, 251                   %w = urem i32 %x, 251
//!  %t = trunc i32 %q to i16                %r = trunc i32 %w to i16
//!  %m = mul nuw i16 %t, 251
//!  %r = sub i16 %n, %m
//! ```
//!
//! From unit test
//! `test_the_narrowed_idiom_is_one_remainder_where_the_divide_is_kept`
//! in `remidiom_tests.rs`, which asserts the `urem` and that no `udiv` or
//! `mul` is left.
//!
//! # Runs
//! After `lsr` and `differences`, in the strength slot
//! ([`crate::pipeline::strength_slot`]).

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::{Ty, Use};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct RemainderIdiom {
    /// The build is for size (-Os, -Oz).
    pub size: bool,
}

/// What was matched: the instructions to go and the operands of the remainder.
struct Idiom {
    sub: InstId,
    mul: InstId,
    trunc: Option<InstId>,
    div: InstId,
    dividend: Operand,
    divisor: Operand,
    signed: bool,
    wide: u32,
    narrow: u32,
    /// The subtrahend's left operand.
    left: Operand,
}

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(def) => Some(def),
        _ => None,
    }
}

/// The value of an integer constant operand, zero-extended.
fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

fn single_user(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|result| function.users(result).iter().count() == 1)
}

fn mask(
    value: u128,
    bits: u32,
) -> u128 {
    if bits >= 128 { value } else { value & ((1u128 << bits) - 1) }
}

/// `sub` as an [`Idiom`] if it is `x - (x / c) * c` (the quotient narrowed or
/// not), else none; whether its left operand is `x` is not yet known.
fn matched(
    context: &Context,
    function: &Function,
    sub: InstId,
) -> Option<Idiom> {
    let one = function.instruction(sub);
    if one.opcode != Opcode::Binary(BinaryOp::Sub) {
        return None;
    }
    let &[left, product] = &one.operands[..] else { return None };
    let narrow = context.types.int_bits(one.ty)?;
    let mul = instruction_of(function, product)?;
    if function.instruction(mul).opcode != Opcode::Binary(BinaryOp::Mul) || !single_user(function, mul) {
        return None;
    }
    let &[first, second] = &function.instruction(mul).operands[..] else { return None };
    let (quotient, factor) = match (constant(context, first), constant(context, second)) {
        (None, Some(factor)) => (first, factor),
        (Some(factor), None) => (second, factor),
        _ => return None,
    };
    let quotient_def = instruction_of(function, quotient)?;
    let (trunc, div) = if function.instruction(quotient_def).opcode == Opcode::Cast(CastOp::Trunc) {
        let inner = instruction_of(function, function.instruction(quotient_def).operands[0])?;
        if !single_user(function, quotient_def) {
            return None;
        }
        (Some(quotient_def), inner)
    } else {
        (None, quotient_def)
    };
    let signed = match function.instruction(div).opcode {
        Opcode::Binary(BinaryOp::UDiv) => false,
        Opcode::Binary(BinaryOp::SDiv) => true,
        _ => return None,
    };
    if !single_user(function, div) {
        return None;
    }
    let &[dividend, divisor] = &function.instruction(div).operands[..] else { return None };
    let amount = constant(context, divisor)?;
    let wide = context.types.int_bits(function.operand_type(context, dividend)?)?;
    if narrow > wide || (trunc.is_none() && narrow != wide) || (trunc.is_some() && narrow == wide) {
        return None;
    }
    // `c` of the product is the divisor's low bits, and the divisor is a real
    // one: 2 up to the signed maximum.
    let limit = if signed { 1u128 << (wide - 1) } else { 1u128 << wide.min(127) };
    if amount < 2 || amount >= limit || mask(amount, narrow) != mask(factor, narrow) {
        return None;
    }
    Some(Idiom { sub, mul, trunc, div, dividend, divisor, signed, wide, narrow, left })
}

/// The constant `k` with `left` = `trunc x + k`: zero where `left` is the
/// dividend (or its `trunc`) itself, structurally; else as ScalarEvolution has
/// it (a counter LSR chose beside the wide one, one past it for `x % c + 1`).
fn offset_of(
    context: &Context,
    function: &Function,
    idiom: &Idiom,
    session: &mut Option<ScevSession>,
    layout: &llrm_mir::datalayout::DataLayout,
    outer: &llrm_mir::passes::Outer,
) -> Option<u128> {
    if idiom.left == idiom.dividend {
        return (idiom.narrow == idiom.wide).then_some(0);
    }
    if let Some(def) = instruction_of(function, idiom.left)
        && idiom.narrow != idiom.wide
        && function.instruction(def).opcode == Opcode::Cast(CastOp::Trunc)
        && function.instruction(def).operands[0] == idiom.dividend
    {
        return Some(0);
    }
    let session = session.get_or_insert_with(|| ScevSession::of(context, function, outer));
    let (left, dividend, narrow, wide) = (idiom.left, idiom.dividend, idiom.narrow, idiom.wide);
    session.ask(context, layout, function, outer, |se, cx| {
        let wanted = se.get_scev(cx, dividend);
        let wanted =
            if narrow == wide { wanted } else { se.get_truncate_expr(cx, Use::of(wanted), Ty::Int(narrow), 0) };
        let left = se.get_scev(cx, left);
        let diff = se.get_minus_scev(cx, Use::of(left), Use::of(wanted), 0, 0);
        se.as_constant(diff).map(|k| mask(k.zext_value(), narrow))
    })
}

impl FunctionPass for RemainderIdiom {
    fn name(&self) -> &'static str {
        "remidiom"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let idioms: Vec<Idiom> = {
            let function = &*unit.function;
            function.walk().map(|(_, inst)| inst).filter_map(|inst| matched(unit.context, function, inst)).collect()
        };
        if idioms.is_empty() {
            return PreservedAnalyses::all();
        }
        let outer = std::rc::Rc::clone(analyses.outer());
        let mut session = None;
        let mut changed = false;
        for idiom in idioms {
            if !outer.target().keeps_division(idiom.wide, self.size) {
                continue;
            }
            let Some(offset) = offset_of(unit.context, unit.function, &idiom, &mut session, unit.layout, &outer) else {
                continue;
            };
            replace(unit.context, unit.function, &idiom, offset);
            changed = true;
        }
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `idiom` as one remainder (and a `trunc` where the product was narrowed, an
/// `add` of `offset` where the counter ran that far past the dividend); the
/// instructions it replaces erased.
fn replace(
    context: &Context,
    function: &mut Function,
    idiom: &Idiom,
    offset: u128,
) {
    let wide_ty = context.types.int(idiom.wide);
    let name = if idiom.signed { BinaryOp::SRem } else { BinaryOp::URem };
    let rem = function.create_instruction(
        Opcode::Binary(name),
        wide_ty,
        vec![idiom.dividend, idiom.divisor],
        Flags::default(),
        None,
    );
    function.insert(rem, Position::Before(idiom.sub)).expect("a position before the subtraction");
    let mut result = Operand::Value(function.instruction(rem).result.expect("a remainder has a value"));
    if idiom.narrow < idiom.wide {
        let ty = function.instruction(idiom.sub).ty;
        let narrowed =
            function.create_instruction(Opcode::Cast(CastOp::Trunc), ty, vec![result], Flags::default(), None);
        function.insert(narrowed, Position::Before(idiom.sub)).expect("a position before the subtraction");
        result = Operand::Value(function.instruction(narrowed).result.expect("a trunc has a value"));
    }
    if offset != 0 {
        let ty = function.instruction(idiom.sub).ty;
        let k = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(offset) }));
        let shifted =
            function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![result, k], Flags::default(), None);
        function.insert(shifted, Position::Before(idiom.sub)).expect("a position before the subtraction");
        result = Operand::Value(function.instruction(shifted).result.expect("an add has a value"));
    }
    let old = function.instruction(idiom.sub).result.expect("a subtraction has a value");
    function.replace_all_uses_with(old, result);
    for gone in [Some(idiom.sub), Some(idiom.mul), idiom.trunc, Some(idiom.div)].into_iter().flatten() {
        function.erase(gone).expect("the idiom's instructions are read only by each other");
    }
}

#[cfg(test)]
#[path = "remidiom_tests.rs"]
mod tests;
