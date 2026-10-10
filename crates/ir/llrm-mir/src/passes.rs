//! LLVM's new pass manager: function and module passes over each module of
//! a program, analyses cached per function and per module and dropped
//! unless a pass says it preserved them, and each pass's change log kept
//! for the rewrite ledger. A function analysis reads its module only
//! through `Outer`, LLVM's outer analysis manager proxy, which holds the
//! module analyses; a module analysis reads its program only through
//! `ModuleAnalyses::program`.
//!
//! Two instruments, as LLVM's `-verify-each` and
//! `-verify-analysis-invalidation`: verifying the module after every pass,
//! and recomputing every analysis a pass claims to have preserved.

use std::any::{Any, TypeId};
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::context::{Context, GlobalId};
use crate::datalayout::DataLayout;
use crate::dominators::DominatorTree;
use crate::hash::{HashMap, HashSet};
use crate::module::{Change, Function, GlobalKind, GlobalValue, MetadataNode, Module};
use crate::program::{Program, ProgramAnalyses, ProgramPass, ProgramProxy, interface};
use crate::target::{Machine, Neutral};

/// Where a timer is plugged in: MIR depends on nothing, so whoever times the
/// pipeline (`llrm-transforms`, when `LLRM_DEBUG=time` is on) installs these
/// once.
pub struct Observer {
    /// Runs the closure as a step of kind `.0` (`mir`, `analysis`) named `.1`.
    pub span: fn(&'static str, &'static str, &mut dyn FnMut()),
    /// Runs the closure with the steps in it charged to the function named
    /// `.0`.
    pub function: fn(&str, &mut dyn FnMut()),
    /// A cached analysis `.0` looked up: found when `.1`, else computed.
    pub count: fn(&'static str, bool),
}

static OBSERVER: std::sync::OnceLock<Observer> = std::sync::OnceLock::new();

/// Installs `observer`; the first install stands.
pub fn observe(observer: Observer) {
    let _ = OBSERVER.set(observer);
}

/// `run` as the MIR step `name`, timed if an observer is installed.
pub fn spanned<T>(
    name: &'static str,
    run: impl FnOnce() -> T,
) -> T {
    spanned_as("mir", name, run)
}

fn spanned_as<T>(
    kind: &'static str,
    name: &'static str,
    run: impl FnOnce() -> T,
) -> T {
    let Some(observer) = OBSERVER.get() else { return run() };
    let (mut run, mut out) = (Some(run), None);
    (observer.span)(kind, name, &mut || out = run.take().map(|run| run()));
    out.expect("the observer ran the step")
}

/// `run` with the steps in it charged to `function`, if an observer is
/// installed.
pub fn in_function<T>(
    function: &str,
    run: impl FnOnce() -> T,
) -> T {
    let Some(observer) = OBSERVER.get() else { return run() };
    let (mut run, mut out) = (Some(run), None);
    (observer.function)(function, &mut || out = run.take().map(|run| run()));
    out.expect("the observer ran the step")
}

pub(crate) fn counted(
    what: &'static str,
    hit: bool,
) {
    if let Some(observer) = OBSERVER.get() {
        (observer.count)(what, hit);
    }
}

/// What a function pass works on: its function, the context its types and
/// constants live in, and the program's datalayout.
pub struct Unit<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub function: &'a mut Function,
    /// The function's own id in its module, where it is one the module names: a
    /// pass that must tell a call to itself asks for it.
    pub id: Option<GlobalId>,
    /// The module's metadata nodes.
    pub metadata: &'a [crate::module::MetadataNode],
    /// Functions the pass declares in the module.
    pub declared: &'a mut Declared,
}

/// What a function pass declares in its module, as LLVM's
/// `Intrinsic::getDeclaration` adds a declaration: its id at once, the
/// declaration once the pass has run over the function.
#[derive(Clone, Debug, Default)]
pub struct Declared {
    /// The module's names, worked out when a pass first declares one: most runs
    /// declare nothing.
    ids: Option<HashMap<String, GlobalId>>,
    /// Where the names come from while `ids` is not made: the module's
    /// `Declarations`.
    held: Option<Rc<Vec<GlobalValue>>>,
    next: u32,
    pending: Vec<(String, crate::types::TypeId)>,
    /// Metadata nodes made, numbered after the module's.
    nodes: Vec<crate::module::MetadataNode>,
    first_node: u32,
}

impl Declared {
    pub fn of(module: &Module) -> Self {
        counted("declared names", false);
        let ids = module
            .globals
            .iter()
            .enumerate()
            .filter_map(|(at, one)| Some((one.name.clone()?, GlobalId(at as u32))))
            .collect();
        Self {
            ids: Some(ids),
            held: None,
            next: module.globals.len() as u32,
            pending: Vec::new(),
            nodes: Vec::new(),
            first_node: module.metadata.len() as u32,
        }
    }

    /// As `of`, over the module's `Declarations` (`held`, its `metadata` node
    /// count): nothing is scanned or copied unless a pass declares a
    /// function. LLVM's `getOrInsertFunction` is a symbol-table lookup, not a
    /// scan.
    pub fn over(
        held: Rc<Vec<GlobalValue>>,
        metadata: usize,
    ) -> Self {
        Self {
            ids: None,
            next: held.len() as u32,
            held: Some(held),
            pending: Vec::new(),
            nodes: Vec::new(),
            first_node: metadata as u32,
        }
    }

    fn names(&mut self) -> &mut HashMap<String, GlobalId> {
        let held = &self.held;
        self.ids.get_or_insert_with(|| {
            counted("declared names", false);
            held.iter()
                .flat_map(|all| all.iter())
                .enumerate()
                .filter_map(|(at, one)| Some((one.name.clone()?, GlobalId(at as u32))))
                .collect()
        })
    }

    /// A metadata node, its id at once, added to the module after the pass.
    pub fn node(
        &mut self,
        node: crate::module::MetadataNode,
    ) -> crate::module::MetadataId {
        self.nodes.push(node);
        crate::module::MetadataId(self.first_node + self.nodes.len() as u32 - 1)
    }

    /// The function `name` of type `ty`, declared where the module has none.
    pub fn declare(
        &mut self,
        name: &str,
        ty: crate::types::TypeId,
    ) -> GlobalId {
        if let Some(&id) = self.names().get(name) {
            return id;
        }
        let id = GlobalId(self.next);
        self.next += 1;
        self.names().insert(name.to_owned(), id);
        self.pending.push((name.to_owned(), ty));
        id
    }

    /// The declarations made, added to `module`; how many.
    pub fn place(
        &mut self,
        module: &mut Module,
    ) -> Result<usize, String> {
        let made = self.pending.len();
        for (name, ty) in std::mem::take(&mut self.pending) {
            let id = module.add_function(&name, ty, crate::module::Linkage::External)?;
            assert_eq!(Some(&id), self.names().get(&name), "declared in order");
        }
        assert_eq!(module.metadata.len() as u32, self.first_node, "nodes numbered after the module's");
        module.metadata.append(&mut self.nodes);
        self.first_node = module.metadata.len() as u32;
        Ok(made)
    }
}

