//! What `sinkRegion` and `hoistRegion` do once they have decided (`LICM.cpp`
//! `hoist`, `sink`, `sinkThroughTriviallyReplaceablePHI`,
//! `cloneInstructionInExitBlock`, `eraseInstruction`): the edits to one loop.

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::build::Place;
use llrm_analysis::mssa::updater::{MemorySsaUpdater, MoveTo};
use llrm_analysis::mssa::{AccessId, Kind, Mssa};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::Outer;

use super::hoist::Reassociation;
use super::sink::Sinking;

/// The memory graph the edits keep right (`MemorySSAUpdater` and the accesses
/// it reads the instructions' effects from).
pub struct Graph<'a> {
    pub mssa: &'a mut Mssa,
    pub accesses: &'a mut Accesses,
    pub updater: &'a mut MemorySsaUpdater,
    pub outer: &'a Outer,
    pub layout: &'a DataLayout,
    /// The access of the last store put in each exit block of the promotion in
    /// hand: the next goes after it (`MSSAInsertPts`).
    pub last_store: IdMap<BlockId, AccessId>,
}

/// Where the access of a new instruction goes in its block.
#[derive(Clone, Copy)]
pub enum Created {
    Beginning,
    End,
    After(AccessId),
    Before(AccessId),
}

impl Graph<'_> {
    /// `eraseInstruction`: the access of `inst` goes (before the instruction
    /// does).
    pub fn erase(
        &mut self,
        function: &Function,
        inst: InstId,
    ) {
        if let Some(access) = self.mssa.get_memory_access(inst) {
            self.updater.remove_memory_access(self.mssa, function, access, true);
        }
    }

    /// `moveToPlace(..., BeforeTerminator)` after `inst` was moved to the end
    /// of `block`: no terminator has an access, so the end of its list.
    pub fn moved_to_end(
        &mut self,
        function: &Function,
        inst: InstId,
        block: BlockId,
    ) {
        if let Some(access) = self.mssa.get_memory_access(inst) {
            self.updater.move_to_place(self.mssa, function, access, block, MoveTo::End, None);
        }
    }

    /// `createMemoryAccessInBB` / `createMemoryAccessAfter`, then `insertDef`
    /// or `insertUse`: the access of `inst`, new, in `block`.
    pub fn created(
        &mut self,
        context: &Context,
        function: &Function,
        inst: InstId,
        block: BlockId,
        at: Created,
    ) -> Option<AccessId> {
        self.created_renaming(context, function, inst, block, at, true)
    }

    /// As [`created`](Self::created), where `rename_uses` says whether
    /// `insertDef` renames the uses the new def now clobbers.
    pub fn created_renaming(
        &mut self,
        context: &Context,
        function: &Function,
        inst: InstId,
        block: BlockId,
        at: Created,
        rename_uses: bool,
    ) -> Option<AccessId> {
        let unit = llrm_analysis::memory::Unit::within(context, self.layout, function, self.outer);
        self.accesses.add_instruction(&unit, inst);
        let effect = llrm_analysis::mssa::shadow::effect_of_in(self.accesses, &unit);
        let access = match at {
            Created::Beginning => {
                self.updater.create_memory_access_in_bb(self.mssa, inst, None, block, Place::Beginning, &effect)?
            }
            Created::End => {
                self.updater.create_memory_access_in_bb(self.mssa, inst, None, block, Place::End, &effect)?
            }
            Created::After(previous) => {
                self.updater.create_memory_access_after(self.mssa, inst, None, previous, &effect)?
            }
            Created::Before(next) => self.updater.create_memory_access_before(self.mssa, inst, None, next, &effect)?,
        };
        if self.mssa.access(access).kind == Kind::Def {
            self.updater.insert_def(self.mssa, function, access, rename_uses);
        } else {
            self.updater.insert_use(self.mssa, function, access, rename_uses);
        }
        Some(access)
    }
}

/// [`hoist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1460):
/// `inst` moved before the preheader's terminator. Every fact an instruction
/// carries survives running where it did not run
/// (`Fact::survives_speculation`), so none is dropped; a call's attributes on
/// its arguments and result are dropped by the caller.
fn hoist(
    function: &mut Function,
    graph: &mut Graph,
    inst: InstId,
    preheader: BlockId,
) -> Result<(), String> {
    let terminator = function.terminator(preheader).ok_or("a preheader with no terminator")?;
    function.move_to(inst, Position::Before(terminator))?;
    graph.moved_to_end(function, inst, preheader);
    Ok(())
}

