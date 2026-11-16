//! A call in tail position that passes the caller's own arguments on.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = r#"
define i32 @core(i32 %a, i32 %b, i32 %c) noinline {
  %x = shl i32 %a, 8
  %y = shl i32 %b, 4
  %s = add i32 %x, %y
  %r = add i32 %s, %c
  ret i32 %r
}

define i32 @fwd(i32 %a, i32 %b, i32 %c) noinline {
  %r = tail call i32 @core(i32 %a, i32 %b, i32 %c)
  ret i32 %r
}

; the callee takes the first two only
define i32 @pair(i32 %a, i32 %b) noinline {
  %r = add i32 %a, %b
  ret i32 %r
}

define i32 @fewer(i32 %a, i32 %b, i32 %c) noinline {
  %r = tail call i32 @pair(i32 %a, i32 %b)
  ret i32 %r
}

; an argument changed on the way is stored into its slot
define i32 @changed(i32 %a, i32 %b, i32 %c) noinline {
  %a1 = add i32 %a, 1
  %r = tail call i32 @core(i32 %a1, i32 %b, i32 %c)
  ret i32 %r
}

; both slots are read before either is written
define i32 @swapped(i32 %a, i32 %b, i32 %c) noinline {
  %r = tail call i32 @core(i32 %c, i32 %b, i32 %a)
  ret i32 %r
}

; the callee's arguments do not fit the caller's: a call
define i32 @wider(i32 %a, i32 %b) noinline {
  %r = tail call i32 @core(i32 %a, i32 %b, i32 7)
  ret i32 %r
}

define i32 @f() {
  %1 = call i32 @fwd(i32 1, i32 2, i32 3)
  %2 = call i32 @fewer(i32 4, i32 5, i32 6)
  %3 = call i32 @changed(i32 1, i32 2, i32 3)
  %4 = call i32 @swapped(i32 1, i32 2, i32 3)
  %5 = call i32 @wider(i32 1, i32 2)
  %e1 = sub i32 %1, 291
  %e2 = sub i32 %2, 9
  %e3 = sub i32 %3, 547
  %e4 = sub i32 %4, 801
  %e5 = sub i32 %5, 295
  %o = or i32 %e1, %e2
  %p = or i32 %o, %e3
  %q = or i32 %p, %e4
  %t = or i32 %q, %e5
  ret i32 %t
}
"#;

fn module() -> String {
    format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")
}

/// The lines of `name`'s label up to the next label.
fn function<'a>(
    text: &'a str,
    name: &str,
) -> String {
    let mut lines = text.lines().skip_while(|line| *line != format!("{name}:"));
    let mut out = vec![lines.next().expect("the function")];
    out.extend(lines.take_while(|line| line.starts_with('\t') || line.starts_with('.')));
    out.join("\n")
}

/// A forwarded call was a frame, a copy of each argument, `jsr`, `adda`, `rts`
/// (`fwd` was 8 instructions); the pin makes it the one `bra core`.
#[test]
fn forwarding_the_callers_arguments_is_a_branch() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&module(), cpu, Level::O2).expect("compiles");
        for name in ["fwd", "fewer"] {
            let body = function(&text, name);
            assert!(body.contains("\tbra\t") && !body.contains("jsr"), "{cpu:?} {name}:\n{body}");
            assert!(!body.contains("rts"), "{cpu:?} {name}:\n{body}");
        }
    }
}

/// An argument that is not the caller's own was stored below a `jsr` and
/// `adda`: 28 B on `swapped`, and the callee returned here. It is stored into
/// its slot and branched to, as gcc does; a callee needing more argument bytes
/// than the caller has stays a call.
#[test]
fn an_argument_that_changed_is_stored_into_its_slot() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&module(), cpu, Level::O2).expect("compiles");
        for name in ["changed", "swapped"] {
            let body = function(&text, name);
            assert!(body.contains("\tbra\t") && !body.contains("jsr") && !body.contains("rts"), "{cpu:?} {name}:\n{body}");
        }
        let wider = function(&text, "wider");
        assert!(wider.contains("jsr"), "{cpu:?} the callee's arguments need more room:\n{wider}");
    }
}

/// The values reach the callee through the branch, with no frame left over.
#[test]
fn the_arguments_arrive_through_a_tail_call() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&module()).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("f", &[]);
        for name in ["f", "fwd", "fewer", "changed", "swapped", "wider", "core", "pair"] {
            all.extend(items_with(&module, name, cpu, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("tail_call");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", flag], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(0), "{cpu:?}");
    }
}
