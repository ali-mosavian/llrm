//! `WebAssemblyMemIntrinsicResults`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyMemIntrinsicResults.cpp)):
//! `memcpy`, `memmove` and `memset` return their destination, so a later read
//! of the destination register that the call dominates reads the call's result
//! instead, and the destination need not survive the call in a local.
//!
//! DEVIATE 1: a register with more than one definition is left alone, on
//! either side; the pin compares value numbers of `LiveIntervals`. The
//! destination and the result of a call are each defined once in the code the
//! selector makes. Exit: the shared live intervals (S4), no later than
//! 2026-11-20.
//!
//! # See also
//! `WebAssemblyMemIntrinsicResults.cpp`, pin d1106deb71cc:
//! `replaceDominatedUses`, `optimizeCall`.

use crate::dagisel::post_ids as post;
use crate::mir::{Function, Operand, VReg};

/// The immediate dominator of each block reachable from the entry (the entry
/// its own), by the iterative algorithm over a reverse post-order.
pub fn dominators(f: &Function) -> Vec<Option<usize>> {
    let n = f.blocks.len();
    let mut order = Vec::with_capacity(n);
    let mut seen = vec![false; n];
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some(&mut (b, ref mut next)) = stack.last_mut() {
        if let Some(&s) = f.succs[b].get(*next) {
            *next += 1;
            if !seen[s] {
                seen[s] = true;
                stack.push((s, 0));
            }
        } else {
            order.push(b);
            stack.pop();
        }
    }
    order.reverse();
    let mut number = vec![usize::MAX; n];
    for (i, &b) in order.iter().enumerate() {
        number[b] = i;
    }
    let mut preds = vec![Vec::new(); n];
    for &b in &order {
        for &s in &f.succs[b] {
            preds[s].push(b);
        }
    }
    let mut idom: Vec<Option<usize>> = vec![None; n];
    idom[0] = Some(0);
    loop {
        let mut changed = false;
        for &b in order.iter().skip(1) {
            let mut new: Option<usize> = None;
            for &p in &preds[b] {
                if idom[p].is_none() {
                    continue;
                }
                new = Some(match new {
                    None => p,
                    Some(mut a) => {
                        let mut c = p;
                        while a != c {
                            while number[a] > number[c] {
                                a = idom[a].expect("processed");
                            }
                            while number[c] > number[a] {
                                c = idom[c].expect("processed");
                            }
                        }
                        a
                    }
                });
            }
            if new != idom[b] {
                idom[b] = new;
                changed = true;
            }
        }
        if !changed {
            return idom;
        }
    }
}

/// Whether block `a` dominates block `b`.
pub fn dominates(
    idom: &[Option<usize>],
    a: usize,
    mut b: usize,
) -> bool {
    loop {
        if a == b {
            return true;
        }
        match idom[b] {
            Some(up) if up != b => b = up,
            _ => return false,
        }
    }
}

/// Whether anything changed.
pub fn mem_intrinsic_results(f: &mut Function) -> bool {
    let uses = f.def_use();
    let mut idom = None;
    let mut changed = false;
    for b in 0..f.blocks.len() {
        for i in 0..f.blocks[b].len() {
            let inst = &f.blocks[b][i];
            if inst.opcode != post::CALL {
                continue;
            }
            let (Some(&Operand::Def(to)), Some(&Operand::Sym(at)), Some(&Operand::Use(from))) =
                (inst.operands.first(), inst.operands.get(1), inst.operands.get(2))
            else {
                continue;
            };
            if !matches!(
                f.symbols[at as usize].0.as_str(),
                "memcpy" | "memmove" | "memset"
            ) {
                continue;
            }
            if f.vreg_type(from) != f.vreg_type(to) || uses.def_count(from) != 1 || uses.def_count(to) != 1 {
                continue;
            }
            let idom = idom.get_or_insert_with(|| dominators(f)).clone();
            changed |= replace_dominated_uses(f, &idom, (b, i), from, to);
        }
    }
    changed
}

/// `replaceDominatedUses`: the reads of `from` the call at `site` dominates
/// read `to`.
fn replace_dominated_uses(
    f: &mut Function,
    idom: &[Option<usize>],
    (block, index): (usize, usize),
    from: VReg,
    to: VReg,
) -> bool {
    let mut changed = false;
    for b in 0..f.blocks.len() {
        let dominated = if b == block { true } else { dominates(idom, block, b) };
        if !dominated {
            continue;
        }
        let first = if b == block { index + 1 } else { 0 };
        for inst in &mut f.blocks[b][first..] {
            for operand in &mut inst.operands {
                if *operand == Operand::Use(from) {
                    *operand = Operand::Use(to);
                    changed = true;
                }
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::{Inst, ValType};

    fn function(
        blocks: Vec<Vec<Inst>>,
        succs: Vec<Vec<usize>>,
    ) -> Function {
        let mut f = Function::new("f");
        for _ in 0..6 {
            f.new_vreg(ValType::I32);
        }
        f.symbols.push(("memcpy".to_owned(), 0));
        f.blocks = blocks;
        f.succs = succs;
        f
    }

    fn call() -> Inst {
        Inst::new(post::CALL, &[Operand::Def(1), Operand::Sym(0), Operand::Use(0), Operand::Use(2), Operand::Use(3)])
    }

    /// `%1 = memcpy(%0, ..)`; a later read of `%0` is a read of `%1`. Before,
    /// `%0` stayed in a local across the call: one `local.get` more per
    /// `memcpy` whose destination is used after it.
    #[test]
    fn a_read_of_the_destination_after_the_call_reads_the_result() {
        let arg = Inst::new(op::ARGUMENT_i32, &[Operand::Def(0), Operand::Imm(0)]);
        let read = Inst::new(op::ADD_I32, &[Operand::Def(4), Operand::Use(0), Operand::Use(2)]);
        let mut f = function(vec![vec![arg, call(), read]], vec![vec![]]);
        assert!(mem_intrinsic_results(&mut f));
        assert_eq!(f.blocks[0][2].operands[1], Operand::Use(1));
        // The call's own arguments keep naming the destination.
        assert_eq!(f.blocks[0][1].operands[2], Operand::Use(0));
    }

    /// A read in a block the call does not dominate keeps the destination.
    #[test]
    fn a_read_the_call_does_not_dominate_is_left_alone() {
        let read = Inst::new(op::ADD_I32, &[Operand::Def(4), Operand::Use(0), Operand::Use(2)]);
        let branch = Inst::new(op::BR_IF, &[Operand::Block(2), Operand::Use(5)]);
        let mut f = function(
            vec![vec![branch], vec![call(), Inst::new(op::BR, &[Operand::Block(3)])], vec![read.clone()], vec![read]],
            vec![vec![1, 2], vec![3], vec![3], vec![]],
        );
        mem_intrinsic_results(&mut f);
        assert_eq!(f.blocks[3][0].operands[1], Operand::Use(0));
    }
}
