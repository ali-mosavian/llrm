//! A pair that only its low byte is read from is a byte.
//!
//! # How it works
//! The values a phi merges (and the copies between them) are one component. A
//! component is narrowed when every definition is a widened byte (`ZEXT8`) or a
//! constant (`LD16ri`) or a copy within it, and every use is `LO` or a copy
//! within it: the high byte is made and never read. Its registers become byte
//! registers, the widening a copy of the byte, the constant its low byte, and
//! `LO` a copy.
//!
//! # Example
//! `strcmp` merges `zext c` and `0` into a pair, then subtracts the byte of it:
//! `ld c, a; ld b, #0` and `ld bc, #0` become a byte kept in `a`.
//!
//! DEVIATE: no LLVM pass. Reason: the pin keeps the `i16` phi as a register
//! pair and relies on sub-register liveness to drop the dead lane; the Z80's
//! pairs have no such tracking in the allocator. Exit: lane liveness in the
//! allocator.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};
use llrm_support::hash::HashMap;

use crate::mfunc::MFunction;
use crate::regalloc::Class;

fn find(
    parent: &mut [usize],
    mut at: usize,
) -> usize {
    while parent[at] != at {
        parent[at] = parent[parent[at]];
        at = parent[at];
    }
    at
}

/// Narrows every pair component of `function` that only its low byte is read
/// from.
pub fn narrow_low_only_pairs(function: &mut MFunction) {
    let count = function.classes.len();
    let pair = |function: &MFunction, v: u32| matches!(function.classes[v as usize], Class::Gr16);
    let mut parent: Vec<usize> = (0..count).collect();
    let mut bad = vec![false; count];
    for block in &function.blocks {
        for inst in &block.insts {
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Virt(a)), Operand::Use(Reg::Virt(b))]) => {
                    match (pair(function, *a), pair(function, *b)) {
                        (true, true) => {
                            let (x, y) = (find(&mut parent, *a as usize), find(&mut parent, *b as usize));
                            parent[x] = y;
                        }
                        (true, false) => bad[*a as usize] = true,
                        (false, true) => bad[*b as usize] = true,
                        (false, false) => {}
                    }
                }
                (Kind::Op("ZEXT8"), [Operand::Def(Reg::Virt(_)), Operand::Use(_)])
                | (Kind::Op("LD16ri"), [Operand::Def(Reg::Virt(_)), Operand::Imm(_)])
                | (Kind::Op("LO"), [Operand::Def(_), Operand::Use(Reg::Virt(_))]) => {}
                _ => {
                    for operand in &inst.operands {
                        if let Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) = operand {
                            if pair(function, *v) {
                                bad[*v as usize] = true;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut spoiled: HashMap<usize, bool> = HashMap::default();
    for v in 0..count {
        if pair(function, v as u32) {
            let root = find(&mut parent, v);
            *spoiled.entry(root).or_default() |= bad[v];
        }
    }
    let mut narrowed = Vec::new();
    for v in 0..count {
        if pair(function, v as u32) {
            let root = find(&mut parent, v);
            if !spoiled[&root] {
                narrowed.push(v);
            }
        }
    }
    if narrowed.is_empty() {
        return;
    }
    let is_narrowed: Vec<bool> = {
        let mut flags = vec![false; count];
        for &v in &narrowed {
            flags[v] = true;
        }
        flags
    };
    for &v in &narrowed {
        function.classes[v] = Class::Gr8;
    }
    for block in &mut function.blocks {
        for inst in &mut block.insts {
            let Some(Operand::Def(Reg::Virt(def))) = inst.operands.first().cloned() else { continue };
            match inst.kind {
                Kind::Op("ZEXT8") if is_narrowed[def as usize] => {
                    let Operand::Use(byte) = inst.operands[1] else { continue };
                    *inst = Inst { origin: inst.origin, ..Inst::copy(Reg::Virt(def), byte) };
                }
                Kind::Op("LD16ri") if is_narrowed[def as usize] => {
                    let Operand::Imm(value) = inst.operands[1] else { continue };
                    *inst = Inst {
                        origin: inst.origin,
                        ..Inst::new(Kind::Op("LD8ri"), vec![Operand::Def(Reg::Virt(def)), Operand::Imm(value & 0xFF)])
                    };
                }
                Kind::Op("LO") => {
                    let Operand::Use(Reg::Virt(from)) = inst.operands[1] else { continue };
                    if is_narrowed[from as usize] {
                        *inst = Inst { origin: inst.origin, ..Inst::copy(Reg::Virt(def), Reg::Virt(from)) };
                    }
                }
                _ => {}
            }
        }
    }
}
