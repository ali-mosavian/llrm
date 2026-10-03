//! A phi of one address computation, read as an address, is that computation
//! in the join; each case's fixture is the pass's input.

use super::AddressSink;
use crate::testing::{managed, parsed, results};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32\"\n\n@g = global [64 x i16] zeroinitializer\n\n";

/// `text` through the pass, answering as before on `inputs`: the text after.
fn sunk(text: &str, inputs: &[&[i128]]) -> String {
    let original = parsed(&format!("{HEAD}{text}"));
    let mut module = original.clone();
    let after = managed(&mut module, AddressSink);
    assert_eq!(results(&module, inputs), results(&original, inputs), "{after}");
    after
}

/// `a(i) = ...` under an `IF` computes its address in each arm; the join's
/// store read a phi of three equal addresses, held in a register, where one
/// addressing form would do: RGBLIGHTS' loop stored through
/// `mov si,bx; add si,offset FSIN4%; add si,1640` (#386).
#[test]
fn test_a_phi_of_one_address_in_every_arm_is_that_address_in_the_join() {
    let text = "define void @f(i16 %i, i16 %c, i16 %d) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %k1 = sub i16 %i, -3
  %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
  store i16 0, ptr %p1
  br label %b5

b2:
  %mid = icmp sgt i16 %c, %d
  br i1 %mid, label %b3, label %b4

b3:
  %k3 = sub i16 %i, -3
  %p3 = getelementptr inbounds i16, ptr @g, i16 %k3
  store i16 1, ptr %p3
  br label %b5

b4:
  %k4 = sub i16 %i, -3
  %p4 = getelementptr inbounds i16, ptr @g, i16 %k4
  store i16 2, ptr %p4
  br label %b5

b5:
  %p = phi ptr [ %p1, %b1 ], [ %p3, %b3 ], [ %p4, %b4 ]
  store i16 %c, ptr %p
  ret void
}
";
    let after = sunk(text, &[&[1, -1, 2], &[4, 5, 2], &[6, 1, 9]]);
    assert!(!after.contains("phi"), "{after}");
}

/// A phi whose value is more than an address, here also added to a number,
/// stays: gvn made it so that the arms' values are not computed twice.
#[test]
fn test_a_phi_read_as_a_value_stays() {
    let text = "define i16 @f(i16 %i, i16 %c) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %p1 = getelementptr inbounds i16, ptr @g, i16 %i
  br label %b3

b2:
  %p2 = getelementptr inbounds i16, ptr @g, i16 %i
  br label %b3

b3:
  %p = phi ptr [ %p1, %b1 ], [ %p2, %b2 ]
  %n = ptrtoint ptr %p to i16
  ret i16 %n
}
";
    assert!(sunk(text, &[&[1, 1], &[2, -1]]).contains("phi"));
}

/// Arms that compute different addresses stay a phi.
#[test]
fn test_a_phi_of_different_addresses_stays() {
    let text = "define void @f(i16 %i, i16 %c) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %p1 = getelementptr inbounds i16, ptr @g, i16 %i
  br label %b3

b2:
  %j = add i16 %i, 1
  %p2 = getelementptr inbounds i16, ptr @g, i16 %j
  br label %b3

b3:
  %p = phi ptr [ %p1, %b1 ], [ %p2, %b2 ]
  store i16 %c, ptr %p
  ret void
}
";
    assert!(sunk(text, &[&[1, 1], &[2, -1]]).contains("phi"));
}
