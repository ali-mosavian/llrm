//! CodeGenPrepare's overflow-math combines: a compare that tests a sum or a
//! difference for wrapping, beside that sum or difference, becomes the
//! `llvm.uadd.with.overflow` / `llvm.usub.with.overflow` call that yields both
//! ([CodeGenPrepare.cpp#L1747](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1747)).
//!
//! # How it works
//! For each `icmp`, `combineToUAddWithOverflow` and then
//! `combineToUSubWithOverflow`: the first that acts ends the visit.
//!
//! - add: `(a + b) u< a`, `a u> (a + b)` and the same with `b`, `~a u< b` and
//!   `b u> ~a`, `(a + 1) == 0`, and the edge cases `a == -1` beside `a + 1` and
//!   `a != 0` beside `a + -1`;
//! - sub: `a u< b` beside `a - b`, and `a u< C` beside `a + -C` (what the sub
//!   of a constant is canonicalized to), with `a u> b`, `a == 0` and `0 u< a`
//!   taken as `a u< 1`-style forms.
//!
//! The target agrees (`Lowering::should_form_overflow_op`), and the math and
//! the compare share a block, or the math is the increment of a loop's counter
//! and the compare sits in that loop where the increment can move up to it
//! (`replaceMathCmpWithIntrinsic`). The call goes where the first of the two
//! was, and the compare and the math become its two results.
//!
//! ```text
//!  %s = add i16 %x, -32                      %o = call {i16, i1} @llvm.usub.with.overflow.i16(i16 %x, i16 32)
//!  %c = icmp ult i16 %x, 32          ==>     %s = extractvalue {i16, i1} %o, 0
//!                                            %c = extractvalue {i16, i1} %o, 1
//! ```
//!
//! MIRROR: `combineToUAddWithOverflow`, `matchUAddWithOverflowConstantEdgeCases`,
//! `combineToUSubWithOverflow`, `replaceMathCmpWithIntrinsic`, `matchIncrement`,
//! `getIVIncrement`, `isIVIncrement` and `m_UAddWithOverflow`.
//!
//! # Example
//! `test_a_compare_with_the_add_of_its_negated_constant_is_a_usub_with_overflow`
//! in `cgpmath_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, after the compare sinks and before `cmpopt`'s steps.

use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::{Constant, ConstantKind, Context, mask};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::lowering::{Lowering, OverflowOp};
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};

use crate::cmpopt::query;

/// What the passes and the module give the combines.
struct Env<'a> {
    context: &'a Context,
    globals: &'a [GlobalValue],
    layout: &'a DataLayout,
}

/// What a combine found: the math (the add, the sub or the `xor`) and its two
/// operands as the intrinsic takes them.
struct Found {
    math: InstId,
    arg0: Operand,
    arg1: Operand,
}

/// The loops and the dominator tree, made once on the first ask.
struct Lazy<'a> {
    make: &'a mut dyn FnMut(&Function) -> (Vec<Loop>, Rc<DomTree>),
    made: Option<(Vec<Loop>, Rc<DomTree>)>,
}

impl Lazy<'_> {
    fn get(
        &mut self,
        function: &Function,
    ) -> &(Vec<Loop>, Rc<DomTree>) {
        let make = &mut self.make;
        self.made.get_or_insert_with(|| make(function))
    }
}

/// The integer a constant operand is.
fn int_constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero | ConstantKind::Null => Some(0),
        _ => None,
    }
}

fn type_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> TypeId {
    match operand {
        Operand::Value(value) => function.value(value).ty,
        Operand::Constant(id) => context.get(id).ty,
        other => unreachable!("an integer operand, not {other:?}"),
    }
}

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

/// `m_Add(m_Value(a), m_Value(b))`: the add `operand` is, and its operands.
fn add_of(
    function: &Function,
    operand: Operand,
) -> Option<(InstId, Operand, Operand)> {
    let inst = instruction_of(function, operand)?;
    let instruction = function.instruction(inst);
    (instruction.opcode == Opcode::Binary(BinaryOp::Add)).then(|| (inst, instruction.operands[0], instruction.operands[1]))
}

