//! A call made after a runtime-sized `alloca` stored its arguments at `(%sp)`,
//! which is the alloca's first bytes: `a[0]` read back as the first argument.
//! With a variable-sized object the call frame is not reserved, so the call
//! lowers `%sp` first (`M68kFrameLowering::eliminateCallFramePseudoInstr`).
//! `f` counts what the call overwrote: bytes 0 and 5 of the alloca, and the
//! callee's result.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

use crate::support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "define i32 @g(i32 %a, i32 %b, i32 %c) noinline {
b0:
  %r = sub i32 %a, %b
  %s = sub i32 %r, %c
  ret i32 %s
}
define i32 @f() {
b0:
  %hide = alloca i32
  store volatile i32 6, ptr %hide
  %n = load volatile i32, ptr %hide
  %a = alloca i8, i32 %n
  %a5 = getelementptr i8, ptr %a, i32 5
  store volatile i8 7, ptr %a
  store volatile i8 9, ptr %a5
  %r = call i32 @g(i32 100, i32 30, i32 20)
  %v0 = load volatile i8, ptr %a
  %v5 = load volatile i8, ptr %a5
  %b0 = icmp ne i8 %v0, 7
  %b5 = icmp ne i8 %v5, 9
  %br = icmp ne i32 %r, 50
  %x0 = zext i1 %b0 to i32
  %x5 = zext i1 %b5 to i32
  %xr = zext i1 %br to i32
  %t = add i32 %x0, %x5
  %bad = add i32 %t, %xr
  ret i32 %bad
}";

#[test]
fn a_call_after_a_runtime_alloca_keeps_the_alloca_intact() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            for name in ["f", "g"] {
                all.extend(
                    items_with(&module, name, cpu, options)
                        .unwrap_or_else(|why| panic!("{cpu:?} size={} @{name}: {why}", options.size)),
                );
            }
            let path = support::unique_path("alloca_call_area");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(0), "{cpu:?} size={}: an argument was stored over the alloca", options.size);
        }
    }
}
