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