/// `m_OneUse(m_Not(m_Value(x)))`: the `xor` `operand` is, and `x`.
fn one_use_not(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<(InstId, Operand)> {
    let inst = instruction_of(function, operand)?;
    let Operand::Value(value) = operand else { return None };
    let xor = function.instruction(inst);
    if xor.opcode != Opcode::Binary(BinaryOp::Xor) || !function.users(value).has_one_use() {
        return None;
    }
    let bits = context.types.int_bits(xor.ty)?;
    let ones = |x: Operand| int_constant(context, x) == Some(mask(bits));
    match xor.operands[..] {
        [x, y] if ones(y) => Some((inst, x)),
        [y, x] if ones(y) => Some((inst, x)),
        _ => None,
    }
}

/// `m_UAddWithOverflow(m_Value(a), m_Value(b), m_BinOp(sum))` over the compare.
fn uadd_pattern(
    env: &Env,
    function: &Function,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Found> {
    let context = env.context;
    // (a + b) u< a, (a + b) u< b
    if pred == IntPredicate::Ult
        && let Some((sum, a, b)) = add_of(function, lhs)
        && (rhs == a || rhs == b)
    {
        return Some(Found { math: sum, arg0: a, arg1: b });
    }
    // a >u (a + b), b >u (a + b)
    if pred == IntPredicate::Ugt
        && let Some((sum, a, b)) = add_of(function, rhs)
        && (lhs == a || lhs == b)
    {
        return Some(Found { math: sum, arg0: a, arg1: b });
    }
    // (~a) <u b
    if pred == IntPredicate::Ult
        && let Some((not, a)) = one_use_not(context, function, lhs)
    {
        return Some(Found { math: not, arg0: a, arg1: rhs });
    }
    // b >u (~a)
    if pred == IntPredicate::Ugt
        && let Some((not, a)) = one_use_not(context, function, rhs)
    {
        return Some(Found { math: not, arg0: a, arg1: lhs });
    }
    // The increment by one: (a + 1) == 0, (1 + a) == 0, 0 == (a + 1), 0 == (1 + a).
    if pred == IntPredicate::Eq {
        let increment = |sum: Operand, zero: Operand| -> Option<Found> {
            let (inst, a, b) = add_of(function, sum)?;
            (int_constant(context, zero) == Some(0)
                && (int_constant(context, a) == Some(1) || int_constant(context, b) == Some(1)))
            .then_some(Found { math: inst, arg0: a, arg1: b })
        };
        return increment(lhs, rhs).or_else(|| increment(rhs, lhs));
    }
    None
}

/// `matchUAddWithOverflowConstantEdgeCases`: `a == -1` beside `a + 1`, and
/// `a != 0` beside `a + -1`.
fn uadd_edge_case(
    env: &Env,
    function: &Function,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Found> {
    let context = env.context;
    // Not expecting non-canonical code: a constant on the left.
    if matches!(lhs, Operand::Constant(_)) {
        return None;
    }
    let ty = type_of(context, function, rhs);
    let bits = context.types.int_bits(ty)?;
    let rhs_value = int_constant(context, rhs)?;
    let wanted = match pred {
        IntPredicate::Eq if rhs_value == mask(bits) => 1,
        IntPredicate::Ne if rhs_value == 0 => mask(bits),
        _ => return None,
    };
    let b = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(wanted) }));
    let Operand::Value(a) = lhs else { return None };
    for one in function.users(a).iter() {
        let user = function.instruction(one.user);
        if user.opcode == Opcode::Binary(BinaryOp::Add) && user.operands[0] == lhs && user.operands[1] == b {
            return Some(Found { math: one.user, arg0: lhs, arg1: b });
        }
    }
    None
}

