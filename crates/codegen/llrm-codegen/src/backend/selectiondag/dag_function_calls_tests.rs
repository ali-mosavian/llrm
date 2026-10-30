//! Calls: the call sequence, arguments and results in registers and on the
//! stack, runtime routines, landing pads, ports and inline blocks.

use super::dag_function::selected;
use super::dag_function_tests::{FAR_CALL, M16_STACK_ARGS, Stub, names, select_with_contracts, verdicts};
use crate::model::ir::{Loc, Operation};
use crate::testing::m16_ir;

const CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare i32 @bar(i32, i32)
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, 1
  %r = call i32 @bar(i32 %b, i32 %s)
  %t = add i32 %r, 3
  ret i32 %t
}
";

/// A call reserves its argument bytes, stores the arguments at SP, calls,
/// releases the bytes, and takes its result from what the call delivers in
/// EAX; it destroys the caller-saved registers. Each was `Unmapped
/// (ADJCALLSTACKDOWN32)`: 114 + 86 functions of the corpora.
#[test]
fn a_call_is_a_sequence_around_a_call_instruction() {
    let module = crate::testing::module(CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns
        .iter()
        .find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call))
        .expect("a call");
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["bar"]);
    assert!(done.calls.contains_key(&call.at));
    assert_eq!(call.delivers.len(), 1, "the result is what the call delivers");
    assert!(
        call.clobbers.contains(&llrm_lir::registers::RegId::EAX),
        "the call destroys the registers the convention does not keep: {:?}",
        call.clobbers
    );
    let names: Vec<String> =
        insns.iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    // The area is the pin's pseudo pair around the call; `callframe.rs` makes
    // them pushes or `sub`/`add`.
    let called = names.iter().position(|n| n == "call").unwrap();
    // After the argument load and the increment, as the pin's finalize-isel
    // has them (`visitTokenFactor` frees the call's chain from the arguments).
    assert_eq!(names.iter().position(|n| n == "call_frame_setup"), Some(2), "{names:?}");
    assert_eq!(names.get(called + 1).map(String::as_str), Some("call_frame_destroy"), "{names:?}");
    assert!(!names.iter().any(|n| n == "sub"), "{names:?}");
}

/// Source order is the default (`SchedInfo::source_order`): it builds
/// `s = a + 1` before the call, as the IR has it, with or without
/// `LLRM_SRC_ORDER`. Before it was the default the call sequence opened the
/// block.
#[test]
fn source_order_keeps_the_ir_order_around_a_call() {
    let module = crate::testing::module(CALL).expect("parses");
    let order = |on: bool| {
        super::schedule::SRC_ORDER.set(on);
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{}", e.0));
        super::schedule::SRC_ORDER.set(false);
        let names: Vec<String> = done
            .body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .map(|w| w.name.as_deref().unwrap_or("").to_owned())
            .collect();
        names.iter().position(|n| n == "call_frame_setup").unwrap()
    };
    assert!(order(false) > 0, "the default is source order");
    assert!(order(true) > 0);
}

const INDIRECT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@fp = global ptr null
define i32 @f(i32 %a) {
entry:
  %p = load ptr, ptr @fp
  %r = call i32 %p(i32 %a)
  ret i32 %r
}
";

/// A call through a pointer is a call with its target as the source and
/// `indirect` set; without the flag it was a direct call with no callee and
/// 71 modules of the vsgcc corpus stopped compiling.
#[test]
fn an_indirect_call_names_its_target() {
    let module = crate::testing::module(INDIRECT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let call = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.clone())
        .find(|w| w.op == crate::model::ir::Operation::Call)
        .expect("a call");
    assert!(call.indirect && call.sources.len() == 1, "{call:?}");
    assert!(done.calls.is_empty());
}

const REGISTER_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare cc 1002 i32 @g(i32)
define i32 @main() {
entry:
  %r = call cc 1002 i32 @g(i32 7)
  ret i32 %r
}
";

