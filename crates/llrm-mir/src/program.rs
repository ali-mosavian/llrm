//! A program: the modules compiled together, resolved by name and never
//! merged, and what they share -- one datalayout, the target, and where
//! the segments put data and the stack. Its analyses are cached in
//! `ProgramAnalyses`; a module's analyses and passes read them, and the
//! shared state, through `ProgramProxy`, as LLVM's
//! `OuterAnalysisManagerProxy` one level up.

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::context::GlobalId;
use crate::datalayout::DataLayout;
use crate::module::{Function, GlobalKind, GlobalValue, Linkage, Module};
use crate::opcode::Attribute;
use crate::types::Types;
use crate::target::Machine;

/// DGROUP as the program links it: its member segments, the addresses cut
/// in each, and which are COMMON. The BC route fills it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DataGroup {
    pub members: Vec<String>,
    pub landmarks: BTreeMap<String, BTreeSet<i64>>,
    pub common: BTreeSet<String>,
}

/// Where the program's segments put its data and its stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentLayout {
    /// The address space of the data group, which a near pointer reaches
    /// through DS.
    pub data_space: u32,
    /// The allocas' address space, which reaches the stack through SS.
    pub stack_space: u32,
    /// SS = DS: the stack lies in the data group.
    pub stack_in_data: bool,
    pub data_group: DataGroup,
}

impl SegmentLayout {
    /// The segments of a program laid out as `layout` states, SS = DS.
    pub fn of(layout: &DataLayout) -> Self {
        Self { data_space: 0, stack_space: layout.alloca_space, stack_in_data: true, data_group: DataGroup::default() }
    }

    /// Whether every pointer in `space` reaches program data: the data
    /// group's or the stack's.
    pub fn program_data(&self, space: u32) -> bool {
        space == self.data_space || space == self.stack_space
    }
}

/// Which of the program's globals code outside it may reach.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Exports {
    /// The names it may link to. None: every one a module does not keep to
    /// itself, as when the program is part of a larger link.
    pub linked: Option<BTreeSet<String>>,
    /// The functions it calls whatever their linkage: the entry points and
    /// what the runtime calls back.
    pub entries: BTreeSet<String>,
}

impl Exports {
    /// `linked` alone: the program is the whole link but its runtime.
    pub fn closed(linked: BTreeSet<String>) -> Self {
        Self { linked: Some(linked), entries: BTreeSet::new() }
    }

    /// Whether outside code may reach `global`.
    pub fn exported(&self, global: &GlobalValue) -> bool {
        let named = |names: &BTreeSet<String>| global.name.as_ref().is_some_and(|name| names.contains(name));
        match &self.linked {
            _ if named(&self.entries) => true,
            _ if matches!(global.linkage, Linkage::Internal | Linkage::Private) => false,
            None => true,
            Some(names) => named(names),
        }
    }
}

pub struct Program {
    pub modules: Vec<Module>,
    pub layout: DataLayout,
    pub target: Rc<dyn Machine>,
    pub segments: SegmentLayout,
    pub exports: Exports,
    /// Declarations alone, of the routines and cells of the runtime the
    /// program links against, with what the frontend knows of them:
    /// attributes, and `!llrm.named` and `!llrm.writes`.
    pub runtime: Rc<Module>,
}

impl Program {
    /// `modules` compiled together for `target`, under the one datalayout
    /// they state.
    pub fn new(modules: Vec<Module>, target: Rc<dyn Machine>) -> Result<Self, String> {
        let stated: BTreeSet<Option<&str>> = modules.iter().map(|one| one.datalayout.as_deref()).collect();
        if stated.len() > 1 {
            return Err(format!("modules state different datalayouts: {stated:?}"));
        }
        let layout = match stated.into_iter().next().flatten() {
            Some(text) => DataLayout::parse(text)?,
            None => DataLayout::default(),
        };
        let segments = SegmentLayout::of(&layout);
        Ok(Self { modules, layout, target, segments, exports: Exports::default(), runtime: Rc::default() })
    }

    /// The program linked against `runtime`: each module's declaration of
    /// what it declares and no module defines takes its attributes, as
    /// LLVM's InferFunctionAttrs states a library's.
    pub fn with_runtime(mut self, runtime: Module) -> Result<Self, String> {
        if let Some(one) = runtime.globals.iter().find(|one| defines(one)) {
            return Err(format!("the runtime defines @{}", one.name.as_deref().unwrap_or_default()));
        }
        for at in 0..self.modules.len() {
            for id in (0..self.modules[at].globals.len() as u32).map(GlobalId) {
                let Some(promise) = self.modules[at].global(id).name.as_deref().and_then(|name| runtime.named(name)) else { continue };
                if self.definition(at, id).is_some() {
                    continue;
                }
                let (GlobalKind::Function(promise), module) = (&runtime.global(promise).kind, &mut self.modules[at]) else { continue };
                let GlobalKind::Function(declaration) = &mut module.globals[id.0 as usize].kind else { continue };
                promised(declaration, &mut module.context.types, promise, &runtime.context.types);
            }
        }
        self.runtime = Rc::new(runtime);
        Ok(self)
    }