/// The edit of a [`Reassociation`]. For `hoistBOAssociation`, `c1 op c2` goes
/// before the preheader's terminator, `bo` is replaced by `lv op` that, and
/// `bo0`, which only `bo` read, is erased. DEVIATE: `c1 op c2` is made even
/// where both are constants (no folding builder). For `hoistGEP`, the GEP of
/// the outer indices goes there and `gep` is replaced by one of the inner
/// indices on it.
fn reassociate(
    function: &mut Function,
    context: &Context,
    graph: &mut Graph,
    preheader: BlockId,
    one: &Reassociation,
) -> Result<(), String> {
    let terminator = function.terminator(preheader).ok_or("a preheader with no terminator")?;
    let old = one.inst();
    let ty = function.instruction(old).ty;
    // The invariant instruction, then the new one: its opcode and flags, the
    // operands before the invariant one, and those after it.
    let (invariant, rebuilt) = match one {
        Reassociation::BinaryOp { op, lv, c1, c2, flags, .. } => (
            (Opcode::Binary(*op), vec![*c1, *c2], *flags, "invariant.op"),
            (Opcode::Binary(*op), *flags, vec![*lv], vec![]),
        ),
        Reassociation::Gep { gep, src, in_bounds } => {
            let (gep, src) = (function.instruction(*gep), function.instruction(*src));
            let (Opcode::GetElementPtr { source: outer }, Opcode::GetElementPtr { source: inner }) =
                (&gep.opcode, &src.opcode)
            else {
                return Err("a GEP reassociation of a non-GEP".into());
            };
            let flags = if *in_bounds { Flags::INBOUNDS } else { Flags::default() };
            let outer_operands = [&src.operands[..1], &gep.operands[1..]].concat();
            (
                (Opcode::GetElementPtr { source: *outer }, outer_operands, flags, "invariant.gep"),
                (Opcode::GetElementPtr { source: *inner }, flags, vec![], src.operands[1..].to_vec()),
            )
        }
    };
    let (opcode, operands, flags, name) = invariant;
    let inv = function.create_instruction(opcode, ty, operands, flags, Some(name));
    function.insert(inv, Position::Before(terminator))?;
    graph.created(context, function, inv, preheader, Created::End);
    let inv_result = function.instruction(inv).result.ok_or("an invariant op with no result")?;
    let (opcode, flags, head, tail) = rebuilt;
    let operands = [head, vec![Operand::Value(inv_result)], tail].concat();
    // LLVM names the new GEP "gep".
    let name = match one {
        Reassociation::BinaryOp { .. } => function
            .instruction(old)
            .result
            .and_then(|result| function.value(result).name.clone())
            .map(|name| format!("{name}.reass")),
        Reassociation::Gep { .. } => Some("gep".to_string()),
    };
    let new = function.create_instruction(opcode, ty, operands, flags, name.as_deref());
    function.insert(new, Position::Before(old))?;
    let home = function.parent(old).ok_or("an instruction in no block")?;
    graph.created(context, function, new, home, Created::End);
    let old_result = function.instruction(old).result.ok_or("a reassociated op with no result")?;
    let new_result = function.instruction(new).result.ok_or("a reassociated op with no result")?;
    function.replace_all_uses_with(old_result, Operand::Value(new_result));
    for gone in [old, one.erased()] {
        graph.erase(function, gone);
        function.erase(gone)?;
    }
    Ok(())
}

/// The first instruction of `block` that is not a phi.
fn first_insertion_point(
    function: &Function,
    block: BlockId,
) -> Position {
    function
        .iter_block(block)
        .find(|&one| function.instruction(one).opcode != Opcode::Phi)
        .map_or(Position::End(block), Position::Before)
}

