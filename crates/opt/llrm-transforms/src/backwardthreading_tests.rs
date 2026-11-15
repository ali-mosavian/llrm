//! Backward threading on the shape of `queens`' column loop: a guard of the
//! inner loop that every trip through the loop already passed.

use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;

use super::BackwardThreading;
use crate::testing::{parsed, printed, results};

/// `head` tests `row > 0` every trip; `latch` joins the path through `loop`
/// (which passed the test) with the one through `hit`.
const COLUMNS: &str = "define i16 @f(i16 %row, i16 %n) {
entry:
  br label %head

head:
  %col = phi i16 [ 0, %entry ], [ %next, %latch ]
  %found = phi i16 [ 0, %entry ], [ %found2, %latch ]
  %g = icmp sgt i16 %row, 0
  br i1 %g, label %enter, label %hit

enter:
  br label %loop

loop:
  %u = icmp eq i16 %col, 3
  br i1 %u, label %latch, label %hit

hit:
  %bumped = add i16 %found, 1
  br label %latch

latch:
  %found2 = phi i16 [ %found, %loop ], [ %bumped, %hit ]
  %next = add i16 %col, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %exit, label %head

exit:
  ret i16 %found2
}
";

const INPUTS: &[&[i128]] = &[&[1, 4], &[0, 3], &[2, 5], &[-1, 6], &[1, 1]];

fn threaded(text: &str) -> (String, bool) {
    let before = parsed(text);
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.add(BackwardThreading { enabled: true });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{text}");
    let changed = printed(&before) != text;
    (text, changed)
}

#[test]
fn a_join_of_a_known_and_an_unknown_path_is_threaded_for_the_known_one() {
    let (text, changed) = threaded(COLUMNS);
    assert!(changed, "the guard is tested again after the path that passed it:\n{text}");
    // The copy of `head` that the path through `loop` takes jumps to `enter`.
    assert_eq!(text.matches("br label %enter").count(), 1, "{text}");
}
