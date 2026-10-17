//! `mem2reg`: an alloca read and written only by direct loads and stores
//! becomes SSA values and phis, and its loads, stores and the alloca go. A
//! translation of LLVM's `PromoteMemToReg` and the pass around it
//! (`PromoteMemoryToRegister.cpp`, `Mem2Reg.cpp` at d1106deb71cc).
//!
//! Unlike [`crate::promote`], which keeps every store for `dse`, this deletes
//! what it promotes. The steps of `PromoteMem2Reg::run` are in LLVM's order:
//! dead allocas, a single store, a single block, then phis at the iterated
//! dominance frontier of the stores where the alloca is live on entry, and a
//! renaming down the CFG from the entry.
//!
//! DEVIATE, each with its reason:
//! - Debug records (`dbg.declare`, assignment tracking): an alloca a `Declare`
//!   record names is not promoted, so `-g` keeps its variables.
//! - No `llvm.assume`, `noundef` metadata (`convertMetadataToAssumes`),
//!   lifetime or droppable intrinsics, `fake_use`: MIR has none.
//! - MIR has `poison` and no `undef`: `UndefValue` is `poison`.
//! - `isGuaranteedNotToBePoison` is true of a constant that is not `poison`
//!   only (no `noundef` arguments).
//! - The iterated dominance frontier is `Dominance::frontiers` iterated with
//!   the live-in pruning, which yields the set `ForwardIDFCalculator` does.
//! - `simplifyInstruction(PN)` is the phi case of `simplifyPHINode`, local
//!   until `simplify_phi` of `llrm_mir::simplify` lands.
//! - `NoSignedZeros` (fast-math flags of the phi): MIR phis carry none.

use llrm_analysis::cfg;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::types::TypeId;
use llrm_mir::{Constant, ConstantKind, Context};
use llrm_support::hash::{HashMap, HashSet};

/// The pass: LLVM's `PromotePass` (`Mem2Reg.cpp`).
pub struct Mem2Reg;

impl FunctionPass for Mem2Reg {
    fn name(&self) -> &'static str {
        "mem2reg"
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        llrm_mir::footprint::Footprint::MEMORY | llrm_mir::footprint::Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        _analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if promote_memory_to_register(unit.context, unit.function) {
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
    function: &mut Function,
) -> bool {
    let Some(entry) = function.entry() else { return false };
    let mut changed = false;
    loop {
        let declared = declared_allocas(function);
        let allocas: Vec<InstId> = function
            .block(entry)
            .instructions()
            .iter()
            .copied()
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }))
            .filter(|&inst| is_alloca_promotable(context, function, inst) && !declared.contains(&inst))
            .collect();
        if allocas.is_empty() {
            return changed;
        }
        promote_mem_to_reg(context, function, &allocas);
        changed = true;
    }
}

/// The allocas a debug record names (DEVIATE: they are not promoted).
fn declared_allocas(function: &Function) -> HashSet<InstId> {
    function
        .debug_records()
        .iter()
        .filter_map(|one| match one.what {
            llrm_mir::DebugWhat::Declare(Operand::Value(value)) => Some(value),
            _ => None,
        })
        .filter_map(|value| match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(inst) => Some(inst),
            _ => None,
        })
        .collect()
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

/// `isAllocaPromotable`: only direct, non-volatile loads and stores of one
/// type; a pointer derived by a bitcast or a gep of zeros is accepted when
/// nothing uses it (what MIR leaves of `onlyUsedByLifetimeMarkers`).
/// PromoteMemoryToRegister.cpp#L66
pub fn is_alloca_promotable(
    context: &Context,
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
            Opcode::Cast(CastOp::BitCast) | Opcode::Cast(CastOp::AddrSpaceCast) => {
                if user.result.is_some_and(|result| !function.users(result).is_empty()) {
                    return false;
                }
            }
            Opcode::GetElementPtr { .. } => {
                if !has_all_zero_indices(context, function, one.user)
                    || user.result.is_some_and(|result| !function.users(result).is_empty())
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
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
    numbers: HashMap<InstId, usize>,
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
        for &one in function.block(block).instructions() {
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
    context: &mut Context,
    ty: TypeId,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }))
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

/// `rewriteSingleStoreAlloca`: one store; each load it dominates takes its
/// value. Whether the alloca is gone; if not, `info.using_blocks` holds the
/// blocks of the loads left. PromoteMemoryToRegister.cpp#L593
fn rewrite_single_store_alloca(
    context: &mut Context,
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
            } else if !dominance.dominates(cfg::id(store_block), cfg::id(load_block)) {
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
    function.erase(alloca).expect("nothing uses it any more");
}

