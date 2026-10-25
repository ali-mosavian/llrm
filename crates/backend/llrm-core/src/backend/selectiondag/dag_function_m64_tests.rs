//! x86-64 (m64): quadword arithmetic, the SysV call sequence, loops over
//! arrays.

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

/// `va_start` at m64 was selected as the 32-bit one (the address of the first
/// stack argument): `varargs.c` printed wrong sums. Until the register save
/// area is translated it is refused.
#[test]
fn va_start_at_m64_is_refused() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"
declare void @llvm.va_start(ptr)
define i32 @f(i32 %n, ...) {
  %ap = alloca [24 x i8]
  call void @llvm.va_start(ptr %ap)
  ret i32 %n
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
    let refused = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).err().expect("refused");
    assert!(refused.0.contains("va_start at m64"), "{}", refused.0);
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
