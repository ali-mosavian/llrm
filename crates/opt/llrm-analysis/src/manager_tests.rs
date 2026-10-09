//! The memory analyses asked through llrm-mir's pass manager.

use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{
    Analyses, Analysis, FunctionPass, ModuleAnalyses, Outer, PassManager, PreservedAnalyses, Unit as PassUnit,
};
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

fn step(
    what: impl FnMut(&mut PassUnit, &mut Analyses) + 'static,
    preserved: PreservedAnalyses,
) -> Step {
    Step(Box::new(what), preserved)
}

impl FunctionPass for Step {
    fn name(&self) -> &'static str {
        "step"
    }

    fn run(
        &mut self,
        unit: &mut PassUnit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        (self.0)(unit, analyses);
        self.1.clone()
    }
}

type Answer = <Annotated as Analysis>::Result;
type Seen = Rc<RefCell<Vec<Rc<Answer>>>>;

/// Records what `Annotated` answers, and keeps everything.
fn ask(seen: &Seen) -> Step {
    let seen = Rc::clone(seen);
    step(
        move |unit, analyses| {
            seen.borrow_mut().push(analyses.get::<Annotated>(unit.context, unit.layout, unit.function))
        },
        PreservedAnalyses::all(),
    )
}

/// Sends @f's store to `%b`, which was to `%a`.
fn redirect(preserved: PreservedAnalyses) -> Step {
    step(
        |unit, _| {
            let function = &mut *unit.function;
            let store = function
                .walk()
                .map(|(_, inst)| inst)
                .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
                .unwrap();
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
    let (seen, _) =
        seen(|seen| vec![ask(seen), redirect(PreservedAnalyses::none().preserve::<Annotated>()), ask(seen)]);
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
            assert_eq!(
                *analyses.get::<Pointers>(&module.context, &layout, function),
                alias::points_to(&unit, None, None),
                "{at}"
            );
            assert_eq!(*analyses.get::<Annotated>(&module.context, &layout, function), alias::annotated(&unit), "{at}");
            assert_eq!(
                *analyses.get::<Registers>(&module.context, &layout, function),
                consts::known(&unit, None, None, None),
                "{at}"
            );
            assert_eq!(
                analyses
                    .get::<DominatedEdges>(&module.context, &layout, function)
                    .as_ref()
                    .as_ref()
                    .map(|states| states.blocks(function))
                    .map_err(String::clone),
                ranges::dominated_edges(&unit),
                "{at}"
            );
            let effects = alias::calls_annotated(&Procedure::of(unit), summaries);
            assert_eq!(*analyses.get::<CallEffects>(&module.context, &layout, function), effects, "{at}");
            let calls: Calls =
                effects.unwrap().into_iter().map(|(at, effect)| (at, effect.stores)).collect::<IndexMap<_, _>>();
            assert_eq!(
                *analyses.get::<ThroughMemory>(&module.context, &layout, function),
                Ok(consts::known(&unit, Some(&calls), None, None)),
                "{at}"
            );
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
        step(
            move |unit, analyses| {
                seen.borrow_mut().push(analyses.get::<Shape>(unit.context, unit.layout, unit.function))
            },
            PreservedAnalyses::all(),
        )
    };
    // Swaps the add's operands: no edge moves.
    let swap = |preserved: PreservedAnalyses| {
        step(
            |unit, _| {
                let function = &mut *unit.function;
                let add = function
                    .walk()
                    .map(|(_, inst)| inst)
                    .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Binary(_)))
                    .unwrap();
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

/// A body that calls something unknown may call back into any entry, so its
/// summary reads each entry's. Solved callees-first, `@p` was visited before
/// `@entry` had @h's store from its callee, and never again: QCport's `dl` and
/// `savegame` came out other than before (#394).
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
    let reaches_g = |name: &str| {
        summaries[name].unknown_write
            || summaries[name].writes.iter().any(|one| one.object.kind == crate::memory::MemoryKind::Global)
    };
    assert!(reaches_g("entry"), "the entry writes @g through @h");
    assert!(reaches_g("p"), "@p's unknown call may call back into @entry");
}

/// Each access asked of its own alloca's uses whether the address is exposed,
/// and the answer was never kept: `mir decide` and `gvn` grew with slope
/// 2.5–2.9 in a function's blocks, `exposes` 21% of the compile of 300 (#557).
/// The manager finds a function's exposed frames once.
#[test]
fn test_a_functions_exposed_frames_are_found_once_not_per_access() {
    let accesses: String =
        (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let module = parsed(&format!(
        "{DOS}declare void @out(ptr)\n\ndefine i16 @f() {{\nentry:\n  %slot = alloca i16\n  %hidden = alloca i16\n{accesses}  call void @out(ptr %hidden)\n  ret i16 %v39\n}}\n"
    ));
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

/// The observers analysis (dse) built its unit without the exposure table, so
/// every access it resolved scanned its alloca's uses: slope 2 in a function's
/// accesses, 4% of compiling 2,000 stores at -O2 (#924).
#[test]
fn test_the_published_objects_analysis_asks_the_exposed_frames_once() {
    let accesses: String =
        (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let module = parsed(&format!(
        "{DOS}declare void @out(ptr)\n\ndefine i16 @f() {{\nentry:\n  %slot = alloca i16\n  %hidden = alloca i16\n{accesses}  call void @out(ptr %hidden)\n  ret i16 %v39\n}}\n"
    ));
    let layout = layout(&module);
    let function = function(&module, "f");
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    analyses.get::<super::ExposedFrames>(&module.context, &layout, function);
    let before = crate::frameescape::scans();
    analyses
        .get::<crate::observers::Published>(&module.context, &layout, function)
        .as_ref()
        .as_ref()
        .expect("publishes");
    assert_eq!(crate::frameescape::scans() - before, 0, "an access scanned its alloca's uses");
}

/// GlobalsAA ran points-to over every body without the exposure table, so each
/// access scanned its alloca's uses: 34% of compiling a function of 1600
/// statements (#560). The table is made once per body.
#[test]
fn test_globals_aa_asks_each_bodys_exposed_frames_once() {
    let accesses: String =
        (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let module = parsed(&format!(
        "{DOS}@g = global i16 0\n\ndefine i16 @f() {{\nentry:\n  %slot = alloca i16\n{accesses}  store i16 %v39, ptr @g\n  ret i16 %v39\n}}\n"
    ));
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

/// A change made to what a loop's bound comes from, outside the loop, is not
/// seen in the loop's blocks: its trips were kept stale. Only the loop it
/// reaches is proved again.
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
        let lim =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(lim)).unwrap();
        let seven = Operand::Value(value(function, "seven"));
        function.set_operand(lim, 0, seven);
        analyses.invalidate(&PreservedAnalyses::none());
    }
    let (context, function) = module.function_mut("f").unwrap();
    let proved = crate::induction::proved();
    let after = analyses.get::<super::Counted>(context, &layout, function);
    // The check derives them afresh to compare, and is counted.
    if std::env::var_os("LLRM_CHECK_REPLAY").is_none() {
        assert_eq!(crate::induction::proved() - proved, 1, "only the loop the change reaches is proved again");
    }
    assert_eq!(
        trips(&after, "h1", function),
        Some(num_bigint::BigInt::from(7)),
        "the loop that reads the changed bound"
    );
    assert_eq!(
        *after,
        *Analyses::new(outer).get::<super::Counted>(context, &layout, function),
        "what was brought up to date is what deriving it afresh gives"
    );
}

/// A branch's bound changed outside the blocks it narrows: the edge facts of
/// the blocks past it were kept stale. Only the blocks the change reaches are
/// worked again, and what comes out is what working them all gives.
#[test]
fn a_change_to_a_branch_bound_reworks_the_blocks_past_it() {
    let mut module = parsed(&format!("{DOS}{LIMITED}"));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    let states = |analyses: &mut Analyses, module: &llrm_mir::module::Module| {
        let function = function(module, "f");
        analyses
            .get::<DominatedEdges>(&module.context, &layout, function)
            .as_ref()
            .as_ref()
            .map(|states| states.blocks(function))
            .map_err(String::clone)
            .unwrap()
    };
    let before = states(&mut analyses, &module);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let lim = value(function, "lim");
        let lim =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(lim)).unwrap();
        let seven = Operand::Value(value(function, "seven"));
        function.set_operand(lim, 0, seven);
        analyses.invalidate(&PreservedAnalyses::none());
    }
    let solved = ranges::blocks_solved();
    let after = states(&mut analyses, &module);
    // The check works them all again to compare, and is counted.
    if std::env::var_os("LLRM_CHECK_REPLAY").is_none() {
        assert!(
            ranges::blocks_solved() - solved < function(&module, "f").layout().len(),
            "{} blocks worked of {}",
            ranges::blocks_solved() - solved,
            function(&module, "f").layout().len()
        );
    }
    assert_ne!(before, after, "the bound the change reached");
    let fresh = Analyses::new(outer);
    let mut fresh = fresh;
    assert_eq!(after, states(&mut fresh, &module), "what was brought up to date is what working every block gives");
}

/// The second loop starts straight from the first's header, below no block of
/// its own, and carries what the first knows of values it never reads.
const DIRECT: &str = "define i16 @f() {
b0:
  %nine = add i16 4, 5
  %three = add i16 1, 2
  %four = add i16 2, 2
  br label %h0

h0:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %l0 ]
  %t = mul i16 %i, %three
  br label %l0

l0:
  %i.next = add i16 %i, 1
  %c = icmp slt i16 %i.next, %nine
  br i1 %c, label %h0, label %h1

h1:
  %j = phi i16 [ 0, %h0 ], [ %j.next, %l1 ]
  %d = icmp slt i16 %j, 5
  br i1 %d, label %l1, label %end

l1:
  %j.next = add i16 %j, 1
  br label %h1

end:
  ret i16 %j
}
";

