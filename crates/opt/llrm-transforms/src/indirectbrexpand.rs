//! Replace `indirectbr` by a `switch` over block numbers, for a target that
//! has no jump through a register.
//!
//! Pass: LLVM `IndirectBrExpandPass` (`CodeGen/IndirectBrExpandPass.cpp`).
//! Translation: each address-taken successor of an `indirectbr` gets a number
//! from 1 (0 is null, which block addresses may be compared with); its
//! `blockaddress` becomes `inttoptr` of the number, and the `indirectbr`
//! becomes a `switch` of the address as an integer, the first block the
//! default. Several `indirectbr`s share one `switch_bb` that takes the address
//! from a phi. A phi of a target block takes `undef` from a jump that did not
//! list it.
//! General solution: one IR pass makes computed goto compile on every target
//! whose backend does not select `BRIND`; the target asks for it with
//! `Tti::expands_indirect_br` (LLVM's `enableIndirectBrExpand`).
//!
//! # Example
//! ```text
//! indirectbr ptr %a, [label %one, label %two]
//!   ==>  %n = ptrtoint ptr %a to i16
//!        switch i16 %n, label %one [ i16 2, label %two ]
//! blockaddress(@f, %one) ==> inttoptr (i16 1 to ptr)
//! ```
//!
//! # See also
//! `simplifycfg` (folds a known address to `br`),
//! `llrm_mir::Context::replace_block_addresses`.

use llrm_mir::context::{Constant, ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

pub struct IndirectBrExpand;

impl ModulePass for IndirectBrExpand {
    fn name(&self) -> &'static str {
        "indirectbr-expand"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let Some(bits) = module
            .datalayout
            .as_deref()
            .and_then(|text| DataLayout::parse(text).ok())
            .map(|layout| layout.pointer(0).bits)
        else {
            return Vec::new();
        };
        (0..module.globals.len() as u32).map(GlobalId).filter(|&function| expand(module, function, bits)).collect()
    }
}

/// One `indirectbr` and the blocks it lists.
struct Jump {
    block: BlockId,
    inst: InstId,
    targets: Vec<BlockId>,
}

fn expand(
    module: &mut Module,
    id: GlobalId,
    bits: u32,
) -> bool {
    let GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else { return false };
    let jumps: Vec<Jump> = function
        .layout()
        .iter()
        .filter_map(|&block| {
            let inst = function.terminator(block)?;
            let one = function.instruction(inst);
            (one.opcode == Opcode::IndirectBr).then(|| Jump {
                block,
                inst,
                targets: one
                    .operands
                    .iter()
                    .filter_map(|operand| if let Operand::Block(target) = operand { Some(*target) } else { None })
                    .collect(),
            })
        })
        .collect();
    // The blocks a jump can reach by an address: those whose address is taken.
    let taken: Vec<BlockId> = function.address_taken_blocks().collect();
    if jumps.is_empty() && taken.is_empty() {
        return false;
    }
    let numbered: Vec<BlockId> = function
        .layout()
        .iter()
        .copied()
        .filter(|block| {
            taken.contains(block) && (jumps.is_empty() || jumps.iter().any(|jump| jump.targets.contains(block)))
        })
        .collect();
    let addresses: Vec<(BlockId, Operand)> =
        jumps.iter().map(|jump| (jump.block, function.instruction(jump.inst).operands[0])).collect();
    let resolved: Vec<(BlockId, BlockId)> = function
        .layout()
        .iter()
        .copied()
        .chain(taken.iter().copied())
        .filter_map(|block| function.resolve_block_address(block).map(|now| (block, now)))
        .collect();

    // The listed blocks first, then any other block whose address is taken
    // (nothing jumps to it, but it compares unequal); a block gone is LLVM's 1.
    let mut number: IdMap<BlockId, u64> =
        numbered.iter().enumerate().map(|(at, &block)| (block, at as u64 + 1)).collect();
    for &(_, now) in &resolved {
        let next = number.len() as u64 + 1;
        if !number.contains_key(&now) {
            number.insert(now, next);
        }
    }
    let int_ty = module.context.types.int(bits);
    module
        .context
        .replace_block_addresses(
            id,
            &|block| match resolved.iter().find(|(old, _)| *old == block) {
                Some((_, now)) => number.get(now).copied(),
                None => Some(1),
            },
            int_ty,
        );
    let mut builder = module.builder(id);
    if jumps.is_empty() {
        // An address nothing jumps to (its `indirectbr` folded away): a number.
        clear(&mut builder);
        return true;
    }
    // Each jump, and the phi values of the blocks it left: what the jump
    // gave, by block it reached.
    let mut cut: Vec<(BlockId, Vec<BlockId>)> = Vec::new();
    for jump in &jumps {
        builder.function.erase(jump.inst).expect("a terminator nothing uses");
        cut.push((jump.block, jump.targets.clone()));
    }
    if numbered.is_empty() {
        // Nothing can be jumped to: LLVM's `unreachable`.
        for jump in &jumps {
            builder.position(jump.block);
            builder.unreachable();
        }
        fix_phis(&mut builder, &cut, &[], None);
        clear(&mut builder);
        return true;
    }
    let pointer_int = |builder: &mut llrm_mir::build::Builder<'_>, block: BlockId, address: Operand| {
        builder.position(block);
        builder.cast(CastOp::PtrToInt, address, int_ty, "switch_cast")
    };
    let cases = |builder: &mut llrm_mir::build::Builder<'_>| -> Vec<(Operand, BlockId)> {
        (1..numbered.len()).map(|at| (builder.int(bits, at as i128 + 1), numbered[at])).collect()
    };
    if let [(block, address)] = addresses[..] {
        let value = pointer_int(&mut builder, block, address);
        let cases = cases(&mut builder);
        builder.switch(value, numbered[0], &cases);
        fix_phis(&mut builder, &cut, &numbered, None);
    } else {
        let hub = builder.block("switch_bb");
        let mut incoming = Vec::new();
        for &(block, address) in &addresses {
            let value = pointer_int(&mut builder, block, address);
            builder.br(hub);
            incoming.push((value, block));
        }
        builder.position(hub);
        let value = builder.phi(int_ty, &incoming, "switch_value");
        // The phis the hub makes for its targets stand before the switch.
        fix_phis(&mut builder, &cut, &numbered, Some(hub));
        builder.position(hub);
        let cases = cases(&mut builder);
        builder.switch(value, numbered[0], &cases);
    }
    clear(&mut builder);
    true
}

