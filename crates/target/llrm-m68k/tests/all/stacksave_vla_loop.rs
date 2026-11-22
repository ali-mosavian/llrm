//! A `stacksave` / `stackrestore` pair around a runtime-sized `alloca` (a VLA
//! in a loop body) was refused: `CannotSelect("StackSave")`, the legalizer
//! asked the pointer type where the pin asks `MVT::Other`. `f` runs the loop,
//! fills each block, and counts a drifted stack pointer or a wrong sum.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

use crate::support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
define i32 @f() {
entry:
  %top = call ptr @llvm.stacksave.p0()
  br label %loop
loop:
  %k = phi i32 [1, %entry], [%k1, %loop]
  %sum = phi i32 [0, %entry], [%sum1, %loop]
  %s = call ptr @llvm.stacksave.p0()
  %a = alloca i8, i32 %k
  store volatile i8 3, ptr %a
  %v = load volatile i8, ptr %a
  %w = zext i8 %v to i32
  %sum1 = add i32 %sum, %w
  call void @llvm.stackrestore.p0(ptr %s)
  %k1 = add i32 %k, 1
  %c = icmp slt i32 %k1, 5000
  br i1 %c, label %loop, label %exit
exit:
  %end = call ptr @llvm.stacksave.p0()
  %drift = icmp ne ptr %top, %end
  %d = zext i1 %drift to i32
  %wrong = icmp ne i32 %sum1, 14997
  %x = zext i1 %wrong to i32
  %r = add i32 %d, %x
  ret i32 %r
}";

#[test]
fn a_vla_in_a_loop_with_stacksave_compiles_and_restores_the_stack() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            all.extend(
                items_with(&module, "f", cpu, options)
                    .unwrap_or_else(|why| panic!("{cpu:?} size={}: {why}", options.size)),
            );
            let path = support::unique_path("stacksave_vla_loop");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(0), "{cpu:?} size={}: stack pointer drifted or block wrong", options.size);
        }
    }
}
