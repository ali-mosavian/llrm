//! Sparse conditional constant propagation: the solver. A translation of
//! LLVM's `SCCPSolver.cpp` (d1106deb71cc) without function specialisation:
//! an optimistic data flow over executable blocks and feasible edges, a
//! [`ValueLatticeElement`] per value, and the rewrites the result allows. It
//! solves one function ([`Solver::new`], for `runSCCP`) or a module
//! ([`Solver::module`], for `runIPSCCP`).
//!
//! # How it works
//! [`Solver::solve`] takes a block off the block work list, visits its
//! instructions once, and puts the users of every value whose lattice moved on
//! the instruction work list; only executable blocks and feasible edges are
//! visited. A value starts unknown, becomes undef, a constant, a range, or
//! overdefined, and only moves towards overdefined. When the lists are empty,
//! [`Solver::resolved_undefs_in`] makes what is still unknown overdefined (an
//! `undef` operand can be any value) and the solve resumes until nothing is
//! left to resolve.
//!
//! Across a module the work lists are shared and each entry names its
//! function. A call to a function whose arguments are tracked makes its entry
//! executable and joins the actuals into its arguments; a tracked return value
//! (`TrackedRetVals`, or one per field of a struct, `MRVFunctionsTracked`) is
//! joined from each `ret` and flows to each call; a tracked global
//! (`TrackedGlobals`) is joined from each store and read by each load. The
//! caller chooses what is tracked. [`crate::sccp`] then rewrites:
//! [`Solver::simplify_insts_in_block`] replaces values with their constants,
//! turns signed operations that cannot be negative into unsigned ones, adds
//! `nuw`/`nsw`/`nneg` the ranges prove, and drops range checks;
//! [`Solver::remove_non_feasible_edges`] cuts the edges that were never taken.
//!
//! ```text
//!  b0: br i1 true, b1, b2     b0: br b1      (b2 never executes)
//!  b1: %x = add i32 1, 2      b1: ret i32 3
//! ```
//!
//! # DEVIATEs
//! 1. `PredicateInfo` has no operand-bundle assumes ([`crate::predicateinfo`]
//!    DEVIATE 1); `removeSSACopies` is in [`crate::sccp`].
//! 2. Interprocedural: translated are
//!    [`addTrackedFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L919),
//!    [`trackValueOfGlobalVariable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L911),
//!    [`addArgumentTrackedFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L937)
//!    and its queries,
//!    [`addToMustPreserveReturnsInFunctions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L929),
//!    [`markFunctionUnreachable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1044),
//!    [`solveWhileResolvedUndefsIn(Module&)`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1049),
//!    and the tracked arms of
//!    [`visitReturnInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1489),
//!    [`visitStoreInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1902),
//!    [`visitLoadInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1945),
//!    [`handleCallArguments`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L2047),
//!    [`handleCallResult`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L2152)
//!    and
//!    [`resolvedUndef`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L2286).
//!    Not translated: the specialisation entry points (`invalidate`,
//!    `resetLatticeValueFor`, `setLatticeValueForSpecializationArguments`,
//!    `solveWhileResolvedUndefs`, `Invalidated`), which only
//!    `FunctionSpecializer` calls. Exit: its translation.
//! 3. No vectors and no scalable vectors: the `vscale` and
//!    `experimental.get.vector.length` arms of `handleCallResult` are absent.
//! 4. A floating-point compare is not folded: `ConstantFoldCompareInstOperands`
//!    in MIR takes integer predicates (an `fcmp` goes overdefined once its
//!    operands are known). Exit: the ConstantFolding translation's FP fold.
//! 5. No `DomTreeUpdater`: the manager recomputes dominators when the CFG
//!    changed.
//! 6. No `!implicit.ref`, `clang.arc.attachedcall` bundles or `indirectbr`: MIR
//!    has none.
//! 7. A phi has one input per edge in MIR, so a block reached twice by one
//!    predecessor is read twice (`removePredecessor` per edge, as ADCE).
//! 8. `ValueId` and `InstId` are local to a function, so the state keyed by
//!    value is per function: the visited function's state is in the solver's
//!    fields and the others are parked. A work-list entry names its function.
//! 9.
//!    [`pushUsersToWorkList`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1105)
//!    of a function or a global: MIR has no use lists of constants, so the
//!    users pushed are the calls and loads that read the tracked value, in the
//!    order they first did. The pin pushes only visited instructions too, and a
//!    store revisited changes nothing; the order of the pushes differs.
//!    `resolvedUndefsIn(M)` runs over the functions given to
//!    [`Solver::module`], in that order; one not given has nothing executable.
//!
//! # Example
//! `%c = icmp eq i32 %a, %a` with `%a` unknown is overdefined; with `%a = 1` it
//! is `true`, and the branch on it goes to one successor.
//!
//! # See also
//! - Uses: [`llrm_mir::valuelattice`], [`llrm_mir::constantrange`],
//!   [`llrm_mir::constantfolding`], [`llrm_mir::simplify`]
//! - Used by: [`crate::sccp`]

use llrm_analysis::occurrence::arms;
use llrm_mir::constantfolding::{self, Env as FoldEnv};
use llrm_mir::constantrange::{ConstantRange, NO_SIGNED_WRAP, NO_UNSIGNED_WRAP, PreferredRangeType, RangeIntrinsic};
use llrm_mir::context::GlobalId;
use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::facts::Facts;
use llrm_mir::lazyvalueinfo::constant_range_from_metadata;
use llrm_mir::module::{BlockId, Function, GlobalKind, GlobalValue, InstId, MetadataNode, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuelattice::{LatticeConstant, MergeOptions, ValueLatticeElement};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;
use llrm_mir::valuetracking::nonzero::null_pointer_is_defined;
use llrm_mir::valuetracking::pattern::{self, IntrinsicId, call_arguments, call_info, called_function};
use llrm_support::hash::{HashMap, HashSet};

use crate::predicateinfo::{CmpPredicate, PredicateBase, PredicateInfo};

type Lattice = ValueLatticeElement;

/// The maximum number of range extensions allowed for operations requiring
/// widening.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L46>
const MAX_NUM_RANGE_EXTENSIONS: u32 = 10;

/// `getMaxWidenStepsOpts`.
fn max_widen_steps_opts() -> MergeOptions {
    MergeOptions::default().set_max_widen_steps(MAX_NUM_RANGE_EXTENSIONS)
}

/// `SCCPSolver::isConstant`.
pub fn is_constant(lv: &Lattice) -> bool {
    lv.is_constant() || (lv.is_constant_range(true) && lv.get_constant_range(true).is_single_element())
}

/// `SCCPSolver::isReplaceableConstant`.
pub fn is_replaceable_constant(lv: &Lattice) -> bool {
    is_constant(lv) && !lv.may_have_different_provenance()
}

/// `SCCPSolver::isOverdefined`.
pub fn is_overdefined(lv: &Lattice) -> bool {
    !lv.is_unknown_or_undef() && !is_constant(lv)
}

/// What the solver reads of the program besides the function: LLVM's `DL`,
/// and the module the function is in.
pub struct Env<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub outer: &'a Outer,
    pub metadata: &'a [MetadataNode],
    /// The function's own id, for a call to itself (`TrackedRetVals`).
    pub id: Option<GlobalId>,
    /// `canTrackReturnsInterprocedurally(&F)`: the function's return value is
    /// tracked.
    pub tracks_returns: bool,
    /// `addPredicateInfo`: the copies made in the function.
    pub predicates: &'a PredicateInfo,
}

/// What the module solver reads besides the functions.
pub struct ModuleEnv<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    /// The module's globals, with the bodies of the functions given.
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
}

/// The state of one function keyed by its values and blocks (DEVIATE 8).
struct Local<'a> {
    function: &'a Function,
    predicates: &'a PredicateInfo,
    bb_executable: IdSet<BlockId>,
    bb_visited: IdSet<BlockId>,
    value_state: IdMap<ValueId, Lattice>,
    struct_state: HashMap<(ValueId, u32), Lattice>,
    in_work: IdSet<InstId>,
    current: Option<InstId>,
    feasible: HashSet<(BlockId, BlockId)>,
    additional_users: IdMap<ValueId, Vec<InstId>>,
}

impl<'a> Local<'a> {
    fn new(
        function: &'a Function,
        predicates: &'a PredicateInfo,
    ) -> Self {
        Self {
            function,
            predicates,
            bb_executable: IdSet::new(),
            bb_visited: IdSet::new(),
            value_state: IdMap::new(),
            struct_state: HashMap::default(),
            in_work: IdSet::new(),
            current: None,
            feasible: HashSet::default(),
            additional_users: IdMap::new(),
        }
    }
}

/// What is tracked of one function across calls, kept outside [`Local`] so a
/// caller reads it.
struct Tracked<'a> {
    id: Option<GlobalId>,
    function: &'a Function,
    /// `TrackedRetVals[F]`.
    ret: Option<Lattice>,
    /// `TrackedMultipleRetVals[{F, i}]`, for `MRVFunctionsTracked`.
    multiple: Option<Vec<Lattice>>,
    /// In `TrackingIncomingArguments`.
    arguments: bool,
    /// In `MustPreserveReturnsInFunctions`.
    must_preserve_return: bool,
    /// The calls that read `ret` or `multiple` (DEVIATE 9).
    users: Users,
}

/// Instructions of any function, each once, in the order added.
#[derive(Default)]
struct Users {
    list: Vec<(u32, InstId)>,
    seen: HashSet<(u32, InstId)>,
}

impl Users {
    fn add(
        &mut self,
        user: (u32, InstId),
    ) {
        if self.seen.insert(user) {
            self.list.push(user);
        }
    }
}

/// What a visit puts on the lists.
pub struct Solver<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    globals: &'a [GlobalValue],
    metadata: &'a [MetadataNode],
    // The state of the function `active` (DEVIATE 8); `parked[active]` holds
    // nothing of use while it is here.
    function: &'a Function,
    predicates: &'a PredicateInfo,
    /// `BBExecutable`: executable, but maybe not visited yet.
    bb_executable: IdSet<BlockId>,
    /// `BBVisited`.
    bb_visited: IdSet<BlockId>,
    /// `ValueState`.
    value_state: IdMap<ValueId, Lattice>,
    /// `StructValueState`.
    struct_state: HashMap<(ValueId, u32), Lattice>,
    in_work: IdSet<InstId>,
    /// `CurI`.
    current: Option<InstId>,
    /// `KnownFeasibleEdges`.
    feasible: HashSet<(BlockId, BlockId)>,
    /// `AdditionalUsers`.
    additional_users: IdMap<ValueId, Vec<InstId>>,
    active: usize,
    parked: Vec<Local<'a>>,
    tracked: Vec<Tracked<'a>>,
    slot_of: HashMap<GlobalId, usize>,
    /// `!TrackingIncomingArguments.empty()`.
    tracks_arguments: bool,
    /// The order of `TrackedRetVals`.
    ret_order: Vec<usize>,
    /// `TrackedGlobals`, and the loads that read each.
    tracked_globals: HashMap<GlobalId, Lattice>,
    global_users: HashMap<GlobalId, Users>,
    /// `InstWorkList`: a stack that holds each instruction once.
    inst_work: Vec<(u32, InstId)>,
    /// `BBWorkList`.
    bb_work: Vec<(u32, BlockId)>,
}

