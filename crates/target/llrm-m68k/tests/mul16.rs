//! `(u64)`-style multiplies in `__muldi3` call `__mulsi3` seven times at the
//! 68000 (754 B against libgcc's 166 B) because the pin selects `mul i32` as
//! `muls.l` or a libcall, never `muls.w`. A product of two values that fit 16
//! bits, both unsigned or both signed, is `mulu.w` / `muls.w`.

use llrm_m68k::dagisel::lowering::prefers_word_multiply;
use llrm_m68k::driver::{Level, assembly, cpu_of};

const CPUS: [&str; 6] = ["68000", "68010", "68020", "68030", "68040", "68060"];

fn product(
    left: &str,
    right: &str,
) -> String {
    let load = |name: &str, how: &str| match how {
        "i32" => format!("  %{name}x = add i32 %{name}, 0\n"),
        _ => format!("  %{name}x = {how} i16 %{name} to i32\n"),
    };
    let ty = |how: &str| if how == "i32" { "i32" } else { "i16" };
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         define i32 @f({} %a, {} %b) {{\n{}{}  %m = mul i32 %ax, %bx\n  ret i32 %m\n}}",
        ty(left),
        ty(right),
        load("a", left),
        load("b", right)
    )
}

fn asm(
    ir: &str,
    cpu: &str,
    level: Level,
) -> String {
    assembly(ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

/// The CPUs with no `muls.l`: the long multiply is a libcall there.
const NO_LONG_MULTIPLY: [&str; 2] = ["68000", "68010"];

#[test]
fn two_unsigned_halves_are_multiplied_with_mulu_w_where_the_long_multiply_is_a_call() {
    for cpu in NO_LONG_MULTIPLY {
        for level in [Level::O2, Level::Os] {
            let out = asm(&product("zext", "zext"), cpu, level);
            assert!(out.contains("mulu\t") && !out.contains("__mulsi3") && !out.contains("mulu.l"), "{cpu} {level:?}:\n{out}");
        }
    }
}

#[test]
fn two_signed_halves_are_multiplied_with_muls_w_where_the_long_multiply_is_a_call() {
    for cpu in NO_LONG_MULTIPLY {
        let out = asm(&product("sext", "sext"), cpu, Level::O2);
        assert!(out.contains("muls\t") && !out.contains("__mulsi3") && !out.contains("muls.l"), "{cpu}:\n{out}");
    }
}

/// bench/memory at 68020 -O1 was 2 B larger with the word multiply: an
/// extended operand costs the `ext.l` and a copy of its source (the tied
/// destination) that `muls.l d0,d0` does not. Where `muls.l` exists the
/// extended operands keep it.
#[test]
fn an_extended_halfword_keeps_the_long_multiply_where_there_is_one() {
    for cpu in CPUS.iter().filter(|cpu| !NO_LONG_MULTIPLY.contains(cpu)) {
        for how in ["sext", "zext"] {
            let out = asm(&product(how, how), cpu, Level::O1);
            assert!(out.contains(".l\t") && !out.contains("muls\t") && !out.contains("mulu\t"), "{cpu} {how}:\n{out}");
        }
    }
}

/// Operands that are already full-width values known to fit 16 bits take
/// the word multiply on every CPU: no extension, no copy, and 2 B shorter.
#[test]
fn masked_operands_take_the_word_multiply_on_every_cpu() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
              define i32 @f(i32 %a, i32 %b) {\n  %x = and i32 %a, 65535\n  %y = and i32 %b, 65535\n  %m = mul i32 %x, %y\n  ret i32 %m\n}";
    for cpu in CPUS {
        let out = asm(ir, cpu, Level::O2);
        assert!(out.contains("mulu\t") && !out.contains("__mulsi3") && !out.contains("mulu.l"), "{cpu}:\n{out}");
    }
}

#[test]
fn a_full_width_operand_keeps_the_long_multiply() {
    let out = asm(&product("zext", "i32"), "68000", Level::O2);
    assert!(out.contains("__mulsi3") && !out.contains("mulu\t"), "68000:\n{out}");
    let out = asm(&product("zext", "i32"), "68020", Level::O2);
    assert!(out.contains("mulu.l") || out.contains("muls.l"), "68020:\n{out}");
}

#[test]
fn halves_of_mixed_signedness_keep_the_long_multiply() {
    let out = asm(&product("zext", "sext"), "68000", Level::O2);
    assert!(out.contains("__mulsi3") && !out.contains("mulu\t") && !out.contains("muls\t"), "{out}");
}

#[test]
fn the_word_multiply_is_priced_no_slower_than_the_long_one_on_every_cpu() {
    for cpu in CPUS {
        assert!(prefers_word_multiply(cpu_of(cpu).unwrap()), "{cpu}");
    }
}

#[test]
fn a_constant_operand_is_left_to_the_pins_folds() {
    // `(x + 3) * 5` is `x * 5 + 15` in the pin; the half multiply must not take the product first
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
              define i32 @f(i16 %a) {\n  %x = zext i16 %a to i32\n  %s = add i32 %x, 3\n  %m = mul i32 %s, 5\n  ret i32 %m\n}";
    let out = asm(ir, "68000", Level::O2);
    assert!(!out.contains("mulu\t") && !out.contains("muls\t"), "{out}");
    assert!(out.contains("#15"), "the fold's constant 3 * 5 is added after the product:\n{out}");
}
