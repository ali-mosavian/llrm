//! A loop counter kept in a `float` or `double` becomes an `i32` one when its
//! start, step and limit are small whole numbers, so the trip count can be
//! worked out and the loop counted down or strength-reduced like any other:
//! `IndVarSimplify::rewriteNonIntegerIVs`, the pass's first step.
//!
//! # How it works
//! A header phi of a loop with two inputs: a constant start from outside, and
//! `fadd %phi, step` from the loop whose only other user is an `fcmp` against
//! a constant that only a conditional branch reads. That branch is in the loop
//! and leaves it. The start, step and limit must be exact integers within the
//! format's gap-free range (2^24 for `float`, 2^53 for `double`) and within
//! `i32`, the step not zero, and the walk from start to limit must neither
//! wrap nor skip the limit of an equality test. The `i32` counter replaces it;
//! a value the loop still reads from the old counter is `sitofp` of the new.
//!
//! ```text
//!  %x = phi double [ 0.0, %pre ], [ %x.next, %body ]       %x.int = phi i32 [ 0, %pre ], [ %x.next.int, %body ]
//!  %x.next = fadd double %x, 1.0                      ==>  %x.next.int = add i32 %x.int, 1
//!  %c = fcmp olt double %x.next, 10.0                      %c = icmp slt i32 %x.next.int, 10
//! ```
//!
//! DEVIATE: the pin converts with `APFloat::convertToInteger`; `float` and
//! `double` here are read as the host's `f32` and `f64`, which hold them
//! exactly. `x86_fp80` is not IEEE-like (`isIEEELikeFP`) and never converts.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::types::{FloatKind, Type};

use crate::edges;

struct FloatingPointIv {
    init: f64,
    incr: f64,
    exit: f64,
    /// The significand bits of the format: the gap-free range is `2^precision`.
    precision: u32,
    compare: InstId,
    add: InstId,
    branch: InstId,
}

