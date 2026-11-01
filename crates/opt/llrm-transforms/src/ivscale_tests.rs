//! A counter read only as `i << k` counts in the shifted unit.

use crate::ivscale::IvScale;
use crate::testing::{parsed, printed, results};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

/// A sum over `half` words of a buffer addressed by the byte offset `j << 2`.
/// The bound is `(n & mask) | 1`, so it lies in `[1, mask | 1]`; the counter
/// starts at `start`, `second` is a second read of it, `pred` the exit test of
/// `j + 1`.
fn butterfly(
    start: i64,
    mask: i64,
    pred: &str,
    second: &str,
) -> String {
    format!(
        "define i32 @f(i16 %n) {{
entry:
  %buf = alloca [32768 x i8], align 2
  %a = and i16 %n, {mask}
  %half = or i16 %a, 1
  br label %loop
loop:
  %j = phi i16 [ {start}, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %o = shl nuw nsw i16 %j, 2
  %p = getelementptr i8, ptr %buf, i16 %o
  %v = load i32, ptr %p, align 2
  %s2 = add nsw i32 %v, %s
{second}  %next = add nuw nsw i16 %j, 1
  %c = icmp {pred} i16 %next, %half
  br i1 %c, label %done, label %loop
done:
  ret i32 %s2
}}
"
    )
}

/// `text` before and after the pass, on `target`.
fn run(
    text: &str,
    target: std::rc::Rc<dyn llrm_mir::target::Machine>,
) -> (String, String) {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let before = printed(&module);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IvScale { size: false });
    manager.run_module(&mut module, target).unwrap();
    (before, printed(&module))
}

fn m16(text: &str) -> (String, String) {
    run(text, std::rc::Rc::new(llrm_x86_m16::Dos::default()))
}

/// The same result for each input before and after.
fn agree(
    before: &str,
    after: &str,
) {
    let inputs: &[&[i128]] = &[&[0], &[1], &[3], &[100], &[127]];
    assert_eq!(results(&parsed(before), inputs), results(&parsed(after), inputs), "{after}");
}

/// The butterfly loop of c's `stft_single` ran `mov bx,ax; shl bx,2` every
/// trip and held both (+22M clocks over main). The counter is the byte offset
/// now: step 4, the bound shifted in the preheader, no shift in the loop.
#[test]
fn the_counter_is_the_byte_offset() {
    let (before, text) = m16(&butterfly(0, 127, "eq", ""));
    agree(&before, &text);
    assert!(text.contains("add i16 %j, 4"), "{text}");
    assert!(text.contains("shl nuw i16 %half, 2"), "{text}");
    assert!(!text.contains("shl nuw nsw i16 %j, 2"), "{text}");
    assert!(text.contains("getelementptr i8, ptr %buf, i16 %j"), "{text}");
}

/// A start above zero is shifted too (bound at least start + 1).
#[test]
fn a_start_is_shifted_with_the_counter() {
    let (before, text) = m16(&butterfly(1, 127, "eq", ""));
    // The bound can be 1, below start + 1: the original wraps, the scaled one
    // would not.
    assert_eq!(before, text);
}

/// A bound the analysis cannot limit may wrap its shift, which exits early.
#[test]
fn an_unlimited_bound_keeps_the_counter() {
    let (before, text) = m16(&butterfly(0, -1, "eq", ""));
    assert_eq!(before, text);
}

/// The shifted bound must fit: 16383 << 2 is the largest, 16384 wraps to 0.
#[test]
fn a_bound_at_the_period_keeps_the_counter() {
    let (_, text) = m16(&butterfly(0, 16383, "eq", ""));
    assert!(text.contains("add i16 %j, 4"), "{text}");
    let (before, text) = m16(&butterfly(0, 16384, "eq", ""));
    assert_eq!(before, text, "a bound of 16385 shifts to 4");
}

/// An ordered exit test reads the counter's order, which scaling wraps.
#[test]
fn an_ordered_exit_keeps_the_counter() {
    let (before, text) = m16(&butterfly(0, 127, "ult", ""));
    assert_eq!(before, text);
}

/// A second reader of the counter sees the unshifted value.
#[test]
fn a_second_use_of_the_counter_keeps_it() {
    let (before, text) = m16(&butterfly(0, 127, "eq", "  %w = zext i16 %j to i32\n  %s3 = add i32 %w, %s2\n"));
    assert_eq!(before, text);
}

/// Shifts by different amounts are two scales of one counter.
#[test]
fn shifts_of_two_sizes_keep_the_counter() {
    let (before, text) =
        m16(&butterfly(0, 127, "eq", "  %q = shl i16 %j, 1\n  %r = zext i16 %q to i32\n  %s3 = add i32 %r, %s2\n"));
    assert_eq!(before, text);
}

/// A target with no pricing of the scaled index keeps the counter.
#[test]
fn a_target_without_the_cost_keeps_the_counter() {
    let (before, text) = run(&butterfly(0, 127, "eq", ""), std::rc::Rc::new(llrm_mir::target::Neutral));
    assert_eq!(before, text);
}

/// stft_single's shape: the bound is `m >> 1` of an outer counter that
/// doubles from 2 while it stays within 256, so it lies in `[1, 128]`.
const DOUBLING: &str = "define i32 @f(i16 %n) {
entry:
  %buf = alloca [1024 x i8], align 2
  br label %outer
outer:
  %m = phi i16 [ 2, %entry ], [ %m2, %latch ]
  %t = phi i32 [ 0, %entry ], [ %t2, %latch ]
  %half = lshr i16 %m, 1
  br label %loop