/// [`cloneInstructionInExitBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1149):
/// a copy of `inst` at the start of the exit block `exit`, each operand defined
/// in the loop read through an LCSSA phi made there.
fn clone_in_exit_block(
    function: &mut Function,
    loop_: &Loop,
    inst: InstId,
    exit: BlockId,
    lcssa: &mut IdMap<llrm_mir::module::ValueId, InstId>,
) -> Result<InstId, String> {
    let clone = function.clone_instruction(inst);
    let operands = function.instruction(clone).operands.clone();
    for (index, operand) in operands.into_iter().enumerate() {
        let Operand::Value(value) = operand else { continue };
        let ValueDef::Instruction(def) = function.value(value).def else { continue };
        let Some(home) = function.parent(def) else { continue };
        if !loop_.body.contains(&llrm_analysis::cfg::id(home)) {
            continue;
        }
        let phi = match lcssa.get(&value) {
            Some(&phi) => phi,
            None => {
                let incoming: Vec<Operand> = function
                    .predecessors(exit)
                    .into_iter()
                    .flat_map(|pred| [Operand::Value(value), Operand::Block(pred)])
                    .collect();
                let ty = function.value(value).ty;
                let name = function.value(value).name.clone().map(|name| format!("{name}.lcssa"));
                let phi = function.create_instruction(Opcode::Phi, ty, incoming, Flags::default(), name.as_deref());
                let first = function.block(exit).first().map_or(Position::End(exit), Position::Before);
                function.insert(phi, first)?;
                lcssa.insert(value, phi);
                phi
            }
        };
        let result = function.instruction(phi).result.expect("a phi's result");
        function.set_operand(clone, index, Operand::Value(result));
    }
    let at = first_insertion_point(function, exit);
    function.insert(clone, at)?;
    Ok(clone)
}

/// [`sink`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1353)
/// through phis that read nothing else: `inst` is copied into each exit block
/// that reads it, the phi replaced by the copy, the original erased unless
/// `keep`: users in the loop still read it.
fn sink(
    function: &mut Function,
    context: &Context,
    graph: &mut Graph,
    loop_: &Loop,
    inst: InstId,
    keep: bool,
) -> Result<(), String> {
    let result = function.instruction(inst).result.ok_or("a sunk instruction with no result")?;
    let mut users: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
    users.retain(|&user| {
        !keep || function.parent(user).is_some_and(|home| !loop_.body.contains(&llrm_analysis::cfg::id(home)))
    });
    users.dedup();
    // Clones of this instruction. Don't create more than one per exit block!
    let mut copies: IdMap<BlockId, InstId> = IdMap::new();
    let mut lcssa: IdMap<BlockId, IdMap<llrm_mir::module::ValueId, InstId>> = IdMap::new();
    for user in users {
        let exit = function.parent(user).ok_or("a user outside every block")?;
        let copy = match copies.get(&exit) {
            Some(&copy) => copy,
            None => {
                let phis = lcssa.get_or_insert_with(exit, IdMap::new);
                let copy = clone_in_exit_block(function, loop_, inst, exit, phis)?;
                graph.created(context, function, copy, exit, Created::Beginning);
                copies.insert(exit, copy);
                copy
            }
        };
        let phi_result = function.instruction(user).result.ok_or("a phi with no result")?;
        let copy_result = function.instruction(copy).result.ok_or("a copy with no result")?;
        function.replace_all_uses_with(phi_result, Operand::Value(copy_result));
        function.erase(user)?;
    }
    if keep {
        return Ok(());
    }
    graph.erase(function, inst);
    function.erase(inst)
}

/// The edits one loop's decisions make, in the order `runOnLoop` makes them:
/// what is dead erased and what sinks sunk (bottom-up), then what hoists
/// moved. Whether anything changed.
pub fn apply_loop(
    function: &mut Function,
    context: &Context,
    graph: &mut Graph,
    loop_: &Loop,
    preheader: Option<BlockId>,
    sunk: &[(InstId, Sinking)],
    hoisted: &[super::hoist::Hoist],
    reassociated: &[(usize, Reassociation)],
) -> Result<bool, String> {
    let mut changed = false;
    for &(inst, how) in sunk {
        match how {
            Sinking::Erased => {
                graph.erase(function, inst);
                function.erase(inst)?;
            }
            Sinking::Sunk => sink(function, context, graph, loop_, inst, false)?,
            Sinking::Cloned => sink(function, context, graph, loop_, inst, true)?,
        }
        changed = true;
    }
    // A reassociation goes to the preheader where the visit found it, among
    // the hoists.
    let mut pending = reassociated.iter().peekable();
    for (at, &super::hoist::Hoist { inst, guaranteed, .. }) in hoisted.iter().enumerate() {
        while let Some((_, one)) = pending.next_if(|(after, _)| *after <= at) {
            let preheader = preheader.ok_or("a reassociation with no preheader")?;
            reassociate(function, context, graph, preheader, one)?;
        }
        let preheader = preheader.ok_or("a hoist with no preheader")?;
        // Hoisting a call that was not guaranteed to execute strips what would
        // make it undefined where it did not run (`LICM.cpp:1478`).
        if !guaranteed {
            function.drop_ub_implying_attrs(inst);
        }
        hoist(function, graph, inst, preheader)?;
        changed = true;
    }
    for (_, one) in pending {
        let preheader = preheader.ok_or("a reassociation with no preheader")?;
        reassociate(function, context, graph, preheader, one)?;
        changed = true;
    }
    Ok(changed)
}

