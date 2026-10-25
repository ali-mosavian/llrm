//! `SSAUpdater`: a value defined in several blocks, rewritten so each use reads
//! the definition that reaches it, with a phi placed where definitions meet.
//!
//! # How it works
//! The client says where the value is defined (`add_available_value`), then
//! rewrites each use (`rewrite_use`). A use in a block with no definition asks
//! for the value at the end of its block: `Impl::get_value` walks back through
//! the predecessors to the blocks that define the value, numbers the blocks
//! it reached in postorder, finds their dominators within that subgraph
//! (Cooper, Harvey and Kennedy), places a phi in the iterated dominance
//! frontier of the definitions, reuses a phi the block already has that
//! matches, and fills in the new phis' inputs. Every block it reached
//! remembers the value that reaches its end, so the next use in it is a lookup.
//!
//! A use in a block that already holds a definition (or a remembered value)
//! asks for the value in the middle of the block, which merges what the
//! predecessors give; a phi it makes that simplifies is not made.
//!
//! This is the translation LLVM's `LoopRotate` uses. [`crate::ssa::SsaUpdater`]
//! is the simpler updater the other passes were written against; it places a
//! phi where a loop's back edge meets the value and removes it by walking, and
//! may leave a phi LLVM's does not, or the reverse.
//!
//! # Example
//! A value `%v` defined in `b1` and `b2`, used in `b4` below them:
//! ```mir
//! b0: br i1 %c, label %b1, label %b2
//! b1: %v1 = ...    br label %b4
//! b2: %v2 = ...    br label %b4
//! b4: use %v
//! ```
//! `add_available_value(b1, %v1)`, `add_available_value(b2, %v2)`,
//! `rewrite_use` of the use in `b4` places `%v = phi [ %v1, %b1 ], [ %v2, %b2
//! ]` in `b4` and reads it.
//!
//! # Deviations
//! - S1 No debug records: `UpdateDebugValues` and `insertDebugValuesForPHIs`
//!   are not translated (R4 of `looprotate`).
//! - S2 The `SSAUpdaterPhiSearchLimit` is the default, 80, with no option.
//!
//! # See also
//! [`SSAUpdater`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdater.cpp),
//! [`SSAUpdaterImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/SSAUpdaterImpl.h).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::inverse_children;
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::TypeId;

/// `SSAUpdaterPhiSearchLimit`.
const PHI_SEARCH_LIMIT: u32 = 80;

/// `SSAUpdater`.
#[derive(Clone, Debug)]
pub struct SsaUpdater {
    proto_type: TypeId,
    proto_name: Option<String>,
    /// `AV`: the value that reaches the end of each block.
    available: IdMap<BlockId, Operand>,
    /// `InsertedPHIs`.
    inserted: Vec<InstId>,
}

/// `BBInfo`: what one block that `Impl` reached knows.
struct BbInfo {
    bb: Option<BlockId>,
    available_val: Option<Operand>,
    def_bb: Option<usize>,
    blk_num: i32,
    idom: Option<usize>,
    /// `Preds`: the range of [`Impl::pool`] holding the infos of the
    /// predecessors.
    preds: std::ops::Range<usize>,
    phi_tag: Option<InstId>,
}

/// `SSAUpdaterImpl`.
struct Impl<'a> {
    updater: &'a mut SsaUpdater,
    context: &'a mut Context,
    function: &'a mut Function,
    infos: Vec<BbInfo>,
    /// The predecessors of every info, end to end: one buffer for the walk,
    /// not a vector for each block it reaches.
    pool: Vec<usize>,
    bb_map: IdMap<BlockId, usize>,
}

impl SsaUpdater {
    /// `SSAUpdater::Initialize`: a value of type `ty`; its phis are named
    /// `name`.
    pub fn new(
        ty: TypeId,
        name: Option<&str>,
    ) -> Self {
        Self { proto_type: ty, proto_name: name.map(str::to_owned), available: IdMap::new(), inserted: Vec::new() }
    }

    /// `AddAvailableValue`.
    pub fn add_available_value(
        &mut self,
        block: BlockId,
        value: Operand,
    ) {
        self.available.insert(block, value);
    }

    /// `HasValueForBlock`.
    pub fn has_value_for_block(
        &self,
        block: BlockId,
    ) -> bool {
        self.available.contains_key(&block)
    }

