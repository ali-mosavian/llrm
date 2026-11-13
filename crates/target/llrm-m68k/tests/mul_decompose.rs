//! `x * 9` was `jsr __mulsi3` on the 68000 (`muls.l` on the 68020) because the
//! m68k left the pin's `decomposeMulByConstant` at its default, false:
//! mul_constants -Os 68000 whole program +554 B against the pin's clang. The
//! pin's blanket answer then grew 45 small 68020 programs by 2 to 28 B: `x * 6`
//! is `moveq #6; muls.l` (6 B) or four shift/add instructions (8 B). The chain
//! is taken where the level's price says so (DEVIATE M68K-MUL-PRICE).

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn mul_by(c: u32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndefine i32 @f(i32 %x) {{\n  %r = mul i32 %x, {c}\n  ret i32 %r\n}}\n"
    )
}

fn compile(
    cpu: &str,
    level: Level,
    c: u32,
) -> String {
    assembly(&mul_by(c), cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn the_68000_has_no_32_bit_multiply_so_a_near_power_of_two_is_always_shifts_and_an_add() {
    for level in [Level::O1, Level::O2, Level::Os] {
        for c in [3, 5, 9, 10, 127, 255] {
            let out = compile("68000", level, c);
            assert!(!out.contains("__mulsi3"), "68000 x*{c}: a call:\n{out}");
        }
    }
}

#[test]
fn the_68020_keeps_muls_l_at_size_and_takes_the_chain_where_clocks_are_priced() {
    // the chain is 6 B or more against `muls.l`'s 4 (its constant register is
    // shared): -Os keeps the multiply
    for c in [6, 9] {
        let out = compile("68020", Level::Os, c);
        assert!(out.contains("muls.l"), "68020 -Os x*{c}: a longer chain:\n{out}");
    }
    // -O2 prices clocks beside bytes: shifts and an add against a 40-odd clock
    // multiply. -O1 stands on the pin's chain unless the multiply is no worse
    // in bytes and in clocks: `x * 9` at -O1 read `moveq #9; muls.l` (43
    // clocks) where the pin's clang has shifts and an add (byte_multiply,
    // shift_int64_variable, select_of_constants: 43 68020 O1 cells with a
    // multiply the best column does not execute).
    for level in [Level::O1, Level::O2] {
        for c in [3, 9, 127] {
            let out = compile("68020", level, c);
            assert!(!out.contains("muls.l"), "68020 {level:?} x*{c}: a multiply:\n{out}");
        }
    }
}
