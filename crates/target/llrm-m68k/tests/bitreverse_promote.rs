//! `llvm.bitreverse.i1` has no legal type to run in: the legalizer refused it
//! (`Unsupported BitReverse I1 "promote result"`), and a difftest seed was
//! rejected. It is the value itself, in the promoted byte.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = "
declare i1 @llvm.bitreverse.i1(i1)
define i32 @f(i32 %x) {
  %b = trunc i32 %x to i1
  %r = call i1 @llvm.bitreverse.i1(i1 %b)
  %z = zext i1 %r to i32
  ret i32 %z
}";

#[test]
fn a_one_bit_reverse_is_the_bit() {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    let options = Options { mid_end: false, ..Options::PINNED };
    let items = items_with(&module, "f", Cpu::M68000, options).expect("compiles");
    let Some(qemu) = support::qemu() else { return };
    for (x, want) in [(0, 0), (1, 1), (6, 0), (7, 1)] {
        let mut all = start("f", &[x]);
        all.extend(items.clone());
        let path = support::unique_path("bitreverse");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(want), "f({x})");
    }
}
