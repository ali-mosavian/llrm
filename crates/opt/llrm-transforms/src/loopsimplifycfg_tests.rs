use super::loop_simplify_cfg;
use crate::testing::{parsed, printed};

/// @f of `text` after LoopSimplifyCFG; whether anything changed.
fn simplified(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = loop_simplify_cfg(context, function);
    (printed(&module), changed)
}

/// A branch on `true` in a loop body is a jump; the arm it did not take is
/// deleted and the blocks left in a chain merge. The pass was not in the
/// pipeline (a registry-missing row), so both arms stayed for SimplifyCFG.
#[test]
fn a_constant_branch_in_a_loop_is_folded_and_the_dead_arm_deleted() {
    let (text, changed) = simplified(
        "declare void @use(i16)
define void @f(i16 %n) {
b0:
  br label %h
h:
  %i = phi i16 [ 0, %b0 ], [ %i1, %latch ]
  br i1 true, label %live, label %dead
live:
  call void @use(i16 %i)
  br label %latch
dead:
  call void @use(i16 -1)
  br label %latch
latch:
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %h, label %x
x:
  ret void
}
",
    );
    assert!(changed);
    assert!(!text.contains("dead:") && !text.contains("i16 -1"), "{text}");
    assert!(!text.contains("br i1 true"), "{text}");
}

/// An exit the loop can no longer take stays reachable from the preheader by
/// a `switch i32 0`, as the pin leaves it for SimplifyCFG.
#[test]
fn a_dead_exit_is_kept_reachable_by_a_switch_in_the_preheader() {
    let (text, changed) = simplified(
        "declare void @use(i16)
define void @f(i16 %n) {
b0:
  br label %h
h:
  %i = phi i16 [ 0, %b0 ], [ %i1, %latch ]
  br i1 false, label %dead.exit, label %latch
latch:
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %h, label %x
x:
  ret void
dead.exit:
  call void @use(i16 7)
  ret void
}
",
    );
    assert!(changed);
    assert!(text.contains("switch i32 0"), "{text}");
    assert!(text.contains("dead.exit"), "{text}");
    // The split preheader follows the preheader, as `SplitBlock` puts it.
    let split = text.find("br label %h").expect("the split jump");
    assert!(
        text.find("switch i32 0").expect("the switch") < split && split < text.find("h:").expect("the header"),
        "{text}"
    );
}

/// Blocks in a chain inside a loop merge into their predecessors.
#[test]
fn a_chain_of_blocks_in_a_loop_is_merged() {
    let (text, changed) = simplified(
        "declare void @use(i16)
define void @f(i16 %n) {
b0:
  br label %h
h:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  call void @use(i16 %i)
  br label %b1
b1:
  call void @use(i16 1)
  br label %b2
b2:
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %h, label %x
x:
  ret void
}
",
    );
    assert!(changed);
    assert!(!text.contains("b1:") && !text.contains("br label %b1"), "{text}");
}

/// A loop with nothing constant and no chain is left alone.
#[test]
fn a_loop_with_nothing_to_fold_is_left_alone() {
    let (_, changed) = simplified(
        "define void @f(i16 %n) {
b0:
  br label %h
h:
  %i = phi i16 [ 0, %b0 ], [ %i1, %h ]
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %h, label %x
x:
  ret void
}
",
    );
    assert!(!changed);
}

/// A dominator tree and loop forest were made for every loop twice over, even
/// where nothing folded: LoopSimplifyCFG took 10.1 M Ir on a d_faces body
/// against the pin's 0.6 M. One of each per run is enough where nothing
/// changes.
#[test]
fn the_analyses_are_made_once_where_no_loop_changes() {
    let mut text = String::from("define void @f(i16 %n) {\nb0:\n  br label %h0\n");
    for k in 0..3 {
        let pre = if k == 0 { "b0".to_string() } else { format!("h{}", k - 1) };
        let next = if k == 2 { "x".to_string() } else { format!("h{}", k + 1) };
        text.push_str(&format!(
            "h{k}:\n  %i{k} = phi i16 [ 0, %{pre} ], [ %j{k}, %h{k} ]\n  %j{k} = add i16 %i{k}, 1\n  %c{k} = icmp slt i16 %j{k}, %n\n  br i1 %c{k}, label %h{k}, label %{next}\n"
        ));
    }
    text.push_str("x:\n  ret void\n}\n");
    let mut module = parsed(&text);
    let (context, function) = module.function_mut("f").expect("@f");
    let before = llrm_mir::counts::get(llrm_mir::counts::Event::Dominators);
    let changed = loop_simplify_cfg(context, function);
    let built = llrm_mir::counts::get(llrm_mir::counts::Event::Dominators) - before;
    assert!(!changed);
    assert_eq!(built, 1, "one tree for three loops that do not change");
}
