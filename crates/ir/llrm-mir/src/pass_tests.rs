use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::context::Context;
use crate::context::GlobalId;
use crate::datalayout::DataLayout;
use crate::module::{Change, Function, GlobalKind, Module, Operand};
use crate::opcode::Attribute;
use crate::parse;
use crate::passes::{
    Analyses, Analysis, Dominators, FunctionPass, Loops, ModuleAnalyses, ModuleAnalysis, ModulePass, PassManager,
    PreservedAnalyses, Unit,
};
use crate::target::Neutral;

const TEXT: &str =
    "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %b\nb:\n  ret i16 0\n}\n";

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

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let function = &mut *unit.function;
        let entry = function.entry().unwrap();
        let branch = function.terminator(entry).unwrap();
        let [.., Operand::Block(_), Operand::Block(second)] = function.instruction(branch).operands[..] else {
            panic!("a conditional branch")
        };
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
    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> usize {
        COMPUTED.set(COMPUTED.get() + 1);
        function.layout().len()
    }
}

struct Look(PreservedAnalyses);

impl FunctionPass for Look {
    fn name(&self) -> &'static str {
        "look"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<Counted>(unit.context, unit.layout, unit.function);
        self.0.clone()
    }
}

struct DropReturn;

impl FunctionPass for DropReturn {
    fn name(&self) -> &'static str {
        "drop-return"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
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
    assert_eq!(
        passes.run_module(&mut module(), Rc::new(Neutral)).err().as_deref(),
        Some("retarget claims to preserve dominators but changed them")
    );

    let mut passes = PassManager { verify_invalidation: true, ..Default::default() };
    passes.add(Retarget(PreservedAnalyses::none()));
    assert!(passes.run_module(&mut module(), Rc::new(Neutral)).is_ok());
}

#[test]
fn an_analysis_is_computed_once_until_a_pass_drops_it() {
    COMPUTED.set(0);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(Retarget(PreservedAnalyses::none()));
    passes.add(Look(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(COMPUTED.get(), 2);
}

/// LLVM's model: a pass that changed nothing says `all`; one that says `none`
/// drops what it did not name, edited or not (1272 of 17,000 drops in QCport
/// were of passes that edited nothing and said `none`, `gvn` and `dead` the
/// most: they return `all` when they change nothing, or pay the fresh run).
#[test]
fn a_pass_that_says_none_drops_what_it_did_not_name_edited_or_not() {
    COMPUTED.set(0);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(Look(PreservedAnalyses::none()));
    passes.add(Look(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(COMPUTED.get(), 2, "a pass that said none kept an analysis");
}

/// Asks for the loops and the dominators, then says what it kept.
struct LooksAtLoops {
    seen: Rc<RefCell<Vec<(Rc<crate::loopinfo::LoopInfo>, Rc<crate::domtree::DomTree>)>>>,
    kept: PreservedAnalyses,
}

impl FunctionPass for LooksAtLoops {
    fn name(&self) -> &'static str {
        "looks-at-loops"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        self.seen.borrow_mut().push((loops, tree));
        self.kept.clone()
    }
}

/// LLVM's `preserveSet<CFGAnalyses>`: a pass that left the CFG alone keeps the
/// loops and dominators, one that names nothing drops both. Without the set
/// the manager recomputed LoopInfo after every pass that only moved
/// instructions.
#[test]
fn a_pass_preserving_the_cfg_keeps_loops_and_dominators() {
    let run = |kept: PreservedAnalyses| {
        let seen = Rc::default();
        let mut passes = PassManager::default();
        passes.add(LooksAtLoops { seen: Rc::clone(&seen), kept });
        passes.add(LooksAtLoops { seen: Rc::clone(&seen), kept: PreservedAnalyses::all() });
        passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
        let seen = seen.borrow();
        (Rc::ptr_eq(&seen[0].0, &seen[1].0), Rc::ptr_eq(&seen[0].1, &seen[1].1))
    };
    assert_eq!(run(PreservedAnalyses::none().preserve_cfg()), (true, true), "the CFG set was not honoured");
    assert_eq!(run(PreservedAnalyses::none()), (false, false), "premise: none drops both");
}

/// A pass that claims the CFG and changes it is caught, as LLVM's
/// `-verify-analysis-invalidation` does.
#[test]
fn a_pass_claiming_the_cfg_it_changed_is_caught() {
    let mut passes = PassManager { verify_invalidation: true, ..Default::default() };
    passes.add(Retarget(PreservedAnalyses::none().preserve_cfg()));
    let error = passes.run_module(&mut module(), Rc::new(Neutral)).err();
    assert!(error.is_some_and(|e| e.starts_with("retarget claims to preserve")), "a changed CFG went unnoticed");
}

/// Asks, drops as a rule that edited would, and asks again, the body unedited.
struct AsksTwice {
    if_edited: bool,
}

impl FunctionPass for AsksTwice {
    fn name(&self) -> &'static str {
        "asks-twice"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<Counted>(unit.context, unit.layout, unit.function);
        if self.if_edited {
            analyses.invalidate_if_edited(unit.function, &PreservedAnalyses::none());
        } else {
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        }
        analyses.get::<Counted>(unit.context, unit.layout, unit.function);
        PreservedAnalyses::all()
    }
}

/// `algebraic` dropped the shape and ranges before each rule that asks for
/// them, edited or not: 9 of its 16 range computations on d_faces -O1 followed
/// no edit since the one before, 4 of them 1.4 M each. A drop for an edit
/// nobody made costs a fresh run when it is asked plainly (the pass named
/// nothing: `all` is what a pass that changed nothing returns), and nothing
/// when it is asked `if_edited`.
#[test]
fn a_drop_for_an_edit_nobody_made_computes_again_what_was_not_named() {
    for (if_edited, computed) in [(false, 2), (true, 1)] {
        COMPUTED.set(0);
        let mut passes = PassManager::default();
        passes.add(Look(PreservedAnalyses::all()));
        passes.add(AsksTwice { if_edited });
        passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
        assert_eq!(COMPUTED.get(), computed, "if_edited {if_edited}");
    }
}

thread_local! {
    static KEPT: Cell<usize> = const { Cell::new(0) };
    static SAME: Cell<bool> = const { Cell::new(true) };
}

/// Says a change to the function leaves it as it was, when `SAME`.
struct Steady;

impl Analysis for Steady {
    type Result = usize;
    const NAME: &'static str = "steady";
    const SKIPS: bool = true;
    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> usize {
        KEPT.set(KEPT.get() + 1);
        function.layout().len()
    }

    fn unaffected(
        _: &[Change],
        _: &Context,
        _: &Function,
    ) -> bool {
        SAME.get()
    }
}

struct LookSteady;

impl FunctionPass for LookSteady {
    fn name(&self) -> &'static str {
        "look-steady"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<Steady>(unit.context, unit.layout, unit.function);
        PreservedAnalyses::all()
    }
}

/// A result the analysis says a change left true stands, however the pass that
/// made it describes it; one it does not is derived again.
#[test]
fn a_result_a_change_leaves_true_stands_past_the_pass_that_made_it() {
    for (same, computed) in [(true, 1), (false, 2)] {
        (KEPT.set(0), SAME.set(same));
        let mut passes = PassManager::default();
        passes.add(LookSteady);
        passes.add(Retarget(PreservedAnalyses::none()));
        passes.add(LookSteady);
        passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
        assert_eq!(KEPT.get(), computed, "unaffected = {same}");
    }
}

/// Rewrites the entry's branch `times` over to what it already is.
struct Rewrite(usize);

impl FunctionPass for Rewrite {
    fn name(&self) -> &'static str {
        "rewrite"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        let function = &mut *unit.function;
        let branch = function.terminator(function.entry().unwrap()).unwrap();
        let at = function.instruction(branch).operands.len() - 1;
        let same = function.instruction(branch).operands[at];
        for _ in 0..self.0 {
            function.set_operand(branch, at, same);
        }
        PreservedAnalyses::none()
    }
}

