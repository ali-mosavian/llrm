//! A masked counter read as `a[i & M]` is scaled to the byte offset.

use crate::maskscale::MaskScale;
use crate::testing::{parsed, printed, results};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

/// A ring sum over a 1024-slot buffer: counter `{start,+,step}`, `and mask`,
/// element `elem`, exit `icmp pred (i + step), exit`; `extra` is added in the
/// loop.
fn ring(
    start: i64,
    step: i64,
    mask: i64,
    elem: &str,
    pred: &str,
    exit: i64,
    extra: &str,
) -> String {
    format!(
        "define i32 @f() {{
entry:
  %buf = alloca [1024 x i32], align 2
  br label %loop
loop:
  %i = phi i16 [ {start}, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %m = and i16 %i, {mask}
  %p = getelementptr inbounds nuw {elem}, ptr %buf, i16 %m
  %v = load i32, ptr %p, align 2
  %s2 = add nsw i32 %v, %s
{extra}  %next = add nuw nsw i16 %i, {step}
  %c = icmp {pred} i16 %next, {exit}
  br i1 %c, label %done, label %loop
done:
  ret i32 %s2
}}
"
    )
}

/// `text` before and after the pass, on `target`; both agree on @f's result.
fn run(
    text: &str,
    target: std::rc::Rc<dyn llrm_mir::target::Machine>,
) -> (String, String) {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let before = printed(&module);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(MaskScale);
    manager.run_module(&mut module, target).unwrap();
    (before, printed(&module))
}

/// The interpreter gives the same sum before and after.
fn agree(
    before: &str,
    after: &str,
) {
    assert_eq!(results(&parsed(before), &[&[]]), results(&parsed(after), &[&[]]), "{after}");
}

fn m16(text: &str) -> (String, String) {
    run(text, std::rc::Rc::new(llrm_x86_m16::Dos::default()))
}

/// The ring loop of c's `ring` at m16 ran `add 5` and `shl 2` per trip (+12000
/// clocks over main). The counter is the byte offset now: `and 4092`, step 20,
/// exit 54476 = -11060 (30003 * 4 mod 2^16), a gep over bytes.
#[test]
fn the_ring_counter_is_the_byte_offset() {
    let (before, text) = m16(&ring(3, 5, 1023, "[4 x i8]", "eq", 30003, ""));
    agree(&before, &text);
    assert!(text.contains("and i16 %i, 4092"), "{text}");
    assert!(text.contains("add i16 %i, 20"), "{text}");
    assert!(text.contains("icmp eq i16 %next, -11060"), "{text}");
    assert!(text.contains("getelementptr inbounds nuw i8, ptr %buf, i16 %m"), "{text}");
    assert!(text.contains("[ 12, %entry ]"), "{text}");
}

/// Trips = the period of the scaled counter (2^14 at scale 4): the scaled
/// counter would exit at its first wrapped value, 16384 trips early. The
/// counter stays; one trip fewer is rewritten.
#[test]
fn a_trip_count_past_the_period_keeps_the_counter() {
    let (before, text) = m16(&ring(0, 1, 1023, "[4 x i8]", "eq", 16385, ""));
    assert_eq!(before, text, "exited at the first wrapped value");
    let (_, text) = m16(&ring(0, 1, 1023, "[4 x i8]", "eq", 16384, ""));
    assert!(text.contains("icmp eq i16 %next, 0"), "{text}");
    assert!(text.contains("and i16 %i, 4092"), "{text}");
}

/// An ordered exit test reads the counter's order, which scaling wraps.
#[test]
fn an_ordered_exit_keeps_the_counter() {
    let (before, text) = m16(&ring(3, 5, 1023, "[4 x i8]", "ult", 30003, ""));
    assert_eq!(before, text);
}

