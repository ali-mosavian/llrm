//! What the module needs before selection: a block that ends in a conditional branch (or a
//! `switch`) to a block with phis puts the phi copies on its own edge, so that edge gets a block.

use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_transforms::edges::split_critical_edge;

/// Splits every critical edge into a block with phis, in every function with a body. The selector
/// refuses a block of several machine blocks (a split branch, a `switch`) that exports phi
/// copies, as one copy list cannot serve its several edges.
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
                let phi = function.iter_block(to).next().is_some_and(|first| function.instruction(first).opcode == Opcode::Phi);
                if phi {
                    split_critical_edge(function, from, to);
                }
            }
        }
    }
}
