//! `mulu.l` and `muls.l` are 68020 instructions. A 32-bit multiply with overflow selected them at
//! every CPU, so the 68000 build of `overflow_builtins` did not assemble ("statement ignored", 5 cells).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare { i32, i1 } @llvm.umul.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)
define i32 @u(i32 %a, i32 %b) {
  %r = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %r, 0
  %o = extractvalue { i32, i1 } %r, 1
  %z = zext i1 %o to i32
  %s = add i32 %v, %z
  ret i32 %s
}
define i32 @s(i32 %a, i32 %b) {
  %r = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %r, 0
  %o = extractvalue { i32, i1 } %r, 1
  %z = zext i1 %o to i32
  %s = add i32 %v, %z
  ret i32 %s
}
";

#[test]
fn the_68000_multiplies_with_overflow_without_a_long_multiply_instruction() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        assert!(!text.contains("mulu.l") && !text.contains("muls.l"), "{level:?}:\n{text}");
    }
}

#[test]
fn the_68020_keeps_its_long_multiplies() {
    let text = assembly(IR, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(text.contains("mulu.l") && text.contains("muls.l"), "{text}");
}
