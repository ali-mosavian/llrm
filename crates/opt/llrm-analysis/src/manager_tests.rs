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

use super::{
    Annotated, CallEffects, DominatedEdges, GlobalsAA, MemorySsa, Pointers, Registers, Summaries, TableLoads,
    ThroughMemory,
};
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
            assert_eq!(*analyses.get::<Registers>(&module.context, &layout, function), consts::known(&unit), "{at}");
            assert_eq!(
                analyses
                    .get::<DominatedEdges>(&module.context, &layout, function)
                    .as_ref()
                    .as_ref()
                    .map(|states| states.blocks(&unit, &unit.registers()))
                    .map_err(String::clone),
                ranges::dominated_edges(&unit),
                "{at}"
            );
            let effects = alias::calls_annotated(&Procedure::of(unit), summaries);
            assert_eq!(*analyses.get::<CallEffects>(&module.context, &layout, function), effects, "{at}");
            let calls: Calls =
                effects.unwrap().into_iter().map(|(at, effect)| (at, effect.stores)).collect::<IndexMap<_, _>>();
            let through = analyses.get::<ThroughMemory>(&module.context, &layout, function);
            assert_eq!(*through, Ok(consts::known_walked(&unit, &calls)), "{at}");
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

/// Each entry whose summary grew woke every caller of something unknown, the
/// bodies still queued among them: visits grew with entries squared (102 for
/// 12 entries; GORILLA's `summaries visit` was 26% of compiling it at -O1).
#[test]
fn test_callbacks_are_taken_once_the_queued_bodies_are_visited() {
    let count: usize = 12;
    let globals: String = (0..count).map(|at| format!("@g{at} = internal global i16 0\n")).collect();
    let bodies: String = (0..count)
        .map(|at| {
            format!(
                "define internal void @h{at}() {{\nentry:\n  store i16 {at}, ptr @g{at}\n  ret void\n}}\n\
                 define void @f{at}() {{\nentry:\n  call void @h{at}()\n  call void @ext()\n  ret void\n}}\n"
            )
        })
        .collect();
    let module = parsed(&format!("{DOS}declare void @ext()\n{globals}\n{bodies}"));
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    let before = alias::visits();
    analyses.get::<Summaries>(&module);
    let visits = alias::visits() - before;
    assert!(visits <= 4 * count, "{visits} visits for {count} entries");
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
        assert_eq!(trips(&before, "h1", function), Some(crate::num::Num::from(5)));
        let lim = value(function, "lim");
        let lim =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(lim)).unwrap();
        let seven = Operand::Value(value(function, "seven"));
        function.set_operand(lim, 0, seven);
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let (context, function) = module.function_mut("f").unwrap();
    let proved = crate::induction::proved();
    let after = analyses.get::<super::Counted>(context, &layout, function);
    // The check derives them afresh to compare, and is counted.
    if !llrm_support::env_set("LLRM_CHECK_REPLAY") {
        assert_eq!(crate::induction::proved() - proved, 1, "only the loop the change reaches is proved again");
    }
    assert_eq!(trips(&after, "h1", function), Some(crate::num::Num::from(7)), "the loop that reads the changed bound");
    assert_eq!(
        *after,
        *Analyses::new(outer).get::<super::Counted>(context, &layout, function),
        "what was brought up to date is what deriving it afresh gives"
    );
}

/// A branch's bound changed outside the blocks it narrows: the edge facts of
/// the blocks past it were kept stale. What is asked after is what working
/// every block gives.
#[test]
fn a_change_to_a_branch_bound_reworks_the_blocks_past_it() {
    let mut module = parsed(&format!("{DOS}{LIMITED}"));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    let states = |analyses: &mut Analyses, module: &llrm_mir::module::Module| {
        let function = function(module, "f");
        let unit = crate::testing::with_registers(Unit::of(module, &layout, function));
        analyses
            .get::<DominatedEdges>(&module.context, &layout, function)
            .as_ref()
            .as_ref()
            .map(|held| held.blocks(&unit, &unit.registers()))
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
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let after = states(&mut analyses, &module);
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
        let held = super::Ranges::of(&module.context, &layout, function, analyses, true);
        let unit = held.unit(&module.context, &layout, function, analyses.outer());
        match held.bounds() {
            Some(Ok(bounds)) => bounds.complete(&unit),
            _ => panic!("bounds"),
        }
    };
    let before = facts(&mut analyses, &module);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let t = value(function, "t");
        let t =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(t)).unwrap();
        let four = Operand::Value(value(function, "four"));
        function.set_operand(t, 1, four);
        analyses.invalidate(function, &PreservedAnalyses::none());
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
        let held = super::Ranges::of(&module.context, &layout, function, analyses, true);
        let unit = held.unit(&module.context, &layout, function, analyses.outer());
        match held.bounds() {
            Some(Ok(bounds)) => bounds.complete(&unit),
            _ => panic!("bounds"),
        }
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
        analyses.invalidate(function, &PreservedAnalyses::none());
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
    analyses.changed(f);
    analyses.invalidate(&PreservedAnalyses::none());
    analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let memo = analyses.memo::<super::SummariesMemo>();
    assert_eq!(super::runs_from_the_log(), 1, "an edit reported to one body made a scan of every body's mark");
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
    let unit =
        super::summarized_in(&module, &program, Result::as_ref(&*globals).unwrap(), &shape, &exposed, None, function);
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
    let unit =
        super::summarized_in(&module, &program, Result::as_ref(&*globals).unwrap(), &shape, &exposed, None, function);
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
    let load = f.block(entry).first().expect("a block with an instruction");
    f.erase(load).expect("an unused load");
    analyses.invalidate(f, &PreservedAnalyses::none());
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