/// A call that takes its arguments in registers must name them: the copies
/// into EAX, EDX and EBX were plain moves to physical registers, so the
/// allocator saw no use of the third, left EBX unset and the program hung
/// (`es_across_copy`).
#[test]
fn a_call_names_the_registers_it_takes_arguments_in() {
    let module = crate::testing::module(REGISTER_CALL).expect("parses");
    let done = selected(&module, "main", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    assert!(call.requires.len() == 1, "{:?} {:?} {:?}", call.requires, call.clobbers, call.what);
    assert!(call.requires.iter().all(|(held, _)| call.uses.contains(&held.value)));
}

const M16_SUM: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1005 i16 @f(i16 %a, i16 %b) addrspace(1) {
entry:
  %s = add i16 %a, %b
  ret i16 %s
}
";

/// At m16 the arguments arrive in registers and the procedure is far. The
/// driver refused every m16 function ("only m32"); once it took them, the
/// return was named `ret` and printed as a near return in a far procedure.
#[test]
fn an_m16_function_takes_register_arguments_and_returns_unnamed() {
    let module = crate::testing::module(M16_SUM).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let arrival = insns.iter().find(|i| !i.delivers.is_empty()).expect("an arrival instruction");
    assert_eq!(arrival.delivers.len(), 2, "both arguments arrive in registers");
    assert!(arrival.delivers.iter().all(|(held, _)| held.width == 2), "as 16-bit views");
    let last = insns.last().and_then(|i| i.what.as_ref()).expect("a return");
    assert_eq!((last.op, last.name.as_deref()), (crate::model::ir::Operation::Return, Some("")));
}

const BOTH_CALLS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare cc 1002 i32 @w(i32)
declare i32 @c(i32)
define i32 @main() {
entry:
  %a = call cc 1002 i32 @w(i32 7)
  %b = call i32 @c(i32 %a)
  ret i32 %b
}
";

/// A call destroys what its callee's convention says: the DAG path gave every
/// call the target-wide set, EAX alone, so a value held in ECX or EDX across a
/// cdecl call was destroyed (`es_across_copy` looped forever).
#[test]
fn a_call_destroys_what_its_conventions_says() {
    use llrm_target::Target;
    let module = crate::testing::module(BOTH_CALLS).expect("parses");
    let done = selected(&module, "main", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let calls: Vec<_> = insns.iter().filter(|i| i.call.is_some()).collect();
    let regs = llrm_lir::registers::Regs(llrm_x86_m32::M32.registers());
    let named = |name: &str| regs.root(llrm_x86_m32::M32.register_named(name));
    assert_eq!(calls.len(), 2);
    assert!(!calls[0].clobbers.contains(&named("ecx")), "watcall keeps ECX: {:?}", calls[0].clobbers);
    assert!(
        calls[1].clobbers.contains(&named("ecx")) && calls[1].clobbers.contains(&named("edx")),
        "{:?}",
        calls[1].clobbers
    );
}

const VARIADIC_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare cc 1002 i32 @w(i32, ...)
define i32 @main() {
entry:
  %a = call cc 1002 i32 (i32, ...) @w(i32 7, i32 8)
  ret i32 %a
}
";

/// A variadic call under watcall passes as cdecl does (`variadic =
/// \"cdecl32\"`) and destroys what cdecl does. The DAG path used watcall's set
/// (EAX alone), so a value held in ECX across `printf` or a variadic `sum` was
/// destroyed: tests/llc/varargs.c printed `1604196 36` for `15 36` at
/// x86_32-wat.
#[test]
fn a_variadic_call_destroys_what_the_variadic_convention_says() {
    use llrm_target::Target;
    let module = crate::testing::module(VARIADIC_CALL).expect("parses");
    let done = selected(&module, "main", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    let regs = llrm_lir::registers::Regs(llrm_x86_m32::M32.registers());
    let named = |name: &str| regs.root(llrm_x86_m32::M32.register_named(name));
    assert!(call.clobbers.contains(&named("ecx")) && call.clobbers.contains(&named("edx")), "{:?}", call.clobbers);
}

const BYVAL_ADDRESS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @bar(ptr)
define void @f(ptr %z, ptr byval([20 x i8]) %x, ptr %y) {
entry:
  call void @bar(ptr %x)
  ret void
}
";

/// The builder numbers a byval parameter's fixed object like any other; the
/// frame left it out, so the address of `x` was the next parameter's, 20 bytes
/// up (byvalue_struct_address passed `y`'s place to `bar`).
#[test]
fn a_byval_parameter_is_a_fixed_object_of_its_own() {
    let module = crate::testing::module(BYVAL_ADDRESS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let lea = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == crate::model::ir::Operation::Address)
        .expect("the address of x");
    let crate::model::ir::Loc::Address(a) = &lea.sources[0] else { panic!("{lea:?}") };
    assert_eq!(a.addr.map(|a| a.disp), Some(12), "x follows z at BP+8: {lea:?}");
}

const DIV64: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %a, i64 %b) {
entry:
  %q = sdiv i64 %a, %b
  ret i64 %q
}
";

/// An `i64` divide is a call to the target's inline routine; the DAG path left
/// the call without the routine's bytes, so the object had an undefined
/// `divN_s` (link failures in c/int64_division.c, c/data_int64.c of the m32
/// run). The routine's code must travel with the selected function, at the
/// call.
#[test]
fn an_i64_divide_carries_the_routines_code_at_its_call() {
    let module = crate::testing::module(DIV64).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let routine = llrm_target::Target::divide_routine(&llrm_x86_m32::M32, true, true).expect("a routine");
    let at = done.calls.iter().find(|(_, name)| name.as_str() == routine.name).map(|(at, _)| *at).expect("a call");
    assert_eq!(done.inline.get(&at), Some(&routine.code));
}

const VARIADIC: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @llvm.va_start.p0(ptr)
define i32 @f(i32 %n, ...) {
entry:
  %ap = alloca ptr, align 4
  call void @llvm.va_start.p0(ptr %ap)
  ret i32 %n
}
";

/// `va_start` stores the address of the next fixed object, the first variadic
/// argument; the frame had no cell for it ("frame index -2" in the variadic
/// programs).
#[test]
fn va_start_finds_the_first_variadic_argument() {
    let module = crate::testing::module(VARIADIC).expect("parses");
    let why = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .err()
        .map(|e| e.0);
    assert!(why.is_none(), "{why:?}");
}

const M16_DIRECT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1005 i16 @g(i16)
define cc 1005 i16 @f(i16 %a) addrspace(1) {
entry:
  %r = call cc 1005 i16 @g(i16 %a)
  ret i16 %r
}
";

/// At m16 a call of a symbol is `CALL16r` of its address; it was emitted as a
/// call through an immediate (`call offset _report`), the biggest cause in the
/// m16 execution run (79 programs).
#[test]
fn an_m16_call_of_a_symbol_is_a_direct_call() {
    let module = crate::testing::module(M16_DIRECT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.calls.values().collect::<Vec<_>>(), ["g"]);
    let insns = done.body.insns();
    let call =
        insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.op == crate::model::ir::Operation::Call).unwrap();
    assert!(!call.indirect && call.sources.is_empty(), "{call:?}");
}

/// Real mode has no `[sp+k]`: the outgoing arguments are pushed. The stores
/// into the area stopped 32 QCport modules from assembling.
#[test]
fn an_m16_call_pushes_its_stack_arguments() {
    let module = crate::testing::module(M16_STACK_ARGS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let pushes =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Push).count();
    assert_eq!(pushes, 2);
    let through_sp = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .any(
            |w| w.dests
                .iter()
                .any(
                    |l| matches!(
                        l,
                        crate::model::ir::Loc::Mem(m) if m.through == llrm_lir::registers::RegId::SP
                    ),
                ),
        );
    assert!(!through_sp, "no store through SP");
}

/// A call of a 16-bit callee destroys the upper half of `esi` and `edi` (it
/// pushes only `si` and `di`). The DAG path left `clobbers_high` empty, so a
/// 32-bit value kept in `esi` across a call came back with a stale upper half
/// (call_keeps, frameless, div_constants_*, phi_slot_unknown_arm under
/// `LLRM_ISEL_REAL`).
#[test]
fn a_call_of_a_16_bit_callee_destroys_the_upper_halves() {
    let module = llrm_mir::parse::module(m16_ir(FAR_CALL)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let high: Vec<_> = done
        .body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.call.is_some())
        .map(|one| one.clobbers_high.clone())
        .collect();
    let expected = std::collections::BTreeSet::from([llrm_lir::registers::RegId::ESI, llrm_lir::registers::RegId::EDI]);
    assert_eq!(high, [expected.clone(), expected]);
}

const PAIR: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i32 %a, i32 %b) {
entry:
  %hi = zext i32 %a to i64
  %sh = shl i64 %hi, 32
  %lo = zext i32 %b to i64
  %r = or i64 %sh, %lo
  ret i64 %r
}
";

