//! The driver builds a function both ways where LSR's end-biased formulae
//! (DEVIATE LG3) differ from the pin's: LSR marks such a function
//! `llrm-end-biased`, and makes none for one marked `llrm-no-end-biased`.

use std::fs;
use std::path::PathBuf;
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
    reduced_with(text, None)
}

fn reduced_with(
    text: &str,
    flag: Option<&str>,
) -> String {
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"));
    if let Some(flag) = flag {
        let (_, function) = module.function_mut("f").expect("f");
        function.attrs.push(llrm_mir::opcode::Attribute::Flag(flag.into()));
    }
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false, bytes_strict: false });
    manager.run_module(&mut module, Rc::new(Flat("i486"))).expect("runs");
    llrm_mir::print::module(&module)
}

fn shape() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ivshare/shapes/B09.p32.ll");
    fs::read_to_string(path).unwrap()
}

/// A counted loop of one array at i486 takes the end form (the exit counter
/// shared with the address); nothing said so, and a driver could not tell
/// whether to build the function the pin's way too.
#[test]
fn a_function_with_end_biased_formulae_is_marked() {
    let out = reduced(&shape());
    assert!(out.contains("llrm-end-biased"), "{out}");
    assert!(out.contains("ptr %p0, i32 400"), "{out}");
}

/// A function marked `llrm-no-end-biased` gets the pin's formulae only.
#[test]
fn a_function_marked_no_end_biased_gets_the_pins_solution() {
    let out = reduced_with(&shape(), Some(llrm_transforms::lsr_pass::NO_END_BIASED));
    assert!(!out.contains("llrm-end-biased"), "{out}");
    assert!(!out.contains("ptr %p0, i32 400"), "{out}");
}
