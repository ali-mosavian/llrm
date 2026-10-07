//! The memory analyses asked through llrm-mir's pass manager.

use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis, FunctionPass, ModuleAnalyses, Outer, PassManager, PreservedAnalyses, Unit as PassUnit};
use llrm_mir::target::{Machine, Neutral};
use llrm_support::hash::IndexMap;

use super::{Annotated, CallEffects, DominatedEdges, Pointers, Registers, Summaries, ThroughMemory};
use crate::alias::{self, Procedure};
use crate::cfg::Shape;
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
    alias::annotated(&crate::testing::with_registers(Unit::of(module, &layout(module), function(module, "f"))))
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
            let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
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

/// A pass keeping the CFG keeps the loops found: dominance and loops were
/// once remembered per CFG shape in a thread-local instead.
#[test]
fn a_pass_that_keeps_the_cfg_keeps_its_loops() {
    use llrm_mir::passes::{Dominators, Loops};

    use crate::cfg::Shape;

    let mut module = parsed(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i16 %i, 1
  %c = icmp ult i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %j
}
",
    );
    let seen = Rc::new(RefCell::new(Vec::<Rc<Shape>>::new()));
    let look = |seen: &Rc<RefCell<Vec<Rc<Shape>>>>| {
        let seen = Rc::clone(seen);
        step(move |unit, analyses| seen.borrow_mut().push(analyses.get::<Shape>(unit.context, unit.layout, unit.function)), PreservedAnalyses::all())
    };
    // Swaps the add's operands: no edge moves.
    let swap = |preserved: PreservedAnalyses| {
        step(
            |unit, _| {
                let function = &mut *unit.function;
                let add = function.walk().map(|(_, inst)| inst).find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Binary(_))).unwrap();
                let operands = function.instruction(add).operands.iter().rev().copied().collect();
                function.set_operands(add, operands);
            },
            preserved,
        )
    };
    let mut passes = PassManager::default();
    passes.verify_invalidation = true;
    passes.add(look(&seen));
    passes.add(swap(PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()));
    passes.add(look(&seen));
    passes.add(swap(PreservedAnalyses::none()));
    passes.add(look(&seen));
    passes.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)).unwrap();
    let seen = seen.take();
    assert_eq!(seen[0].loops.len(), 1);
    assert!(Rc::ptr_eq(&seen[0], &seen[1]), "found again though the CFG was kept");
    assert!(!Rc::ptr_eq(&seen[1], &seen[2]));
    assert_eq!(seen[1], seen[2]);
}

/// Summaries and GlobalsAA ran points-to over every body with a shape of
/// its own: 100k shapes a pipeline run. Each is its body's manager's.
#[test]
fn summaries_read_each_bodys_shape_from_its_manager() {
    let module = parsed("define i16 @f(ptr %p) {\nentry:\n  %x = load i16, ptr %p\n  ret i16 %x\n}\n");
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    analyses.get::<Summaries>(&module);
    assert!(analyses.cached_function::<Shape>(module.named("f").unwrap()).is_some());
}

/// A body that calls something unknown may call back into any entry, so its summary reads each
/// entry's. Solved callees-first, `@p` was visited before `@entry` had @h's store from its callee,
/// and never again: QCport's `dl` and `savegame` came out other than before (#394).
#[test]
fn a_body_calling_the_unknown_is_summarized_again_when_an_entry_changes() {
    let module = parsed(
        "@g = internal global i16 0

declare void @ext()

define internal void @p() {
entry:
  call void @ext()
  ret void
}

define void @entry() {
entry:
  call void @h()
  ret void
}

define internal void @h() {
entry:
  store i16 1, ptr @g
  ret void
}
",
    );
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    let found = analyses.get::<Summaries>(&module);
    let summaries = Result::as_ref(&*found).expect("summarized");
    let reaches_g = |name: &str| summaries[name].unknown_write || summaries[name].writes.iter().any(|one| one.object.kind == crate::memory::MemoryKind::Global);
    assert!(reaches_g("entry"), "the entry writes @g through @h");
    assert!(reaches_g("p"), "@p's unknown call may call back into @entry");
}