/// `pair(a, b)` returned `(b << 32) | b`: the low half was copied to EAX, where
/// `a` was held for the high half, and the `ret` required nothing, so the
/// allocator saw no claim on EAX. The `ret` requires both return registers.
#[test]
fn a_ret_requires_the_registers_it_returns_in() {
    let module = crate::testing::module(PAIR).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let ret = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Return)).expect("a ret");
    let mut registers: Vec<_> = ret.requires.iter().map(|(_, register)| *register).collect();
    registers.sort_by_key(|r| r.index());
    assert_eq!(registers, [llrm_lir::registers::RegId::EAX, llrm_lir::registers::RegId::EDX], "{ret:?}");
}

const EXTRA_ARGS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare watcallcc i32 @none()
define i32 @f() {
entry:
  %r = call watcallcc i32 (i32, i32, i32, i32, i32, i32) @none(i32 1, i32 2, i32 3, i32 4, i32 5, i32 6)
  ret i32 %r
}
";

/// A call with more arguments than the callee declares (an old-style `()`
/// definition): the callee pops what it declares, so the caller removes the
/// rest. The DAG path left them on the stack and `main` returned elsewhere
/// (old_style_extra_args -m32: exception 0Dh).
#[test]
fn the_caller_removes_the_arguments_the_callee_does_not_declare() {
    let module = crate::testing::module(EXTRA_ARGS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let text = format!("{:?}", done.body.insns().iter().filter_map(|i| i.what.as_ref()).collect::<Vec<_>>());
    // Two stack arguments (8 bytes) of six, and the callee pops none of them:
    // the destroy pseudo has the caller remove all 8.
    let insns = done.body.insns();
    let destroy = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.name.as_deref() == Some("call_frame_destroy"))
        .unwrap_or_else(|| panic!("{text}"));
    let values: Vec<i64> =
        destroy.sources.iter().filter_map(|s| if let Loc::Imm(imm) = s { Some(imm.value) } else { None }).collect();
    assert_eq!(values, [8, 0], "{text}");
}

const REMAINDER_EXTENDED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %a) {
entry:
  %r = srem i64 %a, 1000003
  %t = trunc i64 %r to i32
  %s = sext i32 %t to i64
  %m = mul i64 %s, 1000003
  ret i64 %m
}
";

/// The sign extension of a 64-bit remainder's low word reads EDX through the
/// glue of its `CDQ` read of EAX; that read was taken for a second result of
/// the remainder libcall, which then delivered two values in EDX and the
/// allocator could not place them ("could not place every range", the
/// regparm3 matrix callees 4, 22 and 31 under `--isel=dag`).
#[test]
fn no_call_delivers_two_values_in_one_register() {
    let module = crate::testing::module(REMAINDER_EXTENDED).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    for insn in done.body.insns().iter().filter(|i| !i.delivers.is_empty()) {
        let mut registers: Vec<_> = insn.delivers.iter().map(|(_, register)| *register).collect();
        let all = registers.len();
        registers.sort_by_key(|r| r.index());
        registers.dedup();
        assert_eq!(registers.len(), all, "{:?}", insn.delivers);
    }
}

const CALL_RETURNING_I64: &str = "target datalayout = \"@m32-datalayout@\"
define internal cc 1002 i64 @t_(double noundef %0) {
b1:
  %1 = fptosi double %0 to i64
  ret i64 %1
}
define i64 @f(double noundef %d) {
b1:
  %r = call cc 1002 i64 @t_(double noundef %d)
  ret i64 %r
}
";

/// A call returning an i64 delivers both halves. The EDX read took the call's
/// glue only when the convention's clobber list or the target's named EDX; a
/// callee that returns in EDX without clobbering it made `mov v, EDX` read a
/// register nothing defined (`floatalloc: _main reads edx before every path
/// writes it`, double_to_int64.c -O0 under `--isel=dag`).
#[test]
fn a_call_delivers_both_halves_of_an_i64_result() {
    let module = crate::testing::module(CALL_RETURNING_I64).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Call)).expect("a call");
    let mut registers: Vec<_> = call.delivers.iter().map(|(_, register)| *register).collect();
    registers.sort_by_key(|r| r.index());
    assert_eq!(registers, [llrm_lir::registers::RegId::EAX, llrm_lir::registers::RegId::EDX], "{call:?}");
}

/// The load of an incoming stack argument carried no invariant flag to LIR, so
/// MachineCSE and the sinker took it for memory a store may change: the pin's
/// `LowerMemArgument` makes the fixed object immutable and
/// `isDereferenceableInvariantLoad` takes the load.
#[test]
fn the_load_of_an_incoming_stack_argument_is_an_invariant_load_in_lir() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  ret i32 %a
}
";
    let module = crate::testing::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let flags: Vec<bool> = done
        .body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| {
            one.what
                .as_ref()
                .is_some_and(
                    |what| what.op == Operation::Move
                        && matches!(
                            what.sources.as_slice(),
                            [crate::model::ir::Loc::Mem(cell)] if cell.addr.as_ref().is_some_and(|a| a.space == crate::model::ir::Space::Frame)
                        ),
                )
        })
        .map(|one| one.invariant())
        .collect();
    assert!(!flags.is_empty(), "the premise: the argument is loaded from the frame");
    assert!(flags.iter().all(|&flag| flag), "{flags:?}");
}

#[test]
fn a_runtime_call_answering_in_registers_says_so() {
    assert_eq!(verdicts(false), [Ok(super::builder::RuntimeAnswer::Registers)]);
}

