//! An extension moves up through its operand, towards the load.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-n8:16:32\"\n\n";

/// `text` through the pass on the real-mode target: the text after.
fn after(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// `sext (add nsw (load i8), 1)` read in the next block: the sign extension
/// goes up through the add to the load, where the pair is one extending load,
/// and the add is made in the wide type.
#[test]
fn test_a_sign_extended_sum_of_a_load_is_promoted_to_an_extending_load() {
    let text = after(
        "define i32 @f(ptr %p) {
b0:
  %a = load i8, ptr %p
  br label %b1

b1:
  %b = add nsw i8 %a, 1
  %e = sext i8 %b to i32
  ret i32 %e
}
",
    );
    let (entry, rest) = text.split_once("b1:").expect("a second block");
    assert!(entry.contains("sext i8 %a to i32"), "the extension follows the load: {text}");
    assert!(rest.contains("add nsw i32"), "the sum is made wide: {text}");
    assert!(!rest.contains("add nsw i8"), "{text}");
}

/// A sum with no `nsw` is not moved through by a sign extension: the wide sum
/// would not wrap where the narrow one does.
#[test]
fn test_a_sign_extended_wrapping_sum_stays_narrow() {
    let source = "define i32 @f(ptr %p) {
b0:
  %a = load i8, ptr %p
  br label %b1

b1:
  %b = add i8 %a, 1
  %e = sext i8 %b to i32
  ret i32 %e
}
";
    assert_eq!(after(source), printed(&parsed(&format!("{HEAD}{source}"))));
}

/// A zero extension goes through a logical shift right (the bits come from
/// the top), which a sign extension does not.
#[test]
fn test_a_zero_extended_shift_right_of_a_load_is_promoted() {
    let text = after(
        "define i32 @f(ptr %p) {
b0:
  %a = load i8, ptr %p
  br label %b1

b1:
  %b = lshr i8 %a, 1
  %e = zext i8 %b to i32
  ret i32 %e
}
",
    );
    let (entry, rest) = text.split_once("b1:").expect("a second block");
    assert!(entry.contains("zext i8 %a to i32") && rest.contains("lshr i32"), "{text}");
}