/// Each access asked of its own alloca's uses whether the address is exposed, and the answer was never
/// kept: `mir decide` and `gvn` grew with slope 2.5–2.9 in a function's blocks, `exposes` 21% of the
/// compile of 300 (#557). The manager finds a function's exposed frames once.
#[test]
fn test_a_functions_exposed_frames_are_found_once_not_per_access() {
    let accesses: String = (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let module = parsed(&format!("{DOS}declare void @out(ptr)\n\ndefine i16 @f() {{\nentry:\n  %slot = alloca i16\n  %hidden = alloca i16\n{accesses}  call void @out(ptr %hidden)\n  ret i16 %v39\n}}\n"));
    let layout = layout(&module);
    let function = function(&module, "f");
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let before = crate::frameescape::scans();
    analyses.get::<Annotated>(&module.context, &layout, function).as_ref().as_ref().expect("annotates");
    assert_eq!(crate::frameescape::scans() - before, 0, "an access scanned its alloca's uses");
    let exposed = analyses.get::<super::ExposedFrames>(&module.context, &layout, function);
    let names: Vec<_> = exposed.iter().map(|one| function.value(*one).name.clone()).collect();
    assert_eq!(names.len(), 1, "only @hidden's address is handed out: {names:?}");
}

/// GlobalsAA ran points-to over every body without the exposure table, so each access scanned its alloca's
/// uses: 34% of compiling a function of 1600 statements (#560). The table is made once per body.
#[test]
fn test_globals_aa_asks_each_bodys_exposed_frames_once() {
    let accesses: String = (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let module = parsed(&format!("{DOS}@g = global i16 0\n\ndefine i16 @f() {{\nentry:\n  %slot = alloca i16\n{accesses}  store i16 %v39, ptr @g\n  ret i16 %v39\n}}\n"));
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    let before = crate::frameescape::scans();
    analyses.get::<super::GlobalsAA>(&module);
    assert_eq!(crate::frameescape::scans() - before, 0, "an access scanned its alloca's uses");
}

/// Two loops, the second counting to `%lim`, which a block outside both makes.
const LIMITED: &str = "define i16 @f() {
b0:
  %five = add i16 2, 3
  %seven = add i16 3, 4
  br label %h0

h0:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %l0 ]
  %c = icmp slt i16 %i, 9
  br i1 %c, label %l0, label %p1

l0:
  %i.next = add i16 %i, 1
  br label %h0

p1:
  %lim = add i16 %five, 0
  br label %h1

h1:
  %j = phi i16 [ 0, %p1 ], [ %j.next, %l1 ]
  %d = icmp slt i16 %j, %lim
  br i1 %d, label %l1, label %end

l1:
  %j.next = add i16 %j, 1
  br label %h1

end:
  ret i16 %j
}
";

/// A change made to what a loop's bound comes from, outside the loop, is not seen in the loop's blocks: its trips were
/// kept stale. Only the loop it reaches is proved again.
#[test]
fn a_change_outside_a_loop_reaches_the_loop_that_reads_it() {
    let mut module = parsed(&format!("{DOS}{LIMITED}"));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    let trips = |counted: &crate::induction::Counted, header: &str, function: &llrm_mir::module::Function| {
        let header = crate::cfg::id(crate::testing::block(function, header));
        counted[&header].iter().filter_map(|proof| proof.count.clone()).next()
    };
    {
        let (context, function) = module.function_mut("f").unwrap();
        let before = analyses.get::<super::Counted>(context, &layout, function);
        assert_eq!(trips(&before, "h1", function), Some(num_bigint::BigInt::from(5)));
        let lim = value(function, "lim");
        let lim = function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(lim)).unwrap();
        let seven = Operand::Value(value(function, "seven"));
        function.set_operand(lim, 0, seven);
        analyses.invalidate(&PreservedAnalyses::none());
    }
    let (context, function) = module.function_mut("f").unwrap();
    let proved = crate::induction::proved();
    let after = analyses.get::<super::Counted>(context, &layout, function);
    assert_eq!(crate::induction::proved() - proved, 1, "only the loop the change reaches is proved again");
    assert_eq!(trips(&after, "h1", function), Some(num_bigint::BigInt::from(7)), "the loop that reads the changed bound");
    assert_eq!(*after, *Analyses::new(outer).get::<super::Counted>(context, &layout, function), "what was brought up to date is what deriving it afresh gives");
}