const QB_PAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 void @f() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND() to label %next unwind label %landing
next:
  ret void
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  ret void
}
declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)
declare i32 @llrm.qb.personality(...) addrspace(1)
";

/// The runtime enters a landing pad: no edge reaches it. The DAG path left it
/// a block nothing led to (`block 0x0004 is not reachable from the entry`,
/// 13 qb programs that register `ON ERROR`); it needs the old selector's pad
/// call and a root block that branches to it.
#[test]
fn a_landing_pad_is_marked_and_reachable_from_the_entry() {
    let done = select_with_contracts(QB_PAD).unwrap_or_else(|why| panic!("{why}"));
    let landing = done.landing.expect("the pad's address");
    let blocks = &done.body.blocks;
    let by_at: llrm_support::hash::HashMap<i64, &crate::model::lir::Block> = blocks.iter().map(|b| (b.at, b)).collect();
    let mut seen = llrm_support::hash::HashSet::default();
    let mut work = vec![done.body.entry];
    while let Some(at) = work.pop() {
        if seen.insert(at) {
            work.extend(by_at[&at].succ.iter().copied());
        }
    }
    assert!(seen.contains(&landing), "{seen:?} {landing}");
    assert_eq!(done.calls.get(&landing).map(String::as_str), Some("__LANDING"));
}

const INLINE_CODE: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare i32 @llrm.ia16.code.89a60000.2p0(ptr)
define i32 @f() {
entry:
  %slot = alloca i16
  %r = call i32 @llrm.ia16.code.89a60000.2p0(ptr %slot)
  ret i32 %r
}
";

/// Inline code (`llrm.ia16.code`) is laid down at its site, the frame place it
/// names patched in as a displacement from BP: the emitter had no arm for the
/// node and refused "opcode 30013" (QCport's `__emit__`, 3 functions).
#[test]
fn inline_code_is_laid_down_at_its_site_with_its_frame_place() {
    let module = llrm_mir::parse::module(m16_ir(INLINE_CODE)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.inline.len(), 1, "{:?}", done.inline);
    let (at, bytes) = done.inline.iter().next().unwrap();
    assert_eq!(done.inline_places.get(at).map(Vec::len), Some(1));
    assert!(!done.far.contains(at), "inline code is no far call");
    let (_, disp, addend) = done.inline_places[at][0];
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), (disp + addend) as u16);
}

const BYVAL_ODD: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare void @g(ptr byval([41 x i8]) %p)
define void @f(ptr %q) {
entry:
  call void @g(ptr byval([41 x i8]) %q)
  ret void
}
";

/// A byval struct whose size is not a whole number of words (41 bytes) leaves
/// its last byte of the outgoing area unwritten: the stores that cannot be
/// pushes stay in place, based on a copy of SP. It was refused "an outgoing
/// argument area the stores do not cover at m16" (byval_struct -O0).
#[test]
fn an_odd_sized_byval_argument_is_stored_in_place_at_m16() {
    let module = llrm_mir::parse::module(m16_ir(BYVAL_ODD)).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const PAD_READS_THE_ENTRY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@g = internal global i16 0
define cc 1000 void @f(i16 %x) addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  %y = add i16 %x, 1
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND() to label %next unwind label %landing
next:
  ret void
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  store i16 %y, ptr @g
  ret void
}
declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)
declare i32 @llrm.qb.personality(...) addrspace(1)
";

/// The runtime enters a landing pad with no register intact, so a value the
/// entry block defined is not there. The DAG path took such a function and the
/// verifier stopped the compile (`value is read but never defined`, qb
/// `onerr` and `erlnum`): it is refused and the old selector takes it.
#[test]
fn a_landing_pad_reading_an_entry_value_is_refused() {
    let refused = select_with_contracts(PAD_READS_THE_ENTRY).err().expect("refused");
    assert!(refused.contains("read and not defined"), "{refused}");
}

const RUNTIME_CALL_MORE_ARGS_THAN_DECLARED: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 void @f(i16 %a) addrspace(1) {
entry:
  call cc 1000 addrspace(1) void (i16, i16, i16) @llrm.qb.B$RDIM(i16 %a, i16 1, i16 2)
  ret void
}
declare cc 1000 void @llrm.qb.B$RDIM() addrspace(1)
";

/// A runtime routine's declaration names no parameters (`B$RDIM()`), but its
/// callers push their arguments and it pops them all. The builder took the
/// declared count as what the callee pops and left 4 bytes to the caller:
/// `add sp, 4` after the call, and qb `bounds.bas` (`LOCALS`) crashed once
/// pads were selected.
#[test]
fn a_runtime_call_pops_every_argument_it_is_given() {
    let module = crate::testing::module(RUNTIME_CALL_MORE_ARGS_THAN_DECLARED).expect("parses");
    let contracts = super::dag_function::RuntimeContracts {
        hole: 0,
        abi: &Stub { flags: false },
        segments: &crate::backend::target::BASIC,
        runtime: true,
        frame_zeroed: false,
        facts: None,
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let adjusts = done
        .body
        .insns()
        .iter()
        .filter(|i| {
            i.what
                .as_ref()
                .is_some_and(
                    |w| w.name.as_deref() == Some("add")
                        && w.dests.iter().any(|d| {
                            matches!(
                                d,
                                crate::model::ir::Loc::Reg(r) if r.register == crate::model::ir::STACK
                            )
                        }),
                )
        })
        .count();
    assert_eq!(adjusts, 0, "the caller removes nothing");
}

const MERGED_WITH_PHI: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b, ptr %p, ptr %q) {
entry:
  %x = icmp slt i32 %a, %b
  %l = load i32, ptr %p
  %m = load i32, ptr %q
  %s = mul i32 %l, %m
  %k = add i32 %a, 1
  %y = icmp sgt i32 %s, 7
  %o = or i1 %x, %y
  br i1 %o, label %t, label %e
t:
  br label %j
e:
  br label %j
j:
  %r = phi i32 [ %k, %t ], [ %b, %e ]
  %u = add i32 %r, %k
  ret i32 %u
}
";