/// The log tells what an operand was before each rewrite, which doubled its
/// length: 200 rewrites were over the 256 changes up to which a result is asked
/// if it stands (`unaffected`), and `gvn` and the analyses that skip lost
/// the skip on d_faces (+1.9% of its compile).
#[test]
fn the_entries_that_tell_what_an_operand_was_are_not_counted_as_changes() {
    (KEPT.set(0), SAME.set(true));
    let mut passes = PassManager::default();
    passes.add(LookSteady);
    passes.add(Rewrite(200));
    passes.add(LookSteady);
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(KEPT.get(), 1, "200 rewrites were taken for more than 256 changes");
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
    let stages = passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    let changes: Vec<(&str, &[Change])> = stages.iter().map(|one| (one.pass, one.changes.as_slice())).collect();
    assert_eq!(
        changes,
        [
            ("look", &[][..]),
            (
                "retarget",
                &[
                    Change::Operand { inst: branch, index: 1, was: Operand::Block(crate::module::BlockId(1)) },
                    Change::Rewritten(branch)
                ][..]
            )
        ]
    );
}

#[test]
fn verify_each_names_the_pass_that_broke_the_module() {
    let mut passes = PassManager { verify_each: true, ..Default::default() };
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(DropReturn);
    let error = passes.run_module(&mut module(), Rc::new(Neutral)).unwrap_err();
    assert!(error.starts_with("after drop-return: "), "{error}");
}

const TWO_EDGES: &str = "define i16 @p(i1 %c) {\nentry:\n  br i1 %c, label %b, label %b\nb:\n  %x = phi i16 [ 1, %entry ], [ 1, %entry ]\n  ret i16 %x\n}\n";

/// Drops one of the two inputs of the phi that two edges feed: the body
/// `SsaUpdaterBulk` left (#3016), which made SCEV and liveness allocate 3 GB.
#[cfg(debug_assertions)]
struct ShortPhi;

#[cfg(debug_assertions)]
impl FunctionPass for ShortPhi {
    fn name(&self) -> &'static str {
        "short-phi"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        let function = &mut *unit.function;
        let block = function.layout()[1];
        let phi = function.iter_block(block).next().unwrap();
        let first = function.instruction(phi).operands[..2].to_vec();
        function.set_operands(phi, first);
        PreservedAnalyses::none()
    }
}

/// A pass that leaves a phi short of an input for one of its edges is named
/// where it ran, not found as a consumer's runaway allocation later. Only a
/// build with debug assertions checks: LLVM verifies a release build's passes
/// only under `-verify-each`.
#[test]
#[cfg(debug_assertions)]
fn a_pass_that_leaves_a_phi_short_of_an_edge_is_named() {
    let mut module = parse::module(TWO_EDGES).unwrap_or_else(|error| panic!("{error}"));
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(ShortPhi);
    let error = passes.run_module(&mut module, Rc::new(Neutral)).unwrap_err();
    assert!(error.starts_with("after short-phi: a phi in %b has 1 inputs from %entry for 2 edges"), "{error}");
}

/// The well-formed body passes the same check.
#[test]
fn a_pass_that_keeps_every_phi_whole_is_not_refused() {
    let mut module = parse::module(TWO_EDGES).unwrap_or_else(|error| panic!("{error}"));
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
}

/// A bisection limit runs only the first pass runs, as LLVM's
/// `-opt-bisect-limit`: with two functions and a limit of one, the second
/// is left as it was. It found the peephole that added deedlines' z%(i)
/// to the wrong row.
#[test]
fn a_bisection_limit_skips_the_runs_after_it() {
    let text = format!("{TEXT}{}", TEXT.replace("@f", "@g"));
    let mut module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let mut passes = PassManager { bisect: Some(1), ..Default::default() };
    passes.add(Retarget(PreservedAnalyses::none()));
    let stages = passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    assert_eq!(stages.len(), 1);
    let printed = crate::print::module(&module);
    assert!(printed.contains("@g(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b"), "{printed}");
}

thread_local! {
    static SEEN: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
}

/// Whether @g reads memory at most, as the outer proxy declares it.
struct ReadsOnly;

impl Analysis for ReadsOnly {
    type Result = bool;
    const NAME: &'static str = "reads-only";
    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        analyses: &mut Analyses,
    ) -> bool {
        let g = analyses
            .outer()
            .globals
            .iter()
            .find(|one| one.name.as_deref() == Some("g"))
            .and_then(|one| one.function())
            .expect("@g");
        g.attrs.contains(&Attribute::Flag("readonly".to_owned()))
    }
}

/// Records what `ReadsOnly` answers, and keeps everything.
struct Ask;

