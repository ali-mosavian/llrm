//! Each entry of `combine::visitors`, through the driver: the output has the
//! shape `opt -passes=instcombine` gives for the same input.

use llrm_mir::datalayout::DataLayout;

use crate::combine::combine;
use crate::testing::{parsed, printed};

/// `@f` after `combine`.
fn combined(
    ret: &str,
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.clone()), metadata.len());
    combine(context, &globals, &Default::default(), &metadata, &DataLayout::default(), function, &mut declared);
    printed(&module)
}

/// `body` through the driver must contain each of `wanted` and none of
/// `unwanted`; `what` names the visitor.
fn check(
    what: &str,
    ret: &str,
    params: &str,
    body: &str,
    wanted: &[&str],
    unwanted: &[&str],
) {
    let out = combined(ret, params, body);
    for one in wanted {
        assert!(out.contains(one), "{what}: `{one}` missing\n{out}");
    }
    for one in unwanted {
        assert!(!out.contains(one), "{what}: `{one}` present\n{out}");
    }
}

#[test]
fn visit_shl_distributes_over_an_add_after_the_multiply_became_a_shift() {
    check(
        "shl",
        "i16",
        "i16 %x",
        "  %s = add i16 %x, -5\n  %m = mul i16 %s, 8\n  %r = add i16 %m, 7\n  ret i16 %r\n",
        &["shl i16 %x, 3", ", -33"],
        &["mul"],
    );
}

#[test]
fn visit_lshr_of_a_shl_is_a_mask() {
    check(
        "lshr",
        "i16",
        "i16 %x",
        "  %s = shl i16 %x, 3\n  %r = lshr i16 %s, 3\n  ret i16 %r\n",
        &["and i16 %x, 8191"],
        &["shl", "lshr"],
    );
}

#[test]
fn visit_ashr_of_a_shl_keeps_the_pair_and_marks_it_exact() {
    check(
        "ashr",
        "i16",
        "i16 %x",
        "  %s = shl i16 %x, 8\n  %r = ashr i16 %s, 8\n  ret i16 %r\n",
        &["shl i16 %x, 8", "ashr exact i16 %"],
        &[],
    );
}

#[test]
fn visit_udiv_by_a_power_of_two_is_a_shift() {
    check("udiv", "i16", "i16 %x", "  %r = udiv i16 %x, 8\n  ret i16 %r\n", &["lshr i16 %x, 3"], &["udiv"]);
}

#[test]
fn visit_sdiv_exact_by_a_power_of_two_is_an_arithmetic_shift() {
    check("sdiv", "i16", "i16 %x", "  %r = sdiv exact i16 %x, 8\n  ret i16 %r\n", &["ashr exact i16 %x, 3"], &["sdiv"]);
}

#[test]
fn visit_urem_by_a_power_of_two_is_a_mask() {
    check("urem", "i16", "i16 %x", "  %r = urem i16 %x, 8\n  ret i16 %r\n", &["and i16 %x, 7"], &["urem"]);
}

#[test]
fn visit_srem_by_a_negative_power_of_two_drops_the_sign() {
    check("srem", "i16", "i16 %x", "  %r = srem i16 %x, -8\n  ret i16 %r\n", &["srem i16 %x, 8"], &[", -8"]);
}

#[test]
fn visit_trunc_computes_in_the_narrow_type() {
    check(
        "trunc",
        "i8",
        "i8 %a",
        "  %z = zext i8 %a to i32\n  %s = add i32 %z, 1\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        &["add i8 %a, 1"],
        &["zext", "trunc", "i32"],
    );
}

#[test]
fn visit_zext_of_a_trunc_is_a_mask() {
    check(
        "zext",
        "i16",
        "i16 %x",
        "  %t = trunc i16 %x to i8\n  %r = zext i8 %t to i16\n  ret i16 %r\n",
        &["and i16 %x, 255"],
        &["trunc", "zext"],
    );
}

#[test]
fn visit_sext_of_a_zext_is_a_zext() {
    check(
        "sext",
        "i32",
        "i8 %a",
        "  %z = zext i8 %a to i16\n  %r = sext i16 %z to i32\n  ret i32 %r\n",
        &["zext i8 %a to i32"],
        &["sext", "i16"],
    );
}

#[test]
fn visit_and_moves_a_mask_through_an_xor() {
    check(
        "and",
        "i32",
        "i32 %x",
        "  %e = xor i32 %x, 12\n  %r = and i32 %e, 10\n  ret i32 %r\n",
        &["and i32 %x, 10", "xor i32 %", ", 8"],
        &["xor i32 %x, 12"],
    );
}

#[test]
fn visit_xor_hoists_a_constant_out_of_the_inner_xor() {
    check(
        "xor",
        "i32",
        "i32 %x, i32 %y",
        "  %e = xor i32 %x, 77\n  %r = xor i32 %e, %y\n  ret i32 %r\n",
        &["xor i32 %x, %y", ", 77"],
        &[],
    );
}

#[test]
fn visit_or_isolates_the_lowest_set_bit() {
    check(
        "or",
        "i32",
        "i32 %x",
        "  %n = xor i32 %x, -1\n  %d = add i32 %x, -1\n  %r = or i32 %n, %d\n  ret i32 %r\n",
        &["sub i32 0, %x", "and i32 %x, %"],
        &[" or i32"],
    );
}

/// B$SCMP's result is the machine flags: `sext (icmp slt v, 0)` became
/// `ashr v, 15`, which instruction selection cannot read (str_slices and
/// every string compare at -O1 failed to compile).
#[test]
fn a_sext_of_a_flags_compare_stays_a_compare() {
    let mut module = parsed(
        "declare i16 @cmp(ptr, ptr) flagsresult\n\
         define i16 @f(ptr %a, ptr %b) {\nb0:\n  %v = call i16 @cmp(ptr %a, ptr %b)\n  %c = icmp slt i16 %v, 0\n  %r = sext i1 %c to i16\n  ret i16 %r\n}\n",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.clone()), metadata.len());
    combine(context, &globals, &Default::default(), &metadata, &DataLayout::default(), function, &mut declared);
    let out = printed(&module);
    assert!(out.contains("icmp slt i16 %v, 0") && out.contains("sext i1"), "{out}");
    assert!(!out.contains("ashr"), "{out}");
}
