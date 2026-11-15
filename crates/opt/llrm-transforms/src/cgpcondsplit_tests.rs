use super::{Prices, conditions_split};
use crate::testing::{parsed, printed};

/// x87 at 486: `fadd` 8, a dword load 3, a branch 3.
const PRICES: Prices = Prices { float_add: 8, float_multiply: 16, float_divide: 73, load: 1, float_load: 3, branch: 3 };

/// sfm16_dbg: the sum is made before `if (e != 255 && bits(a + b) != want)`.
const BOTH: &str = "define i32 @f(ptr %p) {
b0:
  %x = load float, ptr %p
  %q = getelementptr i8, ptr %p, i32 4
  %y = load float, ptr %q
  %s = fadd float %x, %y
  %u = bitcast float %s to i32
  %r = getelementptr i8, ptr %p, i32 8
  %w = load i32, ptr %r
  %e = and i32 %w, 2139095040
  %skip = icmp eq i32 %e, 2139095040
  %same = icmp eq i32 %u, %w
  %c = select i1 %skip, i1 true, i1 %same
  br i1 %c, label %t, label %f
t:
  ret i32 0
f:
  ret i32 1
}
";

fn after(
    text: &str,
    prices: Prices,
) -> (bool, String) {
    let mut module = parsed(text);
    let llrm_mir::module::Module { context, globals, .. } = &mut module;
    let llrm_mir::module::GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    let changed = conditions_split(context, function, prices);
    (changed, printed(&module))
}

/// The `fld`, `fadd` and `fstp` ran on every trip though the exponent test
/// decided 16 of 400: 400 `fadd` against gcc's 384, +291 clocks at -O1.
#[test]
fn test_the_sum_is_made_after_the_test_that_may_skip_it() {
    let (changed, text) = after(BOTH, PRICES);
    assert!(changed, "{text}");
    assert!(!text.contains("select"), "{text}");
    let first = text.find("br i1 %skip").expect("the first branch");
    for what in ["fadd float", "bitcast float", "icmp eq i32 %u"] {
        assert!(text.find(what).expect(what) > first, "{what} after the first branch: {text}");
    }
    assert!(text.find("icmp eq i32 %e").expect("the first test") < first, "{text}");
}

/// Control: where the cone is as cheap as a branch, the select stays.
#[test]
fn test_a_cheap_second_test_keeps_the_select() {
    let (changed, text) =
        after(BOTH, Prices { float_add: 1, float_multiply: 1, float_divide: 1, load: 1, float_load: 1, ..PRICES });
    assert!(!changed, "{text}");
    assert!(text.contains("select i1 %skip"), "{text}");
}

/// Control: a store between a load and the split may write what the load
/// reads, so the load stays in front.
#[test]
fn test_a_load_a_store_may_reach_stays_in_the_first_block() {
    let (_, text) = after(&BOTH.replace("  %c = select", "  store i32 0, ptr %p\n  %c = select"), PRICES);
    let first = text.find("br i1 %skip").expect("the first branch");
    assert!(text.find("load float").expect("the load") < first, "{text}");
}

/// The `&&` form: `a && b` is `select a, b, false`.
#[test]
fn test_an_and_is_split_the_same_way() {
    let (changed, text) = after(
        &BOTH
            .replace("%skip = icmp eq", "%skip = icmp ne")
            .replace("select i1 %skip, i1 true, i1 %same", "select i1 %skip, i1 %same, i1 false"),
        PRICES,
    );
    assert!(changed, "{text}");
    assert!(text.find("fadd float").unwrap() > text.find("br i1 %skip").unwrap(), "{text}");
}
