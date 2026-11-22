//! `(short)(signed char)x` and `c - (x != 0)` at short width made `ext.w;
//! ext.l` where only the word is read: the dead `ext.l` was 2 B and 4 clk on
//! the 68000 and 68010 (98 `ext.w; ext.l` pairs in QB at 68000 Os). The pin's
//! llc has it too.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

fn asm(
    body: &str,
    cpu: &str,
    level: Level,
) -> String {
    assembly(&format!("{DATALAYOUT}\n{body}"), cpu_of(cpu).unwrap(), level).expect("compiles")
}

const WORD_OF_BYTE: &str = "define i16 @f(i8 %x) {\n  %z = sext i8 %x to i16\n  ret i16 %z\n}\n";
const WORD_SUM: &str = "define i16 @f(i16 %c, i32 %v) {\n  %b = icmp ne i32 %v, 0\n  %z = zext i1 %b to i16\n  %r = add i16 %c, %z\n  ret i16 %r\n}\n";
const LONG_OF_BYTE: &str = "define i32 @f(i8 %x) {\n  %z = sext i8 %x to i32\n  ret i32 %z\n}\n";

#[test]
fn a_byte_extended_to_a_word_is_one_ext_w_on_the_68000_and_68010() {
    for body in [WORD_OF_BYTE, WORD_SUM] {
        for cpu in ["68000", "68010"] {
            for level in [Level::O1, Level::O2, Level::Os] {
                let out = asm(body, cpu, level);
                assert!(out.contains("ext.w") && !out.contains("ext.l"), "{cpu} {level:?}:\n{out}");
            }
        }
    }
}

/// A long result still needs both.
#[test]
fn a_byte_extended_to_a_long_keeps_its_ext_l() {
    for level in [Level::O1, Level::Os] {
        let out = asm(LONG_OF_BYTE, "68000", level);
        assert!(out.contains("ext.w") && out.contains("ext.l"), "{level:?}:\n{out}");
    }
}
