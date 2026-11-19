//! The weights the 6502's prices multiply a block's clocks by.

use llrm_6502::weights::block_weights;
use llrm_mir::datalayout::DataLayout;

/// A loop whose body branches to a block that the branch's `!prof` says runs
/// once in 256 trips (the page walk's wrap). Priced at the loop's weight, as
/// the loop depth alone says, the block weighed ten times what it runs and
/// the page walk of lz4's literal copy priced 610 clocks slower where sim65
/// measured it 6,779 faster.
const WRAP: &str = "define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %j, %join ]
  %w = icmp eq i16 %i, 7
  br i1 %w, label %bump, label %join, !prof !0
bump:
  br label %join
join:
  %j = add i16 %i, 1
  %c = icmp ult i16 %j, %n
  br i1 %c, label %loop, label %done
done:
  ret i16 %j
}
!0 = !{!\"branch_weights\", i32 1, i32 255}
";

fn weights(text: &str) -> Vec<(String, u64)> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let function = module.functions().next().expect("a function").2;
    let layout = DataLayout::default();
    let weights = block_weights(&module, function, &layout);
    let labels = llrm_mir::print::block_labels(function);
    function
        .layout()
        .iter()
        .map(|block| (labels.get(block).cloned().unwrap_or_default(), weights[block.0 as usize]))
        .collect()
}

#[test]
fn a_block_a_branch_runs_one_in_256_trips_weighs_less_than_its_loop() {
    let weighted = weights(&WRAP.replace("\\\"", "\""));
    let at = |name: &str| weighted.iter().find(|(label, _)| label == name).map(|(_, weight)| *weight).unwrap();
    assert_eq!(at("join"), 10, "{weighted:?}");
    assert_eq!(at("bump"), 1, "{weighted:?}");
}

#[test]
fn a_branch_without_weights_leaves_the_loop_weight() {
    let plain = WRAP.replace(", !prof !0", "").replace("\\\"", "\"");
    let weighted = weights(&plain);
    let at = |name: &str| weighted.iter().find(|(label, _)| label == name).map(|(_, weight)| *weight).unwrap();
    assert_eq!(at("bump"), 10, "{weighted:?}");
}
