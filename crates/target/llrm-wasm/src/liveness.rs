//! Block liveness of the machine function, the one analysis `PHIElimination`'s
//! edge split, `RegStackify` and `RegColoring` ask (`LiveVariables` and
//! `LiveIntervals` in the pin).

use llrm_support::hash::HashSet;

use crate::mir::{Function, Operand, PHI, VReg};

/// `LiveVariables`' liveness per block: `(live-in, live-out)`. A PHI's use is
/// in its predecessor, so a register only a PHI reads is not live out of that
/// predecessor, and is live into it when defined before.
pub fn liveness(f: &Function) -> (Vec<HashSet<VReg>>, Vec<HashSet<VReg>>) {
    let n = f.blocks.len();
    let (mut used, mut defined) = (vec![HashSet::default(); n], vec![HashSet::default(); n]);
    for (b, block) in f.blocks.iter().enumerate() {
        for inst in block {
            if inst.opcode != PHI {
                for operand in &inst.operands {
                    if let Operand::Use(r) = operand
                        && !defined[b].contains(r)
                    {
                        used[b].insert(*r);
                    }
                }
            }
            for operand in &inst.operands {
                if let Operand::Def(r) = operand {
                    defined[b].insert(*r);
                }
            }
        }
    }
    // A PHI's use is at the end of its predecessor.
    for block in &f.blocks {
        for phi in block.iter().take_while(|inst| inst.opcode == PHI) {
            for pair in phi.operands[1..].chunks(2) {
                if let [Operand::Use(r), Operand::Block(pred)] = *pair
                    && !defined[pred as usize].contains(&r)
                {
                    used[pred as usize].insert(r);
                }
            }
        }
    }
    let (mut live_in, mut live_out) = (used.clone(), vec![HashSet::default(); n]);
    loop {
        let mut changed = false;
        for b in (0..n).rev() {
            let out: HashSet<VReg> = f.succs[b].iter().flat_map(|&s| live_in[s].iter().copied()).collect();
            let inn: HashSet<VReg> =
                used[b].iter().copied().chain(out.iter().filter(|r| !defined[b].contains(r)).copied()).collect();
            if out != live_out[b] || inn != live_in[b] {
                live_out[b] = out;
                live_in[b] = inn;
                changed = true;
            }
        }
        if !changed {
            return (live_in, live_out);
        }
    }
}