/// A `ThroughMemory` kept past an edit as sound knows no more than it did:
/// `%n` replaced by the 7 it was found equal to leaves the loaded `%v` unknown
/// in it, and a fresh solve has `%v = 7`. Which a pass reads depended on
/// whether something had asked for it earlier in the round: matmul bq32 -O3
/// came out 18 KB of asm with `floatloop` asking it and 34 KB without (#1909).
/// A pass whose decisions follow what is known asks for the body as it stands.
#[test]
fn test_a_pass_that_follows_what_is_known_reads_it_for_the_body_as_it_stands() {
    let mut module = parsed(&format!(
        "{DOS}@g = global i16 0\n\ndefine i16 @f(i16 %n) {{\nb:\n  store i16 %n, ptr @g\n  %v = load i16, ptr @g\n  ret i16 %v\n}}\n"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let (context, f) = module.function_mut("f").expect("@f");
    let (n, v) = (crate::testing::value(f, "n"), crate::testing::value(f, "v"));
    let mut analyses = Analyses::new(outer);
    analyses.get::<ThroughMemory>(context, &layout, f);
    let seven = Operand::Constant(context.int(f.value(n).ty, 7));
    f.replace_value(n, seven);
    analyses.invalidate(f, &PreservedAnalyses::none());
    let known = |result: &Rc<<ThroughMemory as Analysis>::Result>| {
        Result::as_ref(&**result).unwrap().get(&v).map(|fact| fact.n.clone())
    };
    assert_eq!(known(&analyses.get_exact::<ThroughMemory>(context, &layout, f)), Some(7.into()));
}

/// `Annotated` and the `Held` a unit carries asked `Pointers`, so each body
/// state paid for the escape phase (a walk of every instruction and a dataflow
/// over the blocks) of which they read nothing: 88 of d_faces' -O1 compile's
/// body states, 42 M of 3,337 M. They read the pointer values.
#[test]
fn test_annotated_does_not_solve_what_escapes() {
    let mut module = parsed(&format!(
        "{DOS}declare void @g(ptr)\n\ndefine void @f(ptr %a) {{\nb:\n  %p = getelementptr i8, ptr %a, i16 2\n  %v = load i8, ptr %p\n  call void @g(ptr %p)\n  ret void\n}}\n"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let (context, f) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    let before = crate::alias::escape_solves();
    analyses.get::<Annotated>(context, &layout, f);
    super::Held::of(context, &layout, f, &mut analyses, true);
    assert_eq!(crate::alias::escape_solves() - before, 0, "Annotated or Held solved what escapes");
    analyses.get::<Pointers>(context, &layout, f);
    assert_eq!(crate::alias::escape_solves() - before, 1, "Pointers solves what escapes once");
}

/// `Annotated` worked out every value's stride congruence (`congruences_of`,
/// a pass over the loops' inductions) in every solve, for the accesses with a
/// variable index alone to ask: 24.0 M of d_faces' 3,218 M at -O1, 123 solves
/// for 4,185 asks. A body with none asks none.
#[test]
fn test_annotated_asks_no_strides_of_accesses_without_a_variable_index() {
    let mut module = parsed(&format!(
        "{DOS}define i16 @f(i16 %i) {{\nb:\n  %a = alloca [8 x i16]\n  %m = and i16 %i, 7\n  %q = getelementptr i16, ptr %a, i16 %m\n  %w = load i16, ptr %q\n  ret i16 %w\n}}\n\ndefine i16 @g(ptr %a) {{\nb:\n  %p = getelementptr i8, ptr %a, i16 2\n  %v = load i16, ptr %p\n  ret i16 %v\n}}\n"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    let before = crate::alias::congruences_found();
    let (context, g) = module.function_mut("g").expect("@g");
    analyses.get::<Annotated>(context, &layout, g);
    assert_eq!(crate::alias::congruences_found() - before, 0, "strides worked out for no access that asks");
    let mut analyses = Analyses::new(outer);
    let (context, f) = module.function_mut("f").expect("@f");
    analyses.get::<Annotated>(context, &layout, f);
    assert_eq!(crate::alias::congruences_found() - before, 1, "a variable index asks once");
}

/// A pass names what it preserved and the manager drops the rest, whenever the
/// result was derived (LLVM's `PreservedAnalyses`): one asked after the pass's
/// last edit was derived from the body as it stands, and was kept by a rule
/// that read the edit log (383 of 2,248 results dropped at an invalidation
/// over d_faces -O2, 151 after `dead` alone). Now it is derived again when next
/// asked: a cost (+40 computations on d_faces -O2), never a stale fact.
#[test]
fn test_a_result_derived_from_the_body_as_it_stands_is_dropped_at_an_invalidation_that_does_not_name_it() {
    let mut module = parsed(&format!(
        "{DOS}declare void @g(ptr)\n\ndefine void @f(ptr %a) {{\nb:\n  call void @g(ptr %a)\n  ret void\n}}\n"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let (context, f) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    llrm_mir::passes::trace_recomputes(true);
    analyses.get::<CallEffects>(context, &layout, f);
    analyses.invalidate(f, &PreservedAnalyses::none());
    analyses.get::<CallEffects>(context, &layout, f);
    let counts = llrm_mir::passes::recomputes();
    llrm_mir::passes::trace_recomputes(false);
    let of_effects: Vec<_> =
        counts.iter().filter(|(name, ..)| *name == "call-effects").map(|&(_, _, how, n)| (how, n)).collect();
    assert_eq!(of_effects, vec![("first", 1), ("replayed", 1)], "{counts:?}");
}

/// A scalar written in each arm of N joins is reached by up to N stores at
/// every load after, and the cell of each was decoded again for each load that
/// store reaches (N^2 decodes over the body; 85 K instructions a load at
/// `joins` 64, 58% of a `ThroughMemory` solve). Once for a store.
#[test]
fn test_a_store_s_cell_is_decoded_once_however_many_loads_it_reaches() {
    let joins = 12;
    let mut text = format!("{DOS}@g = global i16 0\n\ndefine i16 @f(ptr %g, ");
    text.push_str(&(0..joins).map(|n| format!("i1 %c{n}")).collect::<Vec<_>>().join(", "));
    text.push_str(") {\nb:\n  store i16 99, ptr %g\n  br label %t0\n");
    for n in 0..joins {
        text += &format!(
            "t{n}:\n  br i1 %c{n}, label %w{n}, label %j{n}\nw{n}:\n  store i16 {n}, ptr %g\n  br label %j{n}\nj{n}:\n  %l{n} = load i16, ptr %g\n  br label %{}\n",
            if n + 1 < joins { format!("t{}", n + 1) } else { "end".to_owned() }
        );
    }
    text.push_str("end:\n  ret i16 %l11\n}\n");
    let mut module = parsed(&text);
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let (context, f) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    let before = crate::consts::stores_decoded();
    analyses.get::<ThroughMemory>(context, &layout, f);
    let decoded = crate::consts::stores_decoded() - before;
    assert!(decoded > 0, "premise: the loads were served");
    assert!(decoded <= 2 * joins, "{decoded} decodes for {joins} stores and {joins} loads");
}

/// `Summaries` solved where every pointer points for the direct summary, for
/// the calls' facts and for each summarized call: 4 to 5 value solves a body
/// state, 11.0% of QCport's -O1 compile with the rest of the family. One a
/// body.
#[test]
fn summaries_solve_the_pointer_values_of_a_body_once() {
    let module = parsed(
        "declare void @g(ptr)\n\ndefine void @f(ptr %a) {\nentry:\n  %p = getelementptr i8, ptr %a, i16 2\n  call void @g(ptr %p)\n  call void @g(ptr %a)\n  ret void\n}\n",
    );
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    analyses.get::<GlobalsAA>(&module);
    let before = crate::alias::value_solves();
    analyses.get::<Summaries>(&module);
    assert_eq!(crate::alias::value_solves() - before, 1, "Summaries solved a body's pointer values again");
}

/// What calling back into the module does was added up from every entry's
/// summary for each body that asked (and again for each entry whose summary
/// changed): entries times bodies, 126 and 89 Ginstr of the 385 compiling 4,096
/// small functions at -O2 (call-effects and summaries callbacks, slope 2.0
/// each). Made once, and one entry's change folded in alone.
#[test]
fn test_callbacks_are_added_up_once_for_every_body_that_asks() {
    const BODIES: usize = 24;
    let mut text = String::from("declare void @ext()\n\n");
    for at in 0..BODIES {
        text.push_str(&format!("define void @f{at}() {{\nb0:\n  call void @ext()\n  ret void\n}}\n\n"));
    }
    let module = parsed(&text);
    let layout = layout(&module);
    let mut outer = Outer::of(&module, None);
    outer.require::<GlobalsAA>(&module);
    outer.require::<Summaries>(&module);
    outer.require::<super::Callbacks>(&module);
    let outer = Rc::new(outer);
    let before = alias::callback_entries();
    for at in 0..BODIES {
        let (_, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some(&format!("f{at}"))).unwrap();
        Analyses::new(Rc::clone(&outer)).get::<CallEffects>(&module.context, &layout, function);
    }
    let added = alias::callback_entries() - before;
    assert!(added <= 6 * BODIES, "{added} entries instantiated for {BODIES} bodies");
}

/// The summaries fixed point added the callbacks up from every entry again
/// whenever an entry's summary changed: entries times changes. An entry that
/// changed is folded in alone.
#[test]
fn test_summaries_fold_one_changed_entry_into_the_callbacks_not_all_of_them() {
    const BODIES: usize = 24;
    let mut text = String::from("declare void @ext()\n\n");
    for at in 0..BODIES {
        text.push_str(&format!("define void @f{at}() {{\nb0:\n  call void @ext()\n  ret void\n}}\n\n"));
    }
    let module = parsed(&text);
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    analyses.get::<GlobalsAA>(&module);
    let before = alias::callback_entries();
    analyses.get::<Summaries>(&module);
    let added = alias::callback_entries() - before;
    assert!(added <= 4 * BODIES, "{added} entries instantiated for {BODIES} bodies");
}

/// A module pass spliced a loop into a caller and did not say so (`changed`),
/// and the next module analysis to ask for the caller's shape was given the one
/// from before: no block of the loop in it, no back edge to widen at, and a
/// value solve that gained a slice a round for ever (qb-runtime's i8out.c,
/// minutes in GlobalsAA). An analysis of a function is not returned as it stood
/// before an edit that was not reported.
#[test]
fn test_a_shape_asked_after_an_unreported_splice_is_the_shape_after_it() {
    let mut module = parsed(
        "define internal void @copy(ptr %a, i16 %n) {
entry:
  br label %head

head:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %body, label %exit

body:
  %p = getelementptr inbounds i8, ptr %a, i16 %i
  store i8 0, ptr %p
  %next = add i16 %i, 1
  br label %head

exit:
  ret void
}

define void @f(ptr %x, i16 %m) {
entry:
  call void @copy(ptr %x, i16 %m)
  ret void
}
",
    );
    let (copy, f) = (module.named("copy").unwrap(), module.named("f").unwrap());
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    let before = analyses.function::<Shape>(&module, f);
    assert!(before.loops.is_empty(), "premise: the caller has no loop");
    let body = module.global(copy).function().unwrap().clone();
    let call = {
        let function = module.global(f).function().unwrap();
        function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| llrm_mir::memory::callee(&module.context, function, inst) == Some(copy))
            .unwrap()
    };
    let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[f.0 as usize].kind else {
        unreachable!()
    };
    llrm_mir::splice::splice(&mut module.context, function, call, &body);
    let after = analyses.function::<Shape>(&module, f);
    assert_eq!(*after, Shape::of(module.global(f).function().unwrap()), "the shape from before the splice");
    assert_eq!(after.loops.len(), 1);
}

/// The declarations every function analysis reads are checked, after a pass
/// over one body, against that body alone (`body_edited`): scanning every
/// global after each of N bodies is N squared (chain, `mir interprocedural` own
/// 101 -> 384 -> 1,487 M at N=128, 256, 512).
#[test]
fn test_after_one_body_is_edited_only_its_declaration_is_looked_at() {
    let mut module = parsed(
        "define void @a() {
entry:
  ret void
}

define void @b() {
entry:
  ret void
}
",
    );
    let (a, b) = (module.named("a").unwrap(), module.named("b").unwrap());
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    let nounwind = |outer: &Outer, id: llrm_mir::context::GlobalId| {
        llrm_mir::facts::Facts::of(&outer.globals[id.0 as usize].function().expect("a function").attrs)
            .contains(llrm_mir::facts::Fact::NoUnwind)
    };
    let first = analyses.outer(&module);
    assert!(!nounwind(&first, b));
    // The body reported is looked at: its new attribute is in the declarations.
    module.function_mut("b").unwrap().1.attrs.extend(llrm_mir::facts::Fact::NoUnwind.attribute());
    analyses.body_edited(b, &PreservedAnalyses::none());
    assert!(nounwind(&analyses.outer(&module), b), "the edited body's declaration was not read again");
    // One not reported is not: a pass over `b` says nothing of `a`.
    module.function_mut("a").unwrap().1.attrs.extend(llrm_mir::facts::Fact::NoUnwind.attribute());
    analyses.body_edited(b, &PreservedAnalyses::none());
    assert!(!nounwind(&analyses.outer(&module), a), "every global was looked at after one body's edit");
    // Anything else dropped (`invalidate`) looks at all.
    analyses.invalidate(&PreservedAnalyses::none());
    assert!(nounwind(&analyses.outer(&module), a));
}

/// `LLRM_CHECK_SUMMARIES` compares the facts kept for every body with those a
/// run from nothing finds: a stale entry (calls of an edited body kept) must
/// not pass, or the check says nothing about the incremental run.
#[test]
fn the_summaries_check_refuses_a_kept_fact_other_than_scratch_finds() {
    let module = parsed(&format!(
        "{DOS}define internal void @take(ptr %p) {{
b0:
  store i16 1, ptr %p
  ret void
}}
define void @f(ptr %q) {{
b0:
  call void @take(ptr %q)
  ret void
}}
"
    ));
    let program = llrm_mir::program::Program::new(vec![module.clone()], Rc::new(Neutral)).unwrap();
    let mut analyses = ModuleAnalyses::new(llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0));
    let globals = analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let program = analyses.program().clone();
    let globals = Result::as_ref(&*globals).unwrap();
    let f = module.named("f").unwrap();
    let mut facts = std::mem::take(&mut analyses.memo::<super::SummariesMemo>().facts);
    super::Table::of(&module, &program, globals, &facts, None).check_against_scratch();
    let stale = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut other = facts.clone();
        other[&f].calls = Some(Rc::new(crate::alias::CallFacts::default()));
        super::Table::of(&module, &program, globals, &other, None).check_against_scratch();
    }));
    facts.clear();
    assert!(stale.is_err(), "a body whose kept calls are not scratch's passed the check");
}