impl FunctionPass for Ask {
    fn name(&self) -> &'static str {
        "ask"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let answer = *analyses.get::<ReadsOnly>(unit.context, unit.layout, unit.function);
        SEEN.with_borrow_mut(|seen| seen.push(answer));
        PreservedAnalyses::all()
    }
}

/// Marks @g `readonly`.
struct MarkReadonly;

impl ModulePass for MarkReadonly {
    fn name(&self) -> &'static str {
        "mark-readonly"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let g = module.named("g").unwrap();
        let GlobalKind::Function(function) = &mut module.globals[g.0 as usize].kind else { panic!("a function") };
        function.attrs.push(Attribute::Flag("readonly".to_owned()));
        vec![g]
    }
}

/// A function analysis reads its callee's declaration through the outer
/// proxy; one kept across a change to that declaration would still say
/// the callee may write.
#[test]
fn a_change_to_what_an_analysis_read_of_the_module_drops_it() {
    let text = "declare i16 @g()\n\ndefine i16 @f() {\nentry:\n  %x = call i16 @g()\n  ret i16 %x\n}\n";
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    SEEN.with_borrow_mut(Vec::clear);
    let mut passes = PassManager::default();
    passes.add(Ask);
    passes.add_module(MarkReadonly);
    passes.add(Ask);
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    assert_eq!(SEEN.take(), [false, true]);
}

thread_local! {
    static SUMMED: Cell<usize> = const { Cell::new(0) };
}

/// Counts its own computations: how many functions have bodies.
struct Bodies;

impl ModuleAnalysis for Bodies {
    type Result = usize;
    const NAME: &'static str = "bodies";
    fn run(
        module: &Module,
        _: &mut ModuleAnalyses,
    ) -> usize {
        SUMMED.set(SUMMED.get() + 1);
        module.functions().filter(|(_, _, function)| !function.is_declaration()).count()
    }
}

/// Reads `Bodies` through the outer proxy.
struct Counts(PreservedAnalyses);

impl FunctionPass for Counts {
    fn name(&self) -> &'static str {
        "counts"
    }

    fn run(
        &mut self,
        _: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        assert_eq!(analyses.outer().cached::<Bodies>().as_deref(), Some(&1));
        self.0.clone()
    }
}

#[test]
fn a_required_module_analysis_is_computed_again_only_after_a_pass_drops_it() {
    SUMMED.set(0);
    let mut passes = PassManager::default();
    passes.require::<Bodies>();
    passes.add(Counts(PreservedAnalyses::none().preserve_module::<Bodies>()));
    passes.add(Retarget(PreservedAnalyses::none()));
    passes.add(Counts(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(SUMMED.get(), 2);
}

/// `-verify-analysis-invalidation` of a module analysis: a pass claiming
/// to keep `Bodies` while it empties a body (which `CalleeEffects` reads as
/// `defined`, so it is stale too).
#[test]
fn a_pass_keeping_a_module_analysis_it_changed_is_caught() {
    struct Empty;

    impl FunctionPass for Empty {
        fn name(&self) -> &'static str {
            "empty"
        }

        fn run(
            &mut self,
            unit: &mut Unit,
            _: &mut Analyses,
        ) -> PreservedAnalyses {
            *unit.function = unit.function.declaration();
            PreservedAnalyses::all()
        }
    }

    let mut passes = PassManager { verify_invalidation: true, ..Default::default() };
    passes.require::<Bodies>();
    passes.add(Empty);
    assert_eq!(
        passes.run_module(&mut module(), Rc::new(Neutral)).err().as_deref(),
        Some("empty claims to preserve bodies, callee-effects but changed them")
    );
}

thread_local! {
    static HELD: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
}

/// Records whether dominators were still held, then asks for them.
struct Probe;

impl FunctionPass for Probe {
    fn name(&self) -> &'static str {
        "probe"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        HELD.with_borrow_mut(|held| held.push(analyses.cached::<Dominators>().is_some()));
        analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        PreservedAnalyses::all()
    }
}

/// Dominators read only their function; a change to another global's
/// declaration emptied @f's whole manager and recomputed them.
#[test]
fn a_change_to_the_module_keeps_what_reads_only_the_function() {
    let text = "declare i16 @g()\n\ndefine i16 @f() {\nentry:\n  %x = call i16 @g()\n  ret i16 %x\n}\n";
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    HELD.with_borrow_mut(Vec::clear);
    let mut passes = PassManager::default();
    passes.add(Probe);
    passes.add_module(MarkReadonly);
    passes.add(Probe);
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    assert_eq!(HELD.take(), [false, true]);
}

/// Every edit dropped the module analyses and each was worked out again from
/// every global (66 times a compile of `callers-64`, 1 ms apiece in
/// `call-registers` alone): while the declarations are those it worked from,
/// the result is the same result.
#[test]
fn module_analyses_of_the_declarations_stand_until_a_declaration_changes() {
    use crate::passes::{CallRegisters, CalleeEffects, Declarations, GlobalSizes, TypeAncestry};
    let mut module = module();
    let program = crate::program::ProgramProxy::of(&module, Rc::new(Neutral));
    let mut analyses = ModuleAnalyses::new(program);
    let first = (
        analyses.get::<Declarations>(&module),
        analyses.get::<CalleeEffects>(&module),
        analyses.get::<CallRegisters>(&module),
        analyses.get::<GlobalSizes>(&module),
        analyses.get::<TypeAncestry>(&module),
    );
    // A body edited: no declaration moved.
    let ran = crate::passes::module_runs();
    let (_, function) = module.function_mut("f").unwrap();
    let branch = function.terminator(function.entry().unwrap()).unwrap();
    function.set_operand(
        branch,
        function.instruction(branch).operands.len() - 2,
        Operand::Block(crate::module::BlockId(2)),
    );
    analyses.invalidate(&PreservedAnalyses::none());
    assert!(Rc::ptr_eq(&first.0, &analyses.get::<Declarations>(&module)), "declarations");
    assert!(Rc::ptr_eq(&first.1, &analyses.get::<CalleeEffects>(&module)), "callee effects");
    assert!(Rc::ptr_eq(&first.2, &analyses.get::<CallRegisters>(&module)), "call registers");
    assert!(Rc::ptr_eq(&first.3, &analyses.get::<GlobalSizes>(&module)), "global sizes");
    assert!(Rc::ptr_eq(&first.4, &analyses.get::<TypeAncestry>(&module)), "type ancestry");
    assert_eq!(
        crate::passes::module_runs(),
        ran,
        "{} module analyses were worked out again for an edit to a body",
        crate::passes::module_runs() - ran
    );
    // A declaration moved: an attribute on the function.
    let (_, function) = module.function_mut("f").unwrap();
    function.attrs.push(Attribute::Flag("readnone".to_owned()));
    analyses.invalidate(&PreservedAnalyses::none());
    assert!(!Rc::ptr_eq(&first.0, &analyses.get::<Declarations>(&module)), "declarations did not follow the attribute");
    assert!(
        !Rc::ptr_eq(&first.1, &analyses.get::<CalleeEffects>(&module)),
        "callee effects did not follow the attribute"
    );
}

