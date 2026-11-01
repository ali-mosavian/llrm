//! `mem2reg`: an alloca read and written only by direct loads and stores
//! becomes SSA values and phis, and its loads, stores and the alloca go. A
//! translation of LLVM's `PromoteMemToReg` and the pass around it
//! (`PromoteMemoryToRegister.cpp`, `Mem2Reg.cpp` at d1106deb71cc).
//!
//! # How it works
//! Unlike [`crate::promote`], which keeps every store for `dse`, this deletes
//! what it promotes. The steps of `PromoteMem2Reg::run` are in LLVM's order:
//! dead allocas, a single store, a single block, then phis at the iterated
//! dominance frontier of the stores where the alloca is live on entry, and a
//! renaming down the CFG from the entry.
//!
//! LLVM structure (PromoteMemoryToRegister.cpp): `isAllocaPromotable` L66,
//! `AllocaInfo::AnalyzeAlloca` L235, `LargeBlockInfo` L330,
//! `removeIntrinsicUsers` L522, `rewriteSingleStoreAlloca` L593,
//! `promoteSingleBlockAlloca` L706, `PromoteMem2Reg::run` L800,
//! `ComputeLiveInBlocks` L1044, `QueuePhiNode` L1113, `RenamePass` L1147,
//! `PromoteMemToReg` L1264; `Mem2Reg.cpp` L34.
//!
//! DEVIATE, each with its reason:
//! - Debug records: a `Declare` of the alloca becomes `Value` records of what
//!   is stored (`ConvertDebugDeclareToDebugValue`), a `Piece` where the stored
//!   value is narrower than the alloca. No assignment tracking (`dbg.assign`):
//!   MIR has none.
//! - No `llvm.assume`, so no `convertMetadataToAssumes` (`noundef` metadata)
//!   and no droppable users; no `fake_use`. Lifetime markers are translated.
//! - `isGuaranteedNotToBePoison` is true of a constant that is not `poison`
//!   only (no `noundef` arguments).
//! - The iterated dominance frontier is `Dominance::frontiers` iterated with
//!   the live-in pruning, which yields the set `ForwardIDFCalculator` does.
//! - `simplifyInstruction(PN)` is the phi case of `simplifyPHINode`, local
//!   until `simplify_phi` of `llrm_mir::simplify` lands.
//! - `NoSignedZeros` (fast-math flags of the phi): MIR phis carry none.
//!
//! # Example
//! Before (from `test_stores_in_two_arms_make_a_phi_at_the_join`):
//! ```mir
//! define i32 @f(i1 %c) {
//! b0:
//!   %s = alloca i32
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   store i32 1, ptr %s
//!   br label %b3
//!
//! b2:
//!   store i32 2, ptr %s
//!   br label %b3
//!
//! b3:
//!   %x = load i32, ptr %s
//!   ret i32 %x
//! }
//! ```
//!
//! After:
//! ```mir
//! define i32 @f(i1 %c) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   br label %b3
//!
//! b2:
//!   br label %b3
//!
//! b3:
//!   %x = phi i32 [ 1, %b1 ], [ 2, %b2 ]
//!   ret i32 %x
//! }
//! ```
//!
//! # Used by
//! [`crate::pipeline`]: the round's `Promote` is swapped for `Mem2Reg` when
//! `Options::mem2reg` is set (on by default; `LLRM_NO_MEM2REG` turns it off).

use llrm_analysis::cfg;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::types::TypeId;
use llrm_mir::{Constant, ConstantKind, Context};

/// The pass: LLVM's `PromotePass` (`Mem2Reg.cpp`).
pub struct Mem2Reg;

impl FunctionPass for Mem2Reg {
    fn name(&self) -> &'static str {
        "mem2reg"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("mem2reg")
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        llrm_mir::footprint::Footprint::MEMORY | llrm_mir::footprint::Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if promote_memory_to_register(unit.context, unit.layout, outer.callees(), unit.function) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `promoteMemoryToRegister`: the promotable allocas of the entry block, until
/// there are none. Mem2Reg.cpp#L34
fn promote_memory_to_register(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    let Some(entry) = function.entry() else { return false };
    let mut changed = false;
    loop {
        let allocas: Vec<InstId> = function
            .iter_block(entry)
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }))
            .filter(|&inst| is_alloca_promotable(context, callees, function, inst))
            .collect();
        if allocas.is_empty() {
            return changed;
        }
        promote_mem_to_reg(context, layout, callees, function, &allocas);
        changed = true;
    }
}

/// Whether every operand of `gep` after the base is the constant zero.
fn has_all_zero_indices(
    context: &Context,
    function: &Function,
    gep: InstId,
) -> bool {
    function
        .instruction(gep)
        .operands[1..]
        .iter()
        .all(
            |operand| match operand {
                Operand::Constant(id) => matches!(
                    context.get(*id).kind,
                    ConstantKind::Int(0) | ConstantKind::Zero
                ),
                _ => false,
            },
        )
}

/// `onlyUsedByLifetimeMarkers`: every use of `value` is a `llvm.lifetime.*`
/// call about it. PromoteMemoryToRegister.cpp#L75 (`Instruction.cpp`)
fn only_used_by_lifetime_markers(
    context: &Context,
    callees: &Callees,
    function: &Function,
    value: ValueId,
) -> bool {
    function
        .users(value)
        .iter()
        .all(|one| memory::lifetime(context, callees, function, one.user) == Some(Operand::Value(value)))
}