/// A body whose parameters changed with no instruction touched kept its
/// point values: `remove_parameter` logged nothing, so the body's mark stood
/// and `Summaries` kept facts naming the parameter by its old position
/// (`LLRM_CHECK_SUMMARIES` on QCport -Omax: _ent_lights_apply@3.constprop.1
/// differed from scratch; a caller instantiating the summary read the wrong
/// actual).
#[test]
fn a_body_with_a_parameter_removed_has_its_point_values_made_again() {
    let mut module = parsed(&format!(
        "{DOS}define void @f(ptr %unused, ptr %p) {{
b0:
  store i16 1, ptr %p
  ret void
}}
"
    ));
    let program = llrm_mir::program::Program::new(vec![module.clone()], Rc::new(Neutral)).unwrap();
    let mut analyses = ModuleAnalyses::new(llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0));
    analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let f = module.named("f").unwrap();
    let before = analyses.memo::<super::SummariesMemo>().facts[&f].mark;
    {
        let llrm_mir::module::Module { context, globals, .. } = &mut module;
        let llrm_mir::module::GlobalKind::Function(function) = &mut globals[f.0 as usize].kind else {
            panic!("a function")
        };
        function.remove_parameter(context, 0);
    }
    assert_ne!(before, function(&module, "f").mark(), "removing a parameter left the body's history as it was");
    analyses.changed(f);
    analyses.invalidate(&PreservedAnalyses::none());
    let globals = analyses.get::<super::GlobalsAA>(&module);
    analyses.get::<super::Summaries>(&module);
    let program = analyses.program().clone();
    let globals = Result::as_ref(&*globals).unwrap();
    let facts = std::mem::take(&mut analyses.memo::<super::SummariesMemo>().facts);
    super::Table::of(&module, &program, globals, &facts, None).check_against_scratch();
}

