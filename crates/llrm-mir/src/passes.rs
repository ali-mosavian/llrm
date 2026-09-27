//! LLVM's new pass manager: function passes over a module, analyses cached
//! per function and dropped unless a pass says it preserved them, and each
//! pass's change log kept for the rewrite ledger. A function analysis reads
//! its module and program only through `Outer`, LLVM's outer analysis
//! manager proxy, which also holds the module analyses.
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
use crate::program::ProgramProxy;
use crate::target::{Machine, Neutral};

/// What a function pass works on: its function, the context its types and
/// constants live in, and the module's datalayout.
pub struct Unit<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub function: &'a mut Function,
    /// What each function in the module does to memory.
    pub callees: &'a crate::memory::Callees,
    /// The module's metadata nodes.
    pub metadata: &'a [crate::module::MetadataNode],
    pub sizes: &'a crate::valuetracking::Sizes,
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

/// A fact about the whole module, as LLVM's `GlobalsAA`: computed before
/// each pass once required (`PassManager::require`), and dropped unless
/// every run of a pass preserved it. Function analyses read it through
/// `Outer::cached`. It reads its program through `program`.
pub trait ModuleAnalysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(module: &Module, program: &ProgramProxy) -> Self::Result;
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
/// no other body; its program; and the module analyses computed.
#[derive(Clone)]
pub struct Outer {
    pub metadata: Vec<MetadataNode>,
    pub globals: Vec<GlobalValue>,
    program: Rc<ProgramProxy>,
    modules: HashMap<TypeId, Rc<dyn Any>>,
}

impl Outer {
    /// `module`'s, a program of its own for `target`, or a neutral one.
    pub fn of(module: &Module, target: Option<Rc<dyn Machine>>) -> Self {
        Self::within(module, ProgramProxy::of(module, target.unwrap_or_else(|| Rc::new(Neutral))))
    }

    /// `module`'s, of `program`.
    pub fn within(module: &Module, program: Rc<ProgramProxy>) -> Self {
        Self { metadata: module.metadata.clone(), globals: module.globals.iter().map(GlobalValue::declaration).collect(), program, modules: HashMap::new() }
    }

    /// Computes `M` of `module`, for analyses asked outside a pass manager.
    pub fn require<M: ModuleAnalysis>(&mut self, module: &Module) {
        self.modules.insert(TypeId::of::<M>(), Rc::new(M::run(module, &self.program)));
    }

    pub fn program(&self) -> &ProgramProxy {
        &self.program
    }

