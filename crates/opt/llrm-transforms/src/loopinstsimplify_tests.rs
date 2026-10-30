use llrm_mir::datalayout::DataLayout;

use super::loop_instsimplify;
use crate::testing::{parsed, printed};

/// @f of `text` after LoopInstSimplify; whether anything changed.
fn simplified(text: &str) -> (String, bool) {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = loop_instsimplify(context, &globals, &metadata, &DataLayout::default(), &callees, function);
    (printed(&module), changed)
}

/// `add %x, 0` in a loop body is `%x`: the store reads the argument and the
/// add goes. The pass was not in the pipeline (a registry-missing row), so
/// the add stayed until a later InstCombine.
#[test]
fn an_identity_in_a_loop_body_is_replaced_and_deleted() {
    let (text, changed) = simplified(
        "declare void @use(i16)
define void @f(i16 %x, i16 %n) {
b0:
  br label %b1
b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %a = add i16 %x, 0
  call void @use(i16 %a)
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2
b2:
  ret void
}
",
    );
    assert!(changed);
    assert!(text.contains("call void @use(i16 %x)"), "{text}");
    assert!(!text.contains("%a = add"), "{text}");
}

/// A phi visited before the value it reads is replaced needs a second sweep:
/// `%b = %a + 0` makes `phi [%x, pre], [%b, latch]` a phi of `%x` and itself,
/// which is `%x`. One sweep leaves the phi.
#[test]
fn a_phi_that_reads_a_replaced_value_is_simplified_on_the_next_sweep() {
    let (text, changed) = simplified(
        "declare void @use(i16)
define void @f(i16 %x, i16 %n) {
b0:
  br label %b1
b1:
  %p = phi i16 [ %x, %b0 ], [ %b, %b1 ]
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  call void @use(i16 %p)
  %b = add i16 %p, 0
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2
b2:
  ret void
}
",
    );
    assert!(changed);
    assert!(text.contains("call void @use(i16 %x)"), "{text}");
    assert!(!text.contains("%p = phi"), "{text}");
}

/// An instruction outside every loop is left alone: this is LoopInstSimplify,
/// not InstSimplify.
#[test]
fn code_outside_a_loop_is_left_alone() {
    let (text, changed) = simplified(
        "declare void @use(i16)
define void @f(i16 %x) {
b0:
  %a = add i16 %x, 0
  call void @use(i16 %a)
  ret void
}
",
    );
    assert!(!changed);
    assert!(text.contains("%a = add i16 %x, 0"), "{text}");
}
