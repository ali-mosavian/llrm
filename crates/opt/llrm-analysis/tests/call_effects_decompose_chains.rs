//! What call-effects holds of a call is a decomposition of the pointers the
//! call is passed, read down the chains of instructions below them.
use std::rc::Rc;

use llrm_analysis::manager::CallEffects;
use llrm_analysis::testing::{DOS, layout, value};
use llrm_mir::module::Operand;
use llrm_mir::passes::{Analyses, Outer};

/// A load forwarded to the pointer it loaded is replaced by it, and the effect
/// named the load as the root of its reference. A reference renamed to the GEP
/// would have it as its root, where one derived again goes on through the GEP's
/// base and index: it knows less, and the object depends on which the compile
/// held (qcport triggers -O1 -m32: 'the result kept past 1134 sound edits
/// contradicts what deriving it afresh gives'). The effects are derived again.
#[test]
fn test_call_effects_are_derived_again_for_a_load_replaced_by_a_pointer_that_decomposes() {
    use llrm_mir::passes::Analysis;
    let mut module = llrm_mir::parse::module(&format!(
        "{DOS}declare void @g(ptr initializes((0, 2)))

define void @f(ptr %p, ptr %q, i16 %i) {{
entry:
  %a = getelementptr i8, ptr %q, i16 %i
  %v = load ptr, ptr %p
  call void @g(ptr %v)
  ret void
}}
"
    ))
    .unwrap_or_else(|error| panic!("{error}"));
    let layout = layout(&module);
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let (context, function) = module.function_mut("f").unwrap();
    let before = analyses.get::<CallEffects>(context, &layout, function);
    let (a, v) = (value(function, "a"), value(function, "v"));
    let rename = |one| (one == v).then_some(Operand::Value(a));
    let named = <CallEffects as Analysis>::remapped(&before, &rename, &[], context, function);
    assert!(named.is_none(), "the effects were renamed to a pointer that decomposes");
}
