//! What the module needs before selection: a block that ends in a conditional
//! branch (or a `switch`) to a block with phis puts the phi copies on its own
//! edge, so that edge gets a block.

use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Opcode};
use llrm_transforms::edges::split_critical_edge;

/// Splits every critical edge into a block with phis, in every function with a
/// body. The selector refuses a block of several machine blocks (a split
/// branch, a `switch`) that exports phi copies, as one copy list cannot serve
/// its several edges.
pub fn split_phi_edges(module: &mut Module) {
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        let Some((_, function)) = module.function_mut(&name) else { continue };
        for from in function.layout().to_vec() {
            let Some(terminator) = function.terminator(from) else { continue };
            let targets: Vec<_> = function
                .instruction(terminator)
                .operands
                .iter()
                .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
                .collect();
            for to in targets {
                let phi = function
                    .iter_block(to)
                    .next()
                    .is_some_and(|first| function.instruction(first).opcode == Opcode::Phi);
                if phi {
                    split_critical_edge(function, from, to);
                }
            }
        }
    }
}

/// The bytes of the `__mulsi3` routine (`runtime/z80_rt.s`).
pub const MULSI3_BYTES: u32 = 57;

/// The attribute that lets a function tuned for size call `__mulsi3`.
pub const LONG_MULTIPLY: &str = "llrm-mulsi3";

/// Marks every function with a body when the module multiplies longs often
/// enough to pay for `__mulsi3` (57 B): a long multiply counts one, a long
/// long three (its cross products). A lone small multiply is cheaper inline
/// (copy_runs, switch_jump_table: +10 and +31 B at -Os with the call).
pub fn mark_long_multiplies(module: &mut Module) {
    let bits = |ty| module.context.types.int_bits(ty);
    let sites: u32 = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .map(|(_, _, function)| {
            function
                .layout()
                .iter()
                .flat_map(|&block| function.iter_block(block))
                .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Mul))
                .map(|inst| match bits(function.instruction(inst).ty) {
                    Some(32) => 1,
                    Some(64) => 3,
                    _ => 0,
                })
                .sum::<u32>()
        })
        .sum();
    if sites < 2 {
        return;
    }
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for name in names {
        if let Some((_, function)) = module.function_mut(&name) {
            function.attrs.push(Attribute::Flag(LONG_MULTIPLY.into()));
        }
    }
}