const TABLE: &str = "@t = internal constant [4 x i16] [i16 1, i16 2, i16 3, i16 4]

define i16 @f(i16 %i) {
b0:
  %p = getelementptr inbounds i16, ptr @t, i16 %i
  %x = load i16, ptr %p
  %y = add i16 %x, 1
  ret i16 %y
}
";

/// A pass that points the table load's index at a constant, keeping nothing.
fn pin_the_index() -> Step {
    step(
        |unit, _| {
            let function = &mut *unit.function;
            let gep = function
                .walk()
                .map(|(_, inst)| inst)
                .find(|&inst| matches!(
                    function.instruction(inst).opcode,
                    Opcode::GetElementPtr { .. }
                ))
                .unwrap();
            let mut operands: Vec<Operand> = function.instruction(gep).operands.to_vec();
            let last = operands.len() - 1;
            let ty = unit.context.types.int(16);
            operands[last] = Operand::Constant(unit.context.int(ty, 1));
            function.set_operands(gep, operands.into());
        },
        PreservedAnalyses::none(),
    )
}

/// `Registers` walked the whole body for its table loads at every update:
/// 129 walks on d_faces -O1, 32 M of its 66 M. Asked after a change, it asks
/// only of the loads the change reaches.
#[test]
fn a_registers_update_does_not_walk_the_body_for_its_table_loads() {
    let mut module = parsed(&format!("{DOS}{TABLE}"));
    let walks = Rc::new(RefCell::new(Vec::new()));
    let ask = |walks: &Rc<RefCell<Vec<usize>>>| {
        let walks = Rc::clone(walks);
        step(
            move |unit, analyses| {
                analyses.get::<Registers>(unit.context, unit.layout, unit.function);
                walks.borrow_mut().push(consts::table_walks());
            },
            PreservedAnalyses::all(),
        )
    };
    let mut passes = PassManager::default();
    passes.add(ask(&walks));
    passes.add(pin_the_index());
    passes.add(ask(&walks));
    passes.add(step(
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
        PreservedAnalyses::none(),
    ));
    passes.add(ask(&walks));
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    let walks = walks.take();
    assert_eq!(walks[2], walks[1], "the second update walked the body again: {walks:?}");
}

