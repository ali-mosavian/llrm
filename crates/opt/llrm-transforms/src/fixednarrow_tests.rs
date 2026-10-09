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

/// Every product went through the one-operand `imul` and `shrd`, EAX and EDX held, though
/// the operands' product fits 32 bits.
#[test]
fn a_fixed_product_the_operands_keep_in_32_bits_is_a_mul_and_a_shift() {
    let module = narrowed(PRODUCTS);
    let small = body(&module, "small");
    assert!(small.contains("mul i32 %a, %b") && small.contains("ashr i32") && !small.contains("smul.fix"), "{small}");
}

/// `smul.fix(65536, 65536, 16)` is 65536, which fits; its product is 2^32, which does not.
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

define i32 @wrapped(i32 range(i32 -1500000000, 1500000001) %a, i32 range(i32 0, 2) %b) {
b0:
  %t = add i32 %a, %a
  %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
  ret i32 %p
}
";

/// `x * 2 * y` in Q8 cost an `add` and a `sar` by 8 where one `sar` by 7 is the same value.
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

/// A fixed-point integer made by `<< 8` and scaled back by 24/256 was an `imul`, `shrd` and
/// the shift; it is `a * 24`.
#[test]
fn a_shift_by_the_scale_makes_the_product_whole() {
    let module = narrowed(SHIFTED);
    let whole = body(&module, "whole");
    assert!(whole.contains("mul i32 %a, 24") && !whole.contains("ashr") && !whole.contains("smul.fix"), "{whole}");
}

/// `a + a` wraps for `a` near 2^30: the factor stays, the product keeps its wide form.
#[test]
fn a_doubling_that_may_wrap_is_not_taken_out() {
    let module = narrowed(SHIFTED);
    assert!(body(&module, "wrapped").contains("smul.fix"));
}
