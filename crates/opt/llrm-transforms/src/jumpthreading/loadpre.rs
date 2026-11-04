//! A load that a branch or switch tests and some predecessors already hold the
//! value of is made a `phi` of those values, with a load inserted on the one
//! edge that has none, so the `phi` can be threaded.
//!
//! # How it works
//! A translation of `simplifyPartiallyRedundantLoad`
//! ([L1222](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1222)),
//! called from `processBlock` on the load a branch or switch tests, or the left
//! operand of a compare of it against a constant. The load is first looked for
//! in its own block (`find_available_loaded_value`); if the block is
//! transparent to it, each predecessor is scanned from its end
//! (`find_available_ptr_load_store`, the address translated through the
//! block's phis), and through single predecessors, at most
//! `DefMaxInstsToScan` (6) instructions in all. Predecessors without the value
//! are split off to one block, where the one load goes.
//!
//! ```text
//!   p1: store %v, %p             p1: store %v, %p
//!       br b                         br b
//!   p2: br b             ==>     p2: %x.pr = load %p
//!   b:  %x = load %p                 br b
//!       %c = icmp eq %x, 0       b:  %x = phi [%v, p1] [%x.pr, p2]
//!       br %c, t, e                  %c = icmp eq %x, 0 ...
//! ```
//!
//! DEVIATE: see J15 in `jumpthreading.rs`.
//!
//! # Used by
//! `jumpthreading` (`processBlock`).
//!
//! # Example
//! `%p1` stores 0, `%p2` does not; the load becomes a phi of 0 and a load in
//! `%p2`, and the test is threaded from `%p1`:
//! ```mir
//! p1:
//!   store i32 0, ptr %p
//!   br label %join
//! p2:
//!   br label %join
//! join:
//!   %x = load i32, ptr %p
//!   %z = icmp eq i32 %x, 0
//!   br i1 %z, label %a, label %b
//! ```

use llrm_analysis::aa::{AAInfo, LocationSize, MemoryLocation};
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::loads::{
    Available, DEF_MAX_INSTS_TO_SCAN, ModRefOracle, find_available_loaded_value, find_available_ptr_load_store,
};
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, MetadataId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::guarantee::is_guaranteed_to_transfer_execution_to_successor_range;

use super::State;
use crate::lcssa::from_arms;
use crate::local::combine_metadata_for_cse;

/// The metadata `AAMDNodes` is made of; a load inserted for the one without
/// the value carries the original's.
const AA_METADATA: [&str; 5] = ["tbaa", "tbaa.struct", "alias.scope", "noalias", "noalias.addrspace"];

/// `ScanLimit` of `isGuaranteedToTransferExecutionToSuccessor(Begin, End)`
/// ([ValueTracking.h:754](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueTracking.h#L754)).
const TRANSFER_SCAN_LIMIT: u32 = 32;

/// What the scan of the predecessors found: `AvailablePreds`, `PredsScanned`'s
/// size, `OneUnavailablePred` and `CSELoads`.
struct PredecessorScan {
    available: Vec<(BlockId, Operand)>,
    scanned: usize,
    one_unavailable: Option<BlockId>,
    cse_loads: Vec<InstId>,
}