/// A load whose index becomes a constant is no longer a table load by an
/// index: the update says so, as a new walk does.
#[test]
fn a_table_load_update_drops_the_load_whose_index_went() {
    let mut module = parsed(&format!("{DOS}{TABLE}"));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let ask = |seen: &Rc<RefCell<Vec<(usize, usize)>>>| {
        let seen = Rc::clone(seen);
        step(
            move |unit, analyses| {
                let held = analyses.get::<TableLoads>(unit.context, unit.layout, unit.function);
                let mut again = consts::table_loads(&crate::memory::Unit::within(
                    unit.context,
                    unit.layout,
                    unit.function,
                    analyses.outer(),
                ));
                again.sort_unstable();
                assert_eq!(*held, again);
                seen.borrow_mut().push((held.len(), again.len()));
            },
            PreservedAnalyses::all(),
        )
    };
    let mut passes = PassManager::default();
    passes.add(ask(&seen));
    passes.add(pin_the_index());
    passes.add(ask(&seen));
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    assert_eq!(seen.take(), vec![(1, 1), (0, 0)]);
}

/// A memory view over the module's facts read what calling back into the module
/// does when it was made, so every pass that asked for one was run again each
/// time the callbacks' summary moved: d_faces -O3 ran 19 passes twice more to
/// find nothing. It is read where it is asked for.
#[test]
fn a_memory_view_reads_the_callbacks_where_they_are_asked_for() {
    use llrm_mir::passes::read;
    let module = parsed(&format!("{DOS}define void @f() {{\nb0:\n  ret void\n}}\n"));
    let mut outer = Outer::of(&module, None);
    outer.require::<super::Callbacks>(&module);
    let function = module.global(module.named("f").expect("f")).function().expect("a function");
    let layout = layout(&module);
    read::swap(0);
    let unit = Unit::within(&module.context, &layout, function, &outer);
    assert_eq!(read::swap(0) & read::CALLBACKS, 0, "making the view reads no callbacks");
    unit.callbacks();
    assert_eq!(read::swap(0) & read::CALLBACKS, read::CALLBACKS, "asking for them does");
}

