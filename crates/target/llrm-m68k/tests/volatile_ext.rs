//! Every random program had `move.b (zero,%pc), %d3; and.l #255, %d3` (a
//! `load volatile i8` and its `zext`): the extension was a separate node
//! because the combiner left every volatile load alone, where the pin extends
//! it with its memory operand (`tryToFoldExtOfLoad`). The `and.l #255` was
//! ~60% of the 68000 seed gap to gcc.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@zero = external global i8
define i32 @k(i32 %x) {
  %b = load volatile i8, ptr @zero, align 1
  %w = zext i8 %b to i32
  %r = add i32 %w, %x
  ret i32 %r
}";

#[test]
fn a_volatile_byte_read_is_not_masked_after_the_load() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for size in [false, true] {
            let out = text_with(&module, "k", cpu, Options { size, ..Options::default() }).expect("compiles");
            assert!(!out.contains("and.l\t#255"), "{cpu:?} size={size}:\n{out}");
        }
    }
}
