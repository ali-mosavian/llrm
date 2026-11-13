//! `h(buf + 1, buf + 3)` pushed `move %a2, %d0; addq #1, %d0; move %d0,
//! (4,%sp)` for each pointer argument (8 B); `pea (1,%a2)` is 4 B and changes
//! no register.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @h(ptr %a, ptr %b) #0 {
  %x = load i8, ptr %a
  %y = load i8, ptr %b
  %xw = zext i8 %x to i32
  %yw = zext i8 %y to i32
  %s = shl i32 %xw, 4
  %r = add i32 %s, %yw
  ret i32 %r
}
define i32 @f(i32 %n) {
  %buf = alloca [8 x i8]
  %p0 = getelementptr i8, ptr %buf, i32 0
  store i8 0, ptr %p0
  %p1 = getelementptr i8, ptr %buf, i32 1
  store i8 1, ptr %p1
  %p2 = getelementptr i8, ptr %buf, i32 2
  store i8 2, ptr %p2
  %p3 = getelementptr i8, ptr %buf, i32 3
  store i8 3, ptr %p3
  %p5 = getelementptr i8, ptr %buf, i32 5
  store i8 5, ptr %p5
  %c1 = call i32 @h(ptr %p1, ptr %p3)
  %c2 = call i32 @h(ptr %p2, ptr %p5)
  %s = add i32 %c1, %c2
  %t = add i32 %s, %n
  ret i32 %t
}
attributes #0 = { noinline }
";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(IR).expect("parses")
}

fn options() -> Options {
    Options { size: true, call_targets: false, ..Options::default() }
}

#[test]
fn a_pointer_plus_a_constant_argument_is_one_pea() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module(), "f", cpu, options()).expect("compiles");
        let peas = text.lines().filter(|line| line.contains("pea\t(")).count();
        assert!(peas >= 4, "{cpu:?}: {peas} pea (d,An) of 4:\n{text}");
        assert!(!text.contains("addq.l\t#1, %d") && !text.contains("addq.l\t#3, %d"), "{cpu:?}:\n{text}");
    }
}

/// (1*16+3) + (2*16+5) = 56, plus n = 4.
#[test]
fn the_pushed_addresses_are_the_pointers_plus_the_constants() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[4]);
    for name in ["f", "h"] {
        all.extend(items_with(&module(), name, Cpu::M68000, options()).expect("compiles"));
    }
    let path = support::unique_path("pea_offset");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(60));
}
