//! A long divide of a function tuned for size is the size routine or the speed
//! one, by the module price.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LAYOUT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n";

/// A nested loop over 200 bytes: 40000 trips, a clock total the divide cannot
/// move.
const HOT_LOOP: &str = "@a = global [200 x i8] zeroinitializer
define void @g() minsize {
entry:
  br label %outer
outer:
  %j = phi i16 [ 0, %entry ], [ %j.next, %inner.end ]
  br label %inner
inner:
  %i = phi i16 [ 0, %outer ], [ %i.next, %inner ]
  %p = getelementptr i8, ptr @a, i16 %i
  %v = load i8, ptr %p
  %w = add i8 %v, 1
  store i8 %w, ptr %p
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 200
  br i1 %c, label %inner, label %inner.end
inner.end:
  %j.next = add nuw i16 %j, 1
  %d = icmp ne i16 %j.next, 200
  br i1 %d, label %outer, label %exit
exit:
  ret void
}
";

fn divide(
    bits: u32,
    level: Level,
    rest: &str,
) -> String {
    let ir = format!(
        "{LAYOUT}{rest}define i{bits} @f(i{bits} %a, i{bits} %b) minsize {{\n  %q = udiv i{bits} %a, %b\n  ret i{bits} %q\n}}\n"
    );
    text(&ir, "f", 0, &Request::new(level)).expect("compiles")
}

/// data_int64 -Os called the bit-a-time `__s_udivdi3` (99k T a divide, 8x the
/// speed routine's) for 179 B fewer: 19038 T in a module that was 742 B. A
/// module whose clocks the divide dominates takes the speed routine.
#[test]
fn a_long_long_divide_costing_the_module_its_clocks_is_the_speed_routine() {
    for level in [Level::Os, Level::Oz] {
        let out = divide(64, level, "");
        assert!(out.contains("call __udivdi3") && !out.contains("__s_"), "{level:?}:\n{out}");
    }
}

/// The same divide beside 40000 trips of a loop: its clocks are a rounding
/// error there, the 179 B are not.
#[test]
fn a_long_long_divide_beside_a_hot_loop_stays_the_size_routine() {
    let out = divide(64, Level::Os, HOT_LOOP);
    assert!(out.contains("call __s_udivdi3") && !out.contains("call __udivdi3"), "{out}");
}

/// A remainder by a constant in a 100-trip loop, after `pad` functions of
/// straight-line stores.
fn loop_remainder(pad: usize) -> String {
    let mut ir = format!("{LAYOUT}@t = global [512 x i16] zeroinitializer\ndefine void @pad() minsize {{\n");
    for at in 0..pad {
        ir.push_str(&format!(
            "  %p{at} = getelementptr i16, ptr @t, i16 {at}\n  store i16 {}, ptr %p{at}\n",
            1000 + at * 7
        ));
    }
    ir.push_str(
        "  ret void
}
define i32 @f(ptr %p) minsize {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %t = phi i32 [ 0, %entry ], [ %r, %loop ]
  %m = mul i32 %t, 3
  %g = getelementptr i32, ptr %p, i16 %i
  %v = load i32, ptr %g
  %a = add i32 %m, %v
  %r = srem i32 %a, 1000003
  %i.next = add nuw i16 %i, 1
  %c = icmp ne i16 %i.next, 100
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %r
}
",
    );
    text(&ir, "f", 0, &Request::new(Level::Os)).expect("compiles")
}

/// copy_runs -Os called `__s_modsi3` (6.1k T a trip) in a 767 B module far
/// under its bytes bar and over its clocks bar: 729429 T against 541410. The
/// 315 B of the speed routine are a small part of a module that size.
#[test]
fn a_long_divide_in_a_loop_of_a_module_that_size_is_the_speed_routine() {
    let out = loop_remainder(80);
    assert!(out.contains("call __modsi3") && !out.contains("__s_"), "{out}");
}

/// Alone, the loop is 100 B: the routine's 315 B are the larger loss, and the
/// size routine stays.
#[test]
fn a_long_divide_in_a_loop_of_a_small_module_stays_the_size_routine() {
    let out = loop_remainder(0);
    assert!(out.contains("call __s_modsi3") && !out.contains("call __modsi3"), "{out}");
}

/// div_constants_ulong -Os: the divide's speed routines rode on the alternative
/// that decomposes the long multiply (one switch for every reason a function
/// has an alternative), whose price they spoiled: `__mulsi3` kept for
/// 3 x `q * 31` where the shifts were chosen, +78 B and +9.8M T over the module
/// without it. A function with a multiply alternative keeps the size divide and
/// the multiply's own choice (shifts, `__ashlsi3`).
#[test]
fn a_divide_does_not_spoil_the_choice_for_the_multiply_of_the_same_function() {
    let out =
        text(include_str!("data/divide_beside_multiply.ll"), "c3", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(out.contains("call __ashlsi3") && !out.contains("call __mulsi3"), "{out}");
    assert!(out.contains("call __s_udivremsi4"), "{out}");
}