loop:
  %j = phi i16 [ 0, %outer ], [ %next, %loop ]
  %s = phi i32 [ %t, %outer ], [ %s2, %loop ]
  %o = shl i16 %j, 2
  %p = getelementptr i8, ptr %buf, i16 %o
  %v = load i32, ptr %p, align 2
  %s2 = add nsw i32 %v, %s
  %next = add nuw nsw i16 %j, 1
  %c = icmp eq i16 %half, %next
  br i1 %c, label %latch, label %loop
latch:
  %t2 = add i32 %s2, 1
  %m2 = shl nuw nsw i16 %m, 1
  %again = icmp samesign ult i16 %m2, 257
  br i1 %again, label %outer, label %done
done:
  ret i32 %t2
}
";

/// The doubling bound's range comes from the outer loop's trip count (8).
#[test]
fn a_bound_halved_from_a_doubling_counter_is_limited() {
    let (before, text) = m16(DOUBLING);
    agree(&before, &text);
    assert!(text.contains("add i16 %j, 4"), "{text}");
}

/// At -Os the step `add 4` costs the bytes the shift saves.
#[test]
fn a_function_optimized_for_size_keeps_the_counter() {
    let mut module = parsed(&format!("{HEAD}{}", butterfly(0, 127, "eq", "")));
    let before = printed(&module);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IvScale { size: true });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    assert_eq!(before, printed(&module));
}

/// A bound the loop changes is not the shifted bound made once.
#[test]
fn a_bound_the_loop_changes_keeps_the_counter() {
    let text = butterfly(0, 127, "eq", "")
        .replace("  %s = phi", "  %k = phi i16 [ 1, %entry ], [ %k3, %loop ]\n  %s = phi")
        .replace(
            "  %c = icmp eq i16 %next, %half",
            "  %k2 = and i16 %k, 127\n  %k3 = or i16 %k2, 1\n  %c = icmp eq i16 %next, %k3",
        );
    let (before, after) = m16(&text);
    assert_eq!(before, after);
}

/// `DOUBLING` with `outer` also making `extra` of the bound and `latch`
/// summing `use_` into the result.
fn doubling_with(
    outer: &str,
    use_: &str,
) -> String {
    DOUBLING
        .replace("  br label %loop\n", &format!("{outer}  br label %loop\n"))
        .replace("  %t2 = add i32 %s2, 1\n", &format!("{use_}  %t2 = add i32 %s2, 1\n"))
        .replace("  ret i32 %t2", "  ret i32 %t2")
}

/// stft_single (BASIC) kept `half > 0` as the loop guard: with the bound and
/// the shifted bound both live across the enclosing loop the frame grew 2 B
/// and a reload came in (+15 B). The range (1..=128) decides the guard, so it
/// folds and the bound is dead past its shift.
#[test]
fn a_guard_the_range_decides_is_folded_with_the_rewrite() {
    let text =
        doubling_with("  %g = icmp sgt i16 %half, 0\n", "  %z = select i1 %g, i32 %s2, i32 0\n  %s3 = add i32 %z, 1\n");
    let (before, after) = m16(&text);
    agree(&before, &after);
    assert!(after.contains("add i16 %j, 4"), "{after}");
    assert!(!after.contains("icmp sgt"), "{after}");
}

/// A `shl B, k` already made is the shifted bound: no second one is made.
#[test]
fn a_shift_of_the_bound_already_made_is_the_shifted_bound() {
    let text = doubling_with("  %q = shl i16 %half, 2\n", "  %w = zext i16 %q to i32\n  %s3 = add i32 %w, %s2\n");
    let (before, after) = m16(&text);
    agree(&before, &after);
    assert!(after.contains("add i16 %j, 4"), "{after}");
    assert_eq!(after.matches("shl").count(), 2, "one for the doubling, one for the bound: {after}");
}

/// Another reader of the bound keeps it live beside its shift: left alone.
#[test]
fn a_bound_another_instruction_reads_keeps_the_counter() {
    let text = doubling_with("", "  %w = zext i16 %half to i32\n  %s3 = add i32 %w, %s2\n");
    let (before, after) = m16(&text);
    assert_eq!(before, after);
}

/// The guard of BASIC's loop (`if half > 0`) was a branch: once the range
/// decides it, no branch on a constant is left to cost a `test`/`jne`.
#[test]
fn a_branch_on_a_folded_guard_is_removed() {
    let text = DOUBLING
        .replace(
            "  br label %loop\n",
            "  br label %guard\nguard:\n  %g = icmp sgt i16 %half, 0\n  br i1 %g, label %pre, label %latch\npre:\n  br label %loop\n",
        )
        .replace("[ 0, %outer ], [ %next", "[ 0, %pre ], [ %next")
        .replace("[ %t, %outer ]", "[ %t, %pre ]")
        .replace("br i1 %c, label %latch, label %loop", "br i1 %c, label %exit, label %loop")
        .replace("latch:\n", "exit:\n  br label %latch\nlatch:\n  %s4 = phi i32 [ 0, %guard ], [ %s2, %exit ]\n")
        .replace("%t2 = add i32 %s2, 1", "%t2 = add i32 %s4, 1");
    let (before, after) = m16(&text);
    agree(&before, &after);
    assert!(after.contains("add i16 %j, 4"), "{after}");
    assert!(!after.contains("br i1 %g") && !after.contains("br i1 true"), "{after}");
}