    /// The program with only `exports` named from outside.
    pub fn exporting(self, exports: Exports) -> Self {
        Self { exports, ..self }
    }

    /// `f` over `module` as a program of its own, handed back after.
    pub fn lend<R>(module: &mut Module, target: Rc<dyn Machine>, f: impl FnOnce(&mut Program) -> R) -> Result<R, String> {
        let mut program = Program::new(vec![std::mem::take(module)], target)?;
        let result = f(&mut program);
        *module = program.modules.pop().expect("one module");
        Ok(result)
    }

    /// The global `name` resolves to: the module defining it, else the
    /// first declaring it.
    pub fn resolve(&self, name: &str) -> Option<(usize, GlobalId)> {
        let named = || self.modules.iter().enumerate().filter_map(|(at, module)| Some((at, module.named(name)?)));
        named().find(|&(at, id)| defines(self.modules[at].global(id))).or_else(|| named().next())
    }

    /// `constant` of module `from` in module `to`'s context: none where it
    /// names what only `from` has.
    pub fn imported(&mut self, from: usize, constant: crate::context::ConstantId, to: usize) -> Option<crate::context::ConstantId> {
        if from == to {
            return Some(constant);
        }
        let (source, target) = self.pair(from, to);
        target.context.imported(&source.context, constant)
    }

    /// Body `id` of module `from`'s attributes as declaration `declared` of
    /// module `to`'s; whether that changed it.
    pub fn restate(&mut self, from: usize, id: GlobalId, to: usize, declared: GlobalId) -> bool {
        let (source, target) = self.pair(from, to);
        let (GlobalKind::Function(body), GlobalKind::Function(declaration)) = (&source.global(id).kind, &mut target.globals[declared.0 as usize].kind) else { return false };
        restated(declaration, &mut target.context.types, body, &source.context.types)
    }

    /// Module `from`, and module `to` to change; they differ.
    fn pair(&mut self, from: usize, to: usize) -> (&Module, &mut Module) {
        assert_ne!(from, to, "two modules");
        if from < to {
            let (low, high) = self.modules.split_at_mut(to);
            (&low[from], &mut high[0])
        } else {
            let (low, high) = self.modules.split_at_mut(from);
            (&high[0], &mut low[to])
        }
    }

    /// Module `at`'s globals that stand for one of `defined`: its own, and
    /// its declarations of another module's.
    pub fn local(&self, at: usize, defined: &BTreeSet<(usize, GlobalId)>) -> BTreeSet<GlobalId> {
        (0..self.modules[at].globals.len() as u32).map(GlobalId).filter(|&id| self.definition(at, id).is_some_and(|one| defined.contains(&one))).collect()
    }

    /// The definition global `id` of module `at` is: itself where it
    /// defines it, else the one another module defines under its name. A
    /// global a module keeps to itself is defined there or nowhere.
    pub fn definition(&self, at: usize, id: GlobalId) -> Option<(usize, GlobalId)> {
        let global = self.modules[at].global(id);
        if defines(global) {
            return Some((at, id));
        }
        if matches!(global.linkage, Linkage::Internal | Linkage::Private) {
            return None;
        }
        let name = global.name.as_deref()?;
        self.modules.iter().enumerate().filter(|&(other, _)| other != at).find_map(|(other, module)| {
            let found = module.named(name)?;
            let one = module.global(found);
            (defines(one) && !matches!(one.linkage, Linkage::Internal | Linkage::Private)).then_some((other, found))
        })
    }
}

/// `attrs` of `from`'s types in `types`; none where one names a type
/// `types` cannot hold.
fn attributes(types: &mut Types, from: &Types, attrs: &[Attribute]) -> Option<Vec<Attribute>> {
    attrs.iter().map(|one| one.imported(types, from)).collect()
}

/// `body`'s attributes, of `from`'s types, as `declaration`'s in `types`;
/// whether that changed it. One naming a type `types` cannot hold leaves
/// it as it was.
fn restated(declaration: &mut Function, types: &mut Types, body: &Function, from: &Types) -> bool {
    let (Some(attrs), Some(return_attrs), Some(parameter_attrs)) = (
        attributes(types, from, &body.attrs),
        attributes(types, from, &body.return_attrs),
        body.parameter_attrs.iter().map(|one| attributes(types, from, one)).collect::<Option<Vec<_>>>(),
    ) else {
        return false;
    };
    let changed = (&attrs, &parameter_attrs, &return_attrs) != (&declaration.attrs, &declaration.parameter_attrs, &declaration.return_attrs);
    (declaration.attrs, declaration.parameter_attrs, declaration.return_attrs) = (attrs, parameter_attrs, return_attrs);
    changed
}