impl<'a> Solver<'a> {
    /// `SCCPSolver(DL, GetTLI, Ctx)` with `addTrackedFunction(&F)`,
    /// `markBlockExecutable(&F.front())` and `trackValueOfArgument` for each
    /// argument, as `runSCCP` does.
    pub fn new(
        env: Env<'a>,
        function: &'a Function,
    ) -> Self {
        let Env { context, layout, outer, metadata, id, tracks_returns, predicates } = env;
        let env = ModuleEnv { context, layout, globals: &outer.globals, metadata };
        let mut solver = Self::with(env, vec![(id, function, predicates)]);
        if tracks_returns && let Some(id) = id {
            solver.add_tracked_function(id);
        }
        if let Some(entry) = function.entry() {
            solver.mark_block_executable(entry);
        }
        for (index, &argument) in function.parameters().iter().enumerate() {
            solver.track_value_of_argument(argument, index as u32);
        }
        solver
    }

    /// `SCCPSolver(DL, GetTLI, Ctx)` with `addPredicateInfo` for each defined
    /// function, in module order; `None` when there is none. Nothing is
    /// tracked or executable until the caller says so.
    pub fn module(
        env: ModuleEnv<'a>,
        functions: Vec<(GlobalId, &'a Function, &'a PredicateInfo)>,
    ) -> Option<Self> {
        if functions.is_empty() {
            return None;
        }
        Some(Self::with(
            env,
            functions.into_iter().map(|(id, function, predicates)| (Some(id), function, predicates)).collect(),
        ))
    }

    fn with(
        env: ModuleEnv<'a>,
        functions: Vec<(Option<GlobalId>, &'a Function, &'a PredicateInfo)>,
    ) -> Self {
        let ModuleEnv { context, layout, globals, metadata } = env;
        let (_, function, predicates) = functions[0];
        let mut slot_of = HashMap::default();
        let mut parked = Vec::with_capacity(functions.len());
        let mut tracked = Vec::with_capacity(functions.len());
        for (slot, &(id, function, predicates)) in functions.iter().enumerate() {
            if let Some(id) = id {
                slot_of.insert(id, slot);
            }
            parked.push(Local::new(function, predicates));
            tracked.push(Tracked {
                id,
                function,
                ret: None,
                multiple: None,
                arguments: false,
                must_preserve_return: false,
                users: Users::default(),
            });
        }
        Self {
            context,
            layout,
            globals,
            metadata,
            function,
            predicates,
            bb_executable: IdSet::new(),
            bb_visited: IdSet::new(),
            value_state: IdMap::new(),
            struct_state: HashMap::default(),
            in_work: IdSet::new(),
            current: None,
            feasible: HashSet::default(),
            additional_users: IdMap::new(),
            active: 0,
            parked,
            tracked,
            slot_of,
            tracks_arguments: false,
            ret_order: Vec::new(),
            tracked_globals: HashMap::default(),
            global_users: HashMap::default(),
            inst_work: Vec::new(),
            bb_work: Vec::new(),
        }
    }

    // ---- functions -------------------------------------------------------

    /// Swaps the active function's state with `local`.
    fn exchange(
        &mut self,
        local: &mut Local<'a>,
    ) {
        use std::mem::swap;
        swap(&mut self.function, &mut local.function);
        swap(&mut self.predicates, &mut local.predicates);
        swap(&mut self.bb_executable, &mut local.bb_executable);
        swap(&mut self.bb_visited, &mut local.bb_visited);
        swap(&mut self.value_state, &mut local.value_state);
        swap(&mut self.struct_state, &mut local.struct_state);
        swap(&mut self.in_work, &mut local.in_work);
        swap(&mut self.current, &mut local.current);
        swap(&mut self.feasible, &mut local.feasible);
        swap(&mut self.additional_users, &mut local.additional_users);
    }

    /// Makes the function in `slot` the active one.
    fn enter(
        &mut self,
        slot: usize,
    ) {
        if slot == self.active {
            return;
        }
        let mut parked = std::mem::take(&mut self.parked);
        self.exchange(&mut parked[self.active]);
        self.exchange(&mut parked[slot]);
        self.parked = parked;
        self.active = slot;
    }

    fn slot(
        &self,
        id: GlobalId,
    ) -> usize {
        *self.slot_of.get(&id).expect("a function given to the solver")
    }

    /// The tracked function a call names directly (`getCalledFunction`).
    fn tracked_callee(
        &self,
        call: InstId,
    ) -> Option<usize> {
        let global = memory_callee(self.context, self.function, call)?;
        self.slot_of.get(&global).copied()
    }

    /// `addTrackedFunction`: an entry, F -> unknown.
    pub fn add_tracked_function(
        &mut self,
        id: GlobalId,
    ) {
        let slot = self.slot(id);
        let Type::Function { returns, .. } = self.context.types.get(self.tracked[slot].function.ty).clone() else {
            unreachable!("a function has a function type")
        };
        let tracked = &mut self.tracked[slot];
        if let Some(fields) = self.context.types.fields(returns) {
            tracked.multiple.get_or_insert_with(|| vec![Lattice::default(); fields.len()]);
        } else if !matches!(self.context.types.get(returns), Type::Void) && tracked.ret.is_none() {
            tracked.ret = Some(Lattice::default());
            self.ret_order.push(slot);
        }
    }

    /// `addToMustPreserveReturnsInFunctions`.
    pub fn add_to_must_preserve_returns_in_functions(
        &mut self,
        id: GlobalId,
    ) {
        let slot = self.slot(id);
        self.tracked[slot].must_preserve_return = true;
    }

    /// `mustPreserveReturn`.
    pub fn must_preserve_return(
        &self,
        id: GlobalId,
    ) -> bool {
        self.slot_of.get(&id).is_some_and(|&slot| self.tracked[slot].must_preserve_return)
    }

    /// `addArgumentTrackedFunction`.
    pub fn add_argument_tracked_function(
        &mut self,
        id: GlobalId,
    ) {
        let slot = self.slot(id);
        self.tracked[slot].arguments = true;
        self.tracks_arguments = true;
    }

    /// `isArgumentTrackedFunction`.
    pub fn is_argument_tracked_function(
        &self,
        id: GlobalId,
    ) -> bool {
        self.slot_of.get(&id).is_some_and(|&slot| self.tracked[slot].arguments)
    }

