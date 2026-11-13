//! `memset`, `memcpy` and `memmove` return their destination in A:X on the 6502
//! runtime.

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8";

fn compile(
    ir: &str,
    name: &str,
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
    function_text(&module, name, &selection).expect("emits")
}

/// The destination kept in two statics across `jsr memset` and reloaded after
/// it (8 B and 2 cells of BSS) where the routine had returned it in A:X.
#[test]
fn memset_result_is_the_destination() {
    let ir = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define ptr @f(ptr %p, i8 %c, i16 %n) {
  call void @llvm.memset.p0.i16(ptr %p, i8 %c, i16 %n, i1 false)
  ret ptr %p
}";
    let text = compile(ir, "f");
    assert!(text.contains("memset"), "{text}");
    assert!(!text.contains("s_f_"), "the destination is saved across the call:\n{text}");
}

/// A store through the destination after the call reads the returned pointer.
#[test]
fn memcpy_result_is_the_destination() {
    let ir = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define void @f(ptr %p, ptr %q, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %p, ptr %q, i16 %n, i1 false)
  store i8 1, ptr %p
  ret void
}";
    let text = compile(ir, "f");
    assert!(text.contains("jsr memcpy"), "{text}");
    assert!(!text.contains("s_f_"), "the destination is saved across the call:\n{text}");
}