impl State<'_, '_> {
    /// `simplifyPartiallyRedundantLoad`
    /// ([L1222](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1222)).
    pub(super) fn simplify_partially_redundant_load(
        &mut self,
        load: InstId,
    ) -> bool {
        // Don't hack volatile and ordered loads.
        let Opcode::Load { align, volatile: false } = *self.opcode(load) else { return false };

        // If the load is defined in a block with exactly one predecessor, it
        // can't be partially redundant. Nor in an EH pad, whose edges cannot
        // have other instructions between them.
        let load_bb = self.function().parent(load).expect("a placed load");
        if self.single_predecessor(load_bb).is_some() || self.starts_with_landing_pad(load_bb) {
            return false;
        }

        // If the loaded operand is defined in the LoadBB and its not a phi,
        // it can't be available in predecessors.
        let loaded_ptr = self.op(load, 0);
        if let Some(def) = self.instruction_of(loaded_ptr)
            && self.function().parent(def) == Some(load_bb)
            && *self.opcode(def) != Opcode::Phi
        {
            return false;
        }

        // Scan a few instructions up from the load, to see if it is obviously
        // live at the entry to its block.
        let mut scan_from = Some(load);
        let local = self.with_alias_analysis(|unit, aa| {
            find_available_loaded_value(unit, load, load_bb, &mut scan_from, DEF_MAX_INSTS_TO_SCAN, Some(aa), None)
        });
        if let Some(available) = local {
            // If the value of the load is locally available within the block,
            // just use it. This frequently occurs for reg2mem'd allocas.
            self.replace_load_with_available(load, available);
            return true;
        }

        // Otherwise, if we scanned the whole block and got to the top of the
        // block, we know the block is locally transparent to the load.
        if scan_from != self.function().block(load_bb).first() {
            return false;
        }

        let scan = self.scan_predecessors(load, load_bb);
        // If the loaded value isn't available in any predecessor, it isn't
        // partially redundant.
        if scan.available.is_empty() {
            return false;
        }

        // The value is available in at least one (and maybe all!)
        // predecessors. If it is unavailable in more than one unique
        // predecessor, a merge block for those makes one reload enough.
        let mut guaranteed_to_transfer = None;
        let mut can_speculate_into = |this: &Self| this.can_speculate_load(load, load_bb, &mut guaranteed_to_transfer);
        let mut unavailable_pred = None;
        match scan.one_unavailable {
            // The one unavailable predecessor, if it ends in an unconditional
            // branch, is not a critical edge.
            Some(only) if scan.scanned == scan.available.len() + 1 && self.successor_edges(only).len() == 1 => {
                if !can_speculate_into(self) {
                    return false;
                }
                unavailable_pred = Some(only);
            }
            // Multiple unavailable predecessors, or a critical edge from the
            // one.
            _ if scan.scanned != scan.available.len() => {
                let available_set: IdSet<BlockId> = scan.available.iter().map(|&(pred, _)| pred).collect();
                let mut to_split = Vec::new();
                let mut seen = IdSet::new();
                for pred in self.pred_edges(load_bb) {
                    if !available_set.contains(&pred) {
                        if !can_speculate_into(self) {
                            return false;
                        }
                        if seen.insert(pred) {
                            to_split.push(pred);
                        }
                    }
                }
                unavailable_pred = Some(self.split_block_preds(load_bb, &to_split));
            }
            _ => {}
        }

        // Insert a load on the edge the value isn't available on.
        let mut available: IdMap<BlockId, Operand> = scan.available.into_iter().collect();
        if let Some(pred) = unavailable_pred {
            let new = self.insert_reload(load, align, load_bb, pred);
            available.insert(pred, new);
        }

        // A phi at the start of the block for the PRE'd load value, one input
        // per edge.
        let ty = self.function().instruction(load).ty;
        let mut inputs = Vec::new();
        for pred in self.pred_edges(load_bb) {
            let value = available[&pred];
            // An available value that needs a cast is cast in the
            // predecessor once, and all of its edges use the cast.
            let value = if self.ty_of(value) == ty {
                value
            } else {
                let cast = self.bit_or_pointer_cast(value, ty, pred);
                available.insert(pred, cast);
                cast
            };
            inputs.push((value, pred));
        }
        let first = self.function().block(load_bb).first().expect("a block with a load");
        let phi = self.unit.function.create_instruction(Opcode::Phi, ty, from_arms(&inputs), Flags::default(), None);
        self.unit.function.insert(phi, Position::Before(first)).expect("a placed block");

        for cse in scan.cse_loads {
            combine_metadata_for_cse(self.unit.metadata, self.unit.function, cse, load, true);
            self.lvi.forget_value(self.result(cse));
        }

        let (from, to) = (self.function().instruction(load).result, self.function().instruction(phi).result);
        let (from, to) = (from.expect("a load's value"), to.expect("a phi's value"));
        self.unit.function.take_name(from, to);
        self.unit.function.replace_value(from, Operand::Value(to));
        self.erase(load);
        true
    }

