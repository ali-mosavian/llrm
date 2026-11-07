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

/// Three extending loads of one global, a call between each, share the one
/// `lea`: a `(g,%pc)` each is 2 B more from the third load. The first version
/// of the fix took the shared address apart: 196 of 300 seeds got bigger.
#[test]
fn extending_loads_that_share_an_address_share_the_lea() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n@g16 = global i16 7\ndeclare void @h()\ndefine i32 @f() {{\n  %a = load i16, ptr @g16, align 2\n  call void @h()\n  %b = load i16, ptr @g16, align 2\n  call void @h()\n  %c = load i16, ptr @g16, align 2\n  %x = sext i16 %a to i32\n  %y = sext i16 %b to i32\n  %z = sext i16 %c to i32\n  %s = add i32 %x, %y\n  %t = add i32 %s, %z\n  ret i32 %t\n}}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let text = text_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles");
    assert_eq!(text.matches("lea\t(g16,%pc)").count(), 1, "{text}");
}
