//! x86-64 (m64): quadword arithmetic, the SysV call sequence, loops over
//! arrays.

use llrm_target::Target;

use super::dag_function::selected;

/// An m64 sum of two `i64` arguments stopped at emit with
/// `Unmapped("ADD64rr")`: the m64 target had no 64-bit forms.
#[test]
fn an_m64_function_is_selected_through_the_dag() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(i64 %a, i64 %b) {
entry:
  %s = add i64 %a, %b
  ret i64 %s
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let add = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("add")).expect("an add");
    // The sum is a quadword: a 4-byte `add` would drop the high half.
    assert!(add.dests.iter().all(|d| matches!(d, crate::model::ir::Loc::Held(h) if h.width == 8)), "{add:?}");
}

/// A seven-argument call at m64 came out with 4-byte values and the 32-bit
/// `ESP` (`mov r, ESP`), and was refused. It now passes six arguments in the
/// registers `calling.toml` lists and the seventh on the stack at `RSP`.
#[test]
fn an_m64_call_passes_the_sysv_registers_and_one_stack_slot() {
    use llrm_lir::registers::RegId;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare i64 @g(i64, i64, i64, i64, i64, i64, i64)
define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, i64 %h) {
entry:
  %s = call i64 @g(i64 %b, i64 %a, i64 %c, i64 %d, i64 %e, i64 %f, i64 %h)
  ret i64 %s
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let call =
        insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("call"))).expect("a call");
    let registers: Vec<_> = call.requires.iter().map(|(held, reg)| (held.width, *reg)).collect();
    let expected = [RegId::RDI, RegId::RSI, RegId::RDX, RegId::RCX, RegId::R8, RegId::R9].map(|r| (8, r));
    assert_eq!(registers, expected);
    // The argument area is a multiple of 16: a call finds the stack aligned
    // (printf crashed in `movaps` with the 8-byte area).
    let sub = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.name.as_deref() == Some("sub"))
        .expect("the area reserved");
    assert!(matches!(sub.sources[1], crate::model::ir::Loc::Imm(ref i) if i.value == 16), "{sub:?}");
    let store = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| {
            matches!(
                w.dests.first(),
                Some(crate::model::ir::Loc::Mem(m)) if m.through == RegId::RSP
            )
        })
        .expect("the seventh argument stored at RSP");
    assert!(matches!(store.sources[0], crate::model::ir::Loc::Held(h) if h.width == 8), "{store:?}");
}

/// A quadword sum over an array at m64 stopped at emit with
/// `Unmapped("SUBREG_TO_REG")` (the zero constant `SUBREG_TO_REG 0, MOV32r0`);
/// it now selects a loop whose body adds a scaled quadword from memory.
#[test]
fn an_m64_loop_over_an_array_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(ptr %p, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %j, %loop ]
  %s = phi i64 [ 0, %entry ], [ %t, %loop ]
  %q = getelementptr i64, ptr %p, i64 %i
  %v = load i64, ptr %q
  %t = add i64 %s, %v
  %j = add i64 %i, 1
  %c = icmp slt i64 %j, %n
  br i1 %c, label %loop, label %done
done:
  ret i64 %t
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let names: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref().and_then(|w| w.name.as_deref())).collect();
    assert!(names.contains(&"jl"), "{names:?}");
    let add = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("add")).expect("an add");
    assert!(
        add.sources.iter().any(|s| matches!(s, crate::model::ir::Loc::Mem(m) if m.width == 8 && m.scale == 8)),
        "{add:?}"
    );
}

/// An `i64` select at m64 stopped at emit with `Unmapped("CMOV64rr")`; it is
/// now a compare and a `cmovb` of the two registers.
#[test]
fn an_m64_select_is_a_cmov() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(i64 %a, i64 %b) {
entry:
  %c = icmp ult i64 %a, %b
  %r = select i1 %c, i64 %a, i64 %b
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    // The compare is a `sub` until the pin's PeepholeOptimizer makes the `cmp`.
    let body = crate::backend::peepholeopt::compared(&done.body);
    let insns = body.insns();
    let names: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref().and_then(|w| w.name.as_deref())).collect();
    assert_eq!(names, ["", "cmp", "cmovb", ""]);
}

fn m64_names(text: &str) -> Vec<String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    done.body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)))
        .collect()
}