    /// The part of the walk over `predecessors(LoadBB)`: for each one, the
    /// value of the load at its end, looked up through single predecessors
    /// until `DefMaxInstsToScan` instructions are seen.
    fn scan_predecessors(
        &self,
        load: InstId,
        load_bb: BlockId,
    ) -> PredecessorScan {
        let function = self.function();
        let instruction = function.instruction(load);
        let (ty, loaded_ptr) = (instruction.ty, instruction.operands[0]);
        let size = self.unit.layout.store_size(&self.unit.context.types, ty);
        let aa_info = AAInfo::of(instruction);
        self.with_alias_analysis(|unit, aa| {
            let mut scan =
                PredecessorScan { available: Vec::new(), scanned: 0, one_unavailable: None, cse_loads: vec![] };
            let mut preds_scanned = IdSet::new();
            for pred in self.pred_edges(load_bb) {
                // If we already scanned this predecessor, skip it.
                if !preds_scanned.insert(pred) {
                    continue;
                }
                // If this is a load on a phi pointer, phi-translate it and
                // search for available load/store to the pointer in
                // predecessors.
                let loc = MemoryLocation {
                    ptr: self.phi_translation(loaded_ptr, load_bb, pred),
                    size: LocationSize::Precise(size as u64),
                    aa_info,
                };
                let mut cursor = None;
                let mut num_scanned = 0;
                let mut found = find_available_ptr_load_store(
                    unit,
                    &loc,
                    ty,
                    pred,
                    &mut cursor,
                    DEF_MAX_INSTS_TO_SCAN,
                    Some(&mut *aa),
                    Some(&mut num_scanned),
                );
                // If PredBB has a single predecessor, continue scanning
                // through the single predecessor.
                let mut single = Some(pred);
                while found.is_none()
                    && let Some(block) = single
                    && cursor == function.block(block).first()
                    && num_scanned < DEF_MAX_INSTS_TO_SCAN
                {
                    single = self.single_predecessor(block);
                    if let Some(block) = single {
                        cursor = None;
                        found = find_available_ptr_load_store(
                            unit,
                            &loc,
                            ty,
                            block,
                            &mut cursor,
                            DEF_MAX_INSTS_TO_SCAN - num_scanned,
                            Some(&mut *aa),
                            Some(&mut num_scanned),
                        );
                    }
                }
                let Some(Available { value, is_load_cse }) = found else {
                    scan.one_unavailable = Some(pred);
                    continue;
                };
                if is_load_cse {
                    scan.cse_loads.push(self.instruction_of(value).expect("a load"));
                }
                // This load is partially redundant: remember the value so
                // that a phi can be made.
                scan.available.push((pred, value));
            }
            scan.scanned = preds_scanned.len();
            scan
        })
    }

    /// The lambda `CanSpeculateInto`: the load is safe to execute in a
    /// predecessor, or every instruction before it passes execution on.
    /// DEVIATE J15: the context instruction (`Pred->getTerminator()`) does not
    /// change `is_safe_to_speculatively_execute_in`'s answer, so it is not
    /// passed. Reason: the pin uses it only for `llvm.assume` and the
    /// dominator tree, which `speculatable` does not read. Exit: it takes a
    /// context instruction when it reads assumptions.
    fn can_speculate_load(
        &self,
        load: InstId,
        load_bb: BlockId,
        guaranteed_to_transfer: &mut Option<bool>,
    ) -> bool {
        let unit = &self.unit;
        if crate::licm::hoist::is_safe_to_speculatively_execute_in(
            unit.context,
            unit.layout,
            self.function(),
            &self.outer,
            load,
        ) {
            return true;
        }
        *guaranteed_to_transfer.get_or_insert_with(|| {
            let function = self.function();
            let first = function.block(load_bb).first().expect("a block with a load");
            is_guaranteed_to_transfer_execution_to_successor_range(
                &self.query(),
                function.iter_range(first, load),
                TRANSFER_SCAN_LIMIT,
            )
        })
    }

