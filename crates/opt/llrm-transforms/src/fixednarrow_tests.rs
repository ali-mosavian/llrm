use std::rc::Rc;

use llrm_mir::interpret::{Val, run};
use llrm_mir::passes::PassManager;

use super::FixedNarrow;
use crate::testing::{parsed, printed};

const PRODUCTS: &str = "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define i32 @small(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
b0:
  %p = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 8)
  ret i32 %p
}

define i32 @wide(i32 range(i32 0, 65537) %a, i32 range(i32 0, 65537) %b) {
b0:
  %p = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 16)
  ret i32 %p
}

define i32 @unknown(i32 %a, i32 %b) {
b0:
  %p = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 8)
  ret i32 %p
}
";

fn narrowed(text: &str) -> llrm_mir::module::Module {
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.add(FixedNarrow);
    manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    module
}

fn body(
    module: &llrm_mir::module::Module,
    name: &str,
) -> String {
    let text = printed(module);
    let start = text.find(&format!("@{name}(")).expect("the function");
    text[start..].split("\n}\n").next().unwrap().to_owned()
}

/// Every product went through the one-operand `imul` and `shrd`, EAX and EDX
/// held, though the operands' product fits 32 bits.
#[test]
fn a_fixed_product_the_operands_keep_in_32_bits_is_a_mul_and_a_shift() {
    let module = narrowed(PRODUCTS);
    let small = body(&module, "small");
    assert!(small.contains("mul i32 %a, %b") && small.contains("ashr i32") && !small.contains("smul.fix"), "{small}");
}

/// `smul.fix(65536, 65536, 16)` is 65536, which fits; its product is 2^32,
/// which does not.
#[test]
fn a_product_that_overflows_32_bits_keeps_its_wide_form_though_the_result_fits() {
    let module = narrowed(PRODUCTS);
    assert!(body(&module, "wide").contains("smul.fix"));
    assert!(body(&module, "unknown").contains("smul.fix"));
}

/// The floor: -1 * 1 >> 8 is -1, not 0.
#[test]
fn the_narrow_form_floors_as_the_wide_one_does() {
    let module = narrowed(PRODUCTS);
    for (a, b) in [(-1_i64, 1_i64), (1, -1), (-1000, 1000), (255, 1), (-255, 1), (0, 0)] {
        let args = [a, b].map(|n| Val::Int { bits: n as u128 & 0xFFFF_FFFF, width: 32 }).to_vec();
        let Ok(Val::Int { bits, .. }) = run(&module, "small", args, 1000) else { panic!() };
        assert_eq!(bits as u32 as i32 as i64, (a * b) >> 8, "{a} {b}");
    }
}

const SHIFTED: &str = "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
b0:
  %t = add i32 %a, %a
  %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
  ret i32 %p
}

define i32 @whole(i32 range(i32 -16, 16) %a) {
b0:
  %t = shl i32 %a, 8
  %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 24, i32 8)
  ret i32 %p
}

define i32 @wrapped(i32 range(i32 -1500000000, 1500000001) %a, i32 range(i32 0, 4) %b) {
b0:
  %t = add i32 %a, %a
  %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
  ret i32 %p
}
";

/// `x * 2 * y` in Q8 cost an `add` and a `sar` by 8 where one `sar` by 7 is the
/// same value.
#[test]
fn a_doubled_factor_leaves_the_product_for_the_scale() {
    let module = narrowed(SHIFTED);
    let twice = body(&module, "twice");
    assert!(
        twice.contains("mul i32 %a, %b")
            && twice.contains("ashr i32 %")
            && twice.contains(", 7")
            && !twice.contains("smul.fix"),
        "{twice}"
    );
    for (a, b) in [(-1_i64, 1_i64), (3, -5), (-1000, 1000), (255, 1), (0, 7)] {
        let args = [a, b].map(|n| Val::Int { bits: n as u128 & 0xFFFF_FFFF, width: 32 }).to_vec();
        let Ok(Val::Int { bits, .. }) = run(&module, "twice", args, 1000) else { panic!() };
        assert_eq!(bits as u32 as i32 as i64, (2 * a * b) >> 8, "{a} {b}");
    }
}

/// A fixed-point integer made by `<< 8` and scaled back by 24/256 was an
/// `imul`, `shrd` and the shift; it is `a * 24`.
#[test]
fn a_shift_by_the_scale_makes_the_product_whole() {
    let module = narrowed(SHIFTED);
    let whole = body(&module, "whole");
    assert!(whole.contains("mul i32 %a, 24") && !whole.contains("ashr") && !whole.contains("smul.fix"), "{whole}");
}

/// `a + a` wraps for `a` near 2^30: the factor stays, the product keeps its
/// wide form.
#[test]
fn a_doubling_that_may_wrap_is_not_taken_out() {
    let module = narrowed(SHIFTED);
    assert!(body(&module, "wrapped").contains("smul.fix"));
}

