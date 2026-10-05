//! `window` over huge pointer walks: one that fits one window, ones split
//! into several, constant and runtime counts, and one too short to pay.
//! A split walk computes what it did, by the interpreter.

use std::rc::Rc;

use llrm_analysis::testing::DOS;
use llrm_mir::passes::PassManager;

use super::Window;
use crate::testing::{Tuned, parsed, printed, results};

/// A 486's prices, as far as the split weighs them, and a window of 600
/// bytes: 100 six-byte trips, so a few hundred are several windows.
fn target() -> Tuned {
    let costs = crate::profit::OperationCosts { add: 1, branch: 3, carry: 11, carry_step: 4, ..Default::default() };
    Tuned { costs: costs.clone(), sizes: costs, window: Some((1, 600)), ..Tuned::default() }
}

/// A loop of `trips` over a huge array: it stores a dword and a word
/// through a pointer it steps by six (a stride that does not divide the
/// window), then reads the words back. `trips` may name `%n`.
fn walk(trips: &str) -> String {
    format!(
        "@g = addrspace(1) global [3000 x i8] zeroinitializer

define i16 @f(i16 %n) {{
start:
  %a = addrspacecast ptr addrspace(1) @g to ptr addrspace(3)
  br label %l

l:
  %i = phi i16 [ 0, %start ], [ %i.next, %lb ]
  %p = phi ptr addrspace(3) [ %a, %start ], [ %p.next, %lb ]
  %c = icmp slt i16 %i, {trips}
  br i1 %c, label %lb, label %sum

lb:
  store i32 7, ptr addrspace(3) %p
  %q = getelementptr i8, ptr addrspace(3) %p, i32 4
  store i16 %i, ptr addrspace(3) %q
  %p.next = getelementptr i8, ptr addrspace(3) %p, i32 6
  %i.next = add nsw i16 %i, 1
  br label %l

sum:
  br label %s

s:
  %j = phi i16 [ 0, %sum ], [ %j.next, %sb ]
  %t = phi i16 [ 0, %sum ], [ %t.next, %sb ]
  %r = phi ptr addrspace(3) [ %a, %sum ], [ %r.next, %sb ]
  %d = icmp slt i16 %j, {trips}
  br i1 %d, label %sb, label %done

sb:
  %rq = getelementptr i8, ptr addrspace(3) %r, i32 4
  %v = load i16, ptr addrspace(3) %rq
  %t.next = xor i16 %t, %v
  %r.next = getelementptr i8, ptr addrspace(3) %r, i32 6
  %j.next = add nsw i16 %j, 1
  br label %s

done:
  ret i16 %t
}}
"
    )
}

fn windowed(text: &str) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Window { size: false });
    manager.run_module(&mut module, Rc::new(target())).unwrap();
    printed(&module)
}

/// `text` windowed computes what it did for each `n`.
fn same(text: &str, inputs: &[&[i128]]) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let got = windowed(text);
    assert_eq!(results(&parsed(&got), inputs), results(&before, inputs), "{got}");
    got
}

/// A row of a 2D huge array: every step carried into the selector though
/// no step of the row can (bench/huge's inner loops). One window, no split.
#[test]
fn test_a_walk_inside_one_window_runs_on_a_far_pointer() {
    let got = same(&walk("50"), &[&[0]]);
    assert!(got.contains("call ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3) %a)"), "{got}");
    assert!(got.contains("phi ptr addrspace(1)") && !got.contains("phi ptr addrspace(3)"), "{got}");
    assert!(!got.contains("windows:"), "{got}");
}

/// 350 six-byte steps reach past the window: each step carried into the
/// selector (copy1d's 16 instructions a trip). Split, the walks run far
/// inside each window and only the windows step their huge pointers.
#[test]
fn test_a_walk_past_one_window_is_split_into_windows() {
    let got = same(&walk("350"), &[&[0]]);
    assert!(got.contains("windows:") && got.contains("phi ptr addrspace(1)"), "{got}");
    assert!(got.contains("!\"branch_weights\""), "{got}");
}

/// A count known only at run time: guarded where it runs no trip.
#[test]
fn test_a_walk_of_a_runtime_count_is_split() {
    let got = same(&walk("%n"), &[&[-3], &[0], &[1], &[5], &[99], &[100], &[101], &[350], &[500]]);
    assert!(got.contains("windows:") && got.contains("phi ptr addrspace(1)"), "{got}");
}

/// At most three trips: two windows' setup would cost more than the carries
/// the trips save, so the walk keeps its huge pointer.
#[test]
fn test_a_short_walk_of_a_runtime_count_is_not_split() {
    let text = walk("%m").replace("start:\n", "start:\n  %m = and i16 %n, 3\n");
    let got = windowed(&text);
    assert!(!got.contains("llrm.ia16.window") && got.contains("phi ptr addrspace(3)"), "{got}");
}

/// A walk down from the array's end (down1d): each window's lowest byte is
/// its last trip's, and the next window starts a step below that.
#[test]
fn test_a_walk_down_is_split_into_windows() {
    let text = walk("350")
        .replace("%a = addrspacecast ptr addrspace(1) @g to ptr addrspace(3)", "%g = addrspacecast ptr addrspace(1) @g to ptr addrspace(3)\n  %a = getelementptr i8, ptr addrspace(3) %g, i32 2994")
        .replace("i32 6\n  %i.next", "i32 -6\n  %i.next")
        .replace("i32 6\n  %j.next", "i32 -6\n  %j.next")
        .replace("%t.next = xor i16 %t, %v", "%t3 = mul i16 %t, 3\n  %t.next = add i16 %t3, %v");
    let got = same(&text, &[&[0], &[1], &[99], &[100], &[101], &[350], &[500]]);
    assert!(got.contains("windows:"), "{got}");
}
