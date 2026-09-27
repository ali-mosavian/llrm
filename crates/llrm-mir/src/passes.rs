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
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::context::{Context, GlobalId};
use crate::datalayout::DataLayout;
use crate::dominators::DominatorTree;
use crate::module::{Change, Function, GlobalKind, GlobalValue, MetadataNode, Module};
use crate::program::{Program, ProgramAnalyses, ProgramPass, ProgramProxy, interface};
use crate::target::{Machine, Neutral};

/// What a function pass works on: its function, the context its types and
/// constants live in, and the program's datalayout.
pub struct Unit<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub function: &'a mut Function,
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
    ids: HashMap<String, GlobalId>,
    next: u32,
    pending: Vec<(String, crate::types::TypeId)>,
}

impl Declared {
    pub fn of(module: &Module) -> Self {
        let ids = module.globals.iter().enumerate().filter_map(|(at, one)| Some((one.name.clone()?, GlobalId(at as u32)))).collect();
        Self { ids, next: module.globals.len() as u32, pending: Vec::new() }
    }

    /// The function `name` of type `ty`, declared where the module has none.
    pub fn declare(&mut self, name: &str, ty: crate::types::TypeId) -> GlobalId {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = GlobalId(self.next);
        self.next += 1;
        self.ids.insert(name.to_owned(), id);
        self.pending.push((name.to_owned(), ty));
        id
    }

    /// The declarations made, added to `module`.
    pub fn place(&mut self, module: &mut Module) -> Result<(), String> {
        for (name, ty) in self.pending.drain(..) {
            let id = module.add_function(&name, ty, crate::module::Linkage::External)?;
            assert_eq!(Some(&id), self.ids.get(&name), "declared in order");
        }
        Ok(())
    }
}

/// A fact about a function, computed on demand and cached until a pass
/// fails to preserve it. It reads its module and target through
/// `analyses.outer()`, and asks `analyses` for the analyses it builds on.
pub trait Analysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result;
}

/// A fact about the whole module, as LLVM's `GlobalsAA`: computed on
/// demand, before each pass once required (`PassManager::require`), and
/// dropped unless every run of a pass preserved it. It asks `analyses` for
/// the module analyses it builds on and its program. Function analyses
/// read it through `Outer::cached`.
pub trait ModuleAnalysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> Self::Result;
}

/// What each function does to memory, as its attributes state it.
pub struct CalleeEffects;

impl ModuleAnalysis for CalleeEffects {
    type Result = crate::memory::Callees;
    const NAME: &'static str = "callee-effects";
    fn run(module: &Module, _: &mut ModuleAnalyses) -> Self::Result {
        crate::memory::callees(module)
    }
}

/// Each global variable's size in bytes.
pub struct GlobalSizes;

impl ModuleAnalysis for GlobalSizes {
    type Result = crate::valuetracking::Sizes;
    const NAME: &'static str = "global-sizes";
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> Self::Result {
        crate::valuetracking::sizes(module, &analyses.program().layout)
    }
}

/// Every global as its declaration, by id: what a function may read of
/// the others.
pub struct Declarations;

impl ModuleAnalysis for Declarations {
    type Result = Vec<GlobalValue>;
    const NAME: &'static str = "declarations";
    fn run(module: &Module, _: &mut ModuleAnalyses) -> Self::Result {
        module.declarations()
    }
}

/// LLVM's `DominatorTreeAnalysis`.
pub struct Dominators;

impl Analysis for Dominators {
    type Result = DominatorTree;
    const NAME: &'static str = "dominators";
    fn run(_: &Context, _: &DataLayout, function: &Function, _: &mut Analyses) -> DominatorTree {
        DominatorTree::new(function)
    }
}

/// LLVM's `LoopAnalysis`.
pub struct Loops;

impl Analysis for Loops {
    type Result = crate::loops::LoopInfo;
    const NAME: &'static str = "loops";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> crate::loops::LoopInfo {
        crate::loops::LoopInfo::new(function, &analyses.get::<Dominators>(context, layout, function))
    }
}