/// `isAllocaPromotable`: only direct, non-volatile loads and stores of one
/// type, lifetime markers, and pointers derived by a bitcast or a gep of
/// zeros that only lifetime markers use. PromoteMemoryToRegister.cpp#L66
pub fn is_alloca_promotable(
    context: &Context,
    callees: &Callees,
    function: &Function,
    alloca: InstId,
) -> bool {
    let Some(address) = function.instruction(alloca).result else { return false };
    let mut expected: Option<TypeId> = None;
    for one in function.users(address) {
        let user = function.instruction(one.user);
        match user.opcode {
            Opcode::Load { volatile, .. } => {
                if volatile {
                    return false;
                }
                if expected.is_some_and(|ty| ty != user.ty) {
                    return false;
                }
                expected = Some(user.ty);
            }
            Opcode::Store { volatile, .. } => {
                // A store OF the alloca is no promotion.
                if one.index != 1 || user.operands[0] == Operand::Value(address) || volatile {
                    return false;
                }
                let stored = function.operand_type(context, user.operands[0]);
                if stored.is_none() || (expected.is_some() && expected != stored) {
                    return false;
                }
                expected = stored;
            }
            Opcode::Call(_) => {
                // Only a lifetime marker about the alloca (DEVIATE: no
                // droppable intrinsics in MIR).
                if memory::lifetime(context, callees, function, one.user) != Some(Operand::Value(address)) {
                    return false;
                }
            }
            Opcode::Cast(CastOp::BitCast) | Opcode::Cast(CastOp::AddrSpaceCast) => {
                let Some(result) = user.result else { return false };
                if !only_used_by_lifetime_markers(context, callees, function, result) {
                    return false;
                }
            }
            Opcode::GetElementPtr { .. } => {
                let Some(result) = user.result else { return false };
                if !has_all_zero_indices(context, function, one.user)
                    || !only_used_by_lifetime_markers(context, callees, function, result)
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// `removeIntrinsicUsers`: the lifetime markers of a promotable alloca, and
/// the pointers derived from it, go; a marker leaves a store of poison where
/// it was, as the memory is dead from there. PromoteMemoryToRegister.cpp#L522
fn remove_intrinsic_users(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    alloca: InstId,
) {
    let address = function.instruction(alloca).result.expect("an alloca's address");
    // The type used by the loads and stores (the first one found).
    let promoted_type = function
        .users(address)
        .iter()
        .find_map(
            |one| {
                let user = function.instruction(one.user);
                match user.opcode {
                    Opcode::Load { .. } => Some(user.ty),
                    Opcode::Store { .. } => function.operand_type(context, user.operands[0]),
                    _ => None,
                }
            },
        );
    let align = match function.instruction(alloca).opcode {
        Opcode::Alloca { align, .. } => align,
        _ => None,
    };
    let void = context.types.void();
    let users: Vec<InstId> = function.users(address).iter().map(|one| one.user).collect();
    for user in users {
        if matches!(
            function.instruction(user).opcode,
            Opcode::Load { .. } | Opcode::Store { .. }
        ) {
            continue;
        }
        if let Some(derived) = function.instruction(user).result {
            // Erase the users of the derived pointer now (its lifetime
            // markers).
            for inner in function.users(derived).iter().map(|one| one.user).collect::<Vec<_>>() {
                function.erase(inner).expect("a marker has no value");
            }
        }
        // A marker directly on the alloca: the bytes are dead from here.
        if let Some(ty) = promoted_type
            && memory::lifetime(context, callees, function, user).is_some()
        {
            let value = uninitialized(context, ty);
            let store = function.create_instruction(
                Opcode::Store { align, volatile: false },
                void,
                vec![value, Operand::Value(address)],
                Flags::default(),
                None,
            );
            function.insert(store, Position::Before(user)).expect("a placed marker");
        }
        function.erase(user).expect("nothing uses it any more");
    }
}

/// `AllocaInfo`: what the loads and stores of one alloca are.
/// PromoteMemoryToRegister.cpp#L203
#[derive(Default)]
struct AllocaInfo {
    defining_blocks: Vec<BlockId>,
    using_blocks: Vec<BlockId>,
    only_store: Option<InstId>,
    only_block: Option<BlockId>,
    only_used_in_one_block: bool,
    value_type: Option<TypeId>,
}

impl AllocaInfo {
    /// `AnalyzeAlloca`. PromoteMemoryToRegister.cpp#L235
    fn analyze(
        context: &Context,
        function: &Function,
        alloca: InstId,
    ) -> Self {
        let mut info = Self { only_used_in_one_block: true, ..Self::default() };
        let address = function.instruction(alloca).result.expect("an alloca's address");
        for one in function.users(address) {
            let user = function.instruction(one.user);
            let parent = function.parent(one.user).expect("a placed user");
            match user.opcode {
                Opcode::Store { .. } => {
                    info.defining_blocks.push(parent);
                    info.only_store = Some(one.user);
                    info.value_type.get_or_insert_with(|| {
                        function.operand_type(context, user.operands[0]).expect("a stored value has a type")
                    });
                }
                Opcode::Load { .. } => {
                    info.using_blocks.push(parent);
                    info.value_type.get_or_insert(user.ty);
                }
                // A dead bitcast or gep of zeros: no load or store.
                _ => continue,
            }
            if info.only_used_in_one_block {
                match info.only_block {
                    None => info.only_block = Some(parent),
                    Some(block) if block != parent => info.only_used_in_one_block = false,
                    _ => {}
                }
            }
        }
        info
    }
}

/// `LargeBlockInfo`: the order of the loads and stores of allocas within a
/// block, numbered once a block. PromoteMemoryToRegister.cpp#L330
#[derive(Default)]
struct LargeBlockInfo {
    numbers: IdMap<InstId, usize>,
}

impl LargeBlockInfo {
    /// `isInterestingInstruction`. PromoteMemoryToRegister.cpp#L341
    fn is_interesting(
        function: &Function,
        inst: InstId,
    ) -> bool {
        let op = function.instruction(inst);
        let pointer = match op.opcode {
            Opcode::Load { .. } => 0,
            Opcode::Store { .. } => 1,
            _ => return false,
        };
        matches!(
            op.operands[pointer],
            Operand::Value(value) if matches!( function.value(value).def, llrm_mir::module::ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Alloca { .. }) )
        )
    }

    /// `getInstructionIndex`. PromoteMemoryToRegister.cpp#L347
    fn instruction_index(
        &mut self,
        function: &Function,
        inst: InstId,
    ) -> usize {
        if let Some(&number) = self.numbers.get(&inst) {
            return number;
        }
        let block = function.parent(inst).expect("a placed instruction");
        let mut next = 0;
        for one in function.iter_block(block) {
            if Self::is_interesting(function, one) {
                self.numbers.insert(one, next);
                next += 1;
            }
        }
        self.numbers[&inst]
    }

    /// `deleteValue`. PromoteMemoryToRegister.cpp#L371
    fn delete_value(
        &mut self,
        inst: InstId,
    ) {
        self.numbers.remove(&inst);
    }
}

/// `poison` of `ty`: MIR's `UndefValue`/`PoisonValue`.
fn poison(
    context: &Context,
    ty: TypeId,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }))
}

