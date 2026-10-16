//! What call-effects says of a call names the values the call is passed. A
//! value found equal to another is replaced by it and erased (`replace_value`):
//! the result follows the substitution and is not derived again.
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_analysis::manager::CallEffects;
use llrm_analysis::testing::{DOS, layout, value};
use llrm_mir::module::Operand;
use llrm_mir::passes::{Analyses, Observer, Outer, PreservedAnalyses};

static DERIVED: AtomicUsize = AtomicUsize::new(0);

fn counting(
    what: &'static str,
    hit: bool,
) {
    // A derivation asks for the shape it works from; a result kept as sound is
    // not asked for it.
    if what == "shape" {
        DERIVED.fetch_add(1, Ordering::Relaxed);
    }
    let _ = hit;
}

#[test]
fn test_call_effects_stand_past_a_value_replaced_by_one_found_equal() {
    llrm_mir::passes::observe(Observer { span: |_, _, run| run(), function: |_, run| run(), count: counting });
    let mut module = llrm_mir::parse::module(&format!(
        "{DOS}declare void @g(ptr)

define void @f(ptr %p, ptr %q) {{
entry:
  %a = getelementptr i8, ptr %p, i16 4
  %b = getelementptr i8, ptr %p, i16 4
  call void @g(ptr %b)
  ret void
}}
"
    ))
    .unwrap_or_else(|error| panic!("{error}"));
    let layout = layout(&module);
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let (context, function) = module.function_mut("f").unwrap();
    DERIVED.store(0, Ordering::Relaxed);
    analyses.get::<CallEffects>(context, &layout, function);
    let derived = DERIVED.load(Ordering::Relaxed);
    assert!(derived > 0, "premise: deriving the effects asks for the shape");
    // %b is %a: found again and replaced, then erased.
    let (a, b) = (value(function, "a"), value(function, "b"));
    let b_inst =
        function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(b)).unwrap();
    function.replace_value(b, Operand::Value(a));
    function.erase(b_inst).expect("unused now");
    analyses.invalidate(function, &PreservedAnalyses::none());
    analyses.get::<CallEffects>(context, &layout, function);
    assert_eq!(DERIVED.load(Ordering::Relaxed), derived, "a value replaced by one equal derived the effects again");
    // The call is passed another pointer: what is said of it may differ.
    let call = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(function.instruction(inst).opcode, llrm_mir::Opcode::Call(_)))
        .unwrap();
    function.set_operand(call, 0, Operand::Value(value(function, "q")));
    analyses.invalidate(function, &PreservedAnalyses::none());
    analyses.get::<CallEffects>(context, &layout, function);
    assert!(DERIVED.load(Ordering::Relaxed) > derived, "a call passed another pointer kept what was said of it");
}
