//! Adapted from llrm-core's `optimize/loopclone_tests.rs`, the port of
//! `tests/test_loopclone.py`, each body now MIR text. `is None` is `Ok(None)`
//! and the function unchanged.
//!
//! `test_peeling_clones_pointer_identity_and_seed_facts` is
//! `test_clones_keep_their_originals_metadata`: those facts are metadata.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::module::{Module, Operand};

use super::peeled;
use crate::testing::{f, parsed, printed, results};

/// A loop whose body is a diamond, with an early exit from one arm.
const DIAMOND: &str = "define i16 @f(i16 %seed, i16 %n, i16 %limit) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b5 ]
  %go = icmp ult i16 %carried, %n
  br i1 %go, label %b2, label %b6

b2:
  %bit = and i16 %carried, 1
  %even = icmp eq i16 %bit, 0
  br i1 %even, label %b3, label %b4

b3:
  %left = add i16 %carried, 3
  %big = icmp ugt i16 %left, %limit
  br i1 %big, label %b6, label %b5

b4:
  %right = add i16 %carried, 1
  br label %b5

b5:
  %selected = phi i16 [ %left, %b3 ], [ %right, %b4 ]
  %stepped = add i16 %selected, 1
  br label %b1

b6:
  %answer = phi i16 [ %carried, %b1 ], [ %left, %b3 ]
  ret i16 %answer
}
";

const INPUTS: &[&[i128]] = &[&[0, 0, 100], &[0, 20, 100], &[1, 20, 9], &[4, 5, 6], &[30, 20, 100]];

fn only_loop(module: &mut Module) -> loops::Loop {
    let function = f(module);
    let found = loops::loops(&cfg::graph(function), function.entry().map(cfg::id));
    assert_eq!(found.len(), 1);
    found.into_iter().next().unwrap()
}

/// `text` with `count` iterations peeled, which must go through.
fn peel(
    text: &str,
    count: i64,
) -> Module {
    let mut module = parsed(text);
    let loop_ = only_loop(&mut module);
    peeled(f(&mut module), &loop_, count).unwrap().expect("peeled");
    module
}

/// `text`, refused and left as it is.
fn refused(text: &str) {
    let mut module = parsed(text);
    let before = printed(&module);
    let loop_ = only_loop(&mut module);
    assert_eq!(peeled(f(&mut module), &loop_, 1).unwrap(), None);
    assert_eq!(printed(&module), before);
}

/// Each phi names its block's predecessors and each value dominates its
/// uses, as the verifier checks; the residual loop is the original one.
#[test]
fn test_peeling_clones_diamond_and_early_exit_phis() {
    let mut module = peel(DIAMOND, 2);
    let text = printed(&module);
    assert_eq!(f(&mut module).layout().len(), 7 + 2 * 5);
    assert!(text.contains("b0:\n  br label %0\n"), "{text}");
    assert!(text.contains("\n0:\n  %1 = phi i16 [ %seed, %b0 ]\n"), "{text}");
    assert!(text.contains("b1:\n  %carried = phi i16 [ %stepped, %b5 ], [ %"), "{text}");
    let answer = text.lines().find(|line| line.contains("%answer = phi")).unwrap();
    assert_eq!(answer.matches("[ ").count(), 6, "{answer}");
    let found = only_loop(&mut module);
    assert_eq!(cfg::block(found.header), f(&mut module).layout()[1]);
    assert_eq!(results(&module, INPUTS), results(&parsed(DIAMOND), INPUTS));
}

/// The clones read their own copies; the originals are as they were but
/// for the header, which the last copy now enters.
#[test]
fn test_clones_read_their_own_values() {
    let before = parsed(DIAMOND);
    let mut module = peel(DIAMOND, 1);
    let function = f(&mut module);
    let originals = before.functions().next().unwrap().2.layout().to_vec();
    let defined = originals[1..6]
        .iter()
        .flat_map(|&block| function.block(block).instructions().to_vec())
        .filter_map(|inst| function.instruction(inst).result)
        .collect::<BTreeSet<_>>();
    for &block in function.layout().iter().filter(|block| !originals.contains(block)) {
        for &inst in function.block(block).instructions() {
            assert!(
                !function
                    .instruction(inst)
                    .operands
                    .iter()
                    .any(|operand| matches!(operand, Operand::Value(value) if defined.contains(value)))
            );
        }
    }
    let text = printed(&module);
    let body = |text: &str| text[text.find("b2:").unwrap()..text.find("b6:").unwrap()].to_owned();
    assert_eq!(body(&text), body(&printed(&before)));
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS));
}