/// `promoteSingleBlockAlloca`: loads and stores within one block, in one pass
/// over it. Whether the alloca is gone (false when a load comes before every
/// store). PromoteMemoryToRegister.cpp#L706
fn promote_single_block_alloca(
    context: &mut Context,
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
    let mut replacements: Vec<(InstId, Operand)> = Vec::new();
    for &user in &uses {
        if !matches!(function.instruction(user).opcode, Opcode::Load { .. }) {
            continue;
        }
        let load_index = lbi.instruction_index(function, user);
        let at = stores.partition_point(|&(index, _)| index < load_index);
        let value = if at == 0 {
            if stores.is_empty() {
                poison(context, function.instruction(user).ty)
            } else {
                // No store before this load: it may be affected by the stores
                // after it.
                return false;
            }
        } else {
            function.instruction(stores[at - 1].1).operands[0]
        };
        replacements.push((user, value));
    }
    for (load, mut value) in replacements {
        let load_value = function.instruction(load).result.expect("a load's value");
        if value == Operand::Value(load_value) {
            value = poison(context, function.instruction(load).ty);
        }
        function.replace_value(load_value, value);
        function.erase(load).expect("its uses were replaced");
        lbi.delete_value(load);
    }
    // The stores and the alloca are dead.
    for (_, store) in stores {
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
    context: &'a mut Context,
    function: &'a mut Function,
    allocas: Vec<InstId>,
    dominance: cfg::Dominance,
    /// The phis added, by block and alloca (a `BTreeMap`: its order is
    /// deterministic, as the `DenseMap` of LLVM's is).
    new_phi_nodes: std::collections::BTreeMap<(i64, usize), InstId>,
    phi_to_alloca: HashMap<InstId, usize>,
    alloca_lookup: HashMap<ValueId, usize>,
    alloca_value_types: Vec<Option<TypeId>>,
    visited: HashSet<BlockId>,
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
        // `removeIntrinsicUsers` (L522) has no intrinsic to remove in MIR; the
        // dead pointers derived from an alloca go with it (`erase_alloca`).
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
            if info.only_used_in_one_block && promote_single_block_alloca(self.context, self.function, alloca, &mut lbi)
            {
                self.remove_from_allocas_list(&mut alloca_num);
                alloca_num = alloca_num.wrapping_add(1);
                continue;
            }

            self.alloca_value_types[alloca_num] = info.value_type;
            self.alloca_lookup.insert(address, alloca_num);

            // The blocks that define the value, and those it is live into.
            let def_blocks: HashSet<BlockId> = info.defining_blocks.iter().copied().collect();
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

        // The incoming value of every alloca on entry is poison, for a load
        // before any store.
        let values: Vec<Operand> = (0..self.allocas.len())
            .map(|at| {
                let ty = self.alloca_value_types[at].expect("an accessed alloca has a type");
                poison(self.context, ty)
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
            let first = self.function.block(block).instructions()[0];
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
            for inst in self.function.block(block).instructions().to_vec() {
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
        def_blocks: &HashSet<BlockId>,
    ) -> HashSet<BlockId> {
        let address = self.function.instruction(alloca).result.expect("an alloca's address");
        let mut worklist: Vec<BlockId> = info.using_blocks.clone();
        // A using block that also defines the value: if the first reference is
        // a store, the value is not live into it.
        let mut at = 0;
        while at < worklist.len() {
            let block = worklist[at];
            if def_blocks.contains(&block) {
                let mut kept = true;
                for &inst in self.function.block(block).instructions() {
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
        let mut live_in: HashSet<BlockId> = HashSet::default();
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
        def_blocks: &HashSet<BlockId>,
        live_in_blocks: &HashSet<BlockId>,
    ) -> Vec<BlockId> {
        let frontiers = self.frontiers.get_or_insert_with(|| self.dominance.frontiers(self.function));
        let mut work: Vec<i64> = def_blocks.iter().map(|&block| cfg::id(block)).collect();
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
        let first = self.function.block(block).instructions().first().copied();
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
        let first_phi = self.function.block(block).instructions().first().copied();
        if let Some(first) = first_phi
            && self.phi_to_alloca.contains_key(&first)
        {
            let new_operands = self.function.instruction(first).operands.len();
            let pred = pred.expect("a block with inserted phis has a predecessor");
            // Edges from `pred` to this block (`count(successors(Pred), BB)`).
            let edge_count = edges(self.function, block).iter().filter(|&&one| one == pred).count();
            for inst in self.function.block(block).instructions().to_vec() {
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
            }
        }

        // Do not revisit blocks.
        if !self.visited.insert(block) {
            return;
        }

        for inst in self.function.block(block).instructions().to_vec() {
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
    context: &mut Context,
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
                defined != at && dominance.dominates(cfg::id(defined), cfg::id(at))
            }
            _ => true,
        },
        _ => true,
    };
    available.then_some(value)
}

/// `PromoteMemToReg`: `allocas` promoted. PromoteMemoryToRegister.cpp#L1264
fn promote_mem_to_reg(
    context: &mut Context,
    function: &mut Function,
    allocas: &[InstId],
) {
    if allocas.is_empty() {
        return;
    }
    let dominance = cfg::Dominance::of(function);
    let count = allocas.len();
    PromoteMem2Reg {
        context,
        function,
        allocas: allocas.to_vec(),
        dominance,
        new_phi_nodes: Default::default(),
        phi_to_alloca: Default::default(),
        alloca_lookup: Default::default(),
        alloca_value_types: vec![None; count],
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
