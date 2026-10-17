use crate::context::ConstantKind;
use crate::module::{Module, Operand};
use crate::{parse, simplify};

/// What `simplify_instruction` makes of `%name` in `@f`, spelled `%x` for a
/// value, `N` for a number or `poison`; None where it finds nothing.
fn simplified(
    text: &str,
    name: &str,
) -> Option<String> {
    let mut module: Module = parse::module(text).unwrap_or_else(|why| panic!("{why:?}\n{text}"));
    let id = module.named("f").expect("@f");
    let crate::module::GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else {
        panic!("a function")
    };
    let function = function.clone();
    let inst = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| {
            function.instruction(inst).result.is_some_and(|result| function.value(result).name.as_deref() == Some(name))
        })
        .expect("a named instruction");
    let found = simplify::simplify_instruction(
        &mut module.context,
        &crate::datalayout::DataLayout::default(),
        &function,
        inst,
    )?;
    Some(match found {
        Operand::Value(value) => format!("%{}", function.value(value).name.clone().unwrap_or_default()),
        Operand::Constant(id) => match module.context.get(id).kind {
            ConstantKind::Int(bits) => bits.to_string(),
            ConstantKind::Poison => "poison".to_owned(),
            ConstantKind::Null | ConstantKind::Zero => "0".to_owned(),
            ref other => format!("{other:?}"),
        },
        Operand::Block(_) => unreachable!("an instruction is never a block"),
    })
}

/// A body of `i16`s: `body` after the parameters `%x`, `%y`.
fn of(body: &str) -> String {
    format!("define i16 @f(i16 %x, i16 %y, i1 %c) {{\nb0:\n{body}  ret i16 %x\n}}\n")
}

fn check(
    body: &str,
    name: &str,
    expected: Option<&str>,
) {
    assert_eq!(simplified(&of(body), name).as_deref(), expected, "{body}");
}

#[test]
fn additive_identities_and_negations() {
    check("  %r = add i16 %x, 0\n", "r", Some("%x"));
    check("  %r = add i16 0, %x\n", "r", Some("%x"));
    check("  %n = sub i16 0, %x\n  %r = add i16 %x, %n\n", "r", Some("0"));
    check("  %d = sub i16 %y, %x\n  %r = add i16 %x, %d\n", "r", Some("%y"));
    check("  %nx = xor i16 %x, -1\n  %r = add i16 %x, %nx\n", "r", Some("65535"));
    check("  %r = add i16 %x, %y\n", "r", None);
    check("  %r = sub i16 %x, 0\n", "r", Some("%x"));
    check("  %r = sub i16 %x, %x\n", "r", Some("0"));
    check("  %a = add i16 %x, %y\n  %r = sub i16 %a, %y\n", "r", Some("%x"));
    check("  %r = sub i16 %x, %y\n", "r", None);
}

#[test]
fn multiplicative_and_division_cases() {
    check("  %r = mul i16 %x, 1\n", "r", Some("%x"));
    check("  %r = mul i16 1, %x\n", "r", Some("%x"));
    check("  %r = mul i16 %x, 0\n", "r", Some("0"));
    check("  %r = udiv i16 %x, 0\n", "r", Some("poison"));
    check("  %r = udiv i16 0, %y\n", "r", Some("0"));
    check("  %r = sdiv i16 %x, %x\n", "r", Some("1"));
    check("  %r = urem i16 %x, %x\n", "r", Some("0"));
    check("  %m = mul i16 %x, %y\n  %r = udiv exact i16 %m, %y\n", "r", None);
    check("  %m = mul nuw i16 %x, %y\n  %r = udiv i16 %m, %y\n", "r", Some("%x"));
    check("  %m = mul nuw i16 %x, %y\n  %r = urem i16 %m, %y\n", "r", Some("0"));
}

#[test]
fn shifts() {
    check("  %r = shl i16 %x, 0\n", "r", Some("%x"));
    check("  %r = shl i16 0, %x\n", "r", Some("0"));
    check("  %r = lshr i16 %x, 16\n", "r", Some("poison"));
    check("  %r = lshr i16 %x, %x\n", "r", Some("0"));
    check("  %s = shl nuw i16 %x, %y\n  %r = lshr i16 %s, %y\n", "r", Some("%x"));
    check("  %s = shl nsw i16 %x, %y\n  %r = ashr i16 %s, %y\n", "r", Some("%x"));
    check("  %r = ashr i16 -1, %y\n", "r", Some("65535"));
}

