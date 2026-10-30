//! A store of two values merged into one wider integer is two stores where the
//! target says so.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";

fn after(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// `store (zext(bitcast f) | zext(i) << 32)` was a shift, two extensions, an
/// `or` and a store of eight bytes through the float unit and back; a float
/// and an integer are two stores.
#[test]
fn test_a_float_and_an_integer_merged_into_a_store_are_two_stores() {
    let text = after(
        "define void @f(float %fl, i32 %i, ptr %p) {
b0:
  %b = bitcast float %fl to i32
  %lz = zext i32 %b to i64
  %hz = zext i32 %i to i64
  %hs = shl i64 %hz, 32
  %o = or i64 %lz, %hs
  store i64 %o, ptr %p, align 8
  ret void
}
",
    );
    assert!(!text.contains("store i64"), "{text}");
    assert!(text.contains("store i32 %b, ptr %p, align 8"), "{text}");
    assert!(text.contains("getelementptr i32, ptr %p, i32 1"), "{text}");
    assert!(text.contains(", align 4"), "{text}");
}

/// The operands of the `or` in the other order are the same pattern.
#[test]
fn test_the_or_is_commutative() {
    let text = after(
        "define void @f(float %fl, i32 %i, ptr %p) {
b0:
  %b = bitcast float %fl to i32
  %lz = zext i32 %b to i64
  %hz = zext i32 %i to i64
  %hs = shl i64 %hz, 32
  %o = or i64 %hs, %lz
  store i64 %o, ptr %p, align 8
  ret void
}
",
    );
    assert!(!text.contains("store i64"), "{text}");
}

/// Two integers are left merged: the pin leaves such a pair out until a test
/// proves a win.
#[test]
fn test_two_integers_stay_merged() {
    let text = after(
        "define void @f(i32 %a, i32 %i, ptr %p) {
b0:
  %lz = zext i32 %a to i64
  %hz = zext i32 %i to i64
  %hs = shl i64 %hz, 32
  %o = or i64 %lz, %hs
  store i64 %o, ptr %p, align 8
  ret void
}
",
    );
    assert!(text.contains("store i64"), "{text}");
}

/// A volatile store, or an operand read twice, is not split.
#[test]
fn test_a_volatile_store_and_a_shared_operand_stay() {
    let volatile = after(
        "define void @f(float %fl, i32 %i, ptr %p) {
b0:
  %b = bitcast float %fl to i32
  %lz = zext i32 %b to i64
  %hz = zext i32 %i to i64
  %hs = shl i64 %hz, 32
  %o = or i64 %lz, %hs
  store volatile i64 %o, ptr %p, align 8
  ret void
}
",
    );
    assert!(volatile.contains("store volatile i64"), "{volatile}");
    let shared = after(
        "define i64 @f(float %fl, i32 %i, ptr %p) {
b0:
  %b = bitcast float %fl to i32
  %lz = zext i32 %b to i64
  %hz = zext i32 %i to i64
  %hs = shl i64 %hz, 32
  %o = or i64 %lz, %hs
  store i64 %o, ptr %p, align 8
  ret i64 %lz
}
",
    );
    assert!(shared.contains("store i64"), "{shared}");
}