/// LLVM's `ScalarEvolutionAnalysis`, of add recurrences alone.
pub struct ScalarEvolution;

impl Analysis for ScalarEvolution {
    type Result = crate::scalarevolution::Evolution;
    const NAME: &'static str = "scalar-evolution";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> crate::scalarevolution::Evolution {
        crate::scalarevolution::Evolution::new(context, function, &analyses.get::<Loops>(context, layout, function))
    }
}

/// Which analyses a pass left true.
#[derive(Clone, Debug, Default)]
pub struct PreservedAnalyses {
    all: bool,
    kept: HashSet<TypeId>,
}

impl PreservedAnalyses {
    pub fn all() -> Self {
        Self { all: true, kept: HashSet::new() }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn preserve<A: Analysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<A>());
        self
    }

    pub fn preserve_module<M: ModuleAnalysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<M>());
        self
    }

    /// Whether the pass changed nothing: LLVM's `areAllPreserved`.
    pub fn are_all_preserved(&self) -> bool {
        self.all
    }

    fn keeps(&self, analysis: TypeId) -> bool {
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
    pub fn of(module: &Module, target: Option<Rc<dyn Machine>>) -> Self {
        (*ModuleAnalyses::of(module, target.unwrap_or_else(|| Rc::new(Neutral))).outer(module)).clone()
    }

    /// Computes `M` of `module`, for analyses asked outside a pass manager.
    pub fn require<M: ModuleAnalysis>(&mut self, module: &Module) {
        let result = ModuleAnalyses::new(Rc::clone(&self.program)).get::<M>(module);
        self.modules.insert(TypeId::of::<M>(), result);
    }

    pub fn program(&self) -> &ProgramProxy {
        &self.program
    }

    pub fn target(&self) -> &dyn Machine {
        &*self.program.target
    }

    /// What each function does to memory: `CalleeEffects`.
    pub fn callees(&self) -> &crate::memory::Callees {
        self.cached_ref::<CalleeEffects>().expect("every outer proxy holds the callees' effects")
    }

    /// Each global variable's size: `GlobalSizes`.
    pub fn sizes(&self) -> &crate::valuetracking::Sizes {
        self.cached_ref::<GlobalSizes>().expect("every outer proxy holds the globals' sizes")
    }

    /// `M`'s result, if computed: LLVM's `getCachedResult`.
    pub fn cached<M: ModuleAnalysis>(&self) -> Option<Rc<M::Result>> {
        self.modules.get(&TypeId::of::<M>()).map(|one| Rc::clone(one).downcast::<M::Result>().expect("keyed by its type"))
    }

    /// `cached`, borrowed for as long as the proxy.
    pub fn cached_ref<M: ModuleAnalysis>(&self) -> Option<&M::Result> {
        self.modules.get(&TypeId::of::<M>()).map(|one| one.downcast_ref::<M::Result>().expect("keyed by its type"))
    }

    /// Whether an analysis computed under `other` read what this holds.
    fn same(&self, other: &Self) -> bool {
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
    fn still_true(&self, context: &Context, layout: &DataLayout, function: &Function, outer: &Rc<Outer>) -> bool;
}

struct Entry<A: Analysis>(Rc<A::Result>);

impl<A: Analysis> Cached for Entry<A> {
    fn name(&self) -> &'static str {
        A::NAME
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn still_true(&self, context: &Context, layout: &DataLayout, function: &Function, outer: &Rc<Outer>) -> bool {
        *self.0 == A::run(context, layout, function, &mut Analyses::new(Rc::clone(outer)))
    }
}

/// One function's cached analyses, and what they may read of its module.
pub struct Analyses {
    cache: HashMap<TypeId, Box<dyn Cached>>,
    outer: Rc<Outer>,
}

impl Analyses {
    pub fn new(outer: Rc<Outer>) -> Self {
        Self { cache: HashMap::new(), outer }
    }

    /// An empty cache over the same module and target, for another body.
    pub fn fresh(&self) -> Self {
        Self::new(Rc::clone(&self.outer))
    }

    pub fn outer(&self) -> &Rc<Outer> {
        &self.outer
    }

    /// `A`'s result for `function`, computed once until invalidated.
    pub fn get<A: Analysis>(&mut self, context: &Context, layout: &DataLayout, function: &Function) -> Rc<A::Result> {
        let key = TypeId::of::<A>();
        if let Some(entry) = self.cache.get(&key) {
            let entry = entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
            return Rc::clone(&entry.0);
        }
        let result = Rc::new(A::run(context, layout, function, self));
        self.cache.insert(key, Box::new(Entry::<A>(Rc::clone(&result))));
        result
    }

    /// Drops what `preserved` does not keep: LLVM's
    /// `FunctionAnalysisManager::invalidate`, for a pass running others.
    pub fn invalidate(&mut self, preserved: &PreservedAnalyses) {
        self.cache.retain(|key, _| preserved.keeps(*key));
    }

    /// The cached analyses a fresh computation disagrees with.
    fn stale(&self, context: &Context, layout: &DataLayout, function: &Function) -> Vec<&'static str> {
        let mut out: Vec<&'static str> =
            self.cache.values().filter(|one| !one.still_true(context, layout, function, &self.outer)).map(|one| one.name()).collect();
        out.sort_unstable();
        out
    }
}

