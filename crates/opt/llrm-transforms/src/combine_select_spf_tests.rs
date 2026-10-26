//! `canonicalizeSPF` against `opt`'s output on the same text.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;

use crate::combine::{Combiner, Visit, combine_with_declared};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

fn combined(
    params: &str,
    body: &str,
) -> String {
    let text = format!("define i16 @f({params}) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).expect("placed");
    printed(&module)
}

/// `select (x < 0), 0, x` is `smax(x, 0)` (`opt` agrees). The select clamps
/// of d_faces stayed selects and made known bits ask `isGuaranteedNotToBeUndef`
/// of their arms 371 times against the pin's 7.
#[test]
fn a_select_of_a_compare_with_zero_is_smax() {
    let out = combined("i16 %x", "  %c = icmp slt i16 %x, 0\n  %r = select i1 %c, i16 0, i16 %x\n  ret i16 %r\n");
    assert!(out.contains("call i16 @llvm.smax.i16(i16 %x, i16 0)"), "{out}");
    assert!(!out.contains("select"), "{out}");
}

/// `x < y ? x : y` is `smin(x, y)`, unsigned `umin`; the compare may be shared.
#[test]
fn a_select_of_the_two_compared_values_is_the_min_or_max() {
    let out =
        combined("i16 %x, i16 %y", "  %c = icmp ult i16 %x, %y\n  %r = select i1 %c, i16 %x, i16 %y\n  ret i16 %r\n");
    assert!(out.contains("@llvm.umin.i16(i16 %x, i16 %y)"), "{out}");
}

/// `x < 0 ? -x : x` is `abs(x)` with `INT_MIN` poison only for `neg nsw`.
#[test]
fn a_select_of_a_value_and_its_negation_is_abs() {
    let out = combined(
        "i16 %x",
        "  %c = icmp slt i16 %x, 0\n  %n = sub nsw i16 0, %x\n  %r = select i1 %c, i16 %n, i16 %x\n  ret i16 %r\n",
    );
    assert!(out.contains("@llvm.abs.i16(i16 %x, i1 true)"), "{out}");
}
