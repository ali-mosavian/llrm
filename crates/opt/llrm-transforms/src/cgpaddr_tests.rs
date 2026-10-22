//! An address made in another block is made where it is used.

use super::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32\"\n\n";

/// `text` through the pass on the real-mode target: the text after.
fn sunk(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// The store's block read `a[i]` through a `gep` made in the block before:
/// one register held across the edge, where the access takes the base and the
/// scaled index itself. The pin's `optimizeMemoryInst` makes the address in the
/// store's block.
#[test]
fn test_an_address_made_in_another_block_is_made_where_it_is_used() {
    let after = sunk(
        "define void @f(ptr %a, i16 %i) {
b0:
  %p = getelementptr i32, ptr %a, i16 %i
  br label %b1

b1:
  store i32 0, ptr %p
  ret void
}
",
    );
    let (entry, rest) = after.split_once("b1:").expect("a second block");
    assert!(!entry.contains("getelementptr"), "the address is not made in b0: {after}");
    assert!(rest.contains("mul i16 %i, 4") && rest.contains("getelementptr i8, ptr %a"), "{after}");
}

/// An address made in the access's block stays as it is.
#[test]
fn test_an_address_made_in_the_accesss_block_is_left_alone() {
    let text = "define void @f(ptr %a, i16 %i) {
b0:
  %p = getelementptr i32, ptr %a, i16 %i
  store i32 0, ptr %p
  ret void
}
";
    assert_eq!(sunk(text), printed(&parsed(&format!("{HEAD}{text}"))));
}

/// A constant offset behind a variable index is one `gep` of the constant.
#[test]
fn test_a_field_of_an_element_is_a_scaled_index_and_a_constant() {
    let after = sunk(
        "define void @f(ptr %a, i16 %i) {
b0:
  %e = getelementptr [4 x i16], ptr %a, i16 %i
  %p = getelementptr [4 x i16], ptr %e, i16 0, i16 3
  br label %b1

b1:
  store i16 0, ptr %p
  ret void
}
",
    );
    let (_, rest) = after.split_once("b1:").expect("a second block");
    assert!(rest.contains("mul i16 %i, 8") && rest.contains("i16 6"), "{after}");
}