/// A second reader of the counter sees the unscaled value.
#[test]
fn a_second_use_of_the_counter_keeps_it() {
    let (before, text) = m16(&ring(3, 5, 1023, "[4 x i8]", "eq", 30003, "  %w = zext i16 %i to i32\n"));
    assert_eq!(before, text);
}

/// Step 3 from an odd start over 32-bit elements: the constants are the
/// products mod 2^16.
#[test]
fn an_odd_step_and_start_scale_by_the_element() {
    let (before, text) = m16(&ring(7, 3, 255, "i32", "eq", 3001, ""));
    agree(&before, &text);
    assert!(text.contains("and i16 %i, 1020"), "{text}");
    assert!(text.contains("[ 28, %entry ]"), "{text}");
    assert!(text.contains("add i16 %i, 12"), "{text}");
    assert!(text.contains("icmp eq i16 %next, 12004"), "{text}");
}

/// Scale 2: 16-bit elements, the constants double.
#[test]
fn a_scale_of_two_doubles_the_constants() {
    let (before, text) = m16(&ring(5, 3, 511, "i16", "eq", 2000, ""));
    agree(&before, &text);
    assert!(text.contains("and i16 %i, 1022"), "{text}");
    assert!(text.contains("[ 10, %entry ]"), "{text}");
    assert!(text.contains("add i16 %i, 6"), "{text}");
    assert!(text.contains("icmp eq i16 %next, 4000"), "{text}");
}

/// An 8-bit counter: the products are mod 2^8 (exit 40 * 4 = 160 = -96).
#[test]
fn a_narrow_counter_scales_mod_its_width() {
    let (before, text) = m16(&ring(1, 3, 15, "i32", "eq", 40, "").replace("i16", "i8"));
    agree(&before, &text);
    assert!(text.contains("and i8 %i, 60"), "{text}");
    assert!(text.contains("add i8 %i, 12"), "{text}");
    assert!(text.contains("icmp eq i8 %next, -96"), "{text}");
}

/// A target with no pricing of the scaled index keeps the counter.
#[test]
fn a_target_without_the_cost_keeps_the_counter() {
    let (before, text) =
        run(&ring(3, 5, 1023, "[4 x i8]", "eq", 30003, ""), std::rc::Rc::new(llrm_mir::target::Neutral));
    assert_eq!(before, text);
}

/// The outer loop of c's `tile` after `lsr`: `(y + 61) & 63` is an i6 counter
/// read as `zext`, `shl 7`, beside the i16 trip counter that tests the exit.
/// Real mode then ran `movzx; and; shl` per trip and two counters (+131
/// clocks, +53 instructions over main). One scaled counter holds both:
/// start 61 * 128, step 128, `and 8064`, exit (61 + 25) * 128 = 11008.
fn tile(
    extra: &str,
    exit: i64,
) -> String {
    format!(
        "define i32 @f() {{
entry:
  %buf = alloca [8192 x i8], align 2
  br label %loop
loop:
  %p = phi i6 [ -3, %entry ], [ %pn, %loop ]
  %q = phi i16 [ 0, %entry ], [ %qn, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %z = zext i6 %p to i16
  %o = shl nuw nsw i16 %z, 7
  %g = getelementptr i8, ptr %buf, i16 %o
  %v = load i16, ptr %g, align 2
  %w = sext i16 %v to i32
  %s2 = add nsw i32 %s, %w
{extra}  %qn = add nuw nsw i16 %q, 1
  %c = icmp eq i16 %qn, {exit}
  %pn = add nsw i6 %p, 1
  br i1 %c, label %done, label %loop
done:
  ret i32 %s2
}}
"
    )
}

#[test]
fn a_narrow_counter_and_the_trip_counter_become_one_scaled_counter() {
    let (before, text) = m16(&tile("", 25));
    agree(&before, &text);
    assert!(!text.contains(" i6 "), "{text}");
    assert_eq!(text.matches("phi i16").count(), 1, "{text}");
    assert!(text.contains("[ 7808, %entry ]"), "{text}");
    assert!(text.contains("and i16 %p, 8064"), "{text}");
    assert!(text.contains("icmp eq i16 %pn, 11008"), "{text}");
}

