//! memset's value argument was loaded into a callee-saved register before the
//! loop (`move.l 12(%sp), %d2` and a save and restore of `%d2`: 6 B more
//! at 68020 -Os than gcc, which stores the byte from its slot). A function
//! built for size now stores a stack argument's bytes from the slot:
//! `move.b (11,%sp), (%a1)+`. `memset_raw.ll` is clang's output with no
//! passes, what `llrm-cc` hands the driver.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const MEMSET: &str = include_str!("fixtures/memset_raw.ll");

#[test]
fn a_size_build_stores_the_argument_byte_from_its_slot() {
    for cpu in ["68000", "68020"] {
        let out = assembly(MEMSET, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(out.contains("move.b\t(11,%sp), (%a1)+"), "{cpu}: no store from the slot:\n{out}");
        assert!(!out.contains("%d2"), "{cpu}: the value still takes a saved register:\n{out}");
    }
}

#[test]
fn a_speed_build_keeps_the_value_in_a_register() {
    let out = assembly(MEMSET, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("(11,%sp), (%a1)+"), "the loop reads memory every trip:\n{out}");
}

/// `k(buf, 0x141, 4)` stores the low byte, 0x41: `d[3] + d[0]` is 130. The
/// byte at `(c + 3)` of the big-endian long is the one to store; its high
/// bytes would give 2 or 0.
#[test]
fn the_byte_stored_is_the_low_byte_of_the_long() {
    let Some(qemu) = support::qemu() else { return };
    let text = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @k(ptr %d, i32 %c, i32 %n) #0 {
entry:
  %t = trunc i32 %c to i8
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %p = getelementptr i8, ptr %d, i32 %i
  store i8 %t, ptr %p
  %i1 = add i32 %i, 1
  %e = icmp eq i32 %i1, %n
  br i1 %e, label %done, label %loop
done:
  %l = getelementptr i8, ptr %d, i32 3
  %a = load i8, ptr %l
  %b = load i8, ptr %d
  %x = zext i8 %a to i32
  %y = zext i8 %b to i32
  %r = add i32 %x, %y
  ret i32 %r
}
define i32 @f() {
  %buf = alloca [8 x i8]
  %r = call i32 @k(ptr %buf, i32 321, i32 4)
  ret i32 %r
}
attributes #0 = { noinline optsize }";
    let module = llrm_mir::parse::module(text).expect("parses");
    let asm = text_with(&module, "k", Cpu::M68000, Options { size: true, ..Options::default() }).expect("compiles");
    assert!(asm.contains("(11,%sp)"), "the store does not read the slot, so the run proves nothing:\n{asm}");
    let mut all = start("f", &[]);
    for name in ["f", "k"] {
        all.extend(
            items_with(&module, name, Cpu::M68000, Options { size: true, ..Options::default() }).expect("compiles"),
        );
    }
    let path = support::unique_path("arg_store");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(130));
}
