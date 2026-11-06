use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitAnd`, `visitOr` and `visitXor`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::And) => crate::combine_and::visit_and(c, inst),
        Opcode::Binary(BinaryOp::Or) => crate::combine_or::visit_or(c, inst),
        Opcode::Binary(BinaryOp::Xor) => crate::combine_xor::visit_xor(c, inst),
        _ => Visit::None,
    }
}

/// `@f(params)` returning `ret` after the logic visitors alone.
fn combined(
    ret: &str,
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define {ret} @f({params}) {{\nb0:\n{body}  ret {ret} %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

fn check(
    ret: &str,
    params: &str,
    body: &str,
    wanted: &[&str],
    unwanted: &[&str],
) {
    let out = combined(ret, params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    for one in unwanted {
        assert!(!out.contains(one), "`{one}` present\n{out}");
    }
}

/// bench/lru: `a >= 0 or b >= 0` of two QB booleans stayed `or i16` of two
/// `sext` (a `setns`, `movzx`, `neg` pair per test); the pin makes one `or i1`.
#[test]
fn an_or_of_two_sign_extended_booleans_is_one_boolean_or() {
    check(
        "i16",
        "i1 %a, i1 %b",
        "  %x = sext i1 %a to i16\n  %y = sext i1 %b to i16\n  %r = or i16 %x, %y\n",
        &["or i1 %a, %b", "sext i1 %"],
        &["or i16"],
    );
}

/// A cast with other users still folds while the other has one.
#[test]
fn the_fold_needs_only_one_cast_with_one_use() {
    check(
        "i16",
        "i1 %a, i1 %b, ptr %p",
        "  %x = zext i1 %a to i16\n  %y = zext i1 %b to i16\n  store i16 %y, ptr %p\n  %r = xor i16 %x, %y\n",
        &["xor i1 %a, %b"],
        &["xor i16"],
    );
}

/// Sources of two widths: the narrower is widened and the logic narrowed (then
/// the `and` of a `sext` bool becomes a select), as the pin does.
#[test]
fn sources_of_two_widths_meet_at_the_wider() {
    check(
        "i32",
        "i1 %a, i8 %b",
        "  %x = sext i1 %a to i32\n  %y = sext i8 %b to i32\n  %r = and i32 %x, %y\n",
        &["select i1 %a, i8 %b, i8 0", "sext i8 %"],
        &["and i32"],
    );
}

/// Casts of two kinds do not fold.
#[test]
fn a_zext_and_a_sext_stay() {
    check(
        "i16",
        "i1 %a, i1 %b",
        "  %x = zext i1 %a to i16\n  %y = sext i1 %b to i16\n  %r = or i16 %x, %y\n",
        &["or i16"],
        &["or i1 %"],
    );
}

/// Truncs of one source type narrow to the logic and trunc once.
#[test]
fn an_and_of_two_truncs_is_a_trunc_of_the_and() {
    check(
        "i8",
        "i32 %a, i32 %b",
        "  %x = trunc i32 %a to i8\n  %y = trunc i32 %b to i8\n  %r = and i8 %x, %y\n",
        &["and i32 %a, %b", "trunc i32 %"],
        &["and i8"],
    );
}

/// `(zext i8 x) & 15` stayed an `and i16` (a `movzx` then a mask); the pin
/// narrows it to `zext (and i8 x, 15)`.
#[test]
fn a_zext_masked_by_a_constant_that_fits_is_the_zext_of_the_narrow_and() {
    check(
        "i16",
        "i8 %x",
        "  %z = zext i8 %x to i16\n  %r = and i16 %z, 15\n",
        &["and i8 %x, 15", "zext i8 %"],
        &["and i16"],
    );
}

/// A constant that does not fit the source stays.
#[test]
fn a_constant_wider_than_the_source_stays() {
    check("i16", "i8 %x", "  %z = zext i8 %x to i16\n  %r = or i16 %z, 256\n", &["i16 %z, 256"], &["or i8"]);
}

/// A `sext` takes a constant that is lossless as a signed truncation.
#[test]
fn a_sext_xored_with_a_negative_constant_that_fits_is_the_sext_of_the_narrow_xor() {
    check(
        "i16",
        "i8 %x",
        "  %z = sext i8 %x to i16\n  %r = xor i16 %z, -2\n",
        &["xor i8 %x, -2", "sext i8 %"],
        &["xor i16"],
    );
}
