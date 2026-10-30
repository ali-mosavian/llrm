//! What a ValueTracking query is asked in: LLVM's `SimplifyQuery`. The module
//! (its globals name the intrinsics a call reaches), the function, the data
//! layout, and optionally the dominator tree, the assumptions and a context
//! instruction.
//!
//! # How it works
//! `Query` is `Copy` and read-only. A query without a dominator tree finds no
//! dominating condition, one without assumptions no `llvm.assume`: LLVM does
//! the same when `SQ.DT` or `SQ.AC` is null.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::hash::HashMap;
use crate::intrinsics::Intrinsic;
use crate::module::{Function, GlobalKind, GlobalValue, InstId, Instruction, MetadataNode, Module, Operand, ValueDef};
use crate::opcode::Opcode;
use crate::types::{Type, TypeId};

/// `SimplifyQuery`.
#[derive(Clone, Copy)]
pub struct Query<'a> {
    pub context: &'a Context,
    /// The module's globals, which name the callees; empty where the caller has
    /// no module.
    pub globals: &'a [GlobalValue],
    /// The module's metadata nodes, which `!range` and the like name; empty
    /// where the caller has no module.
    pub metadata: &'a [MetadataNode],
    pub function: &'a Function,
    pub layout: &'a DataLayout,
    pub dt: Option<&'a DomTree>,
    pub assumptions: Option<&'a AssumptionCache>,
    /// `DC`: the conditional branches each value is tested by.
    pub dom_conditions: Option<&'a DomConditionCache>,
    /// `CC`: a condition the query is asked under.
    pub cond_context: Option<&'a CondContext>,
    /// `CxtI`: the instruction the query is asked at.
    pub cxt: Option<InstId>,
}

impl<'a> Query<'a> {
    pub fn new(
        module: &'a Module,
        function: &'a Function,
        layout: &'a DataLayout,
    ) -> Self {
        Self {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    /// A query without a module: no callee has a name, so every call is
    /// unknown.
    pub fn bare(
        context: &'a Context,
        function: &'a Function,
        layout: &'a DataLayout,
    ) -> Self {
        Self {
            context,
            globals: &[],
            metadata: &[],
            function,
            layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    pub fn with_cxt(
        self,
        cxt: Option<InstId>,
    ) -> Self {
        Self { cxt, ..self }
    }

    /// `getWithCondContext`.
    pub fn with_cond_context(
        self,
        cc: &'a CondContext,
    ) -> Self {
        Self { cond_context: Some(cc), ..self }
    }

    /// `getWithoutCondContext`.
    pub fn without_cond_context(self) -> Self {
        Self { cond_context: None, ..self }
    }

    pub fn context(&self) -> &'a Context {
        self.context
    }

    /// The block `inst` is in.
    pub fn block_of(
        &self,
        inst: InstId,
    ) -> Option<crate::module::BlockId> {
        self.function.parent(inst)
    }

    /// Whether `a` is before `b` in their one block
    /// (`Instruction::comesBefore`).
    pub fn comes_before(
        &self,
        a: InstId,
        b: InstId,
    ) -> bool {
        self.block_of(a).is_some_and(|block| Some(block) == self.block_of(b)) && self.function.comes_before(a, b)
    }

    pub fn type_of(
        &self,
        operand: Operand,
    ) -> Option<TypeId> {
        self.function.operand_type(self.context(), operand)
    }

    /// `getBitWidth(Ty, DL)`: an integer's width or a pointer's, 0 for anything
    /// else.
    pub fn scalar_bits(
        &self,
        operand: Operand,
    ) -> u32 {
        match self.type_of(operand).map(|ty| self.context().types.get(ty)) {
            Some(Type::Int(bits)) => *bits,
            Some(Type::Pointer(space)) => self.layout.pointer(*space).bits,
            _ => 0,
        }
    }

    pub fn is_pointer(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            self.type_of(operand).map(|ty| self.context().types.get(ty)),
            Some(Type::Pointer(_))
        )
    }

    /// The constant integer `operand` is, as bits and width.
    pub fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        let Operand::Constant(id) = operand else { return None };
        let constant = self.context().get(id);
        match constant.kind {
            ConstantKind::Int(bits) => Some((bits, self.context().types.int_bits(constant.ty)?)),
            ConstantKind::Zero | ConstantKind::Null => Some((0, self.context().types.int_bits(constant.ty)?)),
            _ => None,
        }
    }

    /// The instruction that defines `operand`, if one does.
    pub fn instruction_of(
        &self,
        operand: Operand,
    ) -> Option<(InstId, &'a Instruction)> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        Some((inst, self.function.instruction(inst)))
    }

    /// `Instruction::getMetadata(Kind)`: the node `inst` carries under `kind`.
    pub fn metadata_node(
        &self,
        inst: InstId,
        kind: &str,
    ) -> Option<&'a MetadataNode> {
        let (_, node) = self.function.instruction(inst).metadata.iter().find(|(one, _)| one == kind)?;
        self.metadata.get(node.0 as usize)
    }

