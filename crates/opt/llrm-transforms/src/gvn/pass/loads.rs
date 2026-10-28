//! The load half of `GVNPass`: `processLoad`, `processNonLocalLoad`, the
//! availability of a load's value in the blocks that reach it, and the SSA
//! construction that merges them.
//!
//! A translation of
//! [`GVN.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp)
//! at d1106deb71cc, with `MemoryDependenceAnalysis` as the engine
//! (`enable-gvn-memdep` on, `enable-gvn-memoryssa` off, the defaults).
//!
//! DEVIATE 5, `getInitialValueOfAllocation` needs `TargetLibraryInfo`
//! (`malloc`, `calloc`); MIR has none, so a load from a heap allocation's fresh
//! memory is not given its initial value. DEVIATE 6, MIR has no `undef`: a load
//! from an `alloca` or past a `lifetime.start` is `poison`, which refines it
//! (`UndefValue::get` in the C++). DEVIATE 7, remarks
//! (`reportMayClobberedLoad`, `reportLoadElim`) are dropped; there is no remark
//! stream. `ImplicitControlFlowTracking` is not kept (`removeUsersOf` has
//! nothing to update); load PRE, the reader of it, is stage 5.
//! DEVIATE 19, `takeName` is not carried to a phi made for a load: names are
//! cosmetic.
//!
//! OWED, stage 5 and 6 of docs/architecture/gvn-port.md: a load with
//! unavailable blocks is not partially eliminated (`performLoopLoadPRE`,
//! `performLoadPRE`), and `performScalarPRE` of a load's GEP indices is not
//! called.

use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::memdep::{Env as MdEnv, MemDepResult, NonLocalDepResult, SelectAddr};
use llrm_analysis::memory::Unit;
use llrm_analysis::ssaupdater::SsaUpdater;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};

use super::exposing;
use super::{GvnPass, local};
use crate::vncoercion::{self, Inserter, MemIntrinsic};

/// `gvn-max-num-deps`.
const MAX_NUM_DEPS: usize = 100;
/// `gvn-max-num-visited-insts`.
const MAX_NUM_VISITED_INSTS: u32 = 100;

/// What an [`AvailableValue`] holds (`AvailableValue::ValType` with its `Val`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    /// A simple offsetted value that is accessed.
    Simple(Operand),
    /// A value produced by a load.
    Load(InstId),
    /// A memory intrinsic which is loaded from.
    MemIntrin(MemIntrinsic),
    /// A value from a dead block, not yet removed from the CFG.
    Undef,
    /// A pointer select which is loaded from, and for which the load can be
    /// replaced by a value select.
    Select { cond: Operand, v1: Operand, v2: Operand },
}

/// [`AvailableValue`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L204):
/// a value we know how to materialize, which never fails.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AvailableValue {
    pub kind: Kind,
    /// The byte offset in the value that is interesting for the load.
    pub offset: u64,
}

impl AvailableValue {
    pub(super) fn get(
        value: Operand,
        offset: u64,
    ) -> Self {
        Self { kind: Kind::Simple(value), offset }
    }

    fn get_mi(
        mi: MemIntrinsic,
        offset: u64,
    ) -> Self {
        Self { kind: Kind::MemIntrin(mi), offset }
    }

    fn get_load(
        load: InstId,
        offset: u64,
    ) -> Self {
        Self { kind: Kind::Load(load), offset }
    }

    fn get_undef() -> Self {
        Self { kind: Kind::Undef, offset: 0 }
    }

    fn get_select(
        cond: Operand,
        v1: Operand,
        v2: Operand,
    ) -> Self {
        Self { kind: Kind::Select { cond, v1, v2 }, offset: 0 }
    }
}

/// [`AvailableValueInBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L300):
/// an available value which can be rematerialized at the end of its block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AvailableValueInBlock {
    pub bb: BlockId,
    pub av: AvailableValue,
}

/// `GVNPass::DepKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DepKind {
    /// Unknown value.
    Other,
    /// Exactly overlapping locations.
    Def,
    /// Reaching value is a superset of the needed bits.
    Clobber,
    /// Reaching value is a select of two reaching addresses.
    Select,
}

/// [`ReachingMemVal`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Scalar/GVN.h#L371):
/// a memory location's value, such that some path to a point leaves it
/// unmodified.
#[derive(Clone, Copy, Debug)]
pub(super) struct ReachingMemVal {
    pub kind: DepKind,
    pub block: Option<BlockId>,
    pub addr: Option<Operand>,
    pub inst: Option<InstId>,
    pub offset: i32,
    /// For `Select` only: the condition and the addresses of the "true" and
    /// "false" side.
    pub select: Option<(Operand, Operand, Operand)>,
}

