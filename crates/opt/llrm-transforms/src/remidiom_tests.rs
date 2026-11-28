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

/// `(i * 7) % 5 == 0`: LSR keeps `-x` as a counter and writes `(x / 5) * 5 +
/// -x`. At -Os that was `div; lea; add` with a spare counter and three
/// spills in `x_life`'s fill loop (+22 B) where `div; test edx` is enough.
const NEGATED: &str = "define void @f(ptr %p) {
entry:
  br label %loop

loop:
  %x = phi i32 [ 0, %entry ], [ %xn, %loop ]
  %y = phi i32 [ 0, %entry ], [ %yn, %loop ]
  %q = udiv i32 %x, 5
  %m = mul nuw i32 %q, 5
  %r = add i32 %y, %m
  %z = icmp eq i32 %r, 0
  %b = zext i1 %z to i8
  store i8 %b, ptr %p
  %xn = add nuw nsw i32 %x, 7
  %yn = add i32 %y, -7
  %c = icmp ult i32 %xn, 40200
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
";

#[test]
fn test_a_negated_counter_is_the_negated_remainder() {
    let text = after(NEGATED, true);
    assert!(text.contains("urem i32 %x, 5"), "{text}");
    assert!(text.contains("sub i32 0, %"), "{text}");
    assert!(!text.contains("udiv") && !text.contains("mul"), "{text}");
    assert!(!text.contains("%y"), "the dead -x counter is gone: {text}");
}

/// What LSR left of `i % 97 + 2` in recpow's loop at m64 -Os: the counter `%n`
/// (= `%x` + 40) is added after `3 - q * 97`, so the subtrahend's left operand
/// is the constant 3. The divide, multiply and subtract stayed (`div`, `imul`,
/// `neg`, `lea` where gcc has `idiv` and `add edx, 2`): recpow -Os
/// 209 B against gcc's 175.
const SHIFTED: &str = "define void @f(ptr %p) {
entry:
  br label %loop

loop:
  %x = phi i32 [ 1, %entry ], [ %xn, %loop ]
  %n = phi i32 [ 41, %entry ], [ %nn, %loop ]
  %q = udiv i32 %x, 97
  %m = mul nuw nsw i32 %q, 97
  %s = sub i32 3, %m
  %t = add i32 %n, %s
  %r = add i32 %t, -41
  store i32 %r, ptr %p
  %xn = add nuw i32 %x, 1
  %nn = add nuw i32 %n, 1
  %c = icmp ult i32 %xn, 400
  br i1 %c, label %loop, label %exit

exit:
  ret void
}
";

#[test]
fn test_a_counter_added_after_a_constant_minus_the_product_is_one_remainder() {
    let text = after(SHIFTED, true);
    assert!(text.contains("urem i32 %x, 97"), "{text}");
    assert!(!text.contains("udiv") && !text.contains("mul"), "{text}");
    assert!(text.contains(", 43"), "k = 40 (the counter) + 3: {text}");
}

/// A constant that is not added to a counter of the dividend is another number.
#[test]
fn test_a_constant_minus_the_product_added_to_an_unrelated_value_stays() {
    let text = after(&SHIFTED.replace("%nn = add nuw i32 %n, 1", "%nn = add nuw i32 %n, 2"), true);
    assert!(text.contains("udiv") && !text.contains("urem"), "{text}");
}
