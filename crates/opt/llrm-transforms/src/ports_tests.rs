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
    manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    let narrowed: Vec<bool> = printed(&module)
        .lines()
        .filter(|line| line.contains("call void"))
        .map(|line| line.ends_with("memory(inaccessiblemem: readwrite)"))
        .collect();
    assert_eq!(narrowed, [true, false, true, false]);
}

/// A pass that asks the manager what the counted loops bound, and changes
/// nothing.
struct AsksBounds;

impl llrm_mir::passes::FunctionPass for AsksBounds {
    fn name(&self) -> &'static str {
        "asks-bounds"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::Bounded>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// A port no constant names made `ports` solve what the counted loops bound by
/// hand, beside the manager's. It reads the manager's.
#[test]
fn test_ports_reads_the_managers_bounds_for_a_port_no_constant_names() {
    let mut module = parsed(
        "declare void @llrm.ia16.out.i8(i16, i8)

define void @f(i16 %p, i8 %v) {
b0:
  br label %h
h:
  %i = phi i16 [ 0, %b0 ], [ %in, %l ]
  %c = icmp slt i16 %i, 4
  br i1 %c, label %l, label %b4
l:
  %in = add nsw i16 %i, 1
  br label %h
b4:
  %low = icmp sge i16 %p, 960
  br i1 %low, label %b1, label %b3
b1:
  %high = icmp sle i16 %p, 975
  br i1 %high, label %b2, label %b3
b2:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v)
  br label %b3
b3:
  ret void
}
",
    );
    let mut manager = PassManager::default();
    manager.add(AsksBounds);
    manager.add(Ports);
    let before = llrm_analysis::ranges::loops_solved();
    manager.run_module(&mut module, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    assert_eq!(llrm_analysis::ranges::loops_solved() - before, 1, "the loop's bounds were worked out more than once");
}