    /// `getArgumentTrackedFunctions`.
    pub fn argument_tracked_functions(&self) -> impl Iterator<Item = GlobalId> + '_ {
        self.tracked.iter().filter(|one| one.arguments).filter_map(|one| one.id)
    }

    /// `trackValueOfGlobalVariable`: only the contents of a scalar global
    /// with an initializer are tracked.
    pub fn track_value_of_global_variable(
        &mut self,
        id: GlobalId,
    ) {
        let globals = self.globals;
        let Some(GlobalKind::Variable(variable)) = globals.get(id.0 as usize).map(|one| &one.kind) else { return };
        let Some(initializer) = variable.initializer else { return };
        if !matches!(
            self.context.types.get(variable.ty),
            Type::Int(_) | Type::Float(_) | Type::Pointer(_) | Type::Vector { .. }
        ) {
            return;
        }
        let constant = LatticeConstant::of(self.context, initializer);
        self.tracked_globals.entry(id).or_default().mark_constant(&constant, false);
    }

    /// `markBlockExecutable` of a block of the function `id`.
    pub fn mark_block_executable_in(
        &mut self,
        id: GlobalId,
        block: BlockId,
    ) -> bool {
        let back = self.active;
        self.enter(self.slot(id));
        let changed = self.mark_block_executable(block);
        self.enter(back);
        changed
    }

    /// `trackValueOfArgument` of the argument `index` of the function `id`.
    pub fn track_value_of_argument_in(
        &mut self,
        id: GlobalId,
        index: u32,
    ) {
        let back = self.active;
        self.enter(self.slot(id));
        let argument = self.function.parameters()[index as usize];
        self.track_value_of_argument(argument, index);
        self.enter(back);
    }

    /// `markOverdefined(V)` of a value of the function `id`.
    pub fn mark_overdefined_in(
        &mut self,
        id: GlobalId,
        value: ValueId,
    ) {
        let back = self.active;
        self.enter(self.slot(id));
        self.mark_overdefined(value);
        self.enter(back);
    }

    /// `markFunctionUnreachable`.
    pub fn mark_function_unreachable(
        &mut self,
        id: GlobalId,
    ) {
        let back = self.active;
        self.enter(self.slot(id));
        self.bb_executable = IdSet::new();
        self.enter(back);
    }

    /// `isBlockExecutable` of a block of the function `id`.
    pub fn is_block_executable_in(
        &mut self,
        id: GlobalId,
        block: BlockId,
    ) -> bool {
        let back = self.active;
        self.enter(self.slot(id));
        let executable = self.is_block_executable(block);
        self.enter(back);
        executable
    }

    /// `getLatticeValueFor` of a value of the function `id`.
    pub fn lattice_value_in(
        &mut self,
        id: GlobalId,
        value: ValueId,
    ) -> Lattice {
        let back = self.active;
        self.enter(self.slot(id));
        let lv = self.get_lattice_value_for(value);
        self.enter(back);
        lv
    }

    /// `getTrackedRetVals`.
    pub fn tracked_ret_vals(&self) -> impl Iterator<Item = (GlobalId, &Lattice)> + '_ {
        self.ret_order
            .iter()
            .filter_map(
                |&slot| {
                    let tracked = &self.tracked[slot];
                    Some((tracked.id?, tracked.ret.as_ref()?))
                },
            )
    }

    /// `getTrackedGlobals`.
    pub fn tracked_globals(&self) -> &HashMap<GlobalId, Lattice> {
        &self.tracked_globals
    }

    /// `getMRVFunctionsTracked`.
    pub fn mrv_functions_tracked(&self) -> impl Iterator<Item = GlobalId> + '_ {
        self.tracked.iter().filter(|one| one.multiple.is_some()).filter_map(|one| one.id)
    }

    /// `pushUsersToWorkList(F)` or of a global: the instructions in `users`.
    fn push_users_in(
        &mut self,
        users: Vec<(u32, InstId)>,
    ) {
        let back = self.active;
        for (slot, user) in users {
            self.enter(slot as usize);
            self.push_to_work_list(user);
        }
        self.enter(back);
    }

    // ---- types -----------------------------------------------------------

    fn type_of(
        &self,
        op: Operand,
    ) -> TypeId {
        self.function.operand_type(self.context, op).expect("a value or constant")
    }

    fn struct_fields(
        &self,
        ty: TypeId,
    ) -> Option<Vec<TypeId>> {
        self.context.types.fields(ty).map(<[TypeId]>::to_vec)
    }

    fn is_int(
        &self,
        ty: TypeId,
    ) -> bool {
        self.context.types.int_bits(ty).is_some()
    }

    fn is_pointer(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.context.types.get(ty), Type::Pointer(_))
    }

    fn address_space(
        &self,
        ty: TypeId,
    ) -> u32 {
        match self.context.types.get(ty) {
            Type::Pointer(space) => *space,
            _ => 0,
        }
    }

    fn query(&self) -> Query<'_> {
        Query {
            context: &*self.context,
            globals: self.globals,
            metadata: self.metadata,
            function: self.function,
            layout: self.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    fn fold_env(&self) -> FoldEnv<'a> {
        FoldEnv { globals: self.globals, layout: self.layout }
    }

    // ---- constants -------------------------------------------------------

    /// `getConstant(LV, Ty)`: the constant a lattice value is, if it is one.
    pub fn get_constant(
        &self,
        lv: &Lattice,
        ty: TypeId,
    ) -> Option<ConstantId> {
        constant_of(self.context, lv, ty)
    }

    /// `getConstantInt(IV, Ty)`: the bits of an integer constant.
    fn get_constant_int(
        &self,
        lv: &Lattice,
        ty: TypeId,
    ) -> Option<u128> {
        let id = self.get_constant(lv, ty)?;
        match self.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero | ConstantKind::Null if self.is_int(ty) => Some(0),
            _ => None,
        }
    }

    // ---- lattice state ---------------------------------------------------

    /// `getValueState(V)` of a non-struct value, by copy.
    fn get_value_state(
        &mut self,
        op: Operand,
    ) -> Lattice {
        match op {
            Operand::Value(value) => self.value_state.get_or_insert_with(value, Lattice::default).clone(),
            Operand::Constant(id) => Lattice::get(&LatticeConstant::of(self.context, id)),
            Operand::Block(_) => unreachable!("a block has no lattice value"),
        }
    }

    /// `ValueState[V]`.
    fn state_mut(
        &mut self,
        value: ValueId,
    ) -> &mut Lattice {
        self.value_state.get_or_insert_with(value, Lattice::default)
    }

    /// `getStructValueState(V, i)`, by copy.
    fn get_struct_value_state(
        &mut self,
        op: Operand,
        index: u32,
    ) -> Lattice {
        match op {
            Operand::Value(value) => self.struct_state.entry((value, index)).or_default().clone(),
            Operand::Constant(id) => {
                let mut lv = Lattice::default();
                match self.aggregate_element(id, index) {
                    Some(element) => {
                        lv.mark_constant(&LatticeConstant::of(self.context, element), false);
                    }
                    None => {
                        lv.mark_overdefined();
                    }
                }
                lv
            }
            Operand::Block(_) => unreachable!("a block has no lattice value"),
        }
    }

    /// `Constant::getAggregateElement(i)` of a struct constant.
    fn aggregate_element(
        &self,
        id: ConstantId,
        index: u32,
    ) -> Option<ConstantId> {
        let constant = self.context.get(id);
        let field = *self.context.types.fields(constant.ty)?.get(index as usize)?;
        match &constant.kind {
            ConstantKind::Aggregate(elements) => elements.get(index as usize).copied(),
            ConstantKind::Zero | ConstantKind::Null => {
                Some(self.context.constant(Constant { ty: field, kind: zero_kind(self.context, field) }))
            }
            ConstantKind::Undef => Some(self.context.constant(Constant { ty: field, kind: ConstantKind::Undef })),
            ConstantKind::Poison => Some(self.context.constant(Constant { ty: field, kind: ConstantKind::Poison })),
            _ => None,
        }
    }

    /// `StructValueState[{V, i}]`, mutably.
    fn struct_state_mut(
        &mut self,
        value: ValueId,
        index: u32,
    ) -> &mut Lattice {
        self.struct_state.entry((value, index)).or_default()
    }

    /// `getLatticeValueFor(V)`: the solved state of a non-struct value.
    pub fn get_lattice_value_for(
        &self,
        value: ValueId,
    ) -> Lattice {
        self.value_state.get(&value).cloned().unwrap_or_default()
    }

    /// `getStructLatticeValueFor(V)`.
    fn get_struct_lattice_value_for(
        &self,
        value: ValueId,
        fields: usize,
    ) -> Vec<Lattice> {
        (0..fields as u32).map(|index| self.struct_state.get(&(value, index)).cloned().unwrap_or_default()).collect()
    }

    // ---- work lists ------------------------------------------------------

    /// `markBlockExecutable`.
    pub fn mark_block_executable(
        &mut self,
        block: BlockId,
    ) -> bool {
        if !self.bb_executable.insert(block) {
            return false;
        }
        self.bb_work.push((self.active as u32, block));
        true
    }

    /// `pushToWorkList`.
    fn push_to_work_list(
        &mut self,
        inst: InstId,
    ) {
        let parent = self.function.parent(inst);
        // Instructions after the one being visited in its block are visited
        // anyway; earlier ones (phis, loads of tracked globals) are not.
        if let Some(current) = self.current
            && parent == self.function.parent(current)
            && self.function.comes_before(current, inst)
        {
            return;
        }
        // An instruction of a block not visited yet is handled when it is.
        if parent.is_some_and(|block| self.bb_visited.contains(&block)) && self.in_work.insert(inst) {
            self.inst_work.push((self.active as u32, inst));
        }
    }

    /// `pushUsersToWorkList`.
    fn push_users_to_work_list(
        &mut self,
        value: ValueId,
    ) {
        let users: Vec<InstId> = self.function.users(value).iter().map(|one| one.user).collect();
        for user in users {
            self.push_to_work_list(user);
        }
        if let Some(extra) = self.additional_users.get(&value) {
            // Copied before notifying: a notified user may add more.
            for user in extra.clone() {
                self.push_to_work_list(user);
            }
        }
    }

    // ---- transitions -----------------------------------------------------

    /// `markConstant(IV, V, C, MayIncludeUndef)`.
    fn mark_constant(
        &mut self,
        value: ValueId,
        constant: &LatticeConstant,
        may_include_undef: bool,
    ) -> bool {
        if !self.state_mut(value).mark_constant(constant, may_include_undef) {
            return false;
        }
        self.push_users_to_work_list(value);
        true
    }

    /// `markNotConstant`.
    fn mark_not_constant(
        &mut self,
        value: ValueId,
        constant: &LatticeConstant,
    ) -> bool {
        if !self.state_mut(value).mark_not_constant(constant) {
            return false;
        }
        self.push_users_to_work_list(value);
        true
    }

    /// `markNotNull`.
    fn mark_not_null(
        &mut self,
        value: ValueId,
    ) -> bool {
        let ty = self.function.value(value).ty;
        let null = LatticeConstant::null_value(self.context, ty);
        self.mark_not_constant(value, &null)
    }

    /// `markOverdefined(IV, V)` of a non-struct value.
    fn mark_overdefined_value(
        &mut self,
        value: ValueId,
    ) -> bool {
        if !self.state_mut(value).mark_overdefined() {
            return false;
        }
        self.push_users_to_work_list(value);
        true
    }

    /// `markOverdefined(IV, V)` of a struct field.
    fn mark_overdefined_field(
        &mut self,
        value: ValueId,
        index: u32,
    ) -> bool {
        if !self.struct_state_mut(value, index).mark_overdefined() {
            return false;
        }
        self.push_users_to_work_list(value);
        true
    }

    /// `markOverdefined(V)`: every field of a struct, or the value.
    fn mark_overdefined(
        &mut self,
        value: ValueId,
    ) {
        let ty = self.function.value(value).ty;
        match self.struct_fields(ty) {
            Some(fields) => {
                for index in 0..fields.len() as u32 {
                    self.mark_overdefined_field(value, index);
                }
            }
            None => {
                self.mark_overdefined_value(value);
            }
        }
    }

    /// `mergeInValue(IV, V, MergeWithV, Opts)` of a non-struct value.
    fn merge_in_value(
        &mut self,
        value: ValueId,
        with: &Lattice,
        opts: MergeOptions,
    ) -> bool {
        if self.state_mut(value).merge_in(with, opts) {
            self.push_users_to_work_list(value);
            return true;
        }
        false
    }

    /// `mergeInValue(IV, V, ..)` of a struct field.
    fn merge_in_field(
        &mut self,
        value: ValueId,
        index: u32,
        with: &Lattice,
        opts: MergeOptions,
    ) -> bool {
        if self.struct_state_mut(value, index).merge_in(with, opts) {
            self.push_users_to_work_list(value);
            return true;
        }
        false
    }

    /// `getArgAttributeVL`.
    fn get_arg_attribute_vl(
        &self,
        index: u32,
        ty: TypeId,
    ) -> Lattice {
        if self.is_int(ty)
            && let Some(range) = pattern::argument_range(&self.query(), index)
        {
            return Lattice::get_range(range, false);
        }
        if param_has_non_null(self.function, index) {
            return Lattice::get_not(&LatticeConstant::null_value(self.context, ty));
        }
        Lattice::get_overdefined()
    }

    /// `trackValueOfArgument`.
    fn track_value_of_argument(
        &mut self,
        argument: ValueId,
        index: u32,
    ) {
        let ty = self.function.value(argument).ty;
        if self.struct_fields(ty).is_some() {
            self.mark_overdefined(argument);
            return;
        }
        let from_attributes = self.get_arg_attribute_vl(index, ty);
        self.merge_in_value(argument, &from_attributes, MergeOptions::default());
    }

    /// `getValueFromMetadata`.
    fn get_value_from_metadata(
        &self,
        inst: InstId,
    ) -> Lattice {
        let instruction = self.function.instruction(inst);
        let q = self.query();
        let ty = instruction.ty;
        if matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
            if self.is_int(ty)
                && let Some(range) = pattern::call_range(&q, instruction)
            {
                return Lattice::get_range(range, false);
            }
            if self.is_pointer(ty) && call_returns_non_null(&q, instruction) {
                return Lattice::get_not(&LatticeConstant::null_value(self.context, ty));
            }
        }
        if let Some(width) = self.context.types.int_bits(ty)
            && let Some(node) = q.metadata_node(inst, "range")
            && let Some(range) = constant_range_from_metadata(self.context, node, width)
        {
            return Lattice::get_range(range, false);
        }
        if q.metadata_node(inst, "nonnull").is_some() {
            return Lattice::get_not(&LatticeConstant::null_value(self.context, ty));
        }
        Lattice::get_overdefined()
    }

    // ---- control flow ----------------------------------------------------

    /// `markEdgeExecutable`.
    fn mark_edge_executable(
        &mut self,
        source: BlockId,
        dest: BlockId,
    ) -> bool {
        if !self.feasible.insert((source, dest)) {
            return false;
        }
        if !self.mark_block_executable(dest) {
            // The destination was already executable: an edge into it became
            // feasible, so its phis have potentially new operands.
            let phis: Vec<InstId> = self
                .function
                .iter_block(dest)
                .take_while(|&one| self.function.instruction(one).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                self.push_to_work_list(phi);
            }
        }
        true
    }

    /// `isEdgeFeasible`.
    pub fn is_edge_feasible(
        &self,
        from: BlockId,
        to: BlockId,
    ) -> bool {
        self.feasible.contains(&(from, to))
    }

    /// `isBlockExecutable`.
    pub fn is_block_executable(
        &self,
        block: BlockId,
    ) -> bool {
        self.bb_executable.contains(&block)
    }

    /// `getFeasibleSuccessors`: one flag for each block operand of the
    /// terminator, in operand order (a switch's default is the first).
    fn get_feasible_successors(
        &mut self,
        term: InstId,
    ) -> Vec<bool> {
        let instruction = self.function.instruction(term);
        let count = instruction.operands.iter().filter(|one| matches!(one, Operand::Block(_))).count();
        let mut succs = vec![false; count];
        match &instruction.opcode {
            Opcode::Br if count == 1 => succs[0] = true,
            Opcode::Br => {
                let condition = instruction.operands[0];
                let state = self.get_value_state(condition);
                let ty = self.type_of(condition);
                match self.get_constant_int(&state, ty) {
                    // Overdefined condition variables, and branches on
                    // unfoldable constant conditions, mean the branch could
                    // go either way.
                    None => {
                        if !state.is_unknown_or_undef() {
                            succs[0] = true;
                            succs[1] = true;
                        }
                    }
                    // Constant condition variables mean the branch can only go
                    // a single way.
                    Some(bits) => succs[usize::from(bits & 1 == 0)] = true,
                }
            }
            // We cannot analyze special terminators, so consider all
            // successors executable.
            Opcode::Invoke(_) => succs.fill(true),
            Opcode::Switch => {
                let cases: Vec<(u128, u32, u32)> = instruction.operands[2..]
                    .chunks(2)
                    .enumerate()
                    .map(|(at, pair)| {
                        let Operand::Constant(value) = pair[0] else { unreachable!("a case value") };
                        let (bits, width) =
                            self.query().int_constant(Operand::Constant(value)).expect("an integer case");
                        (bits, width, 1 + at as u32)
                    })
                    .collect();
                if cases.is_empty() {
                    succs[0] = true;
                    return succs;
                }
                let condition = instruction.operands[0];
                let state = self.get_value_state(condition);
                let ty = self.type_of(condition);
                if let Some(bits) = self.get_constant_int(&state, ty) {
                    let successor = cases.iter().find(|case| case.0 == bits).map_or(0, |case| case.2);
                    succs[successor as usize] = true;
                    return succs;
                }
                // TODO in LLVM: switch on undef is UB; it passes false.
                if state.is_constant_range(false) {
                    let range = *state.get_constant_range(false);
                    let mut reachable = 0_u64;
                    for &(bits, _, at) in &cases {
                        if range.contains(bits) {
                            succs[at as usize] = true;
                            reachable += 1;
                        }
                    }
                    succs[0] = range.is_size_larger_than(reachable);
                    return succs;
                }
                // Overdefined or unknown condition? All destinations are
                // executable!
                if !state.is_unknown_or_undef() {
                    succs.fill(true);
                }
            }
            _ => unreachable!("SCCP: don't know how to handle this terminator"),
        }
        succs
    }

    // ---- visitors --------------------------------------------------------

    fn visit(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        match &instruction.opcode {
            Opcode::Phi => self.visit_phi(inst),
            Opcode::Ret => self.visit_return(inst),
            Opcode::Br | Opcode::Switch => self.visit_terminator(inst),
            Opcode::Invoke(_) => {
                self.visit_call_base(inst);
                self.visit_terminator(inst);
            }
            Opcode::Call(_) => self.visit_call_base(inst),
            Opcode::Cast(op) => self.visit_cast(inst, *op),
            Opcode::Select => self.visit_select(inst),
            Opcode::FNeg => self.visit_unary(inst),
            Opcode::Freeze => self.visit_freeze(inst),
            Opcode::Binary(op) => self.visit_binary(inst, *op),
            Opcode::ICmp(pred) => self.visit_icmp(inst, *pred),
            Opcode::FCmp(_) => self.visit_fcmp(inst),
            Opcode::ExtractValue(_) => self.visit_extract_value(inst),
            Opcode::InsertValue(_) => self.visit_insert_value(inst),
            Opcode::Load { volatile, .. } => self.visit_load(inst, *volatile),
            Opcode::GetElementPtr { .. } => self.visit_get_element_ptr(inst),
            Opcode::Alloca { address_space, .. } => self.visit_alloca(inst, *address_space),
            // `ret`-like and void instructions with nothing to compute.
            Opcode::Resume | Opcode::Unreachable => {}
            Opcode::Store { .. } => self.visit_store(inst),
            // An atomic access is no simple load or store: its result is
            // overdefined and the global it touches is not tracked.
            Opcode::LandingPad { .. } | Opcode::Atomic(_) => self.visit_instruction(inst),
        }
    }

    /// `visitInstruction`: what has no special handling goes overdefined.
    fn visit_instruction(
        &mut self,
        inst: InstId,
    ) {
        if let Some(result) = self.function.instruction(inst).result {
            self.mark_overdefined(result);
        }
    }

    /// `isInstFullyOverDefined`.
    fn is_inst_fully_overdefined(
        &self,
        inst: InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        let Some(result) = instruction.result else { return true };
        match self.context.types.fields(instruction.ty) {
            Some(fields) => (0..fields.len() as u32)
                .all(|index| self.struct_state.get(&(result, index)).is_some_and(Lattice::is_overdefined)),
            None => self.value_state.get(&result).is_some_and(Lattice::is_overdefined),
        }
    }

    /// `visitPHINode`.
    fn visit_phi(
        &mut self,
        phi: InstId,
    ) {
        let result = self.function.instruction(phi).result.expect("a phi has a value");
        let ty = self.function.instruction(phi).ty;
        let incoming = arms(self.function, phi);
        // Super-extra-high-degree PHI nodes are unlikely to ever be marked
        // constant, and slow us down a lot. Just mark them overdefined.
        if incoming.len() > 64 {
            self.mark_overdefined(result);
            return;
        }
        if self.is_inst_fully_overdefined(phi) {
            return;
        }
        let parent = self.function.parent(phi).expect("a placed phi");
        let feasible: Vec<usize> =
            (0..incoming.len()).filter(|&at| self.is_edge_feasible(incoming[at].1, parent)).collect();
        // We allow up to 1 range extension per active incoming value and one
        // additional extension. The count is adjusted to match the number of
        // active incoming values, which limits multiple extensions caused by
        // the same incoming value.
        let opts = MergeOptions::default().set_max_widen_steps(feasible.len() as u32 + 1);
        let extensions = feasible.len() as u32;
        if let Some(fields) = self.struct_fields(ty) {
            for index in 0..fields.len() as u32 {
                let mut state = self.struct_state_mut(result, index).clone();
                if state.is_overdefined() {
                    continue;
                }
                for &at in &feasible {
                    let value = self.get_struct_value_state(incoming[at].0, index);
                    state.merge_in(&value, MergeOptions::default());
                    if state.is_overdefined() {
                        break;
                    }
                }
                self.merge_in_field(result, index, &state, opts);
                let slot = self.struct_state_mut(result, index);
                slot.set_num_range_extensions(extensions.max(slot.get_num_range_extensions()));
            }
        } else {
            let mut state = self.get_value_state(Operand::Value(result));
            for &at in &feasible {
                let value = self.get_value_state(incoming[at].0);
                state.merge_in(&value, MergeOptions::default());
                if state.is_overdefined() {
                    break;
                }
            }
            self.merge_in_value(result, &state, opts);
            let slot = self.state_mut(result);
            slot.set_num_range_extensions(extensions.max(slot.get_num_range_extensions()));
        }
    }

    /// `visitReturnInst`.
    fn visit_return(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let Some(&result) = instruction.operands.first() else { return };
        let slot = self.active;
        // If we are tracking the return value of this function, merge it in.
        if self.tracked[slot].ret.is_some() {
            if self.struct_fields(self.type_of(result)).is_some() {
                return;
            }
            let state = self.get_value_state(result);
            let tracked = self.tracked[slot].ret.as_mut().expect("a tracked return");
            if tracked.merge_in(&state, MergeOptions::default()) {
                self.push_users_in(self.tracked[slot].users.list.clone());
            }
            return;
        }
        // Handle functions that return multiple values.
        if let Some(fields) = self.tracked[slot].multiple.as_ref().map(Vec::len) {
            for index in 0..fields as u32 {
                let state = self.get_struct_value_state(result, index);
                let tracked = &mut self.tracked[slot].multiple.as_mut().expect("tracked returns")[index as usize];
                if tracked.merge_in(&state, MergeOptions::default()) {
                    self.push_users_in(self.tracked[slot].users.list.clone());
                }
            }
        }
    }

    /// `visitStoreInst`.
    fn visit_store(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let (value, pointer) = (instruction.operands[0], instruction.operands[1]);
        // If this store is of a struct, ignore it (tested second: it is the
        // dearer test).
        if self.tracked_globals.is_empty() || self.struct_fields(self.type_of(value)).is_some() {
            return;
        }
        let Some(global) = self.global_operand(pointer) else { return };
        if !self.tracked_globals.contains_key(&global) {
            return;
        }
        // Get the value we are storing into the global, then merge it.
        let state = self.get_value_state(value);
        let tracked = self.tracked_globals.get_mut(&global).expect("a tracked global");
        if tracked.merge_in(&state, MergeOptions::default().set_check_widen(false)) {
            let users = self.global_users.get(&global).map(|one| one.list.clone()).unwrap_or_default();
            self.push_users_in(users);
        }
        if self.tracked_globals[&global].is_overdefined() {
            // No need to keep tracking this!
            self.tracked_globals.remove(&global);
        }
    }

    /// The global `op` is, if it is one.
    fn global_operand(
        &self,
        op: Operand,
    ) -> Option<GlobalId> {
        let Operand::Constant(id) = op else { return None };
        match self.context.get(id).kind {
            ConstantKind::Global(global) => Some(global),
            _ => None,
        }
    }

    /// `visitTerminator`.
    fn visit_terminator(
        &mut self,
        term: InstId,
    ) {
        let feasible = self.get_feasible_successors(term);
        let block = self.function.parent(term).expect("a placed terminator");
        let successors: Vec<BlockId> = self
            .function
            .instruction(term)
            .operands
            .iter()
            .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
            .collect();
        // Mark all feasible successors executable.
        for (at, &to) in successors.iter().enumerate() {
            if feasible[at] {
                self.mark_edge_executable(block, to);
            }
        }
    }

    /// `visitCastInst`.
    fn visit_cast(
        &mut self,
        inst: InstId,
        op: CastOp,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a cast has a value");
        // ResolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            return;
        }
        let operand = instruction.operands[0];
        let source = self.type_of(operand);
        let predicates = self.predicates;
        if op == CastOp::BitCast
            && instruction.ty == source
            && let Some(predicate) = predicates.predicate_info_for(inst)
        {
            self.handle_predicate(inst, operand, predicate);
            return;
        }
        let state = self.get_value_state(operand);
        if state.is_unknown_or_undef() {
            return;
        }
        if let Some(constant) = self.get_constant(&state, source) {
            // Fold the constant as we build.
            if let Some(folded) =
                constantfolding::constant_fold_cast_operand(self.context, self.layout, op, constant, instruction.ty)
            {
                let lv = Lattice::get(&LatticeConstant::of(self.context, folded));
                self.merge_in_value(result, &lv, MergeOptions::default());
                return;
            }
        }
        // Ignore bitcasts, as they may change the number of vector elements.
        if let Some(dest_bits) = self.context.types.int_bits(instruction.ty)
            && let Some(source_bits) = self.context.types.int_bits(source)
            && op != CastOp::BitCast
        {
            let operand_range = state.as_constant_range(source_bits, false);
            let range = if op == CastOp::Trunc {
                operand_range.truncate(dest_bits, no_wrap_kind(instruction.flags))
            } else {
                operand_range.cast_op(op, dest_bits)
            };
            self.get_value_state(Operand::Value(result));
            self.merge_in_value(result, &Lattice::get_range(range, false), MergeOptions::default());
        } else {
            self.mark_overdefined(result);
        }
    }

    /// `handleExtractOfWithOverflow`.
    fn handle_extract_of_with_overflow(
        &mut self,
        extract: InstId,
        call: InstId,
        id: IntrinsicId,
        index: u32,
    ) {
        let result = self.function.instruction(extract).result.expect("an extractvalue has a value");
        let operands = self.function.instruction(call).operands.clone();
        let (lhs, rhs) = (operands[0], operands[1]);
        let ty = self.type_of(lhs);
        let width = self.context.types.int_bits(ty).expect("an integer operand");
        self.add_additional_user(lhs, extract);
        self.add_additional_user(rhs, extract);

        let l = self.get_value_state(lhs);
        if l.is_unknown_or_undef() {
            return; // Wait to resolve.
        }
        let lr = l.as_constant_range(width, false);
        let r = self.get_value_state(rhs);
        if r.is_unknown_or_undef() {
            return; // Wait to resolve.
        }
        let rr = r.as_constant_range(width, false);
        let (op, kind) = with_overflow_op(id);
        if index == 0 {
            let range = lr.binary_op(op, &rr);
            self.merge_in_value(result, &Lattice::get_range(range, false), MergeOptions::default());
        } else {
            debug_assert_eq!(index, 1, "Index can only be 0 or 1");
            let region = ConstantRange::make_guaranteed_no_wrap_region(op, &rr, kind);
            if region.contains_range(&lr) {
                let false_value = LatticeConstant::Int { width: 1, bits: 0 };
                self.mark_constant(result, &false_value, false);
                return;
            }
            self.mark_overdefined(result);
        }
    }

    /// `addAdditionalUser`.
    fn add_additional_user(
        &mut self,
        value: Operand,
        user: InstId,
    ) {
        if let Operand::Value(value) = value {
            let users = self.additional_users.get_or_insert_with(value, Vec::new);
            if !users.contains(&user) {
                users.push(user);
            }
        }
    }

    /// `visitExtractValueInst`.
    fn visit_extract_value(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("an extractvalue has a value");
        let Opcode::ExtractValue(indices) = &instruction.opcode else { unreachable!() };
        // If this returns a struct, mark all elements over defined, we don't
        // track structs in structs.
        if self.struct_fields(instruction.ty).is_some() {
            return self.mark_overdefined(result);
        }
        // resolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            self.mark_overdefined(result);
            return;
        }
        // If this is extracting from more than one level of struct, we don't
        // know.
        if indices.len() != 1 {
            return self.mark_overdefined(result);
        }
        let aggregate = instruction.operands[0];
        if self.struct_fields(self.type_of(aggregate)).is_some() {
            let index = indices[0];
            if let Operand::Value(value) = aggregate
                && let ValueDef::Instruction(def) = self.function.value(value).def
                && let Some(id) = self.with_overflow_intrinsic(def)
            {
                return self.handle_extract_of_with_overflow(inst, def, id, index);
            }
            let element = self.get_struct_value_state(aggregate, index);
            self.merge_in_value(result, &element, MergeOptions::default());
        } else {
            // Otherwise, must be extracting from an array.
            self.mark_overdefined(result);
        }
    }

    /// `dyn_cast<WithOverflowInst>(V)`.
    fn with_overflow_intrinsic(
        &self,
        call: InstId,
    ) -> Option<IntrinsicId> {
        let id = pattern::call_intrinsic(&self.query(), call)?;
        matches!(
            id,
            IntrinsicId::SaddWithOverflow
                | IntrinsicId::SsubWithOverflow
                | IntrinsicId::SmulWithOverflow
                | IntrinsicId::UaddWithOverflow
                | IntrinsicId::UsubWithOverflow
                | IntrinsicId::UmulWithOverflow
        )
        .then_some(id)
    }

    /// `visitInsertValueInst`.
    fn visit_insert_value(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("an insertvalue has a value");
        let Opcode::InsertValue(indices) = &instruction.opcode else { unreachable!() };
        let Some(fields) = self.struct_fields(instruction.ty) else { return self.mark_overdefined(result) };
        // resolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        // If this has more than one index, we can't handle it, drive all
        // results to undef.
        if indices.len() != 1 {
            return self.mark_overdefined(result);
        }
        let aggregate = instruction.operands[0];
        let inserted = indices[0];
        // Compute the result based on what we're inserting.
        for index in 0..fields.len() as u32 {
            // This passes through all values that aren't the inserted element.
            if index != inserted {
                let element = self.get_struct_value_state(aggregate, index);
                self.merge_in_field(result, index, &element, MergeOptions::default());
                continue;
            }
            let value = instruction.operands[1];
            if self.struct_fields(self.type_of(value)).is_some() {
                // We don't track structs in structs.
                self.mark_overdefined_field(result, index);
            } else {
                let state = self.get_value_state(value);
                self.merge_in_field(result, index, &state, MergeOptions::default());
            }
        }
    }

    /// `visitSelectInst`.
    fn visit_select(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a select has a value");
        // If this select returns a struct, just mark the result overdefined.
        if self.struct_fields(instruction.ty).is_some() {
            return self.mark_overdefined(result);
        }
        // resolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        let condition = self.get_value_state(instruction.operands[0]);
        if condition.is_unknown_or_undef() {
            return;
        }
        let condition_ty = self.type_of(instruction.operands[0]);
        if let Some(bits) = self.get_constant_int(&condition, condition_ty) {
            let chosen = if bits & 1 == 0 { instruction.operands[2] } else { instruction.operands[1] };
            let state = self.get_value_state(chosen);
            self.merge_in_value(result, &state, MergeOptions::default());
            return;
        }
        // Otherwise, the condition is overdefined or a constant we can't
        // evaluate. See if we can produce something better than overdefined
        // based on the T/F value.
        let on_true = self.get_value_state(instruction.operands[1]);
        let on_false = self.get_value_state(instruction.operands[2]);
        let state = self.state_mut(result);
        let mut changed = state.merge_in(&on_true, MergeOptions::default());
        changed |= state.merge_in(&on_false, MergeOptions::default());
        if changed {
            self.push_users_to_work_list(result);
        }
    }

    /// `visitUnaryOperator`.
    fn visit_unary(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a unary operator has a value");
        let operand = self.get_value_state(instruction.operands[0]);
        // resolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        // If something is unknown/undef, wait for it to resolve.
        if operand.is_unknown_or_undef() {
            return;
        }
        if is_constant(&operand)
            && let Some(constant) = self.get_constant(&operand, instruction.ty)
            && let Some(folded) = constantfolding::constant_fold_unary_op_operand(self.context, constant)
        {
            let lv = LatticeConstant::of(self.context, folded);
            self.mark_constant(result, &lv, false);
            return;
        }
        self.mark_overdefined(result);
    }

    /// `visitFreezeInst`.
    fn visit_freeze(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a freeze has a value");
        // If this freeze returns a struct, just mark the result overdefined.
        if self.struct_fields(instruction.ty).is_some() {
            return self.mark_overdefined(result);
        }
        let operand = self.get_value_state(instruction.operands[0]);
        // resolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        // If something is unknown/undef, wait for it to resolve.
        if operand.is_unknown_or_undef() {
            return;
        }
        if is_constant(&operand)
            && let Some(constant) = self.get_constant(&operand, instruction.ty)
            && is_guaranteed_not_to_be_undef_or_poison(&self.query(), Operand::Constant(constant), 0)
        {
            let lv = LatticeConstant::of(self.context, constant);
            self.mark_constant(result, &lv, false);
            return;
        }
        self.mark_overdefined(result);
    }

    /// `visitBinaryOperator`.
    fn visit_binary(
        &mut self,
        inst: InstId,
        op: BinaryOp,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a binary operator has a value");
        let (lhs, rhs) = (instruction.operands[0], instruction.operands[1]);
        let state1 = self.get_value_state(lhs);
        let state2 = self.get_value_state(rhs);
        if self.state_mut(result).is_overdefined() {
            return;
        }
        // If something is undef, wait for it to resolve.
        if state1.is_unknown_or_undef() || state2.is_unknown_or_undef() {
            return;
        }
        if state1.is_overdefined() && state2.is_overdefined() {
            self.mark_overdefined(result);
            return;
        }
        // If either of the operands is a constant, try to fold it to a
        // constant. TODO in LLVM: use information from notconstant better.
        if state1.is_constant() || state2.is_constant() {
            let pick = |this: &Self, state: &Lattice, operand: Operand| {
                if is_constant(state) {
                    this.get_constant(state, this.type_of(operand)).map_or(operand, Operand::Constant)
                } else {
                    operand
                }
            };
            let (v1, v2) = (pick(self, &state1, lhs), pick(self, &state2, rhs));
            let folded = llrm_mir::simplify::simplify_binop_operands(
                self.context,
                self.layout,
                self.function,
                op,
                instruction.flags,
                v1,
                v2,
            );
            if let Some(Operand::Constant(constant)) = folded {
                // Conservatively assume that the result may be based on
                // operands that may be undef. The constant is merged with the
                // existing lattice value, as different constants might be
                // found after one of the operands go to overdefined.
                let mut new = Lattice::default();
                new.mark_constant(&LatticeConstant::of(self.context, constant), true);
                self.merge_in_value(result, &new, MergeOptions::default());
                return;
            }
        }
        // Only use ranges for binary operators on integers.
        let Some(width) = self.context.types.int_bits(instruction.ty) else {
            self.mark_overdefined(result);
            return;
        };
        // Try to simplify to a constant range.
        let a = state1.as_constant_range(width, false);
        let b = state2.as_constant_range(width, false);
        let range = a.binary_op_with_flags(
            op,
            &b,
            instruction.flags.contains(Flags::NUW),
            instruction.flags.contains(Flags::NSW),
            instruction.flags.contains(Flags::DISJOINT),
        );
        self.merge_in_value(result, &Lattice::get_range(range, false), MergeOptions::default());
    }

    /// `visitCmpInst` of an `icmp`.
    fn visit_icmp(
        &mut self,
        inst: InstId,
        pred: IntPredicate,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a compare has a value");
        // Do not cache this lookup, getValueState calls later in the function
        // might invalidate the reference.
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        let state1 = self.get_value_state(instruction.operands[0]);
        let state2 = self.get_value_state(instruction.operands[1]);
        let (context, globals, layout) = (&*self.context, self.globals, self.layout);
        let folded = state1.get_compare(pred, &state2, &mut |pred, c1, c2| {
            llrm_mir::valuelattice::fold_compare(context, globals, layout, pred, c1, c2)
        });
        if let Some(value) = folded {
            let mut lv = Lattice::default();
            lv.mark_constant(&LatticeConstant::Int { width: 1, bits: u128::from(value) }, false);
            self.merge_in_value(result, &lv, MergeOptions::default());
            return;
        }
        // If operands are still unknown, wait for it to resolve.
        if (state1.is_unknown_or_undef() || state2.is_unknown_or_undef()) && !is_constant(self.state_mut(result)) {
            return;
        }
        self.mark_overdefined(result);
    }

    /// `visitCmpInst` of an `fcmp`: see DEVIATE 4.
    fn visit_fcmp(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a compare has a value");
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        let state1 = self.get_value_state(instruction.operands[0]);
        let state2 = self.get_value_state(instruction.operands[1]);
        if (state1.is_unknown_or_undef() || state2.is_unknown_or_undef()) && !is_constant(self.state_mut(result)) {
            return;
        }
        self.mark_overdefined(result);
    }

    /// `visitGetElementPtrInst`.
    fn visit_get_element_ptr(
        &mut self,
        inst: InstId,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a getelementptr has a value");
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        let pointer = instruction.operands[0];
        let pointer_state = self.get_value_state(pointer);
        if pointer_state.is_unknown_or_undef() {
            return;
        }
        // gep inbounds/nuw of non-null is non-null.
        if pointer_state.is_not_constant() && pointer_state.get_not_constant().is_null_value() {
            let space = self.address_space(self.type_of(pointer));
            if instruction.flags.contains(Flags::NUW)
                || (instruction.flags.contains(Flags::INBOUNDS) && !null_pointer_is_defined(Some(self.function), space))
            {
                self.mark_not_null(result);
                return;
            }
            return self.mark_overdefined(result);
        }
        let may_have_different_provenance = pointer_state.may_have_different_provenance();
        let mut operands = Vec::with_capacity(instruction.operands.len());
        for &operand in &instruction.operands {
            let state = self.get_value_state(operand);
            if state.is_unknown_or_undef() {
                return; // Operands are not resolved yet.
            }
            match self.get_constant(&state, self.type_of(operand)) {
                Some(constant) => operands.push(constant),
                None => return self.mark_overdefined(result),
            }
        }
        let env = self.fold_env();
        match constantfolding::constant_fold_inst_operands_at(self.context, env, self.function, inst, &operands) {
            Some(folded) => {
                let lv = Lattice::get(&LatticeConstant::of(self.context, folded));
                self.merge_in_value(result, &lv, MergeOptions::default());
                // The pointer operand's lattice has found to be a constant,
                // however, the returned pointer of the GEP may not be freely
                // substituted, as it may have been derived from a pointer with
                // potentially different provenance.
                if may_have_different_provenance {
                    self.state_mut(result).set_may_have_different_provenance(true);
                }
            }
            None => self.mark_overdefined(result),
        }
    }

    /// `visitAllocaInst`.
    fn visit_alloca(
        &mut self,
        inst: InstId,
        address_space: u32,
    ) {
        let result = self.function.instruction(inst).result.expect("an alloca has a value");
        if !null_pointer_is_defined(Some(self.function), address_space) {
            self.mark_not_null(result);
            return;
        }
        self.mark_overdefined(result);
    }

    /// `visitLoadInst`.
    fn visit_load(
        &mut self,
        inst: InstId,
        volatile: bool,
    ) {
        let instruction = self.function.instruction(inst);
        let result = instruction.result.expect("a load has a value");
        // If this load is of a struct or the load is volatile, just mark the
        // result as overdefined.
        if self.struct_fields(instruction.ty).is_some() || volatile {
            return self.mark_overdefined(result);
        }
        // resolvedUndefsIn might mark I as overdefined. Bail out, even if we
        // would discover a concrete value later.
        if self.state_mut(result).is_overdefined() {
            return self.mark_overdefined(result);
        }
        let pointer = instruction.operands[0];
        let pointer_state = self.get_value_state(pointer);
        if pointer_state.is_unknown_or_undef() {
            return; // The pointer is not resolved yet!
        }
        if is_constant(&pointer_state) {
            let pointer_ty = self.type_of(pointer);
            let ptr = self.get_constant(&pointer_state, pointer_ty).expect("a constant pointer");
            // load null is undefined.
            if matches!(
                self.context.get(ptr).kind,
                ConstantKind::Null | ConstantKind::Zero
            ) {
                if null_pointer_is_defined(Some(self.function), self.address_space(pointer_ty)) {
                    self.mark_overdefined(result);
                }
                return;
            }
            // Transform load (constant global) into the value loaded: if we
            // are tracking this global, merge in the known value for it.
            if !self.tracked_globals.is_empty()
                && let ConstantKind::Global(global) = self.context.get(ptr).kind
                && let Some(tracked) = self.tracked_globals.get(&global).cloned()
            {
                // Only a load naming the global is one of its users.
                if self.global_operand(pointer) == Some(global) {
                    self.global_users.entry(global).or_default().add((self.active as u32, inst));
                }
                self.merge_in_value(result, &tracked, max_widen_steps_opts());
                return;
            }
            // Transform load from a constant into a constant if possible.
            if let Some(folded) = constantfolding::constant_fold_load_from_const_ptr(
                self.context,
                self.layout,
                self.globals,
                ptr,
                instruction.ty,
            ) {
                let lv = LatticeConstant::of(self.context, folded);
                self.mark_constant(result, &lv, false);
                return;
            }
        }
        // Fall back to metadata.
        let from_metadata = self.get_value_from_metadata(inst);
        self.merge_in_value(result, &from_metadata, MergeOptions::default());
    }

    /// `visitCallBase`.
    fn visit_call_base(
        &mut self,
        call: InstId,
    ) {
        self.handle_call_result(call);
        self.handle_call_arguments(call);
    }

    /// `handleCallArguments`: a call to a function whose arguments are
    /// tracked makes its entry executable, and merges the actuals into the
    /// formals.
    fn handle_call_arguments(
        &mut self,
        call: InstId,
    ) {
        if !self.tracks_arguments {
            return;
        }
        let Some(callee) = self.tracked_callee(call).filter(|&slot| self.tracked[slot].arguments) else { return };
        let caller = self.active;
        let function = self.tracked[callee].function;
        let actuals: Vec<Operand> = call_arguments(self.function.instruction(call)).to_vec();
        // If this argument is byval, and if the function is not readonly,
        // there will be an implicit copy formed of the input aggregate.
        let copies = llrm_mir::memory::stated(&function.attrs).writes;
        let mut incoming = Vec::with_capacity(actuals.len());
        if callee == caller {
            if let Some(entry) = function.entry() {
                self.mark_block_executable(entry);
            }
        }
        for (index, (&formal, &actual)) in function.parameters().iter().zip(&actuals).enumerate() {
            let value = if is_by_value(function, index) && copies {
                Incoming::Overdefined
            } else if let Some(fields) = self.struct_fields(function.value(formal).ty) {
                Incoming::Fields((0..fields.len() as u32).map(|at| self.get_struct_value_state(actual, at)).collect())
            } else {
                Incoming::Value(self.get_value_state(actual))
            };
            // A call to itself reads each actual after the formals before it
            // are merged, as the pin does.
            if callee == caller {
                self.merge_into_formal(formal, index as u32, value);
            } else {
                incoming.push((formal, index as u32, value));
            }
        }
        if callee == caller {
            return;
        }
        self.enter(callee);
        if let Some(entry) = function.entry() {
            self.mark_block_executable(entry);
        }
        for (formal, index, value) in incoming {
            self.merge_into_formal(formal, index, value);
        }
        self.enter(caller);
    }

    /// Propagate information from a call site into the formal `index` of the
    /// active function.
    fn merge_into_formal(
        &mut self,
        formal: ValueId,
        index: u32,
        value: Incoming,
    ) {
        match value {
            Incoming::Overdefined => self.mark_overdefined(formal),
            Incoming::Fields(fields) => {
                for (at, call_arg) in fields.iter().enumerate() {
                    self.merge_in_field(formal, at as u32, call_arg, max_widen_steps_opts());
                }
            }
            Incoming::Value(call_arg) => {
                let ty = self.function.value(formal).ty;
                let call_arg = call_arg.intersect(&self.get_arg_attribute_vl(index, ty));
                self.merge_in_value(formal, &call_arg, max_widen_steps_opts());
            }
        }
    }

    /// `handlePredicate`: the copy `inst` of `copy_of` is what `copy_of` is
    /// where the predicate holds.
    fn handle_predicate(
        &mut self,
        inst: InstId,
        copy_of: Operand,
        predicate: &PredicateBase,
    ) {
        let result = self.function.instruction(inst).result.expect("a copy has a value");
        let copy_of_val = self.get_value_state(copy_of);
        let Some(constraint) = predicate.get_constraint(self.context, self.function) else {
            self.merge_in_value(result, &copy_of_val, MergeOptions::default());
            return;
        };
        let (pred, other_op) = (constraint.predicate, constraint.other_op);
        // Wait until OtherOp is resolved.
        if self.get_value_state(other_op).is_unknown() {
            self.add_additional_user(other_op, inst);
            return;
        }
        let mut cond_val = self.get_value_state(other_op);
        let copy_of_ty = self.type_of(copy_of);
        if cond_val.is_constant_range(true) || copy_of_val.is_constant_range(true) {
            let bits = self.query().scalar_bits(copy_of);
            let mut imposed = ConstantRange::full(bits);
            // Get the range imposed by the condition.
            if cond_val.is_constant_range(true)
                && let CmpPredicate::Int(pred) = pred
            {
                imposed = ConstantRange::make_allowed_icmp_region(pred, cond_val.get_constant_range(true));
            }
            // Combine range info for the original value with the new range
            // from the condition.
            let mut copy_of_cr = copy_of_val.as_constant_range(bits, true);
            // Treat an unresolved input like a full range.
            if copy_of_cr.is_empty_set() {
                copy_of_cr = ConstantRange::full(copy_of_cr.width);
            }
            let mut new_cr = imposed.intersect_with(&copy_of_cr, PreferredRangeType::Smallest);
            // If the existing information is != x, do not use the information
            // from a chained predicate, as the != x information is more likely
            // to be helpful in practice.
            if !copy_of_cr.contains_range(&new_cr) && copy_of_cr.get_single_missing_element().is_some() {
                new_cr = copy_of_cr;
            }
            // The new range is based on a branch condition. That guarantees
            // that neither of the compare operands can be undef in the branch
            // targets, unless we have conditions that are always true/false.
            self.add_additional_user(other_op, inst);
            self.merge_in_value(result, &Lattice::get_range(new_cr, false), MergeOptions::default());
        } else if pred == CmpPredicate::Int(IntPredicate::Eq) && (cond_val.is_constant() || cond_val.is_not_constant())
        {
            // For non-integer values or integer constant expressions, only
            // propagate equal constants or not-constants.
            self.add_additional_user(other_op, inst);
            if self.is_pointer(copy_of_ty) {
                cond_val.set_may_have_different_provenance(true);
            }
            self.merge_in_value(result, &cond_val, MergeOptions::default());
        } else if pred == CmpPredicate::Int(IntPredicate::Ne) && cond_val.is_constant() {
            // Propagate inequalities.
            self.add_additional_user(other_op, inst);
            let not = Lattice::get_not(cond_val.get_constant());
            self.merge_in_value(result, &not, MergeOptions::default());
        } else {
            self.merge_in_value(result, &copy_of_val, MergeOptions::default());
        }
    }

    /// `handleCallResult`.
    fn handle_call_result(
        &mut self,
        call: InstId,
    ) {
        let instruction = self.function.instruction(call);
        let q = self.query();
        let callee = called_function(&q, &instruction);
        if let Some(id) = pattern::call_intrinsic(&q, call)
            && let Some(intrinsic) = range_intrinsic(id)
        {
            let result = instruction.result.expect("an intrinsic with a range has a value");
            // Compute result range for intrinsics supported by ConstantRange.
            // Do this even if we don't know a range for all operands, as we
            // may still know something about the result range, e.g. of abs(x).
            let operands: Vec<Operand> = call_arguments(&instruction).to_vec();
            let mut ranges = Vec::new();
            for operand in operands {
                let state = self.get_value_state(operand);
                if state.is_unknown_or_undef() {
                    return;
                }
                let width = self.context.types.int_bits(self.type_of(operand)).unwrap_or(1);
                ranges.push(state.as_constant_range(width, false));
            }
            let range = ConstantRange::intrinsic(intrinsic, &ranges);
            self.merge_in_value(result, &Lattice::get_range(range, false), MergeOptions::default());
            return;
        }
        // The common case is that we aren't tracking the callee, either
        // because we are not doing interprocedural analysis or the callee is
        // indirect, or is external. Handle these cases first.
        let callee_is_declaration = callee.is_none_or(|callee| callee.is_declaration());
        if callee_is_declaration {
            return self.handle_call_overdefined(call);
        }
        let slot = self.tracked_callee(call);
        let user = (self.active as u32, call);
        // If this is a single/zero retval case, see if we're tracking the
        // function.
        if let Some(fields) = self.struct_fields(instruction.ty) {
            let Some(slot) = slot.filter(|&slot| self.tracked[slot].multiple.is_some()) else {
                // Not tracking this callee.
                return self.handle_call_overdefined(call);
            };
            // If we are tracking this callee, propagate the result of the
            // function into this call site.
            let result = instruction.result.expect("a call returning a struct");
            self.tracked[slot].users.add(user);
            for index in 0..fields.len() as u32 {
                let tracked = self.tracked[slot].multiple.as_ref().expect("tracked returns")[index as usize].clone();
                self.merge_in_field(result, index, &tracked, max_widen_steps_opts());
            }
            return;
        }
        let Some(tracked) = slot.and_then(|slot| self.tracked[slot].ret.clone()) else {
            // Not tracking this callee.
            return self.handle_call_overdefined(call);
        };
        // If so, propagate the return value of the callee into this call
        // result.
        self.tracked[slot.expect("a tracked callee")].users.add(user);
        let result = instruction.result.expect("a call to a function returning a value");
        self.merge_in_value(result, &tracked, max_widen_steps_opts());
    }

    /// `handleCallOverdefined`.
    fn handle_call_overdefined(
        &mut self,
        call: InstId,
    ) {
        let instruction = self.function.instruction(call);
        // Void return and not tracking callee, just bail.
        let Some(result) = instruction.result else { return };
        // Always mark struct return as overdefined.
        if self.struct_fields(instruction.ty).is_some() {
            return self.mark_overdefined(result);
        }
        let globals = self.globals;
        let named = memory_callee(self.context, self.function, call).and_then(|global| globals.get(global.0 as usize));
        let Some((callee, name)) = named
            .and_then(|global| Some((global.function()?, global.name.owned().unwrap_or_default())))
            .filter(|(callee, _)| callee.is_declaration())
        else {
            let from_metadata = self.get_value_from_metadata(call);
            self.merge_in_value(result, &from_metadata, MergeOptions::default());
            return;
        };
        // Otherwise, if we have a single return value case, and if the
        // function is a declaration, maybe we can constant fold it.
        let info = call_info(&instruction).expect("a call");
        if constantfolding::can_constant_fold_call_to(info, callee, &name) {
            let mut operands = Vec::new();
            for &argument in call_arguments(&instruction) {
                if self.struct_fields(self.type_of(argument)).is_some() {
                    // Can't handle struct args.
                    return self.mark_overdefined(result);
                }
                let state = self.get_value_state(argument);
                if state.is_unknown_or_undef() {
                    return; // Operands are not resolved yet.
                }
                if is_overdefined(&state) {
                    return self.mark_overdefined(result);
                }
                operands.push(self.get_constant(&state, self.type_of(argument)).expect("a constant state"));
            }
            if is_overdefined(&self.get_value_state(Operand::Value(result))) {
                return self.mark_overdefined(result);
            }
            // If we can constant fold this, mark the result of the call as a
            // constant.
            if let Some(folded) =
                constantfolding::constant_fold_call(&*self.context, Some(self.function), info, callee, &name, &operands)
            {
                let lv = Lattice::get(&LatticeConstant::of(self.context, folded));
                self.merge_in_value(result, &lv, MergeOptions::default());
                return;
            }
        }
        // Fall back to metadata.
        let from_metadata = self.get_value_from_metadata(call);
        self.merge_in_value(result, &from_metadata, MergeOptions::default());
    }

    // ---- solving ---------------------------------------------------------

    /// `solve`: process the work lists until they are empty.
    pub fn solve(&mut self) {
        while !self.bb_work.is_empty() || !self.inst_work.is_empty() {
            // Process the instruction work list.
            while let Some((slot, inst)) = self.inst_work.pop() {
                self.enter(slot as usize);
                self.in_work.remove(&inst);
                self.visit(inst);
            }
            // Process the basic block work list.
            while let Some((slot, block)) = self.bb_work.pop() {
                self.enter(slot as usize);
                self.bb_visited.insert(block);
                let insts: Vec<InstId> = self.function.iter_block(block).collect();
                for inst in insts {
                    self.current = Some(inst);
                    self.visit(inst);
                }
                self.current = None;
            }
        }
    }

    /// `resolvedUndef`: what is still unknown when the lists are empty has an
    /// `undef` operand somewhere; it is overdefined.
    fn resolved_undef(
        &mut self,
        inst: InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        // Look for instructions which produce undef values.
        let Some(result) = instruction.result else { return false };
        let is_call = matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_));
        if let Some(fields) = self.struct_fields(instruction.ty) {
            // Only a few things that can be structs matter for undef. Tracked
            // calls must never be marked overdefined in resolvedUndefsIn.
            if is_call && self.tracked_callee(inst).is_some_and(|slot| self.tracked[slot].multiple.is_some()) {
                return false;
            }
            // extractvalue and insertvalue don't need to be marked; they are
            // tracked as precisely as their operands.
            if matches!(
                instruction.opcode,
                Opcode::ExtractValue(_) | Opcode::InsertValue(_)
            ) {
                return false;
            }
            // Send the results of everything else to overdefined. We could be
            // more precise than this but it isn't worth bothering.
            for index in 0..fields.len() as u32 {
                if self.struct_state_mut(result, index).is_unknown() {
                    self.mark_overdefined_field(result, index);
                    return true;
                }
            }
            return false;
        }
        if !self.state_mut(result).is_unknown() {
            return false;
        }
        // There are two reasons a call can have an undef result: it could be
        // tracked, or it could be constant-foldable. Because of the way we
        // solve return values, tracked calls must never be marked overdefined
        // in resolvedUndefsIn.
        if is_call && self.tracked_callee(inst).is_some_and(|slot| self.tracked[slot].ret.is_some()) {
            return false;
        }
        if matches!(instruction.opcode, Opcode::Load { .. }) {
            // A load here means one of two things: a load of undef from a
            // global, a load from an unknown pointer. Either way, having it
            // return undef is okay.
            return false;
        }
        self.mark_overdefined_value(result);
        true
    }

    /// `resolvedUndefsIn`: while solving, an operation with an undef operand
    /// gets no result, so that undef can be lowered to a constant later;
    /// whatever is still unknown now gets one.
    pub fn resolved_undefs_in(&mut self) -> bool {
        let back = self.active;
        let mut made_change = false;
        for slot in 0..self.tracked.len() {
            self.enter(slot);
            made_change |= self.resolved_undefs_in_function();
        }
        self.enter(back);
        made_change
    }

    /// `resolvedUndefsIn(F)` of the active function.
    fn resolved_undefs_in_function(&mut self) -> bool {
        let mut made_change = false;
        let layout: Vec<BlockId> = self.function.layout().to_vec();
        for block in layout {
            if !self.bb_executable.contains(&block) {
                continue;
            }
            let insts: Vec<InstId> = self.function.iter_block(block).collect();
            for inst in insts {
                made_change |= self.resolved_undef(inst);
            }
        }
        made_change
    }

    /// `solveWhileResolvedUndefsIn(M)`.
    pub fn solve_while_resolved_undefs_in(&mut self) {
        loop {
            self.solve();
            if !self.resolved_undefs_in() {
                break;
            }
        }
    }

    /// The tracked return value of the active function (`TrackedRetVals[F]`).
    pub fn tracked_return(&self) -> Option<&Lattice> {
        self.tracked[self.active].ret.as_ref()
    }

    /// `getConstantOrNull(V)`.
    pub fn get_constant_or_null(
        &self,
        value: ValueId,
    ) -> Option<ConstantId> {
        let ty = self.function.value(value).ty;
        if let Some(fields) = self.struct_fields(ty) {
            let lattices = self.get_struct_lattice_value_for(value, fields.len());
            if lattices.iter().any(is_overdefined) {
                return None;
            }
            let elements: Vec<ConstantId> = lattices
                .iter()
                .zip(&fields)
                .map(|(lv, &field)| {
                    if is_constant(lv) {
                        self.get_constant(lv, field).expect("a constant lattice value")
                    } else {
                        self.context.constant(Constant { ty: field, kind: ConstantKind::Undef })
                    }
                })
                .collect();
            return Some(self.context.constant(Constant { ty, kind: ConstantKind::Aggregate(elements) }));
        }
        let lv = self.get_lattice_value_for(value);
        if is_overdefined(&lv) {
            return None;
        }
        if is_constant(&lv) {
            self.get_constant(&lv, ty)
        } else {
            Some(self.context.constant(Constant { ty, kind: ConstantKind::Undef }))
        }
    }

    /// The context, handed back for the rewrites.
    pub fn context(&self) -> &Context {
        self.context
    }

    /// The results the rewrites read, without the borrow of the function: of
    /// the one function of [`Solver::new`].
    pub fn into_results(self) -> Results {
        self.into_module_results().functions.swap_remove(0).1
    }

    /// The results of each function given to [`Solver::module`], in that
    /// order, and `TrackedGlobals`.
    pub fn into_module_results(mut self) -> ModuleResults {
        let mut parked = std::mem::take(&mut self.parked);
        self.exchange(&mut parked[self.active]);
        let functions = parked
            .into_iter()
            .zip(self.tracked)
            .map(|(local, tracked)| {
                let results = Results {
                    bb_executable: local.bb_executable,
                    value_state: local.value_state,
                    struct_state: local.struct_state,
                    tracked_return: tracked.ret,
                    tracked_multiple: tracked.multiple,
                    arguments_tracked: tracked.arguments,
                    must_preserve_return: tracked.must_preserve_return,
                    feasible: local.feasible,
                };
                (tracked.id, results)
            })
            .collect();
        ModuleResults { functions, tracked_globals: self.tracked_globals }
    }
}

