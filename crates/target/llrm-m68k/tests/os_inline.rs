//! A helper of about 30 B called from eight sites was copied into all eight at
//! -Os on the 68020 and up (the pin's `OptSizeThreshold` 50 allows growth):
//! +150 B in `bench_sfm16_float` for a 34 B `canon`. At -Os a callee that stays
//! is inlined only if it does not grow the code; O2 keeps the pin's choice.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const EIGHT_CALLS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @h(i32 %x) {
  %a = add i32 %x, 12345
  %b = mul i32 %a, 7
  %c = xor i32 %b, 99999
  %d = shl i32 %c, 3
  %e = sub i32 %d, %x
  ret i32 %e
}
define i32 @f(i32 %p, i32 %q, i32 %r, i32 %s, i32 %t, i32 %u, i32 %v, i32 %w) ATTR {
  %1 = call i32 @h(i32 %p)
  %2 = call i32 @h(i32 %q)
  %3 = call i32 @h(i32 %r)
  %4 = call i32 @h(i32 %s)
  %5 = call i32 @h(i32 %t)
  %6 = call i32 @h(i32 %u)
  %7 = call i32 @h(i32 %v)
  %8 = call i32 @h(i32 %w)
  %s1 = add i32 %1, %2
  %s2 = add i32 %s1, %3
  %s3 = add i32 %s2, %4
  %s4 = add i32 %s3, %5
  %s5 = add i32 %s4, %6
  %s6 = add i32 %s5, %7
  %s7 = add i32 %s6, %8
  ret i32 %s7
}
attributes #0 = { optsize }";

/// `f` is an `optsize` function (what clang emits at -Os) or not.
fn calls(
    cpu: &str,
    level: Level,
    optsize: bool,
) -> usize {
    let source = EIGHT_CALLS.replace("ATTR", if optsize { "#0" } else { "" });
    let out = assembly(&source, cpu_of(cpu).unwrap(), level).expect("compiles");
    out.lines().filter(|line| line.trim_start().starts_with("jsr\th")).count()
}

#[test]
fn a_helper_called_eight_times_stays_a_call_at_os() {
    for cpu in ["68000", "68020", "68040", "68060"] {
        assert_eq!(calls(cpu, Level::Os, true), 8, "{cpu}: the helper was copied into its call sites");
    }
}

#[test]
fn o2_keeps_the_pins_inline() {
    assert_eq!(calls("68020", Level::O2, false), 0, "68020 O2: the helper is no longer inlined");
}