/// What a loop knows is read from the loop its header's dominator is in, as
/// well as from the blocks above: a change in the first of two such loops to a
/// value the second never reads left the second's facts, which carry it, stale.
#[test]
fn a_change_to_a_loop_reworks_the_loop_that_starts_from_it() {
    let mut module = parsed(&format!("{DOS}{DIRECT}"));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    let facts = |analyses: &mut Analyses, module: &llrm_mir::module::Module| {
        let function = function(module, "f");
        analyses
            .get::<super::Bounded>(&module.context, &layout, function)
            .as_ref()
            .as_ref()
            .map(ranges::Bounds::facts)
            .map_err(String::clone)
            .unwrap()
    };
    let before = facts(&mut analyses, &module);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let t = value(function, "t");
        let t =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(t)).unwrap();
        let four = Operand::Value(value(function, "four"));
        function.set_operand(t, 1, four);
        analyses.invalidate(&PreservedAnalyses::none());
    }
    let after = facts(&mut analyses, &module);
    assert_ne!(before, after, "the value the change reached");
    assert_eq!(
        after,
        facts(&mut Analyses::new(outer), &module),
        "what was brought up to date is what working every loop gives"
    );
}

/// A loop carries what is known of the values made above it, whether it reads
/// them or not: an instruction erased there, which no loop's blocks held, was
/// still in the loop's facts. Seven bench programs and four pipeline tests
/// found it.
#[test]
fn an_instruction_erased_above_a_loop_leaves_the_loops_facts() {
    let mut module = parsed(&format!("{DOS}{LIMITED}"));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    let facts = |analyses: &mut Analyses, module: &llrm_mir::module::Module| {
        let function = function(module, "f");
        analyses
            .get::<super::Bounded>(&module.context, &layout, function)
            .as_ref()
            .as_ref()
            .map(ranges::Bounds::facts)
            .map_err(String::clone)
            .unwrap()
    };
    let seven = {
        let function = function(&module, "f");
        value(function, "seven")
    };
    let before = facts(&mut analyses, &module);
    assert!(before.values().any(|known| known.contains_key(&seven)), "a loop carries it");
    {
        let (_, function) = module.function_mut("f").unwrap();
        let made = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| function.instruction(inst).result == Some(seven))
            .unwrap();
        function.erase(made).unwrap();
        analyses.invalidate(&PreservedAnalyses::none());
    }
    let after = facts(&mut analyses, &module);
    assert!(after.values().all(|known| !known.contains_key(&seven)), "gone from every loop");
    assert_eq!(
        after,
        facts(&mut Analyses::new(outer), &module),
        "what was brought up to date is what working every loop gives"
    );
}