/// A pass that named nothing loses the result though the body did not move:
/// the rerun is told `same-untouched` (383 of the 2,248 results dropped at one
/// d_faces -O2 compile were of that kind on main, which kept them).
#[test]
fn a_result_the_body_has_not_moved_from_is_worked_out_again_when_not_named() {
    let module = module();
    let function = module.global(module.named("f").unwrap()).function().unwrap();
    let layout = DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    crate::passes::trace_recomputes(true);
    analyses.get::<Counted>(&module.context, &layout, function);
    analyses.invalidate(function, &PreservedAnalyses::none());
    crate::passes::note_pass("retarget");
    analyses.get::<Counted>(&module.context, &layout, function);
    let counts = crate::passes::recomputes();
    crate::passes::trace_recomputes(false);
    assert_eq!(counts, vec![("counted", "", "first", 1), ("counted", "retarget", "same-untouched", 1)]);
}

/// Told apart: a rerun on a body no pass touched since the result was made
/// (`same-untouched`: the pass invalidated and changed nothing), and one after
/// an edit the analysis did not read (`same`). The census of x_life and
/// nbody_fixed needed the split to say the first is under 0.5% of a compile.
#[test]
fn a_rerun_is_told_untouched_from_after_an_edit_that_changed_nothing_it_reads() {
    let module = module();
    let mut function = module.global(module.named("f").unwrap()).function().unwrap().clone();
    let function = &mut function;
    let layout = DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    crate::passes::trace_recomputes(true);
    analyses.get::<Counted>(&module.context, &layout, function);
    let branch = function.terminator(function.entry().unwrap()).unwrap();
    let at = function.instruction(branch).operands.len() - 2;
    let target = function.instruction(branch).operands[at + 1];
    function.set_operand(branch, at, target);
    analyses.invalidate(function, &PreservedAnalyses::none());
    crate::passes::note_pass("edited");
    analyses.get::<Counted>(&module.context, &layout, function);
    let counts = crate::passes::recomputes();
    crate::passes::trace_recomputes(false);
    assert_eq!(counts, vec![("counted", "", "first", 1), ("counted", "edited", "same", 1)]);
}

/// LLVM's model: a pass names what it kept and the manager drops the rest. A
/// pass that forgot to name an analysis got it kept on main whenever the log
/// showed the function as it was when the result was derived; now it is
/// computed again, whatever the log says.
#[test]
fn a_pass_that_names_nothing_loses_the_analysis_it_forgot() {
    let module = module();
    let function = module.global(module.named("f").unwrap()).function().unwrap();
    let layout = DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    analyses.get::<Counted>(&module.context, &layout, function);
    COMPUTED.set(0);
    analyses.invalidate(function, &PreservedAnalyses::none());
    assert!(analyses.cached::<Counted>().is_none(), "an analysis the pass did not name stands");
    analyses.get::<Counted>(&module.context, &layout, function);
    assert_eq!(COMPUTED.get(), 1, "the forgotten analysis was not computed again");
}

/// The analysis a pass names stands, and one it does not name goes.
#[test]
fn a_pass_keeps_what_it_names_and_only_that() {
    let module = module();
    let function = module.global(module.named("f").unwrap()).function().unwrap();
    let layout = DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    analyses.get::<Counted>(&module.context, &layout, function);
    analyses.get::<Steady>(&module.context, &layout, function);
    analyses.invalidate(function, &PreservedAnalyses::none().preserve::<Steady>());
    assert!(analyses.cached::<Steady>().is_some(), "the analysis named went");
    assert!(analyses.cached::<Counted>().is_none(), "an analysis not named stands");
}

/// A pass that changed nothing returns `all` and everything stands.
#[test]
fn a_pass_that_changed_nothing_keeps_everything() {
    let module = module();
    let function = module.global(module.named("f").unwrap()).function().unwrap();
    let layout = DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    analyses.get::<Counted>(&module.context, &layout, function);
    analyses.get::<Steady>(&module.context, &layout, function);
    analyses.invalidate(function, &PreservedAnalyses::all());
    assert!(analyses.cached::<Counted>().is_some() && analyses.cached::<Steady>().is_some());
}

/// A late pass: rewrites a branch, adds no memory operation.
struct Quiet;

impl FunctionPass for Quiet {
    fn name(&self) -> &'static str {
        "quiet"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        let function = &mut *unit.function;
        let branch = function.terminator(function.entry().unwrap()).unwrap();
        let at = function.instruction(branch).operands.len() - 2;
        let target = function.instruction(branch).operands[at + 1];
        function.set_operand(branch, at, target);
        PreservedAnalyses::none()
    }
}