/// `promise`'s attributes, of `from`'s types, added to `declaration`'s own
/// in `types`.
fn promised(declaration: &mut Function, types: &mut Types, promise: &Function, from: &Types) {
    let add = |types: &mut Types, own: &mut Vec<Attribute>, promised: &[Attribute]| {
        for one in promised.iter().filter_map(|one| one.imported(types, from)) {
            if !own.contains(&one) {
                own.push(one);
            }
        }
    };
    add(types, &mut declaration.attrs, &promise.attrs);
    add(types, &mut declaration.return_attrs, &promise.return_attrs);
    for (own, promised) in declaration.parameter_attrs.iter_mut().zip(&promise.parameter_attrs) {
        add(types, own, promised);
    }
}

/// Whether `global` is a definition: a function with a body, or a variable
/// with an initializer or storage of its own.
pub fn defines(global: &GlobalValue) -> bool {
    match &global.kind {
        crate::module::GlobalKind::Function(function) => !function.is_declaration(),
        crate::module::GlobalKind::Variable(variable) => variable.initializer.is_some() || !matches!(global.linkage, Linkage::External | Linkage::ExternWeak),
    }
}

/// What other modules may resolve against `module`: each global it does
/// not keep to itself, declared, and whether it defines it.
pub fn interface(module: &Module) -> Vec<(GlobalValue, bool)> {
    module.globals.iter().filter(|one| !matches!(one.linkage, Linkage::Internal | Linkage::Private)).map(|one| (one.declaration(), defines(one))).collect()
}

/// A fact about the whole program, cached in `ProgramAnalyses`.
pub trait ProgramAnalysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(program: &Program, analyses: &mut ProgramAnalyses) -> Self::Result;
}

/// The program analyses computed, kept until invalidated.
#[derive(Default)]
pub struct ProgramAnalyses {
    cache: HashMap<TypeId, Rc<dyn Any>>,
}

impl ProgramAnalyses {
    /// `P`'s result, computed once until invalidated.
    pub fn get<P: ProgramAnalysis>(&mut self, program: &Program) -> Rc<P::Result> {
        if let Some(one) = self.cached::<P>() {
            return one;
        }
        let result = Rc::new(P::run(program, self));
        self.cache.insert(TypeId::of::<P>(), Rc::clone(&result) as Rc<dyn Any>);
        result
    }

    pub fn cached<P: ProgramAnalysis>(&self) -> Option<Rc<P::Result>> {
        self.cache.get(&TypeId::of::<P>()).map(|one| Rc::clone(one).downcast::<P::Result>().expect("keyed by its type"))
    }

    /// Drops every result.
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    /// What a module reads of `program`: its shared state and the results
    /// computed.
    pub fn proxy(&self, program: &Program, module: usize) -> Rc<ProgramProxy> {
        Rc::new(ProgramProxy {
            layout: program.layout.clone(),
            target: Rc::clone(&program.target),
            segments: program.segments.clone(),
            exports: program.exports.clone(),
            runtime: Rc::clone(&program.runtime),
            module,
            results: self.cache.clone(),
        })
    }
}

/// A pass over the whole program. It drops the program results it
/// invalidates.
pub trait ProgramPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String>;
}

/// What a module's analyses and passes may read of its program, read-only:
/// the shared state and the program analyses cached when its run began.
#[derive(Clone)]
pub struct ProgramProxy {
    pub layout: DataLayout,
    pub target: Rc<dyn Machine>,
    pub segments: SegmentLayout,
    pub exports: Exports,
    pub runtime: Rc<Module>,
    /// The module reading it, by its index in the program.
    pub module: usize,
    results: HashMap<TypeId, Rc<dyn Any>>,
}

impl ProgramProxy {
    /// A program of `module` alone for `target`, read by what runs outside a
    /// pass manager.
    pub fn of(module: &Module, target: Rc<dyn Machine>) -> Rc<Self> {
        let layout = module.datalayout.as_deref().map_or_else(|| Ok(DataLayout::default()), DataLayout::parse).expect("a module's datalayout parses");
        Rc::new(Self { segments: SegmentLayout::of(&layout), layout, target, exports: Exports::default(), runtime: Rc::default(), module: 0, results: HashMap::new() })
    }

    /// `P`'s result, if computed: LLVM's `getCachedResult`.
    pub fn cached<P: ProgramAnalysis>(&self) -> Option<Rc<P::Result>> {
        self.results.get(&TypeId::of::<P>()).map(|one| Rc::clone(one).downcast::<P::Result>().expect("keyed by its type"))
    }
}
