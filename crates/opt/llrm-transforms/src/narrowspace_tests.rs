//! A far pointer parameter every call fills from DGROUP is a near one; each
//! case's text is the pass's input.

use llrm_mir::datalayout::DataLayout;

use crate::narrowspace::narrowed;
use crate::testing::{parsed, printed};

fn run(text: &str) -> String {
    on(text, llrm_x86_m16::spaces())
}

fn on(
    text: &str,
    spaces: llrm_mir::spaces::Spaces,
) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("parses");
    narrowed(&mut module, &layout, spaces);
    printed(&module)
}

const CALLEE: &str = "@g = global [16 x i16] zeroinitializer
define internal i16 @sum(ptr addrspace(1) %a, i16 %n) {
b0:
  %done = icmp sle i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %r = call i16 @sum(ptr addrspace(1) %a, i16 %m)
  %v = load i16, ptr addrspace(1) %a
  %s = add i16 %r, %v
  ret i16 %s
out:
  ret i16 0
}
";

/// Nib's `total(table, i)` pushed the segment and offset of a global array at
/// every call and read them back with `les`: DGROUP's selector is no news to
/// anyone.
#[test]
fn a_far_parameter_every_call_fills_from_a_global_is_near() {
    let after = run(&format!(
        "{CALLEE}define i16 @top() {{
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %r = call i16 @sum(ptr addrspace(1) %w, i16 3)
  ret i16 %r
}}
"
    ));
    assert!(after.contains("@sum(ptr %") && after.contains("i16 %n)"), "{after}");
    // The recursive call passes the near pointer it was given.
    assert!(!after.contains("call i16 @sum(ptr addrspace(1)"), "{after}");
}

/// A stack object's selector is SS, which no DGROUP pointer says: the parameter
/// is the stack's.
#[test]
fn a_far_parameter_every_call_fills_from_the_stack_is_a_stack_pointer() {
    let after = run(&format!(
        "{CALLEE}define i16 @top() {{
b0:
  %s = alloca [16 x i16]
  %w = addrspacecast ptr %s to ptr addrspace(1)
  %r = call i16 @sum(ptr addrspace(1) %w, i16 3)
  ret i16 %r
}}
"
    ));
    assert!(after.contains("@sum(ptr addrspace(5) %") && !after.contains("call i16 @sum(ptr addrspace(1)"), "{after}");
}

/// One call from the stack and one from a global: no one space holds both.
#[test]
fn a_parameter_filled_from_the_stack_and_from_a_global_stays_far() {
    let after = run(&format!(
        "{CALLEE}define i16 @top() {{
b0:
  %s = alloca [16 x i16]
  %w = addrspacecast ptr %s to ptr addrspace(1)
  %x = addrspacecast ptr @g to ptr addrspace(1)
  %r = call i16 @sum(ptr addrspace(1) %w, i16 3)
  %q = call i16 @sum(ptr addrspace(1) %x, i16 3)
  %t = add i16 %r, %q
  ret i16 %t
}}
"
    ));
    assert!(after.contains("@sum(ptr addrspace(1) %a"), "{after}");
}

/// One call from elsewhere (a far pointer that came in) keeps it far for all.
#[test]
fn one_far_actual_keeps_the_parameter_far() {
    let after = run(&format!(
        "{CALLEE}define i16 @top(ptr addrspace(1) %p) {{
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %r = call i16 @sum(ptr addrspace(1) %w, i16 3)
  %q = call i16 @sum(ptr addrspace(1) %p, i16 3)
  %s = add i16 %r, %q
  ret i16 %s
}}
"
    ));
    assert!(after.contains("@sum(ptr addrspace(1) %a"), "{after}");
}

/// Code outside the module may pass any pointer.
#[test]
fn an_exported_function_keeps_a_far_parameter() {
    let after = run(&format!(
        "{}define i16 @top() {{
b0:
  %w = addrspacecast ptr @g to ptr addrspace(1)
  %r = call i16 @sum(ptr addrspace(1) %w, i16 3)
  ret i16 %r
}}
",
        CALLEE.replace("internal ", "")
    ));
    assert!(after.contains("@sum(ptr addrspace(1) %a"), "{after}");
}

/// The stack's space was 5 in the pass: a target that numbers it 6 got a stack
/// object's parameter narrowed to the data space's near pointer, read through
/// DS.
#[test]
fn a_target_names_the_space_of_the_stack_a_parameter_narrows_to() {
    let spaces = llrm_mir::spaces::Spaces { stack: 6, ..llrm_x86_m16::spaces() };
    let after = on(
        &format!(
            "{CALLEE}define i16 @top() {{
b0:
  %s = alloca [16 x i16]
  %w = addrspacecast ptr %s to ptr addrspace(1)
  %r = call i16 @sum(ptr addrspace(1) %w, i16 3)
  ret i16 %r
}}
"
        ),
        spaces,
    );
    assert!(after.contains("@sum(ptr addrspace(6) %"), "{after}");
}
