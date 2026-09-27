//! Adapted from llrm-core's `optimize/lcssamerges_tests.rs`, the port of
//! `tests/test_lcssa_merges.py`, each body now MIR text; `==` on bodies is
//! printed text.

use crate::lcssa::closed;
use crate::testing::{f, parsed, printed, results};

/// `loop_with_exit_use` leaving from its latch too, both exits joining
/// before the use.
const MULTIPLE_EXITS: &str = "define i16 @f(i16 %seed, i16 %n, i16 %m) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  %stay = icmp ult i16 %stepped, %m
  br i1 %stay, label %b1, label %b4

b3:
  br label %b5

b4:
  br label %b5

b5:
  %answer = add i16 %carried, 7
  ret i16 %answer
}
";

/// `text` closed, printed; and closed again, unchanged.
fn closed_once(text: &str) -> String {
    let mut module = parsed(text);
    assert!(closed(f(&mut module)).unwrap());
    let once = printed(&module);
    assert!(!closed(f(&mut module)).unwrap(), "closing is idempotent");
    assert_eq!(printed(&module), once);
    once
}

#[test]
fn test_distinct_exits_merge_before_the_downstream_use() {
    assert!(closed_once(MULTIPLE_EXITS).ends_with(
        "b3:
  %carried.lcssa = phi i16 [ %carried, %b1 ]
  br label %b5

b4:
  %carried.lcssa1 = phi i16 [ %carried, %b2 ]
  br label %b5

b5:
  %carried.lcssa2 = phi i16 [ %carried.lcssa, %b3 ], [ %carried.lcssa1, %b4 ]
  %answer = add i16 %carried.lcssa2, 7
  ret i16 %answer
}
"
    ));
}

#[test]
fn test_bypass_phi_keeps_its_non_loop_input() {
    let text = MULTIPLE_EXITS
        .replace("define i16 @f(i16 %seed, i16 %n, i16 %m)", "define i16 @f(i16 %seed, i16 %n, i16 %m, i1 %c)")
        .replace("b0:\n  br label %b1", "b0:\n  br i1 %c, label %b1, label %b6")
        .replace(
            "b5:\n  %answer = add i16 %carried, 7\n  ret i16 %answer",
            "b5:\n  br label %b6\n\nb6:\n  %answer = phi i16 [ %seed, %b0 ], [ %carried, %b5 ]\n  ret i16 %answer",
        );
    assert!(closed_once(&text).ends_with(
        "b5:
  %carried.lcssa2 = phi i16 [ %carried.lcssa, %b3 ], [ %carried.lcssa1, %b4 ]
  br label %b6

b6:
  %answer = phi i16 [ %seed, %b0 ], [ %carried.lcssa2, %b5 ]
  ret i16 %answer
}
"
    ));
}

/// Not SSA in either MIR: `%carried` does not reach the use from `%b0`.
#[test]
fn test_direct_use_after_a_bypass_is_not_fabricated() {
    let text = MULTIPLE_EXITS.replace("b0:\n  br label %b1", "b0:\n  %c = icmp eq i16 %n, 0\n  br i1 %c, label %b1, label %b5");
    let mut module = parsed(&text);
    let before = llrm_mir::print::module(&module);
    assert!(!closed(f(&mut module)).unwrap());
    assert_eq!(llrm_mir::print::module(&module), before);
}

/// The verifier checks each phi names exactly its block's predecessors.
#[test]
fn test_following_cycle_keeps_complete_phi_edges() {
    let text = MULTIPLE_EXITS.replace(
        "  %answer = add i16 %carried, 7\n  ret i16 %answer",
        "  %answer = add i16 %carried, 7\n  %again = icmp ult i16 %answer, %m\n  br i1 %again, label %b5, label %b6\n\nb6:\n  ret i16 %answer",
    );
    assert!(closed_once(&text).contains("  %carried.lcssa2 = phi i16 [ %carried.lcssa, %b3 ], [ %carried.lcssa1, %b4 ], [ %carried.lcssa2, %b5 ]\n"));
}

/// Leaving by either exit, the function returns what it did.
#[test]
fn test_merged_exits_keep_what_the_function_returns() {
    let inputs: &[&[i128]] = &[&[0, 5, 100], &[0, 100, 5], &[9, 3, 4], &[2, 2, 2]];
    let expected = results(&parsed(MULTIPLE_EXITS), inputs);
    let mut module = parsed(MULTIPLE_EXITS);
    closed(f(&mut module)).unwrap();
    assert_eq!(results(&module, inputs), expected);
}

/// An exit also entered from outside the loop is no dedicated exit: the
/// loop is left for loopsimplify first.
#[test]
fn test_an_exit_shared_with_the_loop_entry_is_left_alone() {
    let text = MULTIPLE_EXITS
        .replace("define i16 @f(i16 %seed, i16 %n, i16 %m)", "define i16 @f(i16 %seed, i16 %n, i16 %m, i1 %c)")
        .replace("b0:\n  br label %b1", "b0:\n  br i1 %c, label %b1, label %b4")
        .replace(
            "b5:\n  %answer = add i16 %carried, 7",
            "b5:\n  %answer = phi i16 [ %carried, %b3 ], [ %seed, %b4 ]",
        );
    let mut module = parsed(&text);
    let before = printed(&module);
    assert!(!closed(f(&mut module)).unwrap());
    assert_eq!(printed(&module), before);
}