/// A fact about a function, computed on demand and cached until a pass
/// fails to preserve it. It reads its module and target through
/// `analyses.outer()`, and asks `analyses` for the analyses it builds on.
pub trait Analysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result;

    /// Whether the result reads `analyses.outer()`, itself or through what
    /// it builds on. One that does not outlives a change to the module.
    const READS_OUTER: bool = true;

    /// Whether `update` can bring a result the function has since changed up to
    /// date: such a result is kept past an invalidation, with the point in
    /// the function's history it was true at.
    const INCREMENTAL: bool = false;

    /// Whether `unaffected` can tell a result still true after `changes`: such
    /// a result is kept past an invalidation, and stands again when
    /// `changes` leave it as it was (and, if it reads the outer facts, those
    /// are the ones it read).
    const SKIPS: bool = false;

    /// What the result reads of the function: `unaffected` filters the change
    /// log against it. None, the default, is every change.
    fn depends() -> Option<crate::depends::Depends> {
        None
    }

    /// Whether `changes` leave `previous`, true of the function before them,
    /// true of it now. Must never say so wrongly, as `LLRM_CHECK_REPLAY`
    /// asserts; saying no derives the result afresh. By default what `depends`
    /// declares says.
    fn unaffected(
        changes: &[crate::module::Change],
        context: &Context,
        function: &Function,
    ) -> bool {
        Self::depends().is_some_and(|depends| depends.unaffected(changes, context, function))
    }

    /// `previous`, which was true of the function before `changes`, made true
    /// of it now; none where it would be derived afresh. Must give what
    /// `run` gives, as `LLRM_CHECK_REPLAY` asserts.
    #[allow(unused_variables)]
    fn update(
        previous: &Self::Result,
        changes: &[crate::module::Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        None
    }

    /// `update`, handed the only reference to `previous` there is when no one
    /// else holds the result: one that can change what it holds in place does,
    /// rather than copy it.
    #[allow(unused_variables)]
    fn update_owned(
        previous: Rc<Self::Result>,
        changes: &[crate::module::Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        Self::update(&previous, changes, context, layout, function, analyses)
    }

    /// Whether a pass returning `preserved` left the result true: when it
    /// names this analysis, or keeps the function and the result reads
    /// nothing else. One derived from others asks after them, as LLVM's
    /// `Result::invalidate` does.
    fn preserved(preserved: &PreservedAnalyses) -> bool
    where
        Self: Sized,
    {
        preserved.kept::<Self>() || (!Self::READS_OUTER && preserved.function)
    }
}

/// A fact about the whole module, as LLVM's `GlobalsAA`: computed on
/// demand, before each pass once required (`PassManager::require`), and
/// dropped unless every run of a pass preserved it. It asks `analyses` for
/// the module analyses it builds on and its program. Function analyses
/// read it through `Outer::cached`.
pub trait ModuleAnalysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result;

    /// Whether `stale`, a result held past passes that add no memory operation
    /// (`freeze`), still says no more than `fresh`, what `run` gives now,
    /// does: whatever it states of the module, `fresh` states too. Equal by
    /// default.
    #[allow(unused_variables)]
    fn covers(
        stale: &Self::Result,
        fresh: &Self::Result,
    ) -> bool {
        stale == fresh
    }

    /// Whether `previous`, a result dropped by a pass, is what `run` would give
    /// now, found without working it out: the result then stands as the
    /// same one, so what holds it (an outer proxy) is the same too. Saying no,
    /// the default, runs.
    #[allow(unused_variables)]
    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        previous: &Self::Result,
    ) -> bool {
        false
    }
}

/// The declarations an analysis last worked from: its result stands while
/// `Declarations` is the same result.
struct Seen<A>(Option<Rc<Vec<GlobalValue>>>, std::marker::PhantomData<A>);

impl<A> Default for Seen<A> {
    fn default() -> Self {
        Self(None, std::marker::PhantomData)
    }
}

/// Whether the declarations are those `A` last worked from, and recording them
/// as the ones it works from now.
fn declared_as_before<A: 'static>(
    module: &Module,
    analyses: &mut ModuleAnalyses,
    working: bool,
) -> bool {
    let now = analyses.get::<Declarations>(module);
    let seen = analyses.memo::<Seen<A>>();
    let same = seen.0.as_ref().is_some_and(|then| Rc::ptr_eq(then, &now));
    if working {
        seen.0 = Some(now);
    }
    same
}

/// What each function does to memory, as its attributes state it: of its
/// declaration alone, so it stands while the declarations do.
pub struct CalleeEffects;

impl ModuleAnalysis for CalleeEffects {
    type Result = crate::memory::Callees;
    const NAME: &'static str = "callee-effects";
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        declared_as_before::<Self>(module, analyses, true);
        crate::memory::callees(module)
    }

    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        _: &Self::Result,
    ) -> bool {
        declared_as_before::<Self>(module, analyses, false)
    }
}

/// How many registers a call to each function keeps, as the target says
/// of it by name.
pub struct CallRegisters;

impl ModuleAnalysis for CallRegisters {
    type Result = HashMap<GlobalId, i64>;
    const NAME: &'static str = "call-registers";
    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        _: &Self::Result,
    ) -> bool {
        declared_as_before::<Self>(module, analyses, false)
    }

    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        declared_as_before::<Self>(module, analyses, true);
        let target = Rc::clone(&analyses.program().target);
        module
            .globals
            .iter()
            .enumerate()
            .filter(|(_, global)| matches!(global.kind, crate::module::GlobalKind::Function(_)))
            .map(|(at, global)| (GlobalId(at as u32), target.kept_across(global.name.as_deref())))
            .collect()
    }
}

/// Each global variable's size in bytes.
pub struct GlobalSizes;

impl ModuleAnalysis for GlobalSizes {
    type Result = crate::valuetracking::Sizes;
    const NAME: &'static str = "global-sizes";
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        declared_as_before::<Self>(module, analyses, true);
        crate::valuetracking::sizes(module, &analyses.program().layout)
    }

    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        _: &Self::Result,
    ) -> bool {
        declared_as_before::<Self>(module, analyses, false)
    }
}

/// The metadata the type tree was last made from.
#[derive(Default)]
struct TbaaSeen(Option<Vec<MetadataNode>>);

/// The module's `!tbaa` type tree.
pub struct TypeAncestry;

impl ModuleAnalysis for TypeAncestry {
    type Result = crate::tbaa::Tbaa;
    const NAME: &'static str = "type-ancestry";
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        analyses.metadata_open = false;
        analyses.memo::<TbaaSeen>().0 = Some(module.metadata.clone());
        crate::tbaa::Tbaa::of(&module.metadata)
    }

    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        _: &Self::Result,
    ) -> bool {
        // Function passes add metadata nodes and change none: where only bodies
        // were edited the length says.
        let appended_only = !analyses.metadata_open;
        analyses
            .memo::<TbaaSeen>()
            .0
            .as_ref()
            .is_some_and(
                |then| if appended_only { then.len() == module.metadata.len() } else { *then == module.metadata },
            )
            .then(|| analyses.metadata_open = false)
            .is_some()
    }
}

/// Every global as its declaration, by id: what a function may read of
/// the others.
pub struct Declarations;

impl ModuleAnalysis for Declarations {
    type Result = Vec<GlobalValue>;
    const NAME: &'static str = "declarations";
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        analyses.declarations_verified();
        module.declarations()
    }

    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        previous: &Self::Result,
    ) -> bool {
        let same = module.globals.len() == previous.len()
            && match analyses.declarations_edits() {
                Some(edited) => edited.iter().all(|id| module.global(*id).declares(&previous[id.0 as usize])),
                None => module.globals.iter().zip(previous).all(|(global, declared)| global.declares(declared)),
            };
        if same {
            analyses.declarations_verified();
        }
        same
    }
}

/// LLVM's `DominatorTreeAnalysis`.
pub struct Dominators;

impl Analysis for Dominators {
    type Result = DominatorTree;
    const READS_OUTER: bool = false;
    const NAME: &'static str = "dominators";
    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> DominatorTree {
        DominatorTree::new(function)
    }
}

/// LLVM's `LoopAnalysis`.
pub struct Loops;