/// The user of the compare's variable operand that `combineToUSubWithOverflow`
/// pairs it with: `sub a, b`, or `add a, -C` for `a u< C`.
fn usub_pattern(
    env: &Env,
    function: &Function,
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
) -> Option<Found> {
    let context = env.context;
    let (mut a, mut b) = (lhs, rhs);
    if matches!((a, b), (Operand::Constant(_), Operand::Constant(_))) {
        return None;
    }
    let ty = type_of(context, function, a);
    let bits = context.types.int_bits(ty)?;
    let mut pred = pred;
    // (A u> B) is (B u< A).
    if pred == IntPredicate::Ugt {
        std::mem::swap(&mut a, &mut b);
        pred = IntPredicate::Ult;
    }
    // (A == 0) is (A u< 1).
    if pred == IntPredicate::Eq && int_constant(context, b) == Some(0) {
        b = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(1) }));
        pred = IntPredicate::Ult;
    }
    // (A != 0) is (0 u< A).
    if pred == IntPredicate::Ne && int_constant(context, b) == Some(0) {
        std::mem::swap(&mut a, &mut b);
        pred = IntPredicate::Ult;
    }
    if pred != IntPredicate::Ult {
        return None;
    }
    let variable = if matches!(a, Operand::Constant(_)) { b } else { a };
    let Operand::Value(variable) = variable else { return None };
    for one in function.users(variable).iter() {
        let user = function.instruction(one.user);
        let Opcode::Binary(op) = user.opcode else { continue };
        let [x, y] = user.operands[..] else { continue };
        // A - B, A u< B --> usubo(A, B)
        if op == BinaryOp::Sub && x == a && y == b {
            return Some(Found { math: one.user, arg0: x, arg1: y });
        }
        // A + (-C), A u< C (the canonical form of `sub A, C`)
        if op == BinaryOp::Add
            && x == a
            && let (Some(add_c), Some(cmp_c)) = (int_constant(context, y), int_constant(context, b))
            && add_c == cmp_c.wrapping_neg() & mask(bits)
        {
            return Some(Found { math: one.user, arg0: x, arg1: y });
        }
    }
    None
}

/// `matchIncrement`: `inst` adds or subtracts a constant to an instruction,
/// directly or as the sum of the overflow intrinsic. The instruction.
fn match_increment(
    env: &Env,
    function: &Function,
    inst: InstId,
) -> Option<InstId> {
    let instruction = function.instruction(inst);
    let constant = |operand: Operand| matches!(operand, Operand::Constant(_));
    match instruction.opcode {
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => {
            let [x, step] = instruction.operands[..] else { return None };
            constant(step).then(|| instruction_of(function, x)).flatten()
        }
        Opcode::ExtractValue(ref at) if at[..] == [0] => {
            let call = instruction_of(function, instruction.operands[0])?;
            let q = query(env.context, env.globals, env.layout, function, call);
            let id = pattern::call_intrinsic(&q, call)?;
            if !matches!(id, IntrinsicId::UaddWithOverflow | IntrinsicId::UsubWithOverflow) {
                return None;
            }
            let [x, step, _callee] = function.instruction(call).operands[..] else { return None };
            constant(step).then(|| instruction_of(function, x)).flatten()
        }
        _ => None,
    }
}

/// The innermost of `loops` holding `block` (they come innermost first).
fn loop_for(
    loops: &[Loop],
    block: BlockId,
) -> Option<&Loop> {
    loops.iter().find(|one| one.body.contains(&cfg::id(block)))
}

