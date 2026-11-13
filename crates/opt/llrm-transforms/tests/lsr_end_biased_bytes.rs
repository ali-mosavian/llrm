//! DEVIATE LG3 where bytes outrank clocks: at -Os the end-biased solution is
//! refused when the target's byte table prices it larger than the pin's; at
//! -O1, where clocks are the other axis, only when it also saves no loop
//! instruction (x_radix -O1 lost 1578 clk to a veto that saved 37 B).

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
        Some(Rc::new(
            llrm_x86::tti::X86::of_cpu("i486")
                .unwrap()
                .with_tuning(llrm_mir::target::Tuning { lsr_size_order: true, ..llrm_mir::target::Tuning::PIN }),
        ))
    }
}

/// x_reverse's inner loop: one pointer up from `@s`, one down from `@s + 399`,
/// a byte swapped each trip, a trip count the loop's setup computes.
const SWAP: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"

@s = global [400 x i8] zeroinitializer

define void @f(i32 %n) {
entry:
  %g = icmp sgt i32 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %lo = getelementptr i8, ptr @s, i32 %i
  %back = sub i32 399, %i
  %hi = getelementptr i8, ptr @s, i32 %back
  %x = load i8, ptr %lo
  %y = load i8, ptr %hi
  store i8 %y, ptr %lo
  store i8 %x, ptr %hi
  %next = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
";

fn reduced(size: bool, bytes_strict: bool) -> String {
    let mut module = llrm_mir::parse::module(SWAP).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size, bytes_strict });
    manager.run_module(&mut module, Rc::new(Flat)).expect("runs");
    llrm_mir::print::module(&module)
}

/// The swap loop's end-biased form saves a loop instruction (2 against 3) for
/// one byte of setup (24 against 23). -O1 refused it on the byte alone: x_radix
/// -O1 m32 went from 42660 to 44238 clk.
#[test]
fn o1_keeps_end_biased_that_saves_an_instruction_for_a_byte() {
    let o1 = reduced(false, true);
    assert!(o1.contains("llrm-end-biased"), "-O1 refused a form that saves an instruction\n{o1}");
}

/// -Os has no clock axis: the same form costs a byte and is refused.
#[test]
fn os_refuses_end_biased_that_costs_a_byte() {
    let speed = reduced(false, false);
    assert!(speed.contains("llrm-end-biased"), "premise: at speed the swap loop is end-biased\n{speed}");
    let os = reduced(true, true);
    assert!(!os.contains("llrm-end-biased"), "{os}");
}
