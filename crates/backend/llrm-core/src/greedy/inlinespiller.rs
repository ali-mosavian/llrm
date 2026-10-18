//! `InlineSpiller`: spills a live range by rewriting each instruction that
//! names it through the stack, inside the allocation in progress. Translated
//! from llvm/lib/CodeGen/InlineSpiller.cpp (`spill`, `spillAll`,
//! `spillAroundUses`, `insertReload`, `insertSpill`) at d1106deb71cc.
//!
//! # How it works
//! `spill(reg)` takes every instruction that names `reg` (a parallel copy is
//! one unit) and has the per-instruction decisions of `backend::spiller` make
//! its replacement: a reload before, the instruction rewritten, a store after,
//! or the stack cell folded into the instruction itself. The replacement is put
//! into the edited function with `LiveIntervals::rewrite_instr` (the
//! instruction keeps its slot, so a call keeps its register mask) and
//! `insert_instr_before/after` (new slots between the neighbours: no other
//! interval moves). Each register the replacement names for the first time is
//! made with `LiveRangeEdit::create_from`, gets its interval and weight, and is
//! returned for the queue. The interval of `reg` is erased. Nothing is rebuilt:
//! this is why a spill no longer restarts the allocation.
//!
//! DEVIATE D-A: the choice of reload, store or fold is `backend::spiller`'s
//! (`_source`, `_in_place`, `_tied`, the parallel-copy folds), asked for one
//! instruction at a time through `spilled_from` on a body of that instruction.
//! The fact: the m16 forms (segment-offset pairs, 8/16/32-bit lanes, the folds
//! into a parallel copy) are not forms `TargetInstrInfo::foldMemoryOperand`
//! knows. Exit: those forms expressed as `foldMemoryOperand` rules, with the
//! spiller's whole-body plan (copies, merged updates) gone; issue to be
//! opened with step 2. Boundary: a replacement that does not hold the same
//! number of instructions as the unit it replaces, or that still names `reg`,
//! sets `failed` and the caller falls back to the rounds.
//!
//! DEVIATE D-B: no `analyzeSiblingValues`, `hoistAllSpills` or
//! `reMaterializeAll` ahead of the spill: they need SplitKit's sibling
//! relations (step 2). Exit: SplitKit translated.
//!
//! DEVIATE D-C: a spill slot is a `Frame` cell per original register, shared
//! afterwards by `slots_colored`; LLVM shares through `LiveStacks` as it goes.
//! Exit: `LiveStacks` translated with SplitKit.
//!
//! # Example
//! `%7` read at 48 and written at 64 becomes two registers, `[reload, 48r)` and
//! `[64r, store)`, with a frame cell between them; `%7` has no interval.
//!
//! # Used by
//! `shadow::decide_inline`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::greedy::{SpillContext, Spiller};
use super::liverangeedit::{LiveRangeEdit, NoDelegate};
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::Frame;
use crate::backend::spiller::spilled_from;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
use crate::model::lir::{Block, Insn, LirBlock, LirBody};

/// What the allocator keeps per register and the spiller must extend for the
/// registers it makes: the allocation order and the widest access.
#[derive(Default)]
pub struct Tables {
    pub orders: RefCell<HashMap<u32, Vec<RegId>>>,
    pub widths: RefCell<HashMap<u32, u32>>,
}

/// `InlineSpiller`.
pub struct InlineSpiller<'a> {
    template: &'a LirBody,
    frame: &'a mut Frame,
    classes: &'a RegisterClasses,
    order_of: &'a dyn Fn(u32) -> Vec<RegId>,
    tables: Rc<Tables>,
    reloads: usize,
    stores: usize,
    folded: usize,
    failed: bool,
    spilled: Vec<u32>,
}

impl<'a> InlineSpiller<'a> {
    pub fn new(
        template: &'a LirBody,
        frame: &'a mut Frame,
        classes: &'a RegisterClasses,
        order_of: &'a dyn Fn(u32) -> Vec<RegId>,
        tables: Rc<Tables>,
    ) -> Self {
        Self { template, frame, classes, order_of, tables, reloads: 0, stores: 0, folded: 0, failed: false, spilled: Vec::new() }
    }