/// `UndefValue::get(ty)`.
fn uninitialized(
    context: &Context,
    ty: TypeId,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Undef }))
}

/// `isGuaranteedNotToBePoison` as far as MIR can say: a constant that is not
/// poison. PromoteMemoryToRegister.cpp#L599
fn is_guaranteed_not_poison(
    context: &Context,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if !matches!(context.get(id).kind, ConstantKind::Poison)
    )
}

/// The variables a `Declare` record says live in `address` (`DPUsers`).
fn declared_variables(
    function: &Function,
    address: ValueId,
) -> Vec<llrm_mir::MetadataId> {
    function
        .debug_records()
        .iter()
        .filter(|one| {
            matches!(
                one.what,
                llrm_mir::DebugWhat::Declare(at) if at == Operand::Value(address)
            )
        })
        .map(|one| one.variable)
        .collect()
}

/// The variable fragments the `#dbg_assign` records linked to `alloca` say
/// something of, one record each (`AssignmentTrackingInfo::init`).
fn assigned_variables(
    function: &Function,
    alloca: InstId,
) -> Vec<(llrm_mir::MetadataId, Option<llrm_mir::MetadataId>)> {
    let mut out: Vec<(llrm_mir::MetadataId, Option<llrm_mir::MetadataId>)> = Vec::new();
    for at in function.assignment_markers(alloca) {
        let record = function.debug_records()[at];
        if !out.contains(&(record.variable, record.expression)) {
            out.push((record.variable, record.expression));
        }
    }
    out
}

/// `AssignmentTrackingInfo::updateForDeletedStore`: the `#dbg_assign`s linked
/// to `store` become `#dbg_value`s where they stand, and are deleted; a
/// variable of `assigned` with none linked to the store is its stored value
/// from the store on. PromoteMemoryToRegister.cpp#L148
fn update_for_deleted_store(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    alloca: InstId,
    assigned: &[(llrm_mir::MetadataId, Option<llrm_mir::MetadataId>)],
    store: InstId,
    stored: Operand,
) {
    if assigned.is_empty() {
        return;
    }
    let markers = function.assignment_markers(store);
    let mut seen = Vec::new();
    for &at in &markers {
        let record = function.debug_records()[at];
        let llrm_mir::DebugWhat::Assign { value, .. } = record.what else { continue };
        seen.push((record.variable, record.expression));
        let what = value.map_or(llrm_mir::DebugWhat::Gone, llrm_mir::DebugWhat::Value);
        function.add_debug_record_with(record.before, record.variable, what, record.expression, record.location);
    }
    function.remove_debug_records(&markers);
    for &(variable, expression) in assigned {
        if !seen.contains(&(variable, expression)) {
            convert_declares_to_values(context, layout, function, alloca, &[variable], store, stored);
        }
    }
}

/// `at::deleteAssignmentMarkers`: the `#dbg_assign`s linked to `alloca` go with
/// it. PromoteMemoryToRegister.cpp#L680
fn delete_assignment_markers(
    function: &mut Function,
    alloca: InstId,
) {
    let markers = function.assignment_markers(alloca);
    function.remove_debug_records(&markers);
}

/// `ConvertDebugDeclareToDebugValue`: before `at`, each of `variables` is
/// `value`: all of it, or the `bytes` of it that the value covers.
/// PromoteMemoryToRegister.cpp#L118 (`Local.cpp`)
fn convert_declares_to_values(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    alloca: InstId,
    variables: &[llrm_mir::MetadataId],
    at: InstId,
    value: Operand,
) {
    if variables.is_empty() {
        return;
    }
    let Opcode::Alloca { allocated, .. } = function.instruction(alloca).opcode else { return };
    let covered = function.operand_type(context, value).map(|ty| layout.alloc_size(&context.types, ty));
    let whole = layout.alloc_size(&context.types, allocated);
    for &variable in variables {
        let what = match covered {
            Some(bytes) if bytes != whole => llrm_mir::DebugWhat::Piece { value, offset: 0, bytes: bytes as u32 },
            _ => llrm_mir::DebugWhat::Value(value),
        };
        function.add_debug_record(at, variable, what);
    }
}

