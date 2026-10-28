//! A spiller of the shape `InlineSpiller::spillAroundUses` makes, at the level
//! the greedy allocator needs to be driven: each instruction that names the
//! spilled register is rewritten to a new, tiny register, with a reload put
//! before it where it reads and a store after it where it writes. Translated
//! from llvm/lib/CodeGen/InlineSpiller.cpp (`spill`, `spillAll`,
//! `spillAroundUses`, `insertReload`, `insertSpill`) at d1106deb71cc.
//!
//! # How it works
//! The new ranges are `[reload, use)` and `[def, store)`: shorter than any
//! other range, so the weight calculation marks them not spillable and the
//! allocator gives them a register by evicting, or by recoloring.
//!
//! DEVIATE (no number yet, S1b): the reload and the store are placeholder
//! instructions that name the new register and nothing else (no frame cell, no
//! x86 form); folding a reload into its user (`foldMemoryOperand`),
//! rematerialization (`reMaterializeAll`), spill hoisting and sibling sharing
//! of stack slots are not translated.
//!
//! # Example
//! `%7` read at 48 and written at 64: two new registers, `[reload, 48r)` and
//! `[64r, store)`, and `%7`'s interval is removed.
//!
//! # Used by
//! `greedy`.

use std::sync::Arc;

use super::greedy::{SpillContext, Spiller};
use crate::liveness::slotindex::InstrKey;
use crate::model::lir::Insn;

/// `InlineSpiller`, in the form above.
#[derive(Default)]
pub struct SimpleSpiller {
    pub reloads: usize,
    pub stores: usize,
    pub folded: usize,
}

impl Spiller for SimpleSpiller {
    fn counts(&self) -> (usize, usize, usize) {
        (self.reloads, self.stores, self.folded)
    }

    fn spill(
        &mut self,
        ctx: SpillContext<'_, '_>,
        reg: u32,
        new_vregs: &mut Vec<u32>,
    ) {
        let SpillContext { lis, matrix, vrai, .. } = ctx;
        let vrm = &mut matrix.vrm;
        // The instructions that name the register, once each.
        let mut instrs: Vec<(InstrKey, bool, bool)> = Vec::new();
        for operand in lis.operands(reg).to_vec() {
            match instrs.iter_mut().find(|(key, _, _)| *key == operand.instr) {
                Some(found) => {
                    found.1 |= operand.reads_reg;
                    found.2 |= operand.is_def;
                }
                None => instrs.push((operand.instr, operand.reads_reg, operand.is_def)),
            }
        }
        for (instr, reads, writes) in instrs {
            let index = lis.indexes.get_instruction_index(instr);
            let block = lis.indexes.get_mbb_from_index(&index);
            // A read of a parallel copy is a load from the slot into its
            // destination (a folded reload: `foldMemoryOperand`):
            // no register holds it, as all the reads of a copy
            // would otherwise be live together.
            if reads && !writes && lis.instr(instr).group.is_some() {
                let mut folded = (**lis.instr(instr)).clone();
                folded.uses.retain(|value| *value != reg);
                folded.undefs.retain(|value| *value != reg);
                lis.rewrite_instr(block, instr, Arc::new(folded));
                self.folded += 1;
                continue;
            }
            // And a write of a parallel copy is a store of its source (a folded
            // spill): its destinations would otherwise all be defined, and
            // live, at the copy's one index.
            if writes && !reads && lis.instr(instr).group.is_some() {
                let mut folded = (**lis.instr(instr)).clone();
                folded.defines.retain(|value| *value != reg);
                folded.undefs.retain(|value| *value != reg);
                lis.rewrite_instr(block, instr, Arc::new(folded));
                self.folded += 1;
                continue;
            }
            let new_reg = lis.create_virtual_register();
            vrm.set_is_split_from_reg(new_reg, reg);
            // The instruction names the new register where it named the old
            // one.
            let rewritten = lis.instr(instr).renamed(&|value| if value == reg { new_reg } else { value });
            let rewritten_key = rewritten.id();
            lis.rewrite_instr(block, instr, Arc::clone(&rewritten));
            // A parallel copy is one instruction (a bundle): the reload goes
            // before the first of its members and the store after
            // the last.
            let (first, last) = match rewritten.group {
                Some(group) => {
                    let insns = lis.block_insns(block);
                    let at = insns.iter().position(|one| one.id() == rewritten_key).expect("in its block");
                    let mut from = at;
                    while from > 0 && insns[from - 1].group == Some(group) {
                        from -= 1;
                    }
                    let mut to = at;
                    while to + 1 < insns.len() && insns[to + 1].group == Some(group) {
                        to += 1;
                    }
                    (insns[from].id(), insns[to].id())
                }
                None => (rewritten_key, rewritten_key),
            };
            if reads {
                // insertReload
                let reload = Arc::new(Insn::new(0, None, None, vec![new_reg], Vec::new()));
                lis.insert_instr_before(block, Some(first), reload);
                self.reloads += 1;
            }
            if writes {
                // insertSpill
                let store = Arc::new(Insn::new(0, None, None, Vec::new(), vec![new_reg]));
                lis.insert_instr_after(block, last, store);
                self.stores += 1;
            }
            let mut li = lis.compute_virt_reg_interval(new_reg);
            vrai.calculate_spill_weight_and_hint(&mut li, lis);
            lis.insert_interval(li);
            new_vregs.push(new_reg);
        }
        // Nothing names the spilled register any more.
        lis.remove_interval(reg);
    }
}

/// A spiller that only records the decision: the spilled range leaves the
/// allocation problem (its reloads and stores are made later, by the caller's
/// emitter, and allocated in the next round).
#[derive(Default)]
pub struct RecordingSpiller {
    pub spilled: Vec<u32>,
}

impl Spiller for RecordingSpiller {
    fn spill(
        &mut self,
        ctx: SpillContext<'_, '_>,
        reg: u32,
        _new_vregs: &mut Vec<u32>,
    ) {
        ctx.lis.remove_interval(reg);
        self.spilled.push(reg);
    }

    fn recorded(&self) -> Vec<u32> {
        self.spilled.clone()
    }
}
