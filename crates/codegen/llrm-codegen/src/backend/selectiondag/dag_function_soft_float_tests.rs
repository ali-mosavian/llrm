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

fn calls_of(body: &str, params: &str, ret: &str) -> Vec<String> {
    let declare = if body.contains("llvm.fabs") { "declare float @llvm.fabs.f32(float)\n" } else { "" };
    let text = format!(
        "target datalayout = \"@m16-datalayout@\"
{declare}define {ret} @f({params}) \"use-soft-float\"=\"true\" {{
entry:
{body}
}}
"
    );
    let module = crate::testing::module(&text).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{body}: {}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()).map(|w| w.name.as_deref().unwrap_or("").to_owned()).collect();
    assert!(names.iter().all(|name| !name.starts_with('f') || name.is_empty()), "{body}: an x87 op: {names:?}");
    done.calls.values().cloned().collect()
}

/// Integer-to-float and float-to-integer conversions are compiler-rt's
/// routines; negation, absolute value and select are integer operations on the
/// bits and call nothing.
#[test]
fn a_soft_float_conversion_calls_its_routine_and_the_rest_call_nothing() {
    assert_eq!(calls_of("%r = sitofp i32 %a to float\nret float %r", "i32 %a", "float"), ["__floatsisf"]);
    assert_eq!(calls_of("%r = uitofp i32 %a to float\nret float %r", "i32 %a", "float"), ["__floatunsisf"]);
    assert_eq!(calls_of("%r = sitofp i16 %a to float\nret float %r", "i16 %a", "float"), ["__floatsisf"]);
    assert_eq!(calls_of("%r = fptosi float %a to i32\nret i32 %r", "float %a", "i32"), ["__fixsfsi"]);
    assert_eq!(calls_of("%r = fptoui float %a to i32\nret i32 %r", "float %a", "i32"), ["__fixunssfsi"]);
    assert!(calls_of("%r = fneg float %a\nret float %r", "float %a", "float").is_empty());
    assert!(calls_of("%r = call float @llvm.fabs.f32(float %a)\nret float %r", "float %a", "float").is_empty());
    assert!(calls_of("%r = select i1 %c, float %a, float %b\nret float %r", "i1 %c, float %a, float %b", "float").is_empty());
}

/// A `double` is an `i64` of the same bits: it takes two registers in a call,
/// and arithmetic on it calls the `df3` routines.
#[test]
fn a_soft_double_is_two_registers_and_calls_the_df3_routines() {
    assert_eq!(
        calls_of("%r = fadd double %a, %b\nret double %r", "double %a, double %b", "double"),
        ["__adddf3"]
    );
    assert_eq!(
        calls_of("%c = fcmp olt double %a, %b\n%r = zext i1 %c to i32\nret i32 %r", "double %a, double %b", "i32"),
        ["__ltdf2"]
    );
    assert_eq!(calls_of("%r = fpext float %a to double\nret double %r", "float %a", "double"), ["__extendsfdf2"]);
    assert_eq!(calls_of("%r = fptrunc double %a to float\nret float %r", "double %a", "float"), ["__truncdfsf2"]);
}

/// An `i64` converts through the `di` routines, its value in two registers.
#[test]
fn a_soft_float_i64_conversion_calls_the_di_routines() {
    assert_eq!(calls_of("%r = sitofp i64 %a to float\nret float %r", "i64 %a", "float"), ["__floatdisf"]);
    assert_eq!(calls_of("%r = uitofp i64 %a to double\nret double %r", "i64 %a", "double"), ["__floatundidf"]);
    assert_eq!(calls_of("%r = fptosi float %a to i64\nret i64 %r", "float %a", "i64"), ["__fixsfdi"]);
    assert_eq!(calls_of("%r = fptoui double %a to i64\nret i64 %r", "double %a", "i64"), ["__fixunsdfdi"]);
}

/// clang's `-mno-80387` does not set `use-soft-float`; it takes the x87 out of
/// the target features. A function so marked was selected with `fld`/`fadd`:
/// the soft-float build of a C program was the x87 build.
#[test]
fn a_function_without_the_x87_feature_is_soft_float() {
    let text = "target datalayout = \"@m16-datalayout@\"
define void @f(ptr %p, ptr %q) \"target-features\"=\"-x87\" {
entry:
  %a = load float, ptr %p
  %b = load float, ptr %q
  %c = fadd float %a, %b
  store float %c, ptr %p
  ret void
}
";
    let module = crate::testing::module(text).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.calls.values().map(String::as_str).collect::<Vec<_>>(), ["__addsf3"]);
}

/// The runtime is C: its `__addsf3` is defined under the convention's spelling
/// (`___addsf3@3` in OMF). The call named the bare `__addsf3`, so a program
/// with the library linked in failed with "__addsf3 is an undefined reference".
#[test]
fn a_soft_float_call_is_spelled_as_the_runtime_defines_the_routine() {
    let module = crate::testing::module(FLOAT_ADD).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let spelled = [("__addsf3".to_owned(), 0, "___addsf3@3".to_owned())];
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &spelled).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(done.calls.values().map(String::as_str).collect::<Vec<_>>(), ["___addsf3@3"]);
}

/// The compare routines return a C `int`: 16 bits at m16, read from `ax`. They
/// were read as 32 bits from `eax`, whose upper half the callee never set, so
/// `f != f` was true for every float and a program that canonicalised NaNs
/// summed 400 NaNs (1984619853 expected, -1677721600 printed).
#[test]
fn a_soft_float_compare_reads_a_16_bit_int_result_at_m16() {
    let text = "target datalayout = \"@m16-datalayout@\"
define i32 @f(float %a, float %b) \"target-features\"=\"-x87\" {
entry:
  %c = fcmp olt float %a, %b
  %r = zext i1 %c to i32
  ret i32 %r
}
";
    let module = crate::testing::module(text).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("call"))).expect("a call");
    assert_eq!(call.delivers.iter().map(|(held, _)| format!("{held:?}")).collect::<Vec<_>>().len(), 1);
    assert!(format!("{:?}", call.delivers).contains("width: 2"), "{:?}", call.delivers);
}

/// The call places its arguments by the convention the routine was defined
/// under (the libcall's number), not the target's native one: the C runtime's
/// `__addsf3` is `_*@3`, regparm3, and the call spelled the Watcom library's
/// symbol and used the wrong registers (`__addsf3_ is an undefined reference`).
#[test]
fn a_soft_float_call_places_arguments_by_the_routines_convention() {
    let module = crate::testing::module(FLOAT_ADD).expect("parses");
    let cpu = crate::backend::cpu::profile("386").expect("a cpu");
    let required = |libcalls: &[(String, u32, String)]| -> usize {
        let done = selected(&module, "f", &llrm_x86_m16::M16, cpu, libcalls).unwrap_or_else(|e| panic!("{}", e.0));
        let insns = done.body.insns();
        insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("call"))).expect("a call").requires.len()
    };
    let native = required(&[]);
    let stack = required(&[("__addsf3".to_owned(), 0, "_x".to_owned())]);
    assert!(native > 0, "the native convention passes the floats in registers");
    assert_eq!(stack, 0, "convention 0 (cdecl16) passes them on the stack");
}