/// A condition split into branches whose right side uses a value the join
/// block also reads: the value is exported from the first block and read in
/// the second, the phi takes its inputs from every block that branches to it.
#[test]
fn a_split_condition_exports_what_the_later_blocks_read() {
    let module = crate::testing::module(MERGED_WITH_PHI).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.body.blocks.len(), 5, "entry, the second compare, then, else, join");
}

const PORTS: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare i8 @llrm.ia16.in.i8(i16)
declare void @llrm.ia16.out.i8(i16, i8)
define i8 @f(i16 %p) {
entry:
  call void @llrm.ia16.out.i8(i16 968, i8 7)
  %v = call i8 @llrm.ia16.in.i8(i16 %p)
  ret i8 %v
}
";

/// Port I/O (`OUT`, `INP` of QB): a write to an immediate port, then a read
/// through `dx`, in that order, as `out` and `in` barriers. The builder
/// refused them ("intrinsic call llrm.ia16.out.i8": palette_using_pds71).
#[test]
fn port_writes_and_reads_stay_in_order() {
    let module = llrm_mir::parse::module(m16_ir(PORTS)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<&str> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.op == crate::model::ir::Operation::Barrier)
        .filter_map(|w| w.name.as_deref())
        .collect();
    assert_eq!(names, ["out", "in"]);
}

const INVOKE_RESULT_READ_BELOW: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 void @f() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  %0 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 65) to label %next unwind label %landing
next:
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %0) to label %done unwind label %landing
done:
  ret void
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  ret void
}
declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1)
declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)
declare i32 @llrm.qb.personality(...) addrspace(1)
";

/// An invoke's result read in its normal block: the export copy to a virtual
/// register was skipped (a terminator), so the value was read where no path
/// defined it ('value N is read and not defined on every path': the QB
/// programs with ON ERROR, erlnum, intrinsics_errors_*, ownf_err_*, resdiv,
/// resnext).
#[test]
fn an_invokes_result_read_in_the_normal_block_is_defined() {
    select_with_contracts(INVOKE_RESULT_READ_BELOW).unwrap_or_else(|why| panic!("{why}"));
}

const SDIV_FIX: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %r = call i32 @llvm.sdiv.fix.i32(i32 %a, i32 %b, i32 9)
  ret i32 %r
}
declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg)
";

/// `sdiv.fix` is a divide of the numerator shifted up by the scale, rounded
/// toward negative infinity. The builder widens by one bit and the i33 result
/// stopped the type legalizer (`promote result`: `nbody_fixed` under
/// `--isel=dag`).
#[test]
fn a_fixed_point_divide_is_selected() {
    let made = names(SDIV_FIX).unwrap_or_else(|why| panic!("{why}"));
    // the numerator shifted up, the 64-bit divide and remainder as helper calls
    assert!(made.iter().any(|name| name == "shld"), "{made:?}");
    assert_eq!(made.iter().filter(|name| *name == "call").count(), 2, "{made:?}");
}

const TRUNCATED_LONG_RESULT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"

@_inbuf = global [512 x i8] zeroinitializer
@_inpos = global [2 x i8] zeroinitializer
@_inlen = global [2 x i8] zeroinitializer
@_infile = global [2 x i8] zeroinitializer
@_bad = global [2 x i8] zeroinitializer

define cc 1005 i16 @f() addrspace(1) {
b1:
  %0 = load i16, ptr @_inpos
  %1 = load i16, ptr @_inlen
  %2 = icmp eq i16 %0, %1
  br i1 %2, label %b3, label %b2

b2:
  %3 = phi i16 [ 0, %b3 ], [ %0, %b1 ]
  %4 = add nsw i16 %3, 1
  store i16 %4, ptr @_inpos
  %5 = getelementptr inbounds i8, ptr @_inbuf, i16 %3
  %6 = load i8, ptr %5
  %7 = zext i8 %6 to i16
  br label %b6

b3:
  %8 = load i16, ptr @_infile
  %9 = tail call addrspace(1) i32 @LL$READ(i16 noundef %8, ptr addrspace(1) noundef addrspacecast (ptr @_inbuf to ptr addrspace(1)), i16 noundef 512)
  %10 = trunc i32 %9 to i16
  store i16 %10, ptr @_inlen
  store i16 0, ptr @_inpos
  %11 = icmp slt i16 %10, 1
  br i1 %11, label %b5, label %b2

b5:
  store i16 0, ptr @_inlen
  store i16 1, ptr @_bad
  br label %b6

b6:
  %12 = phi i16 [ 0, %b5 ], [ %7, %b2 ]
  ret i16 %12
}

declare i32 @LL$READ(i16, ptr addrspace(1), i16) addrspace(1)
";

/// A long result cut to an int read DX too: `movzx eax, ax; shl edx, 16; add`
/// built the 32-bit value (a spilled `xor ax, ax; shl eax, 16` once DX was
/// found unneeded), where the int is AX. jpeg_single: +379 bytes at m16.
#[test]
fn a_long_result_truncated_to_an_int_does_not_read_dx() {
    let done = select_with_contracts(m16_ir(TRUNCATED_LONG_RESULT)).unwrap_or_else(|why| panic!("{why}"));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    for wide in ["shl", "or"] {
        assert!(!names.iter().any(|n| n == wide), "{names:?}");
    }
}

const WORD_ARGUMENTS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1005 void @g(i16, i16, i16)
define cc 1005 void @f(i16 %p) {
  %q = add i16 %p, 100
  call cc 1005 void @g(i16 %q, i16 %p, i16 1)
  ret void
}
";