/// Every pass that said it changed something dropped the pointer analyses,
/// though five in six of them (twelve call-effects runs a function in QCport)
/// were true after it: the change was to integers.
#[test]
fn an_integer_edit_leaves_the_pointer_analyses_as_they_were_and_a_pointer_edit_does_not() {
    let mut module = parsed(
        "declare void @use(ptr)\ndefine i32 @f(ptr %p, i32 %a) {\nentry:\n  %x = add i32 %a, 1\n  %y = mul i32 %x, 3\n  %q = getelementptr i32, ptr %p, i32 1\n  call void @use(ptr %q)\n  ret i32 %y\n}\n",
    );
    let id = module.named("f").expect("f");
    let llrm_mir::module::GlobalKind::Function(f) = &mut module.globals[id.0 as usize].kind else {
        panic!("a function")
    };
    let (x, y, q) = {
        let named = |name: &str| value(f, name);
        (named("x"), named("y"), named("q"))
    };
    let a = f.parameters()[1];
    f.take_changes();
    let before = f.mark();
    f.replace_all_uses_with(x, Operand::Value(a));
    let changes = f.changes_since(before).expect("on the log").to_vec();
    assert!(!changes.is_empty());
    let unaffected: [fn(&[llrm_mir::module::Change], &llrm_mir::context::Context, &llrm_mir::module::Function) -> bool;
        4] =
        [Pointers::unaffected, CallEffects::unaffected, super::Writes::unaffected, super::ExposedFrames::unaffected];
    for unaffected in unaffected {
        assert!(
            unaffected(&changes, &module.context, function(&module, "f")),
            "an integer edit moved the pointer analyses"
        );
    }
    let (f, _) = (function(&module, "f"), y);
    let gep = f.walk().map(|(_, one)| one).find(|one| f.instruction(*one).result == Some(q)).expect("the gep");
    assert!(
        !CallEffects::unaffected(&[llrm_mir::module::Change::Rewritten(gep)], &module.context, f),
        "an edit to an address left the pointer analyses as they were"
    );
}