/// A quadword divide and remainder at m64 stopped at `CannotSelect("UDivRem")`:
/// only `i16` and `i32` had a divide sequence. It is now `div`, and `cqo` then
/// `idiv`.
#[test]
fn an_m64_divide_uses_rax_rdx() {
    let names = m64_names(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(i64 %x, i64 %y) {
entry:
  %d = udiv i64 %x, %y
  %e = srem i64 %x, %y
  %f = add i64 %d, %e
  ret i64 %f
}
",
    );
    for want in ["div", "cqo", "idiv"] {
        assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
    }
}

/// A signed divide by a constant at m64 stopped at `CannotSelect("SMulLoHi")`
/// (the multiply by the magic number): `MUL64r`/`IMUL64r` had no case.
#[test]
fn an_m64_divide_by_a_constant_multiplies_by_the_magic_number() {
    let names = m64_names(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(i64 %x) {
entry:
  %d = sdiv i64 %x, 7
  ret i64 %d
}
",
    );
    assert!(names.iter().any(|n| n == "imul"), "{names:?}");
    assert!(!names.iter().any(|n| n == "idiv"), "{names:?}");
}

/// A zero-extending load at m64 stopped with "a non-constant address part":
/// `SUBREG_TO_REG` keeps the folded load's chain after its index operand.
#[test]
fn an_m64_zero_extending_load_is_selected() {
    let names = m64_names(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(ptr %p) {
  %w = load i32, ptr %p
  %s = zext i32 %w to i64
  ret i64 %s
}
",
    );
    assert_eq!(names, ["", "mov", "mov", ""]);
}

/// `p + i + 12` at m64 stopped at `Unmapped("LEA64r")`; it is one `lea` over
/// quadword registers.
#[test]
fn an_m64_address_sum_is_one_lea() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define ptr @f(ptr %p, i64 %i) {
  %q = getelementptr i8, ptr %p, i64 %i
  %r = getelementptr i32, ptr %q, i64 3
  ret ptr %r
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let lea = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("lea")).expect("a lea");
    let Some(crate::model::ir::Loc::Held(result)) = lea.dests.first() else { panic!("{lea:?}") };
    assert_eq!(result.width, 8, "{lea:?}");
}

/// `a[i]` at m64 came out `movsx r, a[rax*4]`, an absolute symbol beside an
/// index: no `%rip`-relative form, and in a PIE the program crashed (array,
/// matrix, sieve of `tests/llc`). The symbol is now loaded by itself and the
/// index added to that register.
#[test]
fn a_global_array_element_is_addressed_through_a_register() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
@a = global [64 x i32] zeroinitializer
define i64 @f(i64 %i) {
  %p = getelementptr i32, ptr @a, i64 %i
  %v = load i32, ptr %p
  %s = sext i32 %v to i64
  ret i64 %s
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let load =
        insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("movsx")).expect("the load");
    let Some(crate::model::ir::Loc::Mem(cell)) = load.sources.first() else { panic!("{load:?}") };
    assert!(cell.base.is_some() && cell.index.is_some(), "{cell:?}");
    assert!(cell.addr.is_none_or(|addr| addr.disp == 0 && addr.base == llrm_lir::registers::RegId::None.iced()));
}

/// A select between a loaded value and a register is `CMOV64rm` at x86-64;
/// the emitter had only the register form ("a conditional move of a
/// non-register") and refused the function.
#[test]
fn an_m64_select_of_a_loaded_value_is_a_conditional_move_from_memory() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(ptr %p, i64 %a, i64 %b) {
entry:
  %v = load i64, ptr %p
  %c = icmp slt i64 %a, %b
  %r = select i1 %c, i64 %v, i64 %a
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let select = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == crate::model::ir::Operation::Select)
        .expect("a conditional move");
    let [crate::model::ir::Loc::Held(_), crate::model::ir::Loc::Mem(cell)] = select.sources.as_slice() else {
        panic!("{select:?}")
    };
    assert_eq!(cell.width, 8, "a quadword is read: {select:?}");
    assert!(select.name.as_deref().is_some_and(|name| name.starts_with("cmov")), "{select:?}");
}

/// A call through a pointer at m64 selected `CALL64r`, which the emitter did
/// not know: the call was a direct one with no callee and the assembler
/// stopped (`fnptr` of `tests/llc`). It is an indirect call.
#[test]
fn a_call_through_a_pointer_is_an_indirect_call() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i32 @f(ptr %p, i32 %a) {
  %r = call i32 %p(i32 %a)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let calls: Vec<bool> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.op == crate::model::ir::Operation::Call)
        .map(|w| w.indirect)
        .collect();
    assert_eq!(calls, [true]);
}