/// Every run of a pass over the module analyses was a recomputation of what the
/// passes after the summaries were made had left as it was (201 of 833 over
/// QCport at -O1): the module analyses frozen at a point of the pipeline are
/// held past passes that add no memory operation, and dropped by one that may.
#[test]
fn a_frozen_module_analysis_is_held_past_passes_that_add_no_memory_operation() {
    SUMMED.set(0);
    let mut passes = PassManager::default();
    passes.require::<Bodies>();
    passes.freeze::<Bodies>();
    passes.add(Quiet);
    passes.add(Quiet);
    passes.add(Counts(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(SUMMED.get(), 1, "a pass that adds no memory operation dropped the frozen analysis");

    SUMMED.set(0);
    let mut passes = PassManager::default();
    passes.require::<Bodies>();
    passes.freeze::<Bodies>();
    passes.add(Quiet);
    passes.add(Retarget(PreservedAnalyses::none()));
    passes.add(Counts(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(SUMMED.get(), 2, "a pass that may add one did not drop it");
}

thread_local! {
    static PEEKED: RefCell<Vec<Option<usize>>> = const { RefCell::new(Vec::new()) };
}

/// Notes what the outer proxy holds of `Bodies`.
struct Peek;

impl FunctionPass for Peek {
    fn name(&self) -> &'static str {
        "peek"
    }

    fn run(
        &mut self,
        _: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        PEEKED.with_borrow_mut(|seen| seen.push(analyses.outer().cached::<Bodies>().as_deref().copied()));
        PreservedAnalyses::all()
    }
}

/// Reports @f changed: the manager drops what it does not keep.
struct Touch;

impl ModulePass for Touch {
    fn name(&self) -> &'static str {
        "touch"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        vec![module.named("f").unwrap()]
    }
}

/// Abandons `Bodies`, as GlobalDCE does once it renumbers the globals.
struct Abandon;

impl crate::program::ProgramPass for Abandon {
    fn name(&self) -> &'static str {
        "abandon"
    }

    fn run(
        &mut self,
        _: &mut crate::program::Program,
        analyses: &mut crate::program::ProgramAnalyses,
    ) -> Result<(), String> {
        analyses.abandon::<Bodies>();
        Ok(())
    }
}

/// Makes a table, as SimplifyCFG's lookup tables are made.
struct Tabulate;

impl FunctionPass for Tabulate {
    fn name(&self) -> &'static str {
        "tabulate"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        let ty = unit.context.types.int(16);
        unit.declared.table("t", ty, unit.context.int(ty, 0));
        PreservedAnalyses::none()
    }
}

/// A table's initializer may hold a tracked global's address: GlobalsAA held
/// past r_portal.c's lookup table of `@_dg_push`, `@_dg_pop` and `@_dg_proj`
/// still had them reached by name only (`LLRM_CHECK_STALE` failed).
#[test]
fn a_pass_that_adds_a_table_abandons_a_cached_module_analysis() {
    SUMMED.set(0);
    PEEKED.with_borrow_mut(Vec::clear);
    let mut passes = PassManager::default();
    passes.require_cached::<Bodies>();
    passes.compute::<Bodies>();
    passes.add(Peek);
    passes.add(Tabulate);
    passes.add(Peek);
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(PEEKED.take(), [Some(1), None], "held past a pass that added a table");
}

/// GlobalsAA was computed 9 times for one compile of d_faces -O2, LLVM's twice:
/// at `RequireAnalysisPass<GlobalsAA>` and `RecomputeGlobalsAAPass`. A cached
/// analysis is made at its points alone, read only once made, and kept past
/// every pass until one abandons it.
#[test]
fn a_cached_module_analysis_is_computed_at_its_points_alone() {
    SUMMED.set(0);
    PEEKED.with_borrow_mut(Vec::clear);
    let mut passes = PassManager::default();
    passes.require_cached::<Bodies>();
    passes.add(Peek);
    passes.compute::<Bodies>();
    passes.add(Retarget(PreservedAnalyses::none()));
    passes.add_module(Touch);
    passes.add_program(Nothing);
    passes.add(Peek);
    passes.recompute::<Bodies>();
    passes.add(Peek);
    passes.add_program(Abandon);
    passes.add(Peek);
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(SUMMED.get(), 2, "computed other than at its two points");
    assert_eq!(PEEKED.take(), [None, Some(1), Some(1), None]);
}

/// What a body holds, as a number: a summary that can only be an upper bound.
struct Size;

impl ModuleAnalysis for Size {
    type Result = usize;
    const NAME: &'static str = "size";
    fn run(
        module: &Module,
        _: &mut ModuleAnalyses,
    ) -> usize {
        module.functions().map(|(_, _, function)| function.walk().count()).sum()
    }
    fn covers(
        stale: &usize,
        fresh: &usize,
    ) -> bool {
        fresh <= stale
    }
}

/// A late pass that says it adds nothing and does.
struct Lies;

impl FunctionPass for Lies {
    fn name(&self) -> &'static str {
        "lies"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        let ty = unit.context.types.int(16);
        let one = unit.context.int(ty, 1);
        let function = &mut *unit.function;
        let first = function.block(function.entry().unwrap()).first().expect("a block with an instruction");
        let made = function.create_instruction(
            crate::opcode::Opcode::Binary(crate::opcode::BinaryOp::Add),
            ty,
            vec![Operand::Constant(one), Operand::Constant(one)],
            crate::opcode::Flags::default(),
            Some("extra"),
        );
        function.insert(made, crate::edit::Position::Before(first)).unwrap();
        PreservedAnalyses::none()
    }
}

/// `LLRM_CHECK_STALE` recomputes what a pass left frozen, and a pass that
/// declared it adds nothing and did is named.
#[test]
fn a_late_pass_that_adds_what_it_said_it_would_not_fails_the_stale_check() {
    crate::passes::check_stale(true);
    let run = |pass: Box<dyn FnOnce(&mut PassManager)>| {
        let mut passes = PassManager::default();
        passes.require::<Size>();
        passes.freeze::<Size>();
        pass(&mut passes);
        passes.run_module(&mut module(), Rc::new(Neutral))
    };
    assert!(run(Box::new(|passes| passes.add(Quiet))).is_ok(), "a rewrite adds nothing");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(Box::new(|passes| passes.add(Lies)))));
    crate::passes::check_stale(false);
    let message = *caught.expect_err("the pass added an instruction").downcast::<String>().expect("a message");
    assert!(message.contains("lies added a memory operation"), "{message}");
}

/// A module pass that changes the module.
struct Edits;

impl ModulePass for Edits {
    fn name(&self) -> &'static str {
        "edits"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        vec![module.named("f").unwrap()]
    }
}

thread_local! {
    static OUTER_RUNS: Cell<usize> = const { Cell::new(0) };
}

/// Reads the outer facts and can be brought up to date.
struct ReadsOuter;

impl Analysis for ReadsOuter {
    type Result = usize;
    const NAME: &'static str = "reads-outer";
    const INCREMENTAL: bool = true;
    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> usize {
        OUTER_RUNS.set(OUTER_RUNS.get() + 1);
        function.layout().len()
    }

    fn update(
        previous: &usize,
        _: &[Change],
        _: &Context,
        _: &DataLayout,
        _: &Function,
        _: &mut Analyses,
    ) -> Option<usize> {
        Some(*previous)
    }
}