/// A module analysis: how to compute it, and whether two results agree.
#[derive(Clone, Copy)]
pub(crate) struct Kind {
    id: TypeId,
    name: &'static str,
    run: fn(&Module, &mut ModuleAnalyses) -> Rc<dyn Any>,
    agree: fn(&dyn Any, &dyn Any) -> bool,
}

impl Kind {
    fn of<M: ModuleAnalysis>() -> Self {
        Self { id: TypeId::of::<M>(), name: M::NAME, run: computed::<M>, agree: agree::<M> }
    }
}

fn computed<M: ModuleAnalysis>(module: &Module, analyses: &mut ModuleAnalyses) -> Rc<dyn Any> {
    Rc::new(M::run(module, analyses))
}

fn agree<M: ModuleAnalysis>(one: &dyn Any, other: &dyn Any) -> bool {
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
}

impl ModuleAnalyses {
    pub fn new(program: Rc<ProgramProxy>) -> Self {
        Self { program, required: Vec::new(), results: HashMap::new(), dropped: HashMap::new(), outer: None, functions: HashMap::new() }
    }

    /// `module`'s, a program of its own for `target`: for analyses asked
    /// outside a pass manager.
    pub fn of(module: &Module, target: Rc<dyn Machine>) -> Self {
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
    pub fn get<M: ModuleAnalysis>(&mut self, module: &Module) -> Rc<M::Result> {
        self.computed(Kind::of::<M>(), module).downcast::<M::Result>().expect("keyed by its type")
    }

    /// `M`'s result, if computed.
    pub fn cached<M: ModuleAnalysis>(&self) -> Option<Rc<M::Result>> {
        self.results.get(&TypeId::of::<M>()).map(|(_, one)| Rc::clone(one).downcast::<M::Result>().expect("keyed by its type"))
    }

    fn computed(&mut self, kind: Kind, module: &Module) -> Rc<dyn Any> {
        if let Some((_, one)) = self.results.get(&kind.id) {
            return Rc::clone(one);
        }
        let fresh = (kind.run)(module, self);
        let result = self.dropped.remove(&kind.id).filter(|old| (kind.agree)(&**old, &*fresh)).unwrap_or(fresh);
        self.results.insert(kind.id, (kind, Rc::clone(&result)));
        result
    }

    /// Drops what `preserved` does not keep.
    pub fn invalidate(&mut self, preserved: &PreservedAnalyses) {
        let gone: Vec<TypeId> = self.results.keys().filter(|one| !preserved.keeps(**one)).copied().collect();
        for one in gone {
            let (_, result) = self.results.remove(&one).expect("held");
            self.dropped.insert(one, result);
        }
    }

    /// `A` of function `id`, from its manager. `A` reads nothing through
    /// `Outer`: the module analysis asking may be one the outer proxy holds.
    pub fn function<A: Analysis>(&mut self, module: &Module, id: GlobalId) -> Rc<A::Result> {
        let function = module.global(id).function().expect("a function");
        if !self.functions.contains_key(&id) {
            let outer = match &self.outer {
                Some(outer) => Rc::clone(outer),
                None => Rc::new(Outer { metadata: Vec::new(), globals: Rc::default(), program: Rc::clone(&self.program), modules: HashMap::new() }),
            };
            self.functions.insert(id, Analyses::new(outer));
        }
        let layout = self.program.layout.clone();
        self.functions.get_mut(&id).expect("inserted above").get::<A>(&module.context, &layout, function)
    }

    /// Function `id`'s manager under `outer`, emptied where `outer` is not
    /// the one its results read.
    pub fn manager(&mut self, id: GlobalId, outer: &Rc<Outer>) -> &mut Analyses {
        let cache = self.functions.entry(id).or_insert_with(|| Analyses::new(Rc::clone(outer)));
        if !Rc::ptr_eq(&cache.outer, outer) {
            *cache = Analyses::new(Rc::clone(outer));
        }
        cache
    }

    /// Drops function `id`'s analyses, as a module pass does to a body it
    /// changed.
    pub fn changed(&mut self, id: GlobalId) {
        self.functions.remove(&id);
    }

    /// What a function analysis reads of `module`: the same proxy as last
    /// time where nothing it holds changed.
    pub fn outer(&mut self, module: &Module) -> Rc<Outer> {
        let every = [Kind::of::<CalleeEffects>(), Kind::of::<GlobalSizes>(), Kind::of::<Declarations>()].into_iter().chain(self.required.clone());
        let modules: HashMap<TypeId, Rc<dyn Any>> = every.map(|kind| (kind.id, self.computed(kind, module))).collect();
        let globals = Rc::clone(&modules[&TypeId::of::<Declarations>()]).downcast::<Vec<GlobalValue>>().expect("keyed by its type");
        let now = Outer { metadata: module.metadata.clone(), globals, program: Rc::clone(&self.program), modules };
        if !self.outer.as_ref().is_some_and(|old| old.same(&now)) {
            self.outer = Some(Rc::new(now));
        }
        Rc::clone(self.outer.as_ref().expect("set above"))
    }

    /// The held results among `kept` a fresh computation disagrees with.
    fn stale(&self, module: &Module, kept: &HashSet<TypeId>) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self
            .results
            .iter()
            .filter(|(id, (kind, result))| kept.contains(id) && !(kind.agree)(&**result, &*(kind.run)(module, &mut ModuleAnalyses::new(Rc::clone(&self.program)))))
            .map(|(_, (kind, _))| kind.name)
            .collect();
        out.sort_unstable();
        out
    }
}

