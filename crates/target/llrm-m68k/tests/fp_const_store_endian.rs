//! A double constant stored as two words puts the high word at the lower
//! address on a big-endian target: `(double)18446744073709551615ULL` at -O0
//! was stored 0, 0x43F00000 (so it read back as a denormal and `zz_f` printed
//! 1 for 0).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define void @f(ptr %p) {
  store double 0x43F0000000000000, ptr %p
  ret void
}
";

#[test]
fn a_double_constant_store_puts_the_high_word_first() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O0, Level::O2] {
            let asm = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            let high = asm
                .lines()
                .find(|line| line.contains("#1139802112"))
                .unwrap_or_else(|| panic!("{cpu} {level:?}: no 0x43F00000 stored:\n{asm}"));
            assert!(!high.contains("(4,"), "{cpu} {level:?}: the high word is not at the pointer:\n{asm}");
        }
    }
}
