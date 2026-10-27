//! `llvm.is.constant` of an argument stayed in the function to the backend,
//! which refused it: `@_f: llvm.is.constant.i16`.

use llrm_analysis::testing::DOS;

use super::LowerConstantIntrinsics;
use crate::testing::{managed, parsed};

fn lowered(text: &str) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    managed(&mut module, LowerConstantIntrinsics)
}

/// An argument is not a constant: the call is false and the branch on it goes
/// to the slow block, which is the only one left.
#[test]
fn is_constant_of_an_argument_is_false_and_its_branch_is_taken() {
    let after = lowered(
        "declare i1 @llvm.is.constant.i16(i16)
define i16 @f(i16 %x) {
b0:
  %c = call i1 @llvm.is.constant.i16(i16 %x)
  br i1 %c, label %fast, label %slow
fast:
  ret i16 1
slow:
  ret i16 2
}
",
    );
    assert!(!after.contains("is.constant.i16(i16 %"), "{after}");
    assert!(!after.contains("ret i16 1"), "{after}");
    assert!(after.contains("ret i16 2"), "{after}");
}

/// A manifest constant is true.
#[test]
fn is_constant_of_a_constant_is_true() {
    let after = lowered(
        "declare i1 @llvm.is.constant.i16(i16)
define i16 @f() {
b0:
  %c = call i1 @llvm.is.constant.i16(i16 7)
  %z = zext i1 %c to i16
  ret i16 %z
}
",
    );
    assert!(after.contains("ret i16 1"), "{after}");
}
