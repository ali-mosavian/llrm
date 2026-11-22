use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
define i32 @f(ptr %p, ptr %o) {
  %a = load i16, ptr %p
  %z = zext i16 %a to i32
  store i16 %a, ptr %o
  ret i32 %z
}
";

/// Once the extending-load fold asked whether a truncate is free (the pin's `ExtendUsesToFormExtLoad`), m68k, whose
/// data registers hold the narrow value in their low word, stopped widening a load that a store also reads: the
/// load became `move.w` + `and.l #65535` (`mul_words_to_long` +4 B, `pattern_fill` -Os +2 B).
#[test]
fn a_zext_of_a_load_a_store_also_reads_is_still_one_move_into_a_cleared_register() {
    for level in [Level::O2, Level::Os] {
        let text = assembly(IR, cpu_of("68000").unwrap(), level).unwrap();
        assert!(text.contains("moveq\t#0, %d0"), "{level:?}\n{text}");
        assert!(!text.contains("and.l"), "{level:?}\n{text}");
    }
}