/// A second reader of the trip counter sees its unscaled value.
#[test]
fn a_second_use_of_the_trip_counter_keeps_both() {
    let (before, text) = m16(&tile("  %x = zext i16 %q to i32\n  %y = add i32 %x, %s2\n", 25));
    assert_eq!(before, text);
}

/// 600 trips of a counter that wraps every 512: the scaled counter would hit
/// the exit value on trip 88, so the pair stays.
#[test]
fn a_trip_count_past_the_scaled_period_keeps_both() {
    let (before, text) = m16(&tile("", 600));
    assert_eq!(before, text);
}

/// A narrow counter of `bits` bits read as `cast; shl shift` beside a trip
/// counter that runs `trips` times; the narrow counter steps by `step` from
/// `start`. `extra` is added in the loop.
fn narrow(
    bits: u32,
    cast: &str,
    shift: u32,
    start: i64,
    step: i64,
    trips: i64,
    extra: &str,
) -> String {
    format!(
        "define i32 @f() {{
entry:
  %buf = alloca [{size} x i8], align 2
  br label %loop
loop:
  %p = phi i{bits} [ {start}, %entry ], [ %pn, %loop ]
  %q = phi i16 [ 0, %entry ], [ %qn, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %z = {cast} i{bits} %p to i16
  %o = shl nuw nsw i16 %z, {shift}
  %g = getelementptr i8, ptr %buf, i16 %o
  %v = load i16, ptr %g, align 2
  %w = sext i16 %v to i32
  %s2 = add nsw i32 %s, %w
{extra}  %qn = add nuw nsw i16 %q, 1
  %c = icmp eq i16 %qn, {trips}
  %pn = add i{bits} %p, {step}
  br i1 %c, label %done, label %loop
done:
  ret i32 %s2
}}
",
        size = 1u32 << (bits + shift)
    )
}

/// The class, not the tile: counters of 3, 5 and 8 bits, scales 2 to 128,
/// steps 1 and 3, an odd start. One scaled counter, the same sum.
#[test]
fn narrow_counters_of_any_width_scale_and_step_merge() {
    for (bits, shift, start, step, trips) in [(3, 4, 5, 1, 20), (5, 3, 9, 3, 33), (8, 7, 200, 1, 90), (6, 1, 1, 5, 40)] {
        let (before, text) = m16(&narrow(bits, "zext", shift, start, step, trips, ""));
        agree(&before, &text);
        assert!(!text.contains(&format!("phi i{bits} ")), "{bits} {shift}: {text}");
        assert_eq!(text.matches("phi i16").count(), 1, "{bits} {shift}: {text}");
        let mask = ((1u32 << bits) - 1) << shift;
        assert!(text.contains(&format!("and i16 %p, {mask}")), "{bits} {shift}: {text}");
    }
}

/// A sign extension reads the narrow counter's sign: not a mask.
#[test]
fn a_sign_extended_narrow_counter_keeps_both() {
    let (before, text) = m16(&narrow(6, "sext", 7, 2, 1, 25, ""));
    assert_eq!(before, text);
}

/// A second reader of the narrow counter sees its narrow value.
#[test]
fn a_second_use_of_the_narrow_counter_keeps_both() {
    let (before, text) = m16(&narrow(6, "zext", 7, 2, 1, 25, "  %x = zext i6 %p to i32\n  %y = add i32 %x, %s2\n"));
    assert_eq!(before, text);
}

/// A trip counter that would not reach the scaled exit on the same trip
/// (here the narrow counter's period is shorter than the trip count) keeps both.
#[test]
fn a_trip_count_past_the_narrow_period_keeps_both() {
    let (before, text) = m16(&narrow(3, "zext", 13, 1, 1, 600, ""));
    assert_eq!(before, text);
}