pub trait FunctionPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses;
}

/// A pass over the whole module, as LLVM's inliner works across functions:
/// it answers which functions it changed. It asks `analyses` for module
/// analyses and its program, and drops those it invalidates as it goes.
pub trait ModulePass {
    fn name(&self) -> &'static str;
    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId>;
}

pub enum Pass {
    Function(Box<dyn FunctionPass>),
    Module(Box<dyn ModulePass>),
}

impl Pass {
    fn name(&self) -> &'static str {
        match self {
            Pass::Function(pass) => pass.name(),
            Pass::Module(pass) => pass.name(),
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
}

impl PassManager {
    pub fn add(&mut self, pass: impl FunctionPass + 'static) {
        self.passes.push(Pass::Function(Box::new(pass)));
    }

    pub fn add_module(&mut self, pass: impl ModulePass + 'static) {
        self.passes.push(Pass::Module(Box::new(pass)));
    }

    /// Keeps `M` computed for every pass, as LLVM's `RequireAnalysisPass`:
    /// a pass that drops it has it computed again before the next.
    pub fn require<M: ModuleAnalysis>(&mut self) {
        if !self.required.iter().any(|one| one.id == TypeId::of::<M>()) {
            self.required.push(Kind::of::<M>());
        }
    }

    /// Runs every pass over every defined function of each module, in
    /// order.
    pub fn run(&mut self, program: &mut Program) -> Result<Vec<Stage>, String> {
        self.managed(program, &mut ProgramAnalyses::default())
    }

