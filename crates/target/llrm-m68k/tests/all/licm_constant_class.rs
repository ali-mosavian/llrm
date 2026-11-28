//! The shared MachineLICM replaced a data-register zero by an equal one the
//! entry block made for a phi (class `XR32`) without narrowing it, so the
//! allocator put it in `%a4` and `subx.l %a4,%d2` reached the assembler
//! (QuickrBasic `frame.c`, `array.c`: 4 files refused at 68020).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @arry_elem_size()
declare i32 @arry_frame_hdr()
define void @frame_layout(ptr %0, i32 %1, i32 %2, ptr %3) {
  br label %5
5:
  %6 = phi i32 [ 0, %4 ], [ %22, %20 ]
  %7 = phi i32 [ 0, %4 ], [ 1, %20 ]
  %8 = phi i32 [ 0, %4 ], [ %1, %20 ]
  %9 = call i32 @arry_elem_size()
  %10 = call i32 @arry_frame_hdr()
  %11 = zext i32 %2 to i64
  %12 = and i32 %6, 16777215
  %13 = zext i32 %12 to i64
  %14 = mul i64 %11, %13
  %15 = icmp ult i64 %14, 2
  %16 = sub i32 0, %8
  %17 = icmp ne i32 %1, %16
  %or.cond = select i1 %15, i1 %17, i1 false
  br i1 %or.cond, label %20, label %18
18:
  %19 = getelementptr [8 x i8], ptr %3, i32 %7
  store i32 0, ptr %19, align 2
  br label %20
20:
  %21 = load ptr, ptr %0, align 2
  %22 = ptrtoint ptr %21 to i32
  br label %5
}
";

#[test]
fn subx_reads_a_data_register_at_every_level() {
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        let asm = assembly(IR, Cpu::M68020, level).expect("compiles");
        let bad: Vec<&str> = asm.lines().filter(|line| line.contains("subx") && line.contains("%a")).collect();
        assert!(bad.is_empty(), "{level:?}: {bad:?}\n{asm}");
    }
}