    /// `GetValueAtEndOfBlock`.
    pub fn get_value_at_end_of_block(
        &mut self,
        context: &mut Context,
        function: &mut Function,
        block: BlockId,
    ) -> Operand {
        if let Some(&value) = self.available.get(&block) {
            return value;
        }
        Impl {
            updater: self,
            context,
            function,
            infos: Vec::with_capacity(16),
            pool: Vec::with_capacity(32),
            bb_map: IdMap::new(),
        }
        .get_value(block)
    }

    /// `GetValueInMiddleOfBlock`.
    pub fn get_value_in_middle_of_block(
        &mut self,
        context: &mut Context,
        layout: &DataLayout,
        function: &mut Function,
        block: BlockId,
    ) -> Operand {
        // If there is no definition of the renamed variable in this block, just
        // use `get_value_at_end_of_block` to do the work.
        if !self.has_value_for_block(block) {
            return self.get_value_at_end_of_block(context, function, block);
        }

        // Otherwise, we have the hard case. Get the live-in values for each
        // predecessor.
        let mut pred_values: Vec<(BlockId, Operand)> = Vec::new();
        let mut singular: Option<Operand> = None;
        // The predecessor list of a block that holds a phi is read from the
        // phi.
        let first = function.block(block).first();
        let preds: Vec<BlockId> = match first {
            Some(some_phi) if function.instruction(some_phi).opcode == Opcode::Phi => function
                .instruction(some_phi)
                .operands
                .iter()
                .skip(1)
                .step_by(2)
                .map(|one| match one {
                    Operand::Block(block) => *block,
                    _ => unreachable!("a phi's block operand"),
                })
                .collect(),
            _ => inverse_children(function, block).collect(),
        };
        for (at, pred) in preds.into_iter().enumerate() {
            let value = self.get_value_at_end_of_block(context, function, pred);
            pred_values.push((pred, value));
            // Compute the singular value.
            if at == 0 {
                singular = Some(value);
            } else if singular != Some(value) {
                singular = None;
            }
        }

        // If there are no predecessors, just return poison.
        if pred_values.is_empty() {
            return self.poison(context);
        }

        // Otherwise, if all the merged values are the same, just use it.
        if let Some(singular) = singular {
            return singular;
        }

        // Otherwise, we do need a phi: check to see if we already have one
        // available in this block that produces the right value.
        let mut mapping = IdMap::<BlockId, Operand>::new();
        for &(pred, value) in &pred_values {
            mapping.insert(pred, value);
        }
        if first.is_some_and(|one| function.instruction(one).opcode == Opcode::Phi) {
            for phi in phis_of(function, block) {
                if is_equivalent_phi(function, phi, &mapping) {
                    return Operand::Value(function.instruction(phi).result.expect("a phi's value"));
                }
            }
        }

        // We have no way out, insert a new one now.
        let ty = self.proto_type;
        let inserted =
            function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), self.proto_name.as_deref());
        let first = function.block(block).first();
        function.insert(inserted, first.map_or(Position::End(block), Position::Before)).expect("a placed block");
        let mut operands = Vec::new();
        for &(pred, value) in &pred_values {
            operands.push(value);
            operands.push(Operand::Block(pred));
        }
        function.set_operands(inserted, operands);

        // See if the phi can be merged to a single value. This can happen in
        // loop cases when we get a phi of itself and one other value.
        if let Some(value) = llrm_mir::simplify::simplify_instruction(context, layout, function, inserted) {
            let result = function.instruction(inserted).result.expect("a phi's value");
            function.replace_value(result, value);
            function.erase(inserted).expect("a replaced phi");
            return value;
        }

        self.inserted.push(inserted);
        Operand::Value(function.instruction(inserted).result.expect("a phi's value"))
    }

    /// `RewriteUse`.
    pub fn rewrite_use(
        &mut self,
        context: &mut Context,
        layout: &DataLayout,
        function: &mut Function,
        one: Use,
    ) {
        let user = function.instruction(one.user);
        let value = if user.opcode == Opcode::Phi {
            let Operand::Block(from) = user.operands[one.index as usize + 1] else {
                unreachable!("a phi's block operand")
            };
            self.get_value_at_end_of_block(context, function, from)
        } else {
            let block = function.parent(one.user).expect("a placed user");
            self.get_value_in_middle_of_block(context, layout, function, block)
        };
        function.set_operand(one.user, one.index as usize, value);
    }

    /// The phis placed so far that stand.
    pub fn inserted_phis(&self) -> &[InstId] {
        &self.inserted
    }

    fn poison(
        &self,
        context: &Context,
    ) -> Operand {
        Operand::Constant(context.constant(Constant { ty: self.proto_type, kind: ConstantKind::Poison }))
    }
}