/// A word argument was required in `EAX`/`EDX`: to the allocator a dword, so
/// the pointer it came from was a dword too and every one live across a call
/// was spilled and reloaded as one (QCport `-Os`: +16% against the old
/// selector, `_host_frame` +1736 bytes).
#[test]
fn a_word_argument_is_required_in_the_word_register() {
    use llrm_lir::registers::RegId;
    let module = crate::testing::module(WORD_ARGUMENTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    let registers: Vec<RegId> = call.requires.iter().map(|(_, register)| *register).collect();
    assert!(registers.iter().all(|r| matches!(*r, RegId::AX | RegId::DX | RegId::CX)), "{registers:?}");
    assert_eq!(registers.len(), 3, "{registers:?}");
}

const CALL_AT_ADDRESS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define void @f() {
entry:
  call void inttoptr (i32 4096 to ptr)()
  ret void
}
";

/// A call whose callee is a constant expression (`inttoptr` of a number) was
/// refused ("call of a constant expression"); the pin calls the value.
#[test]
fn a_call_of_a_constant_expression_calls_its_value() {
    let module = crate::testing::module(CALL_AT_ADDRESS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.op == Operation::Call).expect("a call");
    assert!(call.indirect, "{call:?}");
}

/// A call of a BASIC-convention routine (`pascal far`: cc 1000, no row in
/// calling.toml) was taken to destroy every register, SI and DI too: the
/// pointer a function kept in SI was spilled to the frame and reloaded after
/// each call (`_part_init`: a frame, 6 reloads). The ABI's contract says it
/// keeps SI and DI but their upper halves.
#[test]
fn a_call_of_a_basic_routine_keeps_si_and_di() {
    use llrm_lir::registers::RegId;
    const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 void @f() addrspace(1) {
entry:
  call cc 1000 addrspace(1) void @user(i16 1)
  ret void
}
declare cc 1000 void @user(i16) addrspace(1)
";
    /// The C frontend's ABI: a routine keeps SI and DI.
    struct Keeps;
    impl crate::backend::assemble::Abi for Keeps {
        fn contract(
            &self,
            callee: &str,
            pops: bool,
            pushed: i64,
        ) -> Result<crate::abi::runtime::Contract, String> {
            use crate::abi::runtime::Reg;
            let cleanup =
                if pops { crate::hir::model::StackCleanup::Callee } else { crate::hir::model::StackCleanup::Caller };
            Ok(crate::abi::qb::ordinary(callee, cleanup, pushed, &[Reg::Si, Reg::Di].into_iter().collect()))
        }

        fn linked(
            &self,
            name: &str,
        ) -> String {
            name.to_owned()
        }
    }
    let module = crate::testing::module(IR).expect("parses");
    let contracts = super::dag_function::RuntimeContracts {
        hole: 0,
        abi: &Keeps,
        segments: &crate::backend::target::BASIC,
        runtime: false,
        frame_zeroed: false,
        facts: None,
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let call = done.body.insns().iter().find(|i| i.call.is_some()).cloned().expect("a call");
    let root = |register: RegId| crate::model::ir::root(register);
    let lost: Vec<RegId> = call.clobbers.iter().map(|r| root(*r)).collect();
    assert!(!lost.contains(&RegId::ESI) && !lost.contains(&RegId::EDI), "{lost:?}");
    let high: Vec<RegId> = call.clobbers_high.iter().map(|r| root(*r)).collect();
    assert!(high.contains(&RegId::ESI) && high.contains(&RegId::EDI), "{high:?}");
}

const M16_FAR_POINTER_AS_LONG_ARGUMENT: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1005 i16 @g(i16, i16, i16, i32, i32) addrspace(1)
define cc 1005 i16 @f(i16 %file, ptr addrspace(1) %p) addrspace(1) {
entry:
  %l = ptrtoint ptr addrspace(1) %p to i32
  %r = call cc 1005 addrspace(1) i16 @g(i16 %file, i16 %file, i16 %file, i32 %l, i32 102)
  ret i16 %r
}
";

/// A far pointer passed as a `long` on the stack was merged into one register
/// (`mov dx, ax; shl edx, 16; add edx, ecx; push edx`, 11 bytes) and pushed;
/// its two words are pushed as they are (`push es; push bx`). The pin keeps
/// int halves merged (`isMultiStoresCheaperThanBitsMerge`); m16 deviates
/// (SD240). QCport -Os: 131 stack arguments.
#[test]
fn a_far_pointer_pushed_as_a_long_is_two_word_pushes() {
    let module = crate::testing::module(M16_FAR_POINTER_AS_LONG_ARGUMENT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let names: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(!names.contains(&"shl"), "no merge of the halves: {names:?}");
}

const WATCALL_FROM_M16: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1002 i16 @w(i16)
define i16 @f(i16 %a) {
entry:
  %r = call cc 1002 i16 @w(i16 %a)
  ret i16 %r
}
";

/// A call in a register convention changes FS and GS though its description
/// names neither: the DAG path kept a far pointer's segment in FS across it
/// (`fsgs_call` at -Os read -27430 for 70).
#[test]
fn a_register_convention_call_changes_fs_and_gs() {
    let done = super::dag_function_tests::select_with_contracts(WATCALL_FROM_M16).unwrap_or_else(|why| panic!("{why}"));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    let (fs, gs) = (llrm_lir::registers::RegId::FS, llrm_lir::registers::RegId::GS);
    assert!(call.clobbers.contains(&fs) && call.clobbers.contains(&gs), "{:?}", call.clobbers);
}

/// A hosted QB routine at x86-64, where the caller pops what it pushed: the
/// float's stack block was "a stack cleanup the convention does not model" and
/// the DAG path refused every QB program that prints a number.
#[test]
fn a_runtime_call_the_caller_cleans_up_is_modelled() {
    let text = "target datalayout = \"@m64-datalayout@\"
define void @f() {
entry:
  tail call void @llrm.qb.B$PER4(float 7.0)
  ret void
}
declare void @llrm.qb.B$PER4(float) nocallback
";
    let module = crate::testing::module(text).expect("parses");
    let function = module.global(module.named("f").expect("f")).function().expect("a body");
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let verdicts = super::dag_function::runtime_call_verdicts(
        &module,
        function,
        &layout,
        &llrm_x86_m64::M64,
        &super::dag_function_tests::Stub { flags: false },
    );
    assert_eq!(verdicts.values().copied().collect::<Vec<_>>(), [Ok(super::builder::RuntimeAnswer::Registers)]);
}

const SELECT_BEFORE_CALL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare void @g(i16)
declare void @h(i16)
define void @f(i16 %a) {
entry:
  %c = icmp eq i16 %a, 1
  %s = select i1 %c, i16 99, i16 7
  call void @g(i16 233)
  call void @h(i16 %s)
  ret void
}
";

const PUSH_SEGMENT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@g = global i16 0
declare void @h(ptr addrspace(2), ptr)
define void @f(ptr %p) {
entry:
  %s = addrspacecast ptr %p to ptr addrspace(2)
  call void @h(ptr addrspace(2) %s, ptr %p)
  ret void
}
";

const BYVAL_FORTY: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare void @g(ptr byval([40 x i8]) %p)
define void @f(ptr %q) minsize {
entry:
  call void @g(ptr byval([40 x i8]) %q)
  ret void
}
";

/// A select the IR has before a call is made before it at m16, as the pin's
/// source-order queue has it: the select's branches fell inside the first
/// call's frame, so its argument store was no longer a push and `jumps` grew
/// by 28 B.
#[test]
fn a_select_before_a_call_stays_above_the_call_frame() {
    let module = crate::testing::module(SELECT_BEFORE_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> = done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect();
    let call = names.iter().position(|n| n == "call").expect("a call");
    assert_eq!(names[call - 1], "push", "the argument is pushed: {names:?}");
}

/// The segment of a global is pushed as an immediate (`push DGROUP`, 3 bytes);
/// the DAG path moved it to a register and pushed that (4 bytes, 205 times in
/// the QB corpus). Pin: `X86CallFrameOptimization` folds a `MOV32ri` of one
/// use.
#[test]
fn a_pushed_segment_of_a_global_is_an_immediate() {
    let module = llrm_mir::parse::module(m16_ir(PUSH_SEGMENT)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let pushes: Vec<_> =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.name.as_deref() == Some("push")).collect();
    assert!(pushes.iter().any(|w| matches!(&w.sources[..], [Loc::Imm(imm)] if imm.address.is_some())), "{pushes:?}");
}

const CONSTANT_PUSHED_AROUND_A_SELECT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@\"INPUTVALUE&\" = internal global [4 x i8] zeroinitializer
@\"BRANCHCHOICE%\" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c\"\\04\\00\", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c\"DONE\" }>
define cc 1000 void @f() addrspace(1) \"llrm-fp-eval\"=\"x87-extended\" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @\"INPUTVALUE&\" to ptr addrspace(1)))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @\"BRANCHCHOICE%\" to ptr addrspace(1)))
  %0 = load i32, ptr @\"INPUTVALUE&\"
  %1 = add i32 %0, 1
  store i32 %1, ptr @\"INPUTVALUE&\"
  %2 = load i16, ptr @\"BRANCHCHOICE%\"
  %3 = icmp eq i16 %2, 0
  %4 = mul i32 %1, %1
  %5 = select i1 %3, i32 -1, i32 1
  %6 = add i32 %5, %4
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %6)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %4)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @\"INPUTVALUE&\" to ptr addrspace(1)))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @\"BRANCHCHOICE%\" to ptr addrspace(1)))
  %7 = load i32, ptr @\"INPUTVALUE&\"
  %8 = add i32 %7, 1
  store i32 %8, ptr @\"INPUTVALUE&\"
  %9 = load i16, ptr @\"BRANCHCHOICE%\"
  %10 = icmp eq i16 %9, 0
  %11 = mul i32 %8, %8
  %12 = select i1 %10, i32 -1, i32 1
  %13 = add i32 %12, %11
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %13)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}
declare cc 1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1) nocallback
declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1) nocallback
declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback
declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback
declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback
";