/// A factor that may wrap has the full range, but times a 0 or 1 the product
/// still fits: LazyValueInfo's ranges narrow it, where the old intervals gave
/// up on the wrapped factor.
#[test]
fn a_wrapped_factor_times_a_bit_is_still_narrowed() {
    let module = narrowed(
        "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define i32 @bit(i32 %t, i32 range(i32 0, 2) %b) {
b0:
  %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
  ret i32 %p
}
",
    );
    assert!(!body(&module, "bit").contains("smul.fix"));
    for (t, b) in [(i32::MIN, 1_i64), (i32::MAX, 1), (-77, 0)] {
        let args = [i64::from(t), b].map(|n| Val::Int { bits: n as u128 & 0xFFFF_FFFF, width: 32 }).to_vec();
        let Ok(Val::Int { bits, .. }) = run(&module, "bit", args, 1000) else { panic!() };
        assert_eq!(bits as u32 as i32 as i64, (i64::from(t) * b) >> 8, "{t} {b}");
    }
}

/// A product read by the next product: both narrowed, the second one's operand
/// the first's new shift. The operands were taken before any rewrite, so the
/// second `mul` read the first product's erased result (examples/mandel at
/// -O2: "mul uses the result of an erased instruction").
#[test]
fn a_product_of_a_product_reads_the_first_ones_replacement() {
    let text = "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define i32 @chain(i32 range(i32 -100, 101) %a, i32 range(i32 -100, 101) %b) {
b0:
  %p = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 8)
  %q = call i32 @llvm.smul.fix.i32(i32 %p, i32 %b, i32 8)
  ret i32 %q
}
";
    let module = narrowed(text);
    let chain = body(&module, "chain");
    assert!(!chain.contains("smul.fix"), "{chain}");
    assert!(llrm_mir::verify::verify(&module).is_empty(), "{:?}\n{chain}", llrm_mir::verify::verify(&module));
    for (a, b) in [(100_i64, 100_i64), (-100, 77), (13, -5)] {
        let args = [a, b].map(|n| Val::Int { bits: n as u128 & 0xFFFF_FFFF, width: 32 }).to_vec();
        let Ok(Val::Int { bits, .. }) = run(&module, "chain", args, 1000) else { panic!() };
        assert_eq!(bits as u32 as i32 as i64, ((((a * b) >> 8) * b) >> 8), "{a} {b}");
    }
}

/// examples/mandel: `x*x + y*y > 4` leaves the loop, and the factors are
/// bounded by nothing else; the offsets come from the counters of the loops
/// around it.
const ESCAPE: &str = "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define i32 @mandel() {
b0:
  br label %o

o:
  %px = phi i16 [ -16, %b0 ], [ %px1, %out ]
  %p = sext i16 %px to i32
  %cx = mul i32 %p, 24
  %cy = mul i32 %p, 3
  br label %h

h:
  %i = phi i32 [ 0, %o ], [ %i1, %c ]
  %x = phi i32 [ 0, %o ], [ %x1, %c ]
  %y = phi i32 [ 0, %o ], [ %y1, %c ]
  %xx = call i32 @llvm.smul.fix.i32(i32 %x, i32 %x, i32 8)
  %yy = call i32 @llvm.smul.fix.i32(i32 %y, i32 %y, i32 8)
  %s = add i32 %xx, %yy
  %g = icmp sgt i32 %s, 1024
  br i1 %g, label %out, label %c

c:
  %x2 = add i32 %x, %x
  %xy = call i32 @llvm.smul.fix.i32(i32 %x2, i32 %y, i32 8)
  %y1 = add i32 %xy, %cy
  %d = sub i32 %xx, %yy
  %x1 = add i32 %d, %cx
  %i1 = add nuw nsw i32 %i, 1
  %e = icmp eq i32 %i1, 32
  br i1 %e, label %out, label %h

out:
  %n = phi i32 [ %i, %h ], [ %i1, %c ]
  %px1 = add nsw i16 %px, 1
  %ec = icmp eq i16 %px1, 16
  br i1 %ec, label %done, label %o

done:
  ret i32 %n
}
";

/// mandel.nib at -O2 -m16 kept three `imul` + `shrd` pairs a trip (main: none,
/// 50 instructions against 70): the old `ranges` engine bounded `x` and `y`
/// by the escape test, LazyValueInfo does not.
#[test]
fn factors_bounded_by_the_escape_test_are_narrowed() {
    let module = narrowed(ESCAPE);
    let text = body(&module, "mandel");
    assert!(!text.contains("smul.fix"), "{text}");
    // The bounds are proofs: the loop returns what it returned.
    let before = run(&parsed(ESCAPE), "mandel", vec![], 10_000_000).expect("runs");
    let after = run(&module, "mandel", vec![], 10_000_000).expect("runs");
    assert_eq!(format!("{before:?}"), format!("{after:?}"));
}
