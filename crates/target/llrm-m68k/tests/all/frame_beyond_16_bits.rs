//! A frame of 65 KB put the saved registers' `movem` at a displacement past 16 bits and the
//! compiler stopped: `MOVM32pm: operand 0 = 65308 does not fit its 16-bit field` (QuickrBasic
//! `indent.c`, refused at -O2/-Os on every 68020 cell).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @big(ptr %0) {
  %2 = alloca [65028 x i8], align 1
  %3 = call i16 %0(ptr null, i32 0, ptr %2)
  %4 = load i32, ptr %0, align 2
  ret i32 %4
}
";

#[test]
fn a_frame_past_16_bits_pushes_its_registers_at_every_level() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let asm = assembly(IR, Cpu::M68020, level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        assert!(!asm.contains("movem") || !asm.contains("(650"), "{level:?}:\n{asm}");
    }
}