    /// The name of the function a call's callee names.
    pub fn callee_name(
        &self,
        inst: InstId,
    ) -> Option<&'a str> {
        self.callee_function(inst)?.name.as_deref()
    }

    /// The intrinsic of `pattern::IntrinsicId` a call reaches, from the name's
    /// cached id.
    pub(crate) fn callee_intrinsic_id(
        &self,
        inst: InstId,
    ) -> Option<crate::valuetracking::pattern::IntrinsicId> {
        self.callee_function(inst)?.name.intrinsic_id()
    }

    /// The function global a call's callee names.
    fn callee_function(
        &self,
        inst: InstId,
    ) -> Option<&'a GlobalValue> {
        let instruction = self.function.instruction(inst);
        let (Opcode::Call(_) | Opcode::Invoke(_)) = instruction.opcode else { return None };
        let Operand::Constant(id) = *instruction.operands.last()? else { return None };
        let ConstantKind::Global(global) = self.context().get(id).kind else { return None };
        let global = self.globals.get(global.0 as usize)?;
        matches!(global.kind, GlobalKind::Function(_)).then_some(global)
    }

    /// The intrinsic a call reaches, when MIR has it.
    pub fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<Intrinsic> {
        self.callee_function(inst)?.intrinsic()
    }

    /// Whether `a` is the same value as `b`.
    pub fn same(
        a: Operand,
        b: Operand,
    ) -> bool {
        a == b
    }
}

/// `CondContext`: evaluate a query assuming `cond` holds (`invert`: does not).
#[derive(Clone, Debug)]
pub struct CondContext {
    pub cond: Operand,
    pub invert: bool,
    pub affected_values: crate::hash::HashSet<Operand>,
}

impl CondContext {
    pub fn new(cond: Operand) -> Self {
        Self { cond, invert: false, affected_values: Default::default() }
    }
}

/// `MaxAssumesPerValue`.
const MAX_ASSUMES_PER_VALUE: usize = 1024;

/// `AssumptionCache`: the `llvm.assume` calls of a function, and the values
/// each one tells of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssumptionCache {
    pub assumes: Vec<InstId>,
    affected: HashMap<Operand, Vec<InstId>>,
}

#[cfg(test)]
thread_local! {
    /// The bodies `AssumptionCache::of` did not walk for assumes.
    pub(crate) static SKIPPED_WALKS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `LLRM_CHECK_ASSUME_SKIP` walks the body even where no `llvm.assume` is
/// declared, and fails on an assume found.
fn check_assume_skip() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_ASSUME_SKIP"))
}

impl AssumptionCache {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AssumptionCache.cpp#L261>
    pub fn new(
        module: &Module,
        function: &Function,
    ) -> Self {
        Self::of(&module.context, &module.globals, &module.metadata, function)
    }