/// What a module solve found.
pub struct ModuleResults {
    pub functions: Vec<(Option<GlobalId>, Results)>,
    /// `getTrackedGlobals`.
    pub tracked_globals: HashMap<GlobalId, Lattice>,
}

/// What the solve found, for the rewrites.
pub struct Results {
    bb_executable: IdSet<BlockId>,
    value_state: IdMap<ValueId, Lattice>,
    struct_state: HashMap<(ValueId, u32), Lattice>,
    tracked_return: Option<Lattice>,
    tracked_multiple: Option<Vec<Lattice>>,
    arguments_tracked: bool,
    must_preserve_return: bool,
    feasible: HashSet<(BlockId, BlockId)>,
}

impl Results {
    pub fn is_block_executable(
        &self,
        block: BlockId,
    ) -> bool {
        self.bb_executable.contains(&block)
    }

    pub fn is_edge_feasible(
        &self,
        from: BlockId,
        to: BlockId,
    ) -> bool {
        self.feasible.contains(&(from, to))
    }

    /// `getLatticeValueFor`.
    pub fn lattice_value(
        &self,
        value: ValueId,
    ) -> Lattice {
        self.value_state.get(&value).cloned().unwrap_or_default()
    }

    /// `removeLatticeValueFor`.
    pub fn remove_lattice_value(
        &mut self,
        value: ValueId,
    ) {
        self.value_state.remove(&value);
    }