#[test]
fn test_unclosed_loop_value_is_refused() {
    refused(&DIAMOND.replace("  ret i16 %answer", "  %escape = add i16 %carried, 1\n  ret i16 %escape"));
}

#[test]
fn test_nonpositive_peel_count_is_rejected() {
    for count in [0, -1] {
        let mut module = parsed(DIAMOND);
        let loop_ = only_loop(&mut module);
        assert!(peeled(f(&mut module), &loop_, count).is_err());
    }
}

/// The loop's preheader must fall into it: one with another way out is
/// refused.
#[test]
fn test_a_loop_entered_by_a_conditional_branch_is_refused() {
    refused(
        &DIAMOND
            .replace("b0:\n  br label %b1", "b0:\n  %none = icmp eq i16 %n, 0\n  br i1 %none, label %b7, label %b1")
            .replace("  ret i16 %answer\n}", "  ret i16 %answer\n\nb7:\n  ret i16 0\n}"),
    );
}

/// A loop with two latches is refused.
#[test]
fn test_a_loop_with_two_latches_is_refused() {
    refused(&DIAMOND.replace("[ %stepped, %b5 ]", "[ %stepped, %b5 ], [ %right, %b4 ]").replace(
        "  %right = add i16 %carried, 1\n  br label %b5",
        "  %right = add i16 %carried, 1\n  %odd = icmp ult i16 %right, 8\n  br i1 %odd, label %b1, label %b5",
    ));
}

/// The old opaque dispatch: a multi-way terminator other than `br` and
/// `switch`, here an `invoke`.
#[test]
fn test_opaque_dispatch_is_not_cloned_as_an_ordinary_branch() {
    refused(&format!(
        "declare void @g()\n\ndeclare i32 @personality(...)\n\n{}",
        DIAMOND
            .replace(
                "define i16 @f(i16 %seed, i16 %n, i16 %limit) {",
                "define i16 @f(i16 %seed, i16 %n, i16 %limit) personality ptr @personality {"
            )
            .replace("  br i1 %even, label %b3, label %b4", "  invoke void @g() to label %b3 unwind label %b4")
            .replace("b4:\n", "b4:\n  %pad = landingpad { ptr, i32 } cleanup\n")
    ));
}

#[test]
fn test_clones_keep_their_originals_metadata() {
    let text = format!(
        "{}\n!0 = !{{!\"fact\"}}\n",
        DIAMOND.replace("%left = add i16 %carried, 3", "%left = add i16 %carried, 3, !fact !0")
    );
    let mut module = peel(&text, 2);
    assert_eq!(printed(&module).matches(", !fact !0").count(), 3);
    let function = f(&mut module);
    assert_eq!(function.walk().filter(|&(_, inst)| !function.instruction(inst).metadata.is_empty()).count(), 3);
}

/// A `switch` in the body is cloned with every case retargeted.
#[test]
fn test_peeling_clones_a_switch_and_keeps_its_results() {
    let text = "define i16 @f(i16 %seed, i16 %n, i16 %limit) {
b0:
  br label %b1

b1:
  %i = phi i16 [ %seed, %b0 ], [ %next, %b4 ]
  %sum = phi i16 [ 0, %b0 ], [ %total, %b4 ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %b6, label %b5

b6:
  %low = and i16 %i, 3
  switch i16 %low, label %b3 [ i16 0, label %b2
                               i16 1, label %b5 ]

b2:
  br label %b4

b3:
  br label %b4

b4:
  %add = phi i16 [ %limit, %b2 ], [ %low, %b3 ]
  %total = add i16 %sum, %add
  %next = add i16 %i, 1
  br label %b1

b5:
  %out = phi i16 [ %sum, %b1 ], [ %sum, %b6 ]
  ret i16 %out
}
";
    let module = peel(text, 2);
    assert_eq!(printed(&module).matches("switch i16").count(), 3);
    let inputs: &[&[i128]] = &[&[0, 20, 10], &[2, 20, 7], &[3, 4, 1], &[17, 20, 5], &[400, 20, 3]];
    assert_eq!(results(&module, inputs), results(&parsed(text), inputs));
}
