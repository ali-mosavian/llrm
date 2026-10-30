//! A switch on a narrow integer is a switch on its extension to the target's
//! width.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n\n";
const HEAD16: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";

/// `text` through `cgpaddr` with a 32-bit pointer layout and x86's lowering:
/// the text after.
fn widened(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// `_pl_select_weapon` compiled `cmp ax, 9` on a 16-bit index and a `movzx`
/// of it for the jump table where the pin does `movzwl; cmpl`: the pin
/// extends the switch condition first (weapons -Os).
#[test]
fn test_a_switch_on_an_i16_is_a_switch_on_its_zext_to_i32() {
    let after = widened(
        "define void @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 1, label %a
    i16 -3, label %a
  ]

a:
  ret void

d:
  ret void
}
",
    );
    assert!(after.contains("zext i16 %x to i32"), "{after}");
    assert!(
        after.contains("switch i32 %") && after.contains("i32 1, label %a") && after.contains("i32 65533"),
        "{after}"
    );
}

/// A `signext` argument is extended with `sext`, its negative cases sign
/// extended with it.
#[test]
fn test_a_signext_argument_is_widened_by_sext() {
    let after = widened(
        "define void @f(i8 signext %x) {
b0:
  switch i8 %x, label %d [
    i8 -1, label %a
  ]

a:
  ret void

d:
  ret void
}
",
    );
    assert!(after.contains("sext i8 %x to i32") && after.contains("i32 -1, label %a"), "{after}");
}

/// A switch already as wide as the target prefers is left alone.
#[test]
fn test_a_switch_on_an_i32_is_left_alone() {
    let text = "define void @f(i32 %x) {
b0:
  switch i32 %x, label %d [
    i32 1, label %a
  ]

a:
  ret void

d:
  ret void
}
";
    assert_eq!(widened(text), printed(&parsed(&format!("{HEAD}{text}"))));
}

/// The 16-bit target keeps the narrow switch: a 32-bit compare per case costs
/// an operand-size prefix there (queens, x_life grew 3 to 15 bytes at m16).
#[test]
fn test_a_switch_on_an_i16_stays_at_the_16_bit_target() {
    let text = "define void @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 1, label %a
  ]

a:
  ret void

d:
  ret void
}
";
    let mut module = parsed(&format!("{HEAD16}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    assert_eq!(printed(&module), printed(&parsed(&format!("{HEAD16}{text}"))));
}
