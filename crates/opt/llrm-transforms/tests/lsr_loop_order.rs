//! LSR runs the loops in the order the code generator's loop pass manager
//! does: sibling loops in program order. Run the other way round, the
//! induction variables the inner loops start from the outer loop's values
//! came out of the outer loop's header in the other order.

use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;

/// A flat x86 target: the neutral machine with the CPU's LSR model.
struct Flat(&'static str);

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
        Some(Rc::new(llrm_x86::tti::X86::of_cpu(self.0).unwrap()))
    }
}

fn reduced(text: &str) -> String {
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false });
    manager.run_module(&mut module, Rc::new(Flat("i486"))).expect("runs");
    llrm_mir::print::module(&module)
}

/// Two inner loops of one outer loop each start an induction variable from
/// the outer loop's pointer (`%a`, then `%b`). The pin reduces the first loop
/// first, so the pointer of the second is made last and heads the outer
/// loop's phis: `%b`'s, then `%a`'s. Reduced second loop first, they came
/// out `%a`'s, then `%b`'s, and PLASMA's header phis followed.
#[test]
fn sibling_loops_are_reduced_in_program_order() {
    let out = reduced(include_str!("lsr_loop_order/siblings.ll"));
    let header: Vec<&str> =
        out.lines().skip_while(|line| *line != "outer:").skip(1).take_while(|line| line.contains("phi")).collect();
    let starts: Vec<&str> = header.iter().filter(|line| line.contains("phi ptr")).copied().collect();
    assert_eq!(starts.len(), 2, "{out}");
    assert!(starts[0].contains("%1, %entry"), "{out}");
    assert!(out.contains("%1 = getelementptr i8, ptr %b"), "{out}");
}
