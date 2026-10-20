use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitOr`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Or) {
        super::visit_or(c, inst)
    } else {
        Visit::None
    }
}

/// `@f(params)` returning i32 after `visitOr` alone.
fn combined(
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define i32 @f({params}) {{\nb0:\n{body}  ret i32 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `body` must come out with each of `wanted` and none of `unwanted`, the
/// shape pinned `opt -passes=instcombine` gives.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
    unwanted: &[&str],
) {
    let out = combined(params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    for one in unwanted {
        assert!(!out.contains(one), "`{one}` present\n{out}");
    }
}

const X: &str = "i32 %x";
const XY: &str = "i32 %x, i32 %y";

/// `(X ^ 240) | (Y & 15)` is `(X | Y') ^ 240`: the xor constant does not meet
/// the bits of the other operand.
#[test]
fn a_xor_constant_moves_out_of_an_or_that_it_does_not_overlap() {
    check(
        XY,
        "  %yy = and i32 %y, 15\n  %e = xor i32 %x, 240\n  %r = or i32 %e, %yy\n",
        &["or i32 %x, %yy", "xor i32 %", ", 240"],
        &["xor i32 %x, 240"],
    );
}

/// With bits in common the xor stays.
#[test]
fn a_xor_constant_over_the_bits_of_the_other_operand_stays() {
    check(XY, "  %e = xor i32 %x, 240\n  %r = or i32 %e, %y\n", &["xor i32 %x, 240"], &[]);
}

#[test]
fn a_disjoint_or_of_a_product_and_a_factor_is_a_product_by_the_increment() {
    check(XY, "  %m = mul i32 %x, %y\n  %r = or disjoint i32 %m, %x\n", &["add i32 %y, 1", "mul i32 %x, %"], &[" or "]);
}

#[test]
fn a_not_or_the_decrement_is_the_not_of_the_lowest_set_bit_isolation() {
    check(
        X,
        "  %n = xor i32 %x, -1\n  %d = add i32 %x, -1\n  %r = or i32 %n, %d\n",
        &["sub i32 0, %x", "and i32 %x, %", "xor i32 %", ", -1"],
        &[" or "],
    );
}

#[test]
fn two_masks_that_split_the_bits_keep_one_mask() {
    check(
        "i32 %x, i32 %b",
        "  %o = or i32 %x, %b\n  %p = and i32 %o, 255\n  %q = and i32 %b, -256\n  %r = or i32 %p, %q\n",
        &["and i32 %x, 255", "or i32 %", ", %b"],
        &["-256"],
    );
    check(
        "i32 %x, i32 %b",
        "  %o = xor i32 %x, %b\n  %p = and i32 %o, 255\n  %q = and i32 %b, -256\n  %r = or i32 %p, %q\n",
        &["and i32 %x, 255", "xor i32 %", ", %b"],
        &["-256"],
    );
}

/// Unverified against opt, which goes on (demanded bits) to `(x & 238) | 17`:
/// `((X | 1) & 15) | ((X | 16) & 240)` is `(X | 17) & 255`.
#[test]
fn two_ors_of_one_value_under_disjoint_masks_are_one_or_under_both() {
    check(
        X,
        "  %a = or i32 %x, 1\n  %p = and i32 %a, 15\n  %b = or i32 %x, 16\n  %q = and i32 %b, 240\n  %r = or i32 %p, %q\n",
        &["or i32 %x, 17", "and i32 %", ", 255"],
        &["240"],
    );
}