/// The address of a local array at m64 was a `mov r, offset <frame slot>` (the
/// symbol-address form): the object writer stopped with `KeyError: (FRAME,
/// 0)` (strings, qsortc of `tests/llc`). A frame address is an `lea` off the
/// frame register.
#[test]
fn the_address_of_a_local_is_an_lea() {
    let names = m64_names(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @sink(ptr)
define void @f() {
  %a = alloca [16 x i8]
  call void @sink(ptr %a)
  ret void
}
",
    );
    assert!(names.iter().any(|n| n == "lea"), "{names:?}");
}

/// A struct result's address (`sret`) takes RDI; the next argument was placed
/// in RDI too, so two live-in copies of one register overlapped and the greedy
/// allocator "could not place every range". It takes RSI.
#[test]
fn an_argument_after_an_sret_pointer_is_in_rsi() {
    use llrm_lir::registers::RegId;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define void @f(ptr noalias sret({i64, i64, i64}) %p, i64 %x) {
entry:
  store i64 %x, ptr %p, align 8
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let read: Vec<RegId> =
        done.body.insns().iter().flat_map(|i| i.delivers.iter().chain(&i.requires)).map(|(_, reg)| *reg).collect();
    assert!(read.contains(&RegId::RDI) && read.contains(&RegId::RSI), "{read:?}");
}

/// A `{i64, i64}` returned and received at m64 was refused ("aggregate or
/// vector value"): each member travels in its own return register, RAX then
/// RDX.
#[test]
fn a_pair_of_quadwords_is_returned_and_received_in_rax_and_rdx() {
    use llrm_lir::registers::RegId;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare {i64, i64} @g(i64)
define {i64, i64} @mk(i64 %a, i64 %b) {
entry:
  %x = insertvalue {i64, i64} undef, i64 %a, 0
  %y = insertvalue {i64, i64} %x, i64 %b, 1
  ret {i64, i64} %y
}
define i64 @use(i64 %a) {
entry:
  %r = call {i64, i64} @g(i64 %a)
  %lo = extractvalue {i64, i64} %r, 0
  %hi = extractvalue {i64, i64} %r, 1
  %s = add i64 %lo, %hi
  ret i64 %s
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let registers = |name: &str| -> Vec<RegId> {
        let done = selected(&module, name, &llrm_x86_m64::M64, cpu, &[]).expect("selected");
        done.body.insns().iter().flat_map(|i| i.delivers.iter().chain(&i.requires)).map(|(_, reg)| *reg).collect()
    };
    let returned = registers("mk");
    assert!(returned.contains(&RegId::RAX) && returned.contains(&RegId::RDX), "{returned:?}");
    let received = registers("use");
    assert!(received.contains(&RegId::RAX) && received.contains(&RegId::RDX), "{received:?}");
}

/// `va_start` at m64 was refused ("the SysV register save area is not
/// translated"; `varargs` of `tests/llc`). A variadic function stores the five
/// argument registers its one named parameter left in the register save area,
/// then the four fields of the `va_list`: nine stores.
#[test]
fn a_variadic_function_saves_the_argument_registers_and_fills_the_list() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
define i32 @f(i32 %n, ...) {
  %ap = alloca [24 x i8], align 8
  call void @llvm.va_start.p0(ptr %ap)
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %n
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let stores = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.op == Operation::Move && matches!(w.dests.first(), Some(Loc::Mem(cell)) if cell.width != 16))
        .count();
    assert_eq!(stores, 9);
}

/// `ret {i32, i64}` returned `eax = x`, `edx = x >> 16`: the one-integer path
/// halved the first field into the two return registers. The fields go to
/// `rax` and `rdx` each at its own width.
#[test]
fn an_m64_mixed_width_aggregate_return_fills_eax_and_rdx() {
    use llrm_lir::registers::RegId;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define { i32, i64 } @f(i32 %x, i64 %y) {
entry:
  %a = insertvalue { i32, i64 } undef, i32 %x, 0
  %b = insertvalue { i32, i64 } %a, i64 %y, 1
  ret { i32, i64 } %b
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let ret = done
        .body
        .insns()
        .iter()
        .find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Return))
        .expect("a ret")
        .clone();
    let returned: Vec<_> = ret.requires.iter().map(|(held, reg)| (held.width, *reg)).collect();
    assert_eq!(returned, [(4, RegId::EAX), (8, RegId::RDX)]);
}

