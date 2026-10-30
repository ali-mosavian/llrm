//! `LoopVersioning`: a loop copied, the copy kept for when run-time checks
//! find that pointers may overlap, and the original told that they do not.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopVersioning.cpp`. `version_loop`
//! needs a loop in simplify and LCSSA form with one exit block. It
//! 1. writes the memory checks of [`crate::runtimechecks`] into the preheader,
//!    which becomes `<header>.lver.check`;
//! 2. splits off a new preheader and clones the loop before it
//!    (`<name>.lver.orig`);
//! 3. ends the check block in a branch on the checks: true enters the clone,
//!    the fall-back, false the original, the versioned loop;
//! 4. gives the exit block a phi for each value the loop defines and uses
//!    outside, with an arm from each loop, and makes each loop's exits
//!    dedicated again.
//!
//! `annotate_loop_with_noalias` then gives every memory instruction of the
//! versioned loop the scope of its checked pointer group and the scopes of
//! the groups it was checked against as `noalias`.
//!
//! # Example
//! `a[i] = b[i]`: `loop.lver.check` holds `a < b + 4n && b < a + 4n`; if true
//! control goes to `loop.ph.lver.orig`, else to `loop.ph`. In `loop` the load
//! reads `!alias.scope !0` and the store `!alias.scope !3, !noalias !0`.
//!
//! # DEVIATE 29 (no SCEV predicates)
//! The checks are the memory checks only: `LAI.getPSE().getPredicate()` is
//! always true here (D1), so there is no `expandCodeForPredicate` and no
//! `lver.safe`. `version_loop` asserts there is a memory check, the case
//! where LLVM's predicate alone would be the check. Exit: SCEV predicates.
//!
//! # DEVIATE 30 (no analysis update)
//! The dominator tree, the loop info and `MemorySSA` are not updated
//! (`MSSAU`, the `DT` and `LI` arguments of `SplitBlock`,
//! `cloneLoopWithPreheader` and `formDedicatedExitBlocks`): the caller finds
//! them again, as `simpleloopunswitch` does (SU7). The new loop's blocks are
//! in [`LoopVersioning::non_versioned_blocks`]. Exit: updatable `LoopInfo` and
//! `DomTree`. Boundary: `loopversioning_tests` finds both again and checks
//! that each loop is in simplify form and the module verifies.
//!
//! # DEVIATE 31 (no entry count)
//! The new branch is not marked with unknown weights
//! (`setExplicitlyUnknownBranchWeightsIfProfiled`): MIR has no function
//! entry count, so LLVM would not either. Exit: entry counts in MIR.
//!
//! # DEVIATE 32 (SCEV is the caller's)
//! `forgetLcssaPhiWithNewPredecessor` is not called: the CFG changed, so the
//! caller drops its expressions (`ScevSession::forget_expressions`).
//!
//! # Runs
//! Called by the loop vectorizer and loop distribution once they are
//! translated; not a pass here (`LoopVersioningPass` is a unit-test shim).

use llrm_analysis::runtimecheck::{RuntimePointerCheck, RuntimePointerChecking};
use llrm_analysis::scev::expander::Expander;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::module::{BlockId, Function, InstId, MetadataId, MetadataNode, MetadataOperand, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_support::hash::HashMap;

use crate::edges;
use crate::lcssa::{arms, from_arms};
use crate::loopclone::clone_loop_with_preheader;
use crate::runtimechecks::Rewriter;

/// `-loop-version-annotate-no-alias`, at its default.
const ANNOTATE_NO_ALIAS: bool = true;

/// `LoopVersioning`.
pub struct LoopVersioning<'a> {
    /// The header of the original loop, the "versioned" one: control flows
    /// there if the pointers do not alias.
    versioned_loop: BlockId,
    /// The fall-back loop's blocks, its preheader first: control flows there
    /// if the memchecks fail.
    non_versioned_blocks: Vec<BlockId>,
    /// Maps the instructions of the versioned loop to their counterpart in
    /// the non-versioned one.
    vmap: ValueMap,
    /// The set of alias checks that we are versioning for.
    alias_checks: Vec<RuntimePointerCheck>,
    rtc: &'a RuntimePointerChecking,
    /// `LAI.getDepChecker().getMemoryInstructions()`.
    memory_instructions: &'a [InstId],
    /// Maps a pointer to the pointer checking group that it belongs to.
    ptr_to_group: HashMap<Operand, usize>,
    /// The alias scope corresponding to a pointer checking group.
    group_to_scope: HashMap<usize, MetadataId>,
    /// The list of alias scopes that a pointer checking group can't alias.
    group_to_non_aliasing_scope_list: HashMap<usize, MetadataId>,
    /// The tuples made, which `MDNode::get` makes once.
    tuples: HashMap<Vec<MetadataOperand>, MetadataId>,
}