/// `isIVIncrement(bo)` with `getIVIncrement`: `bo` steps the counter, a header
/// phi of a loop with one latch, by a constant. The loop.
fn iv_increment_loop<'a>(
    env: &Env,
    function: &Function,
    loops: &'a [Loop],
    bo: InstId,
) -> Option<&'a Loop> {
    let phi = match_increment(env, function, bo)?;
    if function.instruction(phi).opcode != Opcode::Phi {
        return None;
    }
    let header = function.parent(phi)?;
    let the_loop = loop_for(loops, header)?;
    if the_loop.header != cfg::id(header) || the_loop.latches.len() != 1 {
        return None;
    }
    let latch = *the_loop.latches.iter().next().expect("one latch");
    let arms = llrm_analysis::occurrence::arms(function, phi);
    let (value, _) = arms.iter().find(|&&(_, from)| cfg::id(from) == latch)?;
    let inc = instruction_of(function, *value)?;
    if loop_for(loops, function.parent(inc)?)?.header != the_loop.header {
        return None;
    }
    (match_increment(env, function, inc) == Some(phi) && inc == bo).then_some(the_loop)
}

/// The body of `replaceMathCmpWithIntrinsic`'s `IsReplacableIVIncrement`.
fn replaceable_iv_increment(
    env: &Env,
    function: &Function,
    lazy: &mut Lazy,
    bo: InstId,
    cmp: InstId,
) -> bool {
    let (loops, dt) = lazy.get(function);
    let Some(the_loop) = iv_increment_loop(env, function, loops, bo) else { return false };
    let (Some(bo_block), Some(cmp_block)) = (function.parent(bo), function.parent(cmp)) else { return false };
    // Do not risk on moving the increment into a child loop.
    if loop_for(loops, cmp_block).is_none_or(|one| one.header != the_loop.header) {
        return false;
    }
    // The insert point must dominate all existing uses of the increment.
    if dt.dominates(cmp_block, bo_block) {
        // Moving up the dominator tree, all uses are trivially dominated:
        // the common case for code produced by LSR.
        return true;
    }
    let latch = the_loop.latches.iter().next().expect("one latch");
    let Some(result) = function.instruction(bo).result else { return false };
    function.users(result).has_one_use() && function.layout().iter().any(|&block| cfg::id(block) == *latch && dt.dominates(cmp_block, block))
}

/// `replaceMathCmpWithIntrinsic(bo, arg0, arg1, cmp, id)`: the call before the
/// first of the math and the compare, the math and the compare read from it.
fn replace_math_cmp_with_intrinsic(
    env: &Env,
    declared: &mut Declared,
    function: &mut Function,
    lazy: &mut Lazy,
    found: &Found,
    cmp: InstId,
    op: OverflowOp,
) -> bool {
    let context = env.context;
    let bo = found.math;
    let (Some(bo_block), Some(cmp_block)) = (function.parent(bo), function.parent(cmp)) else { return false };
    // We used to use a dominator tree here to allow multi-block optimization;
    // the pin keeps the one block, and the increment of a loop's counter.
    if bo_block != cmp_block && !replaceable_iv_increment(env, function, lazy, bo, cmp) {
        return false;
    }
    let math_opcode = function.instruction(bo).opcode.clone();
    let ty = function.instruction(bo).ty;
    let Some(bits) = context.types.int_bits(ty) else { return false };
    let (arg0, mut arg1) = (found.arg0, found.arg1);
    // The canonical `add X, C` is matched back to `usubo X, -C`.
    if math_opcode == Opcode::Binary(BinaryOp::Add) && op == OverflowOp::USub {
        let Some(step) = int_constant(context, arg1) else { return false };
        arg1 = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(step.wrapping_neg() & mask(bits)) }));
    }
    // Insert at the first instruction of the pair. An `xor` may come before
    // an input of the intrinsic is defined.
    let is_xor = math_opcode == Opcode::Binary(BinaryOp::Xor);
    let Some(insert_at) = function.iter_block(cmp_block).find(|&one| (!is_xor && one == bo) || one == cmp) else {
        return false;
    };
    let i1 = context.types.int(1);
    let pair = context.types.intern(Type::Struct { fields: vec![ty, i1], packed: false });
    let function_type = context.types.intern(Type::Function { returns: pair, parameters: vec![ty, ty], variadic: false });
    let name = format!("llvm.{}.with.overflow.i{bits}", if op == OverflowOp::UAdd { "uadd" } else { "usub" });
    let callee = declared.declare(&name, function_type);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); 2],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let call = function.create_instruction(Opcode::Call(Box::new(info)), pair, vec![arg0, arg1, callee], Flags::default(), None);
    function.insert(call, Position::Before(insert_at)).expect("an unplaced call");
    let both = Operand::Value(function.instruction(call).result.expect("a pair"));
    let extract = |function: &mut Function, index: u32, ty: TypeId, name: &str| {
        let one = function.create_instruction(Opcode::ExtractValue(vec![index]), ty, vec![both], Flags::default(), Some(name));
        function.insert(one, Position::Before(insert_at)).expect("an unplaced extract");
        Operand::Value(function.instruction(one).result.expect("a value"))
    };
    if !is_xor {
        let math = extract(function, 0, ty, "math");
        let old = function.instruction(bo).result.expect("the math's value");
        function.replace_all_uses_with(old, math);
    }
    let overflow = extract(function, 1, i1, "ov");
    let old = function.instruction(cmp).result.expect("the compare's value");
    function.replace_all_uses_with(old, overflow);
    function.erase(cmp).expect("nothing reads the compare now");
    function.erase(bo).expect("nothing reads the math now");
    true
}

