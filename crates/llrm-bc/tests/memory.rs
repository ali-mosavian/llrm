//! The raise's memory: what `llrm_analysis` proves of it.

use llrm_analysis::alias::annotated;
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::regions::overlapping;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::{ConstantKind, InstId, Module, Opcode, Operand};

fn raised(fixture: &str) -> Module {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/omf").join(fixture);
    let found = llrm_omf::module::load(&path).expect("reads").expect("an object");
    let module = llrm_bc::raise(&found).unwrap_or_else(|refusal| panic!("{refusal}"));
    let errors = llrm_mir::verify::verify(&module);
    assert!(errors.is_empty(), "{errors:#?}\n{}", llrm_mir::print::module(&module));
    module
}

/// The accesses of `main`, each with what alias analysis makes of it.
fn accesses(module: &Module) -> Vec<(InstId, MemRef)> {
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("parses");
    let function = module.global(module.named("main").expect("a main")).function().expect("a function");
    let references = annotated(&Unit::of(module, &layout, function)).expect("annotates");
    references.into_iter().collect()
}

/// An access of `main` to the global `name`, through its GEPs.
fn access<'a>(module: &Module, all: &'a [(InstId, MemRef)], name: &str) -> &'a MemRef {
    let function = module.global(module.named("main").expect("a main")).function().expect("a function");
    let target = module.named(name).unwrap_or_else(|| panic!("no @{name}"));
    let root = |mut pointer: Operand| loop {
        let Operand::Value(value) = pointer else { return pointer };
        let llrm_mir::ValueDef::Instruction(inst) = function.value(value).def else { return pointer };
        let one = function.instruction(inst);
        if !matches!(one.opcode, Opcode::GetElementPtr { .. }) {
            return pointer;
        }
        pointer = one.operands[0];
    };
    all.iter()
        .find(|(inst, _)| {
            let one = function.instruction(*inst);
            let pointer = match one.opcode {
                Opcode::Load { .. } => one.operands[0],
                Opcode::Store { .. } => one.operands[1],
                _ => return false,
            };
            matches!(root(pointer), Operand::Constant(id) if module.context.get(id).kind == ConstantKind::Global(target))
        })
        .map(|(_, reference)| reference)
        .unwrap_or_else(|| panic!("no access to @{name}"))
}

/// byref2: `PRINT Doubled(d)` reads the DOUBLE through the address the
/// FUNCTION answers, a register: that is its result cell, apart from `d`.
#[test]
fn an_address_in_a_register_keeps_its_object() {
    let module = raised("byref2-q-o.obj");
    let all = accesses(&module);
    let (result, d) = (access(&module, &all, "BC_DATA.0016"), access(&module, &all, "D#"));
    assert_eq!(overlapping(result, d, None, None, None), Ok(false));
}
