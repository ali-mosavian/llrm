//! switch_table_holes at m32 -O2: ours counted the third register past a
//! budget of two (six allocatable, one more kept back for a stack pointer
//! already outside the six, three live elsewhere) and kept one `i32` counter
//! that widens the `i8` add: 130 B against 122. The `i8` counter is our shape,
//! not a claim about the pin's.

use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;

/// m32 as LSR sees it: the i486 model with the six allocatable registers.
struct M32Like;

impl llrm_mir::target::Machine for M32Like {
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
        Some(Rc::new(llrm_x86::tti::X86::of_cpu("i486").unwrap().with_registers(6)))
    }
}

#[test]
fn a_target_that_states_its_registers_keeps_none_back_for_the_stack_pointer() {
    let mut module = llrm_mir::parse::module(include_str!("lsr_register_budget/switch_table_holes.ll"))
        .unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false });
    manager.run_module(&mut module, Rc::new(M32Like)).expect("runs");
    let text = llrm_mir::print::module(&module);
    assert!(text.contains("= phi i8 "), "no i8 counter: {text}");
}
