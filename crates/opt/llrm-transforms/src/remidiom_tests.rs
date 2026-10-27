//! A remainder LSR wrote out as `x - (x / c) * c` is a remainder again where
//! the target keeps the divide.

use crate::remidiom::RemainderIdiom;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n\n";

fn after(
    text: &str,
    size: bool,
) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(RemainderIdiom { size });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// What LSR leaves of `trunc (urem i32 %x, 251)`: a 16-bit counter beside the
/// 32-bit one, the quotient narrowed.
const NARROWED: &str = "define void @f(ptr %p) {
entry:
  br label %loop

loop:
  %x = phi i32 [ 0, %entry ], [ %xn, %loop ]
  %n = phi i16 [ 0, %entry ], [ %nn, %loop ]
  %q = udiv i32 %x, 251
  %t = trunc i32 %q to i16
  %m = mul nuw i16 %t, 251
  %r = sub i16 %n, %m
  store i16 %r, ptr %p
  %xn = add nuw nsw i32 %x, 1
  %nn = add nuw i16 %n, 1
  %c = icmp ult i32 %xn, 40200
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
";

/// `huge.c`'s inner loop at m16 -Os ran `div`, `imul` and `sub` where main ran
/// one `idiv`: 13 instructions for 6, +723,000 clocks.
#[test]
fn test_the_narrowed_idiom_is_one_remainder_where_the_divide_is_kept() {
    let text = after(NARROWED, true);
    assert!(text.contains("urem i32 %x, 251"), "{text}");
    assert!(text.contains("trunc i32"), "{text}");
    assert!(!text.contains("udiv") && !text.contains("mul"), "{text}");
}

/// Not at speed: the quotient there is a multiply-high, cheaper than the
/// divide.
#[test]
fn test_a_build_for_speed_keeps_the_expansion_at_a_dword() {
    let text = after(NARROWED, false);
    assert!(text.contains("udiv i32 %x, 251") && !text.contains("urem"), "{text}");
}

/// A word has no multiply-high: its divide stays at every level.
#[test]
fn test_a_word_remainder_is_folded_at_any_level() {
    let text = after(
        "define i16 @f(i16 %x) {
entry:
  %q = udiv i16 %x, 7
  %m = mul i16 %q, 7
  %r = sub i16 %x, %m
  ret i16 %r
}
",
        false,
    );
    assert!(text.contains("urem i16 %x, 7") && !text.contains("udiv"), "{text}");
}

/// A quotient read elsewhere is a divide that is needed: a remainder beside it
/// would be a second.
#[test]
fn test_a_quotient_with_another_user_stays() {
    let text = after(
        "define i16 @f(i16 %x, ptr %p) {
entry:
  %q = udiv i16 %x, 7
  store i16 %q, ptr %p
  %m = mul i16 %q, 7
  %r = sub i16 %x, %m
  ret i16 %r
}
",
        true,
    );
    assert!(text.contains("udiv") && !text.contains("urem"), "{text}");
}

/// The counter must be the dividend (`trunc` of it): one that steps by two is
/// another number.
#[test]
fn test_a_counter_that_is_not_the_dividend_stays() {
    let text = after(&NARROWED.replace("%nn = add nuw i16 %n, 1", "%nn = add nuw i16 %n, 2"), true);
    assert!(text.contains("udiv") && !text.contains("urem"), "{text}");
}

/// The signed form.
#[test]
fn test_a_signed_remainder_is_folded_too() {
    let text = after(
        "define i16 @f(i16 %x) {
entry:
  %q = sdiv i16 %x, 10
  %m = mul i16 %q, 10
  %r = sub i16 %x, %m
  ret i16 %r
}
",
        true,
    );
    assert!(text.contains("srem i16 %x, 10") && !text.contains("sdiv"), "{text}");
}

/// `(i % 251) + 1`: LSR makes the counter `i + 1` and writes `(i + 1) - (i /
/// 251) * 251`.
const OFFSET: &str = "define void @f(ptr %p) {
entry:
  br label %loop

loop:
  %x = phi i32 [ 0, %entry ], [ %xn, %loop ]
  %y = phi i32 [ 1, %entry ], [ %yn, %loop ]
  %q = udiv i32 %x, 251
  %m = mul nuw nsw i32 %q, 251
  %r = sub i32 %y, %m
  store i32 %r, ptr %p
  %xn = add nuw nsw i32 %x, 1
  %yn = add nuw i32 %y, 1
  %c = icmp ult i32 %xn, 40200
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
";

/// `huge_runs.c`'s `around()` at m16 -Os kept `div; imul; sub` and two spills
/// in the loop where main ran `div; inc`: +19.5% clocks (+31.8 M). The counter
/// beside the dividend is `x + 1`, which is a remainder plus 1.
#[test]
fn test_a_counter_one_past_the_dividend_is_a_remainder_plus_one() {
    let text = after(OFFSET, true);
    assert!(text.contains("urem i32 %x, 251"), "{text}");
    assert!(text.contains("add i32 %") && text.contains(", 1"), "{text}");
    assert!(!text.contains("udiv") && !text.contains("mul"), "{text}");
}