/// `rewriteSingleStoreAlloca`: one store; each load it dominates takes its
/// value. Whether the alloca is gone; if not, `info.using_blocks` holds the
/// blocks of the loads left. PromoteMemoryToRegister.cpp#L593
fn rewrite_single_store_alloca(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    alloca: InstId,
    info: &mut AllocaInfo,
    lbi: &mut LargeBlockInfo,
    dominance: &cfg::Dominance,
) -> bool {
    let only_store = info.only_store.expect("one defining block has a store");
    let mut replacement = function.instruction(only_store).operands[0];
    // Loads may load the stored value or uninitialized memory. If the stored
    // value may be poison, replacing an uninitialized load with it would be
    // wrong; a dominating store makes it sure.
    let require_dominating_store = !is_guaranteed_not_poison(context, replacement);
    let store_block = function.parent(only_store).expect("a placed store");
    let mut store_index: Option<usize> = None;
    info.using_blocks.clear();
    let address = function.instruction(alloca).result.expect("an alloca's address");
    let users: Vec<InstId> = function.users(address).iter().map(|one| one.user).collect();
    for user in users {
        if user == only_store || !matches!(function.instruction(user).opcode, Opcode::Load { .. }) {
            continue;
        }
        let load_block = function.parent(user).expect("a placed load");
        if require_dominating_store {
            if load_block == store_block {
                // A use in the block of the store: the order says which is
                // first.
                let index = *store_index.get_or_insert_with(|| lbi.instruction_index(function, only_store));
                if index > lbi.instruction_index(function, user) {
                    info.using_blocks.push(store_block);
                    continue;
                }
            } else if !dominance.dominates(function, cfg::id(store_block), cfg::id(load_block)) {
                info.using_blocks.push(load_block);
                continue;
            }
        }
        let load_value = function.instruction(user).result.expect("a load's value");
        // The value is the load: this is unreachable code.
        if replacement == Operand::Value(load_value) {
            replacement = poison(context, function.instruction(user).ty);
        }
        function.replace_value(load_value, replacement);
        function.erase(user).expect("its uses were replaced");
        lbi.delete_value(user);
    }
    // After the scan, is the store all that is left?
    if !info.using_blocks.is_empty() {
        return false;
    }
    // The variable is the stored value from the store on.
    let variables = declared_variables(function, address);
    let assigned = assigned_variables(function, alloca);
    let stored = function.instruction(only_store).operands[0];
    convert_declares_to_values(context, layout, function, alloca, &variables, only_store, stored);
    update_for_deleted_store(context, layout, function, alloca, &assigned, only_store, stored);
    function.erase(only_store).expect("a store has no users");
    lbi.delete_value(only_store);
    erase_alloca(function, alloca);
    true
}

/// The alloca with the dead pointers derived from it.
fn erase_alloca(
    function: &mut Function,
    alloca: InstId,
) {
    let address = function.instruction(alloca).result.expect("an alloca's address");
    for user in function.users(address).iter().map(|one| one.user).collect::<Vec<_>>() {
        function.erase(user).expect("a derived pointer nothing uses");
    }
    delete_assignment_markers(function, alloca);
    function.erase(alloca).expect("nothing uses it any more");
}

/// `promoteSingleBlockAlloca`: loads and stores within one block, in one pass
/// over it. Whether the alloca is gone (false when a load comes before every
/// store). PromoteMemoryToRegister.cpp#L706
fn promote_single_block_alloca(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    alloca: InstId,
    lbi: &mut LargeBlockInfo,
) -> bool {
    let address = function.instruction(alloca).result.expect("an alloca's address");
    let uses: Vec<InstId> = function.users(address).iter().map(|one| one.user).collect();
    // Every store, by its index in the block.
    let mut stores: Vec<(usize, InstId)> = uses
        .iter()
        .copied()
        .filter(|&user| matches!(function.instruction(user).opcode, Opcode::Store { .. }))
        .map(|store| (lbi.instruction_index(function, store), store))
        .collect();
    stores.sort_by_key(|&(index, _)| index);
    // A load takes the nearest store above it, or poison if there is none.
    // First pass: decide, so a bail out changes nothing (LLVM bails before
    // the loads it has rewritten matter, as those are replaced by values that
    // stay right).
    // The value a load takes is read from its store when the load is
    // replaced: an earlier replacement may have changed it (a store of
    // another load of this alloca).
    let mut replacements: Vec<(InstId, Result<InstId, Operand>)> = Vec::new();
    for &user in &uses {
        if !matches!(function.instruction(user).opcode, Opcode::Load { .. }) {
            continue;
        }
        let load_index = lbi.instruction_index(function, user);
        let at = stores.partition_point(|&(index, _)| index < load_index);
        let source = if at == 0 {
            if stores.is_empty() {
                Err(uninitialized(context, function.instruction(user).ty))
            } else {
                // No store before this load: it may be affected by the stores
                // after it.
                return false;
            }
        } else {
            Ok(stores[at - 1].1)
        };
        replacements.push((user, source));
    }
    for (load, source) in replacements {
        let load_value = function.instruction(load).result.expect("a load's value");
        let mut value = source.map_or_else(|value| value, |store| function.instruction(store).operands[0]);
        if value == Operand::Value(load_value) {
            value = poison(context, function.instruction(load).ty);
        }
        function.replace_value(load_value, value);
        function.erase(load).expect("its uses were replaced");
        lbi.delete_value(load);
    }
    // The stores and the alloca are dead.
    let variables = declared_variables(function, address);
    let assigned = assigned_variables(function, alloca);
    for (_, store) in stores {
        let stored = function.instruction(store).operands[0];
        convert_declares_to_values(context, layout, function, alloca, &variables, store, stored);
        update_for_deleted_store(context, layout, function, alloca, &assigned, store, stored);
        function.erase(store).expect("a store has no users");
        lbi.delete_value(store);
    }
    erase_alloca(function, alloca);
    true
}