    pub fn tracked_return(&self) -> Option<&Lattice> {
        self.tracked_return.as_ref()
    }

    /// `TrackedMultipleRetVals` of the function, if in `MRVFunctionsTracked`.
    pub fn tracked_multiple(&self) -> Option<&[Lattice]> {
        self.tracked_multiple.as_deref()
    }

    /// `isStructLatticeConstant`: each tracked field return is a constant.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1169>
    pub fn is_struct_lattice_constant(&self) -> bool {
        self.tracked_multiple.as_ref().is_some_and(|fields| fields.iter().all(is_replaceable_constant))
    }

    /// `isArgumentTrackedFunction`.
    pub fn is_argument_tracked(&self) -> bool {
        self.arguments_tracked
    }

    /// `mustPreserveReturn`.
    pub fn must_preserve_return(&self) -> bool {
        self.must_preserve_return
    }

    /// `getConstantOrNull(V)`.
    pub fn constant_or_null(
        &self,
        context: &Context,
        function: &Function,
        value: ValueId,
    ) -> Option<ConstantId> {
        let ty = function.value(value).ty;
        if let Some(fields) = context.types.fields(ty) {
            let fields = fields.to_vec();
            let lattices: Vec<Lattice> = (0..fields.len() as u32)
                .map(|index| self.struct_state.get(&(value, index)).cloned().unwrap_or_default())
                .collect();
            if lattices.iter().any(is_overdefined) {
                return None;
            }
            let elements: Vec<ConstantId> = lattices
                .iter()
                .zip(&fields)
                .map(|(lv, &field)| {
                    if is_constant(lv) {
                        constant_of(context, lv, field).expect("a constant lattice value")
                    } else {
                        context.constant(Constant { ty: field, kind: ConstantKind::Undef })
                    }
                })
                .collect();
            return Some(context.constant(Constant { ty, kind: ConstantKind::Aggregate(elements) }));
        }
        let lv = self.lattice_value(value);
        if is_overdefined(&lv) {
            return None;
        }
        if is_constant(&lv) {
            constant_of(context, &lv, ty)
        } else {
            Some(context.constant(Constant { ty, kind: ConstantKind::Undef }))
        }
    }
}

