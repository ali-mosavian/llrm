//! `LiveIntervals`: the live interval of every virtual register of a body,
//! computed once from its definitions and uses. Translated from
//! llvm/lib/CodeGen/LiveIntervals.cpp (`computeVirtRegInterval`,
//! `createAndComputeVirtRegInterval`, `getInterval`) at d1106deb71cc.
//!
//! # How it works
//! `new` numbers the instructions (`SlotIndexes`), builds the CFG and dominator
//! tree and lists, for each value, the operands that define or read it (the
//! body's `MachineRegisterInfo`). `interval(reg)` builds the interval of one
//! value with `LiveIntervalCalc::calculate` the first time it is asked.
//!
//! DEVIATE (no number): the operand lists are made by one pass over the
//! instructions rather than kept as use-def chains; a body with phis is not
//! translated (the allocator's input has none: PHI elimination is before it).
//!
//! # Example
//! `%1 = 5; %2 = %1 + 1; use %2`: `%1` is `[16r,32r)`, `%2` is `[32r,48r)`.
//!
//! # Used by
//! The allocator's shadow core.

use std::collections::HashMap;

use super::cfg::{Cfg, DomTree};
use super::liverange::LiveInterval;
use super::liverangecalc::{Ctx, LiveIntervalCalc, Operand};
use super::slotindex::SlotIndexes;
use crate::model::lir::LirBody;

/// `LiveIntervals` (LiveIntervals.h).
pub struct LiveIntervals {
    pub indexes: SlotIndexes,
    pub cfg: Cfg,
    pub dom: DomTree,
    operands: HashMap<u32, Vec<Operand>>,
    /// The values defined on entry (parameters), which no instruction defines.
    inputs: std::collections::BTreeSet<u32>,
    intervals: HashMap<u32, LiveInterval>,
    calc: LiveIntervalCalc,
}

impl LiveIntervals {
    pub fn new(body: &LirBody) -> Self {
        assert!(body.blocks.iter().all(|block| block.phis.is_empty()), "phis are eliminated before allocation");
        let indexes = SlotIndexes::new(body);
        let cfg = Cfg::of(body);
        let dom = DomTree::of(&cfg);
        let mut operands: HashMap<u32, Vec<Operand>> = HashMap::new();
        for block in &body.blocks {
            for one in block.insns.iter() {
                let instr = one.id();
                for &value in &one.defines {
                    operands
                        .entry(value)
                        .or_default()
                        .push(
                            Operand {
                                instr,
                                is_def: true,
                                reads_reg: false,
                                early_clobber: false,
                            },
                        );
                }
                for &value in &one.uses {
                    operands
                        .entry(value)
                        .or_default()
                        .push(
                            Operand {
                                instr,
                                is_def: false,
                                reads_reg: true,
                                early_clobber: false,
                            },
                        );
                }
            }
        }
        Self {
            indexes,
            cfg,
            dom,
            operands,
            inputs: body.inputs.clone(),
            intervals: HashMap::new(),
            calc: LiveIntervalCalc::new(),
        }
    }

    /// The values that have an operand.
    pub fn values(&self) -> impl Iterator<Item = u32> + '_ {
        self.operands.keys().copied()
    }

    /// `computeVirtRegInterval` (LiveIntervals.cpp#L190): a new interval for
    /// `reg` (weight 0) from its operands.
    pub fn compute_virt_reg_interval(
        &mut self,
        reg: u32,
    ) -> LiveInterval {
        let mut li = LiveInterval::new(reg, 0.0);
        self.calc.calc.reset(self.cfg.len());
        let ctx = Ctx { cfg: &self.cfg, indexes: &self.indexes, dom: &self.dom };
        // A parameter is defined at the entry of the function, as a PHI-def is
        // at its block's start (DEVIATE: LLVM has a COPY from a
        // physical register).
        if self.inputs.contains(&reg) {
            li.create_dead_def(self.indexes.get_mbb_start_idx(self.cfg.entry));
        }
        let empty = Vec::new();
        let operands = self.operands.get(&reg).unwrap_or(&empty);
        self.calc.calculate(&mut li, operands, &ctx);
        li
    }

    /// `getInterval`/`createAndComputeVirtRegInterval`: the interval of `reg`,
    /// computed the first time.
    pub fn interval(
        &mut self,
        reg: u32,
    ) -> &LiveInterval {
        if !self.intervals.contains_key(&reg) {
            let li = self.compute_virt_reg_interval(reg);
            self.intervals.insert(reg, li);
        }
        &self.intervals[&reg]
    }

    /// `hasInterval`.
    pub fn has_interval(
        &self,
        reg: u32,
    ) -> bool {
        self.intervals.contains_key(&reg)
    }
}