/// A vector with an undo log, `VectorWithUndo`.
/// PromoteMemoryToRegister.cpp#L291
struct VectorWithUndo {
    values: Vec<Operand>,
    undo: Vec<(usize, Operand)>,
}

impl VectorWithUndo {
    /// `undo`. PromoteMemoryToRegister.cpp#L296
    fn undo(
        &mut self,
        size: usize,
    ) {
        while size < self.undo.len() {
            let (at, old) = self.undo.pop().expect("an entry above the mark");
            self.values[at] = old;
        }
    }

    /// `set`. PromoteMemoryToRegister.cpp#L300
    fn set(
        &mut self,
        at: usize,
        value: Operand,
    ) {
        if self.values[at] == value {
            return;
        }
        self.undo.push((at, self.values[at]));
        self.values[at] = value;
    }
}

/// `RenamePassData`. PromoteMemoryToRegister.cpp#L314
struct RenamePassData {
    block: BlockId,
    pred: Option<BlockId>,
    undo_values: usize,
}

/// `PromoteMem2Reg`, for the allocas of one call.
/// PromoteMemoryToRegister.cpp#L375
struct PromoteMem2Reg<'a> {
    layout: &'a DataLayout,
    callees: &'a Callees,
    context: &'a mut Context,
    function: &'a mut Function,
    allocas: Vec<InstId>,
    dominance: cfg::Dominance,
    /// The phis added, by block and alloca (a `BTreeMap`: its order is
    /// deterministic, as the `DenseMap` of LLVM's is).
    new_phi_nodes: std::collections::BTreeMap<(i64, usize), InstId>,
    phi_to_alloca: IdMap<InstId, usize>,
    alloca_lookup: IdMap<ValueId, usize>,
    alloca_value_types: Vec<Option<TypeId>>,
    /// The variables declared to live in each alloca (`AllocaDPUsers`).
    alloca_variables: Vec<Vec<llrm_mir::MetadataId>>,
    /// `AllocaATInfo`: the fragments of variables the `#dbg_assign`s of each
    /// alloca name.
    alloca_assigned: Vec<Vec<(llrm_mir::MetadataId, Option<llrm_mir::MetadataId>)>>,
    visited: IdSet<BlockId>,
    incoming_vals: VectorWithUndo,
    worklist: Vec<RenamePassData>,
    /// The dominance frontiers, worked out once.
    frontiers: Option<std::collections::BTreeMap<i64, std::collections::BTreeSet<i64>>>,
}

impl<'a> PromoteMem2Reg<'a> {
    /// `getNumPreds`: the edges into `block`. PromoteMemoryToRegister.cpp#L451
    fn num_preds(
        &self,
        block: BlockId,
    ) -> usize {
        edges(self.function, block).len()
    }

    /// `RemoveFromAllocasList`. PromoteMemoryToRegister.cpp#L445
    fn remove_from_allocas_list(
        &mut self,
        at: &mut usize,
    ) {
        let last = self.allocas.len() - 1;
        self.allocas.swap(*at, last);
        self.allocas.pop();
        // `--AllocaIdx`, undone by the loop's increment.
        *at = at.wrapping_sub(1);
    }

    /// `pushToWorklist`. PromoteMemoryToRegister.cpp#L472
    fn push_to_worklist(
        &mut self,
        block: BlockId,
        pred: Option<BlockId>,
    ) {
        self.worklist.push(RenamePassData { block, pred, undo_values: self.incoming_vals.undo.len() });
    }

    /// `popFromWorklist`. PromoteMemoryToRegister.cpp#L477
    fn pop_from_worklist(&mut self) -> RenamePassData {
        let data = self.worklist.pop().expect("a nonempty worklist");
        self.incoming_vals.undo(data.undo_values);
        data
    }

