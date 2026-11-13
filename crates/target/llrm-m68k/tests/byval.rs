//! A `byval` parameter was refused ("a parameter attribute that moves it, with
//! a target's calling hook"): any C function taking a struct by value failed
//! to compile (tests/run/c: 7 of 114 programs). `CCPassByVal<4, 4>`: the
//! struct's bytes, at least 4 and rounded to 4, at the larger of its
//! alignment and 4, copied into the argument area; the callee's parameter is
//! the address of that copy.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn run(text: &str) {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("{HEAD}{text}")).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut all = start("f", &[]);
        for name in ["f", "g"] {
            all.extend(items_with(&module, name, cpu, Options::default()).unwrap_or_else(|why| panic!("{cpu:?}: {why}")));
        }
        let path = support::unique_path("byval");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(42), "{cpu:?}");
    }
}

/// Twelve bytes between two ints: a, c and the trailing int read from their
/// slots, 1 + 3 + 38.
#[test]
fn a_struct_of_three_longs_is_read_from_the_argument_area() {
    run("%S = type { i32, i32, i32 }
define internal i32 @g(i32 %p, ptr byval(%S) align 2 %s, i32 %q) {
  %pa = getelementptr %S, ptr %s, i32 0, i32 0
  %pc = getelementptr %S, ptr %s, i32 0, i32 2
  %a = load i32, ptr %pa, align 2
  %c = load i32, ptr %pc, align 2
  %t = add i32 %a, %c
  %u = add i32 %t, %q
  %v = add i32 %u, %p
  ret i32 %v
}
define i32 @f() {
  %b = alloca %S, align 2
  %b0 = getelementptr %S, ptr %b, i32 0, i32 0
  %b1 = getelementptr %S, ptr %b, i32 0, i32 1
  %b2 = getelementptr %S, ptr %b, i32 0, i32 2
  store i32 1, ptr %b0, align 2
  store i32 2, ptr %b1, align 2
  store i32 3, ptr %b2, align 2
  %r = call i32 @g(i32 0, ptr byval(%S) align 2 %b, i32 38)
  ret i32 %r
}
");
}

/// A five-byte struct takes eight, and the argument after it is read from
/// there: 40 + 2.
#[test]
fn an_odd_sized_struct_is_rounded_to_a_word() {
    run("%T = type { i8, i8, i8, i8, i8 }
define internal i32 @g(ptr byval(%T) align 1 %s, i32 %q) {
  %p4 = getelementptr %T, ptr %s, i32 0, i32 4
  %b = load i8, ptr %p4
  %x = zext i8 %b to i32
  %r = add i32 %x, %q
  ret i32 %r
}
define i32 @f() {
  %b = alloca %T, align 1
  %p4 = getelementptr %T, ptr %b, i32 0, i32 4
  store i8 2, ptr %p4
  %r = call i32 @g(ptr byval(%T) align 1 %b, i32 40)
  ret i32 %r
}
");
}

/// A struct with 8-byte alignment after one int starts at offset 8, not 4:
/// the pin's `CCPassByVal` takes the larger of the alignment and 4.
#[test]
fn an_eight_aligned_struct_skips_to_its_alignment() {
    run("%W = type { i64 }
define internal i32 @g(i32 %p, ptr byval(%W) align 8 %s, i32 %q) {
  %w = load i64, ptr %s, align 4
  %t = trunc i64 %w to i32
  %u = add i32 %t, %q
  ret i32 %u
}
define i32 @f() {
  %b = alloca %W, align 8
  store i64 40, ptr %b, align 4
  %r = call i32 @g(i32 7, ptr byval(%W) align 8 %b, i32 2)
  ret i32 %r
}
");
}
