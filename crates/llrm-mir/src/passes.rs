//! LLVM's new pass manager: function passes over a module, analyses cached
//! per function and dropped unless a pass says it preserved them, and each
//! pass's change log kept for the rewrite ledger.
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
use crate::module::{Change, Function, GlobalKind, Module};

/// What a function pass works on: its function, the context its types and
/// constants live in, and the module's datalayout.
pub struct Unit<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub function: &'a mut Function,
}

/// A fact about a function, computed on demand and cached until a pass
/// fails to preserve it.
pub trait Analysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(context: &Context, layout: &DataLayout, function: &Function) -> Self::Result;
}

/// LLVM's `DominatorTreeAnalysis`.
pub struct Dominators;

impl Analysis for Dominators {
    type Result = DominatorTree;
    const NAME: &'static str = "dominators";
    fn run(_: &Context, _: &DataLayout, function: &Function) -> DominatorTree {
        DominatorTree::new(function)
    }
}

/// LLVM's `LoopAnalysis`.
pub struct Loops;

impl Analysis for Loops {
    type Result = crate::loops::LoopInfo;
    const NAME: &'static str = "loops";
    fn run(_: &Context, _: &DataLayout, function: &Function) -> crate::loops::LoopInfo {
        crate::loops::LoopInfo::new(function, &DominatorTree::new(function))
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

    fn keeps(&self, analysis: TypeId) -> bool {
        self.all || self.kept.contains(&analysis)
    }
}

/// A cached result, able to check itself against a fresh computation.
trait Cached {
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn still_true(&self, context: &Context, layout: &DataLayout, function: &Function) -> bool;
}

struct Entry<A: Analysis>(Rc<A::Result>);

impl<A: Analysis> Cached for Entry<A> {
    fn name(&self) -> &'static str {
        A::NAME
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn still_true(&self, context: &Context, layout: &DataLayout, function: &Function) -> bool {
        *self.0 == A::run(context, layout, function)
    }
}

/// One function's cached analyses.
#[derive(Default)]
pub struct Analyses {
    cache: HashMap<TypeId, Box<dyn Cached>>,
}

impl Analyses {
    /// `A`'s result for `function`, computed once until invalidated.
    pub fn get<A: Analysis>(&mut self, context: &Context, layout: &DataLayout, function: &Function) -> Rc<A::Result> {
        let key = TypeId::of::<A>();
        if let Some(entry) = self.cache.get(&key) {
            let entry = entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
            return Rc::clone(&entry.0);
        }
        let result = Rc::new(A::run(context, layout, function));
        self.cache.insert(key, Box::new(Entry::<A>(Rc::clone(&result))));
        result
    }

    fn invalidate(&mut self, preserved: &PreservedAnalyses) {
        self.cache.retain(|key, _| preserved.keeps(*key));
    }

    /// The cached analyses a fresh computation disagrees with.
    fn stale(&self, context: &Context, layout: &DataLayout, function: &Function) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self.cache.values().filter(|one| !one.still_true(context, layout, function)).map(|one| one.name()).collect();
        out.sort_unstable();
        out
    }
}

pub trait FunctionPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses;
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
    pub(crate) passes: Vec<Box<dyn FunctionPass>>,
    pub verify_each: bool,
    pub verify_invalidation: bool,
    /// A directory each pass's output is written to, as `NN-pass.ll`.
    pub dump: Option<std::path::PathBuf>,
}

impl PassManager {
    pub fn add(&mut self, pass: impl FunctionPass + 'static) {
        self.passes.push(Box::new(pass));
    }

    /// Runs every pass over every defined function, in order.
    pub fn run(&mut self, module: &mut Module) -> Result<Vec<Stage>, String> {
        let layout = match &module.datalayout {
            Some(text) => DataLayout::parse(text)?,
            None => DataLayout::default(),
        };
        let mut caches: HashMap<GlobalId, Analyses> = HashMap::new();
        let mut stages = Vec::new();
        for (number, pass) in self.passes.iter_mut().enumerate() {
            let name = pass.name();
            for at in 0..module.globals.len() {
                let id = GlobalId(at as u32);
                let Module { context, globals, .. } = &mut *module;
                let GlobalKind::Function(function) = &mut globals[at].kind else { continue };
                if function.is_declaration() {
                    continue;
                }
                let analyses = caches.entry(id).or_default();
                let preserved = pass.run(&mut Unit { context, layout: &layout, function }, analyses);
                analyses.invalidate(&preserved);
                if self.verify_invalidation {
                    let stale = analyses.stale(context, &layout, function);
                    if !stale.is_empty() {
                        return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                    }
                }
                stages.push(Stage { pass: name, function: id, changes: function.take_changes() });
            }
            if let Some(directory) = &self.dump {
                let file = directory.join(format!("{:02}-{name}.ll", number + 1));
                std::fs::create_dir_all(directory).and_then(|()| std::fs::write(file, crate::print::module(module))).map_err(|error| error.to_string())?;
            }
            if self.verify_each {
                let problems = crate::verify::verify(module);
                if !problems.is_empty() {
                    return Err(format!("after {name}: {}", problems.join("; ")));
                }
            }
        }
        Ok(stages)
    }
}
