//! A counter that steps a `float` or `double` by a constant becomes an `i32`
//! counter, so ScalarEvolution can count the loop. LLVM's IndVarSimplify
//! `rewriteNonIntegerIVs` (`handleFloatingPointIV`).
//!
//! # How it works
//! For each header phi `%x = phi [C0, pre], [%n, latch]` with
//! `%n = fadd %x, C1`, whose only other use is an `fcmp` against a constant
//! that alone decides the exit branch of the loop: the start, step and bound
//! are whole numbers a signed 32-bit integer holds (and the float's mantissa
//! holds exactly), and the integer counter reaches the bound without wrapping.
//! Then an `i32` phi, add and `icmp` take the place of the three, and a read
//! of `%x` elsewhere in the loop becomes `sitofp` of the integer phi.
//!
//! # Example
//! `for (double x = 0; x < 100; x += 1) use(x)` becomes `for (int i = 0;
//! i < 100; i += 1) use((double)i)`.
//!
//! # Used by
//! [`crate::indvars`], first, as the pin runs it.
//!
//! # See also
//! LLVM `IndVarSimplify.cpp`. Tests: `floativ_tests.rs`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::{FloatKind, Type};

/// `LLRM_NO_FLOAT_IV`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_FLOAT_IV")
}

/// The matched pattern (`FloatingPointIV`), with its values as numbers.
struct FloatingPointIv {
    phi: InstId,
    add: InstId,
    compare: InstId,
    branch: InstId,
    init: f64,
    incr: f64,
    exit: f64,
    kind: FloatKind,
    incoming: BlockId,
    back: BlockId,
}

/// The integer counter (`IntegerIV`).
struct IntegerIv {
    init: i64,
    incr: i64,
    exit: i64,
    pred: IntPredicate,
}

/// Every float counter of `function` that can be an `i32` counter, made one;
/// whether anything changed.
pub fn converted(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
    let mut found = Vec::new();
    for l in env.loops.get_loops_in_preorder() {
        let header = l.get_header();
        for phi in function.iter_block(header).take_while(|&one| function.instruction(one).opcode == Opcode::Phi) {
            if let Some(iv) = maybe_floating_point_recurrence(context, function, &l, phi)
                && let Some(int) = try_convert_to_integer_iv(&iv, function)
            {
                found.push((iv, int));
            }
        }
    }
    drop(session);
    let changed = !found.is_empty();
    for (iv, int) in &found {
        canonicalize_to_integer_iv(context, function, iv, int);
    }
    changed
}

/// `maybeFloatingPointRecurrence`.
fn maybe_floating_point_recurrence(
    context: &Context,
    function: &Function,
    l: &llrm_mir::loopinfo::Loop<'_, BlockId>,
    phi: InstId,
) -> Option<FloatingPointIv> {
    let arms = llrm_analysis::occurrence::arms(function, phi);
    if arms.len() != 2 {
        return None;
    }
    let (init_edge, back_edge) = if l.contains(arms[0].1) { (1, 0) } else { (0, 1) };
    let Type::Float(kind @ (FloatKind::Float | FloatKind::Double)) = *context.types.get(function.instruction(phi).ty)
    else {
        return None;
    };
    let init = float_value(context, arms[init_edge].0, kind)?;
    let Operand::Value(next) = arms[back_edge].0 else { return None };
    let ValueDef::Instruction(add) = function.value(next).def else { return None };
    let add_inst = function.instruction(add);
    if add_inst.opcode != Opcode::Binary(BinaryOp::FAdd) {
        return None;
    }
    let phi_value = function.instruction(phi).result?;
    let incr = float_value(context, add_inst.operands[1], kind)?;
    if add_inst.operands[0] != Operand::Value(phi_value) {
        return None;
    }
    // One user is the phi, the other the exit compare.
    let users: Vec<_> = function.users(next).iter().map(|one| one.user).collect();
    if users.len() != 2 {
        return None;
    }
    let compare = users.iter().copied().find(|&one| matches!(function.instruction(one).opcode, Opcode::FCmp(_)))?;
    let compare_result = function.instruction(compare).result?;
    let mut compare_users = function.users(compare_result).iter();
    let branch = compare_users.next()?.user;
    if compare_users.next().is_some() {
        return None;
    }
    let br = function.instruction(branch);
    let (Opcode::Br, [_, Operand::Block(on_true), Operand::Block(on_false)]) = (&br.opcode, br.operands.as_slice())
    else {
        return None;
    };
    // The branch must decide whether the loop goes round.
    let block = function.parent(branch)?;
    if !l.contains(block) || (l.contains(*on_true) && l.contains(*on_false)) {
        return None;
    }
    let exit = float_value(context, function.instruction(compare).operands[1], kind)?;
    Some(FloatingPointIv {
        phi,
        add,
        compare,
        branch,
        init,
        incr,
        exit,
        kind,
        incoming: arms[init_edge].1,
        back: arms[back_edge].1,
    })
}

/// The value of a `float` or `double` constant.
fn float_value(
    context: &Context,
    operand: Operand,
    kind: FloatKind,
) -> Option<f64> {
    let Operand::Constant(constant) = operand else { return None };
    let ConstantKind::Float(bits) = context.get(constant).kind else { return None };
    Some(match kind {
        FloatKind::Float => f32::from_bits(bits as u32) as f64,
        _ => f64::from_bits(bits),
    })
}

