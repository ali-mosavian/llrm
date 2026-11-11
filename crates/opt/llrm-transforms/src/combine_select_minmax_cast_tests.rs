//! The min/max block of `visitSelectInst` against `opt`'s output.

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

/// `p >= 65535.0f ? 65535L : (long)p` stayed an integer select behind an
/// `fptosi`: arm64 stft_single ran one instruction more per bin (+65871 at
/// `-Os`). The pin selects on the floats and converts once.
#[test]
fn a_float_clamp_with_a_converted_arm_converts_after_the_select() {
    let out = combined(
        "define i64 @f(float %p) {\nb0:\n  %c = fcmp oge float %p, 6.553500e+04\n  %t = fptosi float %p to i64\n  \
         %q = select i1 %c, i64 65535, i64 %t\n  ret i64 %q\n}\n",
    );
    let select = out.lines().find(|line| line.contains("select")).expect("a select");
    assert!(select.contains("float %p") && select.contains("float 6.553500e+04"), "a select of the floats: {out}");
    assert!(!select.contains("i64"), "no integer select left: {out}");
    let at = out.find("select").expect("a select");
    assert!(out[at..].contains("fptosi float"), "the conversion follows the select: {out}");
}

/// A select that needs no cast and compares its own arms is left alone.
#[test]
fn a_min_of_the_compared_floats_is_not_rebuilt() {
    let out = combined(
        "define float @f(float %x, float %y) {\nb0:\n  %c = fcmp olt float %x, %y\n  \
         %r = select i1 %c, float %x, float %y\n  ret float %r\n}\n",
    );
    assert!(out.contains("select i1 %c, float %x, float %y"), "{out}");
}
