//! DEVIATE LSR-SIZE at x86-64: the byte model prices the exit test, so a
//! counter that runs down to zero (flags from the step, no `cmp`) is kept at
//! -Os. Without the term crc's 8-trip bit loop counted up from zero and paid a
//! `cmp r16,8` and a zeroing for it: bench/crc -Os +2 B.

use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;

struct Flat {
    x86_64: bool,
}

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
        Some(Rc::new(llrm_x86::tti::X86::of_cpu("x86-64").unwrap().with_tuning(llrm_mir::target::Tuning {
            lsr_size_order: true,
            lsr_size_x86_64: self.x86_64,
            ..llrm_mir::target::Tuning::PIN
        })))
    }
}

/// crc's inner loop: eight trips of a 16-bit counter running down to zero.
const BITS: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"

define i64 @f(i64 %x, i64 %poly) {
entry:
  br label %loop
loop:
  %j = phi i16 [ 8, %entry ], [ %next, %loop ]
  %v = phi i64 [ %x, %entry ], [ %nv, %loop ]
  %lsb = and i64 %v, 1
  %neg = sub i64 0, %lsb
  %mask = and i64 %neg, %poly
  %shifted = lshr i64 %v, 1
  %nv = xor i64 %shifted, %mask
  %next = add nsw i16 %j, -1
  %c = icmp eq i16 %next, 0
  br i1 %c, label %exit, label %loop
exit:
  ret i64 %nv
}
";

fn reduced(x86_64: bool) -> String {
    let mut module = llrm_mir::parse::module(BITS).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: true, bytes_strict: false });
    manager.run_module(&mut module, Rc::new(Flat { x86_64 })).expect("runs");
    llrm_mir::print::module(&module)
}

/// The count down to zero ends on the step's flags; counting up to 8 costs a
/// `cmp r16,8` and a zeroed start.
#[test]
fn a_loop_counting_down_to_zero_keeps_its_flags_exit() {
    let without_the_term = reduced(false);
    assert!(
        without_the_term.contains("icmp eq i16 %next, 8") || without_the_term.contains(", 8\n  br i1"),
        "premise: the 32-bit table counts up to 8\n{without_the_term}"
    );
    let out = reduced(true);
    assert!(!out.contains(", 8\n  br i1"), "counted up to 8 for a cmp:\n{out}");
}
