//! `s += (long)(signed char)p[i]` over a buffer: the sign extension is part of
//! an extending load (`move.b (%a2)+,%d2` then `ext.w; ext.l`), which the sum's
//! `adda.w` form never saw; the sum stayed in a data register (rep_count -O1
//! on the 68000: +6 B and +6.6% clocks over gcc's `lea (%a1,%d0.w),%a1`).

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %n) {
entry:
  %t = alloca [8 x i8]
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %v = mul i32 %i, 37
  %w = trunc i32 %v to i8
  %p = getelementptr i8, ptr %t, i32 %i
  store i8 %w, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 8
  br i1 %c, label %fill, label %l
l:
  %j = phi i32 [0, %fill], [%j1, %l]
  %s = phi i32 [0, %fill], [%s1, %l]
  %q = getelementptr i8, ptr %t, i32 %j
  %x = load i8, ptr %q
  %e = sext i8 %x to i32
  %s1 = add i32 %s, %e
  %j1 = add i32 %j, 1
  %d = icmp slt i32 %j1, %n
  br i1 %d, label %l, label %done
done:
  ret i32 %s1
}
";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(IR).expect("parses")
}

#[test]
fn a_sum_of_loaded_bytes_is_added_with_adda_on_the_68000() {
    let out = text_with(&module(), "f", Cpu::M68000, Options::default()).expect("compiles");
    assert!(out.contains("adda.w") && !out.contains("ext.l"), "68000:\n{out}");
}

/// (i * 37) as a signed byte for i in 0..8: 0 37 74 111 -108 -71 -34 3, sum 12.
#[test]
fn the_sum_of_loaded_bytes_is_the_same_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[8]);
    all.extend(items_with(&module(), "f", Cpu::M68000, Options::default()).unwrap());
    let path = support::unique_path("sext_load");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(12), "sum");
}