/// A variadic function at m64 saves the vector argument registers in the
/// other half of the register save area when `al` says a caller used any
/// (`VASTART_SAVE_XMM_REGS`): `test al, al; je`, then eight `movaps` of
/// xmm0-7 at offsets 48..176. They were left unwritten, so a `double` read
/// back by `va_arg` was whatever the area held.
#[test]
fn a_variadic_function_saves_xmm0_to_xmm7_when_al_is_not_zero() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @llvm.va_start.p0(ptr)
define i32 @f(i32 %n, ...) {
  %ap = alloca [24 x i8], align 8
  call void @llvm.va_start.p0(ptr %ap)
  ret i32 %n
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let semantics: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let saves: Vec<i64> = semantics
        .iter()
        .filter(|w| w.name.as_deref() == Some("movaps"))
        .filter_map(|w| match (&w.dests[..], &w.sources[..]) {
            ([Loc::Mem(cell)], [Loc::Reg(register)]) if register.register.is_xmm() => cell.addr.map(|a| a.disp),
            _ => None,
        })
        .collect();
    assert_eq!(saves.len(), 8, "{semantics:?}");
    assert!(saves.windows(2).all(|pair| pair[1] - pair[0] == 16), "{saves:?}");
    let test = semantics.iter().position(|w| w.name.as_deref() == Some("test")).expect("a test of al");
    let jump = semantics.iter().position(|w| w.op == Operation::Branch).expect("a branch");
    assert!(test < jump);
}

/// A frame object's address plus a variable index and a displacement past the
/// object was made in a 4-byte register and used as the base of a quadword
/// address: the encoder refused the 32-bit base beside a 64-bit index.
#[test]
fn a_frame_address_past_its_object_is_a_quadword_base() {
    use crate::model::ir::Loc;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @use(ptr)
define void @f(i64 %n) {
entry:
  %buf = alloca [20 x i8], align 16
  %m = sub i64 %n, 1
  %p = getelementptr inbounds [20 x i8], ptr %buf, i64 0, i64 %m
  call void @use(ptr %p)
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let leas: Vec<_> =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.name.as_deref() == Some("lea")).collect();
    assert!(!leas.is_empty());
    for lea in &leas {
        assert!(matches!(&lea.dests[0], Loc::Held(h) if h.width == 8), "{lea:?}");
        // A displacement of -1 is -1, not 4294967295.
        assert!(!matches!(&lea.sources[..], [Loc::Mem(cell)] if cell.offset > i64::from(i32::MAX)), "{lea:?}");
    }
}

/// A dense switch at m64 was lowered to `BR_JT` over a `WrapperRIP` table
/// address, which nothing selects ("CannotSelect Target(954)", `-O0` tests/llc
/// switches): the target prices no jump table, so the DAG makes compares as the
/// MIR pass does.
#[test]
fn a_dense_switch_at_m64_is_compares_not_a_jump_table() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i32 @f(i32 %x) {
entry:
  switch i32 %x, label %d [ i32 0, label %a  i32 1, label %b  i32 2, label %c  i32 3, label %e  i32 4, label %g  i32 5, label %h ]
a: ret i32 10
b: ret i32 21
c: ret i32 33
e: ret i32 47
g: ret i32 52
h: ret i32 61
d: ret i32 0
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    assert!(done.body.insns().iter().all(|i| i.what.as_ref().is_none_or(|w| !w.indirect)));
}

/// A block fill or copy at m64 reached `REP_STOSB_64` / `REP_MOVSB_64` with the
/// count and pointers in `ecx`/`edi`/`esi`, as at m32, and stopped with
/// `Unmapped("REP_STOSB_64")`: 24 of the vsgcc programs (sieve, histo,
/// bintree...) were refused under `--isel=dag`. The pin copies through `RCX`,
/// `RDI`, `RSI`.
#[test]
fn an_m64_block_fill_and_copy_are_rep_string_operations_on_the_wide_registers() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
define void @f(ptr %d, ptr %s, i64 %n) {
entry:
  call void @llvm.memset.p0.i64(ptr %d, i8 0, i64 %n, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| Some(i.what.as_ref()?.name.as_deref()?.to_owned())).collect();
    assert!(names.iter().any(|n| n == "stosb") && names.iter().any(|n| n == "movsb"), "{names:?}");
}

