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
use crate::domtree::DomTree;
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

thread_local! {
    static RUNNING_FUNCTION: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The name of the function a function pass is running over, for the debug
/// lines (`LLRM_DEBUG=runs`).
pub fn running_function() -> String {
    RUNNING_FUNCTION.with(|name| name.borrow().clone())
}

/// Tells the debug lines which function a body is about to run over: for a
/// run outside the manager's loop over a pass's functions (the interprocedural
/// stage's reruns), which would otherwise print the last function it set.
pub fn set_running_function(name: &str) {
    RUNNING_FUNCTION.with(|running| {
        let mut running = running.borrow_mut();
        running.clear();
        running.push_str(name);
    });
}

pub(crate) fn counted(
    what: &'static str,
    hit: bool,
) {
    crate::counts::analysis(what, hit);
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

/// What a library routine's declaration states: its convention, its
/// attributes and, on each pointer parameter, `parameter`.
fn routine_facts(
    function: &mut crate::module::Function,
    convention: u32,
    attrs: &[crate::opcode::Attribute],
    parameter: &[crate::opcode::Attribute],
) {
    function.calling_convention = convention;
    function.attrs.extend(attrs.iter().cloned());
    for at in 0..function.parameter_attrs.len() {
        function.parameter_attrs[at].extend(parameter.iter().cloned());
    }
}

/// A global a pass has declared and the module has yet to get.
#[derive(Clone, Debug)]
enum Pending {
    Function(String, crate::types::TypeId),
    /// A library routine: its convention and what the library states of it.
    Routine(String, crate::types::TypeId, u32, Vec<crate::opcode::Attribute>, Vec<crate::opcode::Attribute>),
    Table(String, crate::module::GlobalVariable),
}

/// The size of each function a global variable's initializer names.
fn referenced_sizes(module: &Module) -> HashMap<GlobalId, Size> {
    use crate::context::{ConstantExpr, ConstantId, ConstantKind};
    let mut named: Vec<GlobalId> = Vec::new();
    let mut stack: Vec<ConstantId> = module
        .globals
        .iter()
        .filter_map(|global| match &global.kind {
            GlobalKind::Variable(variable) => variable.initializer,
            GlobalKind::Function(_) => None,
        })
        .collect();
    while let Some(id) = stack.pop() {
        match &module.context.get(id).kind {
            ConstantKind::Aggregate(members) => stack.extend(members.iter().copied()),
            ConstantKind::Global(global) => named.push(*global),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => stack.extend(operands.iter().copied()),
            ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => stack.push(*value),
            _ => {}
        }
    }
    named
        .into_iter()
        .map(|id| {
            let size = match &module.globals[id.0 as usize].kind {
                GlobalKind::Function(function) if !function.is_declaration() => {
                    Size::Instructions(function.live_instructions())
                }
                _ => Size::Declaration,
            };
            (id, size)
        })
        .collect()
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
    pending: Vec<Pending>,
    /// Metadata nodes made, numbered after the module's.
    nodes: Vec<crate::module::MetadataNode>,
    first_node: u32,
    /// The live instructions of the functions a constant names, while a pass
    /// that `wants_sizes` runs (`size_of`).
    sizes: HashMap<GlobalId, Size>,
}

/// How big another function is (`Function::getInstructionCount`), which a
/// function pass cannot see: the manager counts the functions a constant names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    /// Named by no constant, or not counted: nothing is known.
    Unknown,
    Declaration,
    Instructions(usize),
}

impl Declared {
    /// Counts the functions a global initializer of `module` names, for a
    /// pass that `wants_sizes` (`size_of`).
    pub fn count_referenced(
        &mut self,
        module: &Module,
    ) {
        self.sizes = referenced_sizes(module);
    }

    /// How many instructions function `id` has, if a constant names it and
    /// the pass `wants_sizes`.
    pub fn size_of(
        &self,
        id: GlobalId,
    ) -> Size {
        self.sizes.get(&id).copied().unwrap_or(Size::Unknown)
    }

    pub fn of(module: &Module) -> Self {
        counted("declared names", false);
        let ids = module
            .globals
            .iter()
            .enumerate()
            .filter_map(|(at, one)| Some((one.name.owned()?, GlobalId(at as u32))))
            .collect();
        Self {
            ids: Some(ids),
            held: None,
            next: module.globals.len() as u32,
            pending: Vec::new(),
            nodes: Vec::new(),
            first_node: module.metadata.len() as u32,
            sizes: HashMap::default(),
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
            sizes: HashMap::default(),
        }
    }

    fn names(&mut self) -> &mut HashMap<String, GlobalId> {
        let held = &self.held;
        self.ids.get_or_insert_with(|| {
            counted("declared names", false);
            held.iter()
                .flat_map(|all| all.iter())
                .enumerate()
                .filter_map(|(at, one)| Some((one.name.owned()?, GlobalId(at as u32))))
                .collect()
        })
    }

    /// The declarations made and not yet placed, as the module will hold them
    /// from global `from` on: what a pass after the one that declared them
    /// sees, as LLVM's `getOrInsertFunction` has the callee in the module at
    /// once.
    pub fn since(
        &self,
        from: usize,
        context: &Context,
    ) -> Vec<GlobalValue> {
        let first = self.next as usize - self.pending.len();
        self.pending
            .iter()
            .skip(from.saturating_sub(first))
            .map(|one| match one {
                Pending::Function(name, ty) => GlobalValue {
                    name: name.clone().into(),
                    linkage: crate::module::Linkage::External,
                    unnamed_addr: Default::default(),
                    address_space: 0,
                    kind: GlobalKind::Function(Box::new(
                        crate::build::function_declaration(context, name, *ty).expect("a declaration that places"),
                    )),
                },
                Pending::Routine(name, ty, convention, attrs, parameter) => {
                    let mut function =
                        crate::build::function_declaration(context, name, *ty).expect("a declaration that places");
                    routine_facts(&mut function, *convention, attrs, parameter);
                    GlobalValue {
                        name: name.clone().into(),
                        linkage: crate::module::Linkage::External,
                        unnamed_addr: Default::default(),
                        address_space: 0,
                        kind: GlobalKind::Function(Box::new(function)),
                    }
                }
                Pending::Table(name, variable) => GlobalValue {
                    name: name.clone().into(),
                    linkage: crate::module::Linkage::Private,
                    unnamed_addr: Default::default(),
                    address_space: 0,
                    kind: GlobalKind::Variable(variable.clone()),
                },
            })
            .collect()
    }

    /// The name of a function declared and not yet placed in the module.
    pub fn pending_name(
        &self,
        id: GlobalId,
    ) -> Option<&str> {
        let at = (id.0 as usize).checked_sub(self.next as usize - self.pending.len())?;
        match self.pending.get(at)? {
            Pending::Function(name, _) | Pending::Routine(name, ..) => Some(name),
            Pending::Table(..) => None,
        }
    }

    /// The metadata nodes made so far, which the module has after the pass.
    pub fn nodes(&self) -> &[crate::module::MetadataNode] {
        &self.nodes
    }

    /// The id the next `node` gets: what a node that names itself holds.
    pub fn next_node(&self) -> crate::module::MetadataId {
        crate::module::MetadataId(self.first_node + self.nodes.len() as u32)
    }

    /// A metadata node, its id at once, added to the module after the pass.
    pub fn node(
        &mut self,
        node: crate::module::MetadataNode,
    ) -> crate::module::MetadataId {
        self.nodes.push(node);
        crate::module::MetadataId(self.first_node + self.nodes.len() as u32 - 1)
    }

    /// `Module::getFunction`: the global `name`, declared by the module or by
    /// this pass, without declaring it.
    pub fn find(
        &mut self,
        name: &str,
    ) -> Option<GlobalId> {
        self.names().get(name).copied()
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
        self.pending.push(Pending::Function(name.to_owned(), ty));
        id
    }

    /// A library routine `name` of type `ty`, declared where the module has
    /// none (`getOrInsertFunction`): in calling convention `convention`, with
    /// `attrs` on it and `parameter` on each pointer parameter, what the
    /// library states of it (`inferNonMandatoryLibFuncAttrs`).
    pub fn routine(
        &mut self,
        name: &str,
        ty: crate::types::TypeId,
        convention: u32,
        attrs: Vec<crate::opcode::Attribute>,
        parameter: Vec<crate::opcode::Attribute>,
    ) -> GlobalId {
        if let Some(&id) = self.names().get(name) {
            return id;
        }
        let id = GlobalId(self.next);
        self.next += 1;
        self.names().insert(name.to_owned(), id);
        self.pending.push(Pending::Routine(name.to_owned(), ty, convention, attrs, parameter));
        id
    }

    /// A private constant of type `ty`, `initializer` its value, named `stem`
    /// and the first number after it that no global has; a table already made
    /// with the same type and value is that one.
    pub fn table(
        &mut self,
        stem: &str,
        ty: crate::types::TypeId,
        initializer: crate::context::ConstantId,
    ) -> GlobalId {
        let same = |one: &Pending| {
            matches!(
                one,
                Pending::Table(_, held) if held.ty == ty && held.initializer == Some(initializer)
            )
        };
        if let Some(at) = self.pending.iter().position(same) {
            let Pending::Table(name, _) = &self.pending[at] else { unreachable!("a table") };
            let name = name.clone();
            return self.names()[&name];
        }
        let name =
            (0..).map(|n| format!("{stem}.{n}")).find(|name| !self.names().contains_key(name)).expect("a free name");
        let id = GlobalId(self.next);
        self.next += 1;
        self.names().insert(name.clone(), id);
        let variable = crate::module::GlobalVariable {
            externally_initialized: false,
            ty,
            constant: true,
            initializer: Some(initializer),
            align: None,
        };
        self.pending.push(Pending::Table(name, variable));
        id
    }

    /// Whether a table is among the declarations made: its initializer may
    /// hold the address of a global no code took before.
    pub fn tables_pending(&self) -> bool {
        self.pending.iter().any(|one| matches!(one, Pending::Table(..)))
    }

    /// The declarations made, added to `module`; how many.
    pub fn place(
        &mut self,
        module: &mut Module,
    ) -> Result<usize, String> {
        let made = self.pending.len();
        for one in std::mem::take(&mut self.pending) {
            let (name, id) = match one {
                Pending::Function(name, ty) => {
                    let id = module.add_function(&name, ty, crate::module::Linkage::External)?;
                    (name, id)
                }
                Pending::Routine(name, ty, convention, attrs, parameter) => {
                    let id = module.add_function(&name, ty, crate::module::Linkage::External)?;
                    if let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind {
                        routine_facts(function, convention, &attrs, &parameter);
                    }
                    (name, id)
                }
                Pending::Table(name, variable) => {
                    let id = module.add_variable(&name, variable, crate::module::Linkage::Private)?;
                    (name, id)
                }
            };
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

    /// Whether the result stays a true answer, with entries for fewer values
    /// and facts no more precise than a fresh derivation's, after edits that
    /// `sound_edits` finds only keep every live value's meaning: what it says
    /// of a live value holds, a value it has no entry for is unknown, and none
    /// is asked of an erased one. `covers` is the check.
    const SOUND: bool = false;

    /// Whether the result holds values (a pointer, a root), not facts keyed by
    /// them: uses replaced by an equal value (`Substituted`) then leave it
    /// naming one that is gone, and it is not kept past them.
    const NAMES_VALUES: bool = false;

    /// `old` brought to the function after `changes`, which `sound_edits` found
    /// keep every live value's meaning, equal to what `run` gives: each value
    /// `substituted` names replaced by what took its place (LLVM's value
    /// handles follow a RAUW the same way); none where it cannot be (it is
    /// then derived again). Asked of a result that `NAMES_VALUES`, which is
    /// kept only when it returns `Some`.
    #[allow(unused_variables)]
    fn remapped(
        old: &Self::Result,
        substituted: &dyn Fn(crate::module::ValueId) -> Option<crate::module::Operand>,
        changes: &[crate::module::Change],
        context: &Context,
        function: &Function,
    ) -> Option<Self::Result> {
        None
    }

    /// Whether `old`, derived before sound edits, says nothing `fresh`, derived
    /// now, contradicts. `LLRM_CHECK_REPLAY` asserts it for a result kept as
    /// sound.
    #[allow(unused_variables)]
    fn covers(
        old: &Self::Result,
        fresh: &Self::Result,
        context: &Context,
        function: &Function,
    ) -> bool {
        old == fresh
    }

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

    /// Whether a held `old`, the result for the function as it stands, still
    /// holds beyond `==` with a fresh one: for a result that compares equal to
    /// any other (a cache of what was asked) the check of its entries.
    /// `LLRM_CHECK_PRESERVED` and `verify_invalidation` ask it of every entry
    /// a pass kept.
    #[allow(unused_variables)]
    fn consistent(
        old: &Self::Result,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> bool {
        true
    }

    /// Whether `audit` runs on every result this analysis hands out, kept or
    /// derived (`LLRM_CHECK_CAPTURES`): a fact is judged where a pass reads it.
    const AUDITED: bool = false;

    /// An independent check of `result`, made each time it is handed out.
    #[allow(unused_variables)]
    fn audit(
        result: &Self::Result,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) {
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
    /// names this analysis, or changed nothing.
    fn preserved(preserved: &PreservedAnalyses) -> bool
    where
        Self: Sized,
    {
        preserved.kept::<Self>()
    }
}

/// A fact about the whole module, as LLVM's `GlobalsAA`: computed on
/// demand, before each pass once required (`PassManager::require`), and
/// dropped unless every run of a pass preserved it; or made at points alone
/// and kept until abandoned (`PassManager::require_cached`). It asks `analyses`
/// for the module analyses it builds on and its program. Function analyses
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
    type Result = DomTree;
    const READS_OUTER: bool = false;
    const NAME: &'static str = "dominators";
    const INCREMENTAL: bool = true;

    fn preserved(preserved: &PreservedAnalyses) -> bool {
        preserved.kept::<Self>() || preserved.cfg_preserved()
    }

    /// The blocks the changed edges reach are worked out again; nothing else
    /// can move.
    fn update(
        previous: &DomTree,
        changes: &[crate::module::Change],
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> Option<DomTree> {
        previous.updated(function, changes)
    }

    /// In place where nothing else holds the tree: LLVM's pass updates the
    /// tree it owns, and a copy of it per edit is a step in the size of the
    /// function.
    fn update_owned(
        previous: Rc<DomTree>,
        changes: &[crate::module::Change],
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> Option<DomTree> {
        match Rc::try_unwrap(previous) {
            Ok(mut tree) => tree.update_in_place(function, changes).then_some(tree),
            Err(shared) => shared.updated(function, changes),
        }
    }

    fn run(
        _: &Context,
        _: &DataLayout,
        function: &Function,
        _: &mut Analyses,
    ) -> DomTree {
        DomTree::of(function)
    }
}

/// LLVM's `LoopAnalysis`.
pub struct Loops;

impl Analysis for Loops {
    type Result = crate::loopinfo::LoopInfo;
    const READS_OUTER: bool = false;
    const NAME: &'static str = "loops";

    fn preserved(preserved: &PreservedAnalyses) -> bool {
        preserved.kept::<Self>() || preserved.cfg_preserved()
    }

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> crate::loopinfo::LoopInfo {
        crate::loopinfo::LoopInfo::analyze(function, || analyses.get::<Dominators>(context, layout, function))
    }
}

/// Which analyses a pass left true. MIRROR: LLVM's `PreservedAnalyses`
/// (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Analysis.h#L109):
/// a pass that changed something names what it kept, and the manager drops the
/// rest; `all` is a pass that changed nothing.
#[derive(Clone, Debug, Default)]
pub struct PreservedAnalyses {
    all: bool,
    /// LLVM's `preserveSet<CFGAnalyses>`: the pass left the CFG as it was.
    cfg: bool,
    kept: HashSet<TypeId>,
    /// Kept because the pass brought them up to date itself and put the
    /// result back (`Analyses::replace`): LLVM's pass that uses
    /// `MemorySSAUpdater` and then `preserve<MemorySSAAnalysis>()`.
    updated: HashSet<TypeId>,
}

impl PreservedAnalyses {
    pub fn all() -> Self {
        Self { all: true, cfg: false, kept: HashSet::default(), updated: HashSet::default() }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn preserve<A: Analysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<A>());
        self
    }

    /// `A` is kept because the pass updated it: the manager holds it only if
    /// the pass put its updated result back with [`Analyses::replace`] as the
    /// function stands after its last edit; otherwise it drops it (and
    /// `LLRM_CHECK_STALE` fails, naming the pass).
    pub fn preserve_updated<A: Analysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<A>());
        self.updated.insert(TypeId::of::<A>());
        self
    }

    pub fn preserve_module<M: ModuleAnalysis>(mut self) -> Self {
        self.kept.insert(TypeId::of::<M>());
        self
    }

    /// `preserveSet<CFGAnalyses>`: the pass changed no block or edge, so the
    /// analyses that read only the CFG (dominators, loops) stand.
    pub fn preserve_cfg(mut self) -> Self {
        self.cfg = true;
        self
    }

    /// `PreservedAnalyses::allAnalysesInSet<CFGAnalyses>`.
    pub fn cfg_preserved(&self) -> bool {
        self.all || self.cfg
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

    /// Whether the pass kept `analysis` by updating it (`preserve_updated`).
    fn updated_by_pass(
        &self,
        analysis: TypeId,
    ) -> bool {
        self.updated.contains(&analysis)
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
    /// What was worked out of these module analyses and nothing else.
    memo: Memo,
}

/// Results derived from the module analyses of one [`Outer`] alone, kept for
/// its life: a clone starts empty, as does one made by `with_declared`, and
/// `require` empties it, so none outlives the analyses it was made from.
#[derive(Default)]
pub struct Memo(std::cell::RefCell<HashMap<(TypeId, u64), Rc<dyn Any>>>);

impl Clone for Memo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Outer {
    /// The `V` kept under `key`, made by `make` the first time it is asked.
    pub fn memo<V: 'static>(
        &self,
        key: u64,
        make: impl FnOnce() -> V,
    ) -> Rc<V> {
        let at = (TypeId::of::<V>(), key);
        if let Some(found) = self.memo.0.borrow().get(&at) {
            return Rc::clone(found).downcast::<V>().expect("keyed by its type");
        }
        let made = Rc::new(make());
        self.memo.0.borrow_mut().insert(at, Rc::clone(&made) as Rc<dyn Any>);
        made
    }
}

impl Outer {
    /// `self` with the declarations `added` after its globals: the callee table
    /// and the declarations a pass reads include a function a pass of this
    /// run declared, as they will once the module has it.
    pub fn with_declared(
        &self,
        added: Vec<GlobalValue>,
    ) -> Outer {
        let first = self.globals.len();
        let mut globals = (*self.globals).clone();
        globals.extend(added.iter().cloned());
        let globals = Rc::new(globals);
        let mut modules = self.modules.clone();
        modules.insert(TypeId::of::<Declarations>(), Rc::clone(&globals) as Rc<dyn Any>);
        let key = TypeId::of::<CalleeEffects>();
        if let Some(old) = modules.get(&key).and_then(|one| Rc::clone(one).downcast::<crate::memory::Callees>().ok()) {
            let mut callees = (*old).clone();
            for (at, one) in added.iter().enumerate() {
                if let GlobalKind::Function(function) = &one.kind {
                    callees.insert(
                        GlobalId((first + at) as u32),
                        crate::memory::global_summary(one.name.as_deref(), function),
                    );
                }
            }
            modules.insert(key, Rc::new(callees));
        }
        Outer {
            metadata: self.metadata.clone(),
            globals,
            program: Rc::clone(&self.program),
            modules,
            memo: Memo::default(),
        }
    }

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
        self.memo.0.borrow_mut().clear();
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
        read::note(read::of(M::NAME));
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

/// Which parts of the module's facts a pass has read, noted as they are: by
/// what an analysis it asks for reads, and by what it asks of the proxy itself.
/// A body's pipeline is run again only where a part a pass read has changed.
pub mod read {
    use std::cell::Cell;

    /// What each function does to memory and how many registers its call
    /// keeps (`CalleeEffects`, `CallRegisters`).
    pub const CALLEES: u8 = 1;
    /// Each function's summary (`Summaries`).
    pub const SUMMARIES: u8 = 2;
    /// What `GlobalsAA` holds.
    pub const GLOBALS: u8 = 4;
    /// The other functions' and variables' declarations, and the globals'
    /// sizes.
    pub const DECLARATIONS: u8 = 16;
    /// What `GlobalsModRef` holds: what each function does to each tracked
    /// global.
    pub const GLOBALS_MODREF: u8 = 32;
    pub const ALL: u8 = 55;

    thread_local! {
        static READ: Cell<u8> = const { Cell::new(0) };
    }

    /// Notes the parts `bits` as read.
    pub fn note(bits: u8) {
        READ.with(|read| read.set(read.get() | bits));
    }

    /// What was noted, replaced by `bits`.
    pub fn swap(bits: u8) -> u8 {
        READ.with(|read| read.replace(bits))
    }

    /// The part a module analysis of this name is.
    pub fn of(name: &str) -> u8 {
        match name {
            "callee-effects" | "call-registers" => CALLEES,
            "summaries" => SUMMARIES,
            "globals-aa" => GLOBALS,
            "globals-modref" => GLOBALS_MODREF,
            "declarations" | "global-sizes" | "type-ancestry" => DECLARATIONS,
            _ => ALL,
        }
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
    /// Whether the result reads facts of the outer manager.
    fn reads_outer(&self) -> bool;
    /// `still_true`, as `covers` has it, for a result kept as sound.
    fn still_covered(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        outer: &Rc<Outer>,
    ) -> bool;
    /// The function's history when the result was derived.
    fn mark(&self) -> crate::module::Mark;
}

struct Entry<A: Analysis> {
    result: Rc<A::Result>,
    /// What the result may have read of the module.
    outer: Rc<Outer>,
    /// The function's history when the result was derived.
    mark: crate::module::Mark,
    /// Which parts of the module's facts it read (`read`).
    bits: u8,
    /// It was derived while a result it read was older than the body: a pass
    /// edited and then asked without invalidating. It is as true as what it
    /// read, not as the body stands.
    from_older: bool,
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
        let mut fresh = Analyses::new(Rc::clone(outer));
        *self.result == A::run(context, layout, function, &mut fresh)
            && A::consistent(&self.result, context, layout, function, &mut fresh)
    }

    fn preserved(
        &self,
        preserved: &PreservedAnalyses,
    ) -> bool {
        A::preserved(preserved)
    }

    fn incremental(&self) -> bool {
        A::INCREMENTAL || A::SKIPS || A::SOUND
    }

    fn reads_outer(&self) -> bool {
        A::READS_OUTER
    }

    fn still_covered(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        outer: &Rc<Outer>,
    ) -> bool {
        let fresh = A::run(context, layout, function, &mut Analyses::new(Rc::clone(outer)));
        // What `get` would hand out: brought to the function first.
        let Some(changes) = function.changes_since(self.mark) else { return true };
        if !A::NAMES_VALUES {
            return A::covers(&self.result, &fresh, context, function);
        }
        let taken = substitutions(changes);
        match A::remapped(&self.result, &|value| taken.resolve(value), changes, context, function) {
            Some(now) => A::covers(&now, &fresh, context, function),
            None => true,
        }
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
    /// Something asked for since the last derivation began was older than the
    /// body (`Entry::from_older`).
    read_older: bool,
    /// Results kept past edits that only keep what they say true, not derived
    /// afresh: dropped at the next named point (`refresh_soft`).
    soft: HashSet<TypeId>,
    /// Something was kept as sound: what was derived from it since stands as
    /// sound too, and is not what fresh results give (the checks leave it).
    tainted: bool,
    /// The function's attributes as of the last invalidation: hashing them is
    /// the size of the function's declaration, and a result derived since is
    /// stamped with them (an attribute stated after is told at the next
    /// invalidation, which drops it).
    attributes: Option<u64>,
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
    TRACE.with(std::cell::Cell::get) || *ON.get_or_init(|| llrm_env::set("LLRM_WHY"))
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
    if why_printed() {
        eprintln!("WHY {level} {name} {pass} {how}");
    }
}

fn why_printed() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_env::set("LLRM_WHY"))
}

fn check_modules() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_MODULES"))
}

fn check_captures() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_CAPTURES").is_some())
}

fn check_replay() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_REPLAY"))
}

/// Whether `changes` only keep every live value's meaning, as a result that
/// is `SOUND` needs: an instruction erased (an unused value, a dead store);
/// uses replaced by the same value (`replace_value`, which logs
/// `Substituted` first); a value made that nothing existing reads, or a pure
/// one moved. Not: a block made or erased, an operand set to another
/// meaning, an instruction rewritten in place, a store or a call made or
/// moved, a terminator touched.
pub fn sound_edits(
    function: &Function,
    changes: &[crate::module::Change],
) -> bool {
    use crate::module::Change;
    use crate::module::Operand;
    let mut replaced: crate::dense::IdSet<crate::module::ValueId> = crate::dense::IdSet::new();
    let mut said: crate::dense::IdSet<crate::module::InstId> = crate::dense::IdSet::new();
    let writes = |opcode: &crate::opcode::Opcode| {
        opcode.is_terminator()
            || matches!(
                opcode,
                crate::opcode::Opcode::Store { .. }
                    | crate::opcode::Opcode::Call(_)
                    | crate::opcode::Opcode::Invoke(_)
                    | crate::opcode::Opcode::LandingPad { .. }
                    | crate::opcode::Opcode::Resume
                    | crate::opcode::Opcode::Load { volatile: true, .. }
            )
    };
    for change in changes {
        match *change {
            Change::Substituted { from, .. } => {
                replaced.insert(from);
            }
            Change::Operand { inst, was, .. } => match was {
                Operand::Value(value) if replaced.contains(&value) => {
                    said.insert(inst);
                }
                _ => return false,
            },
            Change::Rewritten(inst) => {
                if !said.contains(&inst) {
                    return false;
                }
            }
            Change::Erased { inst, .. } => {
                if function.instruction(inst).opcode.is_terminator() {
                    return false;
                }
            }
            Change::Inserted { inst, .. } | Change::Cloned { to: inst, .. } => {
                if writes(&function.instruction(inst).opcode) {
                    return false;
                }
            }
            Change::Moved { inst, .. } => {
                let opcode = &function.instruction(inst).opcode;
                if writes(opcode)
                    || matches!(
                        opcode,
                        crate::opcode::Opcode::Load { .. } | crate::opcode::Opcode::Alloca { .. }
                    )
                {
                    return false;
                }
            }
            Change::Operands { .. } | Change::BlockCreated(_) | Change::BlockErased(_) | Change::Parameters => {
                return false;
            }
        }
    }
    true
}

/// What the substitutions in a log came to: a value replaced by one replaced
/// later is followed to the last.
struct Substitutions(crate::dense::IdMap<crate::module::ValueId, crate::module::Operand>);

fn substitutions(changes: &[crate::module::Change]) -> Substitutions {
    let mut taken = crate::dense::IdMap::new();
    for change in changes {
        if let crate::module::Change::Substituted { from, to } = *change {
            taken.insert(from, to);
        }
    }
    Substitutions(taken)
}

impl Substitutions {
    fn resolve(
        &self,
        value: crate::module::ValueId,
    ) -> Option<crate::module::Operand> {
        let mut now = *self.0.get(&value)?;
        while let crate::module::Operand::Value(next) = now {
            match self.0.get(&next) {
                Some(&later) => now = later,
                None => break,
            }
        }
        Some(now)
    }
}

impl Analyses {
    /// The functions `declared` holds that the outer facts lack, added to them:
    /// the passes after the one that declared a function see its callee, and
    /// what was derived from the outer facts is derived again.
    pub fn declared(
        &mut self,
        context: &Context,
        declared: &Declared,
    ) {
        let added = declared.since(self.outer.globals.len(), context);
        if added.is_empty() {
            return;
        }
        let outer = Rc::new(self.outer.with_declared(added));
        self.drop_reading_outer();
        self.outer = outer;
    }

    /// Drops what was kept only as sound: a named point (the start of a round,
    /// after the interprocedural step) where the facts are worked out afresh,
    /// as gcc recomputes its alias results at a few fixed points.
    pub fn refresh_soft(&mut self) {
        for key in std::mem::take(&mut self.soft) {
            self.cache.remove(&key);
            self.kept.remove(&key);
        }
    }

    pub fn new(outer: Rc<Outer>) -> Self {
        Self {
            cache: HashMap::default(),
            kept: HashMap::default(),
            evicted: HashMap::default(),
            vouched: None,
            read_older: false,
            soft: HashSet::default(),
            tainted: false,
            attributes: None,
            outer,
        }
    }

    /// An empty cache over the same module and target, for another body.
    pub fn fresh(&self) -> Self {
        Self::new(Rc::clone(&self.outer))
    }

    pub fn outer(&self) -> &Rc<Outer> {
        read::note(read::DECLARATIONS);
        &self.outer
    }

    /// The proxy, not noted as a read: for the manager's own use.
    pub fn outer_unread(&self) -> &Rc<Outer> {
        &self.outer
    }

    /// `A`'s result, if computed, and the point in the function's history it
    /// was true at: the cached one, or one kept past an invalidation for
    /// `update` to bring up to date. What a pass that edits and rarely asks
    /// holds (the updates queue in the change log), for LLVM's lazy
    /// `DomTreeUpdater`.
    pub fn held<A: Analysis>(&self) -> Option<(Rc<A::Result>, crate::module::Mark)> {
        let key = TypeId::of::<A>();
        let entry = self.cache.get(&key).or_else(|| self.kept.get(&key))?;
        let entry = entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
        Some((Rc::clone(&entry.result), entry.mark))
    }

    /// `A`'s result, if computed: LLVM's `getCachedResult`.
    pub fn cached<A: Analysis>(&self) -> Option<Rc<A::Result>> {
        if A::READS_OUTER
            && let Some(entry) = self.cache.get(&TypeId::of::<A>())
        {
            read::note(entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type").bits);
        }
        self.cache
            .get(&TypeId::of::<A>())
            .map(|entry| Rc::clone(&entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type").result))
    }

    /// `result` as `A`'s, for `function` as it stands: a caller that has worked
    /// out what the analysis would derive (a pass that knows what its own
    /// edit did to it) hands it on, and what is asked next of this cache is
    /// not derived again from it.
    pub fn seed<A: Analysis>(
        &mut self,
        function: &Function,
        result: Rc<A::Result>,
    ) {
        self.cache.insert(
            TypeId::of::<A>(),
            Box::new(Entry::<A> {
                result,
                outer: Rc::clone(&self.outer),
                mark: function.mark(),
                bits: if A::READS_OUTER { read::ALL } else { 0 },
                from_older: false,
            }),
        );
    }

    /// `get` for a pass whose decisions follow what is known: the result for
    /// `function` as it stands. One kept past edits as sound (`soft`) knows no
    /// more than when it was derived, and what a fresh derivation knows is what
    /// the body says, not when something first asked.
    pub fn get_exact<A: Analysis>(
        &mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Rc<A::Result> {
        let key = TypeId::of::<A>();
        if self.soft.remove(&key) {
            self.cache.remove(&key);
        }
        // Held to be brought up to date as sound only (`SOUND`): derived again.
        if A::SOUND && !A::INCREMENTAL {
            self.kept.remove(&key);
        }
        self.get::<A>(context, layout, function)
    }

    /// `A`'s result for `function`, computed once until invalidated.
    pub fn get<A: Analysis>(
        &mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Rc<A::Result> {
        let result = self.found::<A>(context, layout, function);
        if A::AUDITED && check_captures() {
            A::audit(&result, context, layout, function, self);
        }
        result
    }

    fn found<A: Analysis>(
        &mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Rc<A::Result> {
        let key = TypeId::of::<A>();
        if let Some(entry) = self.cache.get(&key) {
            let entry = entry.as_any().downcast_ref::<Entry<A>>().expect("keyed by its type");
            if A::READS_OUTER {
                read::note(entry.bits);
            }
            counted(A::NAME, true);
            let result = Rc::clone(&entry.result);
            // Held from before an edit the pass made since it last invalidated.
            if entry.from_older || (entry.mark != function.mark() && self.vouched != Some(function.mark())) {
                self.read_older = true;
            }
            return result;
        }
        counted(A::NAME, false);
        let outside_older = std::mem::replace(&mut self.read_older, false);
        let before_reads = read::swap(0);
        let had_kept = self.kept.contains_key(&key);
        let mut old_reads = 0;
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
                old_reads = old.bits;
                (Rc::clone(&old.result), Rc::clone(&old.outer), old.mark)
            };
            drop(held);
            let changes = function.changes_since(mark)?;
            // What it read of the outer facts is not what is there now: the
            // changes of the function are not all that moved.
            if A::READS_OUTER && !Rc::ptr_eq(&outer, &self.outer) {
                return None;
            }
            if A::SOUND && sound_edits(function, changes) {
                // A result that names values names the replaced ones: it is
                // brought to the values that took their place, or derived
                // again.
                let result = if A::NAMES_VALUES {
                    let taken = substitutions(changes);
                    Rc::new(A::remapped(&result, &|value| taken.resolve(value), changes, context, function)?)
                } else {
                    result
                };
                if check_replay() {
                    let fresh = A::run(context, layout, function, &mut Analyses::new(Rc::clone(&self.outer)));
                    assert!(
                        A::covers(&result, &fresh, context, function),
                        "{}: the result kept past {} sound edits contradicts what deriving it afresh gives",
                        A::NAME,
                        changes.len()
                    );
                }
                self.soft.insert(key);
                self.tainted = true;
                return Some(Rc::clone(&result));
            }
            // Asking of a long log again and again is more than the run it
            // saves.
            if A::SKIPS
                // The entries that tell what an operand was are not changes of
                // their own.
                && changes
                    .iter()
                    .filter(|one| !matches!(
                        one,
                        crate::module::Change::Operand { .. } | crate::module::Change::Operands { .. }
                    ))
                    .count()
                    <= 256
                && (!A::READS_OUTER || Rc::ptr_eq(&outer, &self.outer))
                && A::unaffected(changes, context, function)
            {
                // Derived from what was kept as sound, it is sound, not the
                // same as from fresh results.
                if check_replay() && !self.tainted {
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
            // Derived from what was kept as sound it is sound, not the same as
            // what fresh results give.
            if check_replay() && !self.tainted {
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
        if !replayed {
            self.soft.remove(&key);
        }
        let result = match updated {
            Some(made) => made,
            None => Rc::new(spanned_as("analysis", A::NAME, || A::run(context, layout, function, self))),
        };
        // What it read of the module: what the result it was brought up to date
        // from read, and what the bringing read.
        let mine = read::swap(before_reads);
        let bits = if A::READS_OUTER { mine | old_reads } else { 0 };
        if A::READS_OUTER {
            read::note(bits);
        }
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
            Box::new(Entry::<A> {
                result: Rc::clone(&result),
                outer: Rc::clone(&self.outer),
                mark: function.mark(),
                bits,
                from_older: self.read_older,
            }),
        );
        self.read_older |= outside_older;
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
        if *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_PRESERVED")) {
            let stale = self.stale(context, layout, function);
            assert!(stale.is_empty(), "{pass} left {} held but stale", stale.join(", "));
        }
    }

    /// Puts `result` back as `A`'s result for `function` as it stands now: a
    /// pass that updated a clone of what it asked for (LLVM's
    /// `MemorySSAUpdater` on the analysis itself) stores it here, then
    /// returns `PreservedAnalyses::preserve_updated::<A>()`. Call it after the
    /// last edit: the result is stamped with the function's history now.
    pub fn replace<A: Analysis>(
        &mut self,
        function: &Function,
        result: A::Result,
    ) {
        let key = TypeId::of::<A>();
        self.kept.remove(&key);
        self.evicted.remove(&key);
        self.soft.remove(&key);
        self.cache.insert(
            key,
            Box::new(Entry::<A> {
                result: Rc::new(result),
                outer: Rc::clone(&self.outer),
                mark: function.mark(),
                bits: 0,
                from_older: false,
            }),
        );
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
        self.drop_unpreserved(preserved, Some(function));
        self.vouched = Some(function.mark());
    }

    /// `invalidate`, unless `function` is as the last call left it: nothing
    /// was edited since, so what is held stands. A pass that asks again
    /// between rules, and drops for the edit of the rule before, pays for
    /// the edit only when there was one.
    pub fn invalidate_if_edited(
        &mut self,
        function: &Function,
        preserved: &PreservedAnalyses,
    ) {
        if self.vouched != Some(function.mark()) {
            self.invalidate(function, preserved);
        }
    }

    /// Every result held stands, as `function` is now: a pass that edited and
    /// put back, or that changed nothing.
    pub fn vouch(
        &mut self,
        function: &Function,
    ) {
        self.vouched = Some(function.mark());
    }

    /// Drops what `preserved` does not keep: a pass that changed the function
    /// names what it left true (LLVM's `PreservedAnalyses`), and what it forgot
    /// to name is computed again when asked.
    fn drop_unpreserved(
        &mut self,
        preserved: &PreservedAnalyses,
        current: Option<&Function>,
    ) {
        self.attributes = current.map(Function::attributes_stamp);
        let mut gone: Vec<TypeId> =
            self.cache.iter().filter(|(_, entry)| !entry.preserved(preserved)).map(|(key, _)| *key).collect();
        // Kept because the pass updated it: held only if the pass put the
        // result back as the function stands, else it is as old as the edits.
        if let Some(function) = current {
            for (key, entry) in &self.cache {
                if preserved.updated_by_pass(*key) && !preserved.are_all_preserved() && entry.mark() != function.mark()
                {
                    assert!(
                        !stale_checks_on(),
                        "{} was declared kept updated by {} and not put back with `replace` after its last edit",
                        entry.name(),
                        PASS.with(std::cell::Cell::get)
                    );
                    gone.push(*key);
                }
            }
        }
        self.retire(gone);
    }

    /// Drops what reads facts of the outer manager, where that is not the one
    /// the results were derived under: the results that read the function
    /// alone stand.
    fn drop_reading_outer(&mut self) {
        self.attributes = None;
        let gone: Vec<TypeId> =
            self.cache.iter().filter(|(_, entry)| entry.reads_outer()).map(|(key, _)| *key).collect();
        self.retire(gone);
    }

    fn retire(
        &mut self,
        gone: Vec<TypeId>,
    ) {
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

    /// The held results a fresh computation disagrees with, for a test of what
    /// a pass leaves in the manager.
    pub fn held_stale(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Vec<&'static str> {
        self.stale(context, layout, function)
    }

    /// The cached analyses a fresh computation disagrees with.
    fn stale(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
    ) -> Vec<&'static str> {
        // What was kept as sound is checked as such; what is derived from it
        // while it stands is not the same as from fresh results, so is not.
        let mut out: Vec<&'static str> = self
            .cache
            .iter()
            .filter(|(key, one)| {
                if self.soft.contains(key) {
                    !one.still_covered(context, layout, function, &self.outer)
                } else {
                    !self.tainted && !one.still_true(context, layout, function, &self.outer)
                }
            })
            .map(|(_, one)| one.name())
            .collect();
        out.sort_unstable();
        out
    }
}

/// `LLRM_CHECK_UNREPORTED`: an analysis of a function read after an edit that
/// was not reported fails, where otherwise it is brought up to date.
fn unreported_fails() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_UNREPORTED"))
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
    /// What an outer proxy holds once made, kept until abandoned
    /// (`PassManager::require_cached`).
    stateless: Vec<Kind>,
    results: HashMap<TypeId, (Kind, Rc<dyn Any>)>,
    /// Results dropped, reused when computed again the same, so that an
    /// outer proxy holding them stays the same.
    dropped: HashMap<TypeId, Rc<dyn Any>>,
    /// The results `take_dropped` handed over, for `why` to tell a result
    /// brought up to date from one made from nothing.
    taken: HashSet<TypeId>,
    outer: Option<Rc<Outer>>,
    functions: HashMap<GlobalId, Analyses>,
    /// The bodies reported edited since the declarations were last checked
    /// against the module, and whether anything else may have changed them
    /// (`invalidate`): a pass that edits bodies says which, and the
    /// declarations of the rest then stand without a look at each.
    edited: BTreeSet<GlobalId>,
    /// Every body reported edited, in order and never cleared: an analysis
    /// that keeps a position in it is told which bodies moved since its last
    /// run without a look at each.
    edit_log: Vec<GlobalId>,
    /// Which log this is: positions in `edit_log` mean something only to the
    /// analyses made over it, not to ones carried from before.
    log_epoch: u64,
    declarations_open: bool,
    /// How often `invalidate` has said anything may have changed: a result that
    /// trusts the log of edits for what moved must be of the same count.
    resets: u64,
    metadata_open: bool,
    /// What an analysis keeps for its next run, by its type: the working of an
    /// update that reuses the last.
    memos: HashMap<TypeId, Box<dyn Any>>,
    /// `LLRM_CHECK_MODULES`: an analysis being run again to check what it
    /// brought up to date works everything out afresh.
    scratch: bool,
}

/// What a module's analyses leave for the next that is made over it: the
/// results held, to be reused when worked out again the same, and what the
/// analyses that bring their last run up to date kept (`memo`).
pub struct Carried {
    dropped: HashMap<TypeId, Rc<dyn Any>>,
    memos: HashMap<TypeId, Box<dyn Any>>,
    /// The stateless results, held as they were.
    held: Vec<(Kind, Rc<dyn Any>)>,
    stateless: Vec<Kind>,
}

impl Carried {
    /// Drops `M`'s result, held or not.
    pub(crate) fn abandon<M: ModuleAnalysis>(&mut self) {
        self.held.retain(|(kind, _)| kind.id != TypeId::of::<M>());
        self.dropped.remove(&TypeId::of::<M>());
    }
}

static NEXT_LOG_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl ModuleAnalyses {
    pub fn new(program: Rc<ProgramProxy>) -> Self {
        Self {
            program,
            required: Vec::new(),
            stateless: Vec::new(),
            results: HashMap::default(),
            dropped: HashMap::default(),
            taken: HashSet::default(),
            outer: None,
            functions: HashMap::default(),
            edited: BTreeSet::new(),
            edit_log: Vec::new(),
            log_epoch: NEXT_LOG_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            declarations_open: true,
            resets: 0,
            metadata_open: true,
            memos: HashMap::default(),
            scratch: false,
        }
    }

    /// What `self` leaves for the analyses made next over this module.
    pub fn carried(self) -> Carried {
        let mut dropped = self.dropped;
        let mut held = Vec::new();
        for (id, (kind, result)) in self.results {
            if self.stateless.iter().any(|one| one.id == id) {
                held.push((kind, result));
            } else {
                dropped.insert(id, result);
            }
        }
        Carried { dropped, memos: self.memos, held, stateless: self.stateless }
    }

    /// `self`, with what the analyses before it left.
    pub fn carrying(
        mut self,
        before: Carried,
    ) -> Self {
        self.dropped = before.dropped;
        self.memos = before.memos;
        for kind in before.stateless {
            if !self.stateless.iter().any(|one| one.id == kind.id) {
                self.stateless.push(kind);
            }
        }
        self.results.extend(before.held.into_iter().map(|(kind, result)| (kind.id, (kind, result))));
        self
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

    /// Drops the stateless results, as a pass that adds a table must: its
    /// initializer may take a tracked global's address. DEVIATE: LLVM keeps
    /// GlobalsAA past SimplifyCFG's lookup tables.
    pub fn abandon_stateless(&mut self) {
        for kind in &self.stateless {
            if let Some((_, result)) = self.results.remove(&kind.id) {
                self.dropped.insert(kind.id, result);
            }
        }
    }

    /// The result of `M` a pass dropped, handed over to the run that brings it
    /// up to date: not kept, so that it is the only holder where nothing else
    /// holds it. A run that does not take it leaves it to be compared.
    pub fn take_dropped<M: ModuleAnalysis>(&mut self) -> Option<Rc<M::Result>> {
        let one = self.dropped.remove(&TypeId::of::<M>())?;
        self.taken.insert(TypeId::of::<M>());
        Some(one.downcast::<M::Result>().expect("keyed by its type"))
    }

    /// Whether `M` is made only at its points (`PassManager::require_cached`).
    pub fn is_stateless<M: ModuleAnalysis>(&self) -> bool {
        self.stateless.iter().any(|one| one.id == TypeId::of::<M>())
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
                if check_modules() {
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
        if check_modules() {
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
                None if self.taken.contains(&kind.id) => "diff",
                None => "first",
            };
            record("mod", kind.name, class);
        }
        self.taken.remove(&kind.id);
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
        self.resets += 1;
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
        // A stateless result stands whatever the pass kept, as LLVM's
        // `GlobalsAAResult::invalidate` (GlobalsModRef.cpp:825, d1106deb71cc).
        let gone: Vec<TypeId> = self
            .results
            .keys()
            .filter(|one| !preserved.keeps(**one) && !self.stateless.iter().any(|kind| kind.id == **one))
            .copied()
            .collect();
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
                    memo: Memo::default(),
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
            cache.drop_reading_outer();
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
        self.edit_log.push(id);
    }

    /// The bodies reported edited so far, oldest first.
    pub fn edit_log(&self) -> &[GlobalId] {
        &self.edit_log
    }

    /// How often a pass has said anything may have changed (`invalidate`).
    pub fn resets(&self) -> u64 {
        self.resets
    }

    /// Which `edit_log` this is, for an analysis that keeps a position in it: a
    /// position kept by analyses carried over from before is of another log.
    pub fn log_epoch(&self) -> u64 {
        self.log_epoch
    }

    /// Body `id` was edited by a function pass over it alone, which left
    /// `preserved`.
    pub fn body_edited(
        &mut self,
        id: GlobalId,
        preserved: &PreservedAnalyses,
    ) {
        self.edited.insert(id);
        self.edit_log.push(id);
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
        let required: Vec<Kind> =
            self.required.iter().filter(|one| !self.stateless.iter().any(|kind| kind.id == one.id)).copied().collect();
        let every = [
            Kind::of::<CalleeEffects>(),
            Kind::of::<CallRegisters>(),
            Kind::of::<GlobalSizes>(),
            Kind::of::<Declarations>(),
            Kind::of::<TypeAncestry>(),
        ]
        .into_iter()
        .chain(required);
        let mut modules: HashMap<TypeId, Rc<dyn Any>> =
            every.map(|kind| (kind.id, self.computed(kind, module))).collect();
        // A stateless one as LLVM's `getCachedResult` through the proxy: only
        // what was made.
        for kind in &self.stateless {
            if let Some((_, result)) = self.results.get(&kind.id) {
                modules.insert(kind.id, Rc::clone(result));
            }
        }
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
        let now = Outer {
            metadata: module.metadata.clone(),
            globals,
            program: Rc::clone(&self.program),
            modules,
            memo: Memo::default(),
        };
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

thread_local! {
    /// The first body a pass left with a short phi since the manager last
    /// asked (#3016), as "after <pass>: ...".
    static PHI_PROBLEM: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Runs `pass`, then (in a build with debug assertions: dev, the unit tests)
/// checks that every phi of the body it left has one input per edge, and notes
/// the first pass that left one short for [`take_phi_problem`]. Passes that run
/// others (a slot of several, the fixed-point rounds) run them through here, so
/// the member is named and not the slot. A release build pays nothing: LLVM
/// verifies only under `-verify-each`.
pub fn run_checked(
    pass: &mut dyn FunctionPass,
    unit: &mut Unit,
    analyses: &mut Analyses,
) -> PreservedAnalyses {
    let preserved = pass.run(unit, analyses);
    check_phi_arity(pass.name(), unit.function);
    preserved
}

/// [`run_checked`]'s check, for a pass that edits in steps and names a step
/// that is not a pass of its own.
pub fn check_phi_arity(
    step: &str,
    function: &Function,
) {
    #[cfg(debug_assertions)]
    PHI_PROBLEM.with(|problem| {
        let mut problem = problem.borrow_mut();
        if problem.is_none() {
            let found = crate::verify::phi_arity(function);
            if !found.is_empty() {
                *problem = Some(format!("after {step}: {}", found.join("; ")));
            }
        }
    });
    #[cfg(not(debug_assertions))]
    let _ = (step, function);
}

/// The problem [`run_checked`] noted, if any, and forgets it.
pub fn take_phi_problem() -> Option<String> {
    PHI_PROBLEM.with(|problem| problem.borrow_mut().take())
}

pub trait FunctionPass {
    fn name(&self) -> &'static str;
    /// The names of the passes that run under `name` (a slot that stands for
    /// one pass of LLVM and runs several): the pass itself if it is one.
    fn parts(&self) -> Vec<&'static str> {
        vec![self.name()]
    }
    /// The LLVM pass this one translates; `None` for an llrm stand-in. The
    /// parity registry refuses an Old slot that runs one
    /// (`parity::check_phase`).
    fn translation(&self) -> Option<&'static str> {
        None
    }
    /// [`translation`](Self::translation) of each pass that runs under `name`.
    fn translations(&self) -> Vec<&'static str> {
        self.translation().into_iter().collect()
    }
    /// Whether the pass may add a memory operation a function did not have (a
    /// load speculated, a call made). The module analyses frozen at a point
    /// of the pipeline (`PassManager::freeze`) are held past a pass that says
    /// it does not: what they state of a function stays an upper bound of
    /// what it does. `LLRM_CHECK_STALE` recomputes them after such a pass
    /// and says if it did.
    fn adds_memory_operations(&self) -> bool {
        true
    }

    /// The pass reads how many instructions another function has
    /// (`Function::getInstructionCount`): the manager counts the functions a
    /// global's initializer names, once before the pass runs over the module
    /// and again for each of them the pass changes (`Declared::size_of`).
    fn wants_sizes(&self) -> bool {
        false
    }

    /// The pass dumps the bodies it is run over itself (the pipeline does, per
    /// function), so the manager dumps none.
    fn dumps_inputs_itself(&self) -> bool {
        false
    }
    /// The parts of a function the pass looks at (`footprint`). A pass that
    /// found nothing to do is not run again until an edit reaches one of
    /// them: gcc's `TODO_*` flags and LLVM's preserved analyses say the
    /// same of what a pass needs again. A pass that states less than
    /// everything says why.
    fn reads(&self) -> crate::footprint::Footprint {
        crate::footprint::Footprint::ALL
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
    /// Not a pass: computes the stateless analysis now, again where `again`.
    Compute {
        kind: Kind,
        again: bool,
    },
}

impl Pass {
    fn name(&self) -> &'static str {
        match self {
            Pass::Function(pass) => pass.name(),
            Pass::Module(pass) => pass.name(),
            Pass::Program(pass) => pass.name(),
            Pass::Freeze(_) => "freeze",
            Pass::Compute { again: false, .. } => "require",
            Pass::Compute { again: true, .. } => "recompute",
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
    /// `LLRM_DUMP_INPUTS`: the module each pass is about to run over is dumped
    /// too, as `NNNN-in-pass.ll` with a line in `manifest.txt`.
    pub dump_inputs: bool,
    /// How many pass runs happen, a module pass's one and a function
    /// pass's one per function, as LLVM's `-opt-bisect-limit`: each run is
    /// named on stderr, and those past the limit are skipped.
    pub bisect: Option<usize>,
    pub(crate) required: Vec<Kind>,
    pub(crate) stateless: Vec<Kind>,
    /// Program analyses computed before each module's run.
    pub(crate) program_required: Vec<fn(&Program, &mut ProgramAnalyses)>,
    /// Pass runs so far, for `bisect`.
    pub(crate) runs: usize,
}

impl PassManager {
    /// `pass`, already boxed.
    pub fn add_boxed(
        &mut self,
        pass: Box<dyn FunctionPass>,
    ) {
        self.passes.push(Pass::Function(pass));
    }

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

    /// `M` is made only at `compute` and `recompute`, read through the outer
    /// proxy once made (LLVM's `getCachedResult`), and kept past every pass
    /// until one abandons it (`ProgramAnalyses::abandon`), as LLVM's
    /// `GlobalsAA`. Held past a function pass it is rechecked under
    /// `LLRM_CHECK_STALE` like a frozen one.
    pub fn require_cached<M: ModuleAnalysis>(&mut self) {
        if !self.stateless.iter().any(|one| one.id == TypeId::of::<M>()) {
            self.stateless.push(Kind::of::<M>());
        }
    }

    /// LLVM's `RequireAnalysisPass<M>`: `M` made here unless held, and the
    /// module analyses that read it dropped
    /// (`InvalidateAnalysisPass<AAManager>`).
    pub fn compute<M: ModuleAnalysis>(&mut self) {
        self.passes.push(Pass::Compute { kind: Kind::of::<M>(), again: false });
    }

    /// LLVM's `RecomputeGlobalsAAPass`: `compute`, made afresh even if held.
    pub fn recompute<M: ModuleAnalysis>(&mut self) {
        self.passes.push(Pass::Compute { kind: Kind::of::<M>(), again: true });
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
                    if self.dump_inputs {
                        for (at, module) in program.modules.iter().enumerate() {
                            before(&dumps[at], start, name, module)?;
                        }
                    }
                    let _running = crate::counts::pass_run(name, 0);
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
                    stateless: self.stateless.clone(),
                    ..ModuleAnalyses::new(analyses.proxy(program, at))
                };
                if let Some(before) = analyses.uncarry(at) {
                    modules = modules.carrying(before);
                }
                let made = self.over(at, &mut program.modules[at], &mut modules, start..end, &dumps[at])?;
                analyses.carry(at, modules.carried());
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
        let inputs = self.dump_inputs;
        let mut frozen: Vec<Kind> = Vec::new();
        for (number, pass) in self.passes.iter_mut().enumerate().skip(passes.start).take(passes.len()) {
            let name = pass.name();
            PASS.with(|p| p.set(name));
            let dumped_by_itself = matches!(pass, Pass::Function(one) if one.dumps_inputs_itself());
            if inputs && !dumped_by_itself && !matches!(pass, Pass::Freeze(_) | Pass::Compute { .. }) {
                before(dump, number, name, module)?;
            }
            let pass = match pass {
                Pass::Function(pass) => pass,
                Pass::Program(_) => unreachable!("a program pass runs over every module"),
                Pass::Freeze(kinds) => {
                    frozen.extend(kinds.iter().copied());
                    continue;
                }
                Pass::Compute { kind, again } => {
                    if *again && let Some((_, old)) = analyses.results.remove(&kind.id) {
                        analyses.dropped.insert(kind.id, old);
                    }
                    if !analyses.results.contains_key(&kind.id) {
                        analyses.computed(*kind, module);
                        analyses.invalidate_bodies(&PreservedAnalyses::none());
                    }
                    continue;
                }
                Pass::Module(pass) => {
                    if !bisected(name, "the module") {
                        continue;
                    }
                    let running = crate::counts::pass_run(name, 0);
                    let changed = spanned(name, || pass.run(module, analyses));
                    drop(running);
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
            // The sizes a pass that `wants_sizes` reads: of the functions a
            // constant names (a table of them), the only ones it can ask about.
            let sized = pass.wants_sizes();
            if sized {
                declared.sizes = referenced_sizes(module);
            }
            for at in 0..module.globals.len() {
                let id = GlobalId(at as u32);
                // The other bodies as they stand, for an audit that
                // reads a callee.
                if check_captures() {
                    MODULE_SNAPSHOT.with(|held| *held.borrow_mut() = Some(Rc::new(module.clone())));
                }
                let Module { context, globals, metadata, .. } = &mut *module;
                let global = &mut globals[at];
                let GlobalKind::Function(function) = &mut global.kind else { continue };
                if function.is_declaration()
                    || function.is_deleted()
                    || !bisected(name, global.name.as_deref().unwrap_or_default())
                {
                    continue;
                }
                set_running_function(global.name.as_deref().unwrap_or_default());
                let cache = analyses.manager(id, &outer);
                let before = function.mark();
                let running = crate::counts::pass_run_over(name, function);
                let preserved = in_function(global.name.as_deref().unwrap_or_default(), || {
                    spanned(name, || {
                        run_checked(
                            pass.as_mut(),
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
                drop(running);
                if let Some(problem) = take_phi_problem() {
                    return Err(problem);
                }
                kept.retain(|one| preserved.keeps(*one));
                spanned("invalidate", || cache.invalidate(function, &preserved));
                cache.check_kept(name, context, &layout, function);
                if self.verify_invalidation {
                    let stale = cache.stale(context, &layout, function);
                    if !stale.is_empty() {
                        return Err(format!("{name} claims to preserve {} but changed them", stale.join(", ")));
                    }
                }
                // An analysis that reads the log of edits is told of
                // this one.
                let moved = function.mark() != before;
                stages.push(Stage { pass: name, module: index, function: id, changes: function.take_changes() });
                if moved {
                    analyses.edit_log.push(id);
                }

                if declared.tables_pending() {
                    analyses.abandon_stateless();
                }
                spanned("declared", || declared.place(module))?;
                if sized
                    && let Some(size) = declared.sizes.get_mut(&GlobalId(at as u32))
                    && let GlobalKind::Function(function) = &module.globals[at].kind
                {
                    *size = Size::Instructions(function.live_instructions());
                }
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
            let mut checked = self.stateless.clone();
            if !pass.adds_memory_operations() {
                checked.extend(frozen.iter().copied());
            }
            checked_stale(module, analyses, &checked, name);
            after(dump, self.verify_each, number, name, module)?;
        }
        Ok(stages)
    }
}

thread_local! {
    /// The module as it stood before the function pass now running
    /// (`LLRM_CHECK_CAPTURES`).
    pub static MODULE_SNAPSHOT: std::cell::RefCell<Option<Rc<Module>>> = const { std::cell::RefCell::new(None) };
    static CHECK_STALE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `LLRM_CHECK_STALE` for this thread, for a test.
pub fn check_stale(on: bool) {
    CHECK_STALE.with(|check| check.set(on));
}

/// `LLRM_CHECK_STALE`, or `check_stale(true)` on this thread.
fn stale_checks_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    CHECK_STALE.with(std::cell::Cell::get) || *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_STALE").is_some())
}

static STALE_CHECKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// How many times `LLRM_CHECK_STALE` recomputed held analyses in this process:
/// `LLRM_DEBUG=checks` reports it (llrm-mir cannot name llrm-support's
/// counter), so a sweep sees whether the oracle ran at all.
pub fn stale_checks_run() -> u32 {
    STALE_CHECKS.load(std::sync::atomic::Ordering::Relaxed)
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
    let on = CHECK_STALE.with(std::cell::Cell::get) || *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_STALE"));
    if frozen.is_empty() || !on {
        return;
    }
    STALE_CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

/// `LLRM_DUMP_INPUTS`: the module pass `number` is about to run over, as
/// `NNNN-in-pass.ll`, with a line in `manifest.txt` (pass, number, file,
/// instructions), as the pipeline's `input` writes for a function.
fn before(
    dump: &Option<std::path::PathBuf>,
    number: usize,
    name: &str,
    module: &Module,
) -> Result<(), String> {
    let Some(directory) = dump else { return Ok(()) };
    let file = format!("{:04}-in-{name}.ll", number + 1);
    let instructions: usize = module
        .globals
        .iter()
        .map(|global| match &global.kind {
            GlobalKind::Function(function) => function.walk().count(),
            _ => 0,
        })
        .sum();
    let line = format!("{name} {} {file} {instructions}\n", number + 1);
    std::fs::create_dir_all(directory)
        .and_then(|()| std::fs::write(directory.join(&file), crate::print::module(module)))
        .and_then(|()| {
            let mut out = std::fs::OpenOptions::new().create(true).append(true).open(directory.join("manifest.txt"))?;
            std::io::Write::write_all(&mut out, line.as_bytes())
        })
        .map_err(|error| error.to_string())
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
