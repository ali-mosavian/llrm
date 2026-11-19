//! nbody_fixed -O1: `512 / ((d >> 18) + 1)` was `__udivsi3` (linked 2471 B
//! against 2096 B at -O2, SDCC 2258 B): the pin's O1 has no
//! correlated-propagation to narrow a divide of values that fit 16 bits.
//! Narrowed at selection it is `__udivhi3`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i32 @udiv(i32 %d) nounwind {
  %s = lshr i32 %d, 18
  %t = add i32 %s, 1
  %q = udiv i32 512, %t
  ret i32 %q
}
define i32 @sdiv(i32 %d) nounwind {
  %s = and i32 %d, 16383
  %t = add i32 %s, 1
  %q = sdiv i32 512, %t
  ret i32 %q
}
define i32 @srem(i32 %d) nounwind {
  %s = and i32 %d, 16383
  %t = add i32 %s, 1
  %q = srem i32 512, %t
  ret i32 %q
}
"#;

#[test]
fn a_divide_of_values_that_fit_a_word_is_a_word_divide_at_o1() {
    for name in ["udiv", "sdiv", "srem"] {
        let asm = text(IR, name, 0, &Request::new(Level::O1)).expect("compiles");
        let calls: Vec<&str> = asm.lines().filter_map(|line| line.trim().strip_prefix("call ")).collect();
        assert!(calls.iter().all(|callee| !callee.ends_with("si3")), "{name} made a long call {calls:?}:\n{asm}");
        assert!(
            calls.iter().any(|callee| callee.ends_with("hi3") || callee.ends_with("hi4")),
            "{name} has no word divide {calls:?}:\n{asm}"
        );
    }
}
