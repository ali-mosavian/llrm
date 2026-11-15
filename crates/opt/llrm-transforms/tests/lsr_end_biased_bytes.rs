//! DEVIATE LG3 where bytes outrank clocks: at -Os the end-biased solution is
//! refused when the target's byte table prices it larger than the pin's, and at
//! -O1 where the target does not price bytes at speed. -O1 is judged on clocks
//! (user ruling 2026-11-14): where the byte price at speed counts it keeps a
//! form that saves a loop instruction for a byte, as -O2 does.

use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;
use llrm_transforms::pipeline::{self, Applied, Options};

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

fn reduced(
    size: bool,
    bytes_strict: bool,
) -> String {
    let mut module = llrm_mir::parse::module(SWAP).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size, bytes_strict });
    manager.run_module(&mut module, Rc::new(Flat)).expect("runs");
    llrm_mir::print::module(&module)
}

/// The swap loop's end-biased form saves a loop instruction (2 against 3) for
/// one byte of setup (24 against 23). -O1 refused it on the byte even where
/// the target prices bytes at speed (`speed_rate_at_o1`, -O1 judged on clocks):
/// m16 grep -O1 kept the pin's three counters and spilled the scan loop (46.4M
/// clocks against bcc's 33.9M). The pin's -O1 (no rate) still refuses it.
#[test]
fn o1_keeps_end_biased_that_saves_an_instruction_for_a_byte_where_bytes_are_priced() {
    let rated = Options { speed_rate: true, ..Options::basic() };
    let text = in_strength_slot(rated);
    assert!(text.contains("llrm-end-biased"), "-O1 with the byte price at speed kept the pin's form on a byte\n{text}");
    let unrated = in_strength_slot(Options::basic());
    assert!(!unrated.contains("llrm-end-biased"), "-O1 without the rate holds bytes\n{unrated}");
    let os = in_strength_slot(Options::size());
    assert!(!os.contains("llrm-end-biased"), "-Os has no clock axis\n{os}");
}

fn in_strength_slot(options: Options) -> String {
    let mut module = llrm_mir::parse::module(SWAP).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    for pass in pipeline::strength_slot(&Applied { options, ..Applied::default() }) {
        manager.add_boxed(pass);
    }
    manager.run_module(&mut module, Rc::new(Flat)).expect("runs");
    llrm_mir::print::module(&module)
}

/// -Os has no clock axis: the same form costs a byte and is refused.
#[test]
fn os_refuses_end_biased_that_costs_a_byte() {
    let speed = reduced(false, false);
    assert!(speed.contains("llrm-end-biased"), "premise: at speed the swap loop is end-biased\n{speed}");
    let os = reduced(true, true);
    assert!(!os.contains("llrm-end-biased"), "{os}");
}
