//! No FPU: a float operation at m16 is a call to the compiler-rt routine and
//! never an x87 instruction.

use super::dag_function::selected;

const FLOAT_ADD: &str = "target datalayout = \"@m16-datalayout@\"
define void @f(ptr %p, ptr %q) \"use-soft-float\"=\"true\" {
entry:
  %a = load float, ptr %p
  %b = load float, ptr %q
  %c = fadd float %a, %b
  store float %c, ptr %p
  ret void
}
";

/// With `use-soft-float` a float add selected as `fld`/`fadd`/`fstp`: the
/// function used the x87 the target was told it did not have.
#[test]
fn a_soft_float_add_is_a_call_to_addsf3_and_no_x87_op() {
    let module = crate::testing::module(FLOAT_ADD).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    assert!(names.iter().all(|name| !name.starts_with('f') || name.is_empty()), "an x87 op: {names:?}");
    let callees: Vec<&str> = done.calls.values().map(String::as_str).collect();
    assert_eq!(callees, ["__addsf3"], "{names:?}");
}

const FLOAT_CONSTANT: &str = "target datalayout = \"@m16-datalayout@\"
define void @f(ptr %p) \"use-soft-float\"=\"true\" {
entry:
  %a = load float, ptr %p
  %c = fmul float %a, 1.5
  %i = bitcast float %c to i32
  store i32 %i, ptr %p
  ret void
}
";

/// A float constant is its bits and a bitcast to an integer is the softened
/// value itself: neither needs the x87 or a call of its own.
#[test]
fn a_soft_float_constant_and_bitcast_are_integers() {
    let module = crate::testing::module(FLOAT_CONSTANT).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    assert!(names.iter().all(|name| !name.starts_with('f') || name.is_empty()), "an x87 op: {names:?}");
    let callees: Vec<&str> = done.calls.values().map(String::as_str).collect();
    assert_eq!(callees, ["__mulsf3"], "{names:?}");
}

const FLOAT_ARGS_AND_RESULT: &str = "target datalayout = \"@m16-datalayout@\"
define float @f(float %a, float %b) \"use-soft-float\"=\"true\" {
entry:
  %c = fadd float %a, %b
  ret float %c
}
";

/// A float argument and result travel as the integer of their width: the
/// function reads its arguments from the stack, calls `__addsf3` and returns
/// in `dx:ax`, with no x87 op.
#[test]
fn a_soft_float_function_takes_and_returns_integers() {
    let module = crate::testing::module(FLOAT_ARGS_AND_RESULT).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    assert!(names.iter().all(|name| !name.starts_with('f') || name.is_empty()), "an x87 op: {names:?}");
    let callees: Vec<&str> = done.calls.values().map(String::as_str).collect();
    assert_eq!(callees, ["__addsf3"], "{names:?}");
}

const FLOAT_CALL: &str = "target datalayout = \"@m16-datalayout@\"
declare float @g(float)
define float @f(float %a, float %b) \"use-soft-float\"=\"true\" {
entry:
  %s = fadd float %a, %b
  %r = call float @g(float %s)
  ret float %r
}
";

/// A call passes a float and takes one back as integers, in the registers the
/// native convention gives an integer of that width.
#[test]
fn a_soft_float_call_passes_and_takes_integers() {
    let module = crate::testing::module(FLOAT_CALL).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    assert!(names.iter().all(|name| !name.starts_with('f') || name.is_empty()), "an x87 op: {names:?}");
    assert_eq!(done.calls.values().map(String::as_str).collect::<Vec<_>>(), ["__addsf3", "g"], "{names:?}");
}

fn compare_calls(code: &str) -> Vec<String> {
    let text = format!(
        "target datalayout = \"@m16-datalayout@\"
define i32 @f(float %a, float %b) \"use-soft-float\"=\"true\" {{
entry:
  %c = fcmp {code} float %a, %b
  %r = zext i1 %c to i32
  ret i32 %r
}}
"
    );
    let module = crate::testing::module(&text).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{code}: {}", e.0));
    let mut calls: Vec<String> = done.calls.values().cloned().collect();
    calls.sort();
    calls
}

/// An ordered compare is compiler-rt's routine for it; an unordered-or-less
/// one is the routine of the opposite ordered compare (unordered answers the
/// way that makes it true); `one` and `ueq` take two.
#[test]
fn a_soft_float_compare_calls_the_routine_that_answers_it() {
    assert_eq!(compare_calls("olt"), ["__ltsf2"]);
    assert_eq!(compare_calls("oeq"), ["__eqsf2"]);
    assert_eq!(compare_calls("une"), ["__nesf2"]);
    assert_eq!(compare_calls("uno"), ["__unordsf2"]);
    assert_eq!(compare_calls("ult"), ["__gesf2"]);
    assert_eq!(compare_calls("ugt"), ["__lesf2"]);
    assert_eq!(compare_calls("one"), ["__gtsf2", "__ltsf2"]);
    assert_eq!(compare_calls("ueq"), ["__eqsf2", "__unordsf2"]);
}