    /// `run` over `module` as a program of its own, for `target`.
    pub fn run_module(&mut self, module: &mut Module, target: Rc<dyn Machine>) -> Result<Vec<Stage>, String> {
        Program::lend(module, target, |program| self.run(program))?
    }

    /// Each module's run reads the program results computed before it; a
    /// change to what the module exports drops them.
    fn managed(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<Vec<Stage>, String> {
        let mut stages = Vec::new();
        for at in 0..program.modules.len() {
            let before = interface(&program.modules[at]);
            let mut modules = ModuleAnalyses { required: self.required.clone(), ..ModuleAnalyses::new(analyses.proxy(program)) };
            stages.extend(self.over(at, &mut program.modules[at], &mut modules)?);
            if interface(&program.modules[at]) != before {
                analyses.invalidate();
            }
        }
        Ok(stages)
    }

    fn over(&mut self, index: usize, module: &mut Module, analyses: &mut ModuleAnalyses) -> Result<Vec<Stage>, String> {
        let layout = analyses.program().layout.clone();
        let mut stages = Vec::new();
        let mut runs = 0;
        let bisect = self.bisect;
        let mut bisected = |name: &str, unit: &str| {
            let Some(limit) = bisect else { return true };
            runs += 1;
            let running = runs <= limit;
            eprintln!("BISECT: {}running pass ({runs}) {name} on {unit}", if running { "" } else { "NOT " });
            running
        };
        for (number, pass) in self.passes.iter_mut().enumerate() {
            let name = pass.name();
            // A function's analyses read the outer facts, so a change to
            // them drops every function's.
            let outer = analyses.outer(module);
            let pass = match pass {
                Pass::Function(pass) => pass,
                Pass::Module(pass) => {
                    if !bisected(name, "the module") {
                        continue;
                    }
                    let changed = pass.run(module, analyses);
                    if !changed.is_empty() {
                        analyses.invalidate(&PreservedAnalyses::none());
                    }
                    for id in changed {
                        analyses.changed(id);
                        let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { continue };
                        stages.push(Stage { pass: name, module: index, function: id, changes: function.take_changes() });
                    }
                    after(&self.dump, self.verify_each, number, name, module)?;
                    continue;
                }
            };
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
                let preserved = pass.run(&mut Unit { context, layout: &layout, function, metadata, declared: &mut declared }, cache);
                kept.retain(|one| preserved.keeps(*one));
                cache.invalidate(&preserved);
                if self.verify_invalidation {
                    let stale = cache.stale(context, &layout, function);
                    if !stale.is_empty() {
                        return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                    }
                }
                stages.push(Stage { pass: name, module: index, function: id, changes: function.take_changes() });
                declared.place(module)?;
            }
            if self.verify_invalidation {
                let stale = analyses.stale(module, &kept);
                if !stale.is_empty() {
                    return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                }
            }
            let mut preserved = PreservedAnalyses::none();
            preserved.kept = kept;
            analyses.invalidate(&preserved);
            after(&self.dump, self.verify_each, number, name, module)?;
        }
        Ok(stages)
    }
}

/// The pass manager beneath a program: its passes over each module.
impl ProgramPass for PassManager {
    fn name(&self) -> &'static str {
        "module-passes"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        self.managed(program, analyses).map(|_| ())
    }
}

/// The dump and the verifier after pass `number`.
fn after(dump: &Option<std::path::PathBuf>, verify_each: bool, number: usize, name: &str, module: &Module) -> Result<(), String> {
        if let Some(directory) = dump {
            let file = directory.join(format!("{:02}-{name}.ll", number + 1));
            std::fs::create_dir_all(directory).and_then(|()| std::fs::write(file, crate::print::module(module))).map_err(|error| error.to_string())?;
        }
        if verify_each {
            let problems = crate::verify::verify(module);
            if !problems.is_empty() {
                return Err(format!("after {name}: {}", problems.join("; ")));
            }
        }
        Ok(())
}
