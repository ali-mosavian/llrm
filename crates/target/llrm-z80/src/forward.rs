//! A pair read again after the instruction that needed it in `hl`, `de` or
//! `bc` is read from that register, and `hl` is carried across a loop's back
//! edge in the register.
//!
//! # How it works
//! `add a,(hl)` takes its pointer from `hl`, so the pointer is copied in
//! (`COPY hl, v`) and `v` goes on to be stepped (`inc`) after the load. `v`
//! overlaps the register range that copy starts, so it cannot be `hl`: the
//! loop kept it in `bc` and copied it to `hl` every trip, or spilled.
//! Where nothing writes the register before `v`'s last use in the block, the
//! later uses read a copy of the register (`COPY v', hl`) instead. `v` then
//! ends at the copy-in, `v'` begins at the copy-out, and both can be `hl`.
//!
//! # Example
//! `COPY hl, v; ADD8m a, hl; COPY w, v; INC16 w` becomes `COPY hl, v; ADD8m
//! a, hl; COPY v', hl; COPY w, v'; INC16 w`.
//!
//! DEVIATE: no LLVM pass. Reason: the pin's `InterferenceCache` takes the
//! fixed range of a register copied from the interval itself as interference
//! (the matrix's `CoalescerPair` check does not), so region splitting never
//! puts a pointer that is stepped after its load in the register that loads
//! it. Exit: none; the form is exact for a use that does not write the pair.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};
use llrm_support::hash::{HashMap, HashSet};

use crate::mfunc::MFunction;
use crate::regalloc::Class;
use crate::{REGISTER_INFO, reg};

const PAIRS: [llrm_lir::registers::RegId; 3] = [reg::HL, reg::DE, reg::BC];

/// Reads the pair again from its register wherever that spares a value its
/// overlap with the register's range.
pub fn read_pairs_from_their_registers(function: &mut MFunction) {
    // the blocks each value is mentioned in, and how often it is defined
    let mut blocks_of: HashMap<u32, HashSet<usize>> = HashMap::default();
    let mut defs: HashMap<u32, u32> = HashMap::default();
    for (at, block) in function.blocks.iter().enumerate() {
        for inst in &block.insts {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => {
                        *defs.entry(*v).or_default() += 1;
                        blocks_of.entry(*v).or_default().insert(at);
                    }
                    Operand::Use(Reg::Virt(v)) => {
                        blocks_of.entry(*v).or_default().insert(at);
                    }
                    _ => {}
                }
            }
        }
        for phi in &block.phis {
            for reg in std::iter::once(&phi.result).chain(phi.incoming.iter().map(|(_, reg)| reg)) {
                if let Reg::Virt(v) = reg {
                    blocks_of.entry(*v).or_default().insert(usize::MAX);
                }
            }
        }
    }
    for at in 0..function.blocks.len() {
        let mut from = 0;
        while let Some(done) = forward_one(function, at, from, &blocks_of, &defs) {
            from = done;
        }
    }
}

/// Whether `inst` writes `pair`, or a byte or the whole of it, or calls.
fn clobbers(
    inst: &Inst,
    pair: llrm_lir::registers::RegId,
) -> bool {
    inst.call
        || inst.operands.iter().any(|operand| {
            matches!(
                operand,
                Operand::Def(Reg::Phys(one)) if REGISTER_INFO.root(*one) == REGISTER_INFO.root(pair)
            )
        })
}

fn reads(
    inst: &Inst,
    reg: Reg,
) -> bool {
    inst.operands.iter().any(|operand| *operand == Operand::Use(reg))
}

/// Rewrites the first candidate of block `at` from instruction `from` on;
/// the index to go on from, or `None` when there is none.
fn forward_one(
    function: &mut MFunction,
    at: usize,
    from: usize,
    blocks_of: &HashMap<u32, HashSet<usize>>,
    defs: &HashMap<u32, u32>,
) -> Option<usize> {
    let insts = &function.blocks[at].insts;
    for k in from..insts.len() {
        let (Kind::Copy, [Operand::Def(Reg::Phys(pair)), Operand::Use(Reg::Virt(v))]) =
            (insts[k].kind, &insts[k].operands[..])
        else {
            continue;
        };
        let (pair, v) = (*pair, *v);
        if !PAIRS.contains(&pair) || defs.get(&v) != Some(&1) || blocks_of[&v].len() != 1 {
            continue;
        }
        // the consumer: the next reader of the pair, with the pair untouched
        let Some(m) = (k + 1..insts.len()).find(|&m| reads(&insts[m], Reg::Phys(pair))) else { continue };
        if (k + 1..m).any(|between| clobbers(&insts[between], pair)) || clobbers(&insts[m], pair) {
            continue;
        }
        // the last use of the value after it, with the pair untouched until
        // then and the value not written again
        let Some(last) = (m + 1..insts.len()).rev().find(|&j| reads(&insts[j], Reg::Virt(v))) else { continue };
        if (m + 1..last).any(|between| clobbers(&insts[between], pair))
            || (m + 1..=last).any(|j| insts[j].operands.iter().any(|operand| *operand == Operand::Def(Reg::Virt(v))))
        {
            continue;
        }
        let class: Class = function.classes[v as usize];
        let fresh = function.classes.len() as u32;
        function.classes.push(class);
        let insts = &mut function.blocks[at].insts;
        for inst in &mut insts[m + 1..=last] {
            for operand in &mut inst.operands {
                if *operand == Operand::Use(Reg::Virt(v)) {
                    *operand = Operand::Use(Reg::Virt(fresh));
                }
            }
        }
        insts.insert(m + 1, Inst::copy(Reg::Virt(fresh), Reg::Phys(pair)));
        return Some(m + 2);
    }
    None
}

