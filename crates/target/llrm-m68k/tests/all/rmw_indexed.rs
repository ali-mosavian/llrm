//! `++counts[x]` stayed `moveq #1,%d2; add.w (0,%a0,%d3),%d2; move.w
//! %d2,(0,%a0,%d3)` (10 bytes) where gcc has `addq.w #1,(0,%a0,%d3)` (4): the
//! pin's memory-destination add and subtract patterns cover `(An)` only.
//! Executed against C over every width, a displaced and an indexed address, a
//! negative and a large constant, and a loop where another load precedes the
//! store.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};

use crate::support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define void @inc16(ptr %p, i32 %i) {
  %a = getelementptr i16, ptr %p, i32 %i
  %v = load i16, ptr %a
  %r = add i16 %v, 1
  store i16 %r, ptr %a
  ret void
}
define void @add32(ptr %p, i32 %i) {
  %a = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %a
  %r = add i32 %v, 5
  store i32 %r, ptr %a
  ret void
}
define void @sub8(ptr %p, i32 %i) {
  %a = getelementptr i8, ptr %p, i32 %i
  %v = load i8, ptr %a
  %r = sub i8 %v, 2
  store i8 %r, ptr %a
  ret void
}
define void @addneg16(ptr %p, i32 %i) {
  %a = getelementptr i16, ptr %p, i32 %i
  %v = load i16, ptr %a
  %r = add i16 %v, -3
  store i16 %r, ptr %a
  ret void
}
define void @big16(ptr %p, i32 %i) {
  %a = getelementptr i16, ptr %p, i32 %i
  %v = load i16, ptr %a
  %r = add i16 %v, 1000
  store i16 %r, ptr %a
  ret void
}
define void @disp32(ptr %p) {
  %a = getelementptr i32, ptr %p, i32 3
  %v = load i32, ptr %a
  %r = add i32 %v, 7
  store i32 %r, ptr %a
  ret void
}
define void @tally(ptr %s, ptr %c, i32 %n) {
entry:
  %go = icmp sgt i32 %n, 0
  br i1 %go, label %loop, label %done
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %sa = getelementptr i16, ptr %s, i32 %i
  %x = load i16, ptr %sa
  %xi = zext i16 %x to i32
  %m = and i32 %xi, 255
  %ca = getelementptr i16, ptr %c, i32 %m
  %v = load i16, ptr %ca
  %r = add i16 %v, 1
  store i16 %r, ptr %ca
  %next = add i32 %i, 1
  %more = icmp slt i32 %next, %n
  br i1 %more, label %loop, label %done
done:
  ret void
}";

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    text.split(&format!("{name}:")).nth(1).and_then(|rest| rest.split("rts").next()).expect("a function")
}

#[test]
fn a_memory_add_or_sub_of_a_small_constant_is_one_quick_instruction() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            for (name, wanted) in [
                ("inc16", "addq.w\t#1, ("),
                ("add32", "addq.l\t#5, ("),
                ("sub8", "subq.b\t#2, ("),
                ("addneg16", "subq.w\t#3, ("),
                ("disp32", "addq.l\t#7, ("),
                ("tally", "addq.w\t#1, ("),
            ] {
                let code = body(&text, name);
                assert!(code.contains(wanted), "{cpu} {level:?} {name}: no `{wanted}`\n{code}");
            }
            let big = body(&text, "big16");
            assert!(big.contains("add.w\t#1000, ("), "{cpu} {level:?} big16\n{big}");
        }
    }
}

#[test]
fn every_width_mode_and_constant_computes_what_c_computes_when_run() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for (cpu, level) in [("68000", Level::O1), ("68000", Level::O2), ("68000", Level::Os), ("68020", Level::O2)] {
        let mut text = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
        for name in ["inc16", "add32", "sub8", "addneg16", "big16", "disp32", "tally"] {
            text = text.replace(&format!("{name}:"), &format!(".globl {name}\n{name}:"));
        }
        let source = support::unique_path("rmw-s");
        let object = support::unique_path("rmw-o");
        let exe = support::unique_path("rmw-exe");
        std::fs::write(&source, format!(".text\n{text}")).unwrap();
        assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
        let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/rmw_main.c");
        let mut link = tool("gcc");
        link.args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"]);
        link.arg(main).arg(&object).arg("-o").arg(&exe);
        let linked = link.output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
        for path in [source, object, exe] {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(0), "{cpu} {level:?}: wrong result (see tests/data_rt/rmw_main.c)");
    }
}