/// A call of a variadic function under SysV x86-64 passes the number of
/// vector registers it uses in `al`; the callee's prologue tests it. No call
/// set it, so a callee that read it saw whatever was there.
#[test]
fn a_variadic_call_at_m64_sets_al() {
    use crate::model::ir::Loc;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare i32 @printf(ptr, ...)
define i32 @f(ptr %s, i32 %x) {
entry:
  %r = call i32 (ptr, ...) @printf(ptr %s, i32 %x)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    let al = call
        .requires
        .iter()
        .find(|(_, register)| *register == llrm_lir::registers::RegId::AL)
        .unwrap_or_else(|| panic!("the call does not require al: {:?}", call.requires));
    // no vector argument: zero of them
    let zero = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .any(
            |w| matches!(w.dests.first(), Some(Loc::Held(h)) if h.value == al.0.value)
                && matches!(w.sources.first(), Some(Loc::Imm(imm)) if imm.value == 0),
        );
    assert!(zero);
}

/// A `double` argument at m64 went to the stack (`Convention::kind` made every
/// float `Memory`), so `printf("%f", x)` read garbage and `al` was 0. It goes
/// in `xmm0`, with `al` 1.
#[test]
fn a_double_argument_at_m64_is_passed_in_xmm0_and_counted_in_al() {
    use crate::model::ir::Loc;
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare i32 @printf(ptr, ...)
define i32 @f(ptr %s, double %d) {
entry:
  %r = call i32 (ptr, ...) @printf(ptr %s, double %d)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    assert_eq!(call.requires.len(), 3, "rdi, xmm0 and al: {:?}", call.requires);
    let al = call.requires.iter().find(|(_, register)| *register == llrm_lir::registers::RegId::AL).expect("al").0;
    let one = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .any(
            |w| matches!(w.dests.first(), Some(Loc::Held(h)) if h.value == al.value)
                && matches!(w.sources.first(), Some(Loc::Imm(imm)) if imm.value == 1),
        );
    assert!(one, "al is the count of vector registers");
}

/// A `double` copied through memory at m64 stopped at emit with
/// `Unmapped("MOVSDrm_alt")` (the pin's name for the scalar load), and once
/// mapped came out as 10-byte x87 cells. It is a `movsd` load and store of
/// 8-byte cells, `float` 4.
#[test]
fn an_m64_scalar_float_copy_is_movsd_and_movss_of_their_own_width() {
    for (ty, name, bytes) in [("double", "movsd_x", 8), ("float", "movss", 4)] {
        let text = format!(
            "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define void @f(ptr %p, ptr %q) {{
entry:
  %v = load {ty}, ptr %p
  store {ty} %v, ptr %q
  ret void
}}
"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
        let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
        let insns = done.body.insns();
        let moves: Vec<_> = insns
            .iter()
            .filter_map(|i| i.what.as_ref())
            .filter(|w| w.name.as_deref() == Some(name))
            .map(|w| {
                let width = |loc: &crate::model::ir::Loc| match loc {
                    crate::model::ir::Loc::Mem(m) => m.width,
                    crate::model::ir::Loc::Held(h) => h.width,
                    _ => 0,
                };
                (width(&w.dests[0]), width(&w.sources[0]))
            })
            .collect();
        assert_eq!(moves, [(bytes, bytes), (bytes, bytes)], "{ty}");
    }
}

/// A zero `double` at m64 stopped at `Unmapped("FsFLD0SD")`, and a select of
/// two doubles at `Unmapped("CMOV_FR64")`: the first is `xorps r, r`, the
/// second the pin's branch and phi like `CMOV_GR32`.
#[test]
fn an_m64_zero_double_is_xorps_and_a_double_select_is_a_diamond() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define double @z() {
entry:
  ret double 0.0
}
define double @s(i32 %c, double %a, double %b) {
entry:
  %t = icmp ne i32 %c, 0
  %r = select i1 %t, double %a, double %b
  ret double %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let zero = selected(&module, "z", &llrm_x86_m64::M64, cpu, &[]).expect("a zero double is selected");
    let names: Vec<_> = zero.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
    assert!(names.iter().any(|n| &**n == "xorps"), "{names:?}");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let select = selected(&module, "s", &llrm_x86_m64::M64, cpu, &[]).expect("a double select is selected");
    assert!(select.body.blocks.len() >= 3, "a branch and a sink: {} blocks", select.body.blocks.len());
}

/// `fneg` and `fabs` of a `double` or `float` at m64 stopped at
/// `CannotSelect(FXOR)`: the pin selects the sign mask's bitwise instruction
/// with the 16-byte pool constant as its memory operand (`xorps`/`andps`).
#[test]
fn an_m64_fneg_and_fabs_are_a_bitwise_op_on_a_16_byte_mask() {
    for (op, ty, name) in [
        ("fneg double %x", "double", "xorps"),
        ("call double @llvm.fabs.f64(double %x)", "double", "andps"),
        ("fneg float %x", "float", "xorps"),
        ("call float @llvm.fabs.f32(float %x)", "float", "andps"),
    ] {
        let text = format!(
            "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare double @llvm.fabs.f64(double)
declare float @llvm.fabs.f32(float)
define {ty} @f({ty} %x) {{
entry:
  %r = {op}
  ret {ty} %r
}}
"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
        let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|e| panic!("{op}: {}", e.0));
        let logic: Vec<_> = done
            .body
            .insns()
            .iter()
            .filter_map(|i| i.what.as_ref())
            .filter(|w| w.name.as_deref() == Some(name))
            .filter_map(|w| match w.sources.last() {
                Some(crate::model::ir::Loc::Mem(m)) => Some(m.width),
                _ => None,
            })
            .collect();
        assert_eq!(logic, [16], "{op}");
    }
}

/// A bitcast between a float and an integer stopped at emit with
/// `Unmapped("MOVSDto64rr")` (`fconst`); the pin moves between the general and
/// the xmm register with `movq`/`movd`.
#[test]
fn an_m64_float_integer_bitcast_is_a_movq_or_movd() {
    for (from, to, name) in
        [("double", "i64", "movq"), ("i64", "double", "movq_x"), ("float", "i32", "movd"), ("i32", "float", "movd_x")]
    {
        let text = format!(
            "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define {to} @f({from} %x) {{
entry:
  %r = bitcast {from} %x to {to}
  ret {to} %r
}}
"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
        let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|e| panic!("{from}: {}", e.0));
        let insns = done.body.insns();
        let found = insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.name.as_deref() == Some(name)).count();
        assert_eq!(found, 1, "{from} to {to}");
    }
}

