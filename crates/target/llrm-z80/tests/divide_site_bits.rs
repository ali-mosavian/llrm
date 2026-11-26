//! A long divide in a function tuned for size is priced by the bits its
//! dividend is proven to need, not by a typical width.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LAYOUT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n";

/// A loop whose state is `(state + 74) rem modulus`, from a byte, and
/// whose digit is that state's `rem_op` by 10, after `pad` functions of stores
/// (a module the speed routine's bytes are a small part of). Every dividend is
/// proven by its range alone.
fn digit_loop(
    modulus: u32,
    state_op: &str,
    digit_op: &str,
    pad: usize,
) -> String {
    digit_loop_by(modulus, state_op, digit_op, pad, "add i32 %s, 74")
}

/// [`digit_loop`] with `step` the instruction that makes the dividend of the
/// state's remainder.
fn digit_loop_by(
    modulus: u32,
    state_op: &str,
    digit_op: &str,
    pad: usize,
    step: &str,
) -> String {
    let mut ir = format!("{LAYOUT}@t = global [512 x i16] zeroinitializer\ndefine void @pad() minsize {{\n");
    for at in 0..pad {
        ir.push_str(&format!(
            "  %p{at} = getelementptr i16, ptr @t, i16 {at}\n  store i16 {}, ptr %p{at}\n",
            1000 + at * 7
        ));
    }
    ir.push_str(&format!(
        "  ret void
}}
define i32 @f(i32 %seed) minsize {{
entry:
  %start = and i32 %seed, 255
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %s = phi i32 [ %start, %entry ], [ %n, %loop ]
  %total = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %a = {step}
  %n = {state_op} i32 %a, {modulus}
  %d = {digit_op} i32 %n, 10
  %sum = add i32 %total, %d
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 100
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %sum
}}
"
    ));
    text(&ir, "f", 0, &Request::new(Level::Os)).expect("compiles")
}

/// bench/switch -Os called `__s_modsi3` (6.3k T a call) for a state under
/// 65537 whose remainders by 10 and 14 take 17 dividend bits, 5.4k T on the
/// speed routine, not the 24 a typical dividend is priced at (7.7k T).
#[test]
fn a_remainder_of_a_state_the_loop_keeps_under_65537_is_the_speed_routine() {
    let out = digit_loop(65537, "urem", "urem", 80);
    assert!(out.contains("call __umodsi3") && !out.contains("__s_"), "{out}");
}

/// The same loop over a state of 31 bits: its digit is 27 steps, 10k T a trip
/// against the size routine's 6.3k, and the size routine stays.
#[test]
fn a_remainder_of_a_state_that_needs_the_whole_word_stays_the_size_routine() {
    let out = digit_loop(2147483647, "urem", "urem", 80);
    assert!(out.contains("call __s_umodsi3") && !out.contains("call __umodsi3"), "{out}");
}

/// The ranges proven of the dividends of `body`'s divides, in order.
fn ranges(body: &str) -> Vec<(i64, i64)> {
    let ir = format!("{LAYOUT}define i32 @f(i32 %x, i16 %w) {{\n{body}\n  ret i32 %r\n}}\n");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let layout =
        llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a layout");
    let (_, _, function) = module.functions().next().expect("a function");
    let found = llrm_z80::dividends::dividend_ranges(&module, function, &layout);
    let mut ranges: Vec<(u32, i64, i64)> = found.iter().map(|(at, range)| (at.0, range.low, range.high)).collect();
    ranges.sort_unstable();
    ranges.into_iter().map(|(_, low, high)| (low, high)).collect()
}

/// An unsigned remainder of a value that may be negative is a 32-bit dividend:
/// priced as the span's `[-32768, 32767]` it was 15 bits, the speed routine.
#[test]
fn an_unsigned_divide_of_a_value_below_zero_has_no_proven_range() {
    let body = "  %e = sext i16 %w to i32\n  %r = urem i32 %e, 10";
    assert_eq!(ranges(body), vec![], "{body}");
}

/// The range is the middle end's, signed or unsigned.
#[test]
fn a_dividend_is_asserted_by_the_range_the_middle_end_proves() {
    let masked = "  %m = and i32 %x, 131071\n  %r = urem i32 %m, 10";
    assert_eq!(ranges(masked), vec![(0, 131071)]);
    let signed = "  %e = sext i16 %w to i32\n  %r = srem i32 %e, 10";
    assert_eq!(ranges(signed), vec![(-32768, 32767)]);
}

/// bench/switch -Os: the function's multiplies (`digit * digit * 37`, a call or
/// shifts) were its alternative, so its three remainders kept the size routine
/// (6.3k T each: 46.7M T, 31.2M with the speed routine for 186 B). A function
/// with both has the choice on each.
#[test]
fn a_divide_beside_a_multiply_of_the_same_function_takes_the_speed_routine() {
    let out =
        text(include_str!("data/switch_remainders.ll"), "bench_switch", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(out.contains("call __modsi3") && !out.contains("__s_modsi3"), "{out}");
}