#[test]
fn logic() {
    check("  %r = and i16 %x, %x\n", "r", Some("%x"));
    check("  %r = and i16 %x, 0\n", "r", Some("0"));
    check("  %r = and i16 %x, -1\n", "r", Some("%x"));
    check("  %n = xor i16 %x, -1\n  %r = and i16 %x, %n\n", "r", Some("0"));
    check("  %o = or i16 %x, %y\n  %r = and i16 %o, %x\n", "r", Some("%x"));
    check("  %r = or i16 %x, -1\n", "r", Some("65535"));
    check("  %r = or i16 %x, 0\n", "r", Some("%x"));
    check("  %n = xor i16 %x, -1\n  %r = or i16 %x, %n\n", "r", Some("65535"));
    check("  %a = and i16 %x, %y\n  %r = or i16 %x, %a\n", "r", Some("%x"));
    check("  %r = xor i16 %x, %x\n", "r", Some("0"));
    check("  %r = xor i16 %x, 0\n", "r", Some("%x"));
    check("  %r = xor i16 %x, %y\n", "r", None);
    check("  %a = and i16 %x, 255\n  %r = and i16 %a, 255\n", "r", Some("%a"));
    check("  %s = shl i16 %x, 8\n  %r = and i16 %s, 65280\n", "r", Some("%s"));
}

#[test]
fn select_phi_cast_and_compare() {
    check("  %r = select i1 true, i16 %x, i16 %y\n", "r", Some("%x"));
    check("  %r = select i1 false, i16 %x, i16 %y\n", "r", Some("%y"));
    check("  %r = select i1 %c, i16 %x, i16 %x\n", "r", Some("%x"));
    check("  %r = select i1 %c, i1 true, i1 false\n", "r", Some("%c"));
    let phi = "define i16 @f(i16 %x, i1 %c) {\nb0:\n  br i1 %c, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %r = phi i16 [ %x, %a ], [ %x, %b ]\n  ret i16 %r\n}\n";
    assert_eq!(simplified(phi, "r").as_deref(), Some("%x"));
    check("  %w = zext i16 %x to i32\n  %r = trunc i32 %w to i16\n", "r", Some("%x"));
    check("  %w = sext i16 %x to i32\n  %r = trunc i32 %w to i16\n", "r", Some("%x"));
    check("  %t = trunc i16 %x to i8\n  %r = zext i8 %t to i16\n", "r", None);
    check("  %r = icmp eq i16 %x, %x\n", "r", Some("1"));
    check("  %r = icmp ult i16 %x, %x\n", "r", Some("0"));
    check("  %r = icmp ult i16 %x, 0\n", "r", Some("0"));
    check("  %r = icmp uge i16 %x, 0\n", "r", Some("1"));
    check("  %w = zext i16 %x to i32\n  %v = zext i16 %y to i32\n  %r = icmp ult i32 %w, %v\n", "r", None);
}

/// The compares the zero-half stand-in missed, now answered from the real
/// ValueTracking: a mask is below its source, a shift amount known to be the
/// width is poison, and `lshr` bounds a range.
#[test]
fn compares_answered_from_value_tracking() {
    check("  %a = and i16 %x, %y\n  %c1 = icmp uge i16 %x, %a\n", "c1", Some("1"));
    check("  %o = or i16 %x, %y\n  %c1 = icmp ult i16 %o, %x\n", "c1", Some("0"));
    check("  %s = lshr i16 %x, 3\n  %c1 = icmp ult i16 %s, 8192\n", "c1", Some("1"));
    check("  %o = or i16 %x, 16\n  %c1 = icmp eq i16 %o, 0\n", "c1", Some("0"));
    check("  %r = shl i16 %x, 16\n", "r", Some("poison"));
}

