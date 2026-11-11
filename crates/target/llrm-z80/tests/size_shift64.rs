//! A variable 64-bit shift in a function tuned for size calls the 101 B routine, not the 189 B one.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i64 @f(i64 %x, i32 %n) ATTR {
  %w = zext i32 %n to i64
  %r = shl i64 %x, %w
  ret i64 %r
}
";

fn calls(
    attr: &str,
    level: Level,
) -> String {
    text(&IR.replace("ATTR", attr), "f", 0, &Request::new(level)).expect("compiles")
}

/// u64_shift_select_arms -Os linked the 189 B shift body for two shifts (672 B against SDCC's 557).
#[test]
fn a_size_tuned_function_calls_the_size_routine() {
    let out = calls("minsize", Level::Os);
    assert!(out.contains("call __s_ashldi3") && !out.contains("call __ashldi3"), "{out}");
}

/// The speed routine stays where nothing asks for size.
#[test]
fn a_speed_tuned_function_keeps_the_speed_routine() {
    let out = calls("", Level::O2);
    assert!(out.contains("call __ashldi3") && !out.contains("__s_ashldi3"), "{out}");
}

/// A constant 64-bit shift in a function tuned for size stayed inline (compare_to_int64 +117 B, data_int64 +128 B,
/// wide_shift_by_one +174 B when the size routine took it too).
#[test]
fn a_constant_shift_of_a_size_tuned_function_is_not_a_call() {
    for amount in [3, 13, 21, 36] {
        let ir = format!(
            "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i64 @g(i64 %x) minsize {{
  %r = shl i64 %x, {amount}
  ret i64 %r
}}
"
        );
        let out = text(&ir, "g", 0, &Request::new(Level::Os)).expect("compiles");
        assert!(!out.contains("__s_ashldi3") && !out.contains("call __ashldi3"), "shl {amount}:\n{out}");
    }
}
