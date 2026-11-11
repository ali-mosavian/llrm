//! `x * 1000` was `moveq #125; lsl.l #3; muls.l` at -O2 on the 68020 and a
//! `jsr __mulsi3` on the 68000, where gcc has a chain of shifts and adds: the
//! pin's combine decomposes `(2^n +- 1) << k` only, and 1000 is `125 << 3`
//! (div_constants_long 68020 -O2: 34668 clocks of `muls.l` against gcc's
//! 21060). DEVIATE G2: the cheapest chain by the m68k tables.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn mul_by(c: i32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndefine i32 @f(i32 %x) {{\n  %r = mul i32 %x, {c}\n  ret i32 %r\n}}\n"
    )
}

fn compile(cpu: &str, level: Level, c: i32) -> String {
    assembly(&mul_by(c), cpu_of(cpu).unwrap(), level).expect("compiles")
}

/// `sum += a[i] * c` over `n` elements: the multiply runs every trip.
fn loop_mul_by(c: i32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define i32 @f(ptr %a, i32 %n) {{\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]\n  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]\n  %p = getelementptr i32, ptr %a, i32 %i\n  %x = load i32, ptr %p\n  %m = mul i32 %x, {c}\n  %s1 = add i32 %s, %m\n  %i1 = add i32 %i, 1\n  %d = icmp eq i32 %i1, %n\n  br i1 %d, label %out, label %loop\nout:\n  ret i32 %s1\n}}\n"
    )
}

fn compile_loop(cpu: &str, level: Level, c: i32) -> String {
    assembly(&loop_mul_by(c), cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_constant_that_is_not_near_a_power_of_two_is_a_chain_in_a_loop() {
    for c in [1000, 100, -1000, 641, 31 * 5 * 8] {
        let out = compile_loop("68020", Level::O2, c);
        assert!(!out.contains("muls.l") && !out.contains("__mulsi3"), "68020 -O2 loop x*{c}: a multiply:\n{out}");
    }
}

/// struct_with_i64, struct_odd_sizes, switch_i64, frame_address_stack_args and
/// register_args_truncate at 68020 -O2/-O3 grew 4 to 16 B each with no executed
/// clock gained: the chain was credited its clocks in code that never ran.
#[test]
fn a_straight_line_multiply_is_decided_by_size_not_by_clocks_it_may_never_save() {
    for c in [1000, 100, -1000, 641] {
        let out = compile("68020", Level::O2, c);
        assert!(out.contains("muls.l"), "68020 -O2 straight-line x*{c}: a chain that grew the code:\n{out}");
    }
}

#[test]
fn the_68000_never_calls_the_library_for_a_short_chain() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let out = compile("68000", level, 6);
        assert!(!out.contains("__mulsi3"), "68000 {level:?} x*6: a call:\n{out}");
    }
    // 1000 is a 12 B chain: it beats the call's clocks, not its bytes
    let out = compile_loop("68000", Level::O2, 1000);
    assert!(!out.contains("__mulsi3"), "68000 -O2 loop x*1000: a call:\n{out}");
    for level in [Level::O1, Level::Os] {
        let out = compile("68000", level, 1000);
        assert!(out.contains("__mulsi3"), "68000 {level:?} x*1000: a chain larger than the call site:\n{out}");
    }
}

#[test]
fn a_size_build_keeps_the_multiply_where_the_chain_is_longer() {
    let out = compile("68020", Level::Os, 1000);
    assert!(out.contains("muls.l"), "68020 -Os x*1000: a chain longer than 4 B:\n{out}");
}
