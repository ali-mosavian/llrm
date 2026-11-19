//! A variable 64-bit shift is an in-place loop over eight frame bytes or a
//! call, by the module price.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LAYOUT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n";

/// `shifts` variable shifts of one function, each by its own amount.
fn module(
    attr: &str,
    shifts: usize,
) -> String {
    let mut body = String::new();
    for at in 0..shifts {
        body.push_str(&format!(
            "  %w{at} = zext i32 %n{at} to i64\n  %r{at} = {} i64 %x, %w{at}\n  %s{at} = add i64 %r{at}, %x\n",
            ["shl", "lshr", "ashr"][at % 3]
        ));
    }
    let params: String = (0..shifts).map(|at| format!(", i32 %n{at}")).collect();
    let sum = (0..shifts).map(|at| format!("%s{at}")).collect::<Vec<_>>();
    let mut total = sum[0].clone();
    for (at, one) in sum.iter().enumerate().skip(1) {
        body.push_str(&format!("  %t{at} = add i64 {total}, {one}\n"));
        total = format!("%t{at}");
    }
    format!("{LAYOUT}define i64 @f(i64 %x{params}) {attr} {{\n{body}  ret i64 {total}\n}}\n")
}

fn compiled(
    ir: &str,
    level: Level,
) -> String {
    text(ir, "f", 0, &Request::new(level)).expect("compiles")
}

/// u64_shift_select_arms -Os called the 101 B bit-a-time routine for its two
/// shifts: 583 B and 27453 T against SDCC's 557 B and 15209 T. Two shifts are
/// two loops over the frame bytes, which link no routine (527 B, 14642 T).
#[test]
fn two_shifts_of_a_size_tuned_function_are_loops_not_calls() {
    let out = compiled(&module("minsize", 2), Level::Os);
    assert!(!out.contains("__s_") && !out.contains("__ashldi3") && !out.contains("__lshrdi3"), "{out}");
    assert!(out.contains("sla (hl)") && out.contains("srl (hl)") && out.matches("djnz").count() == 2, "{out}");
}

/// The speed routine (189 B) linked for two shifts at -O1 and -O2 made
/// u64_shift_select_arms 671 B against 557.
#[test]
fn two_shifts_at_the_speed_levels_link_no_routine() {
    for level in [Level::O1, Level::O2] {
        let out = compiled(&module("", 2), level);
        assert!(!out.contains("call __ashldi3") && !out.contains("call __lshrdi3"), "{level:?}:\n{out}");
        assert!(out.matches("djnz").count() == 2, "{level:?}:\n{out}");
    }
}

/// shift_int64_variable -Os called the 101 B routine for its shifts (388 T a bit): 872 B and 12.3M T against SDCC's
/// 1438 B and 11.7M T. Where the module's clocks are the shifts', the loops (half the clocks a bit) win by the
/// smaller relative regret, bytes first or not: 14 shifts are 14 loops and no routine.
#[test]
fn many_shifts_in_a_module_of_shifts_are_loops() {
    let out = compiled(&module("minsize", 14), Level::Os);
    assert!(out.matches("djnz").count() == 14 && !out.contains("__s_"), "{out}");
}

/// Every site is a loop of its own, unless the module's clocks are another loop's: the loops are 34 B a site
/// against a 9 B call and 101 B once, and the size routine's 388 T a bit is lost beside 40000 trips of a loop.
/// Six shifts beside it at -Os are the 101 B routine and six calls.
#[test]
fn many_shifts_beside_a_hot_loop_share_the_routine() {
    let hot = "@a = global [200 x i8] zeroinitializer
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
    let out = compiled(&format!("{LAYOUT}{hot}{}", module("minsize", 6).replace(LAYOUT, "")), Level::Os);
    assert!(out.contains("call __s_ashldi3") && !out.contains("djnz"), "{out}");
}

/// A constant 64-bit shift in a function tuned for size stayed inline
/// (compare_to_int64 +117 B, data_int64 +128 B, wide_shift_by_one +174 B when
/// the size routine took it too).
#[test]
fn a_constant_shift_of_a_size_tuned_function_is_not_a_call() {
    for amount in [3, 13, 21, 36] {
        let ir = format!(
            "{LAYOUT}define i64 @f(i64 %x) minsize {{
  %r = shl i64 %x, {amount}
  ret i64 %r
}}
"
        );
        let out = compiled(&ir, Level::Os);
        assert!(
            !out.contains("__s_ashldi3") && !out.contains("call __ashldi3") && !out.contains("djnz"),
            "shl {amount}:\n{out}"
        );
    }
}
