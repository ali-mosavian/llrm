use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

/// `text` after `visitSelectInst` alone.
fn run(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `@f(params)` returning i16 after `visitSelectInst` alone; the condition is
/// an `icmp` of two `i8`s.
fn combined(body: &str) -> String {
    let text = format!(
        "define i16 @f(i8 %x, i8 %y, i8 %w) {{\nb0:\n  %c = icmp ult i8 %y, %w\n  %e = {body}  ret i16 %r\n}}\n"
    );
    run(&text)
}

/// Opt: `select c, (zext x), 5` is `zext (select c, x, 5)` in `i8`. Without the
/// fold the select stayed `i16` over the extension.
#[test]
fn a_constant_that_fits_narrows_a_select_over_a_zext() {
    let out = combined("zext i8 %x to i16\n  %r = select i1 %c, i16 %e, i16 5\n");
    assert!(out.contains("select i1 %c, i8 %x, i8 5") && out.contains("zext i8 %"), "{out}");
}

/// The extension on the false arm keeps its side.
#[test]
fn the_sext_on_the_false_arm_keeps_its_side() {
    let out = combined("sext i8 %x to i16\n  %r = select i1 %c, i16 -3, i16 %e\n");
    assert!(out.contains("select i1 %c, i8 -3, i8 %x") && out.contains("sext i8 %"), "{out}");
}

/// 300 is not a `zext` of an `i8`, and 200 is not a `sext` of one.
#[test]
fn a_constant_the_narrow_type_cannot_hold_is_left() {
    let out = combined("zext i8 %x to i16\n  %r = select i1 %c, i16 %e, i16 300\n");
    assert!(out.contains("select i1 %c, i16 %e, i16 300"), "{out}");
    let out = combined("sext i8 %x to i16\n  %r = select i1 %c, i16 200, i16 %e\n");
    assert!(out.contains("select i1 %c, i16 200, i16 %e"), "{out}");
}

/// An extension read elsewhere stays, so the select is not narrowed.
#[test]
fn an_extension_with_another_user_is_left() {
    let text = "define i16 @f(i8 %x, i8 %y, i8 %w) {\nb0:\n  %c = icmp ult i8 %y, %w\n  %e = zext i8 %x to i16\n  %r = select i1 %c, i16 %e, i16 5\n  %s = add i16 %r, %e\n  ret i16 %s\n}\n";
    let out = run(text);
    assert!(out.contains("select i1 %c, i16 %e, i16 5"), "{out}");
}

/// A condition that is not an `icmp` or `trunc` of the narrow type does not
/// allow it.
#[test]
fn a_condition_over_another_type_is_left() {
    let text = "define i16 @f(i8 %x, i16 %y, i16 %w) {\nb0:\n  %c = icmp ult i16 %y, %w\n  %e = zext i8 %x to i16\n  %r = select i1 %c, i16 %e, i16 5\n  ret i16 %r\n}\n";
    let out = run(text);
    assert!(out.contains("select i1 %c, i16 %e, i16 5"), "{out}");
}

/// A boolean source needs no matching condition.
#[test]
fn a_boolean_source_narrows_under_any_condition() {
    let text = "define i16 @f(i1 %c, i1 %x) {\nb0:\n  %e = zext i1 %x to i16\n  %r = select i1 %c, i16 1, i16 %e\n  ret i16 %r\n}\n";
    let out = run(text);
    assert!(out.contains("i1 %x") && out.contains("zext i1 %") && !out.contains("select i1 %c, i16"), "{out}");
}