/// Each pass that edited threw away what was known through memory, though
/// finding a value again (`replace_value`) and erasing what it replaced keep
/// every other value's meaning: `d_faces` -O1 derived it 58 times where 29 of
/// the 36 repeats gave what it had. A result that is sound stands past such
/// edits, and is derived again past one that changes a meaning.
#[test]
fn a_result_kept_as_sound_stands_past_edits_that_keep_every_values_meaning() {
    let mut module = parsed(&format!(
        "{DOS}define i16 @f(i16 %x) {{
entry:
  %a = add i16 %x, 1
  %b = add i16 %x, 1
  %c = add i16 %a, %b
  ret i16 %c
}}
"
    ));
    let layout = layout(&module);
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let (context, function) = module.function_mut("f").unwrap();
    let first = analyses.get::<ThroughMemory>(context, &layout, function);
    let inst_of = |function: &llrm_mir::module::Function, name: &str| {
        let value = value(function, name);
        function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(value)).unwrap()
    };
    // %b is %a: found again and replaced, then erased.
    let (a, b) = (value(function, "a"), inst_of(function, "b"));
    function.replace_value(value(function, "b"), Operand::Value(a));
    function.erase(b).expect("unused now");
    analyses.invalidate(function, &PreservedAnalyses::none());
    let kept = analyses.get::<ThroughMemory>(context, &layout, function);
    assert!(Rc::ptr_eq(&first, &kept), "an edit that keeps every meaning derived it again");
    // %a is made another number: a meaning changed.
    let one = Operand::Value(value(function, "x"));
    function.set_operand(inst_of(function, "a"), 0, one);
    analyses.invalidate(function, &PreservedAnalyses::none());
    let again = analyses.get::<ThroughMemory>(context, &layout, function);
    assert!(!Rc::ptr_eq(&kept, &again), "an operand set to another value kept what was said of it");
}

