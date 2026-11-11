//! `hoist_min_max` built a pattern query for every instruction a loop held
//! (43 K `logical_and`/`logical_or` pairs in a memory_expansion -O2 compile),
//! and `LoopCtx::contains` looked a map up for every ask though nothing had
//! moved (89 K asks). Only an `and`, `or` or `select` can be a min/max
//! compare, and an empty map answers nothing.

use llrm_mir::counts::{Event, get};
use llrm_transforms::licm::Licm;

const LOOP: &str = "@g = global i16 0
define void @f(i16 %n, i16 %a) {
b0:
  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %j, %h ]
  %x = add i16 %i, %a
  %y = mul i16 %x, 3
  store i16 %y, ptr @g
  %j = add i16 %i, 1
  %t = icmp slt i16 %j, %n
  br i1 %t, label %h, label %exit

exit:
  ret void
}
";

#[test]
fn test_a_loop_of_adds_and_muls_builds_no_min_max_query_and_no_moved_lookup() {
    let mut module = llrm_mir::parse::module(LOOP).unwrap_or_else(|why| panic!("{why:?}"));
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    let (queries, lookups) = (get(Event::HoistMinMaxQueries), get(Event::LoopCtxMovedLookups));
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm");
    assert_eq!(get(Event::HoistMinMaxQueries) - queries, 0, "a query for an add or a mul");
    assert_eq!(get(Event::LoopCtxMovedLookups) - lookups, 0, "a lookup in a map nothing moved into");
}