/// `hl` copied in at the top of a loop from a value the back edge carries is
/// carried in the register: the edges copy into it.
///
/// The pointer of `forward` still crossed the back edge as a value, and a value
/// that is live across a block with a range of `hl` in the middle is priced
/// two spills there (`addSplitConstraints`), more than the loop's other
/// choices: lz4c's checksum loop kept the pointer in the frame and loaded and
/// stored it every trip (24000 T). With the edges copying into `hl` itself,
/// no value is live across the range.
///
/// DEVIATE: see [`read_pairs_from_their_registers`].
pub fn carry_pairs_across_edges(function: &mut MFunction) {
    let succ = llrm_spike_mf::regalloc::successors(function);
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); function.blocks.len()];
    for (from, tos) in succ.iter().enumerate() {
        for &to in tos {
            if !preds[to].contains(&from) {
                preds[to].push(from);
            }
        }
    }
    for at in 0..function.blocks.len() {
        carry_one(function, at, &preds, &succ);
    }
}

fn carry_one(
    function: &mut MFunction,
    at: usize,
    preds: &[Vec<usize>],
    succ: &[Vec<usize>],
) {
    // a loop's top: one of its predecessors is reached from it
    let mut reached = vec![false; function.blocks.len()];
    let mut work = vec![at];
    while let Some(block) = work.pop() {
        for &next in &succ[block] {
            if !reached[next] {
                reached[next] = true;
                work.push(next);
            }
        }
    }
    if !preds[at].iter().any(|&pred| reached[pred]) {
        return;
    }
    let insts = &function.blocks[at].insts;
    for k in 0..insts.len() {
        let (Kind::Copy, [Operand::Def(Reg::Phys(pair)), Operand::Use(Reg::Virt(x))]) =
            (insts[k].kind, &insts[k].operands[..])
        else {
            continue;
        };
        let (pair, mut value) = (*pair, *x);
        // `hl` alone: a byte is read through it, and its range cannot move
        if pair != reg::HL {
            continue;
        }
        // back through the copies in this block to the value that arrives
        let mut chain = vec![k];
        loop {
            let Some(j) = (0..chain[chain.len() - 1])
                .rev()
                .find(|&j| insts[j].operands.iter().any(|operand| *operand == Operand::Def(Reg::Virt(value))))
            else {
                break;
            };
            let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(from))]) =
                (insts[j].kind, &insts[j].operands[..])
            else {
                chain.clear();
                break;
            };
            chain.push(j);
            value = *from;
        }
        // only the block's phi copies stand between its top and the copy in:
        // the register is held no longer than the copies take
        let only_copies = insts[..k]
            .iter()
            .all(
                |inst| matches!(inst.kind, Kind::Copy)
                    && inst.operands.iter().all(|operand| match operand {
                        Operand::Def(Reg::Phys(one)) | Operand::Use(Reg::Phys(one)) => {
                            REGISTER_INFO.root(*one) != REGISTER_INFO.root(pair)
                        }
                        _ => true,
                    }),
            );
        if chain.len() < 2 || !only_copies {
            continue;
        }
        let arriving = value;
        // the pair is untouched from the first copy of the chain to the last
        let first = *chain.last().expect("chain");
        if (first..k).any(|j| !chain.contains(&j) && clobbers(&insts[j], pair)) {
            continue;
        }
        // the value is defined by one copy in each predecessor and read only
        // by the first copy of the chain; the chain's values by the next copy
        let mut defs: Vec<(usize, usize)> = Vec::new();
        let mut uses = 0;
        for (b, block) in function.blocks.iter().enumerate() {
            for (j, inst) in block.insts.iter().enumerate() {
                for operand in &inst.operands {
                    match operand {
                        Operand::Def(Reg::Virt(v)) if *v == arriving => defs.push((b, j)),
                        Operand::Use(Reg::Virt(v)) if *v == arriving => uses += 1,
                        _ => {}
                    }
                }
            }
            if block.phis.iter().any(|phi| {
                phi.result == Reg::Virt(arriving) || phi.incoming.iter().any(|(_, reg)| *reg == Reg::Virt(arriving))
            }) {
                uses += 2;
            }
        }
        let mut sources: Vec<usize> = defs.iter().map(|(b, _)| *b).collect();
        sources.sort_unstable();
        let mut wanted = preds[at].clone();
        wanted.sort_unstable();
        if uses != 1 || sources != wanted || defs.is_empty() {
            continue;
        }
        // each definition is a copy that nothing after it in its block writes
        // the pair over
        let ok = defs
            .iter()
            .all(
                |&(b, j)| {
                    let block = &function.blocks[b].insts;
                    matches!(block[j].kind, Kind::Copy)
                        && !(j + 1..block.len())
                            .any(|after| clobbers(&block[after], pair) || reads(&block[after], Reg::Phys(pair)))
                        && !(b == at && j >= *chain.last().expect("chain"))
                },
            );
        // the chain's middle values are read once, by the next copy
        let middle_once = chain[1..]
            .iter()
            .all(
                |&j| {
                    let Operand::Def(Reg::Virt(v)) = insts[j].operands[0] else { return false };
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.insts)
                        .flat_map(|inst| &inst.operands)
                        .filter(|operand| **operand == Operand::Use(Reg::Virt(v)))
                        .count()
                        == 1
                },
            );
        if !ok || !middle_once {
            continue;
        }
        for &(b, j) in &defs {
            let Operand::Use(source) = function.blocks[b].insts[j].operands[1] else { continue };
            function.blocks[b].insts[j] = Inst::copy(Reg::Phys(pair), source);
        }
        let mut gone = chain;
        gone.sort_unstable_by(|a, b| b.cmp(a));
        for j in gone {
            function.blocks[at].insts.remove(j);
        }
        return;
    }
}
