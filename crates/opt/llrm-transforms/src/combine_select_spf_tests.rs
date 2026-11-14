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

/// `x < 0 ? 0 : (x > 255 ? 255 : x)` is `smax` and `smin` in ONE combine, as the pin's `InstCombine` leaves it: the inner select becomes
/// `smin`, and the outer one (a user of it) must see that call as the intrinsic. It did not while the declaration was pending in the pass,
/// so `clamp` stayed `select (icmp slt x, 0), 0, smin(x, 255)` and, inlined into wasm jpeg_fixed -O2, its compare folded into the sdiv's
/// operand: +4 executed instructions per pixel channel, +0.76% of the run against the pin.
#[test]
fn a_clamp_of_two_selects_is_min_and_max_in_one_combine() {
    let out = combined(
        "i16 %x",
        "  %lo = icmp slt i16 %x, 0\n  %hi = icmp sgt i16 %x, 255\n  %m = select i1 %hi, i16 255, i16 %x\n  %r = select i1 %lo, i16 0, i16 %m\n  ret i16 %r\n",
    );
    assert!(!out.contains("select"), "{out}");
    assert!(out.contains("@llvm.smax.i16") && out.contains("@llvm.smin.i16"), "{out}");
}

/// A select that clamps a value away from the minimum or maximum is the
/// matching min/max (`opt` agrees). `select (x == 0), 1, x` stayed a compare
/// and a select in sfm16_dbg `add_bits` (150 IR instructions against the
/// pin's 140; wasm32 O1 +224 executed instructions).
#[test]
fn a_clamp_away_from_an_extreme_is_a_min_or_max() {
    for (cmp, new, value, call) in [
        ("0", "1", "umax", "i16 @llvm.umax.i16(i16 %x, i16 1)"),
        ("-1", "-2", "umin", "i16 @llvm.umin.i16(i16 %x, i16 -2)"),
        ("-32768", "-32767", "smax", "i16 @llvm.smax.i16(i16 %x, i16 -32767)"),
        ("32767", "32766", "smin", "i16 @llvm.smin.i16(i16 %x, i16 32766)"),
    ] {
        let body = format!("  %c = icmp eq i16 %x, {cmp}\n  %r = select i1 %c, i16 {new}, i16 %x\n  ret i16 %r\n");
        let out = combined("i16 %x", &body);
        assert!(out.contains(call), "{value}: {out}");
        assert!(!out.contains("select"), "{value}: {out}");
    }
}

/// A different replacement is no clamp: `x == 0 ? 2 : x` stays a select.
#[test]
fn a_clamp_to_a_further_value_stays_a_select() {
    let out = combined("i16 %x", "  %c = icmp eq i16 %x, 0\n  %r = select i1 %c, i16 2, i16 %x\n  ret i16 %r\n");
    assert!(out.contains("select"), "{out}");
}

/// `x < y ? x : y - 1` is `smin(x, y - 1)` and `x > y ? x : y + 1` is `smax`;
/// the other overflow flag is dropped (`opt` agrees). The `y - 1` arm of
/// sfm16_dbg `add_bits` stayed a select.
#[test]
fn a_select_of_a_value_and_the_other_side_stepped_is_a_min_or_max() {
    let out = combined(
        "i16 %x, i16 %y",
        "  %d = add nsw i16 %y, -1\n  %c = icmp slt i16 %x, %y\n  %r = select i1 %c, i16 %x, i16 %d\n  ret i16 %r\n",
    );
    assert!(out.contains("%d = add nsw i16 %y, -1"), "{out}");
    assert!(out.contains("@llvm.smin.i16(i16 %x, i16 %d)"), "{out}");

    let out = combined(
        "i16 %x, i16 %y",
        "  %d = add nuw nsw i16 %y, 1\n  %c = icmp sgt i16 %x, %y\n  %r = select i1 %c, i16 %x, i16 %d\n  ret i16 %r\n",
    );
    assert!(out.contains("%d = add nsw i16 %y, 1"), "{out}");
    assert!(out.contains("@llvm.smax.i16(i16 %x, i16 %d)"), "{out}");

    let out = combined(
        "i16 %x, i16 %y",
        "  %d = add nuw i16 %y, 1\n  %c = icmp ugt i16 %x, %y\n  %r = select i1 %c, i16 %x, i16 %d\n  ret i16 %r\n",
    );
    assert!(out.contains("@llvm.umax.i16(i16 %x, i16 %d)"), "{out}");
}

/// Without the `nsw` the step may wrap, so the select stays (`opt` agrees).
#[test]
fn a_step_that_may_wrap_stays_a_select() {
    let out = combined(
        "i16 %x, i16 %y",
        "  %d = add i16 %y, -1\n  %c = icmp slt i16 %x, %y\n  %r = select i1 %c, i16 %x, i16 %d\n  ret i16 %r\n",
    );
    assert!(out.contains("select"), "{out}");
}