impl ReachingMemVal {
    fn get_unknown(
        block: Option<BlockId>,
        addr: Option<Operand>,
        inst: Option<InstId>,
    ) -> Self {
        Self { kind: DepKind::Other, block, addr, inst, offset: -1, select: None }
    }

    fn get_def(
        function: &llrm_mir::module::Function,
        addr: Option<Operand>,
        inst: InstId,
    ) -> Self {
        Self { kind: DepKind::Def, block: function.parent(inst), addr, inst: Some(inst), offset: -1, select: None }
    }

    fn get_clobber(
        function: &llrm_mir::module::Function,
        addr: Option<Operand>,
        inst: InstId,
    ) -> Self {
        Self { kind: DepKind::Clobber, block: function.parent(inst), addr, inst: Some(inst), offset: -1, select: None }
    }

    fn get_select(
        block: BlockId,
        cond: Operand,
        true_addr: Operand,
        false_addr: Operand,
    ) -> Self {
        Self {
            kind: DepKind::Select,
            block: Some(block),
            addr: None,
            inst: None,
            offset: -1,
            select: Some((cond, true_addr, false_addr)),
        }
    }
}

/// The metadata `AvailableValue::MaterializeAdjustedValue` keeps on a load it
/// coerced from (`dropUnknownNonDebugMetadata`'s known ids).
const KEPT_METADATA: [&str; 6] =
    ["dereferenceable", "dereferenceable_or_null", "invariant.load", "invariant.group", "alias.scope", "noalias"];

