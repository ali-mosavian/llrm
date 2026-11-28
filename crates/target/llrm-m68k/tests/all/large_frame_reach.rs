//! A 65 KB local put the incoming argument at `(65048,%sp)` and the address of
//! the buffer at `(-65032,%a6)`: displacements the 68000 cannot encode, and the
//! object writer stopped (`MOV16rp: operand 1 = 65362 does not fit its 16-bit
//! field`, QuickrBasic `indent.c`, refused at every 68000 and 68020 cell). A
//! frame past 32 KiB takes `%a6`, links without the size and opens it with
//! `suba.l`; an address past 16 bits is the base plus the displacement.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, object};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @use(ptr, i32)
define i32 @f(ptr %p, i32 %n) {
  %buf = alloca [65028 x i8], align 2
  %a = alloca i32, align 4
  %b = alloca i32, align 4
  store i32 %n, ptr %a, align 4
  store i32 7, ptr %b, align 4
  call void @use(ptr %buf, i32 %n)
  %x = load i32, ptr %a, align 4
  %y = load i32, ptr %b, align 4
  %s = add i32 %x, %y
  ret i32 %s
}
";

/// Every `(N,%reg)` displacement in `asm` fits a word.
fn displacements(asm: &str) -> Vec<i64> {
    asm.split('(')
        .skip(1)
        .filter_map(|tail| tail.split_once(",%").and_then(|(number, _)| number.parse::<i64>().ok()))
        .collect()
}

#[test]
fn a_frame_past_32_kib_is_addressed_within_16_bits_at_every_cpu() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for level in [Level::O0, Level::O2, Level::Os] {
            let asm = assembly(IR, cpu, level).unwrap_or_else(|why| panic!("{cpu:?} {level:?}: {why}"));
            let far: Vec<i64> = displacements(&asm).into_iter().filter(|d| i16::try_from(*d).is_err()).collect();
            assert!(far.is_empty(), "{cpu:?} {level:?}: {far:?}\n{asm}");
            object(IR, cpu, level, "f").unwrap_or_else(|why| panic!("{cpu:?} {level:?}: {why}"));
        }
    }
}
