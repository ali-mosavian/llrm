//! Adapted from llrm-codegen's `optimize/lcssamerges_tests.rs`, the port of
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

/// qcport host -Os: an exit entered by a switch's two cases got an exit phi
/// with one input for its two edges, which the verifier refused.
#[test]
fn test_a_switch_with_two_cases_to_one_exit_gives_its_phi_two_inputs() {
    let text = MULTIPLE_EXITS.replace(
        "  br i1 %more, label %b2, label %b3",
        "  switch i16 %carried, label %b2 [\n    i16 5, label %b3\n    i16 9, label %b3\n  ]",
    );
    assert!(closed_once(&text).contains("%carried.lcssa = phi i16 [ %carried, %b1 ], [ %carried, %b1 ]"));
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
    let text = MULTIPLE_EXITS
        .replace("b0:\n  br label %b1", "b0:\n  %c = icmp eq i16 %n, 0\n  br i1 %c, label %b1, label %b5");
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
    assert!(closed_once(&text).contains(
        "  %carried.lcssa2 = phi i16 [ %carried.lcssa, %b3 ], [ %carried.lcssa1, %b4 ], [ %carried.lcssa2, %b5 ]\n"
    ));
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
        .replace("b5:\n  %answer = add i16 %carried, 7", "b5:\n  %answer = phi i16 [ %carried, %b3 ], [ %seed, %b4 ]");
    let mut module = parsed(&text);
    let before = printed(&module);
    assert!(!closed(f(&mut module)).unwrap());
    assert_eq!(printed(&module), before);
}

/// `MULTIPLE_EXITS` with `values` loop values read after the join, then
/// `phis` phis of a block that reads nothing from the loop.
fn many_values_and_phis(
    values: usize,
    phis: usize,
) -> String {
    let defs = (0..values).map(|k| format!("  %v{k} = add i16 %carried, {k}\n")).collect::<String>();
    let uses = (0..values).map(|k| format!("  %u{k} = add i16 %v{k}, 1\n")).collect::<String>();
    let others = (0..phis).map(|k| format!("  %w{k} = phi i16 [ %answer, %b6 ], [ {k}, %b7 ]\n")).collect::<String>();
    MULTIPLE_EXITS
        .replace("  %more = icmp", &format!("{defs}  %more = icmp"))
        .replace(
            "  %answer = add i16 %carried, 7\n  ret i16 %answer\n",
            &format!(
                "  %answer = add i16 %carried, 7\n{uses}  %flag = icmp eq i16 %answer, 0\n  br i1 %flag, label %b6, label %b7\n\nb6:\n  br label %b8\n\nb7:\n  br label %b8\n\nb8:\n{others}  ret i16 %answer\n"
            ),
        )
}

/// A merged value rewrote every phi of the function to find the ones that
/// read it: each of 8 more values over 2 000 phis of an unrelated block cost
/// 2 000 phi reads (about 130 K instructions per value on qbq's d_faces -O2).
/// The work a value adds is that of the phis that read it, not of the function.
#[test]
fn test_merging_a_value_does_not_scan_the_phis_of_unrelated_blocks() {
    let least = |values: usize| {
        let text = many_values_and_phis(values, 2000);
        (0..3)
            .map(|_| {
                let mut module = parsed(&text);
                llrm_support::debug::work_of(|| closed(f(&mut module)).unwrap()).1 as f64
            })
            .fold(f64::MAX, f64::min)
    };
    let (one, nine) = (least(1), least(9));
    let each = (nine - one) / 8.0;
    assert!(each < one / 4.0, "one value {one:.0} work, nine {nine:.0}: {each:.0} for each more");
}

/// `loops` multi-exit loops one after another, each read after its join.
fn chained_multi_exit_loops(loops: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %r0, i16 %n, i16 %m) {\n");
    for k in 0..loops {
        let next = k + 1;
        text += &format!(
            "pre{k}:\n  br label %a{k}\n\na{k}:\n  %c{k} = phi i16 [ %r{k}, %pre{k} ], [ %s{k}, %b{k} ]\n  %more{k} = icmp ult i16 %c{k}, %n\n  br i1 %more{k}, label %b{k}, label %x{k}\n\nb{k}:\n  %s{k} = add i16 %c{k}, 1\n  %stay{k} = icmp ult i16 %s{k}, %m\n  br i1 %stay{k}, label %a{k}, label %y{k}\n\nx{k}:\n  br label %j{k}\n\ny{k}:\n  br label %j{k}\n\nj{k}:\n  %r{next} = add i16 %c{k}, 7\n  br label %pre{next}\n\n"
        );
    }
    text + &format!("pre{loops}:\n  ret i16 %r{loops}\n}}\n")
}

/// Every multi-exit loop rebuilt the set of reachable blocks, so closing a
/// function was loops times blocks: 6 000 instructions of memory_expansion's
/// cost 70 M instructions in lcssa (20 times a linear pass). Four times the
/// loops is about four times the work.
#[test]
fn test_closing_more_loops_costs_in_proportion_to_the_loops() {
    let least = |loops: usize| {
        let text = chained_multi_exit_loops(loops);
        (0..3)
            .map(|_| {
                let mut module = parsed(&text);
                llrm_support::debug::work_of(|| closed(f(&mut module)).unwrap()).1 as f64
            })
            .fold(f64::MAX, f64::min)
    };
    let (few, many) = (least(100), least(400));
    assert!(many < few * 6.0, "100 loops {few:.0} work, 400 loops {many:.0}: {:.1} times", many / few);
}