impl Analysis for Loops {
    type Result = crate::loops::LoopInfo;
    const READS_OUTER: bool = false;
    const NAME: &'static str = "loops";
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> crate::loops::LoopInfo {
        crate::loops::LoopInfo::new(function, &analyses.get::<Dominators>(context, layout, function))
    }
}

/// LLVM's `ScalarEvolutionAnalysis`, of add recurrences alone.
pub struct ScalarEvolution;

impl Analysis for ScalarEvolution {
    type Result = crate::scalarevolution::Evolution;
    const READS_OUTER: bool = false;
    const NAME: &'static str = "scalar-evolution";
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> crate::scalarevolution::Evolution {
        crate::scalarevolution::Evolution::new(context, function, &analyses.get::<Loops>(context, layout, function))
    }
}

/// Which analyses a pass left true.
#[derive(Clone, Debug, Default)]
pub struct PreservedAnalyses {
    all: bool,
    kept: HashSet<TypeId>,
    /// The function itself unchanged, whatever else did.
    function: bool,
}

impl PreservedAnalyses {
    pub fn all() -> Self {
        Self { all: true, kept: HashSet::default(), function: true }
    }

    pub fn none() -> Self {
        Self::default()
    }

    /// What a change to the module leaves of a function it did not change:
    /// every analysis reading only the function.
    pub fn function() -> Self {
        Self { function: true, ..Self::default() }
    }

    pub fn preserve<A: Analysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<A>());
        self
    }

    pub fn preserve_module<M: ModuleAnalysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<M>());
        self
    }

    /// `self`, or everything where `function` logged no change since `before`:
    /// a pass that edited nothing left every analysis true, whatever it
    /// says.
    #[must_use]
    pub fn unless_unchanged(
        self,
        function: &Function,
        before: crate::module::Mark,
    ) -> Self {
        if function.changes_since(before).is_some_and(<[crate::module::Change]>::is_empty) { Self::all() } else { self }
    }

    /// Whether the pass changed nothing: LLVM's `areAllPreserved`.
    pub fn are_all_preserved(&self) -> bool {
        self.all
    }

    /// Whether `A` is kept, named or with everything.
    pub fn kept<A: Analysis>(&self) -> bool {
        self.keeps(TypeId::of::<A>())
    }

    fn keeps(
        &self,
        analysis: TypeId,
    ) -> bool {
        self.all || self.kept.contains(&analysis)
    }
}

/// What a function analysis may read beyond its function: LLVM's outer
/// analysis manager proxy, read-only. The module's metadata and globals,
/// each function by its declaration alone since a function analysis reads
/// no other body; its program; and the module analyses required.
#[derive(Clone)]
pub struct Outer {
    pub metadata: Vec<MetadataNode>,
    pub globals: Rc<Vec<GlobalValue>>,
    program: Rc<ProgramProxy>,
    modules: HashMap<TypeId, Rc<dyn Any>>,
}

impl Outer {
    /// `module`'s, a program of its own for `target`, or a neutral one:
    /// for analyses asked outside a pass manager.
    pub fn of(
        module: &Module,
        target: Option<Rc<dyn Machine>>,
    ) -> Self {
        (*ModuleAnalyses::of(module, target.unwrap_or_else(|| Rc::new(Neutral))).outer(module)).clone()
    }

    /// Computes `M` of `module`, for analyses asked outside a pass manager.
    pub fn require<M: ModuleAnalysis>(
        &mut self,
        module: &Module,
    ) {
        let result = ModuleAnalyses::new(Rc::clone(&self.program)).get::<M>(module);
        self.modules.insert(TypeId::of::<M>(), result);
    }

    pub fn program(&self) -> &ProgramProxy {
        &self.program
    }

    pub fn target(&self) -> &dyn Machine {
        &*self.program.target
    }

    /// How many registers a call to `callee` keeps, any call's where it is
    /// not named: `CallRegisters`.
    pub fn kept_across(
        &self,
        callee: Option<GlobalId>,
    ) -> i64 {
        let kept = self.cached_ref::<CallRegisters>().expect("every outer proxy holds the calls' registers");
        callee.and_then(|one| kept.get(&one).copied()).unwrap_or_else(|| self.target().call_registers())
    }

    /// What each function does to memory: `CalleeEffects`.
    pub fn callees(&self) -> &crate::memory::Callees {
        self.cached_ref::<CalleeEffects>().expect("every outer proxy holds the callees' effects")
    }

    /// Each global variable's size: `GlobalSizes`.
    pub fn sizes(&self) -> &crate::valuetracking::Sizes {
        self.cached_ref::<GlobalSizes>().expect("every outer proxy holds the globals' sizes")
    }

    /// The type tree of the module's `!tbaa` nodes: `TypeAncestry`.
    pub fn tbaa(&self) -> &crate::tbaa::Tbaa {
        self.cached_ref::<TypeAncestry>().expect("every outer proxy holds the type tree")
    }

    /// `M`'s result, if computed: LLVM's `getCachedResult`.
    pub fn cached<M: ModuleAnalysis>(&self) -> Option<Rc<M::Result>> {
        self.modules
            .get(&TypeId::of::<M>())
            .map(|one| Rc::clone(one).downcast::<M::Result>().expect("keyed by its type"))
    }

    /// `cached`, borrowed for as long as the proxy.
    pub fn cached_ref<M: ModuleAnalysis>(&self) -> Option<&M::Result> {
        self.modules.get(&TypeId::of::<M>()).map(|one| one.downcast_ref::<M::Result>().expect("keyed by its type"))
    }

    /// Whether an analysis computed under `other` read what this holds.
    fn same(
        &self,
        other: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.program, &other.program)
            && self.metadata == other.metadata
            && self.globals == other.globals
            && self.modules.len() == other.modules.len()
            && self.modules.iter().all(|(key, one)| other.modules.get(key).is_some_and(|two| Rc::ptr_eq(one, two)))
    }
}

/// A cached result, able to check itself against a fresh computation.
trait Cached {
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn still_true(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        outer: &Rc<Outer>,
    ) -> bool;
    fn preserved(
        &self,
        preserved: &PreservedAnalyses,
    ) -> bool;
    fn incremental(&self) -> bool;
    /// The function's history when the result was derived.
    fn mark(&self) -> crate::module::Mark;
}

struct Entry<A: Analysis> {
    result: Rc<A::Result>,
    /// What the result may have read of the module.
    outer: Rc<Outer>,
    /// The function's history when the result was derived.
    mark: crate::module::Mark,
}

impl<A: Analysis> Cached for Entry<A> {
    fn name(&self) -> &'static str {
        A::NAME
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn still_true(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        outer: &Rc<Outer>,
    ) -> bool {
        *self.result == A::run(context, layout, function, &mut Analyses::new(Rc::clone(outer)))
    }

    fn preserved(
        &self,
        preserved: &PreservedAnalyses,
    ) -> bool {
        A::preserved(preserved)
    }

    fn incremental(&self) -> bool {
        A::INCREMENTAL || A::SKIPS
    }

    fn mark(&self) -> crate::module::Mark {
        self.mark
    }
}

/// One function's cached analyses, and what they may read of its module.
pub struct Analyses {
    cache: HashMap<TypeId, Box<dyn Cached>>,
    /// Results invalidated that `update` may bring up to date.
    kept: HashMap<TypeId, Box<dyn Cached>>,
    evicted: HashMap<TypeId, Box<dyn Cached>>,
    /// The history of the function when a pass last vouched for what it kept
    /// (`invalidate`): a result derived before and held is as true now as
    /// then.
    vouched: Option<crate::module::Mark>,
    outer: Rc<Outer>,
}

