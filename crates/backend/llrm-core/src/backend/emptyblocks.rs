//! A block of nothing but a jump merged into its successor, so a phi's copy
//! lands in the predecessor that fed it. Translated from CodeGenPrepare's
//! `eliminateMostlyEmptyBlocks`
//! ([CodeGenPrepare.cpp#L939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939)).
//!
//! # How it works
//! Passes before selection leave blocks that only jump (`br b6`) between a
//! branch and a block with phis. Left alone, phi elimination puts the copy of
//! a phi's input in that block, and the block stays as a block of its own with
//! the code that follows it duplicated (`xor ebx, ebx; cmp ebx, eax; ja ...`).
//! Each such block `BB` with the jump to `DestBB` is removed when
//! [`can_merge`] (`canMergeBlocks`: no predecessor of `BB` that is already a
//! predecessor of `DestBB` feeds the phis a different value) and
//! [`profitable`] (`isMergingEmptyBlockProfitable`: not a loop preheader that
//! would become a critical edge) say so. A `DestBB` whose only predecessor is
//! `BB` takes `BB`'s place; otherwise the phis of `DestBB` take one input for
//! each predecessor of `BB` and the predecessors jump to `DestBB`:
//!
//! ```text
//!  p:  br c, a, e              p:  br c, a, d
//!  e:  br d                    d:  %x = phi [0, p], [%y, a]
//!  d:  %x = phi [0, e], [%y, a]
//! ```
//!
//! DEVIATE B1 (no number): a block with phis of its own is kept (the pin merges
//! it), `DeleteDeadPHIs` is not run, and a predecessor ending in a `switch`
//! keeps the block whenever `DestBB` has phis (the pin weighs block
//! frequencies there). Exit: those three pieces translated.
//!
//! # Example
//! `b4: br %5, label %21, label %25`, `25: br label %b6`, `b6: %6 = phi [0, %25],
//! ...`: `25` is gone, `b4` branches to `b6`, and the phi reads `0` from `b4`.
//!
//! # Runs
//! Once per program in `driver::optimized`, after `legalize` and before
//! `selects`, at every -O level.

use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, GlobalKind, Module, Operand};
use llrm_mir::opcode::Opcode;

/// Every function of `module` with its mostly empty blocks merged away.
pub fn eliminated(module: &mut Module) -> Result<(), String> {
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        if function.layout().len() > 1 {
            eliminated_in(&module.context, function)?;
        }
    }
    Ok(())
}

fn eliminated_in(
    context: &llrm_mir::context::Context,
    function: &mut Function,
) -> Result<(), String> {
    let mut preheaders: Vec<BlockId> = Vec::new();
    {
        let info = LoopInfo::analyze(function, || llrm_mir::domtree::DomTree::of(function));
        for one in info.get_loops_in_preorder() {
            if let Some(preheader) = one.get_loop_preheader(function) {
                preheaders.push(preheader);
            }
        }
    }
    // Note that this intentionally skips the entry block.
    let blocks: Vec<BlockId> = function.layout().iter().skip(1).copied().collect();
    for bb in blocks {
        if !function.is_placed(bb) {
            continue;
        }
        let Some(dest) = dest_of_mergeable(function, bb) else { continue };
        if !profitable(function, bb, dest, preheaders.contains(&bb)) {
            continue;
        }
        eliminate(context, function, bb, dest)?;
    }
    Ok(())
}

/// `findDestBlockOfMergeableEmptyBlock`: where `bb` jumps, if it holds only
/// that jump and can be merged there.
fn dest_of_mergeable(
    function: &Function,
    bb: BlockId,
) -> Option<BlockId> {
    let end = function.terminator(bb)?;
    let jump = function.instruction(end);
    let (Opcode::Br, [Operand::Block(dest)]) = (&jump.opcode, &jump.operands[..]) else { return None };
    // DEVIATE B1: a block with phis of its own is kept.
    if function.block(bb).first() != Some(end) {
        return None;
    }
    // Do not break infinite loops.
    if *dest == bb {
        return None;
    }
    can_merge(function, bb, *dest).then_some(*dest)
}

/// `canMergeBlocks`: `bb` has no phis here, so only the check of the
/// predecessors it shares with `dest` is left.
fn can_merge(
    function: &Function,
    bb: BlockId,
    dest: BlockId,
) -> bool {
    let bb_preds = function.predecessors(bb);
    let phis = phis_of(function, dest);
    if phis.is_empty() {
        return true;
    }
    for pred in function.predecessors(dest) {
        if !bb_preds.contains(&pred) {
            continue;
        }
        for &phi in &phis {
            let value = |from: BlockId| incoming(function, phi, from);
            if value(pred) != value(bb) {
                return false;
            }
        }
    }
    true
}

