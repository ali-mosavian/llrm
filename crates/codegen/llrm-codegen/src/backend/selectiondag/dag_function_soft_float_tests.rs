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