thread_local! {
    static PASS: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
    static TRACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static RECOMPUTED: std::cell::RefCell<std::collections::BTreeMap<(&'static str, &'static str, &'static str), usize>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// The pass whose invalidation a recomputation is charged to: set where passes
/// invalidate.
pub fn note_pass(name: &'static str) {
    PASS.with(|p| p.set(name));
}

/// `LLRM_WHY`, or `trace_recomputes`: every analysis computed again says
/// whether it came to what it was. Off, the manager keeps nothing and compares
/// nothing.
fn why() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    TRACE.with(std::cell::Cell::get) || *ON.get_or_init(|| std::env::var_os("LLRM_WHY").is_some())
}

/// Starts or stops counting recomputations on this thread (the test form of
/// `LLRM_WHY`).
pub fn trace_recomputes(on: bool) {
    TRACE.with(|t| t.set(on));
    if on {
        RECOMPUTED.with(|r| r.borrow_mut().clear());
    }
}

/// How often each analysis was computed, by the pass that last invalidated it
/// and how it came out: `first` (nothing was held), `same` (the last result,
/// worked out again) and `diff`, or for a function analysis kept for update,
/// `replayed`, `kept-rerun-same` and `kept-rerun-diff`.
pub fn recomputes() -> Vec<(&'static str, &'static str, &'static str, usize)> {
    RECOMPUTED.with(|r| r.borrow().iter().map(|(&(name, pass, how), &n)| (name, pass, how, n)).collect())
}

fn record(
    level: &str,
    name: &'static str,
    how: &'static str,
) {
    let pass = PASS.with(|p| p.get());
    RECOMPUTED.with(|r| *r.borrow_mut().entry((name, pass, how)).or_default() += 1);
    if std::env::var_os("LLRM_WHY").is_some() {
        eprintln!("WHY {level} {name} {pass} {how}");
    }
}

fn check_replay() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_REPLAY").is_some())
}

impl Analyses {
    pub fn new(outer: Rc<Outer>) -> Self {
        Self { cache: HashMap::default(), kept: HashMap::default(), evicted: HashMap::default(), vouched: None, outer }
    }

    /// An empty cache over the same module and target, for another body.
    pub fn fresh(&self) -> Self {
        Self::new(Rc::clone(&self.outer))
    }

    pub fn outer(&self) -> &Rc<Outer> {
        &self.outer
    }