/// Answers `opt -passes=instsimplify` gives that the zero half of the known
/// bits could not: a set low bit survives an exact right shift, an exact
/// division by a divisor with trailing zeros needs a dividend that has them,
/// and the exact-division multiply is matched on its left operand only.
#[test]
fn exact_shifts_and_divisions_read_the_one_bits() {
    check("  %o = or i16 %x, 1\n  %r = lshr exact i16 %o, %y\n", "r", Some("%o"));
    check("  %o = or i16 %x, 1\n  %r = ashr exact i16 %o, %y\n", "r", Some("%o"));
    check("  %o = or i16 %x, 2\n  %r = lshr exact i16 %o, %y\n", "r", None);
    check("  %o = or i16 %x, 1\n  %r = udiv exact i16 %o, 4\n", "r", Some("poison"));
    check("  %o = or i16 %x, 1\n  %r = sdiv exact i16 %o, 6\n", "r", Some("poison"));
    check("  %o = or i16 %x, 2\n  %r = udiv exact i16 %o, 4\n", "r", Some("poison"));
    check("  %m = mul nsw i16 %x, 3\n  %r = udiv exact i16 %m, 3\n", "r", Some("%x"));
    check("  %m = mul nsw i16 3, %x\n  %r = udiv exact i16 %m, 3\n", "r", None);
    check("  %m = mul nuw i16 3, %x\n  %r = sdiv exact i16 %m, 3\n", "r", None);
}

/// The power-of-two cases of `simplifyAnd`.
#[test]
fn and_of_a_power_of_two() {
    let p = "  %p = shl i16 1, %y\n";
    check(&format!("{p}  %n = sub i16 0, %p\n  %r = and i16 %n, %p\n"), "r", Some("%p"));
    check(&format!("{p}  %a = add i16 %p, -1\n  %r = and i16 %a, %p\n"), "r", Some("0"));
    let t = |mask: u32| {
        format!("  %t = and i16 %y, {mask}\n  %p = shl i16 1, %t\n  %a = add i16 %p, -1\n  %r = and i16 %a, 16\n")
    };
    check(&t(3), "r", Some("0"));
    check(&t(7), "r", None);
    check(
        &format!("{p}  %s1 = shl i16 %p, 5\n  %s2 = shl i16 %p, 3\n  %a = add i16 %s2, -1\n  %r = and i16 %s1, %a\n"),
        "r",
        Some("0"),
    );
}

/// The `isImpliedCondition` closing of `simplifyICmpOfBools`.
#[test]
fn compare_of_implied_bools() {
    let b = "  %a = icmp ult i16 %x, %y\n  %b = icmp ule i16 %x, %y\n";
    check(&format!("{b}  %r = icmp uge i1 %b, %a\n"), "r", Some("1"));
    check(&format!("{b}  %r = icmp ule i1 %a, %b\n"), "r", Some("1"));
    check(&format!("{b}  %r = icmp sge i1 %a, %b\n"), "r", Some("1"));
    check(&format!("{b}  %r = icmp sle i1 %b, %a\n"), "r", Some("1"));
    check(&format!("{b}  %r = icmp uge i1 %a, %b\n"), "r", None);
}

/// What the known bits of the other operand of an `or` with a shifted value
/// prove: the shift-out of `((X << A) | Y) >> A` and the mask of `((X << A) |
/// Y) & M`.
#[test]
fn shifted_or_with_a_narrow_operand() {
    let o = "  %s = shl nuw i16 %x, 8\n  %y8 = and i16 %y, 255\n  %o = or i16 %s, %y8\n";
    let p = "  %s = shl nuw i16 %x, 8\n  %y8 = and i16 %y, 255\n  %o = or i16 %y8, %s\n";
    check(&format!("{o}  %r = lshr i16 %o, 8\n"), "r", Some("%x"));
    check(&format!("{p}  %r = lshr i16 %o, 8\n"), "r", Some("%x"));
    check(
        "  %s = shl nuw i16 %x, 8\n  %y9 = and i16 %y, 511\n  %o = or i16 %s, %y9\n  %r = lshr i16 %o, 8\n",
        "r",
        None,
    );
    check(&format!("{o}  %r = and i16 %o, 255\n"), "r", Some("%y8"));
    check(&format!("{o}  %r = and i16 %o, 65280\n"), "r", Some("%s"));
    check(&format!("{p}  %r = and i16 %o, 65280\n"), "r", Some("%s"));
    check(&format!("{o}  %r = and i16 %o, 15\n"), "r", None);
}