/// `getConstant(LV, Ty)`.
fn constant_of(
    context: &Context,
    lv: &Lattice,
    ty: TypeId,
) -> Option<ConstantId> {
    if lv.is_constant() {
        return Some(match lv.get_constant() {
            LatticeConstant::Other { ty, kind } => context.constant(Constant { ty: *ty, kind: kind.clone() }),
            LatticeConstant::Int { bits, .. } => context.int(ty, *bits as i128),
            LatticeConstant::Undef => context.constant(Constant { ty, kind: ConstantKind::Undef }),
        });
    }
    if lv.is_constant_range(true)
        && let Some(single) = lv.get_constant_range(true).get_single_element()
    {
        return Some(context.int(ty, single as i128));
    }
    None
}

/// The bits `zeroinitializer` has as a constant of `ty`.
fn zero_kind(
    context: &Context,
    ty: TypeId,
) -> ConstantKind {
    match context.types.get(ty) {
        Type::Int(_) => ConstantKind::Int(0),
        Type::Pointer(_) => ConstantKind::Null,
        _ => ConstantKind::Zero,
    }
}

/// `getNoWrapKind()` of a `trunc`.
fn no_wrap_kind(flags: Flags) -> u32 {
    (if flags.contains(Flags::NUW) { NO_UNSIGNED_WRAP } else { 0 })
        | (if flags.contains(Flags::NSW) { NO_SIGNED_WRAP } else { 0 })
}