/// `combineToUAddWithOverflow` then `combineToUSubWithOverflow` over `cmp`.
fn combined(
    env: &Env,
    declared: &mut Declared,
    lowering: &dyn Lowering,
    function: &mut Function,
    lazy: &mut Lazy,
    cmp: InstId,
) -> bool {
    let instruction = function.instruction(cmp);
    let Opcode::ICmp(pred) = instruction.opcode else { return false };
    let [lhs, rhs] = instruction.operands[..] else { return false };
    let mut candidates: Vec<(Found, OverflowOp, usize)> = Vec::new();
    if let Some(found) = uadd_pattern(env, function, pred, lhs, rhs) {
        candidates.push((found, OverflowOp::UAdd, 2));
    } else if let Some(found) = uadd_edge_case(env, function, pred, lhs, rhs) {
        candidates.push((found, OverflowOp::UAdd, 1));
    }
    // The pin tries the add, then the sub, each to its end.
    let usub = usub_pattern(env, function, pred, lhs, rhs);
    for (found, op, used) in candidates.into_iter().chain(usub.map(|found| (found, OverflowOp::USub, 1))) {
        let Some(bits) = env.context.types.int_bits(function.instruction(found.math).ty) else { continue };
        let Some(result) = function.instruction(found.math).result else { continue };
        let math_used = function.users(result).has_n_uses_or_more(used);
        if !lowering.should_form_overflow_op(op, bits, math_used) {
            continue;
        }
        // We don't want to move around uses of condition values this late, so
        // the call is made in the block of the compare.
        if function.parent(found.math) != function.parent(cmp) && !function.users(result).has_one_use() {
            continue;
        }
        if replace_math_cmp_with_intrinsic(env, declared, function, lazy, &found, cmp, op) {
            return true;
        }
    }
    false
}

/// The overflow combines over every `icmp` of `function`. Whether any acted.
pub fn overflow_math_formed(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    declared: &mut Declared,
    lowering: &dyn Lowering,
    analyses: &mut dyn FnMut(&Function) -> (Vec<Loop>, Rc<DomTree>),
    function: &mut Function,
) -> bool {
    let cmps: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::ICmp(_)))
        .collect();
    let env = Env { context, globals, layout };
    let mut lazy = Lazy { make: analyses, made: None };
    let mut changed = false;
    for cmp in cmps {
        if !function.is_erased(cmp) {
            changed |= combined(&env, declared, lowering, function, &mut lazy, cmp);
        }
    }
    changed
}

#[cfg(test)]
#[path = "cgpmath_tests.rs"]
mod tests;