    /// `AssumptionCache(F)` from the parts of a module a pass holds.
    pub fn of(
        context: &Context,
        globals: &[GlobalValue],
        metadata: &[MetadataNode],
        function: &Function,
    ) -> Self {
        let layout = DataLayout::default();
        let query = Query { globals, metadata, ..Query::bare(context, function, &layout) };
        // A call reaches `llvm.assume` only through a function of that name:
        // with none declared there is nothing to find in the body.
        let declared = globals.iter().any(|global| {
            matches!(global.kind, GlobalKind::Function(_)) && global.intrinsic() == Some(Intrinsic::Assume)
        });
        let check = !declared && check_assume_skip();
        let assumes: Vec<InstId> = if declared || check {
            function
                .walk()
                .filter(|(_, inst)| query.intrinsic(*inst) == Some(Intrinsic::Assume))
                .map(|(_, inst)| inst)
                .collect()
        } else {
            #[cfg(test)]
            SKIPPED_WALKS.with(|walks| walks.set(walks.get() + 1));
            Vec::new()
        };
        assert!(!check || assumes.is_empty(), "an assume was found where none is declared");
        let mut cache = Self { assumes, affected: HashMap::default() };
        for assume in cache.assumes.clone() {
            cache.update_affected_values(&query, assume);
        }
        cache
    }

    /// `assumptionsFor(V)`: the assumes that tell of `value`, in the order they
    /// were found. MIR has no operand bundles, so each is a plain condition.
    pub fn assumptions_for(
        &self,
        value: Operand,
    ) -> &[InstId] {
        self.affected.get(&value).map_or(&[], Vec::as_slice)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/AssumptionCache.cpp#L99>
    fn update_affected_values(
        &mut self,
        q: &Query,
        assume: InstId,
    ) {
        let mut affected = Vec::new();
        let condition = q.function.instruction(assume).operands[0];
        super::context::find_values_affected_by_condition(q, condition, true, &mut |v| affected.push(v));
        for value in affected {
            let list = self.affected.entry(value).or_default();
            if list.len() >= MAX_ASSUMES_PER_VALUE {
                continue;
            }
            if !list.contains(&assume) {
                list.push(assume);
            }
        }
    }
}

/// `DomConditionCache`: for each value, the conditional branches whose
/// condition tells of it.
#[derive(Clone, Debug, Default)]
pub struct DomConditionCache {
    /// Behind a cell because InstCombine registers a branch as it visits it,
    /// while its queries hold the cache.
    affected: std::cell::RefCell<HashMap<Operand, Vec<InstId>>>,
}

impl DomConditionCache {
    /// `registerBranch` of every conditional branch of `function`.
    pub fn new(
        module: &Module,
        function: &Function,
    ) -> Self {
        let layout = DataLayout::default();
        Self::registered(&Query::new(module, function, &layout), function)
    }

    /// [`DomConditionCache::new`] from the parts of a module a pass holds.
    pub fn of(
        context: &Context,
        globals: &[GlobalValue],
        metadata: &[MetadataNode],
        layout: &DataLayout,
        function: &Function,
    ) -> Self {
        let query = Query { globals, metadata, ..Query::bare(context, function, layout) };
        Self::registered(&query, function)
    }