    /// `PromoteMem2Reg::run`. PromoteMemoryToRegister.cpp#L800
    fn run(&mut self) {
        let mut lbi = LargeBlockInfo::default();
        // All the inserted stores in advance: one numbering of each block.
        for alloca in self.allocas.clone() {
            remove_intrinsic_users(self.context, self.callees, self.function, alloca);
        }
        let mut alloca_num = 0;
        while alloca_num < self.allocas.len() {
            let alloca = self.allocas[alloca_num];
            let address = self.function.instruction(alloca).result.expect("an alloca's address");
            let has_access = self
                .function
                .users(address)
                .iter()
                .any(
                    |one| matches!(
                        self.function.instruction(one.user).opcode,
                        Opcode::Load { .. } | Opcode::Store { .. }
                    ),
                );
            if !has_access {
                // No uses of the alloca: delete it now.
                erase_alloca(self.function, alloca);
                self.remove_from_allocas_list(&mut alloca_num);
                alloca_num = alloca_num.wrapping_add(1);
                continue;
            }

            // The set of read and write locations of the alloca.
            let mut info = AllocaInfo::analyze(self.context, self.function, alloca);

            // A single store: loads it dominates take its value.
            if info.defining_blocks.len() == 1
                && rewrite_single_store_alloca(
                    self.context,
                    self.layout,
                    self.function,
                    alloca,
                    &mut info,
                    &mut lbi,
                    &self.dominance,
                )
            {
                self.remove_from_allocas_list(&mut alloca_num);
                alloca_num = alloca_num.wrapping_add(1);
                continue;
            }

            // Only read and written in one block: a linear sweep.
            if info.only_used_in_one_block
                && promote_single_block_alloca(self.context, self.layout, self.function, alloca, &mut lbi)
            {
                self.remove_from_allocas_list(&mut alloca_num);
                alloca_num = alloca_num.wrapping_add(1);
                continue;
            }

            self.alloca_value_types[alloca_num] = info.value_type;
            self.alloca_variables[alloca_num] = declared_variables(self.function, address);
            self.alloca_assigned[alloca_num] = assigned_variables(self.function, alloca);
            self.alloca_lookup.insert(address, alloca_num);

            // The blocks that define the value, and those it is live into.
            let def_blocks: IdSet<BlockId> = info.defining_blocks.iter().copied().collect();
            let live_in_blocks = self.compute_live_in_blocks(alloca, &info, &def_blocks);

            // The blocks that need phis: the iterated dominance frontier of the
            // definitions, kept where the value is live on entry.
            let mut phi_blocks = self.iterated_frontier(&def_blocks, &live_in_blocks);
            phi_blocks.sort_by_key(|&block| cfg::id(block));
            let mut version = 0;
            for block in phi_blocks {
                self.queue_phi_node(block, alloca_num, &mut version);
            }
            alloca_num = alloca_num.wrapping_add(1);
        }

        if self.allocas.is_empty() {
            return; // All of the allocas were trivial.
        }
        lbi.numbers.clear();

        // The incoming value of every alloca on entry is undef, for a load
        // before any store.
        let values: Vec<Operand> = (0..self.allocas.len())
            .map(|at| {
                let ty = self.alloca_value_types[at].expect("an accessed alloca has a type");
                uninitialized(self.context, ty)
            })
            .collect();
        self.incoming_vals = VectorWithUndo { values, undo: Vec::new() };

        // The renamer uses `visited` to avoid infinite loops.
        self.visited.clear();
        let entry = self.function.entry().expect("a defined function");
        self.push_to_worklist(entry, None);
        loop {
            let data = self.pop_from_worklist();
            self.rename_pass(data.block, data.pred);
            if self.worklist.is_empty() {
                break;
            }
        }

        // The allocas themselves. What is left of their uses is in unreachable
        // blocks the renamer did not walk: they become poison.
        for alloca in std::mem::take(&mut self.allocas) {
            let address = self.function.instruction(alloca).result.expect("an alloca's address");
            if !self.function.users(address).is_empty() {
                let ty = self.function.value(address).ty;
                let value = poison(self.context, ty);
                self.function.replace_value(address, value);
            }
            delete_assignment_markers(self.function, alloca);
            self.function.erase(alloca).expect("nothing uses it");
        }

        // The phis that merge one value and/or poison go; one going can
        // let another go.
        let mut eliminated = true;
        while eliminated {
            eliminated = false;
            let entries: Vec<((i64, usize), InstId)> =
                self.new_phi_nodes.iter().map(|(&key, &phi)| (key, phi)).collect();
            for (key, phi) in entries {
                if let Some(value) = simplify_phi(self.context, self.function, &self.dominance, phi) {
                    let result = self.function.instruction(phi).result.expect("a phi's value");
                    self.function.replace_value(result, value);
                    self.function.erase(phi).expect("its uses were replaced");
                    self.new_phi_nodes.remove(&key);
                    eliminated = true;
                }
            }
        }

        // The renamer reached the reachable code only: a phi may lack inputs
        // from unreachable predecessors, which get poison.
        let phis: Vec<InstId> = self.new_phi_nodes.values().copied().collect();
        for phi in phis {
            let block = self.function.parent(phi).expect("a placed phi");
            let first = self.function.block(block).first().expect("a placed phi's block");
            // Once a block, at its first phi.
            if first != phi {
                continue;
            }
            let have = self.function.instruction(phi).operands.len() / 2;
            if have == self.num_preds(block) {
                continue;
            }
            let mut missing = edges(self.function, block);
            let named: Vec<BlockId> = self
                .function
                .instruction(phi)
                .operands
                .iter()
                .filter_map(|operand| if let Operand::Block(one) = operand { Some(*one) } else { None })
                .collect();
            for one in named {
                if let Some(at) = missing.iter().position(|&pred| pred == one) {
                    missing.remove(at);
                }
            }
            // Every phi this pass inserted in the block has the same count.
            let bad = have;
            for inst in self.function.iter_block(block).collect::<Vec<_>>() {
                let op = self.function.instruction(inst);
                if op.opcode != Opcode::Phi || op.operands.len() / 2 != bad {
                    break;
                }
                let ty = op.ty;
                let value = poison(self.context, ty);
                let mut operands = self.function.instruction(inst).operands.clone();
                for &pred in &missing {
                    operands.extend([value, Operand::Block(pred)]);
                }
                self.function.set_operands(inst, operands);
            }
        }
        self.new_phi_nodes.clear();
    }