/// A terminator rewritten in one loop was a change to the CFG, and every loop
/// of the body was proved again (`unrolled` at -O3: 62 of the 91 reruns, 1155
/// M of the 24.5 G at 64 loops). Only the loop it is in, and those it can
/// guard or enter, are.
#[test]
fn a_terminator_rewritten_in_one_loop_proves_that_loop_again_and_not_the_other() {
    let mut module = parsed(&format!("{DOS}{LIMITED}"));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    {
        let (context, function) = module.function_mut("f").unwrap();
        analyses.get::<super::Counted>(context, &layout, function);
        let latch = crate::testing::block(function, "l0");
        let branch = function.terminator(latch).expect("a latch ends in a branch");
        let same = function.instruction(branch).operands.clone();
        function.set_operands(branch, same);
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let (context, function) = module.function_mut("f").unwrap();
    let proved = crate::induction::proved();
    let after = analyses.get::<super::Counted>(context, &layout, function);
    if !llrm_support::env_set("LLRM_CHECK_REPLAY") {
        assert_eq!(crate::induction::proved() - proved, 1, "only the loop holding the terminator is proved again");
    }
    assert_eq!(
        *after,
        *Analyses::new(outer).get::<super::Counted>(context, &layout, function),
        "what was brought up to date is what deriving it afresh gives"
    );
}

/// Every edit worked out which allocas of the function are exposed from
/// nothing (229 times, 175 Minstr, in a 63-loop function). The allocas behind
/// what the edit read are asked again, the others stand.
#[test]
fn an_edit_asks_only_the_allocas_behind_what_it_reads_whether_they_are_exposed() {
    let mut module = parsed(&format!(
        "{DOS}define void @f() {{
b0:
  %a = alloca i16
  %b = alloca i16
  %c = alloca i16
  store i16 1, ptr %a
  store i16 2, ptr %b
  store ptr %c, ptr %c
  ret void
}}
"
    ));
    let layout = layout(&module);
    let outer = Rc::new(Outer::of(&module, None));
    let mut analyses = Analyses::new(Rc::clone(&outer));
    {
        let (context, function) = module.function_mut("f").unwrap();
        let before = analyses.get::<super::ExposedFrames>(context, &layout, function);
        assert_eq!(before.len(), 1, "%c is stored");
        // The store of %b's value to %b's address now stores %b's address.
        let store = function.iter_block(function.layout()[0]).nth(4).unwrap();
        let b = Operand::Value(value(function, "b"));
        function.set_operand(store, 0, b);
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let (context, function) = module.function_mut("f").unwrap();
    let (whole, asked) = (crate::frameescape::whole_runs(), crate::frameescape::scans());
    let after = analyses.get::<super::ExposedFrames>(context, &layout, function);
    if !llrm_support::env_set("LLRM_CHECK_REPLAY") {
        assert_eq!(crate::frameescape::whole_runs() - whole, 0, "the function's allocas were all asked again");
        assert!(crate::frameescape::scans() - asked <= 1, "only %b is behind what the edit read");
    }
    assert_eq!(after.len(), 2, "%b and %c");
    assert_eq!(
        *after,
        *Analyses::new(outer).get::<super::ExposedFrames>(context, &layout, function),
        "what was brought up to date is what deriving it afresh gives"
    );
}

