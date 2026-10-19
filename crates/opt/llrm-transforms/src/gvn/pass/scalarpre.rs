//! Scalar PRE: an expression computed in all but one of the predecessors of a
//! join (the diamond case) is computed in that one too, and a phi takes its
//! place (`performScalarPRE`, `performScalarPREInsertion`, `performPRE`).
//!
//! A translation of
//! [`GVN.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp)
//! at d1106deb71cc, with `enable-pre` on (the default).
//!
//! DEVIATE 23, MIR has no token types, inline asm or indirect branches, so
//! those three refusals have nothing to refuse. Exit: none, a fact of MIR.

use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};

use super::GvnPass;

impl GvnPass<'_> {
    /// `assignBlockRPONumber`
    /// ([GVN.cpp:3105](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3105)).
    fn assign_block_rpo_number(&mut self) {
        let mut numbers: IdMap<BlockId, u32> = IdMap::new();
        for (next, block) in super::reverse_postorder(self.function).into_iter().enumerate() {
            numbers.insert(block, next as u32);
        }
        self.block_rpo_number = Some(numbers);
    }

    /// `performScalarPREInsertion`
    /// ([GVN.cpp:3588](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3588)):
    /// a copy of `inst` in `pred`, its operands the leaders there. The copy is
    /// made only once every operand has one, where LLVM clones first and
    /// deletes on failure.
    fn perform_scalar_pre_insertion(
        &mut self,
        inst: InstId,
        pred: BlockId,
        curr: BlockId,
    ) -> Option<InstId> {
        let mut operands = self.function.instruction(inst).operands.clone();
        // Because we are going top-down through the block, all value numbers
        // will be available in the predecessor by the time we need them. Any
        // that weren't originally present will have been instantiated earlier
        // in this loop.
        for operand in &mut operands {
            let argument = matches!(
                *operand,
                Operand::Value(value) if matches!(self.function.value(value).def, ValueDef::Argument(_))
            );
            if argument || !matches!(*operand, Operand::Value(_)) {
                continue;
            }
            // This could be a newly inserted instruction, in which case, we
            // won't find a value number, and should give up.
            if !self.vn.exists(*operand) {
                return None;
            }
            let number = self.vn.lookup(*operand, true);
            let translated = {
                let (mut env, vn, leaders) = self.env();
                vn.phi_translate(&mut env, leaders, pred, curr, number)
            };
            // Fail out if an operand is not available in the PRE predecessor.
            // This is typically because of loads which are not value numbered
            // precisely.
            *operand = self.find_leader(pred, translated)?;
        }

        let copy = self.function.clone_instruction(inst);
        self.function.set_operands(copy, operands);
        let terminator = self.function.terminator(pred).expect("a terminated predecessor");
        self.function.insert(copy, Position::Before(terminator)).expect("a placed terminator");
        let result = self.function.instruction(copy).result.expect("a value");
        let name = self.function.instruction(inst).result.and_then(|one| self.function.value(one).name.clone());
        if let Some(name) = name {
            self.function.set_name(result, &format!("{name}.pre"));
        }
        let value = Operand::Value(result);
        let number = self.lookup_or_add(value);
        self.vn.add(value, number, false, None);
        // Update the availability map to include the new instruction.
        self.leader_table.insert(number, value, pred);
        Some(copy)
    }

    /// `performScalarPRE`
    /// ([GVN.cpp:3637](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3637)).
    pub(super) fn perform_scalar_pre(
        &mut self,
        cur_inst: InstId,
    ) -> bool {
        let instruction = self.function.instruction(cur_inst);
        let Some(result) = instruction.result else { return false };
        // Don't do PRE on compares: the phi would keep CodeGenPrepare from
        // sinking the compare again. Nor on GEPs, which would keep it from
        // sinking the addressing mode back to its uses; load PRE is not
        // affected, phi translation moves the GEP to the predecessor.
        if instruction.opcode.is_terminator()
            || matches!(
                instruction.opcode,
                Opcode::Alloca { .. }
                    | Opcode::Phi
                    | Opcode::ICmp(_)
                    | Opcode::FCmp(_)
                    | Opcode::GetElementPtr { .. }
                    | Opcode::LandingPad { .. }
            )
            || llrm_mir::memory::of(self.context, self.outer.callees(), self.function, cur_inst).reads
            || crate::local::may_have_side_effects(
                self.context,
                self.outer.callees(),
                &self.outer.globals,
                self.function,
                cur_inst,
            )
        {
            return false;
        }

        let value = Operand::Value(result);
        let val_no = self.vn.lookup(value, false);

        // Look for the predecessors for PRE opportunities. We're only trying
        // to solve the basic diamond case, where a value is computed in the
        // successor and one predecessor, but not the other. We also explicitly
        // disallow cases where the successor is its own predecessor, because
        // they're more complicated to get right.
        let mut num_with = 0;
        let mut num_without = 0;
        let mut pre_pred: Option<BlockId> = None;
        let current_block = self.function.parent(cur_inst).expect("a placed instruction");

        // Update the RPO numbers for this function.
        if self.block_rpo_number.is_none() {
            self.assign_block_rpo_number();
        }

        let mut pred_map: Vec<(Option<Operand>, BlockId)> = Vec::new();
        for pred in self.function.predecessors(current_block).to_vec() {
            // We're not interested in PRE where blocks with predecessors that
            // are not reachable.
            if !self.dt.is_reachable(pred) {
                num_without = 2;
                break;
            }
            // It is not safe to do PRE when P->CurrentBlock is a loop
            // backedge.
            let numbers = self.block_rpo_number.as_ref().expect("assigned");
            let (Some(&p), Some(&c)) = (numbers.get(&pred), numbers.get(&current_block)) else {
                unreachable!("Invalid BlockRPONumber map.")
            };
            if p >= c {
                num_without = 2;
                break;
            }

            let translated = {
                let (mut env, vn, leaders) = self.env();
                vn.phi_translate(&mut env, leaders, pred, current_block, val_no)
            };
            match self.find_leader(pred, translated) {
                None => {
                    pred_map.push((None, pred));
                    pre_pred = Some(pred);
                    num_without += 1;
                }
                // CurInst dominates this predecessor.
                Some(found) if found == value => {
                    num_without = 2;
                    break;
                }
                Some(found) => {
                    pred_map.push((Some(found), pred));
                    num_with += 1;
                }
            }
        }

        // Don't do PRE when it might increase code size, i.e. when we would
        // need to insert instructions in more than one pred.
        if num_without > 1 || num_with == 0 {
            return false;
        }

        // We may have a case where all predecessors have the instruction, and
        // we just need to insert a phi node. Otherwise, perform insertion.
        let mut pre_instr: Option<InstId> = None;
        if num_without != 0 {
            let pre_pred = pre_pred.expect("one predecessor lacks it");
            if !crate::licm::hoist::is_safe_to_speculatively_execute_in(
                self.context,
                self.layout,
                self.function,
                self.outer,
                cur_inst,
            ) {
                // It is only valid to insert a new instruction if the current
                // instruction is always executed. An instruction with
                // implicit control flow could prevent us from doing it.
                if self.is_dominated_by_icf_from_same_block(cur_inst) {
                    return false;
                }
            }

            // We can't do PRE safely on a critical edge, so instead we
            // schedule the edge to be split and perform the PRE the next time
            // we iterate on the function.
            if crate::edges::is_critical_edge(self.function, pre_pred, current_block) {
                self.to_split.push((pre_pred, current_block));
                return false;
            }
            // We need to insert somewhere, so let's give it a shot.
            let Some(inserted) = self.perform_scalar_pre_insertion(cur_inst, pre_pred, current_block) else {
                return false;
            };
            pre_instr = Some(inserted);
        }

        // Create a phi to make the value available in this block.
        let ty = self.function.instruction(cur_inst).ty;
        let mut arms = Vec::new();
        for &(found, pred) in &pred_map {
            let incoming = match found {
                Some(found) => {
                    // If we use an existing value in this phi, we have to patch
                    // the original value because the phi will be used to
                    // replace a later value.
                    crate::local::patch_replacement_instruction(
                        self.context,
                        &self.outer.globals,
                        &self.outer.metadata,
                        self.function,
                        cur_inst,
                        found,
                    );
                    found
                }
                None => {
                    let inserted = pre_instr.expect("the predecessor without it got a copy");
                    Operand::Value(self.function.instruction(inserted).result.expect("a value"))
                }
            };
            arms.push(incoming);
            arms.push(Operand::Block(pred));
        }
        let name = self.function.value(result).name.clone().map(|name| format!("{name}.pre-phi"));
        let phi = self.function.create_instruction(Opcode::Phi, ty, arms, Flags::default(), name.as_deref());
        let first = self.function.iter_block(current_block).next().expect("a block holding CurInst");
        self.function.insert(phi, Position::Before(first)).expect("a placed block");
        let phi_value = Operand::Value(self.function.instruction(phi).result.expect("a phi's value"));

        self.vn.add(phi_value, val_no, true, Some(phi));
        // After creating a new phi for ValNo, the phi translate result for
        // ValNo will be changed, so erase the related stale entries in phi
        // translate cache.
        self.vn.erase_translate_cache_entry(self.function, val_no, current_block);
        self.leader_table.insert(val_no, phi_value, current_block);
        self.function.replace_all_uses_with(result, phi_value);
        self.md_invalidate_cached_pointer_info(phi_value);
        self.leader_table.erase(val_no, value, current_block);
        self.remove_instruction(cur_inst);
        true
    }

    /// `performPRE`
    /// ([GVN.cpp:3799](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3799)):
    /// a purely local form of PRE that looks for diamond control flow patterns
    /// and attempts simple PRE at the join point.
    pub(super) fn perform_pre(&mut self) -> bool {
        let mut changed = false;
        let Some(entry) = self.function.entry() else { return false };
        for current_block in depth_first(self.function, entry) {
            // Nothing to PRE in the entry block.
            if current_block == entry {
                continue;
            }
            // Don't perform PRE on an EH pad.
            if self
                .function
                .iter_block(current_block)
                .map(|inst| &self.function.instruction(inst).opcode)
                .find(|opcode| **opcode != Opcode::Phi)
                .is_some_and(|opcode| matches!(opcode, Opcode::LandingPad { .. }))
            {
                continue;
            }
            for inst in self.function.iter_block(current_block).collect::<Vec<_>>() {
                if self.function.is_erased(inst) {
                    continue;
                }
                changed |= self.perform_scalar_pre(inst);
            }
        }
        if self.split_scheduled_critical_edges() {
            changed = true;
        }
        changed
    }

    /// `splitCriticalEdges()`
    /// ([GVN.cpp:3839](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3839)):
    /// the edges `performScalarPRE` found.
    fn split_scheduled_critical_edges(&mut self) -> bool {
        let mut changed = false;
        while let Some((pred, succ)) = self.to_split.pop() {
            changed |= self.split_critical_edges(pred, succ).is_some();
        }
        changed
    }

    /// `assignValNumForDeadCode`
    /// ([GVN.cpp:4022](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L4022)).
    pub(super) fn assign_val_num_for_dead_code(&mut self) {
        let dead: Vec<BlockId> = self.dead_blocks.iter().collect();
        for block in dead {
            for inst in self.function.iter_block(block).collect::<Vec<_>>() {
                let Some(result) = self.function.instruction(inst).result else { continue };
                let value = Operand::Value(result);
                let number = self.lookup_or_add(value);
                self.leader_table.insert(number, value, block);
            }
        }
    }
}

/// `depth_first(&F.getEntryBlock())`: the blocks reachable from `entry`, each
/// before the blocks first reached through it.
fn depth_first(
    function: &llrm_mir::module::Function,
    entry: BlockId,
) -> Vec<BlockId> {
    let mut seen = llrm_mir::dense::IdSet::new();
    let mut order = Vec::new();
    let mut stack = vec![(entry, 0_usize)];
    seen.insert(entry);
    order.push(entry);
    while let Some((block, next)) = stack.last_mut() {
        let successors = function.successors(*block);
        if let Some(&successor) = successors.get(*next) {
            *next += 1;
            if seen.insert(successor) {
                order.push(successor);
                stack.push((successor, 0));
            }
        } else {
            stack.pop();
        }
    }
    order
}