    /// The units of `reg`'s instructions: a parallel copy is one, listed by its
    /// members in block order.
    fn units(
        lis: &LiveIntervals,
        reg: u32,
    ) -> Vec<(usize, Vec<InstrKey>)> {
        let mut keys: Vec<InstrKey> = Vec::new();
        for operand in lis.operands(reg) {
            if !keys.contains(&operand.instr) {
                keys.push(operand.instr);
            }
        }
        let mut units: Vec<(usize, Vec<InstrKey>)> = Vec::new();
        for key in keys {
            if units.iter().any(|(_, members)| members.contains(&key)) {
                continue;
            }
            let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(key));
            let insns = lis.block_insns(block);
            let at = insns.iter().position(|one| one.id() == key).expect("in its block");
            let members = match insns[at].group {
                None => vec![key],
                Some(group) => {
                    let (mut from, mut to) = (at, at);
                    while from > 0 && insns[from - 1].group == Some(group) {
                        from -= 1;
                    }
                    while to + 1 < insns.len() && insns[to + 1].group == Some(group) {
                        to += 1;
                    }
                    insns[from..=to].iter().map(|one| one.id()).collect()
                }
            };
            units.push((block, members));
        }
        units
    }

    /// `spillAroundUses` for one unit. Returns the registers it made.
    fn spill_unit(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        block: usize,
        members: &[InstrKey],
    ) -> Option<Vec<u32>> {
        let old: Vec<Arc<Insn>> = members.iter().map(|key| Arc::clone(lis.instr(*key))).collect();
        let mut mini = self.template.with_blocks(vec![Block::from(LirBlock::new(0, old.clone()))]);
        mini.entry = 0;
        mini.inputs.clear();
        mini.frequencies = None;
        let values = std::iter::once(reg).collect();
        let Ok((out, made)) = spilled_from(&mini, &values, Some(&mut *self.frame), lis.next_virtual_register(), self.classes)
        else {
            return None;
        };
        let out = out.insns();
        let is_core = |one: &Arc<Insn>| !one.spill_reload && !one.spill_store;
        let cores: Vec<&Arc<Insn>> = out.iter().filter(|one| is_core(one)).collect();
        if cores.len() != members.len() || cores.iter().any(|one| one.defines.contains(&reg) || one.uses.contains(&reg)) {
            return None;
        }
        let mut keys: Vec<InstrKey> = Vec::with_capacity(cores.len());
        for (key, core) in members.iter().zip(&cores) {
            lis.rewrite_instr(block, *key, Arc::clone(core));
            keys.push(core.id());
        }
        let mut seen = 0;
        let mut last = *keys.last().expect("a unit has members");
        for one in &out {
            if is_core(one) {
                seen += 1;
            } else if seen < keys.len() {
                lis.insert_instr_before(block, Some(keys[seen]), Arc::clone(one));
            } else {
                lis.insert_instr_after(block, last, Arc::clone(one));
                last = one.id();
            }
            if one.spill_reload {
                self.reloads += 1;
            }
            if one.spill_store {
                self.stores += 1;
            }
        }
        if out.iter().all(is_core) {
            self.folded += 1;
        }
        // A register an instruction requires or delivers in one register can be
        // given that one only.
        let pinned: HashMap<u32, RegId> = out
            .iter()
            .flat_map(|one| one.requires.iter().chain(&one.delivers))
            .map(|(held, register)| (held.value, crate::model::ir::root(*register)))
            .collect();
        let width = self.tables.widths.borrow().get(&reg).copied().unwrap_or(0);
        let mut fresh: Vec<u32> = made.into_iter().filter(|value| !lis.reg_nodbg_empty(*value)).collect();
        fresh.sort_unstable();
        for &value in &fresh {
            lis.reserve_virtual_register(value);
            let mut order: Vec<RegId> =
                (self.order_of)(reg).into_iter().map(crate::model::ir::root).collect();
            order.dedup();
            if let Some(only) = pinned.get(&value) {
                order = vec![*only];
            }
            self.tables.orders.borrow_mut().insert(value, order);
            self.tables.widths.borrow_mut().insert(value, width);
        }
        Some(fresh)
    }
}

impl Spiller for InlineSpiller<'_> {
    fn spill(
        &mut self,
        ctx: SpillContext<'_, '_>,
        reg: u32,
        new_vregs: &mut Vec<u32>,
    ) {
        let SpillContext { lis, vrm, vrai } = ctx;
        self.spilled.push(reg);
        let mut delegate = NoDelegate;
        let mut edit = LiveRangeEdit::new(reg, &mut delegate);
        for (block, members) in Self::units(lis, reg) {
            let Some(fresh) = self.spill_unit(lis, reg, block, &members) else {
                self.failed = true;
                return;
            };
            for value in fresh {
                edit.create_from(lis, vrm, value, reg);
                let mut li = lis.compute_virt_reg_interval(value);
                vrai.calculate_spill_weight_and_hint(&mut li, lis);
                lis.insert_interval(li);
                new_vregs.push(value);
            }
        }
        edit.erase_virt_reg(lis, reg);
    }

    fn counts(&self) -> (usize, usize, usize) {
        (self.reloads, self.stores, self.folded)
    }

    fn failed(&self) -> bool {
        self.failed
    }

    fn recorded(&self) -> Vec<u32> {
        self.spilled.clone()
    }
}
