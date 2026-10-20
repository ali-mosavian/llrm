//! `combine` keeps what a function computes: each body below is run on a range of
//! inputs before and after the whole driver, and the answers must be the same.

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
    combine(context, &globals, &metadata, &DataLayout::default(), function, &mut declared);
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
    same_answers("  %q = sdiv i16 %t, 50\n  %m = mul i16 %q, 50\n  %s = mul i16 %m, 40\n  %r = sdiv i16 %s, 100\n  ret i16 %r\n");
}
