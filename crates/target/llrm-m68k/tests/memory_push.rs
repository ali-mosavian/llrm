//! An argument loaded from memory and pushed took a register and two
//! instructions (`move.l (8,%sp), %d1` and `move.l %d1, -(%sp)`, 6 B); the
//! push can read the memory itself (`move.l (8,%sp), -(%sp)`, 4 B), as gcc's
//! does. structcopy -Os: 44 B against gcc's 42.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `g(a, b) = a + 2b` tells the order. `pc` loads its first argument, `pk`
/// stores to that memory between the load and the call, so the old value must
/// be the one pushed.
const CALLS: &str = "define internal i32 @g(i32 %a, i32 %b) #0 {
  %t = shl i32 %b, 1
  %r = add i32 %a, %t
  ret i32 %r
}
define internal i32 @pc(ptr %p, i32 %y) #0 {
  %v = load i32, ptr %p
  %r = call i32 @g(i32 %v, i32 %y)
  ret i32 %r
}
define internal i32 @pk(ptr %p, i32 %y) #0 {
  %v = load i32, ptr %p
  store i32 9, ptr %p
  %r = call i32 @g(i32 %v, i32 %y)
  ret i32 %r
}
define i32 @f1() {
  %x = alloca i32
  store i32 5, ptr %x
  %r = call i32 @pc(ptr %x, i32 3)
  ret i32 %r
}
define i32 @f2() {
  %x = alloca i32
  store i32 5, ptr %x
  %r = call i32 @pk(ptr %x, i32 3)
  ret i32 %r
}
attributes #0 = { noinline }";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{CALLS}")).expect("parses")
}

fn options(size: bool) -> Options {
    Options { size, ..Options::default() }
}

#[test]
fn a_loaded_argument_is_pushed_from_memory_with_no_register_between() {
    for size in [true, false] {
        let text = text_with(&module(), "pc", Cpu::M68000, options(size)).expect("compiles");
        assert!(text.contains("move.l\t(8,%sp), -(%sp)"), "size {size}: no push from memory:\n{text}");
        assert!(!text.contains("%d1"), "size {size}: a temporary is left:\n{text}");
    }
}

#[test]
fn a_store_between_the_load_and_the_call_keeps_the_load_first() {
    let text = text_with(&module(), "pk", Cpu::M68000, options(true)).expect("compiles");
    let load = text.find("(%a0), %d0").or_else(|| text.find("(%a0), %d1")).unwrap_or(usize::MAX);
    let store = text.find("#9").unwrap_or(0);
    assert!(load < store, "the old value must be read before the store:\n{text}");
}

fn run(
    entry: &str,
    size: bool,
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start(entry, &[]);
    for name in [entry, "pc", "pk", "g"] {
        all.extend(items_with(&module(), name, Cpu::M68000, options(size)).expect("compiles"));
    }
    let path = support::unique_path("memory_push");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    code
}

/// `g(5, 3)` is 11 either way; swapped arguments would be 3 + 10.
#[test]
fn pushed_from_memory_the_arguments_arrive_in_order_and_unchanged() {
    for size in [true, false] {
        for entry in ["f1", "f2"] {
            if let Some(code) = run(entry, size) {
                assert_eq!(code, 11, "{entry}, size {size}");
            }
        }
    }
}
