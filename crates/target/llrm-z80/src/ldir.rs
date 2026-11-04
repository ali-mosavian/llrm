//! A byte-copy loop as `ldir`.
//!
//! # How it works
//! A block that is its own loop, with three phis and nothing else but
//!
//! ```text
//! a = load (s);  store a -> (d);  s' = s + 1;  d' = d + 1;  n' = n - 1;  continue unless n' == 0
//! ```
//!
//! copies `n` bytes forward one at a time, an overlap repeating; that is what
//! `ldir` does, in 21 T a byte against the loop's 52 and with no registers
//! for the loop's own state. The loop becomes the three pairs copied into the
//! fixed registers (`hl` the destination, `de` the source, `bc` the count: the
//! runtime's `memcpy`'s), the instruction, and the values after the loop copied
//! out of what it leaves (`hl` the end of the source, `de` the end of the
//! destination, `bc` zero).
//!
//! A count of zero takes the loop 65536 times, as it does `ldir`: no test is
//! kept or needed. A volatile access is not a candidate: `ldir` makes no
//! promise of the loop's accesses, one each, in this order.
//!
//! DEVIATE: no LLVM pass. Reason: the pin's `LoopIdiomRecognize` forms a
//! `memcpy` or `memmove` only where it proves no overlap or a direction, and
//! the overlap of an LZ4 match copy is neither; `ldir` is the instruction whose
//! semantics are this loop's. Exit: none; the target's own form of the idiom
//! (X86's `rep movsb` from a loop is the same wish).
//!
//! # See also
//! `llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp`.

use std::collections::{HashMap, HashSet};

use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use crate::reg;

/// The loops of `function` that copy bytes, as `ldir`.
pub fn copy_loops_to_ldir(function: &mut MFunction) -> bool {
    let mut changed = false;
    for at in 0..function.blocks.len() {
        if let Some(found) = recognised(function, at) {
            apply(function, at, found);
            changed = true;
        }
    }
    changed
}

/// A copy loop and what to rewrite.
struct Found {
    /// The block that enters the loop, as a phi names it.
    pre: usize,
    /// Where the loop is left, a block id.
    exit: u32,
    /// The phi results: destination, source and count.
    dst: Reg,
    src: Reg,
    count: Reg,
    /// Values defined in the loop and used after it: the end of the source, of
    /// the destination, and the zero count.
    src_end: Vec<Reg>,
    dst_end: Vec<Reg>,
    count_end: Vec<Reg>,
}

fn is_block(
    operand: &Operand,
    id: u32,
) -> bool {
    *operand == Operand::Block(id)
}

