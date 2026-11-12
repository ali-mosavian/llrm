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

/// Every site is a loop of its own: past the point where the loops together are
/// larger than one routine linked once and a call at each site, the calls win.
/// Six shifts at -Os are the 101 B routine and six calls.
#[test]
fn many_shifts_share_the_routine() {
    let out = compiled(&module("minsize", 6), Level::Os);
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
