//! A web of phis between loads and bitcasts is made the type of the bitcasts,
//! on a target that says so.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const ARM: &str = "target datalayout = \"e-m:e-i64:64-i128:128-n32:64-S128\"\n\n";
const DOS: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";

/// A phi of two loaded integers, read as a float.
const WEB: &str = "define float @f(i1 %c, ptr %p, ptr %q) {
b0:
  br i1 %c, label %a, label %b

a:
  %x = load i32, ptr %p, align 4
  br label %j

b:
  %y = load i32, ptr %q, align 4
  br label %j

j:
  %m = phi i32 [ %x, %a ], [ %y, %b ]
  %r = bitcast i32 %m to float
  ret float %r
}
";

fn after(
    head: &str,
    text: &str,
    machine: std::rc::Rc<dyn llrm_mir::target::Machine>,
) -> String {
    let mut module = parsed(&format!("{head}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, machine).unwrap();
    printed(&module)
}

/// Both loads read a float, the phi is a float, no bitcast reads the phi.
/// Before the step, `%m` stayed an `i32` phi and each value crossed from the
/// integer to the float registers after the join.
#[test]
fn a_phi_of_loads_read_as_a_float_is_a_float_phi() {
    let text = after(
        ARM,
        WEB,
        std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
    );
    assert!(text.contains("phi float"), "{text}");
    assert!(!text.contains("phi i32"), "{text}");
    assert_eq!(text.matches("bitcast i32").count(), 2, "one per load: {text}");
}

/// A target that answers no keeps the integer phi: x86 below 64 bits
/// (X86ISelLowering.cpp L36785), here real-mode DOS.
#[test]
fn a_target_that_says_no_keeps_the_integer_phi() {
    let text = after(DOS, WEB, std::rc::Rc::new(llrm_x86_m16::Dos::default()));
    assert!(text.contains("phi i32"), "{text}");
}

const ARM_MACHINE: fn() -> std::rc::Rc<dyn llrm_mir::target::Machine> =
    || std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux });

/// A constant arm of the web is the same bits in the new type.
#[test]
fn a_constant_arm_becomes_a_constant_of_the_new_type() {
    let text = after(
        ARM,
        "define float @f(i1 %c, ptr %p) {
b0:
  br i1 %c, label %a, label %j

a:
  %x = load i32, ptr %p, align 4
  br label %j

j:
  %m = phi i32 [ %x, %a ], [ 1065353216, %b0 ]
  %r = bitcast i32 %m to float
  ret float %r
}
",
        ARM_MACHINE(),
    );
    assert!(text.contains("phi float"), "{text}");
    assert!(!text.contains("phi i32"), "{text}");
}

/// A volatile load in the web keeps it as it is (`isSimple`).
#[test]
fn a_volatile_load_keeps_the_integer_phi() {
    let text = after(
        ARM,
        "define float @f(i1 %c, ptr %p, ptr %q) {
b0:
  br i1 %c, label %a, label %b

a:
  %x = load volatile i32, ptr %p, align 4
  br label %j

b:
  %y = load i32, ptr %q, align 4
  br label %j

j:
  %m = phi i32 [ %x, %a ], [ %y, %b ]
  %r = bitcast i32 %m to float
  ret float %r
}
",
        ARM_MACHINE(),
    );
    assert!(text.contains("phi i32"), "{text}");
}

/// Loads, bitcasts and stores only (no bitcast away from a load and into
/// something else) have no anchor: converting would move the bitcasts, not
/// remove them.
#[test]
fn a_web_between_loads_and_stores_has_no_anchor_and_stays() {
    let text = after(
        ARM,
        "define void @f(i1 %c, ptr %p, ptr %q, ptr %o) {
b0:
  br i1 %c, label %a, label %b

a:
  %x = load float, ptr %p, align 4
  %xi = bitcast float %x to i32
  br label %j

b:
  %y = load float, ptr %q, align 4
  %yi = bitcast float %y to i32
  br label %j

j:
  %m = phi i32 [ %xi, %a ], [ %yi, %b ]
  store i32 %m, ptr %o, align 4
  ret void
}
",
        ARM_MACHINE(),
    );
    assert!(text.contains("phi i32"), "{text}");
}