/// `isMergingEmptyBlockProfitable`.
fn profitable(
    function: &Function,
    bb: BlockId,
    dest: BlockId,
    is_preheader: bool,
) -> bool {
    // Do not delete loop preheaders if doing so would create a critical edge.
    if is_preheader {
        let alone = match function.predecessors(bb)[..] {
            [pred] => function.successors(pred).len() == 1,
            _ => false,
        };
        if !alone {
            return false;
        }
    }
    // A unique predecessor ending in a switch: the pin weighs block
    // frequencies; the block is kept where `dest` has phis (DEVIATE B1).
    if let [pred] = function.predecessors(bb)[..]
        && let Some(end) = function.terminator(pred)
        && function.instruction(end).opcode == Opcode::Switch
        && !phis_of(function, dest).is_empty()
    {
        return false;
    }
    true
}

/// The phis at the top of `block`.
fn phis_of(
    function: &Function,
    block: BlockId,
) -> Vec<llrm_mir::module::InstId> {
    function.iter_block(block).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// The value `phi` takes from `from`.
fn incoming(
    function: &Function,
    phi: llrm_mir::module::InstId,
    from: BlockId,
) -> Option<Operand> {
    function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).map(|pair| pair[0])
}

/// `eliminateMostlyEmptyBlock`.
fn eliminate(
    context: &llrm_mir::context::Context,
    function: &mut Function,
    bb: BlockId,
    dest: BlockId,
) -> Result<(), String> {
    // If the destination block has a single pred, then this is a trivial edge,
    // just collapse it.
    if function.predecessors(dest)[..] == [bb] {
        if llrm_transforms::cfg::merge_block_into_predecessor(context, function, dest, false) {
            function.erase_blocks(&[dest])?;
        }
        return Ok(());
    }
    // Otherwise the phis of `dest` take an input for each edge `bb` had.
    let preds = function.edge_predecessors(bb);
    for phi in phis_of(function, dest) {
        let operands = function.instruction(phi).operands.clone();
        let mut made: Vec<Operand> = Vec::with_capacity(operands.len() + 2 * preds.len());
        for pair in operands.chunks(2) {
            if pair[1] == Operand::Block(bb) {
                for &pred in &preds {
                    made.extend([pair[0], Operand::Block(pred)]);
                }
            } else {
                made.extend([pair[0], pair[1]]);
            }
        }
        function.set_operands(phi, made);
    }
    // Everything that named `bb` now names `dest`; `bb` goes.
    let jump = function.terminator(bb).ok_or("a jump")?;
    function.replace_block_uses_with(bb, dest);
    function.erase(jump)?;
    function.erase_blocks(&[bb])
}

#[cfg(test)]
mod tests {
    use llrm_mir::interpret::{Val, run};

    fn merged(text: &str) -> (llrm_mir::module::Module, llrm_mir::module::Module) {
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        super::eliminated(&mut after).unwrap();
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        (before, after)
    }

    fn blocks(module: &llrm_mir::module::Module) -> usize {
        let function = module.named("f").and_then(|id| module.global(id).function()).expect("f");
        function.layout().len()
    }

    /// collatz's guard: `p` branches to the loop or to an empty block that
    /// jumps to the join, whose phi reads `0` from it. The empty block put the
    /// copy of the `0` in a block of its own, with the join's compare copied
    /// after it (x_collatz +313 clocks at -O3 once the header copy went).
    #[test]
    fn test_an_empty_block_feeding_a_phi_is_merged_into_the_join() {
        let text = "define i32 @f(i32 %a, i32 %b) {
entry:
  %c = icmp ne i32 %a, 1
  br i1 %c, label %loop, label %empty
loop:
  %x = add i32 %a, %b
  br label %join
empty:
  br label %join
join:
  %p = phi i32 [ %x, %loop ], [ 0, %empty ]
  ret i32 %p
}
";
        let (before, after) = merged(text);
        assert_eq!(blocks(&before), 4);
        assert_eq!(blocks(&after), 3, "{}", llrm_mir::print::module(&after));
        for (a, b) in [(1, 5), (2, 5), (0, 0)] {
            let arguments = vec![Val::Int { bits: a, width: 32 }, Val::Int { bits: b, width: 32 }];
            assert_eq!(run(&after, "f", arguments.clone(), 100).unwrap(), run(&before, "f", arguments, 100).unwrap());
        }
    }

    /// A predecessor that already feeds the join a different value keeps the
    /// block (`canMergeBlocks`).
    #[test]
    fn test_a_shared_predecessor_with_another_value_keeps_the_block() {
        let text = "define i32 @f(i32 %a) {
entry:
  %c = icmp ne i32 %a, 1
  br i1 %c, label %join, label %empty
empty:
  br label %join
join:
  %p = phi i32 [ 5, %entry ], [ 7, %empty ]
  ret i32 %p
}
";
        let (_, after) = merged(text);
        assert_eq!(blocks(&after), 3, "{}", llrm_mir::print::module(&after));
    }
}
