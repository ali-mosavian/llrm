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