/// An incremental result made under other facts of the module than the ones now
/// (a global that became constant, say) was brought up to date from the
/// function's changes alone: `registers` kept no fact of a load from a
/// table `globalopt` had since made constant, and `crc` at -Omax panicked under
/// `LLRM_CHECK_REPLAY`.
#[test]
fn an_incremental_result_made_under_other_outer_facts_is_derived_again() {
    OUTER_RUNS.set(0);
    let mut module = module();
    let id = module.named("f").unwrap();
    let (first, second) =
        (Rc::new(crate::passes::Outer::of(&module, None)), Rc::new(crate::passes::Outer::of(&module, None)));
    let layout = DataLayout::default();
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    {
        let function = module.global(id).function().unwrap();
        analyses.manager(id, &first).get::<ReadsOuter>(&module.context, &layout, function);
    }
    {
        let (_, function) = module.function_mut("f").unwrap();
        let entry = function.entry().unwrap();
        let branch = function.terminator(entry).unwrap();
        let at = function.instruction(branch).operands.len() - 1;
        let same = function.instruction(branch).operands[at];
        function.set_operand(branch, at, same);
    }
    let function = module.global(id).function().unwrap();
    analyses.manager(id, &first).invalidate(function, &PreservedAnalyses::none());
    analyses.manager(id, &second).get::<ReadsOuter>(&module.context, &layout, function);
    assert_eq!(OUTER_RUNS.get(), 2, "brought up to date under other outer facts");
}

/// The required module analyses were made before every pass: before a module
/// pass that changed the module and dropped them, for the next to drop them
/// again (2% of the -O1 compile of QCport). A function pass reads them through
/// `Outer`; they are made before it.
#[test]
fn the_required_module_analyses_are_made_before_a_function_pass_not_before_a_module_pass() {
    SUMMED.set(0);
    let mut passes = PassManager::default();
    passes.require::<Bodies>();
    passes.add_module(Edits);
    passes.add_module(Edits);
    passes.add(Counts(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(SUMMED.get(), 1, "made before a module pass");
}

thread_local! {
    static LOGGED: Cell<usize> = const { Cell::new(0) };
}

/// Records how many edits the log of reported edits held when it was computed.
struct Logged;

impl ModuleAnalysis for Logged {
    type Result = usize;
    const NAME: &'static str = "logged";
    fn run(
        _: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> usize {
        LOGGED.set(analyses.edit_log().len());
        analyses.edit_log().len()
    }
}

/// `Summaries` trusts the log of reported edits for which bodies it looks at:
/// a body a function pass edited under the manager was not in it, and the
/// summaries (and the point values kept with them) were of the body as it was
/// (`dse` kept an observed store's summaries; a compile died at `alias.rs`
/// `incoming[&block]`).
#[test]
fn a_body_a_function_pass_edited_is_in_the_log_of_edits() {
    LOGGED.set(0);
    let mut passes = PassManager::default();
    passes.require::<Logged>();
    passes.add(Retarget(PreservedAnalyses::none()));
    passes.add(LookSteady);
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(LOGGED.get(), 1, "the edit the pass made was not reported");
}

/// Reads what each function does to memory, from the outer facts.
struct ReadsEffects;

impl Analysis for ReadsEffects {
    type Result = usize;
    const NAME: &'static str = "reads-effects";
    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        analyses: &mut Analyses,
    ) -> usize {
        analyses.outer().callees().len()
    }
}

/// Reads nothing outside the function.
struct ReadsNone;

impl Analysis for ReadsNone {
    type Result = usize;
    const NAME: &'static str = "reads-none";
    const READS_OUTER: bool = false;
    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> usize {
        function.layout().len()
    }
}

/// A pass is run again only where what it read of the module has changed, so
/// what it read has to be said: by the analysis it asks for, the first time and
/// when the result is held, and by what it asks of the proxy.
#[test]
fn what_an_analysis_read_of_the_module_is_noted_each_time_it_is_asked_for() {
    use crate::passes::read;
    let module = module();
    let id = module.named("f").unwrap();
    let function = module.global(id).function().unwrap();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    let layout = DataLayout::default();
    read::swap(0);
    analyses.get::<ReadsNone>(&module.context, &layout, function);
    assert_eq!(read::swap(0), 0, "an analysis of the function alone reads nothing of the module");
    analyses.get::<ReadsEffects>(&module.context, &layout, function);
    assert_eq!(read::swap(0), read::CALLEES | read::DECLARATIONS, "the first time: the proxy, and what the callees do");
    analyses.get::<ReadsEffects>(&module.context, &layout, function);
    assert_eq!(read::swap(0), read::CALLEES | read::DECLARATIONS, "held: it is still what the pass read");
    analyses.outer();
    assert_eq!(read::swap(0), read::DECLARATIONS, "the proxy asked for is the declarations read");
}

/// Which edits keep every live value's meaning, from the log alone: finding a
/// value again and erasing what it replaced do; an operand set to another
/// value, a store made and a block made do not.
#[test]
fn sound_edits_are_read_from_the_log_not_promised_by_a_pass() {
    use crate::passes::sound_edits;
    let mut module = parse::module(
        "define i16 @f(i16 %x, ptr %p) {\nentry:\n  %a = add i16 %x, 1\n  %b = add i16 %x, 1\n  %c = add i16 %a, %b\n  ret i16 %c\n}\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let (_, function) = module.function_mut("f").expect("@f");
    let find = |function: &Function, name: &str| {
        function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(|value| function.value(value).name.as_deref() == Some(name))
            })
            .expect("the instruction")
    };
    let value_of = |function: &Function, inst| function.instruction(inst).result.expect("a value");
    let (a, b, c) = (find(function, "a"), find(function, "b"), find(function, "c"));
    let before = function.mark();
    function.replace_value(value_of(function, b), Operand::Value(value_of(function, a)));
    function.erase(b).expect("unused now");
    assert!(sound_edits(function, function.changes_since(before).unwrap()), "found again, then erased");
    let before = function.mark();
    function.set_operand(c, 0, Operand::Value(value_of(function, a)));
    assert!(!sound_edits(function, function.changes_since(before).unwrap()), "an operand set to another value");
    let before = function.mark();
    let ret = function.terminator(function.entry().unwrap()).unwrap();
    function.erase(ret).expect("the last instruction");
    assert!(!sound_edits(function, function.changes_since(before).unwrap()), "a terminator erased");
}

thread_local!(static READS_RUN: Cell<usize> = const { Cell::new(0) });

/// The values `%c` reads, as a result that names values.
struct Reads;

impl Analysis for Reads {
    type Result = Vec<crate::module::ValueId>;
    const NAME: &'static str = "reads";
    const SOUND: bool = true;
    const NAMES_VALUES: bool = true;

    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> Self::Result {
        READS_RUN.with(|runs| runs.set(runs.get() + 1));
        let c = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(|value| function.value(value).name.as_deref() == Some("c"))
            })
            .unwrap();
        function
            .instruction(c)
            .operands
            .iter()
            .filter_map(|one| match one {
                Operand::Value(value) => Some(*value),
                _ => None,
            })
            .collect()
    }

    fn remapped(
        old: &Self::Result,
        substituted: &dyn Fn(crate::module::ValueId) -> Option<Operand>,
        _: &[Change],
        _: &Context,
        _: &Function,
    ) -> Option<Self::Result> {
        old.iter()
            .map(|value| match substituted(*value) {
                None => Some(*value),
                Some(Operand::Value(now)) => Some(now),
                Some(_) => None,
            })
            .collect()
    }
}

