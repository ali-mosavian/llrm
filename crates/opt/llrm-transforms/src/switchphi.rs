//! `forwardSwitchConditionToPHI`, a step of `simplifySwitch`
//! ([L6511](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6511)),
//! with `findPHIForConditionForwarding`
//! ([L6477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6477)).
//!
//! A phi input that is a case's constant, on the edge of that case, is the
//! switched value; so is the input of a phi behind an empty block the case
//! jumps to, when that makes two inputs alike or the phi already has the value.
//!
//! # How it works
//! For each case, a phi of its destination whose one input from the switch's
//! block is the case's constant takes the switched value instead. A phi behind
//! an empty block that only the switch enters, whose input from that block is
//! the constant, is collected; once all cases are seen, such a phi is changed
//! if two of its inputs are collected or it already has the switched value.
//!
//! # Example
//! ```text
//!   switch i16 %x, label %d [ i16 1, label %m ]        m:
//!   m:                                                   %r = phi i16 [ %x, %b0 ], [ 0, %d ]
//!     %r = phi i16 [ 1, %b0 ], [ 0, %d ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | The forwarding phis are visited in the order they were found; LLVM keys a `DenseMap` by phi, and the phis are independent. |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_analysis::occurrence::{arms, from_arms};
use llrm_mir::context::Context;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;

use crate::edges::{phis, single_predecessor};
use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;

/// `phi`'s input `index` replaced by `value`.
fn input_set(
    function: &mut Function,
    phi: InstId,
    index: usize,
    value: Operand,
) {
    let mut now = arms(function, phi);
    now[index].0 = value;
    function.set_operands(phi, from_arms(&now));
}

/// `findPHIForConditionForwarding`: the phi behind `block` that has `case` as
/// its input from `block`, and that input's index, if `block` is an empty
/// block only the switch enters.
fn phi_for_forwarding(
    function: &Function,
    case: Operand,
    block: BlockId,
) -> Option<(InstId, usize)> {
    // Empty, to be a candidate for simplification.
    if function
        .iter_block(block)
        .any(|one| function.instruction(one).opcode != Opcode::Phi && Some(one) != function.terminator(block))
    {
        return None;
    }
    // Dominated by the switch.
    single_predecessor(function, block)?;
    let last = function.terminator(block)?;
    let &[Operand::Block(successor)] = &function.instruction(last).operands[..] else { return None };
    phis(function, successor).into_iter().find_map(|phi| {
        let index =
            arms(function, phi).iter().position(|&(_, from)| from == block).expect("an input for each predecessor");
        (arms(function, phi)[index].0 == case).then_some((phi, index))
    })
}

/// Whether a phi input of a successor of the `switch` ending `block` became
/// the switched value.
pub fn folded(
    _options: &Options,
    _context: &Context,
    _spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let operands = function.instruction(last).operands.clone();
    let condition = operands[0];
    let mut forwarding: Vec<(InstId, Vec<usize>)> = Vec::new();
    let mut found: IdMap<InstId, usize> = IdMap::new();
    let mut changed = false;
    for pair in operands[2..].chunks(2) {
        let (case, Operand::Block(dest)) = (pair[0], pair[1]) else { continue };
        // Replace phi operands that use the case constant, not the switched
        // value. Only with exactly one edge from the switch to the phi: more
        // mean cases that map to one value, which is not the switched one.
        for phi in phis(function, dest) {
            let incoming = arms(function, phi);
            let index = incoming.iter().position(|&(_, from)| from == block).expect("an input for each predecessor");
            if incoming[index].0 == case && incoming.iter().filter(|&&(_, from)| from == block).count() == 1 {
                input_set(function, phi, index, condition);
                changed = true;
            }
        }
        // Phis that use the case constants indirectly.
        if let Some((phi, index)) = phi_for_forwarding(function, case, dest) {
            let at = match found.get(&phi) {
                Some(&at) => at,
                None => {
                    forwarding.push((phi, Vec::new()));
                    found.insert(phi, forwarding.len() - 1);
                    forwarding.len() - 1
                }
            };
            forwarding[at].1.push(index);
        }
    }
    for (phi, indexes) in forwarding {
        // Check if it helps to fold the phi.
        if indexes.len() < 2 && !arms(function, phi).iter().any(|&(value, _)| value == condition) {
            continue;
        }
        for index in indexes {
            input_set(function, phi, index, condition);
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "switchphi_tests.rs"]
mod tests;