/// What a promotion edits: the loop and where its pieces go.
pub struct PromotionSite<'a> {
    pub preheader: BlockId,
    /// The unique exit blocks, each with the instruction its stores go before
    /// (`InsertPts`: the first non-phi at the time the loop's promotion
    /// began, so successive promotions' stores follow each other).
    pub exits: &'a [(BlockId, InstId)],
    /// The innermost loop of a block, to tell an exit use that needs an LCSSA
    /// phi.
    pub loop_of: &'a dyn Fn(BlockId) -> Option<&'a Loop>,
}

/// `LoopInfo::wouldBeOutOfLoopUseRequiringLCSSA`: `value` is defined in a loop
/// that does not contain `block`.
fn requires_lcssa(
    function: &Function,
    site: &PromotionSite,
    value: Operand,
    block: BlockId,
) -> bool {
    let Operand::Value(value) = value else { return false };
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let Some(home) = function.parent(def) else { return false };
    match (site.loop_of)(home) {
        Some(loop_) => !loop_.body.contains(&llrm_analysis::cfg::id(block)),
        None => false,
    }
}

/// `maybeInsertLCSSAPHI`: `value` read in the exit block `block`, through a phi
/// of it there when it is defined in a loop that block is outside of.
fn maybe_insert_lcssa_phi(
    function: &mut Function,
    site: &PromotionSite,
    value: Operand,
    block: BlockId,
) -> Result<Operand, String> {
    if !requires_lcssa(function, site, value, block) {
        return Ok(value);
    }
    let Operand::Value(source) = value else { return Ok(value) };
    let incoming: Vec<Operand> =
        function.predecessors(block).into_iter().flat_map(|pred| [value, Operand::Block(pred)]).collect();
    let ty = function.value(source).ty;
    let name = function.value(source).name.clone().map(|name| format!("{name}.lcssa"));
    let phi = function.create_instruction(Opcode::Phi, ty, incoming, Flags::default(), name.as_deref());
    let first = function.block(block).first().map_or(Position::End(block), Position::Before);
    function.insert(phi, first)?;
    Ok(Operand::Value(function.instruction(phi).result.expect("a phi's result")))
}

