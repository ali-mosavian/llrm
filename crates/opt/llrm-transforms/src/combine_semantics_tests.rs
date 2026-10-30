//! `combine` keeps what a function computes: each body below is run on a range
//! of inputs before and after the whole driver, and the answers must be the
//! same.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::interpret::{Val, run};
use llrm_mir::passes::Declared;

use crate::combine::combine;
use crate::testing::parsed;

/// `body` (an `i16` function of `%t`) on `0..=120` and a few negatives, before
/// and after `combine`.
fn same_answers(body: &str) {
    let text = format!("define i16 @f(i16 %t) {{\nb0:\n{body}}}\n");
    let before = parsed(&text);
    let mut after = before.clone();
    let globals = after.globals.clone();
    let metadata = after.metadata.clone();
    let mut declared = Declared::of(&after);
    let (context, function) = after.function_mut("f").expect("@f");
    combine(context, &globals, &Default::default(), &metadata, &DataLayout::default(), function, &mut declared);
    declared.place(&mut after).expect("placed");
    let answer = |module: &llrm_mir::module::Module, t: i128| {
        run(module, "f", vec![Val::Int { bits: (t as u128) & 0xffff, width: 16 }], 10_000)
            .unwrap_or_else(|trap| panic!("{trap:?}"))
    };
    for t in (0..=120).chain([-1, -49, -50, -51, -100, -32768, 32767]) {
        assert_eq!(answer(&after, t), answer(&before, t), "t = {t}:\n{}", llrm_mir::print::module(&after));
    }
}

/// easing.nib's `|t| t // 50 * 50`, a signed quotient scaled back, and the
/// slide's `start + (end - start) * f(t) // 100` after it: the step curve gave
/// 0 for every `t`, where the answer is 0, 0, 0, 20, 20, 40.
#[test]
fn a_quotient_scaled_back_keeps_its_value() {
    same_answers("  %q = sdiv i16 %t, 50\n  %m = mul i16 %q, 50\n  ret i16 %m\n");
    same_answers(
        "  %q = sdiv i16 %t, 50\n  %m = mul i16 %q, 50\n  %s = mul i16 %m, 40\n  %r = sdiv i16 %s, 100\n  ret i16 %r\n",
    );
}

/// The floored form the Nib frontend writes for `t // 50 * 50`.
const FLOOR_STEP: &str = "  %q = sdiv i16 %t, 50
  %r = srem i16 %t, 50
  %nz = icmp ne i16 %r, 0
  %a = zext i1 %nz to i8
  %x = xor i16 %r, 50
  %neg = icmp slt i16 %x, 0
  %b = zext i1 %neg to i8
  %c = and i8 %a, %b
  %d = zext i8 %c to i16
  %e = and i16 %d, 1
  %f = sub i16 %q, %e
  %m = mul i16 %f, 50
";

#[test]
fn a_floored_quotient_scaled_back_keeps_its_value() {
    same_answers(&format!("{FLOOR_STEP}  ret i16 %m\n"));
    same_answers(&format!(
        "{FLOOR_STEP}  %s = mul i16 %m, 40\n  %g = sdiv i16 %s, 100\n  %h = srem i16 %s, 100\n  %z = icmp ne i16 %h, 0\n  %za = zext i1 %z to i8\n  %y = xor i16 %h, 100\n  %n2 = icmp slt i16 %y, 0\n  %nb = zext i1 %n2 to i8\n  %w = and i8 %za, %nb\n  %v = zext i8 %w to i16\n  %u = and i16 %v, 1\n  %o = sub i16 %g, %u\n  ret i16 %o\n"
    ));
}

/// easing.nib after range narrowing: `lshr (zext i8 X), 15` became a poison
/// `lshr i8 X, 15`, and the step row printed 0 for every `t`.
#[test]
fn a_narrowed_quotient_scaled_back_keeps_its_value() {
    same_answers(
        "  %a = trunc i16 %t to i8
  %q = udiv i8 %a, 50
  %qz = zext i8 %q to i16
  %r = urem i8 %a, 50
  %rz = zext i8 %r to i16
  %s = lshr i16 %rz, 15
  %f = add nuw nsw i16 %s, %qz
  %m = mul nuw nsw i16 %f, 50
  ret i16 %m\n",
    );
}
