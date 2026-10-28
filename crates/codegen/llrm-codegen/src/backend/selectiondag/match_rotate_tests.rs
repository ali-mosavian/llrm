//! `MatchRotate` (DEVIATE SD321): an `or` of two shifts is a rotate or a
//! funnel shift.

use super::dag_function::selected;

fn names(body: &str) -> Vec<String> {
    let ir = format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{body}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

/// x_popcount's `rev32` step stayed `shl; shr; or` with three copies (six
/// instructions) where the pin emits one `shld`.
#[test]
fn an_or_of_a_shl_and_a_srl_of_two_values_is_a_funnel_shift() {
    let seen = names(
        "define i32 @f(i32 %x, i32 %y) {
  %a = shl i32 %x, 4
  %b = lshr i32 %y, 28
  %r = or i32 %a, %b
  ret i32 %r
}",
    );
    assert!(seen.iter().any(|name| name == "shld"), "{seen:?}");
    assert!(!seen.iter().any(|name| name == "shl" || name == "shr" || name == "or"), "{seen:?}");
}

#[test]
fn an_or_of_a_shl_and_a_srl_of_one_value_is_a_rotate() {
    let seen = names(
        "define i32 @f(i32 %x) {
  %a = shl i32 %x, 7
  %b = lshr i32 %x, 25
  %r = or i32 %a, %b
  ret i32 %r
}",
    );
    assert!(seen.iter().any(|name| name == "rol"), "{seen:?}");
}

/// The amounts must add to the width: `4 + 27` is two shifts and an `or`.
#[test]
fn shifts_that_do_not_add_to_the_width_stay_shifts() {
    let seen = names(
        "define i32 @f(i32 %x, i32 %y) {
  %a = shl i32 %x, 4
  %b = lshr i32 %y, 27
  %r = or i32 %a, %b
  ret i32 %r
}",
    );
    assert!(!seen.iter().any(|name| name == "shld" || name == "rol"), "{seen:?}");
}

/// A constant `and` on a half moves to the result (`ApplyMasks`).
#[test]
fn a_masked_half_of_a_funnel_shift_masks_the_result() {
    let seen = names(
        "define i32 @f(i32 %x, i32 %y) {
  %a = shl i32 %x, 4
  %m = and i32 %a, 48
  %b = lshr i32 %y, 28
  %r = or i32 %m, %b
  ret i32 %r
}",
    );
    assert!(seen.iter().any(|name| name == "shld"), "{seen:?}");
    assert!(seen.iter().any(|name| name == "and"), "{seen:?}");
}