impl GvnPass<'_> {
    /// `MemoryDependenceResults::getDependency` for `inst`, on the function as
    /// it stands.
    pub(super) fn md_get_dependency(
        &mut self,
        inst: InstId,
    ) -> MemDepResult {
        let unit = exposing(Unit::within(&*self.context, self.layout, &*self.function, self.outer), self.exposed);
        let mut aa = BasicAA::new(&unit);
        self.md.get_dependency(MdEnv { unit: &unit, dt: &self.dt }, &mut aa, inst)
    }

    /// `invalidateCachedPointerInfo` for `value`, if it is a pointer.
    pub(super) fn md_invalidate_cached_pointer_info(
        &mut self,
        value: Operand,
    ) {
        let unit = exposing(Unit::within(&*self.context, self.layout, &*self.function, self.outer), self.exposed);
        self.md.invalidate_cached_pointer_info(&unit, value);
    }

    /// `isLifetimeStart`
    /// ([GVN.cpp:1214](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1214)).
    fn is_lifetime_start(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(self.function.instruction(inst).opcode, Opcode::Call(_))
            && exposing(Unit::within(&*self.context, self.layout, &*self.function, self.outer), self.exposed)
                .intrinsic(inst)
                == Some(Intrinsic::LifetimeStart)
    }

    /// The memory intrinsic `inst` is, if it is a `memset`, `memcpy` or
    /// `memmove`.
    fn mem_intrinsic(
        &self,
        inst: InstId,
    ) -> Option<MemIntrinsic> {
        MemIntrinsic::of_in(&self.context, &self.outer.globals, self.function, inst)
    }

    /// `AvailableValue::MaterializeAdjustedValue`
    /// ([GVN.cpp:1148](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1148)):
    /// code before `insert_pt` that gives the value `av` defines, adjusted to
    /// the type of `load`.
    pub(super) fn materialize_adjusted_value(
        &mut self,
        av: &AvailableValue,
        load: InstId,
        insert_pt: InstId,
    ) -> Operand {
        let load_ty = self.function.instruction(load).ty;
        let result = match av.kind {
            Kind::Simple(value) => {
                if self.type_of(value) != load_ty {
                    self.value_for_load(value, av.offset, load_ty, insert_pt)
                } else {
                    value
                }
            }
            Kind::Load(coerced) => {
                let coerced_value = Operand::Value(self.function.instruction(coerced).result.expect("a load"));
                if self.function.instruction(coerced).ty == load_ty && av.offset == 0 {
                    local::combine_metadata_for_cse(&self.outer.metadata, self.function, coerced, load, false);
                    coerced_value
                } else {
                    let res = self.value_for_load(coerced_value, av.offset, load_ty, insert_pt);
                    let has_noundef =
                        self.function.instruction(coerced).metadata.iter().any(|(kind, _)| kind == "noundef");
                    if !has_noundef {
                        let dropped: Vec<String> = self
                            .function
                            .instruction(coerced)
                            .metadata
                            .iter()
                            .filter(|(kind, _)| !KEPT_METADATA.contains(&kind.as_str()))
                            .map(|(kind, _)| kind.clone())
                            .collect();
                        for kind in dropped {
                            self.function.unannotate(coerced, &kind);
                        }
                    }
                    res
                }
            }
            Kind::MemIntrin(mi) => {
                let mut builder = Inserter {
                    context: &mut *self.context,
                    function: &mut *self.function,
                    layout: self.layout,
                    before: insert_pt,
                };
                vncoercion::get_mem_inst_value_for_load(&mut builder, mi, load_ty)
                    .expect("analyzeLoadFromClobberingMemInst accepted it")
            }
            Kind::Select { cond, v1, v2 } => {
                let select = self
                    .function
                    .create_instruction(Opcode::Select, load_ty, vec![cond, v1, v2], Flags::default(), None);
                self.function.insert(select, Position::Before(insert_pt)).expect("a placed insertion point");
                Operand::Value(self.function.instruction(select).result.expect("a select"))
            }
            Kind::Undef => unreachable!("Should not materialize value from dead block"),
        };
        result
    }

    /// `getValueForLoad` at `before`.
    fn value_for_load(
        &mut self,
        value: Operand,
        offset: u64,
        load_ty: llrm_mir::types::TypeId,
        before: InstId,
    ) -> Operand {
        let mut builder =
            Inserter { context: &mut *self.context, function: &mut *self.function, layout: self.layout, before };
        vncoercion::get_value_for_load(&mut builder, value, offset, load_ty)
    }

    /// `constructSSAForLoadSet`
    /// ([GVN.cpp:1120](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1120)):
    /// the value of `load` given its available values, with phis where they
    /// meet.
    pub(super) fn construct_ssa_for_load_set(
        &mut self,
        load: InstId,
        values_per_block: &[AvailableValueInBlock],
    ) -> Operand {
        let load_block = self.function.parent(load).expect("a placed load");
        // Check for the fully redundant, dominating load case: no phi is
        // needed.
        if let [only] = values_per_block
            && self.dt.properly_dominates(only.bb, load_block)
        {
            assert!(only.av.kind != Kind::Undef, "Dead BB dominate this block");
            let terminator = self.function.terminator(only.bb).expect("a terminated block");
            return self.materialize_adjusted_value(&only.av, load, terminator);
        }
        // Otherwise, we have to construct SSA form.
        let load_ty = self.function.instruction(load).ty;
        let name = self.function.instruction(load).result.and_then(|value| self.function.value(value).name.clone());
        let load_value = self.function.instruction(load).result.map(Operand::Value);
        let mut updater = SsaUpdater::new(load_ty, name.as_deref());
        for one in values_per_block {
            let bb = one.bb;
            // If the value isn't defined in a dead block, skip it.
            if one.av.kind == Kind::Undef {
                continue;
            }
            // If the block already has a value, skip it.
            if updater.has_value_for_block(bb) {
                continue;
            }
            // The load itself as its block's value is no value for the load.
            if bb == load_block {
                let is_itself = match one.av.kind {
                    Kind::Simple(value) => Some(value) == load_value,
                    Kind::Load(coerced) => coerced == load,
                    _ => false,
                };
                if is_itself {
                    continue;
                }
            }
            let terminator = self.function.terminator(bb).expect("a terminated block");
            let value = self.materialize_adjusted_value(&one.av, load, terminator);
            updater.add_available_value(bb, value);
        }
        updater.get_value_in_middle_of_block(self.context, self.layout, self.function, load_block)
    }

    /// `findDominatingValue`
    /// ([GVN.cpp:1308](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1308)):
    /// a store or load of the location, of the load's type, that dominates
    /// `from` along single predecessors with nothing modifying it between.
    fn find_dominating_value(
        &self,
        loc: &llrm_analysis::aa::MemoryLocation,
        load_ty: llrm_mir::types::TypeId,
        from: InstId,
    ) -> Option<Operand> {
        let function = &*self.function;
        let unit = exposing(Unit::within(&*self.context, self.layout, function, self.outer), self.exposed);
        let mut aa = BasicAA::new(&unit);
        let mut batch = llrm_analysis::aa::BatchAAResults::new(&mut aa);
        let mut visited = 0u32;
        let from_bb = function.parent(from).expect("a placed instruction");
        let mut bb = Some(from_bb);
        while let Some(block) = bb {
            let mut inst = if block == from_bb { Some(from) } else { function.terminator(block) };
            while let Some(current) = inst {
                visited += 1;
                if visited > MAX_NUM_VISITED_INSTS {
                    return None;
                }
                if batch.get_mod_ref_info(current, Some(loc)).is_mod_set() {
                    let instruction = function.instruction(current);
                    if matches!(instruction.opcode, Opcode::Store { volatile: false, .. })
                        && instruction.operands[1] == loc.ptr
                        && function.operand_type(&self.context, instruction.operands[0]) == Some(load_ty)
                    {
                        return Some(instruction.operands[0]);
                    }
                    return None;
                }
                let instruction = function.instruction(current);
                if matches!(instruction.opcode, Opcode::Load { .. })
                    && instruction.operands[0] == loc.ptr
                    && instruction.ty == load_ty
                {
                    return instruction.result.map(Operand::Value);
                }
                inst = function.prev_in_block(current);
            }
            bb = crate::edges::single_predecessor(function, block);
        }
        None
    }

    /// `analyzeSelectAvailability`
    /// ([GVN.cpp:1334](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1334)).
    fn analyze_select_availability(
        &self,
        load: InstId,
        cond: Operand,
        true_addr: Operand,
        false_addr: Operand,
        from: InstId,
    ) -> Option<AvailableValue> {
        let unit = exposing(Unit::within(&*self.context, self.layout, &*self.function, self.outer), self.exposed);
        let loc = llrm_analysis::aa::MemoryLocation::get(&unit, load).expect("a load");
        let load_ty = self.function.instruction(load).ty;
        let v1 = self.find_dominating_value(&loc.with_new_ptr(true_addr), load_ty, from)?;
        let v2 = self.find_dominating_value(&loc.with_new_ptr(false_addr), load_ty, from)?;
        Some(AvailableValue::get_select(cond, v1, v2))
    }

    /// `analyzeLoadAvailability`, for one dependence
    /// ([GVN.cpp:1357](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1357)).
    pub(super) fn analyze_load_availability_one(
        &mut self,
        load: InstId,
        dep: &ReachingMemVal,
        address: Option<Operand>,
    ) -> Option<AvailableValue> {
        let dep_inst = dep.inst.expect("a local dependence");
        let load_ty = self.function.instruction(load).ty;
        // A load from fresh stack memory, or past `lifetime.start`, reads
        // nothing defined (DEVIATE 6).
        if dep.kind == DepKind::Def
            && (matches!(
                self.function.instruction(dep_inst).opcode,
                Opcode::Alloca { .. }
            )
                || self.is_lifetime_start(dep_inst))
        {
            let poison = self.context.constant(Constant { ty: load_ty, kind: ConstantKind::Poison });
            return Some(AvailableValue::get(Operand::Constant(poison), 0));
        }
        let function = &*self.function;
        let (context, layout) = (&*self.context, self.layout);
        let dep_instruction = function.instruction(dep_inst);
        if dep.kind == DepKind::Clobber {
            match &dep_instruction.opcode {
                Opcode::Store { .. } => {
                    if let Some(address) = address
                        && let Some(offset) = vncoercion::analyze_load_from_clobbering_store(
                            context, layout, function, load_ty, address, dep_inst,
                        )
                    {
                        return Some(AvailableValue::get(dep_instruction.operands[0], offset));
                    }
                }
                Opcode::Load { .. } => {
                    if dep_inst != load
                        && let Some(address) = address
                    {
                        let dep_value = Operand::Value(dep_instruction.result.expect("a load"));
                        let mut offset: i64 = i64::from(dep.offset);
                        if vncoercion::can_coerce_must_aliased_value_to_load(
                            context, layout, function, dep_value, load_ty,
                        ) {
                            offset = match self.md.clobber_offset(dep_inst) {
                                Some(clobber) if clobber >= 0 => i64::from(clobber),
                                _ => -1,
                            };
                        }
                        if offset == -1 {
                            offset = vncoercion::analyze_load_from_clobbering_load(
                                context, layout, function, load_ty, address, dep_inst,
                            )
                            .map_or(-1, |one| one as i64);
                        }
                        if offset != -1 {
                            return Some(AvailableValue::get_load(dep_inst, offset as u64));
                        }
                    }
                }
                _ => {}
            }
            if let Some(mi) = self.mem_intrinsic(dep_inst)
                && let Some(address) = address
                && let Some(offset) =
                    vncoercion::analyze_load_from_clobbering_mem_inst(context, layout, function, load_ty, address, mi)
            {
                return Some(AvailableValue::get_mi(mi, offset));
            }
            return None;
        }
        assert!(dep.kind == DepKind::Def, "follows from above");
        match &dep_instruction.opcode {
            Opcode::Store { .. } => {
                let stored = dep_instruction.operands[0];
                if !vncoercion::can_coerce_must_aliased_value_to_load(context, layout, function, stored, load_ty) {
                    return None;
                }
                Some(AvailableValue::get(stored, 0))
            }
            Opcode::Load { .. } => {
                let value = Operand::Value(dep_instruction.result.expect("a load"));
                if !vncoercion::can_coerce_must_aliased_value_to_load(context, layout, function, value, load_ty) {
                    return None;
                }
                Some(AvailableValue::get_load(dep_inst, 0))
            }
            Opcode::Select => {
                let [cond, on_true, on_false] = dep_instruction.operands[..] else { return None };
                self.analyze_select_availability(load, cond, on_true, on_false, dep_inst)
            }
            _ => None,
        }
    }

    /// `analyzeLoadAvailability`, for all the dependences of `load`
    /// ([GVN.cpp:1499](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L1499)):
    /// the available value in each block that has one, and the blocks that have
    /// not.
    pub(super) fn analyze_load_availability(
        &mut self,
        load: InstId,
        deps: &[ReachingMemVal],
        values_per_block: &mut Vec<AvailableValueInBlock>,
        unavailable_blocks: &mut Vec<BlockId>,
    ) {
        for dep in deps {
            let dep_bb = dep.block.expect("a block for each dependence");
            if self.dead_blocks.contains(&dep_bb) {
                values_per_block.push(AvailableValueInBlock { bb: dep_bb, av: AvailableValue::get_undef() });
                continue;
            }
            if dep.kind == DepKind::Other {
                unavailable_blocks.push(dep_bb);
                continue;
            }
            if dep.kind == DepKind::Select {
                let (cond, true_addr, false_addr) = dep.select.expect("a select dependence");
                let terminator = self.function.terminator(dep_bb).expect("a terminated block");
                match self.analyze_select_availability(load, cond, true_addr, false_addr, terminator) {
                    Some(av) => values_per_block.push(AvailableValueInBlock { bb: dep_bb, av }),
                    None => unavailable_blocks.push(dep_bb),
                }
                continue;
            }
            match self.analyze_load_availability_one(load, dep, dep.addr) {
                Some(av) => values_per_block.push(AvailableValueInBlock { bb: dep_bb, av }),
                None => unavailable_blocks.push(dep_bb),
            }
        }
        assert!(deps.len() == values_per_block.len() + unavailable_blocks.len(), "post condition violation");
    }

    /// `processLoad`
    /// ([GVN.cpp:2804](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2804)):
    /// eliminate a load locally, else non-locally.
    pub(super) fn process_load(
        &mut self,
        load: InstId,
    ) -> bool {
        // This code hasn't been audited for ordered or volatile memory access.
        if matches!(
            self.function.instruction(load).opcode,
            Opcode::Load { volatile: true, .. }
        ) {
            return false;
        }
        let result = self.function.instruction(load).result.expect("a load has a value");
        if self.function.users(result).is_empty() {
            self.salvage_and_remove_instruction(load);
            return true;
        }
        let pointer = self.function.instruction(load).operands[0];
        // The load may depend on a store or a load of the same pointer.
        let dep = self.md_get_dependency(load);
        // If it is defined in another block, try harder.
        if dep.is_non_local() {
            return self.process_non_local_load(load);
        }
        // Only handle the local case below.
        let mem_val = match dep {
            MemDepResult::Def(inst) => ReachingMemVal::get_def(self.function, Some(pointer), inst),
            MemDepResult::Clobber(inst) => ReachingMemVal::get_clobber(self.function, Some(pointer), inst),
            _ => ReachingMemVal::get_unknown(None, None, None),
        };
        if mem_val.kind == DepKind::Other {
            // This might be a NonFuncLocal or an Unknown.
            return false;
        }
        let Some(av) = self.analyze_load_availability_one(load, &mem_val, Some(pointer)) else { return false };
        let available = self.materialize_adjusted_value(&av, load, load);
        // `materialize_adjusted_value` is responsible for combining metadata.
        self.function.replace_all_uses_with(result, available);
        self.salvage_and_remove_instruction(load);
        // Tell MemDep to reexamine the reused pointer since we might have more
        // information after forwarding it.
        self.md_invalidate_cached_pointer_info(available);
        true
    }

    /// `processNonLocalLoad`
    /// ([GVN.cpp:2059](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2059)):
    /// the dependences MemDep finds in the blocks that reach `load`.
    fn process_non_local_load(
        &mut self,
        load: InstId,
    ) -> bool {
        let mut deps: Vec<NonLocalDepResult> = Vec::new();
        {
            let unit = exposing(Unit::within(&*self.context, self.layout, &*self.function, self.outer), self.exposed);
            let mut aa = BasicAA::new(&unit);
            self.md.get_non_local_pointer_dependency(MdEnv { unit: &unit, dt: &self.dt }, &mut aa, load, &mut deps);
        }
        // If we had to process more than one hundred blocks to find the
        // dependencies, this load isn't worth worrying about.
        if deps.len() > MAX_NUM_DEPS {
            return false;
        }
        let mem_vals: Vec<ReachingMemVal> = deps
            .iter()
            .map(|dep| {
                let function = &*self.function;
                if dep.result == MemDepResult::Select {
                    let SelectAddr::Select { cond, addrs } = dep.address else {
                        unreachable!("a select has addresses")
                    };
                    return ReachingMemVal::get_select(dep.bb, cond, addrs.0, addrs.1);
                }
                let SelectAddr::Addr(address) = dep.address else { unreachable!("an address") };
                match dep.result {
                    MemDepResult::Clobber(inst) => ReachingMemVal::get_clobber(function, address, inst),
                    MemDepResult::Def(inst) => ReachingMemVal::get_def(function, address, inst),
                    other => ReachingMemVal::get_unknown(Some(dep.bb), address, other.inst()),
                }
            })
            .collect();
        self.process_non_local_load_with(load, &mem_vals)
    }

    /// `processNonLocalLoad`, for the dependences as `ReachingMemVal`s
    /// ([GVN.cpp:2102](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2102)).
    fn process_non_local_load_with(
        &mut self,
        load: InstId,
        deps: &[ReachingMemVal],
    ) -> bool {
        // Step 1: Find the non-local dependencies of the load.
        if deps.len() == 1 && deps[0].kind == DepKind::Other {
            // The only dependence is unknown.
            return false;
        }
        let mut changed = false;
        // This is a limited form of scalar PRE for load indices. If this load
        // follows a GEP, see if we can PRE the indices before analyzing.
        if self.pre {
            let pointer = self.function.instruction(load).operands[0];
            if let Operand::Value(value) = pointer
                && let ValueDef::Instruction(gep) = self.function.value(value).def
                && matches!(
                    self.function.instruction(gep).opcode,
                    Opcode::GetElementPtr { .. }
                )
            {
                for index in self.function.instruction(gep).operands[1..].to_vec() {
                    // Instructions inserted by GVN during this iteration (e.g.
                    // coercion casts from MaterializeAdjustedValue) may not
                    // have value numbers yet, so they are skipped.
                    if let Operand::Value(index) = index
                        && let ValueDef::Instruction(index) = self.function.value(index).def
                        && self.vn.exists(Operand::Value(self.function.instruction(index).result.expect("a value")))
                    {
                        changed |= self.perform_scalar_pre(index);
                    }
                }
            }
        }
        // Step 2: Analyze the availability of the load.
        let mut values_per_block = Vec::new();
        let mut unavailable_blocks = Vec::new();
        self.analyze_load_availability(load, deps, &mut values_per_block, &mut unavailable_blocks);
        // If we have no predecessors that produce a known value for this load,
        // exit early.
        if values_per_block.is_empty() {
            return changed;
        }
        // Step 3: Eliminate fully redundancy.
        if unavailable_blocks.is_empty() {
            let result = self.function.instruction(load).result.expect("a load has a value");
            let value = self.construct_ssa_for_load_set(load, &values_per_block);
            self.function.replace_all_uses_with(result, value);
            self.md_invalidate_cached_pointer_info(value);
            self.salvage_and_remove_instruction(load);
            return true;
        }
        // Step 4: Eliminate partial redundancy (`enable-load-pre`).
        if !self.pre {
            return changed;
        }
        self.perform_loop_load_pre(load, &mut values_per_block, &unavailable_blocks)
            || self.perform_load_pre(load, &mut values_per_block, &unavailable_blocks)
            || changed
    }
}