    /// `ComputeLiveInBlocks`: the blocks the value is live into, those that
    /// lead to a use. PromoteMemoryToRegister.cpp#L1044
    fn compute_live_in_blocks(
        &self,
        alloca: InstId,
        info: &AllocaInfo,
        def_blocks: &IdSet<BlockId>,
    ) -> IdSet<BlockId> {
        let address = self.function.instruction(alloca).result.expect("an alloca's address");
        let mut worklist: Vec<BlockId> = info.using_blocks.clone();
        // A using block that also defines the value: if the first reference is
        // a store, the value is not live into it.
        let mut at = 0;
        while at < worklist.len() {
            let block = worklist[at];
            if def_blocks.contains(&block) {
                let mut kept = true;
                for inst in self.function.iter_block(block) {
                    let op = self.function.instruction(inst);
                    match op.opcode {
                        Opcode::Store { .. } if op.operands[1] == Operand::Value(address) => {
                            kept = false;
                            break;
                        }
                        Opcode::Load { .. } if op.operands[0] == Operand::Value(address) => break,
                        _ => {}
                    }
                }
                if !kept {
                    worklist.swap_remove(at);
                    continue;
                }
            }
            at += 1;
        }
        // Their predecessors, until the whole region the value is live in.
        let mut live_in: IdSet<BlockId> = IdSet::new();
        while let Some(block) = worklist.pop() {
            if !live_in.insert(block) {
                continue;
            }
            for pred in self.function.predecessors(block) {
                if def_blocks.contains(&pred) {
                    continue;
                }
                worklist.push(pred);
            }
        }
        live_in
    }

    /// `ForwardIDFCalculator::calculate` with live-in blocks
    /// (`GenericIteratedDominanceFrontier.h`): the frontier of the definitions
    /// and of the phis placed, kept where the value is live.
    fn iterated_frontier(
        &mut self,
        def_blocks: &IdSet<BlockId>,
        live_in_blocks: &IdSet<BlockId>,
    ) -> Vec<BlockId> {
        let frontiers = self.frontiers.get_or_insert_with(|| self.dominance.frontiers(self.function));
        let mut work: Vec<i64> = def_blocks.iter().map(|block| cfg::id(block)).collect();
        let mut placed: std::collections::BTreeSet<i64> = Default::default();
        let mut out = Vec::new();
        while let Some(at) = work.pop() {
            for &frontier in frontiers.get(&at).into_iter().flatten() {
                if !live_in_blocks.contains(&cfg::block(frontier)) || !placed.insert(frontier) {
                    continue;
                }
                out.push(cfg::block(frontier));
                if !def_blocks.contains(&cfg::block(frontier)) {
                    work.push(frontier);
                }
            }
        }
        out
    }

    /// `QueuePhiNode`: a phi for the alloca at the top of the block. Whether
    /// one was made. PromoteMemoryToRegister.cpp#L1113
    fn queue_phi_node(
        &mut self,
        block: BlockId,
        alloca_no: usize,
        version: &mut usize,
    ) -> bool {
        if self.new_phi_nodes.contains_key(&(cfg::id(block), alloca_no)) {
            return false;
        }
        let ty = self.alloca_value_types[alloca_no].expect("an accessed alloca has a type");
        let address = self.function.instruction(self.allocas[alloca_no]).result.expect("an alloca's address");
        let name = self.function.value(address).name.clone().map(|one| format!("{one}.{version}"));
        *version += 1;
        let phi = self.function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), name.as_deref());
        let first = self.function.block(block).first();
        self.function.insert(phi, first.map_or(Position::End(block), Position::Before)).expect("a placed block");
        self.new_phi_nodes.insert((cfg::id(block), alloca_no), phi);
        self.phi_to_alloca.insert(phi, alloca_no);
        true
    }

    /// `RenamePass`: the loads and stores of the allocas in `block`, entered
    /// from `pred`; the successors go on the worklist.
    /// PromoteMemoryToRegister.cpp#L1147
    fn rename_pass(
        &mut self,
        block: BlockId,
        pred: Option<BlockId>,
    ) {
        // The phis inserted into this block are already in it: each takes the
        // value of its alloca from `pred`.
        let first_phi = self.function.block(block).first();
        if let Some(first) = first_phi
            && self.phi_to_alloca.contains_key(&first)
        {
            let new_operands = self.function.instruction(first).operands.len();
            let pred = pred.expect("a block with inserted phis has a predecessor");
            // Edges from `pred` to this block (`count(successors(Pred), BB)`).
            let edge_count = edges(self.function, block).iter().filter(|&&one| one == pred).count();
            for inst in self.function.iter_block(block).collect::<Vec<_>>() {
                let op = self.function.instruction(inst);
                if op.opcode != Opcode::Phi || op.operands.len() != new_operands {
                    break;
                }
                let Some(&alloca_no) = self.phi_to_alloca.get(&inst) else { break };
                let value = self.incoming_vals.values[alloca_no];
                let mut operands = self.function.instruction(inst).operands.clone();
                for _ in 0..edge_count {
                    operands.extend([value, Operand::Block(pred)]);
                }
                // The new phi is the alloca's value from here on.
                self.function.set_operands(inst, operands);
                let result = self.function.instruction(inst).result.expect("a phi's value");
                self.incoming_vals.set(alloca_no, Operand::Value(result));
                // And the variable is the phi from the top of the block.
                let mut variables = std::mem::take(&mut self.alloca_variables[alloca_no]);
                // `updateForNewPhi`: each fragment an assign names is the phi
                // too.
                let declared = variables.len();
                for &(variable, _) in &self.alloca_assigned[alloca_no] {
                    if !variables.contains(&variable) {
                        variables.push(variable);
                    }
                }
                let top =
                    self.function.iter_block(block).find(|&one| self.function.instruction(one).opcode != Opcode::Phi);
                if let Some(top) = top {
                    let alloca = self.allocas[alloca_no];
                    convert_declares_to_values(
                        self.context,
                        self.layout,
                        self.function,
                        alloca,
                        &variables,
                        top,
                        Operand::Value(result),
                    );
                }
                variables.truncate(declared);
                self.alloca_variables[alloca_no] = variables;
            }
        }

        // Do not revisit blocks.
        if !self.visited.insert(block) {
            return;
        }

        for inst in self.function.iter_block(block).collect::<Vec<_>>() {
            let op = self.function.instruction(inst);
            match op.opcode {
                Opcode::Load { .. } => {
                    let Operand::Value(pointer) = op.operands[0] else { continue };
                    let Some(&alloca_no) = self.alloca_lookup.get(&pointer) else { continue };
                    let value = self.incoming_vals.values[alloca_no];
                    let load_value = op.result.expect("a load's value");
                    // Anything using the load uses the current value.
                    self.function.replace_value(load_value, value);
                    self.function.erase(inst).expect("its uses were replaced");
                }
                Opcode::Store { .. } => {
                    let Operand::Value(pointer) = op.operands[1] else { continue };
                    let Some(&alloca_no) = self.alloca_lookup.get(&pointer) else { continue };
                    // The value written is the alloca's from here on.
                    let value = op.operands[0];
                    self.incoming_vals.set(alloca_no, value);
                    let variables = std::mem::take(&mut self.alloca_variables[alloca_no]);
                    let alloca = self.allocas[alloca_no];
                    convert_declares_to_values(
                        self.context,
                        self.layout,
                        self.function,
                        alloca,
                        &variables,
                        inst,
                        value,
                    );
                    self.alloca_variables[alloca_no] = variables;
                    let assigned = std::mem::take(&mut self.alloca_assigned[alloca_no]);
                    update_for_deleted_store(self.context, self.layout, self.function, alloca, &assigned, inst, value);
                    self.alloca_assigned[alloca_no] = assigned;
                    self.function.erase(inst).expect("a store has no users");
                }
                _ => {}
            }
        }

        // 'Recurse' to the successors, once each, the last first.
        for successor in self.function.successors(block).iter().rev() {
            self.push_to_worklist(*successor, Some(block));
        }
    }
}