/// One segment pushed by two calls with a select between them: the second
/// push is in a block after the select's arms, so the first block saw one read
/// and pushed the immediate, removing the `mov` the second push read
/// (`value 1 is read and not defined on every path`): `qb/gvnjn.bas` and
/// `gvnphi.bas` stopped compiling at -O2 m16.
#[test]
fn a_segment_pushed_by_two_calls_around_a_select_is_still_defined() {
    select_with_contracts(m16_ir(CONSTANT_PUSHED_AROUND_A_SELECT)).unwrap_or_else(|e| panic!("{e}"));
}

/// A 40-byte byval copy is past the store limit (4 at -Os, 8 otherwise), so the
/// pin's `emitConstantSizeRepmov` makes it a `rep movs`: ten dword stores were
/// 28 bytes more in `byval_struct.c` (249 against the old selector's 193).
#[test]
fn a_forty_byte_byval_argument_is_a_string_move() {
    let module = llrm_mir::parse::module(m16_ir(BYVAL_FORTY)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let ops: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
    assert!(ops.iter().any(|name| name.starts_with("movs")), "{ops:?}");
}

/// A direct call of a private function that took part in `CalleeFacts` was
/// taken to destroy the whole convention, not what the callee wrote: `main`
/// kept a value in CX across `lsum_`, which wrote CX and saved nothing, and
/// `varargs` printed 4324 for 15 under DOSBox at m16.
#[test]
fn a_call_of_a_private_function_destroys_what_it_wrote() {
    use llrm_lir::registers::RegId;
    const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define internal i16 @leaf(i16 %a) {
entry:
  ret i16 %a
}
define i16 @f(i16 %x) {
entry:
  %r = call i16 @leaf(i16 %x)
  ret i16 %r
}
";
    let module = crate::testing::module(IR).expect("parses");
    let (facts, _) = crate::backend::calleefacts::CalleeFacts::of(&module);
    let frame = llrm_target::FrameRegisters {
        pointer: RegId::BP,
        stack: RegId::SP,
        saved: vec![],
        slot: 2,
        align: 2,
        optional: false,
        enter: false,
        free: false,
        wrap_split_size: false,
        slot_scratch: None,
    };
    let leaf = selected(&module, "leaf", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    facts.record("leaf", &leaf.body, &frame, &Default::default());
    let written = facts.written("leaf").expect("leaf takes part");
    assert!(!written.whole.contains(&RegId::ECX), "the leaf writes no CX: {:?}", written.whole);
    let contracts = super::dag_function::RuntimeContracts {
        hole: 0,
        abi: &Stub { flags: false },
        segments: &crate::backend::target::BASIC,
        runtime: false,
        frame_zeroed: false,
        facts: Some(&facts),
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let call = done.body.insns().iter().find(|i| i.call.is_some()).cloned().expect("a call");
    let lost: Vec<RegId> = call.clobbers.iter().map(|r| crate::model::ir::root(*r)).collect();
    assert!(!lost.contains(&RegId::ECX), "a call of leaf destroys {lost:?}, leaf wrote {:?}", written.whole);
}

const M16_UDIV_TEN: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1005 i64 @f(i64 %x) addrspace(1) {
entry:
  %q = udiv i64 %x, 10
  ret i64 %q
}
";

/// `x / 10` on an `i64` was always the `$div64` routine; the pin
/// (`expandDIVREMByConstant`) multiplies by the magic number at speed and
/// keeps the call at `-Os`.
#[test]
fn an_i64_udiv_by_a_constant_is_inline_at_speed_and_a_call_at_size() {
    let module = crate::testing::module(M16_UDIV_TEN).expect("parses");
    let calls = |size: bool| {
        let mut cpu = crate::backend::cpu::profile("386").expect("a cpu").clone();
        cpu.size = size;
        let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body
            .insns()
            .iter()
            .filter(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call))
            .count()
    };
    assert_eq!(calls(false), 0, "speed: multiplied by the magic number");
    assert_eq!(calls(true), 1, "-Os: the divide routine");
}

/// An indirect call with no stack arguments said nothing of what its callee
/// pops (`LowerCallFrame` says it only where a call frame was set up), so the
/// frame reader (`masm`) refused the function once the allocator gave the frame
/// register to a value: "the frame register was given to a value and the frame
/// cannot be addressed through the stack pointer" (QB runtime `B$VIEW` at m32
/// `-Os`, which broke the runtime build).
#[test]
fn an_indirect_call_with_no_arguments_says_it_pops_none() {
    let ir = "target datalayout = \"@m32-datalayout@\"
define void @f(ptr %fp) {
  call void %fp()
  ret void
}
";
    let module = crate::testing::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.body.call_pops.values().copied().collect::<Vec<_>>(), vec![0], "{:?}", done.body.call_pops);
}

const BASIC_CALL: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define cc 1000 i32 @g(i32 %a) {
entry:
  ret i32 %a
}
define cc 1000 i32 @f(i32 %a) {
entry:
  %r = call cc 1000 i32 @g(i32 %a)
  ret i32 %r
}
";

/// A call of a BASIC function disturbed every register the call clobbers, so
/// a caller that used none of them saved all of them for its own caller
/// (QB `parity/branch` -Os bq32: `BenchBranch` pushed ebx, ecx, edx, esi and
/// edi where the old selector pushed nothing, +57% executed instructions).
#[test]
fn a_call_of_a_basic_function_disturbs_what_its_convention_says() {
    let module = crate::testing::module(BASIC_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|one| one.call.is_some()).expect("a call");
    let disturbs = &call.call.as_ref().expect("a call").disturbs;
    assert!(disturbs.is_empty(), "{disturbs:?}");
}

const WATCALL_ARGUMENTS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare cc 1002 i32 @w(i32, i32, i32)
define cc 1002 i32 @f(i32 %a) {
entry:
  %r = call cc 1002 i32 @w(i32 %a, i32 2, i32 3)
  ret i32 %r
}
";

/// The registers a watcall callee's arguments arrive in are its to change
/// (`arguments_clobbered`), though its `clobbered` list names only EAX: a call
/// disturbed just that list (302389dd83), so a caller that keeps EDX or EBX
/// for its own caller never saved them, and QCport (Watcom) ran into an
/// illegal instruction.
#[test]
fn a_call_disturbs_the_registers_its_arguments_arrive_in() {
    use llrm_target::Target;
    let module = crate::testing::module(WATCALL_ARGUMENTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|one| one.call.is_some()).expect("a call");
    let disturbs = &call.call.as_ref().expect("a call").disturbs;
    let regs = llrm_lir::registers::Regs(llrm_x86_m32::M32.registers());
    let named = |name: &str| regs.root(llrm_x86_m32::M32.register_named(name));
    assert!(disturbs.contains(&named("edx")) && disturbs.contains(&named("ebx")), "{disturbs:?}");
}

const MINSIZE_CONSTANT_ARGUMENTS: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare void @g(i32, i32)
define void @f() minsize {
entry:
  call void @g(i32 0, i32 -1)
  ret void
}
";

/// At minsize 0 and -1 are stored with `and [m], 0` / `or [m], -1`; the
/// arguments stayed stores (`sub sp, 8; mov bx, sp; and [bx], 0`, +4 B
/// each, 4383 B over BAS at -Oz) where the pin's `classifyInstruction`
/// converts both to pushes.
#[test]
fn minsize_constant_arguments_are_pushed_not_stored() {
    let module = crate::testing::module(MINSIZE_CONSTANT_ARGUMENTS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> = done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect();
    let call = names.iter().position(|n| n == "call").expect("a call");
    assert_eq!(&names[call - 2..call], ["push", "push"], "both arguments are pushed: {names:?}");
    assert!(!names.iter().any(|n| n == "and" || n == "or"), "no constant store is left: {names:?}");
}

const MINSIZE_CONSTANT_STORES: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@a = global i16 7
@b = global i32 7
define void @f() minsize {
entry:
  store i16 0, ptr @a
  store i32 -1, ptr @b
  ret void
}
";

/// The pin's `OptForMinSize` store patterns (X86InstrCompiler.td:1308) key on
/// the function's `minsize`, which the selector ignored: `mov dword [b], -1`
/// (9 B at m16) stayed where `or dword [b], -1` is 6.
#[test]
fn minsize_stores_of_zero_and_minus_one_are_and_or() {
    let module = crate::testing::module(MINSIZE_CONSTANT_STORES).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> = done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect();
    assert!(names.iter().any(|n| n == "and") && names.iter().any(|n| n == "or"), "{names:?}");
    assert!(!names.iter().any(|n| n == "mov"), "no mov of an immediate is left: {names:?}");
}