/// The phis a block begins with.
fn phis_of(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

/// `IsEquivalentPHI`.
fn is_equivalent_phi(
    function: &Function,
    phi: InstId,
    mapping: &IdMap<BlockId, Operand>,
) -> bool {
    let operands = &function.instruction(phi).operands;
    if operands.len() / 2 != mapping.len() {
        return false;
    }
    operands
        .chunks(2)
        .all(
            |pair| match pair[1] {
                Operand::Block(block) => mapping.get(&block) == Some(&pair[0]),
                _ => false,
            },
        )
}

impl Impl<'_> {
    fn new_info(
        &mut self,
        bb: Option<BlockId>,
        available_val: Option<Operand>,
    ) -> usize {
        self.infos.push(BbInfo { bb, available_val, def_bb: None, blk_num: 0, idom: None, preds: 0..0, phi_tag: None });
        let at = self.infos.len() - 1;
        if available_val.is_some() {
            self.infos[at].def_bb = Some(at);
        }
        at
    }

    /// `GetValue`.
    fn get_value(
        &mut self,
        bb: BlockId,
    ) -> Operand {
        let mut block_list = Vec::new();
        let pseudo_entry = self.build_block_list(bb, &mut block_list);

        // Special case: bail out if the block is unreachable.
        if block_list.is_empty() {
            let value = self.updater.poison(self.context);
            self.updater.available.insert(bb, value);
            return value;
        }

        self.find_dominators(&block_list, pseudo_entry);
        self.find_phi_placement(&block_list);
        self.find_available_vals(&block_list);

        let at = *self.bb_map.get(&bb).expect("a reached block");
        let def = self.infos[at].def_bb.expect("a defining block");
        self.infos[def].available_val.expect("an available value")
    }

    /// The next of the blocks `block` is entered from, one for each edge, or
    /// of the blocks of a phi it begins with: the `cursor`th, or none.
    fn next_predecessor(
        &self,
        block: BlockId,
        cursor: &mut usize,
    ) -> Option<BlockId> {
        let first = self.function.block(block).first();
        match first {
            Some(some_phi) if self.function.instruction(some_phi).opcode == Opcode::Phi => {
                let operands = &self.function.instruction(some_phi).operands;
                match operands.get(1 + 2 * *cursor) {
                    Some(Operand::Block(block)) => {
                        *cursor += 1;
                        Some(*block)
                    }
                    Some(_) => unreachable!("a phi's block operand"),
                    None => None,
                }
            }
            _ => {
                let at = *cursor;
                *cursor += 1;
                inverse_children(self.function, block).nth(at)
            }
        }
    }

    /// `BuildBlockList`: from `bb`, back through its predecessors to the blocks
    /// that define the value.
    fn build_block_list(
        &mut self,
        bb: BlockId,
        block_list: &mut Vec<usize>,
    ) -> usize {
        let mut root_list: Vec<usize> = Vec::new();
        let mut work_list: Vec<usize> = Vec::new();

        let info = self.new_info(Some(bb), None);
        self.bb_map.insert(bb, info);
        work_list.push(info);

        // Search backward from the block, creating infos along the way and
        // stopping when reaching blocks that define the value. Record those
        // defining blocks on the root list.
        while let Some(info) = work_list.pop() {
            let start = self.pool.len();
            let block = self.infos[info].bb.expect("a block");
            let mut cursor = 0;
            while let Some(pred) = self.next_predecessor(block, &mut cursor) {
                // Check if the map already has an info for the predecessor.
                if let Some(&existing) = self.bb_map.get(&pred) {
                    self.pool.push(existing);
                    continue;
                }
                // Create a new info for the predecessor.
                let pred_val = self.updater.available.get(&pred).copied();
                let pred_info = self.new_info(Some(pred), pred_val);
                self.bb_map.insert(pred, pred_info);
                self.pool.push(pred_info);
                if self.infos[pred_info].available_val.is_some() {
                    root_list.push(pred_info);
                    continue;
                }
                work_list.push(pred_info);
            }
            self.infos[info].preds = start..self.pool.len();
        }

        // Now that we know what blocks are backwards-reachable from the
        // starting block, do a forward depth-first traversal to assign
        // postorder numbers to those blocks.
        let pseudo_entry = self.new_info(None, None);
        let mut blk_num: i32 = 1;

        // Initialize the worklist with the roots from the backward traversal.
        while let Some(info) = root_list.pop() {
            self.infos[info].idom = Some(pseudo_entry);
            self.infos[info].blk_num = -1;
            work_list.push(info);
        }

        while let Some(&info) = work_list.last() {
            if self.infos[info].blk_num == -2 {
                // All the successors have been handled; assign the postorder
                // number. If not a root, put it on the block list.
                self.infos[info].blk_num = blk_num;
                blk_num += 1;
                if self.infos[info].available_val.is_none() {
                    block_list.push(info);
                }
                work_list.pop();
                continue;
            }

            // Leave this entry on the worklist, but set its number to mark that
            // its successors have been put on the worklist. When it returns to
            // the top of the list, after handling its successors, it will be
            // assigned a number.
            self.infos[info].blk_num = -2;

            // Add unvisited successors to the work list.
            let block = self.infos[info].bb.expect("a block");
            let mut cursor = 0;
            while let Some(succ) = llrm_mir::loopinfo::LoopGraph::child(&*self.function, block, &mut cursor) {
                let Some(&succ_info) = self.bb_map.get(&succ) else { continue };
                if self.infos[succ_info].blk_num != 0 {
                    continue;
                }
                self.infos[succ_info].blk_num = -1;
                work_list.push(succ_info);
            }
        }
        self.infos[pseudo_entry].blk_num = blk_num;
        pseudo_entry
    }

    /// `IntersectDominators`: the common dominator of two blocks, by their
    /// postorder numbers.
    fn intersect_dominators(
        &self,
        mut blk1: usize,
        mut blk2: usize,
    ) -> usize {
        while blk1 != blk2 {
            while self.infos[blk1].blk_num < self.infos[blk2].blk_num {
                match self.infos[blk1].idom {
                    Some(idom) => blk1 = idom,
                    None => return blk2,
                }
            }
            while self.infos[blk2].blk_num < self.infos[blk1].blk_num {
                match self.infos[blk2].idom {
                    Some(idom) => blk2 = idom,
                    None => return blk1,
                }
            }
        }
        blk1
    }

    /// `FindDominators`.
    fn find_dominators(
        &mut self,
        block_list: &[usize],
        pseudo_entry: usize,
    ) {
        loop {
            let mut changed = false;
            // Iterate over the list in reverse order, i.e., forward on CFG
            // edges.
            for &info in block_list.iter().rev() {
                let mut new_idom: Option<usize> = None;

                // Iterate through the block's predecessors.
                for p in self.infos[info].preds.clone() {
                    let pred = self.pool[p];

                    // Treat an unreachable predecessor as a definition with
                    // poison.
                    if self.infos[pred].blk_num == 0 {
                        let value = self.updater.poison(self.context);
                        self.infos[pred].available_val = Some(value);
                        self.updater.available.insert(self.infos[pred].bb.expect("a block"), value);
                        self.infos[pred].def_bb = Some(pred);
                        self.infos[pred].blk_num = self.infos[pseudo_entry].blk_num;
                        self.infos[pseudo_entry].blk_num += 1;
                    }

                    new_idom = Some(match new_idom {
                        None => pred,
                        Some(one) => self.intersect_dominators(one, pred),
                    });
                }

                // Check if the idom value has changed.
                if new_idom.is_some() && new_idom != self.infos[info].idom {
                    self.infos[info].idom = new_idom;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// `IsDefInDomFrontier`: whether a block between `pred` and `idom` in the
    /// dominator tree holds a definition.
    fn is_def_in_dom_frontier(
        &self,
        mut pred: usize,
        idom: usize,
    ) -> bool {
        while pred != idom {
            if self.infos[pred].def_bb == Some(pred) {
                return true;
            }
            pred = self.infos[pred].idom.expect("a dominator below the immediate dominator");
        }
        false
    }

    /// `FindPHIPlacement`.
    fn find_phi_placement(
        &mut self,
        block_list: &[usize],
    ) {
        loop {
            let mut changed = false;
            // Iterate over the list in reverse order, i.e., forward on CFG
            // edges.
            for &info in block_list.iter().rev() {
                // If this block already needs a phi, there is nothing to do
                // here.
                if self.infos[info].def_bb == Some(info) {
                    continue;
                }

                // Default to use the same def as the immediate dominator.
                let idom = self.infos[info].idom.expect("an immediate dominator");
                let mut new_def_bb = self.infos[idom].def_bb;
                for p in self.infos[info].preds.clone() {
                    if self.is_def_in_dom_frontier(self.pool[p], idom) {
                        // Need a phi here.
                        new_def_bb = Some(info);
                        break;
                    }
                }

                // Check if anything changed.
                if new_def_bb != self.infos[info].def_bb {
                    self.infos[info].def_bb = new_def_bb;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// `FindSingularVal`: if all predecessors have the same available value, it
    /// is the value for the block.
    fn find_singular_val(
        &mut self,
        info: usize,
    ) -> bool {
        let preds = self.infos[info].preds.clone();
        let Some(&first) = self.pool[preds.clone()].first() else { return false };
        let def_val = |this: &Self, pred: usize| this.infos[pred].def_bb.and_then(|def| this.infos[def].available_val);
        let Some(singular) = def_val(self, first) else { return false };
        for &pred in &self.pool[preds][1..] {
            if def_val(self, pred) != Some(singular) {
                return false;
            }
        }
        // Record the singular value.
        self.updater.available.insert(self.infos[info].bb.expect("a block"), singular);
        self.infos[info].available_val = Some(singular);
        self.infos[info].def_bb = self.infos[first].def_bb;
        true
    }

    /// `FindAvailableVals`.
    fn find_available_vals(
        &mut self,
        block_list: &[usize],
    ) {
        // Go through the worklist in forward order (i.e., backward through the
        // CFG) and check if existing phis can be used. If not, create empty
        // phis where they are needed.
        for &info in block_list {
            // Check if there needs to be a phi in the block.
            if self.infos[info].def_bb != Some(info) {
                continue;
            }

            // Look for a singular value.
            if self.find_singular_val(info) {
                continue;
            }

            // Look for an existing phi.
            self.find_existing_phi(self.infos[info].bb.expect("a block"));
            if self.infos[info].available_val.is_some() {
                continue;
            }

            let block = self.infos[info].bb.expect("a block");
            let ty = self.updater.proto_type;
            let phi = self
                .function
                .create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), self.updater.proto_name.as_deref());
            let first = self.function.block(block).first();
            self.function.insert(phi, first.map_or(Position::End(block), Position::Before)).expect("a placed block");
            let value = Operand::Value(self.function.instruction(phi).result.expect("a phi's value"));
            self.infos[info].available_val = Some(value);
            self.updater.available.insert(block, value);
        }

        // Now go back through the worklist in reverse order to fill in the
        // arguments for any new phis added in the forward traversal.
        for &info in block_list.iter().rev() {
            if self.infos[info].def_bb != Some(info) {
                // Record the available value to speed up subsequent uses of
                // this updater for the same value.
                let def = self.infos[info].def_bb.expect("a defining block");
                let value = self.infos[def].available_val.expect("an available value");
                self.updater.available.insert(self.infos[info].bb.expect("a block"), value);
                continue;
            }

            // Check if this block contains a newly added phi.
            let Some(Operand::Value(value)) = self.infos[info].available_val else { continue };
            let llrm_mir::module::ValueDef::Instruction(phi) = self.function.value(value).def else { continue };
            if self.function.instruction(phi).opcode != Opcode::Phi
                || !self.function.instruction(phi).operands.is_empty()
            {
                continue;
            }

            // Iterate through the block's predecessors.
            let mut operands = Vec::new();
            for p in self.infos[info].preds.clone() {
                let mut pred_info = self.pool[p];
                let pred = self.infos[pred_info].bb.expect("a block");
                // Skip to the nearest preceding definition.
                if self.infos[pred_info].def_bb != Some(pred_info) {
                    pred_info = self.infos[pred_info].def_bb.expect("a defining block");
                }
                operands.push(self.infos[pred_info].available_val.expect("an available value"));
                operands.push(Operand::Block(pred));
            }
            self.function.set_operands(phi, operands);
            self.updater.inserted.push(phi);
        }
    }

    /// `FindExistingPHI`: looks through the phis in a block to see if any of
    /// them match what is needed.
    fn find_existing_phi(
        &mut self,
        bb: BlockId,
    ) {
        let mut tagged: Vec<usize> = Vec::new();
        // The search limit guards against pathological cases where a large
        // number of searches all fail.
        let mut count = 0;
        for phi in phis_of(self.function, bb) {
            count += 1;
            if count > PHI_SEARCH_LIMIT {
                break;
            }
            if self.check_if_phi_matches(phi, &mut tagged) {
                self.record_matching_phis(&tagged);
                break;
            }
        }
    }

    /// `CheckIfPHIMatches`: whether a phi matches the placement and values
    /// found.
    fn check_if_phi_matches(
        &mut self,
        phi: InstId,
        tagged: &mut Vec<usize>,
    ) -> bool {
        let matched = self.matches(phi, tagged);
        if !matched {
            // Match failed: clear all the tags of the blocks visited.
            for &block in tagged.iter() {
                self.infos[block].phi_tag = None;
            }
            tagged.clear();
        }
        matched
    }

    fn matches(
        &mut self,
        phi: InstId,
        tagged: &mut Vec<usize>,
    ) -> bool {
        let mut work_list = vec![phi];

        // Mark that the block containing this phi has been visited.
        let Some(&phi_block) = self.function.parent(phi).and_then(|block| self.bb_map.get(&block)) else {
            return false;
        };
        self.infos[phi_block].phi_tag = Some(phi);
        tagged.push(phi_block);

        while let Some(phi) = work_list.pop() {
            // Iterate through the phi's incoming values.
            let operands = self.function.instruction(phi).operands.clone();
            for pair in operands.chunks(2) {
                let incoming_val = pair[0];
                let Operand::Block(incoming_block) = pair[1] else { return false };
                let Some(&mut_info) = self.bb_map.get(&incoming_block) else { return false };
                let mut pred_info = mut_info;
                // Skip to the nearest preceding definition.
                if self.infos[pred_info].def_bb != Some(pred_info) {
                    match self.infos[pred_info].def_bb {
                        Some(def) => pred_info = def,
                        None => return false,
                    }
                }

                // Check if it matches the expected value.
                if let Some(available) = self.infos[pred_info].available_val {
                    if incoming_val == available {
                        continue;
                    }
                    return false;
                }

                // Check if the value is a phi in the correct block.
                let Operand::Value(value) = incoming_val else { return false };
                let llrm_mir::module::ValueDef::Instruction(incoming_phi) = self.function.value(value).def else {
                    return false;
                };
                if self.function.instruction(incoming_phi).opcode != Opcode::Phi
                    || self.function.parent(incoming_phi) != self.infos[pred_info].bb
                {
                    return false;
                }

                // If this block has already been visited, check if this phi
                // matches.
                if let Some(tag) = self.infos[pred_info].phi_tag {
                    if incoming_phi == tag {
                        continue;
                    }
                    return false;
                }
                self.infos[pred_info].phi_tag = Some(incoming_phi);
                tagged.push(pred_info);

                work_list.push(incoming_phi);
            }
        }
        // A match was found: keep the tags.
        true
    }

    /// `RecordMatchingPHIs`: for each phi that matches, record it in both maps.
    fn record_matching_phis(
        &mut self,
        tagged: &[usize],
    ) {
        for &block in tagged {
            let phi = self.infos[block].phi_tag.expect("a tagged phi");
            let bb = self.function.parent(phi).expect("a placed phi");
            let value = Operand::Value(self.function.instruction(phi).result.expect("a phi's value"));
            self.updater.available.insert(bb, value);
            let at = *self.bb_map.get(&bb).expect("a reached block");
            self.infos[at].available_val = Some(value);
        }
    }
}