/// `llvm.copysign` was a call to the C library at m64; the pin selects the
/// magnitude and sign masks (`andps` x2) and joins them (`orps`).
#[test]
fn an_m64_copysign_is_two_masks_and_an_or() {
    for ty in ["double", "float"] {
        let text = format!(
            "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare {ty} @llvm.copysign.{s}({ty}, {ty})
define {ty} @f({ty} %x, {ty} %y) {{
entry:
  %r = call {ty} @llvm.copysign.{s}({ty} %x, {ty} %y)
  ret {ty} %r
}}
",
            s = if ty == "double" { "f64" } else { "f32" }
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
        let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|e| panic!("{ty}: {}", e.0));
        let insns = done.body.insns();
        let names: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
        let count = |n: &str| names.iter().filter(|x| **x == n).count();
        assert_eq!((count("andps"), count("orps")), (2, 1), "{ty}: {names:?}");
    }
}

/// `umul.with.overflow.i64` at m64 stopped at `CannotSelect("Target(776)")`:
/// `X86ISD::UMUL` had the one-operand `mul` at 8, 16 and 32 bits only. It is
/// `mul` with the operand in `rax`.
#[test]
fn an_m64_unsigned_multiply_with_overflow_is_a_quadword_mul() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare { i64, i1 } @llvm.umul.with.overflow.i64(i64, i64)
define i64 @f(i64 %a, i64 %b) {
entry:
  %r = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %a, i64 %b)
  %v = extractvalue { i64, i1 } %r, 0
  %o = extractvalue { i64, i1 } %r, 1
  %z = select i1 %o, i64 0, i64 %v
  ret i64 %z
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let names: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
    assert!(names.iter().any(|n| &**n == "mul"), "{names:?}");
}

/// `sitofp i32 to double` at m64 stopped at `CannotSelect("Target(586)")`:
/// the conversion took the x87 `fild` because the lowering was told there is
/// no SSE2. x86-64 has it, and the conversion is `cvtsi2sd`.
#[test]
fn an_m64_integer_to_double_conversion_is_cvtsi2sd() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define double @f(i32 %a) {
entry:
  %r = sitofp i32 %a to double
  ret double %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let names: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
    assert!(names.iter().any(|n| &**n == "cvtsi2sd"), "{names:?}");
}

