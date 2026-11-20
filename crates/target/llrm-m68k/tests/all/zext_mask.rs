//! `w[(i * 3) & 127]` with a `short` `i` was `and.w #127,%d4; and.l #65535,%d4`
//! (10 B and a copy) where gcc has `moveq #127,%d2; and.l %d0,%d2` (4): the
//! narrow `and` stayed narrow under the extension that widens it again
//! (word_array_abs_x O2: +24 B, +9% clocks). Executed against C over a byte and
//! a word, small and large masks, negative inputs.

use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

use crate::support;

const WORD_MASKS: [u32; 6] = [63, 127, 255, 1000, 0x7FFF, 0xFFFE];
const BYTE_MASKS: [u32; 4] = [7, 0x3F, 0x80, 0xFE];

fn ir() -> String {
    let mut text =
        String::from("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n");
    for (bits, masks) in [(16, &WORD_MASKS[..]), (8, &BYTE_MASKS[..])] {
        for mask in masks {
            text += &format!(
                "define i32 @m{bits}_{mask}(i{bits} %x) {{\n  %a = and i{bits} %x, {mask}\n  %z = zext i{bits} %a to i32\n  ret i32 %z\n}}\n"
            );
        }
    }
    text
}

fn names() -> Vec<String> {
    let mut all = Vec::new();
    for (bits, masks) in [(16, &WORD_MASKS[..]), (8, &BYTE_MASKS[..])] {
        all.extend(masks.iter().map(|mask| format!("m{bits}_{mask}")));
    }
    all
}

fn main_c() -> String {
    let mut text = String::from(
        "static long sys1(long n, long a) {\n    register long d0 __asm__(\"d0\") = n, d1 __asm__(\"d1\") = a;\n    __asm__ volatile(\"trap #0\" : \"+d\"(d0) : \"d\"(d1) : \"memory\");\n    return d0;\n}\n",
    );
    for (bits, ty, masks) in [(16, "short", &WORD_MASKS[..]), (8, "signed char", &BYTE_MASKS[..])] {
        for mask in masks {
            text += &format!("long m{bits}_{mask}({ty} x);\n");
        }
    }
    text += "int main(void) {\n    int n = 0;\n";
    for (bits, ty, masks) in [(16, "short", &WORD_MASKS[..]), (8, "signed char", &BYTE_MASKS[..])] {
        for mask in masks {
            text += &format!(
                "    for (int v = -300; v <= 300; v += 7) {{ {ty} x = ({ty})v; if (m{bits}_{mask}(x) != (long)(unsigned long)(x & {mask}u)) return n + 1; }}\n    ++n;\n"
            );
        }
    }
    text += "    return 0;\n}\nvoid _start(void) { sys1(1, main()); for (;;) ; }\n";
    text
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    text.split(&format!("{name}:")).nth(1).and_then(|rest| rest.split("rts").next()).expect("a function")
}

#[test]
fn the_mask_of_a_zero_extended_byte_or_word_is_one_long_and() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text =
                assembly(&ir(), cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            for name in names() {
                let code = body(&text, &name);
                let narrow = code
                    .lines()
                    .any(
                        |line| {
                            let line = line.trim_start();
                            line.starts_with("and.w")
                                || line.starts_with("and.b")
                                || line.contains("#65535")
                                || line.contains("#255,")
                        },
                    );
                assert!(!narrow, "{cpu} {level:?} {name}: a narrow and under the extension\n{code}");
            }
        }
    }
}

#[test]
fn every_mask_computes_what_c_computes_when_run() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for (cpu, level) in [("68000", Level::O1), ("68000", Level::Os), ("68020", Level::O2)] {
        let mut text =
            assembly(&ir(), cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
        for name in names() {
            text = text.replace(&format!("{name}:"), &format!(".globl {name}\n{name}:"));
        }
        let source = support::unique_path("zm-s");
        let object = support::unique_path("zm-o");
        let main = support::unique_path("zm-m");
        let exe = support::unique_path("zm-exe");
        std::fs::write(&source, format!(".text\n{text}")).unwrap();
        std::fs::write(&main, main_c()).unwrap();
        assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
        let mut link = tool("gcc");
        link.args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
        link.args(["-x", "c"]).arg(&main).args(["-x", "none"]).arg(&object).arg("-o").arg(&exe);
        let linked = link.output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
        assert_eq!(code, Some(0), "{cpu} {level:?}: wrong result at check number {code:?}");
    }
}