impl<'a> LoopVersioning<'a> {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L52
    /// `LoopVersioning(LAI, Checks, L, ...)` for the loop headed by `header`:
    /// `rtc` is `LAI.getRuntimePointerChecking()` and `memory_instructions`
    /// what `LAI.getDepChecker().getMemoryInstructions()` lists.
    pub fn new(
        rtc: &'a RuntimePointerChecking,
        checks: &[RuntimePointerCheck],
        memory_instructions: &'a [InstId],
        header: BlockId,
    ) -> Self {
        Self {
            versioned_loop: header,
            non_versioned_blocks: Vec::new(),
            vmap: ValueMap::default(),
            alias_checks: checks.to_vec(),
            rtc,
            memory_instructions,
            ptr_to_group: HashMap::default(),
            group_to_scope: HashMap::default(),
            group_to_non_aliasing_scope_list: HashMap::default(),
            tuples: HashMap::default(),
        }
    }

    /// `getVersionedLoop`: the header of the loop control flows to if all
    /// the memchecks pass.
    pub fn versioned_loop(&self) -> BlockId {
        self.versioned_loop
    }

    /// The blocks of `getNonVersionedLoop`, its cloned preheader first;
    /// empty before `version_loop`.
    pub fn non_versioned_blocks(&self) -> &[BlockId] {
        &self.non_versioned_blocks
    }

    /// The header of `getNonVersionedLoop`.
    pub fn non_versioned_header(&self) -> BlockId {
        self.vmap.blocks[&self.versioned_loop]
    }