/// `WithOverflowInst::getBinaryOp` and `getNoWrapKind`.
fn with_overflow_op(id: IntrinsicId) -> (BinaryOp, u32) {
    match id {
        IntrinsicId::SaddWithOverflow => (BinaryOp::Add, NO_SIGNED_WRAP),
        IntrinsicId::UaddWithOverflow => (BinaryOp::Add, NO_UNSIGNED_WRAP),
        IntrinsicId::SsubWithOverflow => (BinaryOp::Sub, NO_SIGNED_WRAP),
        IntrinsicId::UsubWithOverflow => (BinaryOp::Sub, NO_UNSIGNED_WRAP),
        IntrinsicId::SmulWithOverflow => (BinaryOp::Mul, NO_SIGNED_WRAP),
        IntrinsicId::UmulWithOverflow => (BinaryOp::Mul, NO_UNSIGNED_WRAP),
        _ => unreachable!("not a with.overflow intrinsic"),
    }
}

/// `ConstantRange::isIntrinsicSupported`.
fn range_intrinsic(id: IntrinsicId) -> Option<RangeIntrinsic> {
    Some(match id {
        IntrinsicId::UaddSat => RangeIntrinsic::UAddSat,
        IntrinsicId::UsubSat => RangeIntrinsic::USubSat,
        IntrinsicId::SaddSat => RangeIntrinsic::SAddSat,
        IntrinsicId::SsubSat => RangeIntrinsic::SSubSat,
        IntrinsicId::Umin => RangeIntrinsic::UMin,
        IntrinsicId::Umax => RangeIntrinsic::UMax,
        IntrinsicId::Smin => RangeIntrinsic::SMin,
        IntrinsicId::Smax => RangeIntrinsic::SMax,
        IntrinsicId::Abs => RangeIntrinsic::Abs,
        IntrinsicId::Ctlz => RangeIntrinsic::Ctlz,
        IntrinsicId::Cttz => RangeIntrinsic::Cttz,
        IntrinsicId::Ctpop => RangeIntrinsic::Ctpop,
        _ => return None,
    })
}

