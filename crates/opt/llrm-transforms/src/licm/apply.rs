//! What `sinkRegion` and `hoistRegion` do once they have decided (`LICM.cpp`
//! `hoist`, `sink`, `sinkThroughTriviallyReplaceablePHI`,
//! `cloneInstructionInExitBlock`, `eraseInstruction`): the edits to one loop.

use llrm_analysis::graph::loops::Loop;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};

use super::sink::Sinking;

/// [`hoist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L1460):
/// `inst` moved before the preheader's terminator. Every fact an instruction
/// carries survives running where it did not run
/// (`Fact::survives_speculation`), so none is dropped.
fn hoist(
    function: &mut Function,
    inst: InstId,
    preheader: BlockId,
) -> Result<(), String> {
    let terminator = function.terminator(preheader).ok_or("a preheader with no terminator")?;
    function.move_to(inst, Position::Before(terminator))
}

/// The first instruction of `block` that is not a phi.
fn first_insertion_point(
    function: &Function,
    block: BlockId,
) -> Position {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
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
                let first =
                    function.block(exit).instructions().first().copied().map_or(Position::End(exit), Position::Before);
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
/// that reads it, the phi replaced by the copy, the original erased.
fn sink(
    function: &mut Function,
    loop_: &Loop,
    inst: InstId,
) -> Result<(), String> {
    let result = function.instruction(inst).result.ok_or("a sunk instruction with no result")?;
    let mut users: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
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
                copies.insert(exit, copy);
                copy
            }
        };
        let phi_result = function.instruction(user).result.ok_or("a phi with no result")?;
        let copy_result = function.instruction(copy).result.ok_or("a copy with no result")?;
        function.replace_all_uses_with(phi_result, Operand::Value(copy_result));
        function.erase(user)?;
    }
    function.erase(inst)
}

/// The edits one loop's decisions make, in the order `runOnLoop` makes them:
/// what is dead erased and what sinks sunk (bottom-up), then what hoists
/// moved. Whether anything changed.
pub fn apply_loop(
    function: &mut Function,
    loop_: &Loop,
    preheader: Option<BlockId>,
    sunk: &[(InstId, Sinking)],
    hoisted: &[(InstId, BlockId)],
) -> Result<bool, String> {
    let mut changed = false;
    for &(inst, how) in sunk {
        match how {
            Sinking::Erased => function.erase(inst)?,
            Sinking::Sunk => sink(function, loop_, inst)?,
        }
        changed = true;
    }
    for &(inst, _) in hoisted {
        let preheader = preheader.ok_or("a hoist with no preheader")?;
        hoist(function, inst, preheader)?;
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
    let first = function.block(block).instructions().first().copied().map_or(Position::End(block), Position::Before);
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
        ssa.add_available_value(site.preheader, Operand::Value(function.instruction(load).result.expect("a load")));
        preheader_load = Some(load);
    } else {
        let poison = context.constant(Constant { ty, kind: ConstantKind::Poison });
        ssa.add_available_value(site.preheader, Operand::Constant(poison));
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
        for inst in function.block(block).instructions().to_vec() {
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
        function.erase(inst)?;
    }
    // If the SSA updater didn't use the load in the preheader, just zap it now.
    if let Some(load) = preheader_load {
        let result = function.instruction(load).result.expect("a load");
        if function.users(result).is_empty() {
            function.erase(load)?;
        }
    }
    Ok(())
}
