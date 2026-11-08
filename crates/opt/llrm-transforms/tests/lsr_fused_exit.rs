//! Where the exit compare macro-fuses with its branch, a counter shared with
//! the addresses saves nothing (DEVIATE LG3 is off there).
use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;

struct Flat;

impl llrm_mir::target::Machine for Flat {
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }

    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        Some(Rc::new(llrm_x86::tti::X86::of_cpu("x86-64").unwrap()))
    }
}

const SRC: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\n@src = global [1200 x i8] zeroinitializer\n\ndefine void @f() {\nentry:\n  br label %loop\nloop:\n  %i = phi i64 [ 0, %entry ], [ %next, %loop ]\n  %a0 = getelementptr i8, ptr @src, i64 %i\n  store i8 1, ptr %a0\n  %i1 = add i64 %i, 1\n  %a1 = getelementptr i8, ptr @src, i64 %i1\n  store i8 2, ptr %a1\n  %i2 = add i64 %i, 2\n  %a2 = getelementptr i8, ptr @src, i64 %i2\n  store i8 3, ptr %a2\n  %i3 = add i64 %i, 3\n  %a3 = getelementptr i8, ptr @src, i64 %i3\n  store i8 4, ptr %a3\n  %next = add nuw nsw i64 %i, 4\n  %c = icmp eq i64 %next, 1200\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n}\n";

fn reduced(text: &str) -> String {
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false });
    manager.run_module(&mut module, Rc::new(Flat)).expect("runs");
    llrm_mir::print::module(&module)
}

/// x86-64 memory_expansion: four stores at consecutive offsets from a global
/// shared the exit counter (-1200 up to 0) through a base at the array's end,
/// `[rax+rbx+0x4b3-k]` with a disp32 on each of 1900 accesses: +1010 B. The
/// compare fuses with its branch there, so the pointer walks and the counter
/// is only the exit test.
#[test]
fn a_fused_exit_test_does_not_share_its_counter_with_the_addresses() {
    let out = reduced(SRC);
    assert!(!out.contains("-1200"), "{out}");
}
