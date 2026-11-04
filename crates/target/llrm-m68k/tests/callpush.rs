//! Outgoing arguments as pushes. Three two-word calls stored their arguments
//! into a reserved call area (`suba.l #8,%sp`, `move.l %d4,(4,%sp)`,
//! `adda.l #8,%sp`, 12 bytes of frame and 6 of stores per call); pushed, the
//! frame goes and each call pays `move.l %dN,-(%sp)` twice and `addq.l #8,%sp`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `g(a, b) = a + 2b` tells the order of the arguments; `f` keeps its own
/// arguments in the frame between the pushes.
const CALLS: &str = "define internal i32 @g(i32 %a, i32 %b) #0 {
  %t = shl i32 %b, 1
  %r = add i32 %a, %t
  ret i32 %r
}
define i32 @f(i32 %x, i32 %y) {
  %p = call i32 @g(i32 %x, i32 %y)
  %q = call i32 @g(i32 %y, i32 %x)
  %r = call i32 @g(i32 %p, i32 %y)
  %s = add i32 %p, %q
  %t = add i32 %s, %r
  ret i32 %t
}
attributes #0 = { noinline }";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{CALLS}")).expect("parses")
}

fn size() -> Options {
    Options { size: true, ..Options::default() }
}

#[test]
fn calls_with_two_words_push_them_and_the_frame_goes() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module(), "f", cpu, size()).expect("compiles");
        assert!(text.matches("-(%sp)").count() >= 6, "{cpu:?}: no pushes:\n{text}");
        assert_eq!(text.matches("addq.l\t#8, %sp").count(), 3, "{cpu:?}: no cleanup per call:\n{text}");
        assert!(!text.contains("suba") && !text.contains("adda") && !text.contains("lea"), "{cpu:?}: a frame:\n{text}");
    }
}

/// `f(1, 2)`: `g(1,2)` is 5, `g(2,1)` is 4, `g(5,2)` is 9: 5 + 4 + 9.
#[test]
fn pushed_arguments_arrive_in_order_and_the_frame_slots_are_still_found() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[1, 2]);
    for name in ["f", "g"] {
        all.extend(items_with(&module(), name, Cpu::M68000, size()).expect("compiles"));
    }
    let path = support::unique_path("callpush");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(18));
}
