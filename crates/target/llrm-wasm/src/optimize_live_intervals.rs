//! `WebAssemblyOptimizeLiveIntervals`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyOptimizeLiveIntervals.cpp)):
//! a register with several disconnected live ranges, as the coalescer leaves
//! after it joins unrelated copies, becomes one register per range, so that
//! `RegStackify` and `RegColoring` see single-definition registers wherever the
//! values are separate. The dead `IMPLICIT_DEF`s `FixIrreducibleControlFlow`
//! put in the entry block go.
//!
//! # How it works
//! `LiveIntervals::splitSeparateComponents` classifies a register's value
//! numbers by `ConnectedVNInfoEqClasses`: two definitions are one class when
//! some use is reached by both. Here the definitions reaching each use come
//! from a forward dataflow over the register's own definitions, and the classes
//! are their union-find sets.
//!
//! DEVIATE 1: no `LiveIntervals`; a use's reaching definitions replace the
//! value numbers (the same classes). The frame base register needs no special
//! case: nothing here records it. Exit: the shared live intervals (S4), no
//! later than 2026-11-20.
//!
//! # See also
//! `LiveIntervals.cpp`, pin d1106deb71cc: `splitSeparateComponents`;
//! `LiveInterval.cpp`: `ConnectedVNInfoEqClasses::Classify`.

use std::collections::BTreeSet;

use llrm_support::hash::HashMap;

use crate::mir::{Function, IMPLICIT_DEF, Operand, VReg};

pub fn optimize_live_intervals(f: &mut Function) -> bool {
    let mut defs: Vec<Vec<(usize, usize)>> = vec![Vec::new(); f.vreg_count()];
    for (b, block) in f.blocks.iter().enumerate() {
        for (i, inst) in block.iter().enumerate() {
            for reg in inst.defs() {
                if defs[reg as usize].last() != Some(&(b, i)) {
                    defs[reg as usize].push((b, i));
                }
            }
        }
    }
    let mut changed = false;
    for (reg, sites) in defs.iter().enumerate() {
        if sites.len() > 1 {
            changed |= split(f, reg as VReg, sites);
        }
    }
    changed | remove_dead_implicit_defs(f)
}

/// The union-find root of `x`.
fn find(
    parent: &mut [usize],
    mut x: usize,
) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// Split `reg`, defined at `sites` (more than one), into one register per class
/// of definitions that share a use.
fn split(
    f: &mut Function,
    reg: VReg,
    sites: &[(usize, usize)],
) -> bool {
    let blocks = f.blocks.len();
    let id: HashMap<(usize, usize), usize> = sites.iter().enumerate().map(|(n, &site)| (site, n)).collect();
    let mut last_def: Vec<Option<usize>> = vec![None; blocks];
    for (n, &(b, _)) in sites.iter().enumerate() {
        last_def[b] = Some(n);
    }
    // The definitions that reach the start of each block.
    let mut reach_in: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); blocks];
    loop {
        let mut grew = false;
        for b in 0..blocks {
            let out: BTreeSet<usize> = match last_def[b] {
                Some(d) => BTreeSet::from([d]),
                None => reach_in[b].clone(),
            };
            for &s in &f.succs[b] {
                for &d in &out {
                    grew |= reach_in[s].insert(d);
                }
            }
        }
        if !grew {
            break;
        }
    }
    let mut parent: Vec<usize> = (0..sites.len()).collect();
    let reaching = |b: usize, current: Option<usize>| -> Vec<usize> {
        match current {
            Some(d) => vec![d],
            None => reach_in[b].iter().copied().collect(),
        }
    };
    for (b, block) in f.blocks.iter().enumerate() {
        let mut current = None;
        for (i, inst) in block.iter().enumerate() {
            if inst.operands.contains(&Operand::Use(reg)) {
                let seen = reaching(b, current);
                for pair in seen.windows(2) {
                    let (a, c) = (find(&mut parent, pair[0]), find(&mut parent, pair[1]));
                    parent[a] = c;
                }
            }
            if let Some(&d) = id.get(&(b, i)) {
                current = Some(d);
            }
        }
    }
    // The classes, numbered by their first definition.
    let mut class_of: HashMap<usize, usize> = HashMap::default();
    let mut members = Vec::new();
    for n in 0..sites.len() {
        let root = find(&mut parent, n);
        let next = class_of.len();
        let class = *class_of.entry(root).or_insert(next);
        members.push(class);
    }
    if class_of.len() < 2 {
        return false;
    }
    let ty = f.vreg_type(reg);
    let names: Vec<VReg> = (0..class_of.len()).map(|class| if class == 0 { reg } else { f.new_vreg(ty) }).collect();
    for b in 0..blocks {
        let mut current = None;
        for i in 0..f.blocks[b].len() {
            let use_class = reaching(b, current).first().map(|&d| members[d]);
            let defined = id.get(&(b, i)).copied();
            for operand in &mut f.blocks[b][i].operands {
                match operand {
                    Operand::Use(r) if *r == reg => {
                        if let Some(class) = use_class {
                            *r = names[class];
                        }
                    }
                    Operand::Def(r) if *r == reg => *r = names[members[defined.expect("a recorded definition")]],
                    _ => {}
                }
            }
            if let Some(d) = defined {
                current = Some(d);
            }
        }
    }
    true
}