    /// `A`'s result, if computed: LLVM's `getCachedResult`.
    pub fn cached<A: Analysis>(&self) -> Option<Rc<A::Result>> {
        self.cache
            .get(&TypeId::of::<A>())
            .map(|entry| Rc::clone(&entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type").result))
    }

    /// `A`'s result for `function`, computed once until invalidated.
    pub fn get<A: Analysis>(
        &mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Rc<A::Result> {
        let key = TypeId::of::<A>();
        if let Some(entry) = self.cache.get(&key) {
            let entry = entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
            counted(A::NAME, true);
            return Rc::clone(&entry.result);
        }
        counted(A::NAME, false);
        let had_kept = self.kept.contains_key(&key);
        let old_kept = if why() {
            self.kept.get(&key).map(|o| {
                (Rc::clone(&o.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type").result), o.mark())
            })
        } else {
            None
        };
        let updated = self.kept.remove(&key).and_then(|held| {
            // Taken out of its box, so that `update_owned` is handed the only
            // reference there is.
            let (result, outer, mark) = {
                let old = held.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
                (Rc::clone(&old.result), Rc::clone(&old.outer), old.mark)
            };
            drop(held);
            let changes = function.changes_since(mark)?;
            // Asking of a long log again and again is more than the run it
            // saves.
            if A::SKIPS
                && changes.len() <= 256
                && (!A::READS_OUTER || Rc::ptr_eq(&outer, &self.outer))
                && A::unaffected(changes, context, function)
            {
                if check_replay() {
                    let whole = A::run(context, layout, function, &mut Analyses::new(Rc::clone(&self.outer)));
                    assert!(
                        *result == whole,
                        "{}: the result kept past {} changes is not what deriving it afresh gives: {:?}",
                        A::NAME,
                        changes.len(),
                        changes
                            .iter()
                            .map(|c| match *c {
                                crate::module::Change::Inserted { inst, .. }
                                | crate::module::Change::Erased { inst, .. }
                                | crate::module::Change::Moved { inst, .. }
                                | crate::module::Change::Rewritten(inst)
                                | crate::module::Change::Cloned { to: inst, .. } =>
                                    format!("{:?}:{}", c, function.instruction(inst).opcode.mnemonic()),
                                _ => format!("{c:?}"),
                            })
                            .collect::<Vec<_>>()
                    );
                }
                return Some(Rc::clone(&result));
            }
            let made =
                spanned_as("analysis", A::NAME, || A::update_owned(result, changes, context, layout, function, self))?;
            if check_replay() {
                let whole = A::run(context, layout, function, &mut Analyses::new(Rc::clone(&self.outer)));
                assert!(
                    made == whole,
                    "{}: the result brought up to date is not what deriving it afresh gives",
                    A::NAME
                );
            }
            Some(Rc::new(made))
        });
        let replayed = updated.is_some();
        let result = match updated {
            Some(made) => made,
            None => Rc::new(spanned_as("analysis", A::NAME, || A::run(context, layout, function, self))),
        };
        if why() {
            let class = match self.evicted.remove(&key) {
                Some(old) => {
                    let old = old.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
                    match (*old.result == *result, old.mark == function.mark()) {
                        (true, true) => "same-untouched",
                        (true, false) => "same",
                        (false, true) => "diff-untouched",
                        (false, false) => "diff",
                    }
                }
                None if replayed => "replayed",
                None if had_kept => match &old_kept {
                    Some((old, mark)) if **old == *result => {
                        if *mark == function.mark() {
                            "kept-rerun-same-untouched"
                        } else {
                            "kept-rerun-same"
                        }
                    }
                    _ => "kept-rerun-diff",
                },
                None => "first",
            };
            record("fn", A::NAME, class);
        }
        self.cache.insert(
            key,
            Box::new(Entry::<A> { result: Rc::clone(&result), outer: Rc::clone(&self.outer), mark: function.mark() }),
        );
        result
    }

    /// `LLRM_CHECK_PRESERVED`: every analysis still held after `pass` is what a
    /// fresh run gives.
    pub fn check_kept(
        &self,
        pass: &str,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_PRESERVED").is_some()) {
            let stale = self.stale(context, layout, function);
            assert!(stale.is_empty(), "{pass} left {} held but stale", stale.join(", "));
        }
    }

    /// Drops what `preserved` does not keep: LLVM's
    /// `FunctionAnalysisManager::invalidate`, for a pass running others. What
    /// it keeps the pass vouches for as `function` is now: it is stamped
    /// with this history, so that an edit made after, and not reported, is
    /// told from the pass's own.
    pub fn invalidate(
        &mut self,
        function: &Function,
        preserved: &PreservedAnalyses,
    ) {
        self.drop_unpreserved(preserved);
        self.vouched = Some(function.mark());
    }

    /// Every result held stands, as `function` is now: a pass that edited and
    /// put back, or that changed nothing.
    pub fn vouch(
        &mut self,
        function: &Function,
    ) {
        self.vouched = Some(function.mark());
    }

    fn drop_unpreserved(
        &mut self,
        preserved: &PreservedAnalyses,
    ) {
        let gone: Vec<TypeId> =
            self.cache.iter().filter(|(_, entry)| !entry.preserved(preserved)).map(|(key, _)| *key).collect();
        for key in gone {
            let entry = self.cache.remove(&key).expect("held");
            if entry.incremental() {
                self.kept.insert(key, entry);
            } else if why() {
                self.evicted.insert(key, entry);
            }
        }
    }

    /// What `function` was edited without saying so since each cached result
    /// was derived or vouched for: those results are no longer held as they
    /// stand, and are brought up to date from the history (or computed
    /// again) when asked. A module pass that changes a body and does not
    /// call `ModuleAnalyses::changed` would otherwise read what the
    /// body was (the shape a splice's new loop was not in, a GlobalsAA that
    /// never ended). `LLRM_CHECK_UNREPORTED` makes it a failure, naming the
    /// analysis.
    fn forget_unreported(
        &mut self,
        function: &Function,
    ) {
        let now = function.mark();
        if self.vouched == Some(now) {
            // Every result held was vouched for at this history, or derived at
            // it.
            return;
        }
        let behind: Vec<TypeId> =
            self.cache.iter().filter(|(_, entry)| entry.mark() != now).map(|(key, _)| *key).collect();
        for key in behind {
            let entry = self.cache.remove(&key).expect("held");
            if unreported_fails() && entry.mark().same_history(now) {
                use crate::module::Change;
                let edits: Vec<String> = function
                    .changes_since(entry.mark())
                    .map_or_else(
                        Vec::new,
                        |changes| changes
                            .iter()
                            .map(|change| match *change {
                                Change::Inserted { inst, .. } => {
                                    format!("insert {}", function.instruction(inst).opcode.mnemonic())
                                }
                                Change::Erased { inst, .. } => {
                                    format!("erase {}", function.instruction(inst).opcode.mnemonic())
                                }
                                Change::Rewritten(inst) => {
                                    format!("rewrite {}", function.instruction(inst).opcode.mnemonic())
                                }
                                Change::Moved { inst, .. } => {
                                    format!("move {}", function.instruction(inst).opcode.mnemonic())
                                }
                                ref other => format!("{other:?}"),
                            })
                            .collect(),
                    );
                eprintln!("EDITS {edits:?}");
            }
            // A function replaced by a copy (a trial put back, a transaction
            // committed) is another history, not an edit that was not said.
            assert!(
                !unreported_fails() || !entry.mark().same_history(now),
                "{} was read after an edit that was not reported (the body went from {:?} to {now:?}, last pass {}): call `changed` as the edit is made",
                entry.name(),
                entry.mark(),
                PASS.with(std::cell::Cell::get)
            );
            if entry.incremental() {
                self.kept.insert(key, entry);
            } else if why() {
                self.evicted.insert(key, entry);
            }
        }
    }

    /// The cached analyses a fresh computation disagrees with.
    fn stale(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self
            .cache
            .values()
            .filter(|one| !one.still_true(context, layout, function, &self.outer))
            .map(|one| one.name())
            .collect();
        out.sort_unstable();
        out
    }
}

/// `LLRM_CHECK_UNREPORTED`: an analysis of a function read after an edit that
/// was not reported fails, where otherwise it is brought up to date.
fn unreported_fails() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_UNREPORTED").is_some())
}

/// A module analysis: how to compute it, and whether two results agree.
#[derive(Clone, Copy)]
pub(crate) struct Kind {
    id: TypeId,
    name: &'static str,
    run: fn(&Module, &mut ModuleAnalyses) -> Rc<dyn Any>,
    agree: fn(&dyn Any, &dyn Any) -> bool,
    covers: fn(&dyn Any, &dyn Any) -> bool,
    unchanged: fn(&Module, &mut ModuleAnalyses, &dyn Any) -> bool,
}

impl Kind {
    fn of<M: ModuleAnalysis>() -> Self {
        Self {
            id: TypeId::of::<M>(),
            name: M::NAME,
            run: computed::<M>,
            agree: agree::<M>,
            covers: covers::<M>,
            unchanged: unchanged::<M>,
        }
    }
}

fn covers<M: ModuleAnalysis>(
    stale: &dyn Any,
    fresh: &dyn Any,
) -> bool {
    match (stale.downcast_ref::<M::Result>(), fresh.downcast_ref::<M::Result>()) {
        (Some(stale), Some(fresh)) => M::covers(stale, fresh),
        _ => false,
    }
}

fn unchanged<M: ModuleAnalysis>(
    module: &Module,
    analyses: &mut ModuleAnalyses,
    previous: &dyn Any,
) -> bool {
    previous.downcast_ref::<M::Result>().is_some_and(|previous| M::unchanged(module, analyses, previous))
}

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many module analyses this thread has run (not counting those kept as
/// they were), for a test that an unchanged one is not.
pub fn module_runs() -> usize {
    RUNS.with(std::cell::Cell::get)
}

fn computed<M: ModuleAnalysis>(
    module: &Module,
    analyses: &mut ModuleAnalyses,
) -> Rc<dyn Any> {
    RUNS.with(|runs| runs.set(runs.get() + 1));
    Rc::new(spanned_as("analysis", M::NAME, || M::run(module, analyses)))
}

fn agree<M: ModuleAnalysis>(
    one: &dyn Any,
    other: &dyn Any,
) -> bool {
    one.downcast_ref::<M::Result>() == other.downcast_ref::<M::Result>()
}

/// One module's cached analyses, what they may read of its program, and
/// each function's manager: LLVM's `ModuleAnalysisManager` with its
/// `FunctionAnalysisManagerModuleProxy`. The pass manager and a module pass
/// rerunning function passes share one.
pub struct ModuleAnalyses {
    program: Rc<ProgramProxy>,
    /// What every outer proxy holds, beside the callees' effects, the
    /// globals' sizes and declarations.
    required: Vec<Kind>,
    results: HashMap<TypeId, (Kind, Rc<dyn Any>)>,
    /// Results dropped, reused when computed again the same, so that an
    /// outer proxy holding them stays the same.
    dropped: HashMap<TypeId, Rc<dyn Any>>,
    outer: Option<Rc<Outer>>,
    functions: HashMap<GlobalId, Analyses>,
    /// The bodies reported edited since the declarations were last checked
    /// against the module, and whether anything else may have changed them
    /// (`invalidate`): a pass that edits bodies says which, and the
    /// declarations of the rest then stand without a look at each.
    edited: BTreeSet<GlobalId>,
    declarations_open: bool,
    metadata_open: bool,
    /// What an analysis keeps for its next run, by its type: the working of an
    /// update that reuses the last.
    memos: HashMap<TypeId, Box<dyn Any>>,
    /// `LLRM_CHECK_MODULES`: an analysis being run again to check what it
    /// brought up to date works everything out afresh.
    scratch: bool,
}

impl ModuleAnalyses {
    pub fn new(program: Rc<ProgramProxy>) -> Self {
        Self {
            program,
            required: Vec::new(),
            results: HashMap::default(),
            dropped: HashMap::default(),
            outer: None,
            functions: HashMap::default(),
            edited: BTreeSet::new(),
            declarations_open: true,
            metadata_open: true,
            memos: HashMap::default(),
            scratch: false,
        }
    }

    /// The `T` an analysis left for its next run, made empty the first time:
    /// what survives `invalidate`, for an analysis that brings its last
    /// result up to date instead of working it out again. Whether an
    /// analysis is to work everything out afresh and keep nothing of its last
    /// run (`LLRM_CHECK_MODULES` runs it so, to check what it brought up to
    /// date).
    pub fn from_scratch(&self) -> bool {
        self.scratch
    }

    pub fn memo<T: Default + 'static>(&mut self) -> &mut T {
        self.memos
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut::<T>()
            .expect("keyed by its type")
    }

    /// `module`'s, a program of its own for `target`: for analyses asked
    /// outside a pass manager.
    pub fn of(
        module: &Module,
        target: Rc<dyn Machine>,
    ) -> Self {
        Self::new(ProgramProxy::of(module, target))
    }

    pub fn program(&self) -> &Rc<ProgramProxy> {
        &self.program
    }

    /// Keeps `M` in every outer proxy, as LLVM's `RequireAnalysisPass`.
    pub fn require<M: ModuleAnalysis>(&mut self) {
        if !self.required.iter().any(|one| one.id == TypeId::of::<M>()) {
            self.required.push(Kind::of::<M>());
        }
    }

    /// `M`'s result for `module`, computed once until invalidated.
    pub fn get<M: ModuleAnalysis>(
        &mut self,
        module: &Module,
    ) -> Rc<M::Result> {
        self.computed(Kind::of::<M>(), module).downcast::<M::Result>().expect("keyed by its type")
    }

    /// `M`'s result, if computed.
    pub fn cached<M: ModuleAnalysis>(&self) -> Option<Rc<M::Result>> {
        self.results
            .get(&TypeId::of::<M>())
            .map(|(_, one)| Rc::clone(one).downcast::<M::Result>().expect("keyed by its type"))
    }

    fn computed(
        &mut self,
        kind: Kind,
        module: &Module,
    ) -> Rc<dyn Any> {
        if let Some((_, one)) = self.results.get(&kind.id) {
            counted(kind.name, true);
            return Rc::clone(one);
        }
        if let Some(old) = self.dropped.get(&kind.id).cloned() {
            if (kind.unchanged)(module, self, &*old) {
                counted(kind.name, true);
                self.dropped.remove(&kind.id);
                if std::env::var_os("LLRM_CHECK_MODULES").is_some() {
                    let fresh = (kind.run)(module, self);
                    assert!(
                        (kind.agree)(&*old, &*fresh),
                        "{}: the result kept is not what working it out again gives",
                        kind.name
                    );
                }
                self.results.insert(kind.id, (kind, Rc::clone(&old)));
                return old;
            }
        }
        counted(kind.name, false);
        let fresh = (kind.run)(module, self);
        if std::env::var_os("LLRM_CHECK_MODULES").is_some() {
            self.scratch = true;
            let again = (kind.run)(module, self);
            self.scratch = false;
            assert!(
                (kind.agree)(&*fresh, &*again),
                "{}: brought up to date, it is not what working it out afresh gives",
                kind.name
            );
        }
        if why() {
            let class = match self.dropped.get(&kind.id) {
                Some(old) => {
                    if (kind.agree)(&**old, &*fresh) {
                        "same"
                    } else {
                        "diff"
                    }
                }
                None => "first",
            };
            record("mod", kind.name, class);
        }
        let result = self.dropped.remove(&kind.id).filter(|old| (kind.agree)(&**old, &*fresh)).unwrap_or(fresh);
        self.results.insert(kind.id, (kind, Rc::clone(&result)));
        result
    }

    /// Drops what `preserved` does not keep.
    pub fn invalidate(
        &mut self,
        preserved: &PreservedAnalyses,
    ) {
        self.declarations_open = true;
        self.metadata_open = true;
        self.invalidate_bodies(preserved);
    }

    /// `invalidate` for a pass that edited only bodies, each reported to
    /// `changed` (or `body_edited`) and none of them touching the metadata
    /// or another global's declaration: the declarations and the
    /// metadata are checked against those bodies alone.
    pub fn invalidate_bodies(
        &mut self,
        preserved: &PreservedAnalyses,
    ) {
        let gone: Vec<TypeId> = self.results.keys().filter(|one| !preserved.keeps(**one)).copied().collect();
        for one in gone {
            let (_, result) = self.results.remove(&one).expect("held");
            self.dropped.insert(one, result);
        }
    }

    /// `A` of function `id`, from its manager. `A` reads nothing through
    /// `Outer`: the module analysis asking may be one the outer proxy holds.
    pub fn function<A: Analysis>(
        &mut self,
        module: &Module,
        id: GlobalId,
    ) -> Rc<A::Result> {
        let function = module.global(id).function().expect("a function");
        if !self.functions.contains_key(&id) {
            let outer = match &self.outer {
                Some(outer) => Rc::clone(outer),
                None => Rc::new(Outer {
                    metadata: Vec::new(),
                    globals: Rc::default(),
                    program: Rc::clone(&self.program),
                    modules: HashMap::default(),
                }),
            };
            self.functions.insert(id, Analyses::new(outer));
        }
        let layout = self.program.layout.clone();
        let analyses = self.functions.get_mut(&id).expect("inserted above");
        analyses.forget_unreported(function);
        analyses.get::<A>(&module.context, &layout, function)
    }

    /// `A` of function `id`, if its manager holds it.
    pub fn cached_function<A: Analysis>(
        &self,
        id: GlobalId,
    ) -> Option<Rc<A::Result>> {
        self.functions.get(&id)?.cached::<A>()
    }

    /// Function `id`'s manager under `outer`, keeping only what reads no
    /// outer facts where `outer` is not the one its results read.
    pub fn manager(
        &mut self,
        id: GlobalId,
        outer: &Rc<Outer>,
    ) -> &mut Analyses {
        let cache = self.functions.entry(id).or_insert_with(|| Analyses::new(Rc::clone(outer)));
        if !Rc::ptr_eq(&cache.outer, outer) {
            cache.drop_unpreserved(&PreservedAnalyses::function());
            cache.outer = Rc::clone(outer);
        }
        cache
    }

    /// Drops function `id`'s analyses, as a module pass does to a body it
    /// changed.
    pub fn changed(
        &mut self,
        id: GlobalId,
    ) {
        self.functions.remove(&id);
        self.edited.insert(id);
    }

    /// Body `id` was edited by a function pass over it alone, which left
    /// `preserved`.
    pub fn body_edited(
        &mut self,
        id: GlobalId,
        preserved: &PreservedAnalyses,
    ) {
        self.edited.insert(id);
        self.invalidate_bodies(preserved);
    }

    /// The bodies the declarations are to be checked against, or `None` where
    /// anything may have changed.
    fn declarations_edits(&self) -> Option<&BTreeSet<GlobalId>> {
        (!self.declarations_open).then_some(&self.edited)
    }

    fn declarations_verified(&mut self) {
        self.declarations_open = false;
        self.edited.clear();
    }

    /// What a function analysis reads of `module`: the same proxy as last
    /// time where nothing it holds changed.
    pub fn outer(
        &mut self,
        module: &Module,
    ) -> Rc<Outer> {
        let every = [
            Kind::of::<CalleeEffects>(),
            Kind::of::<CallRegisters>(),
            Kind::of::<GlobalSizes>(),
            Kind::of::<Declarations>(),
            Kind::of::<TypeAncestry>(),
        ]
        .into_iter()
        .chain(self.required.clone());
        let modules: HashMap<TypeId, Rc<dyn Any>> = every.map(|kind| (kind.id, self.computed(kind, module))).collect();
        // The declarations and the metadata are results among these: where
        // every result is the one the proxy holds, so are they, and
        // neither is read again.
        if let Some(old) = self.outer.as_ref().filter(|old| {
            Rc::ptr_eq(&old.program, &self.program)
                && old.modules.len() == modules.len()
                && modules.iter().all(|(key, one)| old.modules.get(key).is_some_and(|two| Rc::ptr_eq(one, two)))
        }) {
            return Rc::clone(old);
        }
        let globals = Rc::clone(&modules[&TypeId::of::<Declarations>()])
            .downcast::<Vec<GlobalValue>>()
            .expect("keyed by its type");
        let now = Outer { metadata: module.metadata.clone(), globals, program: Rc::clone(&self.program), modules };
        if !self.outer.as_ref().is_some_and(|old| old.same(&now)) {
            self.outer = Some(Rc::new(now));
        }
        Rc::clone(self.outer.as_ref().expect("set above"))
    }

    /// `outer`, without reading the module again where nothing was dropped
    /// since: for a caller that asks again and again between changes (a
    /// decision per call site) and so pays the module's size for each.
    pub fn outer_held(
        &mut self,
        module: &Module,
    ) -> Rc<Outer> {
        match &self.outer {
            Some(held) if self.dropped.is_empty() => Rc::clone(held),
            _ => self.outer(module),
        }
    }

    /// The held results among `kept` a fresh computation disagrees with.
    fn stale(
        &self,
        module: &Module,
        kept: &HashSet<TypeId>,
    ) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self
            .results
            .iter()
            .filter(|(id, (kind, result))| {
                kept.contains(id)
                    && !(kind.agree)(
                        &**result,
                        &*(kind.run)(module, &mut ModuleAnalyses::new(Rc::clone(&self.program))),
                    )
            })
            .map(|(_, (kind, _))| kind.name)
            .collect();
        out.sort_unstable();
        out
    }
}

pub trait FunctionPass {
    fn name(&self) -> &'static str;
    /// Whether the pass may add a memory operation a function did not have (a
    /// load speculated, a call made). The module analyses frozen at a point
    /// of the pipeline (`PassManager::freeze`) are held past a pass that says
    /// it does not: what they state of a function stays an upper bound of
    /// what it does. `LLRM_CHECK_STALE` recomputes them after such a pass
    /// and says if it did.
    fn adds_memory_operations(&self) -> bool {
        true
    }
    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses;
}

/// A pass over the whole module, as LLVM's inliner works across functions:
/// it answers which functions it changed. It asks `analyses` for module
/// analyses and its program, and drops those it invalidates as it goes.
pub trait ModulePass {
    fn name(&self) -> &'static str;
    /// As `FunctionPass::adds_memory_operations`.
    fn adds_memory_operations(&self) -> bool {
        true
    }
    fn run(
        &mut self,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
    ) -> Vec<GlobalId>;
}

pub(crate) enum Pass {
    Function(Box<dyn FunctionPass>),
    Module(Box<dyn ModulePass>),
    /// Over every module at once, between the module-by-module runs of the
    /// passes before and after it.
    Program(Box<dyn ProgramPass>),
    /// Not a pass: from here the module analyses it names are not dropped by a
    /// pass that adds no memory operation.
    Freeze(Vec<Kind>),
}

impl Pass {
    fn name(&self) -> &'static str {
        match self {
            Pass::Function(pass) => pass.name(),
            Pass::Module(pass) => pass.name(),
            Pass::Program(pass) => pass.name(),
            Pass::Freeze(_) => "freeze",
        }
    }
}

/// One pass over one function, and what it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub pass: &'static str,
    /// The function's module, by its index in the program.
    pub module: usize,
    pub function: GlobalId,
    pub changes: Vec<Change>,
}