    fn registered(
        query: &Query,
        function: &Function,
    ) -> Self {
        let cache = Self::default();
        for &block in function.layout() {
            if let Some(branch) = function.terminator(block)
                && function.instruction(branch).opcode == Opcode::Br
                && function.instruction(branch).operands.len() == 3
            {
                cache.register_branch(query, branch);
            }
        }
        cache
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DomConditionCache.cpp#L19>
    pub fn register_branch(
        &self,
        q: &Query,
        branch: InstId,
    ) {
        let mut affected = Vec::new();
        let condition = q.function.instruction(branch).operands[0];
        super::context::find_values_affected_by_condition(q, condition, false, &mut |v| affected.push(v));
        let mut map = self.affected.borrow_mut();
        for value in affected {
            let list = map.entry(value).or_default();
            if !list.contains(&branch) {
                list.push(branch);
            }
        }
    }

    /// `conditionsFor(V)`.
    pub fn conditions_for(
        &self,
        value: Operand,
    ) -> std::cell::Ref<'_, [InstId]> {
        std::cell::Ref::map(self.affected.borrow(), |map| map.get(&value).map_or(&[][..], Vec::as_slice))
    }
}

/// What a pass builds once per function so that its queries find the
/// assumptions and the dominating conditions of LLVM's `SimplifyQuery{DL, DT,
/// AC}`.
pub struct FunctionAnalyses {
    layout: DataLayout,
    dt: DomTree,
    assumptions: AssumptionCache,
    dom_conditions: Option<DomConditionCache>,
}

impl FunctionAnalyses {
    pub fn new(
        module: &Module,
        function: &Function,
    ) -> Self {
        let layout = module.datalayout.as_deref().and_then(|text| DataLayout::parse(text).ok()).unwrap_or_default();
        Self {
            layout,
            dt: DomTree::of(function),
            assumptions: AssumptionCache::new(module, function),
            dom_conditions: None,
        }
    }

    /// The same from the parts of a module a pass holds, not the module.
    pub fn of(
        context: &Context,
        globals: &[GlobalValue],
        metadata: &[MetadataNode],
        layout: &DataLayout,
        function: &Function,
    ) -> Self {
        Self {
            layout: layout.clone(),
            dt: DomTree::of(function),
            assumptions: AssumptionCache::of(context, globals, metadata, function),
            dom_conditions: None,
        }
    }

    /// The same with the branch conditions that tell of a value (`SQ.DC`, the
    /// `DomConditionCache` InstCombine keeps): a query at an instruction then
    /// reads what the branches dominating it say.
    pub fn with_dom_conditions(
        mut self,
        module: &Module,
        function: &Function,
    ) -> Self {
        self.dom_conditions = Some(DomConditionCache::new(module, function));
        self
    }

    /// The same with an empty cache that the pass fills as it visits each
    /// conditional branch (`DC.registerBranch` in `visitBranchInst`): a query
    /// sees the branches visited before it, as LLVM's does.
    pub fn with_lazy_dom_conditions(mut self) -> Self {
        self.dom_conditions = Some(DomConditionCache::default());
        self
    }

    /// [`FunctionAnalyses::with_dom_conditions`] from the parts of a module.
    pub fn with_dom_conditions_of(
        mut self,
        context: &Context,
        globals: &[GlobalValue],
        metadata: &[MetadataNode],
        function: &Function,
    ) -> Self {
        self.dom_conditions = Some(DomConditionCache::of(context, globals, metadata, &self.layout, function));
        self
    }

    /// The dominating-condition cache (`SQ.DC`), where asked for.
    pub fn dom_conditions(&self) -> Option<&DomConditionCache> {
        self.dom_conditions.as_ref()
    }

    /// The dominator tree (`SQ.DT`).
    pub fn dominator_tree(&self) -> &DomTree {
        &self.dt
    }

    /// The assumptions (`SQ.AC`).
    pub fn assumption_cache(&self) -> &AssumptionCache {
        &self.assumptions
    }

    /// A query at `cxt` (`CxtI`), or at no instruction.
    pub fn query<'a>(
        &'a self,
        module: &'a Module,
        function: &'a Function,
        cxt: Option<InstId>,
    ) -> Query<'a> {
        Query {
            dt: Some(&self.dt),
            assumptions: Some(&self.assumptions),
            dom_conditions: self.dom_conditions.as_ref(),
            ..Query::new(module, function, &self.layout).with_cxt(cxt)
        }
    }
}