/// A call in tail position at m64 stopped at `Unmapped("TCRETURNdi64")`
/// (`gfx_set_background`, `B_SPLY` of the QB runtime): the sibling call of the
/// 64-bit target is the 32-bit target's, a `jmp` to the callee.
#[test]
fn an_m64_sibling_call_is_a_jump_to_the_callee() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare i64 @g(i64)
define i64 @f(i64 %a) {
entry:
  %r = tail call i64 @g(i64 %a)
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let jump = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == crate::model::ir::Operation::Return && w.name.as_deref() == Some("jmp"));
    assert!(jump.is_some(), "a jmp that returns");
}

/// A tail call through a pointer that arrives in `rdi` jumped through a copy
/// (`mov rax,rdi; jmp *rax`, one instruction more than the pin's `jmp *rdi`):
/// the call's register was the first free one, not the one the pointer held.
#[test]
fn an_m64_sibling_call_through_an_argument_jumps_through_its_own_register() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define void @f(ptr %p) {
entry:
  tail call void %p()
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let jump = insns
        .iter()
        .find(|i| i.what.as_ref().is_some_and(|w| w.indirect && w.name.as_deref() == Some("jmp")))
        .expect("an indirect jmp");
    let regs = llrm_lir::registers::Regs(llrm_x86_m64::M64.registers());
    let (_, through) = jump.requires.last().expect("the callee's register");
    assert_eq!(regs.root(*through), regs.root(llrm_lir::registers::RegId::RDI));
}

/// A function returning a `double` at m64 carried the x87 return pseudo (an
/// unnamed `FloatStore` with no destination) that the stackifier reads at m32;
/// the listing printer indexed its empty destinations and panicked
/// (`masm.rs:2361`), and the ELF writer refused it. The result is in `xmm0`.
#[test]
fn an_m64_double_result_has_no_x87_return_store() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define double @f(i32 %a) {
entry:
  %r = sitofp i32 %a to double
  ret double %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    assert!(
        insns.iter().filter_map(|i| i.what.as_ref()).all(|w| w.op != crate::model::ir::Operation::FloatStore),
        "{insns:?}"
    );
}

/// `fptoui float to i64` at m64 was refused (no SSE form for an unsigned
/// quadword) or went through the x87 `fist` in memory. It is the signed
/// `cvttss2si` of the value less 2^63 when it is at least that, with the sign
/// bit put back.
#[test]
fn an_m64_unsigned_quadword_from_a_float_is_cvttss2si() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i64 @f(float %a) {
entry:
  %r = fptoui float %a to i64
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{}", why.0));
    let names: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
    assert!(names.iter().any(|n| &**n == "cvttss2si"), "{names:?}");
    assert!(names.iter().all(|n| !n.starts_with("fist")), "{names:?}");
}

/// Two `long double` arguments at m64 were required in XMM0 and XMM1 (the
/// float argument registers), so the allocator could not place an x87 value in
/// them: "the greedy allocator could not place every range"
/// (tests/run/c/long_double_variadic.c). SysV passes an extended real in
/// memory.
#[test]
fn an_m64_long_double_argument_is_not_required_in_a_vector_register() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @g(x86_fp80, x86_fp80)
define void @f(x86_fp80 %a, x86_fp80 %b) {
entry:
  call void @g(x86_fp80 %b, x86_fp80 %a)
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let call = done
        .body
        .insns()
        .into_iter()
        .find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("call")))
        .expect("a call");
    let in_vectors: Vec<_> = call.requires.iter().filter(|(_, reg)| reg.is_xmm()).collect();
    assert!(in_vectors.is_empty(), "long doubles required in {in_vectors:?}");
}

/// An address with an index and no base (`4 + 4*x`, a select of 4 and 8) at m64
/// widened `$noreg` like a value: `LEA64_32r`'s base was an `INSERT_SUBREG` of
/// nothing and the function was refused ("value N is read and not defined on
/// every path"; gcc.c-torture 20010422-1). The pin's is `leal 4(,%rax,4)`.
#[test]
fn an_m64_lea_with_no_base_has_no_base() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 5
  %r = select i1 %c, i32 4, i32 8
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
}