/// Every edit found every body's calls, actuals and exposed frames again
/// (`Procedure::of` and `exposed_frames` per body per run, O(module) for an
/// edit to one: 190 M instructions of excess work at chain-32). A body not
/// edited since keeps them.
#[test]
fn an_edit_to_one_body_finds_that_bodys_calls_alone() {
    let mut module = parsed(&format!(
        "{DOS}define i16 @f(i16 %a) {{
b0:
  %x = add i16 %a, 1
  %y = mul i16 %x, 2
  ret i16 %y
}}
define i16 @h(i16 %a) {{
b0:
  %x = add i16 %a, 3
  %y = mul i16 %x, 4
  ret i16 %y
}}
"
    ));
    let program = llrm_mir::program::Program::new(vec![module.clone()], Rc::new(Neutral)).unwrap();
    let mut analyses = ModuleAnalyses::new(llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0));
    analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let (f, h) = (module.named("f").unwrap(), module.named("h").unwrap());
    let before = {
        let memo = analyses.memo::<super::SummariesMemo>();
        (memo.facts[&f].calls.clone().expect("found"), memo.facts[&h].calls.clone().expect("found"))
    };
    let (_, function) = module.function_mut("f").unwrap();
    let (x, a) = (value(function, "x"), function.parameters()[0]);
    function.replace_all_uses_with(x, Operand::Value(a));
    analyses.invalidate(&PreservedAnalyses::none());
    analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let memo = analyses.memo::<super::SummariesMemo>();
    assert!(Rc::ptr_eq(&before.1, memo.facts[&h].calls.as_ref().unwrap()), "an unedited body's calls were found again");
    assert!(!Rc::ptr_eq(&before.0, memo.facts[&f].calls.as_ref().unwrap()), "an edited body's calls were kept");
}

/// The calls kept were found as a plain unit sees them, which knows less than
/// the unit a body is summarized in (its globals' facts): screen.c's summaries
/// came out broader and the compile cost 4.6% more. What is kept is what the
/// summarized unit finds.
#[test]
fn the_calls_kept_are_those_the_summarized_unit_finds() {
    let module = parsed(&format!(
        "{DOS}@g = internal global i16 0
define internal void @take(ptr %p) {{
b0:
  store i16 1, ptr %p
  ret void
}}
define void @f() {{
b0:
  %q = alloca i16
  store ptr %q, ptr @g
  call void @take(ptr getelementptr (i8, ptr @g, i16 1))
  call void @take(ptr %q)
  ret void
}}
"
    ));
    let program = llrm_mir::program::Program::new(vec![module.clone()], Rc::new(Neutral)).unwrap();
    let mut analyses = ModuleAnalyses::new(llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0));
    let globals = analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let f = module.named("f").unwrap();
    let kept = format!("{:?}", analyses.memo::<super::SummariesMemo>().facts[&f].calls.as_ref().unwrap());
    let layout = layout(&module);
    let function = function(&module, "f");
    let shape = crate::cfg::Shape::of(function);
    let exposed = crate::memory::exposed_frames(&Unit::of(&module, &layout, function));
    let program = analyses.program().clone();
    let unit = super::summarized_in(&module, &program, Result::as_ref(&*globals).unwrap(), &shape, &exposed, function);
    assert_eq!(kept, format!("{:?}", crate::alias::CallFacts::of(&unit)));
}

