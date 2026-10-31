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
fn same(
    text: &str,
    inputs: &[&[i128]],
) -> String {
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

/// copy1d's first fill loop after LSR: the i16 counter was folded into the
/// i32 value counter, so the exit test reads `trunc` of it.
const TRUNC_EXIT: &str = "@g = addrspace(1) global [3000 x i8] zeroinitializer

define i32 @f() {
start:
  %a = addrspacecast ptr addrspace(1) @g to ptr addrspace(3)
  br label %l

l:
  %p = phi ptr addrspace(3) [ %a, %start ], [ %p.next, %l ]
  %v = phi i32 [ 0, %start ], [ %v.next, %l ]
  store i32 %v, ptr addrspace(3) %p
  %v.next = add nuw nsw i32 %v, 1
  %t = trunc i32 %v.next to i16
  %p.next = getelementptr i8, ptr addrspace(3) %p, i32 4
  %c = icmp eq i16 %t, 700
  br i1 %c, label %done, label %l

done:
  ret i32 0
}
";

/// copy1d's first fill, 16 instructions a trip: LSR counted it by `trunc` of
/// the stored i32, the induction proof saw no count through the truncation,
/// and the huge pointer's step kept its carry (`sbb; and; add`) on every
/// trip, where the other loops of the file were split into windows.
#[test]
fn test_a_walk_counted_by_a_truncation_of_its_counter_is_split_into_windows() {
    let got = same(TRUNC_EXIT, &[&[]]);
    assert!(got.contains("windows:") && got.contains("phi ptr addrspace(1)"), "{got}");
}

/// A 16-bit pointer of a target whose indexed mode adds `bits`.
const SMALL: &str = "target datalayout = \"e-p:16:16-i32:16-n8:16\"\n\n";

/// Bytes copied from a buffer of 600 filled with its own index to a second,
/// by two pointers stepped by one, as many as `%n`; the answer folds the
/// copy. The lengths below cross the 256 and 512 byte windows.
const COPY: &str = "define i16 @f(i16 %n) {
entry:
  %a = alloca [600 x i8]
  %b = alloca [600 x i8]
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %i2, %fill ]
  %p = getelementptr inbounds i8, ptr %a, i16 %i
  %v = trunc i16 %i to i8
  store i8 %v, ptr %p
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 600
  br i1 %c, label %fill, label %pre
pre:
  br label %copy
copy:
  %sp = phi ptr [ %a, %pre ], [ %sp2, %body ]
  %dp = phi ptr [ %b, %pre ], [ %dp2, %body ]
  %k = phi i16 [ %n, %pre ], [ %k2, %body ]
  %more = icmp ne i16 %k, 0
  br i1 %more, label %body, label %sum
body:
  %x = load i8, ptr %sp
  store i8 %x, ptr %dp
  %sp2 = getelementptr inbounds i8, ptr %sp, i16 1
  %dp2 = getelementptr inbounds i8, ptr %dp, i16 1
  %k2 = sub nuw i16 %k, 1
  br label %copy
sum:
  %j = phi i16 [ 0, %copy ], [ %j2, %sum ]
  %t = phi i16 [ 0, %copy ], [ %t2, %sum ]
  %q = getelementptr inbounds i8, ptr %b, i16 %j
  %y = load i8, ptr %q
  %w = zext i8 %y to i16
  %t3 = mul i16 %t, 3
  %t2 = add i16 %t3, %w
  %j2 = add i16 %j, 1
  %d = icmp ult i16 %j2, 600
  br i1 %d, label %sum, label %done
done:
  ret i16 %t
}
";

fn paged(
    text: &str,
    bits: Option<u32>,
) -> String {
    paged_for(text, bits, false)
}

fn paged_for(
    text: &str,
    bits: Option<u32>,
    size: bool,
) -> String {
    let mut module = parsed(&format!("{SMALL}{text}"));
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Window { size });
    manager.run_module(&mut module, Rc::new(Tuned { index_bits: bits, ..Tuned::default() })).unwrap();
    printed(&module)
}

const LENGTHS: &[&[i128]] = &[&[0], &[1], &[2], &[255], &[256], &[257], &[300], &[511], &[512], &[513], &[599]];