/// A result that names values was derived again past a `replace_value`, though
/// the value that took the place of the replaced one is known: a kept result is
/// brought to it instead (LLVM's value handles follow a RAUW), one that cannot
/// be is derived again.
#[test]
fn a_result_that_names_values_is_followed_through_a_substitution() {
    let mut module = parse::module(
        "define i16 @f(i16 %x) {\nentry:\n  %a = add i16 %x, 1\n  %b = add i16 %x, 1\n  %c = add i16 %a, %b\n  ret i16 %c\n}\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    let (context, function) = module.function_mut("f").expect("@f");
    let named = |function: &Function, name: &str| {
        function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(|value| function.value(value).name.as_deref() == Some(name))
            })
            .expect("the instruction")
    };
    let (a, b) = (named(function, "a"), named(function, "b"));
    let (a_value, b_value) = (function.instruction(a).result.unwrap(), function.instruction(b).result.unwrap());
    assert_eq!(*analyses.get::<Reads>(context, &layout, function), vec![a_value, b_value]);
    function.replace_value(b_value, Operand::Value(a_value));
    function.erase(b).expect("unused now");
    analyses.invalidate(function, &PreservedAnalyses::none());
    assert_eq!(*analyses.get::<Reads>(context, &layout, function), vec![a_value, a_value], "still named %b");
    assert_eq!(READS_RUN.with(Cell::get), 1, "derived again past a substitution it could follow");
}

/// Hashing a function's attributes is the size of its declaration, and every
/// result derived and every result checked at an invalidation did it: 12.9 M
/// of 316 M instructions on a 64-link call chain, and `gate (cost)` read it as
/// a 4% rise at 2N. It is done once per invalidation.
#[test]
fn the_attributes_are_hashed_once_per_invalidation_not_once_per_result() {
    let module = module();
    let layout = DataLayout::default();
    let function = module.global(module.named("f").unwrap()).function().unwrap();
    let mut analyses = Analyses::new(Rc::new(crate::passes::Outer::of(&module, None)));
    let before = crate::module::attribute_stamps();
    analyses.get::<Steady>(&module.context, &layout, function);
    analyses.get::<ReadsOuter>(&module.context, &layout, function);
    analyses.invalidate(function, &PreservedAnalyses::none());
    assert!(crate::module::attribute_stamps() - before <= 2, "{}", crate::module::attribute_stamps() - before);
}

thread_local! {
    static BASE_RUNS: Cell<usize> = const { Cell::new(0) };
    static ABOVE_RUNS: Cell<usize> = const { Cell::new(0) };
}

/// The number of instructions of the body.
struct Base;

impl Analysis for Base {
    type Result = usize;
    const NAME: &'static str = "base";
    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> usize {
        BASE_RUNS.set(BASE_RUNS.get() + 1);
        function.walk().count()
    }
}

/// Twice `Base`.
struct Above;

impl Analysis for Above {
    type Result = usize;
    const NAME: &'static str = "above";
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> usize {
        ABOVE_RUNS.set(ABOVE_RUNS.get() + 1);
        2 * *analyses.get::<Base>(context, layout, function)
    }
}

/// A pass edited and then asked, without invalidating: `above` was derived from
/// the `base` the body had before the edit, and took the mark of the edit. An
/// invalidation does not keep a result derived from the body as it stands when
/// what it was derived from was not: `sroa` did this after a split and the
/// manager held `points-to` stale (`LLRM_CHECK_ORDER` found it).
#[test]
fn a_result_derived_from_one_held_from_before_the_edit_is_not_kept_as_current() {
    let mut module = module();
    let layout = DataLayout::default();
    let id = module.named("f").unwrap();
    let outer = Rc::new(crate::passes::Outer::of(&module, None));
    let mut analyses = Analyses::new(outer);
    let function = module.global(id).function().unwrap();
    let context = &module.context;
    let before = *analyses.get::<Base>(context, &layout, function);
    let (context, function) = module.function_mut("f").unwrap();
    let entry = function.entry().unwrap();
    let branch = function.terminator(entry).unwrap();
    let at = function.instruction(branch).operands.len() - 2;
    let target = function.instruction(branch).operands[at + 1];
    function.set_operand(branch, at, target);
    // The edit moved the mark; the pass asks `above` without invalidating.
    ABOVE_RUNS.set(0);
    assert_eq!(*analyses.get::<Above>(context, &layout, function), 2 * before, "derived from the base it held");
    analyses.invalidate(function, &PreservedAnalyses::none());
    analyses.get::<Above>(context, &layout, function);
    assert_eq!(ABOVE_RUNS.get(), 2, "kept as derived from the body as it stands");
}

/// Asks for `Counted`, edits the function, and says it kept `Counted` by
/// updating it: it puts the result back when `put_back` (LLVM's pass that
/// uses `MemorySSAUpdater` and preserves `MemorySSAAnalysis`).
struct UpdatesCounted {
    put_back: bool,
}

