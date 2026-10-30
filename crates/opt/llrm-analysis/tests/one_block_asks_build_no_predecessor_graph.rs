//! `_unreachable`, `stranded` and the range and fact walks built a predecessor
//! graph of the whole body to ask what names a few blocks (1.9 M of a d_faces
//! -O2 compile): they ask the block's users, as LLVM's `predecessors(BB)` does.

use llrm_analysis::memory::Unit;
use llrm_analysis::testing::{DOS, with_registers};
use llrm_analysis::{cfg, consts, noreturn, ranges};
use llrm_support::graph::predecessor_graphs_built;

const COUNTED_LOOP: &str = "define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b4

b1:
  %i = phi i32 [ 0, %b0 ], [ %n, %b2 ]
  %t = icmp ult i32 %i, 10
  br i1 %t, label %b2, label %b3

b2:
  %n = add i32 %i, 1
  br label %b1

b3:
  ret i32 %i

b4:
  %j = phi i32 [ 0, %b0 ], [ 1, %b5 ]
  ret i32 %j

b5:
  br label %b4
}
";

#[test]
fn test_unreachable_stranded_ranges_and_consts_build_no_predecessor_graph() {
    let mut module = llrm_mir::parse::module(&format!("{DOS}{COUNTED_LOOP}")).unwrap_or_else(|error| panic!("{error}"));
    let layout = llrm_analysis::testing::layout(&module);
    let context = module.context.clone();
    let before = predecessor_graphs_built();
    {
        let function = llrm_analysis::testing::function(&module, "f");
        let unit = with_registers(Unit::of(&module, &layout, function));
        ranges::dominated_edges(&unit).expect("edges");
        consts::known(&unit);
        let header = cfg::id(function.layout()[1]);
        noreturn::stranded(function, header);
    }
    let function = module.function_mut("f").expect("@f").1;
    assert!(cfg::_unreachable(&context, function), "b5 is dropped");
    let built = predecessor_graphs_built() - before;
    assert_eq!(built, 0, "{built} predecessor graphs for four asks about one block's predecessors");
}
