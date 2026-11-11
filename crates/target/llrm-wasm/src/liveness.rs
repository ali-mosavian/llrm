//! Block liveness of the machine function, the one analysis `PHIElimination`'s
//! edge split, `RegStackify` and `RegColoring` ask (`LiveVariables` and
//! `LiveIntervals` in the pin).

use llrm_mir::dense::IdSet;

use crate::mir::{Function, Operand, PHI, VReg};

/// `LiveVariables`' liveness per block: `(live-in, live-out)`. A PHI's use is
/// in its predecessor, so a register only a PHI reads is not live out of that
/// predecessor, and is live into it when defined before. The sets are bit
/// vectors over the register numbers, as the pin's `LiveVariables` keeps them
/// (`SparseBitVector`), not hashed.
pub fn liveness(f: &Function) -> (Vec<IdSet<VReg>>, Vec<IdSet<VReg>>) {
    let n = f.blocks.len();
    let (mut used, mut defined) = (vec![IdSet::new(); n], vec![IdSet::new(); n]);
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
    let (mut live_in, mut live_out) = (used.clone(), vec![IdSet::new(); n]);
    loop {
        let mut changed = false;
        for b in (0..n).rev() {
            let mut out = IdSet::new();
            for &s in &f.succs[b] {
                out.union_with(&live_in[s]);
            }
            let mut inn = used[b].clone();
            for r in out.difference(&defined[b]) {
                inn.insert(r);
            }
            // Both only grow, so a changed size is the only change.
            if out.len() != live_out[b].len() || inn.len() != live_in[b].len() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::{Inst, ValType};

    fn function(
        blocks: Vec<Vec<Inst>>,
        succs: Vec<Vec<usize>>,
        vregs: usize,
    ) -> Function {
        let mut f = Function::new("f");
        for _ in 0..vregs {
            f.new_vreg(ValType::I32);
        }
        f.blocks = blocks;
        f.succs = succs;
        f
    }

    fn def(reg: VReg) -> Inst {
        Inst::new(op::CONST_I32, &[Operand::Def(reg), Operand::Imm(1)])
    }

    fn read(reg: VReg) -> Inst {
        Inst::new(op::RETURN, &[Operand::Use(reg)])
    }

    /// A value defined in the first block and read in the last is live out of
    /// every block but the last and into every block but the first, and the
    /// registers come out in ascending order: the hashed sets listed them in a
    /// different order on every run, so `RegColoring` numbered locals
    /// differently from one compile to the next.
    #[test]
    fn a_value_read_in_the_last_block_is_live_through_the_chain_in_register_order() {
        let blocks = vec![vec![def(0), def(5), def(3)], vec![], vec![], vec![read(0), read(5), read(3)]];
        let f = function(blocks, vec![vec![1], vec![2], vec![3], vec![]], 6);
        let (live_in, live_out) = liveness(&f);
        for b in 0..3 {
            assert_eq!(live_out[b].iter().collect::<Vec<_>>(), [0, 3, 5], "live out of {b}");
        }
        for b in 1..4 {
            assert_eq!(live_in[b].iter().collect::<Vec<_>>(), [0, 3, 5], "live into {b}");
        }
        assert!(live_in[0].is_empty() && live_out[3].is_empty());
    }

    /// A read before the definition in a loop body keeps the register live
    /// around the back edge, out of the body and into the header.
    #[test]
    fn a_read_before_the_definition_in_a_loop_is_live_around_the_back_edge() {
        let blocks = vec![vec![def(0)], vec![read(0), def(0)], vec![]];
        let f = function(blocks, vec![vec![1], vec![1, 2], vec![]], 1);
        let (live_in, live_out) = liveness(&f);
        assert!(live_in[1].contains(&0) && live_out[1].contains(&0) && live_out[0].contains(&0));
        assert!(!live_in[2].contains(&0));
    }
}
