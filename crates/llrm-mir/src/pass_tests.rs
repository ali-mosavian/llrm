use std::cell::Cell;

use crate::context::Context;
use crate::datalayout::DataLayout;
use crate::module::{Change, Function, Module, Operand};
use crate::parse;
use crate::passes::{Analyses, Analysis, Dominators, FunctionPass, PassManager, PreservedAnalyses, Unit};

const TEXT: &str = "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %b\nb:\n  ret i16 0\n}\n";

fn module() -> Module {
    parse::module(TEXT).unwrap_or_else(|error| panic!("{error}"))
}

/// Sends the entry's first edge straight to its second, leaving `%a`
/// unreachable: a dominator change.
struct Retarget(PreservedAnalyses);

impl FunctionPass for Retarget {
    fn name(&self) -> &'static str {
        "retarget"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let function = &mut *unit.function;
        let entry = function.entry().unwrap();
        let branch = function.terminator(entry).unwrap();
        let [.., Operand::Block(_), Operand::Block(second)] = function.instruction(branch).operands[..] else { panic!("a conditional branch") };
        let at = function.instruction(branch).operands.len() - 2;
        function.set_operand(branch, at, Operand::Block(second));
        self.0.clone()
    }
}

thread_local! {
    static COMPUTED: Cell<usize> = const { Cell::new(0) };
}

/// Counts its own computations.
struct Counted;

impl Analysis for Counted {
    type Result = usize;
    const NAME: &'static str = "counted";
    fn run(_: &Context, _: &DataLayout, function: &Function) -> usize {
        COMPUTED.set(COMPUTED.get() + 1);
        function.layout().len()
    }
}

struct Look(PreservedAnalyses);

impl FunctionPass for Look {
    fn name(&self) -> &'static str {
        "look"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        analyses.get::<Counted>(unit.context, unit.layout, unit.function);
        self.0.clone()
    }
}

struct DropReturn;

impl FunctionPass for DropReturn {
    fn name(&self) -> &'static str {
        "drop-return"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut Analyses) -> PreservedAnalyses {
        let last = *unit.function.layout().last().unwrap();
        let ret = unit.function.terminator(last).unwrap();
        unit.function.erase(ret).unwrap();
        PreservedAnalyses::none()
    }
}

/// LLVM's `-verify-analysis-invalidation`: a pass that keeps a dominator tree
/// it made stale would have every later pass read wrong dominance.
#[test]
fn a_pass_keeping_dominators_it_changed_is_caught() {
    let mut passes = PassManager { verify_invalidation: true, ..Default::default() };
    passes.add(Retarget(PreservedAnalyses::none().preserve::<Dominators>()));
    assert_eq!(passes.run(&mut module()).err().as_deref(), Some("retarget claims to preserve dominators but changed them"));

    let mut passes = PassManager { verify_invalidation: true, ..Default::default() };
    passes.add(Retarget(PreservedAnalyses::none()));
    assert!(passes.run(&mut module()).is_ok());
}

#[test]
fn an_analysis_is_computed_once_until_a_pass_drops_it() {
    COMPUTED.set(0);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(Look(PreservedAnalyses::none()));
    passes.add(Look(PreservedAnalyses::all()));
    passes.run(&mut module()).unwrap();
    assert_eq!(COMPUTED.get(), 2);
}

/// The ledger reads which pass made which change from here.
#[test]
fn each_stage_carries_its_own_changes() {
    let mut module = module();
    let branch = {
        let function = module.function_mut("f").unwrap().1;
        function.terminator(function.entry().unwrap()).unwrap()
    };
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(Retarget(PreservedAnalyses::none()));
    let stages = passes.run(&mut module).unwrap();
    let changes: Vec<(&str, &[Change])> = stages.iter().map(|one| (one.pass, one.changes.as_slice())).collect();
    assert_eq!(changes, [("look", &[][..]), ("retarget", &[Change::Rewritten(branch)][..])]);
}

#[test]
fn verify_each_names_the_pass_that_broke_the_module() {
    let mut passes = PassManager { verify_each: true, ..Default::default() };
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(DropReturn);
    let error = passes.run(&mut module()).unwrap_err();
    assert!(error.starts_with("after drop-return: "), "{error}");
}