/// Call facts found under one unit were served to a body summarized under
/// another (screen.c: +4.6% compile cost, found by the gate's worst-file rule).
/// What the memo kept was found under the globals' facts of the run before:
/// when those are other facts the entry is dropped and found again under the
/// unit of this run, and the answer is a fresh computation under its own unit.
#[test]
fn call_facts_found_under_other_globals_facts_are_found_again() {
    let module = parsed(&format!(
        "{DOS}@g = internal global i16 0
define internal void @take(ptr %p) {{
b0:
  store i16 1, ptr %p
  ret void
}}
define void @f() {{
b0:
  call void @take(ptr @g)
  ret void
}}
"
    ));
    let program = llrm_mir::program::Program::new(vec![module.clone()], Rc::new(Neutral)).unwrap();
    let mut analyses = ModuleAnalyses::new(llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0));
    analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let f = module.named("f").unwrap();
    let kept = analyses.memo::<super::SummariesMemo>().facts[&f].calls.clone().expect("found");
    // The same run again: the facts are the same, the entry stands.
    analyses.invalidate(&PreservedAnalyses::none());
    analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    assert!(
        Rc::ptr_eq(&kept, analyses.memo::<super::SummariesMemo>().facts[&f].calls.as_ref().unwrap()),
        "same facts, found again"
    );
    // Other globals' facts than the entry was made under: found again.
    analyses.memo::<super::SummariesMemo>().globals = Some(Rc::new(Ok(super::Globals::default())));
    analyses.invalidate(&PreservedAnalyses::none());
    let globals = analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let again = analyses.memo::<super::SummariesMemo>().facts[&f].calls.clone().expect("found");
    assert!(!Rc::ptr_eq(&kept, &again), "calls found under other facts were served to this run");
    let layout = layout(&module);
    let function = function(&module, "f");
    let shape = crate::cfg::Shape::of(function);
    let exposed = crate::memory::exposed_frames(&Unit::of(&module, &layout, function));
    let program = analyses.program().clone();
    let unit = super::summarized_in(&module, &program, Result::as_ref(&*globals).unwrap(), &shape, &exposed, function);
    assert_eq!(format!("{again:?}"), format!("{:?}", crate::alias::CallFacts::of(&unit)));
}

/// What each call does is nothing a dead load's erasure can change: the load
/// has no user left and is no call. A pointer load was a part of `CallEffects`'
/// declaration like any, so DCE worked the effects of every call out again to
/// the same answer (4209 of 7009 runs over QCport at -O1 came to what they
/// were).
#[test]
fn test_erasing_a_dead_pointer_load_does_not_work_the_call_effects_out_again() {
    let mut module = parsed(&format!(
        "{DOS}declare void @g(ptr)\n@p = global ptr null\n\ndefine void @f(ptr %a) {{\nb:\n  %dead = load ptr, ptr @p\n  call void @g(ptr %a)\n  ret void\n}}\n"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let (context, f) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    llrm_mir::passes::trace_recomputes(true);
    analyses.get::<CallEffects>(context, &layout, f);
    let entry = f.entry().expect("an entry");
    let load = f.block(entry).instructions()[0];
    f.erase(load).expect("an unused load");
    analyses.invalidate(&PreservedAnalyses::none());
    analyses.get::<CallEffects>(context, &layout, f);
    let counts = llrm_mir::passes::recomputes();
    llrm_mir::passes::trace_recomputes(false);
    let of_effects: Vec<_> =
        counts.iter().filter(|(name, ..)| *name == "call-effects").map(|&(_, _, how, n)| (how, n)).collect();
    assert_eq!(of_effects, vec![("first", 1), ("replayed", 1)], "{counts:?}");
}

/// `Pointers` and `CallEffects` each solved where every pointer value points,
/// then asked what escapes (the call arguments and captures enter only there):
/// 34,762 value solves over QCport at -O1, 11.0% of the compile, for 6,209 body
/// states. The two ask one analysis for the solve.
#[test]
fn test_pointers_and_call_effects_of_a_body_solve_its_pointer_values_once() {
    let mut module = parsed(&format!(
        "{DOS}declare void @g(ptr)\n\ndefine void @f(ptr %a) {{\nb:\n  %p = getelementptr i8, ptr %a, i16 2\n  call void @g(ptr %p)\n  ret void\n}}\n"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let (context, f) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    let before = crate::alias::value_solves();
    analyses.get::<Pointers>(context, &layout, f);
    analyses.get::<CallEffects>(context, &layout, f);
    assert_eq!(crate::alias::value_solves() - before, 1, "the value solve was made again");
}
