//! `SCEVExpander` over expressions built by hand, as `createSCEV` (S3) does
//! not yet make them from the IR: what each node expands to, printed.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;
use llrm_mir::valuetracking::AssumptionCache;

use super::ap::Ap;
use super::expander::{Env, Expander};
use super::expr::{Scev, Ty, Use};
use super::{Cx, Flags, ScalarEvolution};

const BODY: &str = "define i32 @f(i32 %a, i32 %b, i64 %w) {
entry:
  ret i32 0
}
";

/// `build`'s SCEV expanded before the `ret` of `BODY`, and the body printed.
fn expanded(build: impl FnOnce(&mut ScalarEvolution, &Cx, &[Operand]) -> Scev) -> String {
    let module = llrm_mir::parse::module(BODY).expect("a module");
    let (_, _, original) = module.functions().next().expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let mut se = ScalarEvolution::new();
    let arguments: Vec<Operand> = (0..3).map(|at| Operand::Value(function.parameters()[at])).collect();
    let s = build(&mut se, &env.cx(&context, &function), &arguments);
    let ret = function.terminator(function.entry().expect("an entry")).expect("a ret");
    let mut expander = Expander::new("t");
    expander.set_insert_point(ret);
    let mut declared = Declared::of(&module);
    let result = expander.expand_code_for(&env, &mut context, &mut declared, &mut function, &mut se, s, None);
    assert!(matches!(function.instruction(ret).opcode, Opcode::Ret));
    let _ = result;
    llrm_mir::print::body(&context, &function)
}

fn unknown(
    se: &mut ScalarEvolution,
    cx: &Cx,
    v: Operand,
) -> Use {
    Use::of(se.get_unknown(cx, v))
}

fn constant(
    se: &mut ScalarEvolution,
    width: u32,
    value: u128,
) -> Use {
    Use::of(se.get_constant(Ap::new(width, value)))
}

/// The body's instructions, one per line, without the `ret`.
fn lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|l| l.starts_with('%') && l.contains(" = ")).collect()
}

#[test]
fn test_an_add_of_a_scaled_unknown_is_a_shift_and_an_add() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let four = constant(se, 32, 4);
        let scaled = se.get_mul_expr(cx, vec![four, b], Flags::default(), 0);
        se.get_add_expr(cx, vec![a, scaled], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text), ["%0 = shl i32 %b, 2", "%1 = add i32 %a, %0"]);
}

/// `a + (-1 * b)` is a subtract, not a negate and an add.
#[test]
fn test_a_negated_unknown_is_subtracted() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let minus_two = constant(se, 32, u32::MAX as u128 - 1);
        let scaled = se.get_mul_expr(cx, vec![minus_two, b], Flags::default(), 0);
        se.get_add_expr(cx, vec![a, scaled], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text), ["%0 = shl i32 %b, 1", "%1 = sub i32 %a, %0"]);
}

#[test]
fn test_a_udiv_by_a_power_of_two_is_a_shift_right() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let eight = constant(se, 32, 8);
        se.get_udiv_expr(cx, a, eight)
    });
    assert_eq!(lines(&text), ["%0 = lshr i32 %a, 3"]);
}

#[test]
fn test_a_max_of_unknowns_is_the_intrinsic() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        se.get_umax_expr(cx, vec![a, b])
    });
    assert_eq!(lines(&text), ["%0 = call i32 @1(i32 %b, i32 %a)"]);
}

#[test]
fn test_a_sequential_min_freezes_what_it_may_skip() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        se.get_sequential_min_max_expr(cx, super::expr::Type::SequentialUMin, vec![a, b])
    });
    // The last operand is frozen; the first (`i == 0`) is not.
    assert_eq!(lines(&text), ["%0 = freeze i32 %b", "%1 = call i32 @1(i32 %0, i32 %a)"]);
}

/// A truncate of a zero-extend reads back the narrow value.
#[test]
fn test_casts_expand_to_casts() {
    let text = expanded(|se, cx, args| {
        let w = unknown(se, cx, args[2]);
        se.get_truncate_expr(cx, w, Ty::Int(32), 0)
    });
    assert_eq!(lines(&text), ["%0 = trunc i64 %w to i32"]);
}

/// Two expansions of one binop at one point are one instruction:
/// `InsertBinop`'s scan of the six before the insert point.
#[test]
fn test_an_equal_binop_just_before_the_insert_point_is_reused() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let sum = se.get_add_expr(cx, vec![a, b], Flags::default(), 0);
        se.get_add_expr(cx, vec![sum, sum], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text).len(), 2, "{text}");
}
