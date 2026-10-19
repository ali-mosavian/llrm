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

/// `@f(params)` returning `ret` after `visitSelectInst` alone.
fn combined(
    ret: &str,
    params: &str,
    body: &str,
) -> String {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

const PARAMS: &str = "i1 %c, i16 %x, i16 %a, i16 %b";

/// `select c, (zext a), (zext b)` is `zext (select c, a, b)`.
#[test]
fn two_casts_of_one_type_share_one_cast_after_the_select() {
    let out = combined(
        "i32",
        "i1 %c, i16 %a, i16 %b",
        "  %x = zext i16 %a to i32\n  %y = zext i16 %b to i32\n  %r = select i1 %c, i32 %x, i32 %y\n  ret i32 %r\n",
    );
    assert!(out.contains("select i1 %c, i16 %a, i16 %b") && out.contains("zext i16 %"), "{out}");
    assert_eq!(out.matches("zext").count(), 1, "{out}");
}

/// A common operand comes out of the select, on either side and, for a
/// commutative operation, crosswise.
#[test]
fn a_common_operand_of_two_adds_comes_out_of_the_select() {
    for second in ["add i16 %x, %b", "add i16 %b, %x"] {
        let out = combined(
            "i16",
            PARAMS,
            &format!("  %p = add i16 %x, %a\n  %q = {second}\n  %r = select i1 %c, i16 %p, i16 %q\n  ret i16 %r\n"),
        );
        assert!(out.contains("select i1 %c, i16 %a, i16 %b") && out.contains("add i16 %x, %"), "{out}");
        assert_eq!(out.matches("add").count(), 1, "{out}");
    }
}

/// The flags both arms had are kept, the others dropped.
#[test]
fn only_the_flags_both_arms_had_are_kept() {
    let out = combined(
        "i16",
        PARAMS,
        "  %p = add nsw nuw i16 %x, %a\n  %q = add nsw i16 %x, %b\n  %r = select i1 %c, i16 %p, i16 %q\n  ret i16 %r\n",
    );
    assert!(out.contains("add nsw i16 %x, %"), "{out}");
}

/// The common operand of a non-commutative operation must be in the same place.
#[test]
fn a_subtraction_with_the_common_operand_on_either_side_is_one_subtraction() {
    let out = combined(
        "i16",
        PARAMS,
        "  %p = sub i16 %a, %x\n  %q = sub i16 %b, %x\n  %r = select i1 %c, i16 %p, i16 %q\n  ret i16 %r\n",
    );
    assert!(out.contains("sub i16 %") && out.contains(", %x") && out.contains("select i1 %c, i16 %a, i16 %b"), "{out}");
}

/// A compare with a common operand takes it after the select.
#[test]
fn two_compares_with_a_common_operand_are_one_compare() {
    let out = combined(
        "i1",
        PARAMS,
        "  %p = icmp eq i16 %x, %a\n  %q = icmp eq i16 %x, %b\n  %r = select i1 %c, i1 %p, i1 %q\n  ret i1 %r\n",
    );
    assert!(out.contains("select i1 %c, i16 %a, i16 %b") && out.contains("icmp eq i16 %x, %"), "{out}");
    assert_eq!(out.matches("icmp").count(), 1, "{out}");
}

/// A common dividend: a poison condition chosen as zero could divide by zero,
/// so the condition is frozen; a common divisor needs no freeze.
#[test]
fn a_division_with_a_common_dividend_freezes_the_condition() {
    let out = combined(
        "i16",
        PARAMS,
        "  %p = udiv i16 %x, %a\n  %q = udiv i16 %x, %b\n  %r = select i1 %c, i16 %p, i16 %q\n  ret i16 %r\n",
    );
    assert!(out.contains("freeze i1 %c") && out.contains("udiv i16 %x, %"), "{out}");
    let out = combined(
        "i16",
        PARAMS,
        "  %p = udiv i16 %a, %x\n  %q = udiv i16 %b, %x\n  %r = select i1 %c, i16 %p, i16 %q\n  ret i16 %r\n",
    );
    assert!(!out.contains("freeze") && out.contains(", %x"), "{out}");
}

/// An arm with another user is not rewritten: the operation would be made
/// twice.
#[test]
fn an_arm_with_another_user_is_left() {
    let out = combined(
        "i16",
        "i1 %c, i16 %x, i16 %a, i16 %b, ptr %m",
        "  %p = add i16 %x, %a\n  %q = add i16 %x, %b\n  store i16 %p, ptr %m\n  %r = select i1 %c, i16 %p, i16 %q\n  ret i16 %r\n",
    );
    assert!(out.contains("select i1 %c, i16 %p, i16 %q"), "{out}");
}