impl FunctionPass for UpdatesCounted {
    fn name(&self) -> &'static str {
        "updates-counted"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<Counted>(unit.context, unit.layout, unit.function);
        let last = *unit.function.layout().last().unwrap();
        let ret = unit.function.terminator(last).unwrap();
        unit.function.erase(ret).unwrap();
        if self.put_back {
            analyses.replace::<Counted>(unit.function, unit.function.layout().len());
        }
        PreservedAnalyses::none().preserve_updated::<Counted>()
    }
}

/// A pass that updated an analysis and put it back keeps it for the next pass:
/// the analysis is built once over the two passes that ask. Without
/// `replace` the manager would have built it again after the edit.
#[test]
fn an_analysis_a_pass_updated_and_put_back_is_not_computed_again() {
    COMPUTED.set(0);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(UpdatesCounted { put_back: true });
    passes.add(Look(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(COMPUTED.get(), 1, "the updated result was not kept");
}

/// A pass that says it kept an analysis by updating it and did not put the
/// result back left it as old as the edit: the manager computes it again.
#[test]
fn an_analysis_a_pass_said_it_updated_and_did_not_put_back_is_computed_again() {
    COMPUTED.set(0);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(UpdatesCounted { put_back: false });
    passes.add(Look(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(COMPUTED.get(), 2, "a stale result stood");
}

/// `LLRM_CHECK_STALE` names the pass that declared it and did not put back.
#[test]
fn the_stale_check_names_a_pass_that_declared_an_update_it_did_not_put_back() {
    crate::passes::check_stale(true);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(UpdatesCounted { put_back: false });
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = passes.run_module(&mut module(), Rc::new(Neutral));
    }));
    crate::passes::check_stale(false);
    let message = caught.expect_err("the lying pass went unnoticed");
    let message = message.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("updates-counted") && message.contains("replace"), "{message}");
}

/// A function pass that changes nothing.
struct Idle;

impl FunctionPass for Idle {
    fn name(&self) -> &'static str {
        "idle"
    }

    fn run(
        &mut self,
        _: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        PreservedAnalyses::all()
    }
}

/// A program pass that changes nothing.
struct Nothing;

impl crate::program::ProgramPass for Nothing {
    fn name(&self) -> &'static str {
        "nothing"
    }

    fn run(
        &mut self,
        _: &mut crate::program::Program,
        _: &mut crate::program::ProgramAnalyses,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// `LLRM_DUMP_INPUTS` dumped the input of the pipeline's function passes only:
/// a module or program pass (the inliner, GlobalDCE, IPSCCP) wrote nothing, and
/// a miscompile after inlining could not name its pass (#2982).
#[test]
fn a_module_or_program_pass_dumps_its_input_when_inputs_are_dumped() {
    let text = "define i16 @g() {\nentry:\n  ret i16 0\n}\n\ndefine i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %b\nb:\n  ret i16 0\n}\n";
    let run = |inputs: bool| {
        let directory = std::env::temp_dir().join(format!("llrm-dump-inputs-{}-{inputs}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}"));
        let mut passes = PassManager::default();
        passes.dump = Some(directory.clone());
        passes.dump_inputs = inputs;
        passes.add_module(MarkReadonly);
        passes.add_program(Nothing);
        passes.add(Idle);
        passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
        directory
    };
    let directory = run(true);
    let manifest = std::fs::read_to_string(directory.join("manifest.txt")).expect("a manifest");
    let lines: Vec<Vec<&str>> = manifest.lines().map(|line| line.split(' ').collect()).collect();
    let named: Vec<(&str, &str)> = lines.iter().map(|line| (line[0], line[2])).collect();
    assert_eq!(
        named,
        [("mark-readonly", "0001-in-mark-readonly.ll"), ("nothing", "0002-in-nothing.ll"), ("idle", "0003-in-idle.ll"),]
    );
    assert!(lines.iter().all(|line| line[3].parse::<usize>().unwrap() > 0), "{manifest}");
    for (_, file) in named {
        assert!(directory.join(file).exists(), "{file}");
    }
    let off = run(false);
    assert!(!off.join("manifest.txt").exists(), "an input dump without LLRM_DUMP_INPUTS");
    let _ = std::fs::remove_dir_all(directory);
    let _ = std::fs::remove_dir_all(off);
}

/// A dump per pass added: five passes of the three kinds wrote fewer input
/// files and manifest lines than that when a kind was left out.
#[test]
fn a_manager_with_n_passes_writes_n_input_dumps_and_n_manifest_lines() {
    let directory = std::env::temp_dir().join(format!("llrm-dump-count-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let mut module = parse::module(TEXT).unwrap_or_else(|error| panic!("{error}"));
    let mut passes = PassManager::default();
    passes.dump = Some(directory.clone());
    passes.dump_inputs = true;
    passes.add_program(Nothing);
    passes.add(Idle);
    passes.add_program(Nothing);
    passes.add(Idle);
    passes.add(Idle);
    passes.run_module(&mut module, Rc::new(Neutral)).unwrap();
    let files = std::fs::read_dir(&directory)
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().contains("-in-"))
        .count();
    let manifest = std::fs::read_to_string(directory.join("manifest.txt")).expect("a manifest");
    assert_eq!((files, manifest.lines().count()), (5, 5), "{manifest}");
    let _ = std::fs::remove_dir_all(directory);
}

/// RunsOver the bodies it is run over.
struct RunsOver(std::rc::Rc<std::cell::Cell<usize>>);

impl FunctionPass for RunsOver {
    fn name(&self) -> &'static str {
        "runs-over"
    }

    fn run(
        &mut self,
        _: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        self.0.set(self.0.get() + 1);
        PreservedAnalyses::all()
    }
}

/// A private function whose last call was inlined has a body of one
/// `unreachable` (`bare`): the pin's CGSCC walk deletes it, so no function pass
/// runs over it. The late slots ran on four such leaves of d_faces, in every
/// slot.
#[test]
fn a_function_the_inliner_deleted_is_not_run_over() {
    let mut module = parse::module(
        "define internal i16 @gone() {\nb1:\n  unreachable\n}\n\ndefine i16 @kept(i16 %x) {\nb1:\n  ret i16 %x\n}\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let runs = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut passes = PassManager::default();
    passes.add(RunsOver(std::rc::Rc::clone(&runs)));
    passes.run_module(&mut module, std::rc::Rc::new(crate::target::Neutral)).expect("a run");
    assert_eq!(runs.get(), 1, "only @kept is run over");
}
