use std::rc::Rc;

use llrm_mir::passes::PassManager;

use super::Ports;
use crate::testing::{parsed, printed};

/// The DAC's write index, the 8237's mode register, a port the branches
/// bound to the VGA's, and one nothing bounds: each call's line, in order.
const CALLS: &str = "declare void @llrm.ia16.out.i8(i16, i8)

define void @f(i16 %p, i8 %v) {
b0:
  call void @llrm.ia16.out.i8(i16 968, i8 %v)
  call void @llrm.ia16.out.i8(i16 11, i8 %v)
  %low = icmp sge i16 %p, 960
  br i1 %low, label %b1, label %b3
b1:
  %high = icmp sle i16 %p, 975
  br i1 %high, label %b2, label %b3
b2:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v)
  br label %b3
b3:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v)
  ret void
}
";

/// Every OUT counted as reading memory and writing none, so a store before
/// one that starts DMA could be dropped and a load after it kept.
#[test]
fn test_a_port_call_is_narrowed_only_where_the_target_says_its_device_reaches_no_memory() {
    let mut module = parsed(CALLS);
    let mut manager = PassManager::default();
    manager.add(Ports);
    manager.run_module(&mut module, Rc::new(llrm_x86_code16::Dos::default())).unwrap();
    let narrowed: Vec<bool> =
        printed(&module).lines().filter(|line| line.contains("call void")).map(|line| line.ends_with("memory(inaccessiblemem: readwrite)")).collect();
    assert_eq!(narrowed, [true, false, true, false]);
}
