//! Loop strength reduction under the m68k `Tti`.

use std::rc::Rc;

use llrm_m68k::machine::M68kMachine;
use llrm_mir::passes::PassManager;

const RECPOW: &str = "\
target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"

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

/// m68k's `Tti` left `isLegalAddImmediate`/`isLegalICmpImmediate` at the base
/// `false`, where the pin's `M68kTTIImpl` (a `BasicTTIImplBase` over a lowering
/// that does not override them) answers `true`: LSR never rewrote `i + 40` as
/// `iv + 41` (recpow on wasm: 22 bytes over the pin).
#[test]
fn an_immediate_offset_of_the_counter_is_legal() {
    let mut module = llrm_mir::parse::module(RECPOW).expect("parses");
    let mut manager = PassManager::default();
    let elements = llrm_transforms::pipeline_text::parse("loop-reduce").expect("pipeline");
    llrm_transforms::pipeline_text::build(&elements, &mut manager).ok().expect("builds");
    manager.run_module(&mut module, Rc::new(M68kMachine)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(text.contains(", 41\n") && !text.contains(", 40\n"), "{text}");
}