/// `FixIrreducibleControlFlow` put an `IMPLICIT_DEF` in the entry block for
/// every register a loop entry might read; those nothing reads are dead.
fn remove_dead_implicit_defs(f: &mut Function) -> bool {
    let uses = f.def_use();
    let before = f.blocks[0].len();
    f.blocks[0].retain(|inst| inst.opcode != IMPLICIT_DEF || inst.defs().any(|reg| !uses.use_empty(reg)));
    f.blocks[0].len() != before
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

    fn konst(
        reg: VReg,
        value: i64,
    ) -> Inst {
        Inst::new(op::CONST_I32, &[Operand::Def(reg), Operand::Imm(value)])
    }

    fn ret(reg: VReg) -> Inst {
        Inst::new(op::RETURN, &[Operand::Use(reg)])
    }

    /// `%0 = 1; use %0; %0 = 2; use %0` is two values in one register: the
    /// coalescer's join of two unrelated copies left a register `RegStackify`
    /// treats as several-definition, and every use of it stayed in a local.
    #[test]
    fn two_values_in_sequence_become_two_registers() {
        let drop = |r| Inst::new(ValType::I32.drop(), &[Operand::Use(r)]);
        let mut f = function(vec![vec![konst(0, 1), drop(0), konst(0, 2), ret(0)]], vec![vec![]], 1);
        assert!(optimize_live_intervals(&mut f));
        assert_eq!(f.vreg_count(), 2);
        let used: Vec<VReg> = f.blocks[0].iter().filter_map(|i| i.defs().next()).collect();
        assert_ne!(used[0], used[1]);
        assert_eq!(f.blocks[0][1].operands, vec![Operand::Use(used[0])]);
        assert_eq!(f.blocks[0][3].operands, vec![Operand::Use(used[1])]);
    }

    /// Both arms define `%0` and the join reads it: one value, one register.
    #[test]
    fn definitions_that_meet_at_a_use_stay_one_register() {
        let mut f = function(
            vec![
                vec![Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(1)])],
                vec![konst(0, 1), Inst::new(op::BR, &[Operand::Block(3)])],
                vec![konst(0, 2)],
                vec![ret(0)],
            ],
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            2,
        );
        assert!(!optimize_live_intervals(&mut f));
        assert_eq!(f.vreg_count(), 2);
    }

    /// A loop-carried register is one value: the use at the top reads both the
    /// initial definition and the one at the bottom.
    #[test]
    fn a_loop_carried_register_stays_one_register() {
        let add = Inst::new(op::ADD_I32, &[Operand::Def(0), Operand::Use(0), Operand::Use(0)]);
        let mut f = function(
            vec![
                vec![konst(0, 1)],
                vec![add, Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(1)])],
                vec![ret(0)],
            ],
            vec![vec![1], vec![1, 2], vec![]],
            2,
        );
        assert!(!optimize_live_intervals(&mut f));
    }

    /// An `IMPLICIT_DEF` nothing reads is removed from the entry block.
    #[test]
    fn a_dead_implicit_def_in_the_entry_block_goes() {
        let mut f =
            function(vec![vec![Inst::new(IMPLICIT_DEF, &[Operand::Def(0)]), konst(1, 3), ret(1)]], vec![vec![]], 2);
        assert!(optimize_live_intervals(&mut f));
        assert_eq!(f.blocks[0].len(), 2);
    }
}