/// `ComputeNumSignBits` of the real ValueTracking: an arithmetic shift of a
/// value of all sign bits is the value.
#[test]
fn ashr_of_all_sign_bits() {
    check("  %b = icmp slt i16 %x, 0\n  %s = sext i1 %b to i16\n  %r = ashr i16 %s, %y\n", "r", Some("%s"));
    check(
        "  %t = trunc i16 %x to i8\n  %s = sext i8 %t to i16\n  %u = ashr i16 %s, 8\n  %r = ashr i16 %u, %y\n",
        "r",
        Some("%u"),
    );
    check("  %u = ashr i16 %x, 15\n  %v = or i16 %u, %u\n  %r = ashr i16 %v, %y\n", "r", Some("%v"));
    check("  %u = ashr i16 %x, 15\n  %v = select i1 %c, i16 %u, i16 0\n  %r = ashr i16 %v, %y\n", "r", Some("%v"));
}

/// `isImpliedCondition` closes `and`/`or` of bools, and `MaskedValueIsZero`
/// merges `(V + N) & C1 | V & C2`.
#[test]
fn and_or_of_implied_bools_and_masked_adds() {
    let b = "  %a = icmp ult i16 %x, %y\n  %b = icmp ule i16 %x, %y\n";
    check(&format!("{b}  %r = and i1 %a, %b\n"), "r", Some("%a"));
    check(&format!("{b}  %r = and i1 %b, %a\n"), "r", Some("%a"));
    check("  %a = icmp ult i16 %x, %y\n  %b = icmp ugt i16 %x, %y\n  %r = and i1 %a, %b\n", "r", Some("0"));
    check(&format!("{b}  %r = or i1 %a, %b\n"), "r", Some("%b"));
    check(&format!("{b}  %r = or i1 %b, %a\n"), "r", Some("%b"));
    check("  %a = icmp ult i16 %x, %y\n  %b = icmp uge i16 %x, %y\n  %r = or i1 %a, %b\n", "r", Some("1"));
    let m = "  %n = shl i16 %y, 8\n  %p = add i16 %x, %n\n";
    check(&format!("{m}  %a = and i16 %p, 65280\n  %b = and i16 %x, 255\n  %r = or i16 %a, %b\n"), "r", Some("%p"));
    check(&format!("{m}  %a = and i16 %p, 255\n  %b = and i16 %x, 65280\n  %r = or i16 %a, %b\n"), "r", None);
}

/// `simplifyShift` asks `KnownBits::shl` with no wrap flags: with `nsw` it
/// left the poison of `shl nsw 32767, X` (X >= 1) unfound.
#[test]
fn nsw_shl_by_a_nonzero_amount_of_a_large_value() {
    let o = "  %o = or i16 %x, 9\n";
    check(&format!("{o}  %r = shl nsw i16 32767, %o\n"), "r", Some("poison"));
    check(&format!("{o}  %r = shl nsw i16 32767, %y\n"), "r", None);
    check("  %o = or i16 %x, 1\n  %r = shl nsw i16 -32768, %o\n", "r", Some("poison"));
}