/// A branch sent to other blocks was judged to leave the pointer analyses as
/// they were (a branch is no pointer), so `PointsTo` kept the predecessor
/// lists of the old edges: `LLRM_CHECK_REPLAY` on tests/run static_inline under
/// the LLVM order found `escaped_before` of block 1 with preds [5, 2] where
/// deriving it gave [5]. Here the object escapes in `b`, which the branch then
/// stops reaching `c`.
#[test]
fn a_branch_sent_to_other_blocks_makes_the_pointer_analyses_derive_again() {
    let mut module = parsed(
        "declare void @use(ptr)\ndeclare void @peek()\ndefine void @f(i1 %k) {\nentry:\n  %s = alloca i32\n  br i1 %k, label %a, label %b\na:\n  br label %c\nb:\n  call void @use(ptr %s)\n  br label %c\nc:\n  call void @peek()\n  ret void\n}\n",
    );
    let id = module.named("f").expect("f");
    let llrm_mir::module::GlobalKind::Function(f) = &mut module.globals[id.0 as usize].kind else {
        panic!("a function")
    };
    let br = f
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(f.instruction(inst).opcode, Opcode::Br) && f.instruction(inst).operands.len() == 3)
        .expect("the branch");
    let a = match f.instruction(br).operands[1] {
        Operand::Block(a) => a,
        _ => panic!("a block"),
    };
    f.take_changes();
    let before = f.mark();
    f.set_operand(br, 2, Operand::Block(a));
    let changes = f.changes_since(before).expect("on the log").to_vec();
    for (name, unaffected) in [
        (
            "points-to",
            Pointers::unaffected
                as fn(&[llrm_mir::module::Change], &llrm_mir::context::Context, &llrm_mir::module::Function) -> bool,
        ),
        ("call-effects", CallEffects::unaffected),
        ("writes", super::Writes::unaffected),
    ] {
        assert!(
            !unaffected(&changes, &module.context, function(&module, "f")),
            "{name} kept past a branch sent elsewhere"
        );
    }
}

/// `LLRM_CHECK_CAPTURES` judges the points-to facts each time `get` hands them
/// out, so a fact a pass reads (dse dropping a store behind `phi [%slot, %q]`)
/// is checked where it is used: a planted result that forgot the escape of
/// the slot is refused by the audit `get` runs.
#[test]
fn the_audit_of_points_to_refuses_facts_that_forgot_a_join_with_an_unknown_arm() {
    let module = parsed(&format!(
        "{DOS}define i32 @f(i1 %c, ptr %cell) {{\nentry:\n  %s = alloca i32\n  %q = load ptr, ptr %cell\n  br i1 %c, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %p = phi ptr [ %s, %a ], [ %q, %b ]\n  %v = load i32, ptr %p\n  ret i32 %v\n}}\n"
    ));
    let layout = layout(&module);
    let f = function(&module, "f");
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let mut facts = (*analyses.get::<Pointers>(&module.context, &layout, f)).clone().expect("facts");
    facts.escaped.clear();
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Pointers::audit(&Ok(facts), &module.context, &layout, f, &mut analyses)
    }));
    assert!(refused.is_err(), "facts that lost the slot behind the join were accepted by the audit");
}

/// A gep replaced by another of the same operands that differs in `inbounds`
/// does not decompose alike (an inbounds one goes on through its index), and
/// the effects named it as a leaf where a fresh decomposition reads base and
/// index (qcport triggers -O1 with LLRM_CHECK_REPLAY, #2158). Equal geps still
/// do.
#[test]
fn a_gep_that_differs_in_inbounds_is_not_read_alike_and_an_equal_one_is() {
    let module = parsed(
        "define void @f(ptr %q, i16 %i) {\nentry:\n  %plain = getelementptr i8, ptr %q, i16 %i\n  %bounded = getelementptr inbounds i8, ptr %q, i16 %i\n  %again = getelementptr i8, ptr %q, i16 %i\n  ret void\n}\n",
    );
    let f = function(&module, "f");
    let (plain, bounded, again) = (value(f, "plain"), value(f, "bounded"), value(f, "again"));
    assert!(!super::decomposes_alike(f, plain, bounded), "an inbounds gep was read like a plain one");
    assert!(super::decomposes_alike(f, plain, again), "two equal geps were read apart");
}

/// The graph of a body is made once per body state and shared: the asks of
/// EarlyCSE and LICM in a row made it again each (the manager had no MemorySSA
/// analysis), and an edit that drops the analyses has it made again.
#[test]
fn test_the_memory_graph_is_made_once_per_body_state() {
    let mut module = parsed(&format!("{DOS}{TWO}"));
    let graphs = Rc::new(RefCell::new(Vec::new()));
    let ask_graph = |graphs: &Rc<RefCell<Vec<_>>>| {
        let graphs = Rc::clone(graphs);
        step(
            move |unit, analyses| {
                let graph = analyses.get::<MemorySsa>(unit.context, unit.layout, unit.function);
                graphs.borrow_mut().push((graph, ()));
            },
            PreservedAnalyses::all(),
        )
    };
    let mut passes = PassManager::default();
    passes.add(ask_graph(&graphs));
    passes.add(ask_graph(&graphs));
    passes.add(redirect(PreservedAnalyses::none()));
    passes.add(ask_graph(&graphs));
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    let graphs = graphs.take();
    let graph = |at: usize| &graphs[at].0;
    assert!(Rc::ptr_eq(graph(0), graph(1)), "two asks of one body state made two graphs");
    assert!(!Rc::ptr_eq(graph(1), graph(2)), "an edit left the old graph");
}
