//! `getRangeForUnknownRecurrence`: the range of a shift recurrence from the
//! loop's maximum trip count.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::AssumptionCache;

use super::{Cx, ScalarEvolution};

/// `%s` starts at 255 and is shifted right by 1 on each of at most 4 trips.
const SHIFTED: &str = "define i32 @f() {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %s = phi i32 [ 255, %entry ], [ %s2, %h ]
  %s2 = lshr i32 %s, 1
  %i2 = add nuw nsw i32 %i, 1
  %c = icmp ult i32 %i2, 4
  br i1 %c, label %h, label %exit

exit:
  ret i32 %s2
}
";

/// A start of 255 shifted right at most three times (four trips) stays at or
/// above 31: the range was the whole of 0..=255 that known bits give.
#[test]
fn test_a_right_shift_recurrence_over_four_trips_stays_above_its_last_value() {
    let module = llrm_mir::parse::module(SHIFTED).expect("a module");
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
    let phi = function.walk().map(|(_, inst)| inst).find(|&inst| {
        function.instruction(inst).result.is_some_and(|value| function.value(value).name.as_deref() == Some("s"))
    });
    let phi = phi.expect("%s");
    let value = function.instruction(phi).result.expect("a result");
    let mut se = ScalarEvolution::new();
    let s = se.get_scev(&cx, Operand::Value(value));
    let range = se.get_unsigned_range(&cx, s);
    assert_eq!(range.get_unsigned_min(), 31, "{range:?}");
}
