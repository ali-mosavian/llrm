//! The memory analyses asked through llrm-mir's pass manager.

use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis, FunctionPass, Outer, PassManager, PreservedAnalyses, Unit as PassUnit};
use llrm_mir::target::{Machine, Neutral};
use llrm_support::hash::IndexMap;

use super::{Annotated, CallEffects, DominatedEdges, Pointers, Registers, Summaries, ThroughMemory};
use crate::alias::{self, Procedure};
use crate::consts::{self, Calls};
use crate::memory::Unit;
use crate::ranges;
use crate::regions::tests::Dos;
use crate::testing::{DOS, corpus, function, layout, parsed, value};

/// A pass that does its first and preserves its second.
struct Step(Box<dyn FnMut(&mut PassUnit, &mut Analyses)>, PreservedAnalyses);

fn step(what: impl FnMut(&mut PassUnit, &mut Analyses) + 'static, preserved: PreservedAnalyses) -> Step {
    Step(Box::new(what), preserved)
}

impl FunctionPass for Step {
    fn name(&self) -> &'static str {
        "step"
    }

    fn run(&mut self, unit: &mut PassUnit, analyses: &mut Analyses) -> PreservedAnalyses {
        (self.0)(unit, analyses);
        self.1.clone()
    }
}

type Answer = <Annotated as Analysis>::Result;
type Seen = Rc<RefCell<Vec<Rc<Answer>>>>;

/// Records what `Annotated` answers, and keeps everything.
fn ask(seen: &Seen) -> Step {
    let seen = Rc::clone(seen);
    step(move |unit, analyses| seen.borrow_mut().push(analyses.get::<Annotated>(unit.context, unit.layout, unit.function)), PreservedAnalyses::all())
}

/// Sends @f's store to `%b`, which was to `%a`.
fn redirect(preserved: PreservedAnalyses) -> Step {
    step(
        |unit, _| {
            let function = &mut *unit.function;
            let store = function.walk().map(|(_, inst)| inst).find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. })).unwrap();
            let b = value(function, "b");
            function.set_operand(store, 1, Operand::Value(b));
        },
        preserved,
    )
}

const TWO: &str = "define i16 @f() {
entry:
  %a = alloca i16
  %b = alloca i16
  store i16 1, ptr %a
  %v = load i16, ptr %a
  ret i16 %v
}
";

/// What `alias::annotated` says of @f of `module`, asked directly.
fn direct(module: &Module) -> Answer {
    alias::annotated(&Unit::of(module, &layout(module), function(module, "f")))
}

/// What the passes saw of `Annotated`, run in turn on `TWO`, and the
/// module after.
fn seen(steps: impl FnOnce(&Seen) -> Vec<Step>) -> (Vec<Rc<Answer>>, Module) {
    let mut module = parsed(&format!("{DOS}{TWO}"));
    let seen = Seen::default();
    let mut passes = PassManager::default();
    for step in steps(&seen) {
        passes.add(step);
    }
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    (seen.take(), module)
}

#[test]
fn asking_again_without_a_change_reuses_the_result() {
    let (seen, _) = seen(|seen| vec![ask(seen), ask(seen)]);
    assert!(Rc::ptr_eq(&seen[0], &seen[1]));
}

#[test]
fn a_pass_that_drops_an_analysis_has_it_answer_for_its_edit() {
    let before = direct(&parsed(&format!("{DOS}{TWO}")));
    let (seen, after) = seen(|seen| vec![ask(seen), redirect(PreservedAnalyses::none()), ask(seen)]);
    assert_eq!(*seen[0], before);
    assert_eq!(*seen[1], direct(&after));
    assert_ne!(seen[0], seen[1]);
}

#[test]
fn a_pass_that_preserves_an_analysis_leaves_it_as_it_was() {
    let (seen, _) = seen(|seen| vec![ask(seen), redirect(PreservedAnalyses::none().preserve::<Annotated>()), ask(seen)]);
    assert!(Rc::ptr_eq(&seen[0], &seen[1]));
}

/// The target reaches `regions` through the pass manager: a far store
/// into foreign memory keeps @g's cell only where the target says the
/// segment is foreign.
#[test]
fn the_pass_managers_target_decides_foreign_memory() {
    let text = format!(
        "{DOS}@g = global i16 0

define i16 @f() {{
b0:
  store i16 7, ptr @g
  %s = inttoptr i16 -18432 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %far
  %r = load i16, ptr @g
  ret i16 %r
}}
"
    );
    let known = |dos: bool| {
        let mut module = parsed(&text);
        let r = value(function(&module, "f"), "r");
        let got = Rc::new(RefCell::new(None));
        let mut passes = PassManager::default();
        passes.require::<Summaries>();
        let into = Rc::clone(&got);
        passes.add(step(
            move |unit, analyses| {
                let known = analyses.get::<ThroughMemory>(unit.context, unit.layout, unit.function);
                *into.borrow_mut() = Result::as_ref(&*known).unwrap().get(&r).cloned();
            },
            PreservedAnalyses::all(),
        ));
        let target: Rc<dyn Machine> = if dos { Rc::new(Dos::default()) } else { Rc::new(Neutral) };
        passes.run_module(&mut module, target).unwrap();
        got.take().map(|fact| fact.n)
    };
    assert_eq!(known(false), None);
    assert_eq!(known(true), Some(7.into()));
}

/// Through the manager each function sees its module by declarations and
/// its callees by `Summaries`; it answers as when asked directly.
#[test]
fn every_corpus_function_answers_through_the_manager_as_directly() {
    for (name, module) in corpus() {
        let layout = layout(&module);
        let mut outer = Outer::of(&module, None);
        outer.require::<Summaries>(&module);
        let summaries = outer.cached::<Summaries>().unwrap();
        let summaries = Result::as_ref(&*summaries).unwrap_or_else(|error| panic!("{name}: {error}"));
        let outer = Rc::new(outer);
        for (_, global, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            let at = format!("{name}/@{}", global.name.as_deref().unwrap_or(""));
            let unit = Unit::of(&module, &layout, function);
            let mut analyses = Analyses::new(Rc::clone(&outer));
            assert_eq!(*analyses.get::<Pointers>(&module.context, &layout, function), alias::points_to(&unit, None, None), "{at}");
            assert_eq!(*analyses.get::<Annotated>(&module.context, &layout, function), alias::annotated(&unit), "{at}");
            assert_eq!(*analyses.get::<Registers>(&module.context, &layout, function), consts::known(&unit, None, None, None), "{at}");
            assert_eq!(*analyses.get::<DominatedEdges>(&module.context, &layout, function), ranges::dominated_edges(&unit), "{at}");
            let effects = alias::calls_annotated(&Procedure::of(unit), summaries);
            assert_eq!(*analyses.get::<CallEffects>(&module.context, &layout, function), effects, "{at}");
            let calls: Calls = effects.unwrap().into_iter().map(|(at, effect)| (at, effect.stores)).collect::<IndexMap<_, _>>();
            assert_eq!(*analyses.get::<ThroughMemory>(&module.context, &layout, function), Ok(consts::known(&unit, Some(&calls), None, None)), "{at}");
        }
    }
}
