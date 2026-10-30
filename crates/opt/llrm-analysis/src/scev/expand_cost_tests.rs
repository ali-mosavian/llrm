//! `isHighCostExpansion` over expressions the function computes, and over ones
//! it does not.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::AssumptionCache;

use super::ap::Ap;
use super::expand_cost::UnitCosts;
use super::expander::Expander;
use super::expr::{Scev, Use};
use super::{Cx, Flags, ScalarEvolution};

const BODY: &str = "define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  %t = add i32 %s, 1
  ret i32 %t
}
";

/// Whether the expansion of what `build` makes, before the `ret` of `BODY`,
/// is over `budget`.
fn high_cost(
    budget: u32,
    build: impl FnOnce(&mut ScalarEvolution, &Cx, &[Operand]) -> Scev,
) -> bool {
    let module = llrm_mir::parse::module(BODY).expect("a module");
    let (_, _, function) = module.functions().next().expect("a function");
    let layout = DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let mut se = ScalarEvolution::new();
    let entry = function.entry().expect("an entry");
    let values: Vec<Operand> =
        function.iter_block(entry).filter_map(|inst| function.instruction(inst).result.map(Operand::Value)).collect();
    // The instructions before the `ret` are read first, as the pass that asks
    // has read them: that is what `ExprValueMap` is made of.
    for &value in &values {
        se.get_scev(&cx, value);
    }
    let arguments: Vec<Operand> = function.parameters().iter().map(|&p| Operand::Value(p)).collect();
    let s = build(&mut se, &cx, &arguments);
    let ret = function.terminator(entry).expect("a ret");
    Expander::new("t").is_high_cost_expansion(
        &cx,
        &mut se,
        &[s],
        entry,
        budget,
        &UnitCosts { minimum_size: false },
        ret,
    )
}

fn sum(
    se: &mut ScalarEvolution,
    cx: &Cx,
    args: &[Operand],
    constant: u128,
) -> Scev {
    let a = Use::of(se.get_scev(cx, args[0]));
    let b = Use::of(se.get_scev(cx, args[1]));
    let c = Use::of(se.get_constant(Ap::new(32, constant)));
    se.get_add_expr(cx, vec![a, b, c], Flags::default(), 0).scev
}

#[test]
fn test_two_adds_are_over_a_budget_of_one() {
    assert!(high_cost(1, |se, cx, args| sum(se, cx, args, 2)));
    assert!(!high_cost(2, |se, cx, args| sum(se, cx, args, 2)));
}

/// `%t` already computes `a + b + 1` before the `ret`: the expansion is free,
/// whatever the budget. Counting its two adds made `rewriteLoopExitValues`
/// refuse a value the function already holds.
#[test]
fn test_an_expression_the_function_already_computes_is_free() {
    assert!(!high_cost(0, |se, cx, args| sum(se, cx, args, 1)));
    assert!(high_cost(0, |se, cx, args| sum(se, cx, args, 2)));
}

#[test]
fn test_an_unknown_costs_nothing() {
    assert!(!high_cost(0, |se, cx, args| se.get_scev(cx, args[0])));
}