/// `getIntegerPredicate`.
fn integer_predicate(pred: FloatPredicate) -> Option<IntPredicate> {
    use FloatPredicate::*;
    Some(match pred {
        Oeq | Ueq => IntPredicate::Eq,
        One | Une => IntPredicate::Ne,
        Ogt | Ugt => IntPredicate::Sgt,
        Oge | Uge => IntPredicate::Sge,
        Olt | Ult => IntPredicate::Slt,
        Ole | Ule => IntPredicate::Sle,
        _ => return None,
    })
}

/// `ConvertToSInt`: the value as an `i64` where that is exact.
fn to_sint(value: f64) -> Option<i64> {
    (value.is_finite() && value == value.trunc() && value >= -(2f64.powi(63)) && value < 2f64.powi(63))
        .then_some(value as i64)
}

/// `isRepresentableAsExactInteger`: `value` is below 2^24 (`float`) or 2^53
/// (`double`) in magnitude.
fn exact_integer(
    kind: FloatKind,
    value: i64,
) -> bool {
    let precision = if kind == FloatKind::Float { 24 } else { 53 };
    value.unsigned_abs() < 1u64 << precision
}

/// `tryConvertToIntegerIV`.
fn try_convert_to_integer_iv(
    iv: &FloatingPointIv,
    function: &Function,
) -> Option<IntegerIv> {
    let Opcode::FCmp(float_pred) = function.instruction(iv.compare).opcode else { return None };
    let pred = integer_predicate(float_pred)?;
    let (init, incr, exit) = (to_sint(iv.init)?, to_sint(iv.incr)?, to_sint(iv.exit)?);
    if !exact_integer(iv.kind, init) || !exact_integer(iv.kind, exit) {
        return None;
    }
    if [init, incr, exit].iter().any(|&one| i32::try_from(one).is_err()) || incr == 0 {
        return None;
    }
    if incr > 0 {
        if init >= exit {
            return None;
        }
        let mut range = (exit - init) as u32;
        if matches!(pred, IntPredicate::Sle | IntPredicate::Sgt) {
            range = range.wrapping_add(1);
            if range == 0 {
                return None;
            }
        }
        let leftover = range % incr as u32;
        if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) && leftover != 0 {
            return None;
        }
        if leftover != 0 && ((exit + incr) as i32 as i64) < exit {
            return None;
        }
    } else {
        if init <= exit {
            return None;
        }
        let mut range = (init - exit) as u32;
        if matches!(pred, IntPredicate::Sge | IntPredicate::Slt) {
            range = range.wrapping_add(1);
            if range == 0 {
                return None;
            }
        }
        let leftover = range % (-incr) as u32;
        if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) && leftover != 0 {
            return None;
        }
        if leftover != 0 && ((exit + incr) as i32 as i64) > exit {
            return None;
        }
    }
    Some(IntegerIv { init, incr, exit, pred })
}

/// `canonicalizeToIntegerIV`.
fn canonicalize_to_integer_iv(
    context: &mut Context,
    function: &mut Function,
    iv: &FloatingPointIv,
    int: &IntegerIv,
) {
    let i32_ty = context.types.int(32);
    let constant = |context: &Context, value: i64| Operand::Constant(context.int(i32_ty, value as i128));
    let new_phi = function.create_instruction(
        Opcode::Phi,
        i32_ty,
        vec![constant(context, int.init), Operand::Block(iv.incoming)],
        Flags::default(),
        None,
    );
    function.insert(new_phi, Position::Before(iv.phi)).expect("a placed phi");
    let phi_value = Operand::Value(function.instruction(new_phi).result.expect("a phi"));
    let new_add = function.create_instruction(
        Opcode::Binary(BinaryOp::Add),
        i32_ty,
        vec![phi_value, constant(context, int.incr)],
        Flags::default(),
        None,
    );
    function.insert(new_add, Position::Before(iv.add)).expect("a placed add");
    let add_value = Operand::Value(function.instruction(new_add).result.expect("an add"));
    function.set_operands(
        new_phi,
        vec![constant(context, int.init), Operand::Block(iv.incoming), add_value, Operand::Block(iv.back)],
    );
    let new_compare = function.create_instruction(
        Opcode::ICmp(int.pred),
        function.instruction(iv.compare).ty,
        vec![add_value, constant(context, int.exit)],
        Flags::default(),
        None,
    );
    function.insert(new_compare, Position::Before(iv.branch)).expect("a placed compare");
    let new_compare_value = Operand::Value(function.instruction(new_compare).result.expect("a compare"));
    function.set_operand(iv.branch, 0, new_compare_value);
    function.erase(iv.compare).expect("the compare has no user left");
    let phi_result = function.instruction(iv.phi).result.expect("a phi");
    // The phi still read anywhere but by its own increment: an `sitofp` of the
    // integer counter (preferred to `uitofp`, faster on most platforms).
    let read_elsewhere = function.users(phi_result).iter().any(|one| one.user != iv.add);
    if read_elsewhere {
        let header = function.parent(iv.phi).expect("placed");
        let first = function
            .iter_block(header)
            .find(|&one| !matches!(
                function.instruction(one).opcode,
                Opcode::Phi | Opcode::LandingPad { .. }
            ))
            .expect("a terminated block");
        let conv = function.create_instruction(
            Opcode::Cast(CastOp::SIToFP),
            function.instruction(iv.phi).ty,
            vec![phi_value],
            Flags::default(),
            Some("indvar.conv"),
        );
        function.insert(conv, Position::Before(first)).expect("a placed cast");
        let conv_value = Operand::Value(function.instruction(conv).result.expect("a cast"));
        // The increment read the phi; it goes with it.
        function.replace_all_uses_with(phi_result, conv_value);
    }
    function.erase_all(&[iv.phi, iv.add]).expect("the old counter is dead");
}

#[cfg(test)]
#[path = "floativ_tests.rs"]
mod tests;