#[derive(Default)]
pub struct PassManager {
    pub(crate) passes: Vec<Pass>,
    pub verify_each: bool,
    pub verify_invalidation: bool,
    /// A directory each pass's output is written to, as `NN-pass.ll`.
    pub dump: Option<std::path::PathBuf>,
    /// How many pass runs happen, a module pass's one and a function
    /// pass's one per function, as LLVM's `-opt-bisect-limit`: each run is
    /// named on stderr, and those past the limit are skipped.
    pub bisect: Option<usize>,
    pub(crate) required: Vec<Kind>,
    /// Program analyses computed before each module's run.
    pub(crate) program_required: Vec<fn(&Program, &mut ProgramAnalyses)>,
    /// Pass runs so far, for `bisect`.
    pub(crate) runs: usize,
}

impl PassManager {
    pub fn add(
        &mut self,
        pass: impl FunctionPass + 'static,
    ) {
        self.passes.push(Pass::Function(Box::new(pass)));
    }

    /// From this point of the pipeline `M` is held as it is past every pass
    /// that adds no memory operation: gcc computes its modref and points-to
    /// summaries at fixed points and the passes after them read that one, which
    /// stays conservative.
    pub fn freeze<M: ModuleAnalysis>(&mut self) {
        self.passes.push(Pass::Freeze(vec![Kind::of::<M>()]));
    }