    /// `versionLoop()`: `versionLoop` with `findDefsUsedOutsideOfLoop`.
    pub fn version_loop(
        &mut self,
        rw: &mut Rewriter,
    ) {
        let env = rw.env;
        let lp = env.loops.get_loop_for(self.versioned_loop).expect("the versioned loop");
        let defs = find_defs_used_outside_of_loop(rw.function, lp);
        self.version_loop_with(rw, &defs);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L66
    /// `versionLoop(DefsUsedOutside)`: the CFG manipulation part of
    /// versioning the loop. The original loop is the versioned one, and
    /// receives control if all the memchecks pass.
    pub fn version_loop_with(
        &mut self,
        rw: &mut Rewriter,
        defs_used_outside: &[InstId],
    ) {
        let env = rw.env;
        let lp = env.loops.get_loop_for(self.versioned_loop).expect("the versioned loop");
        assert!(lp.get_unique_exit_block(&*rw.function).is_some(), "No single exit block");
        assert!(lp.is_loop_simplify_form(&*rw.function), "Loop is not in loop-simplify form");
        // Assert that the loop is in LCSSA form. This is a precondition of
        // versioning.
        debug_assert!(is_recursively_lcssa_form(rw.function, env.loops, env.tree, lp), "Loop is not in LCSSA form");
        assert!(!self.alias_checks.is_empty(), "called even though we don't need any runtime checks (D26)");

        // Add the memcheck in the original preheader (this is empty
        // initially).
        let runtime_check_bb = lp.get_loop_preheader(&*rw.function).expect("a preheader");
        let terminator = rw.function.terminator(runtime_check_bb).expect("a terminated preheader");
        let mut exp = Expander::new("induction");
        let runtime_check = rw
            .add_runtime_checks(&mut exp, terminator, self.versioned_loop, self.rtc, &self.alias_checks, false)
            .expect("called even though we don't need any runtime checks");

        // Rename the block to make the IR more readable.
        let header_name = rw.function.block(self.versioned_loop).name.clone().unwrap_or_default();
        rw.function.set_block_name(runtime_check_bb, &format!("{header_name}.lver.check"));

        // Create empty preheader for the loop (and after cloning for the
        // non-versioned loop).
        let ph = edges::split_block_at(rw.function, runtime_check_bb, terminator);
        rw.function.set_block_name(ph, &format!("{header_name}.ph"));
        rw.function.move_block_after(ph, runtime_check_bb).expect("a placed block");

        // Clone the loop including the preheader.
        //
        // FIXME: This does not currently preserve SimplifyLoop because the
        // exit block is a join between the two loops.
        self.non_versioned_blocks = clone_loop_with_preheader(rw.function, ph, lp, ".lver.orig", &mut self.vmap);
        for &block in &self.non_versioned_blocks {
            for inst in rw.function.iter_block(block).collect::<Vec<_>>() {
                rw.function.remap_instruction(inst, &self.vmap);
            }
        }

        // Insert the conditional branch based on the result of the memchecks.
        let orig_term = rw.function.terminator(runtime_check_bb).expect("a terminated check block");
        let void = rw.function.instruction(orig_term).ty;
        let non_versioned_ph = self.vmap.blocks[&ph];
        let branch = rw
            .function
            .create_instruction(
                Opcode::Br,
                void,
                vec![runtime_check, Operand::Block(non_versioned_ph), Operand::Block(ph)],
                Flags::default(),
                None,
            );
        rw.function.insert(branch, Position::Before(orig_term)).expect("a placed terminator");
        rw.function.set_operands(orig_term, Vec::new());
        rw.function.erase(orig_term).expect("the old jump");

        // Adds the necessary PHI nodes for the versioned loops based on the
        // loop-defined values used outside of the loop.
        self.add_phi_nodes(rw, lp, defs_used_outside);
        form_dedicated_exit_blocks(rw.function, &self.non_versioned_blocks[1..]);
        form_dedicated_exit_blocks(rw.function, lp.get_blocks());
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L177
    /// `addPHINodes(DefsUsedOutside)`: a phi in the exit block for each value
    /// the loop defines and reads outside, with an arm from each loop.
    fn add_phi_nodes(
        &mut self,
        rw: &mut Rewriter,
        lp: Loop,
        defs_used_outside: &[InstId],
    ) {
        let phi_block = lp.get_exit_block(&*rw.function).expect("No single successor to loop exit block");
        let exiting = lp.get_exiting_block(&*rw.function).expect("an exiting block");

        // First add a single-operand PHI for each DefsUsedOutside if one does
        // not exists yet.
        for &inst in defs_used_outside {
            let Some(def) = rw.function.instruction(inst).result else { continue };
            // See if we have a single-operand PHI with the value defined by
            // the original loop.
            let found = edges::phis(rw.function, phi_block)
                .into_iter()
                .any(|phi| arms(rw.function, phi)[0].0 == Operand::Value(def));
            // If not create it.
            if !found {
                let ty = rw.function.instruction(inst).ty;
                let name = rw.function.value(def).name.clone().unwrap_or_default();
                let phi = rw
                    .function
                    .create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some(&format!("{name}.lver")));
                let first = rw.function.iter_block(phi_block).next().expect("a terminated block");
                rw.function.insert(phi, Position::Before(first)).expect("a placed block");
                let pn = Operand::Value(rw.function.instruction(phi).result.expect("a phi's value"));
                let mut users_to_update: Vec<InstId> = Vec::new();
                for used in rw.function.users(def) {
                    let user = used.user;
                    let outside = rw.function.parent(user).is_some_and(|block| !lp.contains(block));
                    if outside && !users_to_update.contains(&user) {
                        users_to_update.push(user);
                    }
                }
                for user in users_to_update {
                    let operands = rw
                        .function
                        .instruction(user)
                        .operands
                        .iter()
                        .map(|&operand| if operand == Operand::Value(def) { pn } else { operand })
                        .collect();
                    rw.function.set_operands(user, operands);
                }
                rw.function.set_operands(phi, from_arms(&[(Operand::Value(def), exiting)]));
            }
        }

        // Then for each PHI add the operand for the edge from the cloned loop.
        let non_versioned_exiting = self.vmap.blocks[&exiting];
        for phi in edges::phis(rw.function, phi_block) {
            let mut incoming = arms(rw.function, phi);
            assert_eq!(incoming.len(), 1, "Exit block should only have on predecessor");

            // If the definition was cloned used that otherwise use the same
            // value.
            let mut cloned_value = incoming[0].0;
            if let Operand::Value(value) = cloned_value
                && let Some(&mapped) = self.vmap.values.get(&value)
            {
                cloned_value = mapped;
            }
            incoming.push((cloned_value, non_versioned_exiting));
            rw.function.set_operands(phi, from_arms(&incoming));
        }
    }

    /// `MDNode::get(Context, Ops)`: the tuple of `operands`, made once.
    fn tuple(
        &mut self,
        rw: &mut Rewriter,
        operands: Vec<MetadataOperand>,
    ) -> MetadataId {
        if let Some(&made) = self.tuples.get(&operands) {
            return made;
        }
        let module = rw.env.metadata;
        let found = module
            .iter()
            .chain(rw.declared.nodes())
            .position(|node| !node.distinct && node.operands == operands)
            .map(|at| MetadataId(at as u32));
        let id =
            found.unwrap_or_else(|| rw.declared.node(MetadataNode { distinct: false, operands: operands.clone() }));
        self.tuples.insert(operands, id);
        id
    }

    /// `MDBuilder::createAnonymousAliasScope(Domain)` or, with no domain,
    /// `createAnonymousAliasScopeDomain(Name)`: a distinct node that names
    /// itself first.
    fn anonymous_scope(
        rw: &mut Rewriter,
        second: MetadataOperand,
    ) -> MetadataId {
        let id = rw.declared.next_node();
        rw.declared.node(MetadataNode { distinct: true, operands: vec![MetadataOperand::Node(id), second] })
    }

    /// The operands of a metadata node, the module's or one made since.
    fn operands_of(
        rw: &Rewriter,
        id: MetadataId,
    ) -> Vec<MetadataOperand> {
        let module = rw.env.metadata.len();
        match (id.0 as usize).checked_sub(module) {
            None => rw.env.metadata[id.0 as usize].operands.clone(),
            Some(made) => rw.declared.nodes()[made].operands.clone(),
        }
    }

    /// `MDNode::concatenate(A, B)`: the operands of `a`, then those of `b`
    /// not among them.
    fn concatenate(
        &mut self,
        rw: &mut Rewriter,
        a: Option<MetadataId>,
        b: &[MetadataOperand],
    ) -> MetadataId {
        let mut operands = a.map(|a| Self::operands_of(rw, a)).unwrap_or_default();
        for operand in b {
            if !operands.contains(operand) {
                operands.push(operand.clone());
            }
        }
        self.tuple(rw, operands)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L223
    /// `prepareNoAliasMetadata`: set up the aliasing scopes based on the
    /// memchecks. It maps each pointer checking group (a set of pointers
    /// memchecked together) to an alias scope and then also maps each group
    /// to the list of scopes it can't alias. Call it before the first
    /// `annotate_inst_with_noalias`.
    pub fn prepare_no_alias_metadata(
        &mut self,
        rw: &mut Rewriter,
    ) {
        let rtc = self.rtc;
        // First allocate an aliasing scope for each pointer checking group.
        //
        // While traversing through the checking groups in the loop, also
        // create a reverse map from pointers to the pointer checking group
        // they were assigned to.
        let domain = Self::anonymous_scope(rw, MetadataOperand::String("LVerDomain".to_owned()));
        for (group_id, group) in rtc.checking_groups.iter().enumerate() {
            self.group_to_scope.insert(group_id, Self::anonymous_scope(rw, MetadataOperand::Node(domain)));
            for &ptr_idx in &group.members {
                self.ptr_to_group.insert(rtc.pointers[ptr_idx].pointer_value, group_id);
            }
        }

        // Go through the checks and for each pointer group, collect the
        // scopes for each non-aliasing pointer group.
        let mut order: Vec<usize> = Vec::new();
        let mut group_to_non_aliasing_scopes: HashMap<usize, Vec<MetadataOperand>> = HashMap::default();
        for &(first, second) in &self.alias_checks {
            if !group_to_non_aliasing_scopes.contains_key(&first) {
                order.push(first);
            }
            group_to_non_aliasing_scopes
                .entry(first)
                .or_default()
                .push(MetadataOperand::Node(self.group_to_scope[&second]));
        }

        // Finally, transform the above to actually map to scope list which is
        // what the metadata uses.
        for group in order {
            let list = self.tuple(rw, group_to_non_aliasing_scopes.remove(&group).expect("a collected group"));
            self.group_to_non_aliasing_scope_list.insert(group, list);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L267
    /// `annotateLoopWithNoAlias`: the memory instructions of the versioned
    /// loop get the no-alias metadata the memchecks issued allow.
    pub fn annotate_loop_with_no_alias(
        &mut self,
        rw: &mut Rewriter,
    ) {
        if !ANNOTATE_NO_ALIAS {
            return;
        }
        // First prepare the maps.
        self.prepare_no_alias_metadata(rw);

        // Add the scope and no-alias metadata to the instructions.
        for &inst in self.memory_instructions {
            self.annotate_inst_with_no_alias(rw, inst, inst);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L281
    /// `getNoAliasMetadataFor(OrigInst)`: the `alias.scope` and `noalias`
    /// nodes of a load or store, if it has them.
    pub fn get_no_alias_metadata_for(
        &mut self,
        rw: &mut Rewriter,
        orig_inst: InstId,
    ) -> (Option<MetadataId>, Option<MetadataId>) {
        if !ANNOTATE_NO_ALIAS {
            return (None, None);
        }
        let instruction = rw.function.instruction(orig_inst);
        let ptr = match instruction.opcode {
            Opcode::Load { .. } => instruction.operands[0],
            Opcode::Store { .. } => instruction.operands[1],
            _ => panic!("a load or store"),
        };
        let existing = |kind: &str| instruction.metadata.iter().find(|(one, _)| one == kind).map(|&(_, node)| node);
        let (existing_scope, existing_noalias) = (existing("alias.scope"), existing("noalias"));

        let mut alias_scope = None;
        let mut no_alias = None;
        // Find the group for the pointer and then add the scope metadata.
        if let Some(&group) = self.ptr_to_group.get(&ptr) {
            let scope = MetadataOperand::Node(self.group_to_scope[&group]);
            alias_scope = Some(self.concatenate(rw, existing_scope, &[scope]));

            // Add the no-alias metadata.
            if let Some(&list) = self.group_to_non_aliasing_scope_list.get(&group) {
                let scopes = Self::operands_of(rw, list);
                no_alias = Some(self.concatenate(rw, existing_noalias, &scopes));
            }
        }
        (alias_scope, no_alias)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopVersioning.cpp#L307
    /// `annotateInstWithNoAlias(VersionedInst, OrigInst)`: `orig_inst` is the
    /// instruction in the original loop that `versioned_inst` corresponds to.
    pub fn annotate_inst_with_no_alias(
        &mut self,
        rw: &mut Rewriter,
        versioned_inst: InstId,
        orig_inst: InstId,
    ) {
        let (alias_scope_md, no_alias_md) = self.get_no_alias_metadata_for(rw, orig_inst);
        if let Some(md) = alias_scope_md {
            rw.function.set_metadata(versioned_inst, "alias.scope", Some(md));
        }
        if let Some(md) = no_alias_md {
            rw.function.set_metadata(versioned_inst, "noalias", Some(md));
        }
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L124
/// `findDefsUsedOutsideOfLoop(L)`: the loop's instructions some instruction
/// outside it reads.
pub fn find_defs_used_outside_of_loop(
    function: &Function,
    lp: Loop,
) -> Vec<InstId> {
    let mut used_outside = Vec::new();
    for &block in lp.get_blocks() {
        for inst in function.iter_block(block) {
            let Some(result) = function.instruction(inst).result else { continue };
            if function
                .users(result)
                .into_iter()
                .any(|used| function.parent(used.user).is_some_and(|b| !lp.contains(b)))
            {
                used_outside.push(inst);
            }
        }
    }
    used_outside
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopInfo.cpp#L501
/// `Loop::isRecursivelyLCSSAForm(DT, LI)`: no block has a use outside its
/// innermost loop, a use by a phi counting in the block it comes from.
pub fn is_recursively_lcssa_form(
    function: &Function,
    loops: &LoopInfo,
    tree: &DomTree,
    lp: Loop,
) -> bool {
    lp.get_blocks()
        .iter()
        .all(
            |&block| {
                let inner = loops.get_loop_for(block).expect("a loop's block is in a loop");
                function.iter_block(block).all(|inst| {
                    let Some(result) = function.instruction(inst).result else { return true };
                    function.users(result).into_iter().all(|used| {
                        let user = function.instruction(used.user);
                        let mut user_block = function.parent(used.user).expect("a placed user");
                        if user.opcode == Opcode::Phi
                            && let Operand::Block(from) = user.operands[used.index as usize + 1]
                        {
                            user_block = from;
                        }
                        user_block == block || inner.contains(user_block) || !tree.is_reachable_from_entry(user_block)
                    })
                })
            },
        )
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L66
/// `formDedicatedExitBlocks(L, ...)` for the loop of `blocks`: each exit
/// block that is also reached from outside the loop gets a block that the
/// loop's edges go to instead. Whether any was made.
pub fn form_dedicated_exit_blocks(
    function: &mut Function,
    blocks: &[BlockId],
) -> bool {
    let mut in_loop = llrm_mir::dense::IdSet::<BlockId>::new();
    for &block in blocks {
        in_loop.insert(block);
    }
    let mut changed = false;
    let mut visited: Vec<BlockId> = Vec::new();
    for &block in blocks {
        for succ in function.successors(block).into_iter().collect::<Vec<_>>() {
            // We're looking for exit blocks so skip in-loop successors.
            if in_loop.contains(&succ) {
                continue;
            }
            // Visit each exit block exactly once.
            if visited.contains(&succ) {
                continue;
            }
            visited.push(succ);
            let preds: Vec<BlockId> = function.predecessors(succ).into_iter().collect();
            let in_loop_predecessors: Vec<BlockId> =
                preds.iter().copied().filter(|pred| in_loop.contains(pred)).collect();
            assert!(!in_loop_predecessors.is_empty(), "Must have *some* loop predecessor!");
            // Nothing to do if this is already a dedicated exit.
            if in_loop_predecessors.len() == preds.len() {
                continue;
            }
            let new_exit = split_block_predecessors_lcssa(function, succ, &in_loop_predecessors);
            // `SplitBlockPredecessors` puts the new block right before `succ`.
            let before = function.layout().iter().position(|&one| one == succ).expect("a placed exit");
            if before > 0 {
                let after = function.layout()[before - 1];
                function.move_block_after(new_exit, after).expect("a placed block");
            }
            if let Some(name) = function.block(succ).name.clone() {
                function.set_block_name(new_exit, &format!("{name}.loopexit"));
            }
            changed = true;
        }
    }
    changed
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1241
/// `SplitBlockPredecessors(bb, preds, .., PreserveLCSSA)` for predecessors in
/// a loop that `bb` is outside of: `UpdatePHINodes` with `HasLoopExit`, so
/// every phi of `bb` takes its arm from a phi `<name>.ph` of the new block,
/// even where the values agree.
fn split_block_predecessors_lcssa(
    function: &mut Function,
    bb: BlockId,
    preds: &[BlockId],
) -> BlockId {
    let new = edges::split_block_predecessors(function, bb, preds);
    let jump = function.terminator(new).expect("a terminated block");
    for phi in edges::phis(function, bb) {
        let mut incoming = arms(function, phi);
        let at = incoming.iter().position(|&(_, from)| from == new).expect("an arm from the new block");
        let value = incoming[at].0;
        // Where the values differ the split already made the phi.
        let gathered = matches!(
            value,
            Operand::Value(v) if matches!(function.value(v).def, ValueDef::Instruction(inst) if function.instruction(inst).opcode == Opcode::Phi && function.parent(inst) == Some(new))
        );
        if gathered {
            continue;
        }
        let ty = function.instruction(phi).ty;
        let name = function.instruction(phi).result.and_then(|v| function.value(v).name.clone()).unwrap_or_default();
        let from_preds: Vec<_> = preds.iter().map(|&pred| (value, pred)).collect();
        let gather = function.create_instruction(
            Opcode::Phi,
            ty,
            from_arms(&from_preds),
            Flags::default(),
            Some(&format!("{name}.ph")),
        );
        function.insert(gather, Position::Before(jump)).expect("a placed jump");
        incoming[at].0 = Operand::Value(function.instruction(gather).result.expect("a phi's value"));
        function.set_operands(phi, from_arms(&incoming));
    }
    new
}

#[cfg(test)]
#[path = "loopversioning_tests.rs"]
mod tests;
