//! `getSCEVAtScope` where computing a value forgets it: `ValuesAtScopes[V]`
//! makes its entry again, and the 'held' expect did not (a panic in `sc.c` at
//! `-O2 -m16` from `indvars`, #2438).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::AssumptionCache;

use super::{Cx, ScalarEvolution};

const BODY: &str = "define i32 @f(i32 %a) {
entry:
  ret i32 %a
}
";

#[test]
fn test_a_value_forgotten_while_its_scope_is_computed_does_not_panic() {
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
    let a = se.get_unknown(&cx, Operand::Value(function.parameters()[0]));
    se.compute_hook = Some(|se, _, v| se.forget_memoized_results(&[v]));
    assert_eq!(se.get_scev_at_scope_use(&cx, a, None).scev, a);
}
