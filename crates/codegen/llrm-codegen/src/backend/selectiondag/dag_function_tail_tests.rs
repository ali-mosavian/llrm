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

const REGISTER_TAIL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(ptr %p) {
entry:
  %q = getelementptr i8, ptr %p, i32 4
  %r = tail call i32 %q()
  ret i32 %r
}
";

fn the_jump(done: &crate::backend::isel::Selected) -> crate::model::ir::Semantics {
    done.body
        .insns()
        .iter()
        .filter_map(|i| i.what.clone())
        .find(|w| w.op == Operation::Return && w.name.as_deref() == Some("jmp"))
        .expect("a jmp that returns")
}

/// A tail call through a pointer was refused (`Unmapped("TCRETURNri")`): it
/// is the epilogue and a `jmp` through a register the epilogue leaves alone.
#[test]
fn a_sibling_call_through_a_pointer_jumps_through_a_register() {
    let module = llrm_mir::parse::module(REGISTER_TAIL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    let jump = the_jump(&done);
    assert!(jump.indirect && matches!(jump.sources.as_slice(), [crate::model::ir::Loc::Held(_)]), "{jump:?}");
    assert!(
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).all(|w| w.op != Operation::Call),
        "no call remains"
    );
}

/// `mov eax, [fp]; jmp eax` where the pin has `jmp [fp]` (`TCRETURNmi`,
/// 2026-11-05: every function-pointer tail call was one instruction and 1 to 3
/// bytes longer, through_pointer in tail_calls.c at -Os): the callee's load is
/// folded into the jump.
#[test]
fn a_sibling_call_through_a_loaded_pointer_jumps_through_its_cell() {
    let module = llrm_mir::parse::module(INDIRECT_TAIL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    let done = done.unwrap_or_else(|e| panic!("{}", e.0));
    let jump = the_jump(&done);
    assert!(jump.indirect && matches!(jump.sources.as_slice(), [crate::model::ir::Loc::Mem(_)]), "{jump:?}");
    let moves = done.body.insns().iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == Operation::Move).count();
    assert_eq!(moves, 0, "the pointer is not loaded first");
}

/// The same at m64, where the pin folds the load unless the arguments take too
/// many registers.
#[test]
fn an_m64_sibling_call_through_a_loaded_pointer_jumps_through_its_cell() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
@fp = global ptr null
define i32 @f(i32 %x) {
entry:
  %p = load ptr, ptr @fp
  %r = tail call i32 %p(i32 %x)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let jump = the_jump(&done);
    assert!(jump.indirect && matches!(jump.sources.as_slice(), [crate::model::ir::Loc::Mem(_)]), "{jump:?}");
}

const LOADED_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@fp = global ptr null
define i32 @f(i32 %x) {
entry:
  %p = load ptr, ptr @fp
  %r = call i32 %p(i32 %x)
  %s = add i32 %r, 1
  ret i32 %s
}
";

fn the_call(done: &crate::backend::isel::Selected) -> crate::model::ir::Semantics {
    done.body
        .insns()
        .iter()
        .filter_map(|i| i.what.clone())
        .find(|w| w.op == Operation::Call && w.indirect)
        .expect("an indirect call")
}

/// `mov eax, [fp]; call eax` where the pin has `call [fp]` (2026-11-05,
/// `PreprocessISelDAG`'s `isCalleeLoad`/`moveBelowOrigChain` were not
/// translated): the load of the callee moves below the call sequence and is
/// folded, whatever the arguments.
#[test]
fn a_call_through_a_loaded_pointer_calls_through_its_cell() {
    let module = llrm_mir::parse::module(LOADED_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
    let call = the_call(&done.unwrap_or_else(|e| panic!("{}", e.0)));
    assert!(matches!(call.sources.as_slice(), [crate::model::ir::Loc::Mem(_)]), "{call:?}");
}

/// The same at m64.
#[test]
fn an_m64_call_through_a_loaded_pointer_calls_through_its_cell() {
    let text = LOADED_CALL.replace("e-p:32:32-i64:32-n8:16:32", "e-p:64:64-i64:64-n8:16:32:64");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let call = the_call(&done);
    assert!(matches!(call.sources.as_slice(), [crate::model::ir::Loc::Mem(_)]), "{call:?}");
}

/// A volatile load, or one with a second use, is not moved (`isCalleeLoad`):
/// the pointer is loaded into a register.
#[test]
fn a_callee_load_that_cannot_move_stays_a_load() {
    let volatile = LOADED_CALL.replace("load ptr", "load volatile ptr");
    let reused = LOADED_CALL
        .replace("  %r = call", "  store ptr %p, ptr @g\n  %r = call")
        .replace("@fp = global ptr null", "@fp = global ptr null\n@g = global ptr null");
    for text in [volatile, reused] {
        let module = llrm_mir::parse::module(&text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[]);
        let call = the_call(&done.unwrap_or_else(|e| panic!("{}", e.0)));
        assert!(matches!(call.sources.as_slice(), [crate::model::ir::Loc::Held(_)]), "{text}\n{call:?}");
    }
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
