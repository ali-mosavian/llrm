use llrm_analysis::testing::corpus;
use llrm_mir::interpret;
use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};

// Enough for every corpus entry that finishes at all.
const FUEL: u64 = 2_000_000;

#[test]
fn the_pipeline_keeps_every_corpus_module_verifying_and_computing_the_same() {
    let applied = Applied::default();
    let mut ran = 0;
    for (name, mut module) in corpus() {
        let entry = module.named("main").filter(|&id| module.global(id).function().is_some_and(|one| !one.is_declaration() && one.parameters().is_empty()));
        let before = entry.map(|_| interpret::run(&module, "main", Vec::new(), FUEL));
        Program::lend(&mut module, std::rc::Rc::new(llrm_cycles::target::Dos::default()), |program| pipeline::applied(program, &applied))
            .and_then(|done| done)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        if let Some(Ok(before)) = before {
            assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), Ok(before), "{name}");
            ran += 1;
        }
    }
    assert!(ran > 0, "no corpus entry runs");
}

/// Commutes @f's add so that its constant comes first or last.
struct Commute {
    name: &'static str,
    constant_first: bool,
}

impl llrm_mir::passes::FunctionPass for Commute {
    fn name(&self) -> &'static str {
        self.name
    }

    fn run(&mut self, unit: &mut llrm_mir::passes::Unit, _: &mut llrm_mir::passes::Analyses) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_mir::module::Operand;
        let (_, add) = unit.function.walk().find(|&(_, one)| unit.function.instruction(one).opcode == llrm_mir::opcode::Opcode::Binary(llrm_mir::opcode::BinaryOp::Add)).expect("an add");
        let operands = unit.function.instruction(add).operands.clone();
        if matches!(operands[0], Operand::Constant(_)) == self.constant_first {
            return llrm_mir::passes::PreservedAnalyses::all();
        }
        unit.function.set_operands(add, vec![operands[1], operands[0]]);
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// Arenas never shrink, so before bodies were compared by their text two
/// passes undoing each other ran the whole size-scaled limit.
#[test]
#[should_panic(expected = "cycle after 1 rounds")]
fn passes_that_undo_each_other_stop_after_one_cycle() {
    let mut module = llrm_analysis::testing::parsed("define i16 @f(i16 %x) {\nb0:\n  %y = add i16 %x, 1\n  ret i16 %y\n}\n");
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![Box::new(Commute { name: "first", constant_first: true }), Box::new(Commute { name: "last", constant_first: false })];
    crate::testing::managed(&mut module, fixed);
}
