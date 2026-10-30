//! `combine_castcall`: a call through a mismatched prototype takes the
//! callee's.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine_with_sinking;
use crate::testing::{parsed, printed};

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.to_vec()), metadata.len());
    combine_with_sinking(
        context,
        &globals,
        &callees,
        &metadata,
        &DataLayout::default(),
        function,
        |c, inst| {
            if matches!(
                c.function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Call(_)
            ) {
                c.visit_call(inst)
            } else {
                crate::combine::Visit::None
            }
        },
        false,
        &mut declared,
    );
    printed(&module)
}

const NONE_DEFINED: &str = "%S = type { i32, i32, i32 }

define i16 @none() {
b0:
  ret i16 42
}

define i16 @f(ptr %t) {
b0:
  %tmp = alloca %S, align 2
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %tmp, ptr align 2 %t, i16 12, i1 false)
  %r = call i16 (ptr) @none(ptr byval(%S) align 2 %tmp)
  ret i16 %r
}

declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
";

/// `int none() {..}` called as `none(t)` (gcc.c-torture/20051012-1,
/// old_style_extra_args): the call kept its `byval` struct, so the caller built
/// a 12-byte copy and pushed it for a callee that takes nothing (m16 -Os, 113
/// bytes over llrm-c).
#[test]
fn a_struct_passed_to_a_function_defined_without_parameters_is_dropped() {
    let after = combined(NONE_DEFINED);
    assert!(after.contains("call i16 @none()"), "{after}");
    assert!(!after.contains("byval"), "{after}");
}

/// A declaration may be a placeholder (`void @foo()`): its arguments stay.
#[test]
fn a_declared_callee_keeps_its_arguments() {
    let text = NONE_DEFINED.replace("define i16 @none() {\nb0:\n  ret i16 42\n}", "declare i16 @none()");
    let after = combined(&text);
    assert!(after.contains("byval"), "{after}");
}

/// A `byval` parameter the callee does not have on both sides is no cast to
/// drop.
#[test]
fn a_byval_disagreement_on_a_common_parameter_stays() {
    let text = "%S = type { i32 }

define i16 @none(ptr %p) {
b0:
  ret i16 1
}

define i16 @f(ptr %t) {
b0:
  %r = call i16 (ptr) @none(ptr byval(%S) align 2 %t)
  ret i16 %r
}
";
    assert!(combined(text).contains("byval"));
}

/// A parameter the call lacks is passed as null.
#[test]
fn a_missing_argument_is_null() {
    let text = "define i16 @two(i16 %a, i16 %b) {
b0:
  ret i16 %b
}

define i16 @f() {
b0:
  %r = call i16 (i16) @two(i16 7)
  ret i16 %r
}
";
    let after = combined(text);
    assert!(after.contains("@two(i16 7, i16 0)"), "{after}");
}