/// Each edge into `block`, by the block it leaves.
fn edges(
    function: &Function,
    block: BlockId,
) -> Vec<BlockId> {
    function
        .block_users(block)
        .iter()
        .filter(|one| function.instruction(one.user).opcode.is_terminator())
        .filter_map(|one| function.parent(one.user))
        .collect()
}

/// The phi case of `simplifyInstruction`: a phi whose inputs are one value
/// (the phi itself and poison aside) is that value, when the value is a
/// constant, an argument or dominates the phi; one with nothing but those is
/// poison. Local until `llrm_mir::simplify::simplify_phi` lands.
/// InstructionSimplify.cpp `simplifyPHINode`
fn simplify_phi(
    context: &Context,
    function: &Function,
    dominance: &cfg::Dominance,
    phi: InstId,
) -> Option<Operand> {
    let op = function.instruction(phi);
    let own = op.result.map(Operand::Value);
    let mut common: Option<Operand> = None;
    for pair in op.operands.chunks(2) {
        let value = pair[0];
        if Some(value) == own {
            continue;
        }
        if matches!(
            value,
            Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Poison)
        ) {
            continue;
        }
        match common {
            None => common = Some(value),
            Some(seen) if seen == value => {}
            Some(_) => return None,
        }
    }
    let ty = op.ty;
    let Some(value) = common else { return Some(poison(context, ty)) };
    // The value must be available at the phi: the poison inputs were on paths
    // it may not reach.
    let at = function.parent(phi)?;
    let available = match value {
        Operand::Value(id) => match function.value(id).def {
            llrm_mir::module::ValueDef::Instruction(def) => {
                let defined = function.parent(def)?;
                defined != at && dominance.dominates(function, cfg::id(defined), cfg::id(at))
            }
            _ => true,
        },
        _ => true,
    };
    available.then_some(value)
}

/// `PromoteMemToReg`: `allocas` promoted. PromoteMemoryToRegister.cpp#L1264

pub(crate) fn promote_mem_to_reg(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    allocas: &[InstId],
) {
    if allocas.is_empty() {
        return;
    }
    let dominance = cfg::Dominance::of(function);
    let count = allocas.len();
    PromoteMem2Reg {
        layout,
        callees,
        context,
        function,
        allocas: allocas.to_vec(),
        dominance,
        new_phi_nodes: Default::default(),
        phi_to_alloca: Default::default(),
        alloca_lookup: Default::default(),
        alloca_value_types: vec![None; count],
        alloca_variables: vec![Vec::new(); count],
        alloca_assigned: vec![Vec::new(); count],
        visited: Default::default(),
        incoming_vals: VectorWithUndo { values: Vec::new(), undo: Vec::new() },
        worklist: Vec::new(),
        frontiers: None,
    }
    .run();
}

#[cfg(test)]
#[path = "mem2reg_tests.rs"]
mod tests;
