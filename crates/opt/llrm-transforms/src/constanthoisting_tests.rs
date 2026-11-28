//! An expensive constant used in two blocks, on real-mode x86 with the pass on
//! (`Tuning.constant_hoisting` is off there until the staging ends).

use super::ConstantHoisting;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

/// `text` through the pass on real-mode x86.
fn hoisted(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(ConstantHoisting);
    manager
        .run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos { constant_hoisting: true, ..Default::default() }))
        .unwrap();
    printed(&module)
}

/// An `i64` too wide for an immediate, read in both arms of a branch, was
/// materialized in each arm. It is one `bitcast` in the entry that both read,
/// and the other constant, 8 more, is an `add` of it.
#[test]
fn test_a_wide_constant_used_in_two_blocks_is_made_once() {
    let after = hoisted(
        "define i64 @f(i64 %a, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %x = xor i64 %a, 81985529216486895
  ret i64 %x

b2:
  %y = xor i64 %a, 81985529216486903
  ret i64 %y
}
",
    );
    let (entry, rest) = after.split_once("b1:").expect("a second block");
    assert!(entry.contains("%const = bitcast i64 81985529216486895 to i64"), "{after}");
    assert_eq!(after.matches("81985529216486895").count(), 1, "{after}");
    assert!(rest.contains("xor i64 %a, %const"), "{after}");
    assert!(rest.contains("add i64 %const, 8"), "{after}");
}

/// A constant used once is left as it is.
#[test]
fn test_a_wide_constant_used_once_is_left_alone() {
    let text = "define i64 @f(i64 %a) {
b0:
  %x = xor i64 %a, 81985529216486895
  ret i64 %x
}
";
    assert_eq!(hoisted(text), printed(&parsed(&format!("{HEAD}{text}"))));
}