    /// The load `NewVal`, inserted before the terminator of `pred`: the
    /// original's load of the phi-translated address.
    fn insert_reload(
        &mut self,
        load: InstId,
        align: Option<u64>,
        load_bb: BlockId,
        pred: BlockId,
    ) -> Operand {
        assert_eq!(self.successor_edges(pred).len(), 1, "Can't handle critical edge here!");
        let ty = self.function().instruction(load).ty;
        let ptr = self.phi_translation(self.op(load, 0), load_bb, pred);
        let name = self.value_name(load).map(|name| format!("{name}.pr"));
        let new = self
            .unit
            .function
            .create_instruction(
                Opcode::Load { align, volatile: false },
                ty,
                vec![ptr],
                Flags::default(),
                name.as_deref(),
            );
        let term = self.function().terminator(pred).expect("a terminated predecessor");
        self.unit.function.insert(new, Position::Before(term)).expect("a placed terminator");
        let tags: Vec<(String, MetadataId)> = self
            .function()
            .instruction(load)
            .metadata
            .iter()
            .filter(|(kind, _)| AA_METADATA.contains(&kind.as_str()))
            .cloned()
            .collect();
        for (kind, node) in tags {
            self.unit.function.annotate(new, &kind, node);
        }
        self.result(new)
    }

    /// The value the scan in the load's own block found replaces the load.
    fn replace_load_with_available(
        &mut self,
        load: InstId,
        available: Available,
    ) {
        let Available { mut value, is_load_cse } = available;
        if is_load_cse {
            let earlier = self.instruction_of(value).expect("a load");
            combine_metadata_for_cse(self.unit.metadata, self.unit.function, earlier, load, false);
            self.lvi.forget_value(value);
        }
        let ty = self.function().instruction(load).ty;
        // The value is the load itself only in a dead loop.
        if value == self.result(load) {
            value = Operand::Constant(self.unit.context.constant(Constant { ty, kind: ConstantKind::Poison }));
        }
        if self.ty_of(value) != ty {
            value = self.bit_or_pointer_cast_before(value, ty, load);
        }
        let result = self.function().instruction(load).result.expect("a load's value");
        self.unit.function.replace_value(result, value);
        self.erase(load);
    }

    /// `CastInst::CreateBitOrPointerCast(value, ty)` at the end of `block`,
    /// before its terminator.
    fn bit_or_pointer_cast(
        &mut self,
        value: Operand,
        ty: llrm_mir::types::TypeId,
        block: BlockId,
    ) -> Operand {
        let term = self.function().terminator(block).expect("a terminated block");
        self.bit_or_pointer_cast_before(value, ty, term)
    }

    /// `CastInst::CreateBitOrPointerCast(value, ty, before)`.
    fn bit_or_pointer_cast_before(
        &mut self,
        value: Operand,
        ty: llrm_mir::types::TypeId,
        before: InstId,
    ) -> Operand {
        let from = self.ty_of(value);
        let types = &self.unit.context.types;
        let op = match (types.get(from), types.get(ty)) {
            (Type::Pointer(_), Type::Int(_)) => CastOp::PtrToInt,
            (Type::Int(_), Type::Pointer(_)) => CastOp::IntToPtr,
            (Type::Pointer(_), Type::Pointer(_)) => CastOp::AddrSpaceCast,
            _ => CastOp::BitCast,
        };
        let cast = self.unit.function.create_instruction(Opcode::Cast(op), ty, vec![value], Flags::default(), None);
        self.unit.function.insert(cast, Position::Before(before)).expect("a placed instruction");
        self.result(cast)
    }

    /// BasicAA over the function as it stands, with its dominator tree off:
    /// the tree is updated lazily and may not be valid at this point.
    fn with_alias_analysis<R>(
        &self,
        query: impl FnOnce(&llrm_analysis::memory::Unit, &mut dyn ModRefOracle) -> R,
    ) -> R {
        let unit = llrm_analysis::memory::Unit::over(
            &*self.unit.context,
            self.unit.layout,
            self.unit.metadata,
            &self.globals,
            self.function(),
        );
        let mut basic = BasicAA::new(&unit);
        let mut batch = llrm_analysis::aa::BatchAAResults::new(&mut basic);
        batch.disable_dominator_tree();
        query(&unit, &mut batch)
    }
}
