//! Branch probabilities of the DAG body (DEVIATE SD403): the IR's, so that
//! block frequencies and spill weights see a loop as a loop.

use super::dag_function::selected;

const LOOP: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  %j = add i32 %i, 1
  %more = icmp ne i32 %j, %n
  br i1 %more, label %loop, label %done
done:
  ret i32 %j
}
";

/// queens' `_place` kept every edge at 50/50 in the DAG body: its inner-loop
/// temporaries weighed less than the values around the loop, and the counter,
/// `n` and the induction variable went to the stack (AM-BT-6, +40% instructions
/// against the old selector). A loop's back edge is the likely one.
#[test]
fn a_loops_back_edge_is_the_likely_edge_of_the_dag_body() {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let body = &done.body;
    let looped = body.blocks.iter().find(|block| block.succ.contains(&block.at)).expect("a block that loops");
    let back = body.odds.taken.get(&(looped.at, looped.at)).copied().expect("the back edge has odds");
    assert!(
        f64::from(back) > 0.9 * f64::from(crate::model::lir::BlockOdds::CERTAIN),
        "the back edge is {back} of {}",
        crate::model::lir::BlockOdds::CERTAIN
    );
}