fn recognised(
    function: &MFunction,
    at: usize,
) -> Option<Found> {
    let block = &function.blocks[at];
    if block.phis.len() != 3 || block.insts.len() < 4 {
        return None;
    }
    let index_of = |id: u32| function.blocks.iter().position(|one| one.id == id);
    // the three trailing instructions: the compare, the conditional jump and
    // the jump
    let [.., cmp, jcc, jump] = &block.insts[..] else { return None };
    let (Kind::Op("CP16i"), Kind::Op(cond), Kind::Op("JP")) = (cmp.kind, jcc.kind, jump.kind) else { return None };
    let [Operand::Block(first), ..] = jcc.operands[..] else { return None };
    let [Operand::Block(second)] = jump.operands[..] else { return None };
    // continue to this block: directly, or through a block that only jumps
    // back
    let (exit, latch) = match cond {
        "JPNZ" if first == block.id => (second, None),
        "JPZ" => {
            let latch = index_of(second)?;
            let only_back = matches!(
                &function.blocks[latch].insts[..],
                [one] if one.kind == Kind::Op("JP") && is_block(&one.operands[0], block.id)
            ) && function.blocks[latch].phis.is_empty();
            if !only_back {
                return None;
            }
            (first, Some(latch))
        }
        _ => return None,
    };
    // each phi: the value from the entering block, the value from the loop
    let back = latch.unwrap_or(at);
    let mut pre = None;
    let mut phis = Vec::new();
    for phi in &block.phis {
        let [(one, from_one), (other, from_other)] = phi.incoming[..] else { return None };
        let ((p, entered), looped) =
            if one == back { ((other, from_other), from_one) } else { ((one, from_one), from_other) };
        if (one == back) == (other == back) || pre.is_some_and(|known| known != p) {
            return None;
        }
        pre = Some(p);
        phis.push((phi.result, entered, looped));
    }
    let pre = pre?;
    let mut root: HashMap<Reg, Reg> = HashMap::new();
    let resolve = |root: &HashMap<Reg, Reg>, reg: Reg| *root.get(&reg).unwrap_or(&reg);
    let (mut load, mut store, mut incs, mut decs, mut byte_out, mut byte_in) =
        (None, None, Vec::new(), Vec::new(), 0, 0);
    let accumulator = Reg::Phys(reg::A);
    let mut defined: HashSet<Reg> = block.phis.iter().map(|phi| phi.result).collect();
    for inst in &block.insts[..block.insts.len() - 3] {
        for operand in &inst.operands {
            if let Operand::Def(reg @ Reg::Virt(_)) = operand {
                defined.insert(*reg);
            }
        }
        match (inst.kind, &inst.operands[..]) {
            (Kind::Copy, [Operand::Def(to @ Reg::Virt(_)), Operand::Use(from @ Reg::Virt(_))]) => {
                let target = resolve(&root, *from);
                root.insert(*to, target);
            }
            // the byte, out of a and back into it
            (Kind::Copy, [Operand::Def(Reg::Virt(_)), Operand::Use(from)]) if *from == accumulator => byte_out += 1,
            (Kind::Copy, [Operand::Def(to), Operand::Use(Reg::Virt(_))]) if *to == accumulator => byte_in += 1,
            (Kind::Op("LD8Arp"), [Operand::Def(a), Operand::Use(pointer)])
                if *a == accumulator && load.is_none() && !inst.side_effects =>
            {
                load = Some(resolve(&root, *pointer));
            }
            (Kind::Op("LD8prA"), [Operand::Use(a), Operand::Use(pointer)])
                if *a == accumulator && store.is_none() && !inst.side_effects =>
            {
                store = Some(resolve(&root, *pointer));
            }
            (Kind::Op("INC16"), [Operand::Def(out), Operand::Use(from)]) => incs.push((*out, resolve(&root, *from))),
            (Kind::Op("DEC16"), [Operand::Def(out), Operand::Use(from)]) => decs.push((*out, resolve(&root, *from))),
            _ => return None,
        }
    }
    // the load before the store, the byte passing through a once
    let (load, store) = (load?, store?);
    if incs.len() != 2 || decs.len() != 1 || byte_out > 1 || byte_in > 1 {
        return None;
    }
    let result_of = |reg: Reg| phis.iter().find(|(result, ..)| *result == reg).copied();
    let (src, dst) = (result_of(load)?, result_of(store)?);
    let (dec_out, dec_from) = decs[0];
    let count = result_of(dec_from)?;
    if src.0 == dst.0 || src.0 == count.0 || dst.0 == count.0 {
        return None;
    }
    // each state steps by its own add and loops back as that add's result
    let steps = |state: (Reg, Reg, Reg), steps: &[(Reg, Reg)]| -> Option<Reg> {
        let (result, _, looped) = state;
        let (out, _) = steps.iter().find(|(_, from)| *from == result)?;
        (resolve(&root, looped) == *out).then_some(*out)
    };
    let src_out = steps(src, &incs)?;
    let dst_out = steps(dst, &incs)?;
    let count_out = steps(count, &decs)?;
    if src_out == dst_out || count_out != dec_out {
        return None;
    }
    // the exit tests the new count
    let Operand::Use(tested) = cmp.operands[1] else { return None };
    if resolve(&root, tested) != dec_out || cmp.operands.get(2) != Some(&Operand::Imm(0)) {
        return None;
    }
    // what the loop defines and the rest of the function reads: only the
    // ends, which `ldir` leaves
    let mut src_end = Vec::new();
    let mut dst_end = Vec::new();
    let mut count_end = Vec::new();
    let mut outside = |reg: Reg| -> bool {
        if !defined.contains(&reg) {
            return true;
        }
        let end = resolve(&root, reg);
        let list = if end == src_out {
            &mut src_end
        } else if end == dst_out {
            &mut dst_end
        } else if end == count_out {
            &mut count_end
        } else {
            return false;
        };
        if !list.contains(&reg) {
            list.push(reg);
        }
        true
    };
    for (index, other) in function.blocks.iter().enumerate() {
        if index == at || Some(index) == latch {
            continue;
        }
        let uses = other
            .insts
            .iter()
            .flat_map(|inst| inst.operands.iter())
            .filter_map(
                |operand| match operand {
                    Operand::Use(reg) => Some(*reg),
                    _ => None,
                },
            );
        let in_phis = other.phis.iter().flat_map(|phi| phi.incoming.iter().map(|(_, reg)| *reg));
        if !uses.chain(in_phis).all(&mut outside) {
            return None;
        }
    }
    Some(Found { pre, exit, dst: dst.0, src: src.0, count: count.0, src_end, dst_end, count_end })
}

fn apply(
    function: &mut MFunction,
    at: usize,
    found: Found,
) {
    let (hl, de, bc) = (Reg::Phys(reg::HL), Reg::Phys(reg::DE), Reg::Phys(reg::BC));
    let block: &mut Block = &mut function.blocks[at];
    let mut insts = Vec::new();
    // the phis had one way in: their values are the entering block's
    for phi in std::mem::take(&mut block.phis) {
        let entered = phi.incoming.iter().find(|(from, _)| *from == found.pre).map(|(_, reg)| *reg).expect("entered");
        insts.push(Inst::copy(phi.result, entered));
    }
    insts.push(Inst::copy(hl, found.dst));
    insts.push(Inst::copy(de, found.src));
    insts.push(Inst::copy(bc, found.count));
    insts.push(Inst {
        sets_flags: true,
        may_load: true,
        may_store: true,
        ..Inst::new(
            Kind::Op("LDIR"),
            vec![
                Operand::Use(hl),
                Operand::Use(de),
                Operand::Use(bc),
                Operand::Def(hl),
                Operand::Def(de),
                Operand::Def(bc),
                Operand::Def(Reg::Phys(reg::FLAGS)),
            ],
        )
    });
    // `ex de,hl` before the copy leaves the end of the source in hl and of
    // the destination in de
    for (regs, from) in [(&found.src_end, hl), (&found.dst_end, de), (&found.count_end, bc)] {
        insts.extend(regs.iter().map(|reg| Inst::copy(*reg, from)));
    }
    let mut jump = block.insts.pop().expect("a jump");
    let _ = block.insts.pop();
    let _ = block.insts.pop();
    jump.operands = vec![Operand::Block(found.exit)];
    insts.push(jump);
    block.insts = insts;
}