/// Where the target indexes by a byte, a pointer walk of unknown length is
/// split into windows of 256 trips: one index counts a window, the pointers
/// stay fixed inside it. The copy is the same at every length around the
/// windows' edges.
#[test]
fn test_a_pointer_walk_runs_on_one_page_index() {
    let before = parsed(&format!("{SMALL}{COPY}"));
    let got = paged(COPY, Some(8));
    assert_eq!(results(&parsed(&got), LENGTHS), results(&before, LENGTHS), "{got}");
    assert!(got.contains("windows:") && got.contains("phi i8"), "{got}");
    assert!(!got.contains("%sp2") && !got.contains("%dp2"), "a pointer still steps:\n{got}");
}

/// The window is as wide as the index the target states: a 4-bit index
/// wraps every 16 bytes. With a byte hard-coded the index stayed `i8`.
#[test]
fn test_the_window_is_as_wide_as_the_targets_index() {
    let before = parsed(&format!("{SMALL}{COPY}"));
    let got = paged(COPY, Some(4));
    assert_eq!(results(&parsed(&got), LENGTHS), results(&before, LENGTHS), "{got}");
    assert!(got.contains("phi i4") && !got.contains("phi i8"), "{got}");
}

/// A target with no narrow index is left alone: the same text, unsplit.
#[test]
fn test_a_target_with_no_index_register_keeps_its_pointers() {
    let got = paged(COPY, None);
    assert!(!got.contains("windows:") && got.contains("%sp2"), "{got}");
}

/// The same copy as a loop tested after each trip, behind its guard, with the
/// pointer past the last byte read after it (lz4's literal and match copies,
/// which hand the next token its source). The answer adds that pointer's
/// offset to the fold of the copy.
const COPY_ON: &str = "define i16 @f(i16 %n) {
entry:
  %a = alloca [600 x i8]
  %b = alloca [600 x i8]
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %i2, %fill ]
  %p = getelementptr inbounds i8, ptr %a, i16 %i
  %v = trunc i16 %i to i8
  store i8 %v, ptr %p
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 600
  br i1 %c, label %fill, label %guard
guard:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %pre
pre:
  br label %copy
copy:
  %sp = phi ptr [ %a, %pre ], [ %sp2, %copy ]
  %dp = phi ptr [ %b, %pre ], [ %dp2, %copy ]
  %k = phi i16 [ %n, %pre ], [ %k2, %copy ]
  %x = load i8, ptr %sp
  store i8 %x, ptr %dp
  %sp2 = getelementptr inbounds i8, ptr %sp, i16 1
  %dp2 = getelementptr inbounds i8, ptr %dp, i16 1
  %k2 = sub nuw i16 %k, 1
  %d = icmp ne i16 %k2, 0
  br i1 %d, label %copy, label %out
out:
  br label %done
done:
  %e = phi ptr [ %b, %guard ], [ %dp2, %out ]
  %ei = ptrtoint ptr %e to i16
  %bi = ptrtoint ptr %b to i16
  %off = sub i16 %ei, %bi
  br label %sum
sum:
  %j = phi i16 [ 0, %done ], [ %j2, %sum ]
  %t = phi i16 [ %off, %done ], [ %t2, %sum ]
  %q = getelementptr inbounds i8, ptr %b, i16 %j
  %y = load i8, ptr %q
  %w = zext i8 %y to i16
  %t3 = mul i16 %t, 3
  %t2 = add i16 %t3, %w
  %j2 = add i16 %j, 1
  %m = icmp ult i16 %j2, 600
  br i1 %m, label %sum, label %end
end:
  ret i16 %t
}
";

/// A pointer read after the loop (the one past the last byte) is no reason to
/// keep the pointer stepping: the exit takes the window's end. With the
/// reader refused, lz4's copies kept their 16-bit counters and pointer steps.
#[test]
fn test_a_pointer_read_after_the_walk_is_the_windows_end() {
    let before = parsed(&format!("{SMALL}{COPY_ON}"));
    let got = paged(COPY_ON, Some(8));
    assert_eq!(results(&parsed(&got), LENGTHS), results(&before, LENGTHS), "{got}");
    assert!(got.contains("windows:"), "{got}");
    assert!(!got.contains("%sp2") && !got.contains("%dp2"), "a pointer still steps:\n{got}");
}

/// Priced in bytes (-Os) the windows' setup always exceeds what one index
/// saves, so no loop is split: the copy keeps its pointers and its count.
#[test]
fn test_a_size_build_does_not_window_a_pointer_walk() {
    let got = paged_for(COPY, Some(8), true);
    assert!(!got.contains("windows:") && got.contains("%sp2"), "{got}");
}
