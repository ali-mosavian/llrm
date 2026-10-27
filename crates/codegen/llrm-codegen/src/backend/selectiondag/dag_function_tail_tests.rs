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
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
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

/// `LLRM_TAIL_CALLS=0` keeps the call and the return, as before the default
/// changed: a program that tail-jumped wrongly can be built without them.
#[test]
fn the_off_switch_keeps_a_call_and_a_return() {
    let module = llrm_mir::parse::module(INDIRECT_TAIL).expect("parses");
    super::dag_function::TAIL_CALLS_OFF.set(true);
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    super::dag_function::TAIL_CALLS_OFF.set(false);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    let ops: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()).map(|w| w.op).collect();
    assert!(ops.contains(&Operation::Call), "{ops:?}");
}

const M16_TAIL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1005 i16 @g(i16) addrspace(1)
define cc 1005 i16 @f(i16 %a) addrspace(1) {
entry:
  %r = tail call cc 1005 addrspace(1) i16 @g(i16 %a)
  ret i16 %r
}
";

/// At m16 a call in tail position was always a call and a far return: the
/// sibling-call path was m32 only. It is the epilogue and a jump to the far
/// callee, which returns to the caller's caller.
#[test]
fn an_m16_tail_call_jumps_to_the_callee() {
    let module = llrm_mir::parse::module(M16_TAIL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[]);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["g"]);
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    assert!(whats.iter().any(|w| w.op == Operation::Return && w.name.as_deref() == Some("jmp")), "a jmp that returns");
    assert!(whats.iter().all(|w| w.op != Operation::Call), "no call remains");
}

const M16_FORWARD_STACK: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1005 i32 @g(i32, i32, i32, i32, i32) addrspace(1)
define cc 1005 i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e) addrspace(1) {
entry:
  %r = tail call cc 1005 addrspace(1) i32 @g(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e)
  ret i32 %r
}
";

/// A call that passes the caller's own stack arguments on, in place, was a
/// call at m16: the callee's cells were counted from the near return address
/// and the caller's from the far one, so no argument matched its own place
/// (`fwd5` of tail_calls_edges.c). It is the epilogue and a `jmp far`.
#[test]
fn an_m16_call_passing_its_stack_arguments_on_is_a_jump() {
    let module = llrm_mir::parse::module(M16_FORWARD_STACK).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[]);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    assert!(whats.iter().any(|w| w.op == Operation::Return && w.name.as_deref() == Some("jmp")), "a jmp that returns");
    assert!(whats.iter().all(|w| w.op != Operation::Call), "no call remains");
}
