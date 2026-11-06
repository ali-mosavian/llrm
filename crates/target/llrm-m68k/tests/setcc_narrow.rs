//! hex tests `v & 15 < 10` with `cmpi.l #10,%d2`, 6 B and 14 clk at 68000; the
//! operand is below 16 and the constant fits a byte, so `cmpi.b` is 4 B and 8
//! clk and gcc narrows. A compare of a value whose upper bits are known zero
//! against a constant that fits is made at the narrow width (eq/ne and the
//! unsigned conditions; the pin narrows only from an explicit extension). The
//! signed guard is a DAG test (`selectiondag/tests/setcc_known_bits.rs`): the
//! mid-end makes a signed compare of a non-negative value unsigned first.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// `if ((x & MASK) COND BOUND) 1 else 2`, with the masked value also stored so
/// it keeps its other use, as hex's digit does.
fn compare(
    mask: u32,
    cond: &str,
    bound: u32,
) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         define i32 @f(i32 %x, ptr %p) {{\n  %m = and i32 %x, {mask}\n  store i32 %m, ptr %p\n  \
         %c = icmp {cond} i32 %m, {bound}\n  br i1 %c, label %a, label %b\na:\n  ret i32 1\nb:\n  ret i32 2\n}}"
    )
}

fn asm(
    ir: &str,
    cpu: &str,
    level: Level,
) -> String {
    assembly(ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_masked_byte_is_compared_at_byte_width() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = asm(&compare(15, "ult", 10), cpu, level);
            assert!(out.contains("cmpi.b\t#10,") && !out.contains("cmpi.l\t#10,"), "{cpu} {level:?}:\n{out}");
        }
    }
}

/// A value below 512 against a bound a byte cannot hold is a word compare.
#[test]
fn a_constant_too_big_for_a_byte_is_compared_at_word_width() {
    let out = asm(&compare(511, "ult", 300), "68000", Level::O2);
    assert!(out.contains("cmpi.w\t#300,") && !out.contains("cmpi.l\t#300,"), "{out}");
}

/// Nothing is known of the register: the compare stays long.
#[test]
fn an_unbounded_register_is_compared_long() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
              define i32 @f(i32 %x) {\n  %c = icmp ult i32 %x, 10\n  br i1 %c, label %a, label %b\na:\n  ret i32 1\nb:\n  ret i32 2\n}";
    let out = asm(ir, "68000", Level::O2);
    assert!(out.contains("cmpi.l\t#10,") && !out.contains("cmpi.b"), "{out}");
}
