//! The high word of an `i64` phi nothing reads is not computed.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @pick(i32 %m, i32 %e) {
  %x = zext i32 %m to i64
  %c = icmp sgt i32 %e, 22
  br i1 %c, label %a, label %b
a:
  %s1 = add nsw i32 %e, -23
  %z1 = zext nneg i32 %s1 to i64
  %l = shl i64 %x, %z1
  br label %j
b:
  %s2 = sub nsw i32 23, %e
  %z2 = zext nneg i32 %s2 to i64
  %r = lshr i64 %x, %z2
  br label %j
j:
  %p = phi i64 [ %l, %a ], [ %r, %b ]
  %t = trunc i64 %p to i32
  ret i32 %t
}
"#;

/// `report((long)(e >= 23 ? m << (e - 23) : m >> (23 - e)))` with a 64-bit `m`
/// kept the shift's high word: its two selects became branches (`lsr.l #1`,
/// `eori.w #31`, a second `bpl` a copy), 38 instructions where the low word
/// takes 21, and 200 clocks a call.
#[test]
fn the_high_word_of_a_truncated_phi_is_not_computed() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(IR, cpu, Level::O2).expect("compiles");
        let body: Vec<&str> = text.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).collect();
        assert!(!text.contains("eori.w\t#31"), "{cpu:?}: the high word's funnel shift is back:\n{text}");
        assert!(body.len() <= 26, "{cpu:?}: {} instructions:\n{text}", body.len());
    }
}
