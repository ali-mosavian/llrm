//! `s += p[i] > t` summed as `slt; ext.w; ext.l; sub.l %d3, %d0` (countgt -O2 on the 68000: 52 clk over
//! gcc's `suba.w %d3, %a1`). The sum in an address register takes the word with `suba.w`: no `ext.l`,
//! and the saved `%d3` goes too at -Os.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %t, i32 %n) {
entry:
  %a = alloca [8 x i32]
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %v = mul i32 %i, 5
  %w = and i32 %v, 7
  %p = getelementptr i32, ptr %a, i32 %i
  store i32 %w, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 8
  br i1 %c, label %fill, label %count
count:
  %j = phi i32 [0, %fill], [%j1, %count]
  %s = phi i32 [0, %fill], [%s1, %count]
  %q = getelementptr i32, ptr %a, i32 %j
  %x = load i32, ptr %q
  %g = icmp sgt i32 %x, %t
  %z = zext i1 %g to i32
  %s1 = add i32 %s, %z
  %j1 = add i32 %j, 1
  %d = icmp slt i32 %j1, %n
  br i1 %d, label %count, label %done
done:
  ret i32 %s1
}
";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(IR).expect("parses")
}

#[test]
fn a_sum_takes_a_sign_extended_word_with_suba_on_the_68000_only() {
    let text = |cpu| text_with(&module(), "f", cpu, Options::default()).expect("compiles");
    let old = text(Cpu::M68000);
    assert!(old.contains("suba.w") && !old.contains("ext.l"), "68000:\n{old}");
    let new = text(Cpu::M68020);
    assert!(!new.contains("suba.w"), "68020 has extb.l, so the word form is dearer:\n{new}");
}

/// (i*5) & 7 for i in 0..8 is 0 5 2 7 4 1 6 3; more than 4: 5 7 6, so three (t = 4, n = 8).
#[test]
fn the_sum_in_an_address_register_counts_the_same() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[4, 8]);
    all.extend(items_with(&module(), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("sext_acc");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(3));
}
