//! `*p |= m` stayed `move.b (%a0),%d0; move.b m,%d1; or.b %d0,%d1; move.b
//! %d1,(%a0)` (10 bytes) where gcc has `or.b %d0,(%a0)` (2): the pin has no
//! memory-destination pattern with a register source, nor for `and`/`or` with
//! an immediate (spill_store_order -Os +26 B against gcc). Executed against C
//! over every width, operation and address mode, and a subtraction whose loaded
//! value is the right operand, which must not fold.

use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

use crate::support;

const OPS: [(&str, &str); 4] = [("add", "+"), ("sub", "-"), ("and", "&"), ("or", "|")];
const WIDTHS: [(u32, &str, &str); 3] = [(8, "b", "char"), (16, "w", "short"), (32, "l", "long")];
const MODES: [&str; 3] = ["plain", "disp", "idx"];

fn ir() -> String {
    let mut text =
        String::from("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n");
    for (op, _) in OPS {
        for (bits, _, _) in WIDTHS {
            for mode in MODES {
                let address = match mode {
                    "plain" => "  %a = getelementptr i8, ptr %p, i32 0\n".to_owned(),
                    "disp" => format!("  %a = getelementptr i{bits}, ptr %p, i32 3\n"),
                    _ => format!("  %a = getelementptr i{bits}, ptr %p, i32 %i\n"),
                };
                text += &format!(
                    "define void @{op}{bits}{mode}(ptr %p, i32 %i, i{bits} %v) {{\n{address}  %o = load i{bits}, ptr %a\n  %r = {op} i{bits} %o, %v\n  store i{bits} %r, ptr %a\n  ret void\n}}\n"
                );
                if op != "sub" {
                    // the loaded value is the second operand, and the first is
                    // a load too (a global read before it)
                    text += &format!(
                        "define void @{op}{bits}{mode}c(ptr %p, i32 %i, ptr %q) {{\n{address}  %v = load i{bits}, ptr %q\n  %o = load i{bits}, ptr %a\n  %r = {op} i{bits} %v, %o\n  store i{bits} %r, ptr %a\n  ret void\n}}\n"
                    );
                }
            }
        }
    }
    text += "define void @rsub32(ptr %p, i32 %i, i32 %v) {\n  %a = getelementptr i32, ptr %p, i32 %i\n  %o = load i32, ptr %a\n  %r = sub i32 %v, %o\n  store i32 %r, ptr %a\n  ret void\n}\n";
    text
}

fn names() -> Vec<String> {
    let mut all = Vec::new();
    for (op, _) in OPS {
        for (bits, _, _) in WIDTHS {
            for mode in MODES {
                all.push(format!("{op}{bits}{mode}"));
            }
        }
    }
    all.push("rsub32".to_owned());
    all
}

fn main_c() -> String {
    let mut text = String::from(
        "static long sys1(long n, long a) {\n    register long d0 __asm__(\"d0\") = n, d1 __asm__(\"d1\") = a;\n    __asm__ volatile(\"trap #0\" : \"+d\"(d0) : \"d\"(d1) : \"memory\");\n    return d0;\n}\n",
    );
    for (_, _, c) in WIDTHS {
        for (op, _) in OPS {
            for (bits, _, ty) in WIDTHS {
                if ty == c {
                    for mode in MODES {
                        text += &format!("void {op}{bits}{mode}({ty} *p, int i, {ty} v);\n");
                    }
                }
            }
        }
    }
    text += "void rsub32(long *p, int i, long v);\nint main(void) {\n    int n = 0;\n";
    for (op, sign) in OPS {
        for (bits, _, ty) in WIDTHS {
            for mode in MODES {
                let at = match mode {
                    "plain" => "0",
                    "disp" => "3",
                    _ => "i",
                };
                text += &format!(
                    "    {{ {ty} a[8] = {{ 0, 1, -1, 127, 5, 6, 7, 8 }}, e[8] = {{ 0, 1, -1, 127, 5, 6, 7, 8 }};\n      for (int i = 0; i < 8; ++i) {{ {ty} v = ({ty})(90 + i * 7); {op}{bits}{mode}(a, i, v); e[{at}] = ({ty})(e[{at}] {sign} v); }}\n      for (int i = 0; i < 8; ++i) if (a[i] != e[i]) return n + 1; }}\n    ++n;\n"
                );
            }
        }
    }
    text += "    { long a[4] = { 9, 100, -5, 1 }; for (int i = 0; i < 4; ++i) rsub32(a, i, 1000); if (a[0] != 991 || a[1] != 900 || a[2] != 1005 || a[3] != 999) return n + 1; }\n    return 0;\n}\nvoid _start(void) { sys1(1, main()); for (;;) ; }\n";
    text
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    text.split(&format!("{name}:")).nth(1).and_then(|rest| rest.split("rts").next()).expect("a function")
}

#[test]
fn a_memory_operation_with_a_register_is_one_memory_destination_instruction() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text =
                assembly(&ir(), cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            for (op, _) in OPS {
                for (bits, size, _) in WIDTHS {
                    for mode in MODES {
                        for suffix in if op == "sub" { &[""][..] } else { &["", "c"][..] } {
                            let name = format!("{op}{bits}{mode}{suffix}");
                            let code = body(&text, &name);
                            let wanted = format!("{op}.{size}\t%d");
                            assert!(
                                code.lines().any(
                                    |line| line.trim_start().starts_with(&wanted) && line.trim_end().ends_with(')')
                                ),
                                "{cpu} {level:?} {name}: no `{wanted}..., (...)`\n{code}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_operation_width_and_mode_computes_what_c_computes_when_run() {
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
        let source = support::unique_path("rmwr-s");
        let object = support::unique_path("rmwr-o");
        let main = support::unique_path("rmwr-m");
        let exe = support::unique_path("rmwr-exe");
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
