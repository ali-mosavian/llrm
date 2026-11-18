//! A pointer walk is post-indexed only where every access of it runs each
//! trip. quicksort's partition loop stores to `a[j]` under a condition: LSR
//! still made a pointer phi for it (reassociation was switched off for any
//! address that may be post-incremented), kept the index for the `slt` exit
//! test, and added one step a trip: 131,470 instructions at -O2 against
//! 119,494 for the index with a scaled address.

use llrm_arm64::tti::AArch64;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

const IR: &str = "target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n\
define void @k(ptr %a, i64 %n, i32 %pivot) {\n\
entry:\n  br label %loop\n\
loop:\n  %j = phi i64 [ 0, %entry ], [ %next, %latch ]\n\
  %p = getelementptr i32, ptr %a, i64 %j\n  %v = load i32, ptr %p\n\
  %c = icmp slt i32 %v, %pivot\n  br i1 %c, label %then, label %latch\n\
then:\n  store i32 0, ptr %p\n  br label %latch\n\
latch:\n  %next = add nuw nsw i64 %j, 1\n  %d = icmp slt i64 %next, %n\n  br i1 %d, label %loop, label %exit\n\
exit:\n  ret void\n}\n";

fn phis(ir: &str) -> usize {
    let mut module = llrm_mir::parse::module(ir).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("datalayout");
    let tti = AArch64::of_cpu("generic").unwrap();
    let outer = Outer::of(&module, None);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("k").expect("a function");
    llrm_transforms::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
    llrm_mir::print::module(&module).matches("phi").count()
}

#[test]
fn a_conditionally_stored_address_keeps_one_induction_variable() {
    assert_eq!(phis(IR), 1);
}
