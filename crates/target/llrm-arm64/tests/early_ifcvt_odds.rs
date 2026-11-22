//! The edge odds of a block early if-conversion joined.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;
use llrm_arm64::mfunc::{MBlock, MFunc, OpList};

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

const LOOP: &str = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i2, %latch ]
  %s = phi i32 [ 0, %entry ], [ %s2, %latch ]
  %g = getelementptr i32, ptr %p, i32 %i
  %x = load i32, ptr %g
  %c = icmp sgt i32 %x, 7
  br i1 %c, label %then, label %latch
then:
  %t = add i32 %s, %x
  br label %latch
latch:
  %s2 = phi i32 [ %s, %loop ], [ %t, %then ]
  %i2 = add i32 %i, 1
  %d = icmp slt i32 %i2, %n
  br i1 %d, label %loop, label %exit
exit:
  ret i32 %s2
}
";

/// Joining the loop's latch into its head left the head with no edges (the
/// tail's block had no place), so the allocator saw the loop's back edge and
/// exit as unlikely-to-matter: no odds at all for the merged block.
#[test]
fn a_joined_head_carries_its_tails_edges() {
    let program = driver::program(&format!("{LAYOUT}{LOOP}"), Platform::Linux).unwrap();
    let module = &program.modules[0];
    let function = module.functions().next().unwrap().2;
    let ids: Vec<u32> = function.layout().iter().map(|b| b.0).collect();
    // entry, loop, then, latch, exit: `then` and `latch` were merged into
    // `loop`.
    let mut machine = MFunc::default();
    let labels = [".Lf.0", ".Lf.1", ".Lf.4"];
    for label in labels {
        machine.blocks.push(MBlock { label: label.to_owned(), ops: OpList::from(vec![]) });
    }
    for (label, id) in [(".Lf.0", ids[0]), (".Lf.1", ids[1]), (".Lf.2", ids[2]), (".Lf.3", ids[3]), (".Lf.4", ids[4])] {
        machine.block_ids.insert(label.to_owned(), id);
    }
    machine.joined = vec![(".Lf.1".into(), ".Lf.3".into())];
    let odds = llrm_arm64::layout::edge_odds(module, function, &program.layout, &machine);
    assert!(odds.contains_key(&(1, 1)), "no back edge from the joined head: {odds:?}");
    assert!(odds.contains_key(&(1, 2)), "no exit edge from the joined head: {odds:?}");
}