    pub fn add_module(
        &mut self,
        pass: impl ModulePass + 'static,
    ) {
        self.passes.push(Pass::Module(Box::new(pass)));
    }

    pub fn add_program(
        &mut self,
        pass: impl ProgramPass + 'static,
    ) {
        self.passes.push(Pass::Program(Box::new(pass)));
    }

    /// Keeps `M` computed for every pass, as LLVM's `RequireAnalysisPass`:
    /// a pass that drops it has it computed again before the next.
    pub fn require<M: ModuleAnalysis>(&mut self) {
        if !self.required.iter().any(|one| one.id == TypeId::of::<M>()) {
            self.required.push(Kind::of::<M>());
        }
    }

    /// Computes program analysis `P` before each module's run, for the
    /// module's analyses to read through their program proxy.
    pub fn require_program<P: crate::program::ProgramAnalysis>(&mut self) {
        self.program_required.push(|program, analyses| {
            analyses.get::<P>(program);
        });
    }

    /// Runs every pass over every defined function of each module, in
    /// order.
    pub fn run(
        &mut self,
        program: &mut Program,
    ) -> Result<Vec<Stage>, String> {
        self.managed(program, &mut ProgramAnalyses::default())
    }

    /// `run` over `module` as a program of its own, for `target`.
    pub fn run_module(
        &mut self,
        module: &mut Module,
        target: Rc<dyn Machine>,
    ) -> Result<Vec<Stage>, String> {
        Program::lend(module, target, |program| self.run(program))?
    }