/// A `{ i64, i48 }` result (a packed struct returned in `rax:rdx`) stopped at
/// `Legalize(Unsupported CopyToReg i48 "promote operand")` (gcc.c-torture
/// 20131127-1): the member was copied to `rdx` at its own width. The pin
/// any-extends it to the register type (`getCopyToParts`).
#[test]
fn an_m64_aggregate_result_with_an_odd_member_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
@c = global { i64, i48 } zeroinitializer
define { i64, i48 } @f() {
entry:
  %v = load { i64, i48 }, ptr @c
  ret { i64, i48 } %v
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
}

/// The caller of a `{ i64, i48 }` function read the `i48` from `rdx` at its own
/// width (`Legalize(Unsupported CopyFromReg i48 "promote result")`, 20131127-1
/// at -O0): the register is read whole and truncated.
#[test]
fn an_m64_call_of_an_aggregate_with_an_odd_member_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare { i64, i48 } @g()
define i64 @f() {
entry:
  %v = call { i64, i48 } @g()
  %b = extractvalue { i64, i48 } %v, 1
  %z = zext i48 %b to i64
  ret i64 %z
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
}

/// A `long double` compare at m64 was `Emit(Unmapped("UCOM_FpIr80"))`: the
/// x86-64 CPUs have `fcomi`, so the table selects its flag-setting form.
#[test]
fn an_m64_long_double_compare_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-f80:128-n8:16:32:64\"
define i1 @f(x86_fp80 %a, x86_fp80 %b) {
entry:
  %r = fcmp olt x86_fp80 %a, %b
  ret i1 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    assert!(done.body.insns().iter().any(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("fcom"))));
}

/// A `long double` converted to `u64` at m64 was
/// `Emit(Unmapped("CMOVBE_Fp80"))` (`double_to_uint64.c`): the x87 conditional
/// moves name their condition in the opcode, and the select's diamond was only
/// made for `CMOV_RFP80`.
#[test]
fn an_m64_long_double_to_unsigned_quadword_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-f80:128-n8:16:32:64\"
define i64 @f(x86_fp80 %d) {
entry:
  %r = fptoui x86_fp80 %d to i64
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
}

/// A sibling call through a pointer at m64 was `CannotSelect("Target(6)")`: the
/// generated table checked the callee as `i32` (`ByHwMode0` resolved to the
/// 32-bit type), so `TCRETURNri64` never matched. tail_calls*.c were refused.
#[test]
fn an_m64_tail_call_through_a_pointer_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i32 @f(ptr %g, i32 %x) {
entry:
  %r = tail call i32 %g(i32 %x)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    assert!(done.body.insns().iter().any(|i| i.what.as_ref().is_some_and(|w| w.indirect)));
}

/// An `i24` parameter (a three-byte struct coerced, `struct_odd_sizes.c`) was
/// refused with "a i24 value": the convention knew only the legal widths. It is
/// promoted to a dword like the pin's `CCPromoteToType`.
#[test]
fn an_m64_parameter_of_an_odd_width_is_selected() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define i32 @f(i32 %n, i24 %x) {
entry:
  %t = lshr i24 %x, 16
  %e = zext i24 %t to i32
  %r = add i32 %n, %e
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
}

/// `llvm.ptrmask` was no known intrinsic: it was called as a global with no
/// name and the assembler panicked ("no entry found for key", clang's
/// `va_arg` of a `long double`, long_double_variadic.c). It is an AND.
#[test]
fn an_m64_ptrmask_is_an_and() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare ptr @llvm.ptrmask.p0.i64(ptr, i64)
define ptr @f(ptr %p) {
entry:
  %r = call ptr @llvm.ptrmask.p0.i64(ptr %p, i64 -16)
  ret ptr %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    assert!(done.body.insns().iter().any(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("and"))));
}

/// A store of an `i48` (a packed struct's member) stopped at
/// `Legalize(Unsupported Store i48 "promote operand")` (gcc.c-torture
/// 20131127-1 at -O2): the pin stores the promoted value in a dword and a word.
#[test]
fn an_m64_store_of_an_odd_width_integer_is_two_stores() {
    let text = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
define void @f(ptr %p, i64 %x) {
entry:
  %t = trunc i64 %x to i48
  store i48 %t, ptr %p, align 1
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).expect("selected");
    let stores = done
        .body
        .insns()
        .iter()
        .filter(|i| {
            i.what.as_ref().is_some_and(|w| {
                w.name.as_deref() == Some("mov") && matches!(w.dests.first(), Some(crate::model::ir::Loc::Mem(_)))
            })
        })
        .count();
    assert_eq!(stores, 2, "a dword and a word");
}