struct IntegerIv {
    init: i64,
    incr: i64,
    exit: i64,
    predicate: IntPredicate,
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L514
/// `rewriteNonIntegerIVs` for each loop; whether any counter was rewritten.
pub fn rewritten(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    loops: &[Loop],
) -> bool {
    let mut changed = false;
    for loop_ in loops {
        let header = cfg::block(loop_.header);
        for phi in edges::phis(function, header) {
            changed |= handled(context, callees, function, loop_, phi);
        }
    }
    changed
}

/// The value of a float constant operand, with the precision of its type.
fn constant_fp(
    context: &Context,
    operand: Operand,
) -> Option<(f64, u32)> {
    let Operand::Constant(id) = operand else { return None };
    let Constant { ty, kind: ConstantKind::Float(bits) } = context.get(id) else { return None };
    match context.types.get(*ty) {
        Type::Float(FloatKind::Float) => Some((f64::from(f32::from_bits(*bits as u32)), 24)),
        Type::Float(FloatKind::Double) => Some((f64::from_bits(*bits), 53)),
        _ => None,
    }
}

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L264
/// `maybeFloatingPointRecurrence`.
fn recurrence(
    context: &Context,
    function: &Function,
    loop_: &Loop,
    phi: InstId,
) -> Option<FloatingPointIv> {
    let pn = function.instruction(phi);
    let [value0, Operand::Block(block0), value1, Operand::Block(_)] = pn.operands[..] else { return None };
    let result = pn.result?;
    // `L->contains(PN->getIncomingBlock(0))`: the backedge is input 0.
    let (incoming, back) = if loop_.body.contains(&cfg::id(block0)) { (value1, value0) } else { (value0, value1) };

    let (init, precision) = constant_fp(context, incoming)?;
    let add = instruction_of(function, back)?;
    let increment = function.instruction(add);
    if increment.opcode != Opcode::Binary(BinaryOp::FAdd) {
        return None;
    }
    let (step, _) = constant_fp(context, increment.operands[1])?;
    if increment.operands[0] != Operand::Value(result) {
        return None;
    }
    // One user is the phi and the other the exit test.
    let add_value = increment.result?;
    if !function.users(add_value).has_n_uses(2) {
        return None;
    }
    let compare = function
        .users(add_value)
        .iter()
        .map(|one| one.user)
        .find(|&user| matches!(function.instruction(user).opcode, Opcode::FCmp(_)))?;
    let compare_value = function.instruction(compare).result?;
    // The branch must control the iteration count: in the loop, leaving it.
    let branch = function.users(compare_value).only()?.user;
    let b = function.instruction(branch);
    if b.opcode != Opcode::Br || b.operands.len() != 3 {
        return None;
    }
    let [Operand::Block(then), Operand::Block(otherwise)] = b.operands[1..] else { return None };
    let at = function.parent(branch)?;
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    if !inside(at) || (inside(then) && inside(otherwise)) {
        return None;
    }
    let (exit, _) = constant_fp(context, function.instruction(compare).operands[1])?;
    Some(FloatingPointIv { init, incr: step, exit, precision, compare, add, branch })
}

/// `getIntegerPredicate`.
fn integer_predicate(predicate: FloatPredicate) -> Option<IntPredicate> {
    use FloatPredicate as F;
    Some(match predicate {
        F::Oeq | F::Ueq => IntPredicate::Eq,
        F::One | F::Une => IntPredicate::Ne,
        F::Ogt | F::Ugt => IntPredicate::Sgt,
        F::Oge | F::Uge => IntPredicate::Sge,
        F::Olt | F::Ult => IntPredicate::Slt,
        F::Ole | F::Ule => IntPredicate::Sle,
        _ => return None,
    })
}

/// `ConvertToSInt`: the value as an `i64` where it is exactly one.
fn to_signed(value: f64) -> Option<i64> {
    (value.is_finite() && value == value.trunc() && value.abs() < 9.223_372_036_854_775_8e18).then(|| value as i64)
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L331
/// `tryConvertToIntegerIV`.
fn convertible(
    function: &Function,
    iv: &FloatingPointIv,
) -> Option<IntegerIv> {
    let Opcode::FCmp(float_predicate) = function.instruction(iv.compare).opcode else { return None };
    let predicate = integer_predicate(float_predicate)?;
    let (init, incr, exit) = (to_signed(iv.init)?, to_signed(iv.incr)?, to_signed(iv.exit)?);
    // `isRepresentableAsExactInteger`: below the format's first gap.
    let gap_free = |value: i64| value.unsigned_abs() < (1_u64 << iv.precision);
    if !gap_free(init) || !gap_free(exit) {
        return None;
    }
    // Start, stride and limit fit `i32`, and the stride moves.
    let fits = |value: i64| i32::try_from(value).is_ok();
    if !fits(init) || !fits(incr) || !fits(exit) || incr == 0 {
        return None;
    }
    let (init32, incr32, exit32) = (init as i32, incr as i32, exit as i32);
    let equality = matches!(predicate, IntPredicate::Eq | IntPredicate::Ne);
    if incr > 0 {
        if init >= exit {
            return None;
        }
        let mut range = (exit32 as u32).wrapping_sub(init32 as u32);
        // while (i <= Exit) or until (i > Exit)
        if matches!(predicate, IntPredicate::Sle | IntPredicate::Sgt) {
            range = range.wrapping_add(1);
            if range == 0 {
                return None;
            }
        }
        let leftover = range % (incr32 as u32);
        if equality && leftover != 0 {
            return None;
        }
        if leftover != 0 && exit32.wrapping_add(incr32) < exit32 {
            return None;
        }
    } else {
        if init <= exit {
            return None;
        }
        let mut range = (init32 as u32).wrapping_sub(exit32 as u32);
        // while (i >= Exit) or until (i < Exit)
        if matches!(predicate, IntPredicate::Sge | IntPredicate::Slt) {
            range = range.wrapping_add(1);
            if range == 0 {
                return None;
            }
        }
        let leftover = range % (incr32.wrapping_neg() as u32);
        if equality && leftover != 0 {
            return None;
        }
        if leftover != 0 && exit32.wrapping_add(incr32) > exit32 {
            return None;
        }
    }
    Some(IntegerIv { init, incr, exit, predicate })
}

fn handled(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    loop_: &Loop,
    phi: InstId,
) -> bool {
    let Some(float) = recurrence(context, function, loop_, phi) else { return false };
    let Some(int) = convertible(function, &float) else { return false };
    canonicalize(context, callees, function, loop_, phi, &float, &int);
    true
}

/// `canonicalizeToIntegerIV`.
fn canonicalize(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    loop_: &Loop,
    phi: InstId,
    float: &FloatingPointIv,
    int: &IntegerIv,
) {
    let i32_ty = context.types.int(32);
    let constant = |value: i64| Operand::Constant(context.int(i32_ty, i128::from(value)));
    let pn = function.instruction(phi).clone();
    let [value0, Operand::Block(block0), value1, Operand::Block(block1)] = pn.operands[..] else { unreachable!() };
    let (incoming_block, back_block) =
        if loop_.body.contains(&cfg::id(block0)) { (block1, block0) } else { (block0, block1) };
    let _ = (value0, value1);
    let phi_value = pn.result.expect("a phi's value");
    let phi_name = function.value(phi_value).name.clone();
    let add_value = function.instruction(float.add).result.expect("an fadd's value");
    let add_name = function.value(add_value).name.clone();

    // The new counter, before the old.
    let new_phi = function.create_instruction(
        Opcode::Phi,
        i32_ty,
        Vec::new(),
        Flags::default(),
        phi_name.as_ref().map(|name| format!("{name}.int")).as_deref(),
    );
    function.insert(new_phi, Position::Before(phi)).expect("a placed phi");
    let new_phi_value = function.instruction(new_phi).result.expect("a phi's value");
    let new_add = function.create_instruction(
        Opcode::Binary(BinaryOp::Add),
        i32_ty,
        vec![Operand::Value(new_phi_value), constant(int.incr)],
        Flags::default(),
        add_name.as_ref().map(|name| format!("{name}.int")).as_deref(),
    );
    function.insert(new_add, Position::Before(float.add)).expect("a placed add");
    let new_add_value = function.instruction(new_add).result.expect("an add's value");
    function.set_operands(
        new_phi,
        vec![
            constant(int.init),
            Operand::Block(incoming_block),
            Operand::Value(new_add_value),
            Operand::Block(back_block),
        ],
    );

    let new_compare = function.create_instruction(
        Opcode::ICmp(int.predicate),
        function.instruction(float.compare).ty,
        vec![Operand::Value(new_add_value), constant(int.exit)],
        Flags::default(),
        None,
    );
    function.insert(new_compare, Position::Before(float.branch)).expect("a placed compare");
    // The branch reads the new test; the old one goes and its name moves.
    let old_compare = function.instruction(float.compare).result.expect("a compare's value");
    let new_compare_value = function.instruction(new_compare).result.expect("a compare's value");
    function.take_name(old_compare, new_compare_value);
    function.replace_all_uses_with(old_compare, Operand::Value(new_compare_value));
    crate::dead::dead_from(context, callees, function, [old_compare]);

    // The old increment goes: the phi's backedge input becomes poison.
    let poison =
        Operand::Constant(context.constant(Constant { ty: function.value(add_value).ty, kind: ConstantKind::Poison }));
    function.replace_all_uses_with(add_value, poison);
    crate::dead::dead_from(context, callees, function, [add_value]);

    // Anything else that reads the old counter reads a `sitofp` of the new one.
    if !function.is_erased(phi) && !function.users(phi_value).is_empty() {
        let at = function
            .iter_block(function.parent(phi).expect("a placed phi"))
            .find(|&one| function.instruction(one).opcode != Opcode::Phi)
            .expect("a terminator");
        let conversion = function.create_instruction(
            Opcode::Cast(CastOp::SIToFP),
            pn.ty,
            vec![Operand::Value(new_phi_value)],
            Flags::default(),
            Some("indvar.conv"),
        );
        function.insert(conversion, Position::Before(at)).expect("a placed block");
        let converted = function.instruction(conversion).result.expect("a cast's value");
        function.replace_all_uses_with(phi_value, Operand::Value(converted));
        crate::dead::dead_from(context, callees, function, [phi_value]);
    }
}

#[cfg(test)]
#[path = "floativ_tests.rs"]
mod tests;