    /// The passes between program passes run module by module, each
    /// module's run reading the program results computed before it; a
    /// module the run changed drops them. A program pass runs over them
    /// all.
    fn managed(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<Vec<Stage>, String> {
        let mut stages = Vec::new();
        self.runs = 0;
        let count = program.modules.len();
        let dumps: Vec<_> = (0..count)
            .map(|at| {
                self.dump.as_ref().map(|one| if count > 1 { one.join(format!("module{at}")) } else { one.clone() })
            })
            .collect();
        // A module a frontend made wrong is its maker's, not the first pass's.
        for module in &program.modules {
            if self.verify_each {
                let problems = spanned("verify", || crate::verify::verify(module));
                if !problems.is_empty() {
                    return Err(format!("before the first pass: {}", problems.join("; ")));
                }
            }
        }
        let mut start = 0;
        while start < self.passes.len() {
            if let Pass::Program(pass) = &mut self.passes[start] {
                let name = pass.name();
                if self.bisect.is_none_or(|limit| {
                    self.runs += 1;
                    let running = self.runs <= limit;
                    eprintln!(
                        "BISECT: {}running pass ({}) {name} on the program",
                        if running { "" } else { "NOT " },
                        self.runs
                    );
                    running
                }) {
                    spanned(name, || pass.run(program, analyses))?;
                }
                for (at, module) in program.modules.iter_mut().enumerate() {
                    for (index, global) in module.globals.iter_mut().enumerate() {
                        let GlobalKind::Function(function) = &mut global.kind else { continue };
                        let changes = function.take_changes();
                        if !changes.is_empty() {
                            stages.push(Stage { pass: name, module: at, function: GlobalId(index as u32), changes });
                        }
                    }
                }
                for (at, module) in program.modules.iter().enumerate() {
                    after(&dumps[at], self.verify_each, start, name, module)?;
                }
                start += 1;
                continue;
            }
            let end = (start..self.passes.len())
                .find(|&one| matches!(self.passes[one], Pass::Program(_)))
                .unwrap_or(self.passes.len());
            for at in 0..program.modules.len() {
                for require in &self.program_required {
                    require(program, analyses);
                }
                let before = interface(&program.modules[at]);
                let mut modules = ModuleAnalyses {
                    required: self.required.clone(),
                    ..ModuleAnalyses::new(analyses.proxy(program, at))
                };
                let made = self.over(at, &mut program.modules[at], &mut modules, start..end, &dumps[at])?;
                if made.iter().any(|one| !one.changes.is_empty())
                    || spanned("interface", || interface(&program.modules[at])) != before
                {
                    analyses.invalidate();
                }
                stages.extend(made);
            }
            start = end;
        }
        Ok(stages)
    }

    fn over(
        &mut self,
        index: usize,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
        passes: std::ops::Range<usize>,
        dump: &Option<std::path::PathBuf>,
    ) -> Result<Vec<Stage>, String> {
        let layout = analyses.program().layout.clone();
        let mut stages = Vec::new();
        let runs = &mut self.runs;
        let bisect = self.bisect;
        let mut bisected = |name: &str, unit: &str| {
            let Some(limit) = bisect else { return true };
            *runs += 1;
            let running = *runs <= limit;
            eprintln!("BISECT: {}running pass ({runs}) {name} on {unit}", if running { "" } else { "NOT " });
            running
        };
        let mut frozen: Vec<Kind> = Vec::new();
        for (number, pass) in self.passes.iter_mut().enumerate().skip(passes.start).take(passes.len()) {
            let name = pass.name();
            PASS.with(|p| p.set(name));
            let pass = match pass {
                Pass::Function(pass) => pass,
                Pass::Program(_) => unreachable!("a program pass runs over every module"),
                Pass::Freeze(kinds) => {
                    frozen.extend(kinds.iter().copied());
                    continue;
                }
                Pass::Module(pass) => {
                    if !bisected(name, "the module") {
                        continue;
                    }
                    let changed = spanned(name, || pass.run(module, analyses));
                    if !changed.is_empty() {
                        let mut preserved = PreservedAnalyses::none();
                        if !pass.adds_memory_operations() {
                            preserved.kept = frozen.iter().map(|one| one.id).collect();
                        }
                        analyses.invalidate(&preserved);
                        if !pass.adds_memory_operations() {
                            checked_stale(module, analyses, &frozen, name);
                        }
                    }
                    for id in changed {
                        analyses.changed(id);
                        let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { continue };
                        stages.push(Stage {
                            pass: name,
                            module: index,
                            function: id,
                            changes: function.take_changes(),
                        });
                    }
                    after(dump, self.verify_each, number, name, module)?;
                    continue;
                }
            };
            // A function's analyses read the outer facts, so a change to them
            // drops every function's. Only a function pass reads
            // them through `Outer`: a module pass asks `analyses` for what it
            // needs, so the required analyses are made before
            // a function pass and not before every pass (a module pass that
            // changes the module drops them, so they were made
            // for nothing: 2% of the -O1 compile of QCport).
            let outer = spanned("outer analyses", || analyses.outer(module));
            // The module analyses every run of the pass preserved.
            let mut kept: HashSet<TypeId> = analyses.results.keys().copied().collect();
            let mut declared = Declared::of(module);
            for at in 0..module.globals.len() {
                let id = GlobalId(at as u32);
                let Module { context, globals, metadata, .. } = &mut *module;
                let global = &mut globals[at];
                let GlobalKind::Function(function) = &mut global.kind else { continue };
                if function.is_declaration() || !bisected(name, global.name.as_deref().unwrap_or_default()) {
                    continue;
                }
                let cache = analyses.manager(id, &outer);
                let before = function.mark();
                let preserved = in_function(global.name.as_deref().unwrap_or_default(), || {
                    spanned(name, || {
                        pass.run(
                            &mut Unit {
                                context,
                                layout: &layout,
                                function,
                                id: Some(id),
                                metadata,
                                declared: &mut declared,
                            },
                            cache,
                        )
                    })
                });
                let preserved = preserved.unless_unchanged(function, before);
                kept.retain(|one| preserved.keeps(*one));
                spanned("invalidate", || cache.invalidate(function, &preserved));
                cache.check_kept(name, context, &layout, function);
                if self.verify_invalidation {
                    let stale = cache.stale(context, &layout, function);
                    if !stale.is_empty() {
                        return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                    }
                }
                stages.push(Stage { pass: name, module: index, function: id, changes: function.take_changes() });
                spanned("declared", || declared.place(module))?;
            }
            if self.verify_invalidation {
                let stale = analyses.stale(module, &kept);
                if !stale.is_empty() {
                    return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                }
            }
            // What the pipeline froze is held past a pass that adds no memory
            // operation.
            if !pass.adds_memory_operations() {
                kept.extend(frozen.iter().map(|one| one.id).filter(|one| analyses.results.contains_key(one)));
            }
            let mut preserved = PreservedAnalyses::none();
            preserved.kept = kept;
            analyses.invalidate(&preserved);
            if !pass.adds_memory_operations() {
                checked_stale(module, analyses, &frozen, name);
            }
            after(dump, self.verify_each, number, name, module)?;
        }
        Ok(stages)
    }
}

thread_local! {
    static CHECK_STALE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `LLRM_CHECK_STALE` for this thread, for a test.
pub fn check_stale(on: bool) {
    CHECK_STALE.with(|check| check.set(on));
}

/// `LLRM_CHECK_STALE`: the module analyses held past `pass`, which said it adds
/// no memory operation, still cover what working them out afresh gives.
fn checked_stale(
    module: &Module,
    analyses: &mut ModuleAnalyses,
    frozen: &[Kind],
    pass: &str,
) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let on =
        CHECK_STALE.with(std::cell::Cell::get) || *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_STALE").is_some());
    if frozen.is_empty() || !on {
        return;
    }
    for kind in frozen {
        let Some((_, held)) = analyses.results.get(&kind.id) else { continue };
        let held = Rc::clone(held);
        let fresh = (kind.run)(module, &mut ModuleAnalyses::new(Rc::clone(analyses.program())));
        assert!(
            (kind.covers)(&*held, &*fresh),
            "{pass} added a memory operation: the frozen {} no longer covers the module",
            kind.name
        );
    }
}

/// The pass manager beneath a program: its passes over each module.
impl ProgramPass for PassManager {
    fn name(&self) -> &'static str {
        "module-passes"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        self.managed(program, analyses).map(|_| ())
    }
}

/// The dump and the verifier after pass `number`.
fn after(
    dump: &Option<std::path::PathBuf>,
    verify_each: bool,
    number: usize,
    name: &str,
    module: &Module,
) -> Result<(), String> {
    if dump.is_none() && !verify_each {
        return Ok(());
    }
    spanned("verify after pass", || after_pass(dump, verify_each, number, name, module))
}

fn after_pass(
    dump: &Option<std::path::PathBuf>,
    verify_each: bool,
    number: usize,
    name: &str,
    module: &Module,
) -> Result<(), String> {
    if let Some(directory) = dump {
        let file = directory.join(format!("{:02}-{name}.ll", number + 1));
        std::fs::create_dir_all(directory)
            .and_then(|()| std::fs::write(file, crate::print::module(module)))
            .map_err(|error| error.to_string())?;
    }
    if verify_each {
        let problems = crate::verify::verify(module);
        if !problems.is_empty() {
            return Err(format!("after {name}: {}", problems.join("; ")));
        }
    }
    Ok(())
}