/// `simplifyICmpWithBinOp`: a compare of a binary operator against an
/// operand of it, or against another binary operator, answered as LLVM does.
#[test]
fn compare_with_binary_operators() {
    let cases: &[(&str, Option<&str>)] = &[
        ("  %a = add nuw i16 %x, %y\n  %r = icmp ult i16 %a, %x\n", Some("0")),
        ("  %a = add i16 %x, %y\n  %r = icmp eq i16 %a, %x\n", None),
        ("  %a = add nsw i16 %x, %y\n  %r = icmp slt i16 %x, %a\n", None),
        ("  %a = add nuw i16 %x, 1\n  %b = add nuw i16 %y, 1\n  %r = icmp ult i16 %a, %b\n", None),
        ("  %a = add nsw i16 %x, 1\n  %b = add nsw i16 %x, 2\n  %r = icmp slt i16 %a, %b\n", Some("1")),
        ("  %a = add i16 %x, 1\n  %b = add nsw i16 %x, 2\n  %r = icmp slt i16 %a, %b\n", Some("1")),
        ("  %a = add i16 %x, -2\n  %b = add nsw i16 %x, -3\n  %r = icmp sgt i16 %a, %b\n", Some("1")),
        ("  %o = or i16 %x, %y\n  %n = xor i16 %y, -1\n  %r = icmp slt i16 %o, %x\n", None),
        ("  %p = and i16 %x, 32767\n  %o = or i16 %p, %y\n  %r = icmp sge i16 %o, %p\n", None),
        ("  %m = and i16 %y, 32767\n  %o = or i16 %x, %m\n  %r = icmp slt i16 %o, %x\n", Some("0")),
        ("  %u = urem i16 %x, %y\n  %r = icmp ult i16 %u, %y\n", Some("1")),
        ("  %u = urem i16 %x, %y\n  %r = icmp eq i16 %u, %y\n", Some("0")),
        ("  %u = urem i16 %x, %y\n  %r = icmp sgt i16 %u, %y\n", None),
        ("  %p = and i16 %y, 32767\n  %u = urem i16 %x, %p\n  %r = icmp sgt i16 %u, %p\n", Some("0")),
        ("  %p = and i16 %y, 32767\n  %u = urem i16 %x, %p\n  %r = icmp sle i16 %u, %p\n", Some("1")),
        ("  %z = or i16 %x, 1\n  %s = lshr i16 %z, 2\n  %r = icmp ult i16 %s, %z\n", Some("1")),
        ("  %z = or i16 %x, 1\n  %s = udiv i16 %z, 3\n  %r = icmp uge i16 %s, %z\n", Some("0")),
        ("  %m = mul i16 %x, 3\n  %d = udiv i16 %m, 3\n  %r = icmp ugt i16 %d, %x\n", Some("0")),
        ("  %m = mul i16 %x, 5\n  %d = udiv i16 %m, 3\n  %r = icmp ugt i16 %d, %x\n", None),
        ("  %m = mul i16 %x, 3\n  %d = lshr i16 %m, 2\n  %r = icmp ule i16 %d, %x\n", Some("1")),
        ("  %m = shl i16 %x, 1\n  %d = udiv i16 %m, 3\n  %r = icmp ule i16 %d, %x\n", Some("1")),
        ("  %m = shl i16 %x, 2\n  %d = udiv i16 %m, 3\n  %r = icmp ule i16 %d, %x\n", None),
        ("  %s = sub i16 5, %x\n  %r = icmp eq i16 %s, %x\n", Some("0")),
        ("  %s = sub i16 6, %x\n  %r = icmp ne i16 %s, %x\n", None),
        (
            "  %z = zext i8 3 to i16\n  %w = zext i1 %c to i16\n  %n = sub i16 0, %w\n  %r = icmp slt i16 %n, 2\n",
            Some("1"),
        ),
        ("  %w = zext i1 %c to i16\n  %n = sub i16 0, %w\n  %r = icmp sle i16 %n, 0\n", Some("1")),
        ("  %w = zext i1 %c to i16\n  %n = sub i16 0, %w\n  %r = icmp eq i16 %n, 3\n", Some("0")),
        ("  %w = zext i1 %c to i16\n  %n = sub i16 0, %w\n  %r = icmp sgt i16 %n, 0\n", Some("0")),
        ("  %s = shl i16 4, %y\n  %r = icmp eq i16 %s, 6\n", Some("0")),
        ("  %s = shl i16 4, %y\n  %r = icmp eq i16 %s, 0\n", None),
        ("  %s = shl nuw i16 4, %y\n  %r = icmp eq i16 %s, 0\n", Some("0")),
        ("  %s = shl i16 1, %y\n  %r = icmp ne i16 %s, 0\n", Some("1")),
        ("  %s = shl i16 4, %y\n  %r = icmp ugt i16 %s, 32768\n", Some("0")),
        ("  %s = shl i16 4, %y\n  %r = icmp ule i16 %s, 32768\n", Some("1")),
        ("  %a = and i16 %x, 3\n  %b = and i16 %x, 7\n  %r = icmp ule i16 %a, %b\n", Some("1")),
        ("  %a = and i16 %x, 3\n  %b = and i16 %x, 7\n  %r = icmp ugt i16 %b, %a\n", None),
        ("  %a = and i16 %x, 3\n  %b = and i16 %x, 7\n  %r = icmp ugt i16 %a, %b\n", Some("0")),
        ("  %a = or i16 %x, 3\n  %b = or i16 %x, 7\n  %r = icmp sle i16 %a, %b\n", Some("1")),
        ("  %a = or i16 %x, 3\n  %b = or i16 %x, 32775\n  %r = icmp sle i16 %a, %b\n", None),
        ("  %a = or i16 %x, 3\n  %b = or i16 %x, 32775\n  %r = icmp ule i16 %a, %b\n", Some("1")),
        ("  %a = or i16 %x, 3\n  %b = or i16 %x, 5\n  %r = icmp ule i16 %a, %b\n", None),
        (
            "  %o = or i16 %y, 1\n  %a = shl nuw i16 %x, %o\n  %b = shl nuw i16 %x, %y\n  %r = icmp ult i16 %a, %b\n",
            None,
        ),
        (
            "  %n = or i16 %x, 1\n  %a = shl nuw i16 %n, %x\n  %b = shl nuw i16 %n, %y\n  %r = icmp eq i16 %a, %b\n",
            None,
        ),
        (
            "  %n = or i16 %x, 1\n  %a = shl nuw i16 %n, %x\n  %b = shl nuw i16 %n, %y\n  %r = icmp slt i16 %a, %b\n",
            None,
        ),
        (
            "  %n = or i16 %x, 1\n  %a = shl nuw nsw i16 %n, %x\n  %b = shl nuw nsw i16 %n, %y\n  %r = icmp slt i16 %a, %b\n",
            None,
        ),
        ("  %a = lshr exact i16 %x, %y\n  %b = lshr exact i16 %x, %y\n  %r = icmp ult i16 %a, %b\n", Some("0")),
        ("  %a = sdiv exact i16 %x, %y\n  %b = sdiv exact i16 %x, %y\n  %r = icmp eq i16 %a, %b\n", Some("1")),
        ("  %a = ashr exact i16 %x, %y\n  %b = ashr exact i16 %x, %y\n  %r = icmp slt i16 %a, %b\n", Some("0")),
        ("  %a = shl nuw i16 %x, %y\n  %b = shl nuw i16 %x, %y\n  %r = icmp ult i16 %a, %b\n", Some("0")),
        ("  %a = shl nsw i16 %x, %y\n  %b = shl nsw i16 %x, %y\n  %r = icmp slt i16 %a, %b\n", Some("0")),
    ];
    let wrong: Vec<_> =
        cases.iter().filter(|(body, expected)| simplified(&of(body), "r").as_deref() != *expected).collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Differential check against `opt -passes=instsimplify` (not part of the
/// gate): `LLRM_SIMPLIFY_CASES=in.ll LLRM_SIMPLIFY_OUT=out.txt cargo test -p
/// llrm-mir --lib simplify_cases -- --ignored`. Each function `@fN` has a
/// target `%t`; the output says, for each, whether `simplify_instruction` found
/// a value for it.
#[test]
#[ignore]
fn simplify_cases() {
    let (Some(input), Some(output)) = (std::env::var_os("LLRM_SIMPLIFY_CASES"), std::env::var_os("LLRM_SIMPLIFY_OUT"))
    else {
        return;
    };
    let text = std::fs::read_to_string(input).expect("cases");
    let mut module: Module = parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut lines = String::new();
    for at in 0..module.globals.len() {
        let crate::module::GlobalKind::Function(function) = &module.globals[at].kind else { continue };
        let function = function.clone();
        let Some(inst) = function.walk().map(|(_, inst)| inst).find(|&inst| {
            function.instruction(inst).result.is_some_and(|result| function.value(result).name.as_deref() == Some("t"))
        }) else {
            continue;
        };
        let found = simplify::simplify_instruction(
            &mut module.context,
            &crate::datalayout::DataLayout::default(),
            &function,
            inst,
        )
        .map(|value| match value {
            Operand::Value(value) => format!("%{}", function.value(value).name.clone().unwrap_or_default()),
            Operand::Constant(id) => match module.context.get(id).kind {
                ConstantKind::Int(bits) => bits.to_string(),
                ConstantKind::Poison => "poison".to_owned(),
                ConstantKind::Null | ConstantKind::Zero => "0".to_owned(),
                ref other => format!("{other:?}"),
            },
            Operand::Block(_) => unreachable!("an instruction is never a block"),
        });
        lines += &format!(
            "{} {}\n",
            module.globals[at].name.clone().unwrap_or_default(),
            found.map_or("none".to_owned(), |value| format!("simplified {value}"))
        );
    }
    std::fs::write(output, lines).expect("out");
}

/// `instsimplify` on the body `pipeline.body` of each module in a list, as
/// LLVM's pass does it (delete the trivially dead, replace what simplifies,
/// until nothing does), then the opcodes left: the differential on real pass
/// inputs. `LLRM_SIMPLIFY_FILES=list.txt LLRM_SIMPLIFY_OUT=out.tsv cargo test
/// -p llrm-mir --lib simplify_dumps -- --ignored`.
#[test]
#[ignore]
fn simplify_dumps() {
    let (Some(list), Some(output)) = (std::env::var_os("LLRM_SIMPLIFY_FILES"), std::env::var_os("LLRM_SIMPLIFY_OUT"))
    else {
        return;
    };
    let mut lines = String::new();
    for path in std::fs::read_to_string(list).expect("list").lines() {
        let text = std::fs::read_to_string(path).expect("module");
        let Ok(mut module) = parse::module(&text) else {
            lines += &format!("{path}\tunparsed\n");
            continue;
        };
        let Some((context, function)) = module.function_mut("pipeline.body") else {
            lines += &format!("{path}\tno body\n");
            continue;
        };
        loop {
            let mut changed = false;
            for inst in function.walk().map(|(_, inst)| inst).collect::<Vec<_>>() {
                if function.parent(inst).is_none() {
                    continue;
                }
                let instruction = function.instruction(inst);
                let Some(result) = instruction.result else { continue };
                let unused = function.users(result).is_empty();
                use crate::opcode::Opcode;
                let pure = matches!(
                    instruction.opcode,
                    Opcode::Binary(_)
                        | Opcode::Cast(_)
                        | Opcode::ICmp(_)
                        | Opcode::FCmp(_)
                        | Opcode::Select
                        | Opcode::Phi
                        | Opcode::GetElementPtr { .. }
                        | Opcode::Alloca { .. }
                        | Opcode::FNeg
                        | Opcode::Freeze
                ) || matches!(instruction.opcode, Opcode::Load { volatile: false, .. });
                if unused && pure {
                    function.erase(inst).expect("unused");
                    changed = true;
                    continue;
                }
                if let Some(found) =
                    simplify::simplify_instruction(context, &crate::datalayout::DataLayout::default(), function, inst)
                    && found != Operand::Value(result)
                {
                    function.replace_value(result, found);
                    function.erase(inst).expect("replaced");
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for (_, inst) in function.walk() {
            *counts.entry(function.instruction(inst).opcode.mnemonic()).or_default() += 1;
        }
        lines += &format!(
            "{path}\t{}\n",
            counts.iter().map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(",")
        );
    }
    std::fs::write(output, lines).expect("out");
}

#[test]
fn address_with_only_zero_indices_is_its_pointer() {
    let text = "define ptr @f(ptr %p, i16 %i) {\nb0:\n  %z = getelementptr i16, ptr %p, i16 0\n  %w = getelementptr [4 x i16], ptr %p, i16 0, i16 0\n  %v = getelementptr i16, ptr %p, i16 %i\n  ret ptr %z\n}\n";
    assert_eq!(simplified(text, "z").as_deref(), Some("%p"));
    assert_eq!(simplified(text, "w").as_deref(), Some("%p"));
    assert_eq!(simplified(text, "v"), None);
}
