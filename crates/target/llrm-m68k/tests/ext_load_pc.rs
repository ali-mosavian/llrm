//! A sign- or zero-extending load of a global took the address into a
//! register (`lea (g,%pc), %a0; move.w (%a0), %d0`, 4 B more than the plain
//! load's `move.w (g,%pc), %d0`): the pin matches `ARI` before `PCD` for it.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn asm(
    body: &str,
    cpu: Cpu,
) -> String {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n@g8 = global i8 7\n@g16 = global i16 7\ndefine i32 @f() {{\n{body}\n}}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

#[test]
fn an_extending_load_of_a_global_reads_it_pc_relative_at_every_cpu() {
    for (body, global) in [
        ("%v = load i8, ptr @g8, align 1\n  %e = sext i8 %v to i32\n  ret i32 %e", "g8"),
        ("%v = load i8, ptr @g8, align 1\n  %e = zext i8 %v to i32\n  ret i32 %e", "g8"),
        ("%v = load i16, ptr @g16, align 2\n  %e = sext i16 %v to i32\n  ret i32 %e", "g16"),
        ("%v = load i16, ptr @g16, align 2\n  %e = zext i16 %v to i32\n  ret i32 %e", "g16"),
    ] {
        for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040, Cpu::M68060] {
            let text = asm(body, cpu);
            assert!(text.contains(&format!("({global},%pc)")), "{cpu:?} {body}\n{text}");
            assert!(!text.contains("lea"), "{cpu:?}: the address went through a register\n{text}");
        }
    }
}
