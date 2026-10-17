//! `DsePoc` under the pass manager on bodies whose stores a read still needs.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use super::DsePoc;
use crate::testing::{parsed, printed};

/// `text` through the POC alone: @f printed.
fn kept(text: &str) -> String {
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(DsePoc);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = printed(&module);
    after[after.find("@f(").unwrap()..].to_owned()
}

fn stores(text: &str) -> usize {
    text.lines().filter(|line| line.trim_start().starts_with("store ")).count()
}

/// A phi whose block a killer dominates still carries the value an inner loop's
/// back edge brings from the candidate: that edge never meets the killer.
/// Skipping the phi removed the increment of `mandel`'s inner counter.
#[test]
fn test_a_loop_carried_store_is_read_through_a_phi_the_killer_dominates() {
    let after = kept(
        "define i32 @f(i32 %n) {
b0:
  %s = alloca i32
  %t = alloca i32
  store i32 0, ptr %t
  br label %outer

outer:
  store i32 0, ptr %s
  br label %inner

inner:
  %v = load i32, ptr %s
  %c = icmp slt i32 %v, 3
  br i1 %c, label %body, label %after

body:
  %w = add i32 %v, 1
  store i32 %w, ptr %s
  %a = load i32, ptr %t
  %a2 = add i32 %a, 1
  store i32 %a2, ptr %t
  br label %inner

after:
  %a3 = load i32, ptr %t
  %c2 = icmp slt i32 %a3, 4
  br i1 %c2, label %outer, label %done

done:
  %r = load i32, ptr %t
  ret i32 %r
}
",
    );
    assert!(after.contains("store i32 %w, ptr %s"), "{after}");
}

/// A store of two of four bytes is not the store that overwrites the four: the
/// bytes it leaves are read. Taking it as a complete killer removed the wider
/// store (`lru`, `quicksort`, `x_bubble`: wrong results).
#[test]
fn test_a_partial_overwrite_does_not_kill_the_wider_store() {
    let after = kept(
        "define i32 @f(ptr %p) {
b0:
  store i32 1, ptr %p
  store i16 2, ptr %p
  %r = load i32, ptr %p
  ret i32 %r
}
",
    );
    assert_eq!(stores(&after), 2, "{after}");
}
