//! Sibling calls (tail calls): `TC_RETURN` and what follows it.

use super::dag_function::selected;
use crate::model::ir::Operation;

const INDIRECT_TAIL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@fp = global ptr null
define i32 @f() {
entry:
  %p = load ptr, ptr @fp
  %r = tail call i32 %p()
  ret i32 %r
}
";

/// A tail call through a pointer was refused (`Unmapped("TCRETURNri")`): it
/// is the epilogue and a `jmp` through a register the epilogue leaves alone.
#[test]
fn a_sibling_call_through_a_pointer_jumps_through_a_register() {
    let module = llrm_mir::parse::module(INDIRECT_TAIL).expect("parses");
    super::dag_function::TAIL_CALLS.set(true);
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    super::dag_function::TAIL_CALLS.set(false);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let jump = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::Return && w.name.as_deref() == Some("jmp"))
        .expect("a jmp that returns");
    assert!(jump.indirect && matches!(jump.sources.as_slice(), [crate::model::ir::Loc::Held(_)]), "{jump:?}");
    assert!(insns.iter().filter_map(|i| i.what.as_ref()).all(|w| w.op != Operation::Call), "no call remains");
}
