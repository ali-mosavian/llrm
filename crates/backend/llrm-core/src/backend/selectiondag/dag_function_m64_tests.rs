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
    let insns = done.body.insns();
    let names: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref().and_then(|w| w.name.as_deref())).collect();
    assert_eq!(names, ["", "cmp", "cmovb", ""]);
}
