//! `getRangeViaFactoring`: the range of a recurrence whose start and step are
//! selects of constants on one condition.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::AssumptionCache;

use super::{Cx, ScalarEvolution};

/// `{c ? 10 : 20,+,c ? 1 : 2}` over at most four trips is the union of
/// `{10,+,1}` and `{20,+,2}`: 10 to 26. The selects hid both from the range
/// (0 to 39 before).
const FACTORED: &str = "define i32 @f(i1 %c) {
entry:
  %start = select i1 %c, i32 10, i32 20
  %step = select i1 %c, i32 1, i32 2
  br label %h

h:
  %i = phi i32 [ %start, %entry ], [ %i2, %h ]
  %n = phi i32 [ 0, %entry ], [ %n2, %h ]
  %i2 = add i32 %i, %step
  %n2 = add nuw nsw i32 %n, 1
  %e = icmp ult i32 %n2, 4
  br i1 %e, label %h, label %exit

exit:
  ret i32 %i2
}
";

#[test]
fn test_an_affine_recurrence_of_selects_has_the_union_of_its_two_ranges() {
    let module = llrm_mir::parse::module(FACTORED).expect("a module");
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
    let phi = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| {
            function.instruction(inst).result.is_some_and(|value| function.value(value).name.as_deref() == Some("i"))
        })
        .expect("%i");
    let value = function.instruction(phi).result.expect("a result");
    let mut se = ScalarEvolution::new();
    let i = se.get_scev(&cx, Operand::Value(value));
    let range = se.get_unsigned_range(&cx, i);
    assert_eq!((range.get_unsigned_min(), range.get_unsigned_max()), (10, 26), "{range:?}");
}