/// `Argument::hasNonNullAttr()` (without the dereferenceable rule: it needs
/// the function's null-pointer-valid state, which `A->hasNonNullAttr()` reads
/// too).
fn param_has_non_null(
    function: &Function,
    index: u32,
) -> bool {
    function.parameter_attrs.get(index as usize).is_some_and(|attrs| Facts::of(attrs).non_null())
}

/// `Argument::hasByValAttr()`.
fn is_by_value(
    function: &Function,
    index: usize,
) -> bool {
    function
        .parameter_attrs
        .get(index)
        .is_some_and(|attrs| attrs.iter().any(|one| matches!(one, Attribute::Type(name, _) if name == "byval")))
}

/// What a call passes for one formal of a tracked function.
enum Incoming {
    Overdefined,
    Fields(Vec<Lattice>),
    Value(Lattice),
}

/// `CallBase::isReturnNonNull`.
fn call_returns_non_null(
    q: &Query,
    instruction: &llrm_mir::module::Instruction,
) -> bool {
    call_info(instruction).is_some_and(|info| Facts::of(&info.return_attrs).non_null())
        || called_function(q, instruction).is_some_and(|callee| Facts::of(&callee.return_attrs).non_null())
}

/// The function a call names.
fn memory_callee(
    context: &Context,
    function: &Function,
    call: InstId,
) -> Option<GlobalId> {
    llrm_mir::memory::callee(context, function, call)
}
