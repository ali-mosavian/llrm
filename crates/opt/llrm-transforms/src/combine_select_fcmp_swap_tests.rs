//! The unordered-compare select canonicalisation against `opt`'s output.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;

use crate::combine::{Combiner, Visit, combine_with_declared};
use crate::testing::{parsed, printed};

fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).expect("placed");
    printed(&module)
}

/// `(X ugt Y) ? X : Y` stayed unordered: x86 lowers only the ordered form to
/// `minss`/`maxss` (the pin's `.inv`), the clamp loop of stft_single kept a
/// branch and grew 56 bytes.
#[test]
fn an_unordered_compare_of_the_selected_values_is_ordered_with_swapped_arms() {
    let out = combined(
        "define float @f(float %x, float %y) {\nb0:\n  %c = fcmp ugt float %x, %y\n  \
         %r = select i1 %c, float %x, float %y\n  ret float %r\n}\n",
    );
    assert!(out.contains("fcmp ole float %x, %y"), "{out}");
    let select = out.lines().find(|line| line.contains("select")).expect("a select");
    assert!(select.ends_with(", float %y, float %x"), "{out}");
}

/// An ordered compare, or one that is not of the arms, is left alone.
#[test]
fn an_ordered_compare_is_not_swapped() {
    let out = combined(
        "define float @f(float %x, float %y) {\nb0:\n  %c = fcmp ogt float %x, %y\n  \
         %r = select i1 %c, float %x, float %y\n  ret float %r\n}\n",
    );
    assert!(out.contains("fcmp ogt float %x, %y") && out.contains("float %x, float %y"), "{out}");
}
