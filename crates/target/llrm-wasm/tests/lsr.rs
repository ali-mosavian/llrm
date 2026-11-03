//! Loop strength reduction under wasm's `Tti`.

use std::rc::Rc;

use llrm_mir::passes::PassManager;
use llrm_wasm::machine::WasmMachine;

const RECPOW: &str = "\
target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"

declare fastcc i32 @power(i32, i32) nounwind memory(none)

define i32 @f(i32 %n) nounwind {
entry:
  %c = icmp slt i32 %n, 1
  br i1 %c, label %out, label %pre
pre:
  br label %loop
loop:
  %i = phi i32 [ %next, %loop ], [ 1, %pre ]
  %s = phi i32 [ %sum, %loop ], [ 0, %pre ]
  %r = urem i32 %i, 97
  %b = add nuw nsw i32 %r, 2
  %e = add nuw nsw i32 %i, 40
  %p = tail call fastcc i32 @power(i32 noundef %b, i32 noundef %e)
  %t = trunc nsw i32 %p to i16
  %m = srem i16 %t, 1000
  %x = sext i16 %m to i32
  %sum = add nsw i32 %s, %x
  %next = add nuw i32 %i, 1
  %d = icmp eq i32 %i, %n
  br i1 %d, label %exit, label %loop
exit:
  br label %out
out:
  %z = phi i32 [ 0, %entry ], [ %sum, %exit ]
  ret i32 %z
}
";

/// wasm's `Tti` left `isLegalAddImmediate`/`isLegalICmpImmediate` at the base
/// `false`, where the pin's `WebAssemblyTTIImpl` (a `BasicTTIImplBase`) takes
/// the lowering's `true`: LSR never rewrote `i + 40` as `lsr.iv + 41`, and
/// recpow's code section was 22 bytes (282 vs 260) over the pin's.
#[test]
fn an_immediate_offset_of_the_counter_is_legal() {
    let mut module = llrm_mir::parse::module(RECPOW).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(WasmMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(text.contains(", 41\n") && !text.contains(", 40\n"), "{text}");
}
