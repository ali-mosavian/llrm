//! Consecutive byte stores of constants are one wider store whose first byte
//! is the most significant: the merge built the little-endian constant, so
//! `struct_odd_sizes` at -O0 stored 513 (bytes 2, 1) for `c = 1, d = 2` and
//! printed 301 for 393.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define void @f(ptr %p) {
  store i8 1, ptr %p
  %q = getelementptr i8, ptr %p, i32 1
  store i8 2, ptr %q
  ret void
}
";

#[test]
fn merged_constant_stores_put_the_first_byte_in_the_high_bits() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O0, Level::O2] {
            let asm = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{why}"));
            assert!(!asm.contains("#513"), "{cpu} {level:?}: the bytes are stored in the wrong order:\n{asm}");
            assert!(
                asm.contains("#258") || asm.matches("move.b").count() == 2,
                "{cpu} {level:?}: neither a word store of 0x0102 nor two byte stores:\n{asm}"
            );
        }
    }
}

const SWAPPED: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i16 @r(ptr %p) {
  %a = load i8, ptr %p
  %q = getelementptr i8, ptr %p, i32 1
  %b = load i8, ptr %q
  %za = zext i8 %a to i16
  %zb = zext i8 %b to i16
  %s = shl i16 %zb, 8
  %o = or i16 %s, %za
  ret i16 %o
}
define void @w(ptr %p, i16 %x) {
  %lo = trunc i16 %x to i8
  store i8 %lo, ptr %p
  %h = lshr i16 %x, 8
  %hi = trunc i16 %h to i8
  %q = getelementptr i8, ptr %p, i32 1
  store i8 %hi, ptr %q
  ret void
}
";

fn swaps_or_stays_bytes(asm: &str) -> bool {
    asm.contains("ror.w\t#8") || asm.contains("rol.w\t#8") || asm.contains("move.b")
}

/// `p[0] | p[1] << 8` was one `move.w (%a0), %d0`, the big-endian value.
#[test]
fn a_little_endian_byte_load_is_not_one_word_load() {
    for level in [Level::O2, Level::Os] {
        let asm = assembly(SWAPPED, cpu_of("68020").unwrap(), level).unwrap_or_else(|why| panic!("{why}"));
        let load = asm.split("w:").next().unwrap();
        assert!(swaps_or_stays_bytes(load), "{level:?}: the byte order is lost:\n{asm}");
    }
}

/// `p[0] = x; p[1] = x >> 8` was one `move.w %d0, (%a0)`, the big-endian value.
#[test]
fn little_endian_byte_stores_are_not_one_word_store() {
    for level in [Level::O2, Level::Os] {
        let asm = assembly(SWAPPED, cpu_of("68020").unwrap(), level).unwrap_or_else(|why| panic!("{why}"));
        let store = asm.split("w:").nth(1).unwrap();
        assert!(swaps_or_stays_bytes(store), "{level:?}: the byte order is lost:\n{asm}");
    }
}
