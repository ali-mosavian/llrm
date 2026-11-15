//! u64_shift_variable_kinds -Os (68000): each argument of `__ashldi3` and its
//! kin was a volatile word of the frame, loaded into `%a1` and stored into the
//! call area (`move.l (20,%sp), %a1; move.l %a1, (4,%sp)`, 8 B a word): 936 B
//! of program against gcc's 834. Two causes: `foldable` refused every load
//! with side effects, a volatile one included, though moving the load to its
//! own adjacent store keeps the access, once and in order; and a call area
//! that cannot be pushed (the loads share `%a1`, and the volatile loads may
//! not change order) was dropped whole, its `move.l (20,%sp), (4,%sp)` too.
//! Now 792 B.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::libcalls::Mode;

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @report(i32) optsize
define i32 @main() optsize {
  %x = alloca i64, align 2
  %n = alloca i32, align 2
  store volatile i64 -7, ptr %x, align 2
  store volatile i32 3, ptr %n, align 2
  %v = load volatile i64, ptr %x, align 2
  %c = load volatile i32, ptr %n, align 2
  %w = zext i32 %c to i64
  %r = shl i64 %v, %w
  %t = trunc i64 %r to i32
  call void @report(i32 %t)
  %v2 = load volatile i64, ptr %x, align 2
  %c2 = load volatile i32, ptr %n, align 2
  %w2 = zext i32 %c2 to i64
  %r2 = lshr i64 %v2, %w2
  %t2 = trunc i64 %r2 to i32
  call void @report(i32 %t2)
  ret i32 0
}
"#;

#[test]
fn a_volatile_word_passed_to_the_shift_runtime_moves_memory_to_memory() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let options = Options { size: true, size_libcalls: Mode::Call, ..Options::default() };
        let module = llrm_mir::parse::module(IR).expect("parses");
        let asm = text_with(&module, "main", cpu, options).expect("compiles");
        assert!(asm.contains("__ashldi3") && asm.contains("__lshrdi3"), "the shifts are calls:\n{asm}");
        let detours = asm.lines().filter(|line| line.trim_start().starts_with("move.l\t%a1, (")).count();
        assert_eq!(detours, 0, "{cpu:?}: a volatile word goes through %a1 to the call area:\n{asm}");
    }
}
