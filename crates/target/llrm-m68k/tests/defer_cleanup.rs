//! `g(1,2); g(3,4); g(5,6); g(7,8)` cleaned up after every call (`addq.l #8,
//! %sp` four times, 8 B); gcc lets the arguments pile up and pops them once
//! (`lea (32,%sp), %sp`, 4 B). The callee does not care what lies below its
//! arguments; the caller's own stack-relative accesses are shifted by what is
//! owed, and a branch, a label, a return or a kept call area finds the stack
//! popped.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const HEADER: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @g(i32 %a, i32 %b) #0 {
  %x = xor i32 %a, %b
  %r = add i32 %x, %a
  ret i32 %r
}
attributes #0 = { noinline }
";

/// Four calls whose results are summed, with a local array read between them.
fn module() -> llrm_mir::module::Module {
    let ir = format!(
        "{HEADER}define i32 @f(i32 %n) {{
  %buf = alloca [4 x i32]
  %p = getelementptr [4 x i32], ptr %buf, i32 0, i32 1
  store i32 %n, ptr %p
  %c1 = call i32 @g(i32 1, i32 2)
  %v1 = load i32, ptr %p
  %c2 = call i32 @g(i32 3, i32 4)
  %v2 = load i32, ptr %p
  %c3 = call i32 @g(i32 5, i32 6)
  %c4 = call i32 @g(i32 7, i32 8)
  %s1 = add i32 %c1, %c2
  %s2 = add i32 %s1, %c3
  %s3 = add i32 %s2, %c4
  %s4 = add i32 %s3, %v1
  %s5 = add i32 %s4, %v2
  ret i32 %s5
}}"
    );
    llrm_mir::parse::module(&ir).expect("parses")
}

fn options() -> Options {
    Options { size: true, call_targets: false, ..Options::default() }
}

#[test]
fn the_cleanups_of_calls_in_a_row_are_one() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module(), "f", cpu, options()).expect("compiles");
        let cleanups =
            text.lines().filter(|line| line.contains("addq.l\t#8, %sp") || line.contains("(8,%sp), %sp")).count();
        assert!(cleanups <= 2, "{cpu:?}: {cleanups} cleanups of 8 B:\n{text}");
    }
}

/// g(1,2)=1^2+1=4, g(3,4)=7+3=10, g(5,6)=3+5=8, g(7,8)=15+7=22: 44, plus the
/// local read twice (n = 5): 54.
#[test]
fn a_local_read_between_calls_is_found_with_arguments_still_on_the_stack() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[5]);
    for name in ["f", "g"] {
        all.extend(items_with(&module(), name, Cpu::M68000, options()).expect("compiles"));
    }
    let path = support::unique_path("defer_cleanup");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(54));
}