    pub fn target(&self) -> &dyn Machine {
        &*self.program.target
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

/// A required module analysis: how to compute it, and whether two results
/// agree.
pub(crate) struct Required {
    id: TypeId,
    name: &'static str,
    run: fn(&Module, &ProgramProxy) -> Rc<dyn Any>,
    agree: fn(&dyn Any, &dyn Any) -> bool,
}

fn computed<M: ModuleAnalysis>(module: &Module, program: &ProgramProxy) -> Rc<dyn Any> {
    Rc::new(M::run(module, program))
}

fn agree<M: ModuleAnalysis>(one: &dyn Any, other: &dyn Any) -> bool {
    one.downcast_ref::<M::Result>() == other.downcast_ref::<M::Result>()
}

pub trait FunctionPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses;
}

/// A pass over the whole module, as LLVM's inliner works across functions:
/// it answers which functions it changed.
pub trait ModulePass {
    fn name(&self) -> &'static str;
    fn run(&mut self, module: &mut Module) -> Vec<GlobalId>;
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
    /// What analyses may ask of the target; none, where it keeps nothing
    /// they ask about.
    pub target: Option<Rc<dyn Machine>>,
    pub(crate) required: Vec<Required>,
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
            self.required.push(Required { id: TypeId::of::<M>(), name: M::NAME, run: computed::<M>, agree: agree::<M> });
        }
    }

    /// Runs every pass over every defined function, in order.
    pub fn run(&mut self, module: &mut Module) -> Result<Vec<Stage>, String> {
        let layout = match &module.datalayout {
            Some(text) => DataLayout::parse(text)?,
            None => DataLayout::default(),
        };
        let mut caches: HashMap<GlobalId, Analyses> = HashMap::new();
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
        // Module analyses computed, and those dropped, kept to be reused
        // when computed again the same.
        let mut modules: HashMap<TypeId, Rc<dyn Any>> = HashMap::new();
        let mut dropped: HashMap<TypeId, Rc<dyn Any>> = HashMap::new();
        let program = ProgramProxy::of(module, self.target.clone().unwrap_or_else(|| Rc::new(Neutral)));
        let mut last: Option<Rc<Outer>> = None;
        for (number, pass) in self.passes.iter_mut().enumerate() {
            let name = pass.name();
            for one in &self.required {
                if modules.contains_key(&one.id) {
                    continue;
                }
                let fresh = (one.run)(module, &program);
                let result = dropped.remove(&one.id).filter(|old| (one.agree)(&**old, &*fresh)).unwrap_or(fresh);
                modules.insert(one.id, result);
            }
            // A function's analyses read the outer facts, so a change to
            // them drops every function's.
            let now = Outer { modules: modules.clone(), ..Outer::within(module, Rc::clone(&program)) };
            if !last.as_ref().is_some_and(|old| old.same(&now)) {
                last = Some(Rc::new(now));
            }
            let outer = Rc::clone(last.as_ref().expect("set above"));
            let callees = crate::memory::callees(module);
            let sizes = crate::valuetracking::sizes(module, &layout);
            let pass = match pass {
                Pass::Function(pass) => pass,
                Pass::Module(pass) => {
                    if !bisected(name, "the module") {
                        continue;
                    }
                    let changed = pass.run(module);
                    if !changed.is_empty() {
                        dropped.extend(modules.drain());
                    }
                    for id in changed {
                        caches.remove(&id);
                        let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { continue };
                        stages.push(Stage { pass: name, function: id, changes: function.take_changes() });
                    }
                    after(&self.dump, self.verify_each, number, name, module)?;
                    continue;
                }
            };
            // The module analyses every run of the pass preserved.
            let mut kept: HashSet<TypeId> = modules.keys().copied().collect();
            let mut declared = Declared::of(module);
            for at in 0..module.globals.len() {
                let id = GlobalId(at as u32);
                let Module { context, globals, metadata, .. } = &mut *module;
                let global = &mut globals[at];
                let GlobalKind::Function(function) = &mut global.kind else { continue };
                if function.is_declaration() || !bisected(name, global.name.as_deref().unwrap_or_default()) {
                    continue;
                }
                let analyses = caches.entry(id).or_insert_with(|| Analyses::new(Rc::clone(&outer)));
                if !Rc::ptr_eq(&analyses.outer, &outer) {
                    *analyses = Analyses::new(Rc::clone(&outer));
                }
                let preserved = pass.run(&mut Unit { context, layout: &layout, function, callees: &callees, metadata, sizes: &sizes, declared: &mut declared }, analyses);
                kept.retain(|one| preserved.keeps(*one));
                analyses.invalidate(&preserved);
                if self.verify_invalidation {
                    let stale = analyses.stale(context, &layout, function);
                    if !stale.is_empty() {
                        return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                    }
                }
                stages.push(Stage { pass: name, function: id, changes: function.take_changes() });
                declared.place(module)?;
            }
            if self.verify_invalidation {
                let mut stale: Vec<&str> = self
                    .required
                    .iter()
                    .filter(|one| kept.contains(&one.id) && !(one.agree)(&*modules[&one.id], &*(one.run)(module, &program)))
                    .map(|one| one.name)
                    .collect();
                stale.sort_unstable();
                if !stale.is_empty() {
                    return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                }
            }
            let gone: Vec<TypeId> = modules.keys().filter(|one| !kept.contains(*one)).copied().collect();
            for one in gone {
                dropped.insert(one, modules.remove(&one).expect("computed"));
            }
            after(&self.dump, self.verify_each, number, name, module)?;
        }
        Ok(stages)
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