/// [`promoteLoopAccessesToScalars`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1692)
/// from "Lets do the promotion" on, with `LoopPromoter` and
/// [`LoadAndStorePromoter::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdater.cpp#L360):
/// the loop's loads of the pointers read an SSA value, a load before the loop
/// makes the value on entry, and when `store_safe` the last value is stored
/// in each exit block and the loop's stores go.
pub fn promote(
    function: &mut Function,
    context: &mut llrm_mir::context::Context,
    graph: &mut Graph,
    site: &PromotionSite,
    found: &super::promote::Promotion,
) -> Result<(), String> {
    use llrm_analysis::ssa::SsaUpdater;
    use llrm_mir::context::{Constant, ConstantKind};
    let some_ptr = found.pointers[0];
    let ty = found.access_ty;
    let mut ssa = SsaUpdater::new(ty, None);
    // Set up the preheader to have a definition of the value.
    let mut preheader_load: Option<InstId> = None;
    if found.found_load_to_promote || !found.store_is_guaranteed_to_execute {
        let name = match some_ptr {
            Operand::Value(value) => function.value(value).name.clone().map(|name| format!("{name}.promoted")),
            _ => Some("promoted".to_owned()),
        };
        let load = function.create_instruction(
            Opcode::Load { align: Some(found.alignment), volatile: false },
            ty,
            vec![some_ptr],
            Flags::default(),
            name.as_deref(),
        );
        if found.load_is_guaranteed_to_execute
            && let Some(node) = found.tbaa
        {
            function.annotate(load, "tbaa", node);
        }
        let terminator = function.terminator(site.preheader).ok_or("a preheader with no terminator")?;
        function.insert(load, Position::Before(terminator))?;
        graph.created(context, function, load, site.preheader, Created::End);
        ssa.add_available_value(site.preheader, Operand::Value(function.instruction(load).result.expect("a load")));
        preheader_load = Some(load);
    } else {
        let undef = context.constant(Constant { ty, kind: ConstantKind::Undef });
        ssa.add_available_value(site.preheader, Operand::Constant(undef));
    }
    // Bucket up the uses by the block they occur in: the SSA updater is for
    // references across blocks, a block's own order is ours.
    let mut by_block: IdMap<BlockId, Vec<InstId>> = IdMap::new();
    for &inst in &found.uses {
        let block = function.parent(inst).ok_or("a use outside every block")?;
        by_block.get_or_insert_with(block, Vec::new).push(inst);
    }
    let mut live_in_loads: Vec<InstId> = Vec::new();
    let mut replaced: IdMap<InstId, Operand> = IdMap::new();
    let is_store =
        |function: &Function, inst: InstId| matches!(function.instruction(inst).opcode, Opcode::Store { .. });
    let blocks: Vec<BlockId> = by_block.keys().collect();
    for block in blocks {
        let uses = by_block.get(&block).cloned().unwrap_or_default();
        if uses.is_empty() {
            continue;
        }
        // A block with one use: a store defines the value in the block, a load
        // reads the value on entry.
        if let [only] = uses[..] {
            if is_store(function, only) {
                ssa.add_available_value(block, function.instruction(only).operands[0]);
            } else {
                live_in_loads.push(only);
            }
            continue;
        }
        // A block of loads alone reads the value on entry.
        if !uses.iter().any(|&inst| is_store(function, inst)) {
            live_in_loads.extend(uses);
            continue;
        }
        // Mixed: a scan of the block tells the order. A load after a store
        // reads the stored value; the last store defines the value at the end.
        let mut stored: Option<Operand> = None;
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            if !uses.contains(&inst) {
                continue;
            }
            if is_store(function, inst) {
                stored = Some(function.instruction(inst).operands[0]);
            } else if let Some(value) = stored {
                let result = function.instruction(inst).result.expect("a load's result");
                function.replace_all_uses_with(result, value);
                replaced.insert(inst, value);
            } else {
                live_in_loads.push(inst);
            }
        }
        ssa.add_available_value(block, stored.expect("a store in the block"));
    }
    // Now that all of the values are set up, handle the live-in loads.
    for load in live_in_loads {
        let block = function.parent(load).ok_or("a load outside every block")?;
        let mut value = ssa.value_in_middle_of_block(context, function, block);
        let result = function.instruction(load).result.expect("a load's result");
        // Avoid assertions in unreachable code.
        if value == Operand::Value(result) {
            value = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }));
        }
        function.replace_all_uses_with(result, value);
        replaced.insert(load, value);
    }
    // Insert stores after in the loop exit blocks. Each exit block gets a store
    // of the live-out value that feeds it.
    if found.store_safe {
        for &(exit, before) in site.exits {
            let live_in = ssa.value_in_middle_of_block(context, function, exit);
            let live_in = maybe_insert_lcssa_phi(function, site, live_in, exit)?;
            let pointer = maybe_insert_lcssa_phi(function, site, some_ptr, exit)?;
            let void = context.types.void();
            let store = function.create_instruction(
                Opcode::Store { align: Some(found.alignment), volatile: false },
                void,
                vec![live_in, pointer],
                Flags::default(),
                None,
            );
            if found.store_is_guaranteed_to_execute
                && let Some(node) = found.tbaa
            {
                function.annotate(store, "tbaa", node);
            }
            function.insert(store, Position::Before(before))?;
            let at = match graph.last_store.get(&exit) {
                Some(&previous) => Created::After(previous),
                None => Created::Beginning,
            };
            if let Some(access) = graph.created(context, function, store, exit, at) {
                graph.last_store.insert(exit, access);
            }
        }
    }
    // Now that everything is rewritten, delete the instructions.
    for &inst in &found.uses {
        if is_store(function, inst) && !found.store_safe {
            continue;
        }
        if let Some(result) = function.instruction(inst).result
            && !function.users(result).is_empty()
        {
            // A load that still has uses was added as a live value in the SSA
            // updater (the loaded value was stored later): follow the
            // replacements to the real value.
            let mut value = *replaced.get(&inst).ok_or("a load with uses that was not replaced")?;
            while let Operand::Value(next) = value
                && let ValueDef::Instruction(def) = function.value(next).def
                && let Some(&again) = replaced.get(&def)
            {
                value = again;
            }
            function.replace_all_uses_with(result, value);
        }
        graph.erase(function, inst);
        function.erase(inst)?;
    }
    // If the SSA updater didn't use the load in the preheader, just zap it now.
    if let Some(load) = preheader_load {
        let result = function.instruction(load).result.expect("a load");
        if function.users(result).is_empty() {
            graph.erase(function, load);
            function.erase(load)?;
        }
    }
    Ok(())
}