/// No `blockaddress` of the function is left; a block whose address was taken
/// is an ordinary block.
fn clear(builder: &mut llrm_mir::build::Builder<'_>) {
    builder.function.clear_address_taken();
}

/// A block of `numbered` is now reached from the switch: from the
/// `indirectbr`'s own block when there was one, else from `hub`. Its phis take
/// `undef` from a jump that did not list it; a block a jump listed and the
/// switch does not reach loses its edge.
fn fix_phis(
    builder: &mut llrm_mir::build::Builder<'_>,
    cut: &[(BlockId, Vec<BlockId>)],
    numbered: &[BlockId],
    hub: Option<BlockId>,
) {
    let sources: Vec<BlockId> = cut.iter().map(|(block, _)| *block).collect();
    let mut touched: Vec<BlockId> = numbered.to_vec();
    for (_, listed) in cut {
        for block in listed {
            if !touched.contains(block) {
                touched.push(*block);
            }
        }
    }
    for block in touched {
        let phis: Vec<InstId> = builder
            .function
            .iter_block(block)
            .take_while(|&inst| builder.function.instruction(inst).opcode == Opcode::Phi)
            .collect();
        let reached = numbered.contains(&block);
        for phi in phis {
            let one = builder.function.instruction(phi);
            let ty = one.ty;
            let mut pairs: Vec<(Operand, BlockId)> = one
                .operands
                .chunks(2)
                .filter_map(|pair| match pair {
                    [value, Operand::Block(from)] => Some((*value, *from)),
                    _ => None,
                })
                .collect();
            // What each jump gave this block before it was cut.
            let gave = |pairs: &[(Operand, BlockId)], from: BlockId| {
                pairs.iter().find(|(_, one)| *one == from).map(|(v, _)| *v)
            };
            let undef = Operand::Constant(builder.context.constant(Constant { ty, kind: ConstantKind::Undef }));
            let values: Vec<(BlockId, Operand)> = cut
                .iter()
                .map(|(from, listed)| {
                    let value = if listed.contains(&block) { gave(&pairs, *from).unwrap_or(undef) } else { undef };
                    (*from, value)
                })
                .collect();
            pairs.retain(|(_, from)| !sources.contains(from));
            if reached {
                match hub {
                    None => pairs.extend(values.iter().map(|&(from, value)| (value, from))),
                    Some(hub) => {
                        let merged = if values.iter().all(|(_, value)| *value == values[0].1) {
                            values[0].1
                        } else {
                            builder.position(hub);
                            let incoming: Vec<(Operand, BlockId)> =
                                values.iter().map(|&(from, value)| (value, from)).collect();
                            builder.phi(ty, &incoming, "")
                        };
                        pairs.push((merged, hub));
                    }
                }
            }
            let operands = pairs.iter().flat_map(|&(value, from)| [value, Operand::Block(from)]).collect();
            builder.function.set_operands(phi, operands);
        }
    }
}
