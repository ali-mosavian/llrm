use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::context::Context;
use crate::context::GlobalId;
use crate::datalayout::DataLayout;
use crate::module::{Change, Function, GlobalKind, Module, Operand};
use crate::opcode::Attribute;
use crate::parse;
use crate::passes::{
    Analyses, Analysis, Dominators, FunctionPass, ModuleAnalyses, ModuleAnalysis, ModulePass, PassManager,
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

/// A pass that says it dropped everything and edited nothing (1272 of 17,000
/// drops in QCport, `gvn` and `dead` the most) cost every analysis a fresh run,
/// and the next to ask got no more than the one before.
#[test]
fn a_pass_that_edited_nothing_drops_nothing_whatever_it_says() {
    COMPUTED.set(0);
    let mut passes = PassManager::default();
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(Look(PreservedAnalyses::none()));
    passes.add(Look(PreservedAnalyses::all()));
    passes.run_module(&mut module(), Rc::new(Neutral)).unwrap();
    assert_eq!(COMPUTED.get(), 1, "a pass that logged no change dropped an analysis");
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
    assert_eq!(changes, [("look", &[][..]), ("retarget", &[Change::Rewritten(branch)][..])]);
}

#[test]
fn verify_each_names_the_pass_that_broke_the_module() {
    let mut passes = PassManager { verify_each: true, ..Default::default() };
    passes.add(Look(PreservedAnalyses::all()));
    passes.add(DropReturn);
    let error = passes.run_module(&mut module(), Rc::new(Neutral)).unwrap_err();
    assert!(error.starts_with("after drop-return: "), "{error}");
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
/// to keep `Bodies` while it empties a body.
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
        Some("empty claims to preserve bodies but changed them")
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
