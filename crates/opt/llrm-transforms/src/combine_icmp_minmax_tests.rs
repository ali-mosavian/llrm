//! `foldICmpWithMinMax` against `opt -passes=instcombine` on the same text
//! (pin d1106deb71cc): `Z` is a constant, `X` has a known relation to it, `Y`
//! is unknown.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// The driver's visit of the compares.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::ICmp(_) => crate::combine::visit(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `icmp pred min|max(X, Y), Z` with `X` against `Z` known was kept as the
/// intrinsic and a compare: the pin leaves one compare on `Y` (equality, the
/// fold of `X != Z`, the relational tables) or a constant. One row per table
/// entry; `opt` printed each expected line.
#[test]
fn a_compare_of_a_min_or_max_with_a_known_operand_is_the_pins_one_compare() {
    let rows: &[(&str, &str, &str, &str, &str, &str)] = &[
        ("umin", "eq", "%x = or i8 %a, 64", "%m", "8", "icmp eq i8 %y, 8"),
        ("umax", "eq", "%x = and i8 %a, 7", "%m", "8", "icmp eq i8 %y, 8"),
        ("umax", "eq", "%x = and i8 %a, 0", "%m", "0", "icmp eq i8 %y, 0"),
        ("smin", "eq", "%x = and i8 %a, 0", "%m", "0", "icmp sgt i8 %y, -1"),
        ("smax", "eq", "%x = and i8 %a, 7", "%m", "8", "icmp eq i8 %y, 8"),
        ("smax", "eq", "%x = and i8 %a, 0", "%m", "0", "icmp slt i8 %y, 1"),
        ("umin", "ult", "%x = or i8 %a, 64", "%m", "8", "icmp ult i8 %y, 8"),
        ("umax", "ult", "%x = and i8 %a, 7", "%m", "8", "icmp ult i8 %y, 8"),
        ("umax", "ugt", "%x = or i8 %a, 64", "8", "%m", "ret i1 false"),
        ("umin", "uge", "%x = and i8 %a, 7", "%m", "8", "ret i1 false"),
        ("smax", "sgt", "%x = and i8 %a, 7", "8", "%m", "icmp slt i8 %y, 8"),
        ("umin", "ne", "%x = or i8 %a, 64", "%m", "8", "icmp ne i8 %y, 8"),
    ];
    for &(name, pred, xdef, lhs, rhs, want) in rows {
        let text = format!(
            "define i1 @f(i8 %a, i8 %y) {{\nb0:\n  {xdef}\n  %m = call i8 @llvm.{name}.i8(i8 %x, i8 %y)\n  %c = icmp {pred} i8 {lhs}, {rhs}\n  ret i1 %c\n}}\ndeclare i8 @llvm.{name}.i8(i8, i8)\n"
        );
        let out = combined(&text);
        assert!(!out.contains("@llvm.") || out.contains("declare"), "{name} {pred}: {out}");
        assert!(out.contains(want), "{name} {pred} {lhs} {rhs}: want `{want}`: {out}");
    }
}
