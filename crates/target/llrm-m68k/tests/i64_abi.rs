//! An `i64` argument or result was passed low word first, the opposite of the
//! pin (`getCopyToParts` reverses the parts on a big-endian target): the high
//! word of an argument is at `(4,%sp)` and a result is high word in `d0`, low
//! word in `d1`. Code that calls or is called by another compiler's got the
//! halves swapped.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "
define i32 @hi(i64 %a) {
  %s = lshr i64 %a, 32
  %t = trunc i64 %s to i32
  ret i32 %t
}
define i64 @join(i32 %h, i32 %l) {
  %a = zext i32 %h to i64
  %b = shl i64 %a, 32
  %c = zext i32 %l to i64
  %d = or i64 %b, %c
  ret i64 %d
}";

fn text(name: &str) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    text_with(&module, name, Cpu::M68020, Options::default()).expect("compiles")
}

#[test]
fn the_high_word_of_an_i64_argument_is_the_first_word() {
    let out = text("hi");
    assert!(out.contains("move.l\t(4,%sp), %d0"), "the high word is not read from the first slot:\n{out}");
}

#[test]
fn an_i64_result_is_the_high_word_in_d0_and_the_low_in_d1() {
    let out = text("join");
    assert!(
        out.contains("move.l\t(4,%sp), %d0") && out.contains("move.l\t(8,%sp), %d1"),
        "the halves are swapped:\n{out}"
    );
}
