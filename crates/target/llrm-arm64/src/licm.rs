//! Early `MachineLICM` for arm64: what a loop recomputes from values it does
//! not change (a global's page, a constant) is made once before it.
//!
//! # How it works
//! The walk is the shared [`licm_core`] one, over the allocator's
//! [`MFunction`]. The policy is the pin's `IsProfitableToHoist` for a
//! rematerializable instruction (`MOVaddr`, `MOVi32imm`): it always moves, the
//! allocator makes it again where a register is short. A copy between register
//! classes (the `fmov s, w` of a float constant, the pin's cross-class `COPY`)
//! moves with the constant it reads (`IsProfitableToHoist` L1273 reaching
//! `CanCauseHighRegPressure` L1235), unless the values live at once in the loop
//! and the copies already moved fill the class (DEVIATE ML3, exit 2026-11-20:
//! the peak over the loop's blocks, not the pin's per-set back trace; a loop
//! with a call counts the values it names against `d8`-`d15`, the registers a
//! call keeps; a block that does not run on every trip keeps its copies,
//! `AvoidSpeculation` L1349 without the peak). Nothing that reads memory or the
//! flags, has an effect, or touches a physical register moves.
//!
//! # Example
//! ```text
//!   pre:  b loop                          pre:  adrp x9, _g@PAGE ; b loop
//!   loop: adrp x9, _g@PAGE          =>    loop: ldr w0, [x9, _g@PAGEOFF]
//!         ldr w0, [x9, _g@PAGEOFF]
//! ```
//!
//! # See also
//! `llrm_codegen::backend::licm_core` (the walk), `lir_view` (runs it before
//! allocation).

use std::collections::{BTreeSet, HashMap};

use llrm_codegen::backend::licm_core::{self, Block, Facts, Inst as CoreInst, Policy};
use llrm_spike_mf::mfunc::{Inst, Kind, MFunction, Operand, Reg};

use crate::lir_view::{Class, FMOV_TO_FPR};

/// Registers a loop can hold values of a class in: `x0`-`x28`, `v0`-`v31`.
const GPR_LIMIT: usize = 29;
const FPR_LIMIT: usize = 32;
/// `d8`-`d15`, the float registers a call keeps.
const FPR_CALLEE_SAVED: usize = 8;

#[derive(Clone)]
struct Wrapped {
    inst: Inst,
    defs: Vec<u32>,
    uses: Vec<u32>,
    /// It names a physical register: the allocator's input, not a value.
    physical: bool,
}

impl Wrapped {
    fn new(inst: Inst) -> Wrapped {
        let (mut defs, mut uses, mut physical) = (Vec::new(), Vec::new(), false);
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(n)) => defs.push(*n),
                Operand::Use(Reg::Virt(n)) => uses.push(*n),
                Operand::Def(Reg::Phys(_)) | Operand::Use(Reg::Phys(_)) => physical = true,
                _ => {}
            }
        }
        Wrapped { inst, defs, uses, physical }
    }
}

impl CoreInst for Wrapped {
    fn defines(&self) -> &[u32] {
        &self.defs
    }

    fn uses(&self) -> &[u32] {
        &self.uses
    }

    fn is_copy(&self) -> bool {
        self.inst.kind == Kind::Copy
    }

    fn is_move(&self) -> bool {
        self.inst.kind == Kind::Copy || self.inst.cheap_as_move
    }

    fn is_meta(&self) -> bool {
        false
    }

    fn is_jump(&self) -> bool {
        self.inst.terminator
    }

    fn is_plain_load(&self) -> bool {
        self.inst.may_load && !self.inst.invariant_load
    }

    fn identical(
        &self,
        other: &Wrapped,
    ) -> bool {
        let blank = |inst: &Inst| -> Vec<Operand> {
            inst.operands
                .iter()
                .map(|operand| match operand {
                    Operand::Def(_) => Operand::Imm(0),
                    other => other.clone(),
                })
                .collect()
        };
        self.inst.kind == other.inst.kind && blank(&self.inst) == blank(&other.inst)
    }

    fn rename_use(
        &mut self,
        from: u32,
        to: u32,
    ) {
        for operand in self.inst.operands.iter_mut() {
            if *operand == Operand::Use(Reg::Virt(from)) {
                *operand = Operand::Use(Reg::Virt(to));
            }
        }
        self.uses.iter_mut().filter(|value| **value == from).for_each(|value| *value = to);
    }
}

struct Arm64<'a> {
    classes: &'a [Class],
    /// Values of each class (general, floating) the loop names, and the copies
    /// this walk has moved out of it.
    named: [usize; 2],
    /// The loop calls: a float it names is kept in `d8`-`d15` or not at all.
    calls: bool,
    /// The values live out of each block.
    live: HashMap<i64, BTreeSet<u32>>,
    /// The most values of each class live at once in the loop.
    peak: [usize; 2],
    /// The copies this loop has moved out, by class.
    hoisted: [usize; 2],
}

impl Arm64<'_> {
    /// `live` held at one point of the loop.
    fn note(
        &mut self,
        live: &BTreeSet<u32>,
    ) {
        let mut counts = [0, 0];
        for &value in live {
            counts[self.slot(value)] += 1;
        }
        for slot in 0..2 {
            self.peak[slot] = self.peak[slot].max(counts[slot]);
        }
    }

    fn slot(
        &self,
        value: u32,
    ) -> usize {
        usize::from(self.classes.get(value as usize) != Some(&Class::Gpr))
    }
}

impl Policy<Wrapped> for Arm64<'_> {
    fn begin(
        &mut self,
        facts: &Facts<'_, Wrapped>,
    ) {
        self.named = [0, 0];
        self.calls = facts.inside_blocks().any(|block| block.insns.iter().any(|one| one.inst.call));
        self.peak = [0, 0];
        self.hoisted = [0, 0];
        for block in facts.inside_blocks() {
            let mut now: BTreeSet<u32> = self.live.get(&block.at).cloned().unwrap_or_default();
            self.note(&now);
            for one in block.insns.iter().rev() {
                for value in &one.defs {
                    now.remove(value);
                }
                now.extend(one.uses.iter().copied());
                self.note(&now);
            }
        }
        for &value in facts.named {
            let slot = self.slot(value);
            self.named[slot] += 1;
        }
    }

    fn movable(
        &mut self,
        facts: &Facts<'_, Wrapped>,
        at: i64,
        insns: &[Wrapped],
        index: usize,
    ) -> bool {
        let one = &insns[index];
        let inst = &one.inst;
        // A copy between classes (the pin's `COPY` that becomes `fmov`); one
        // within a class is the coalescer's, and hoisting it moved a loop's
        // constant out of reach of the `mov` that made it (algos -O1, +8 B).
        let across = |one: &Wrapped| one.uses.first().is_some_and(|&from| self.slot(from) != self.slot(one.defs[0]));
        if (inst.kind == Kind::Op(FMOV_TO_FPR) || (inst.kind == Kind::Copy && !one.defs.is_empty() && across(one)))
            && !inst.rematerializable
            && one.defs.len() == 1
            && one.uses.len() == 1
            && !one.physical
            && facts.defs.get(&one.defs[0]) == Some(&1)
            && !facts.copied.contains(&one.defs[0])
            && (facts.every_trip)(at)
        {
            let slot = self.slot(one.defs[0]);
            // A float held across a call lives in `d8`-`d15` or nowhere.
            let limit = if slot == 1 && self.calls { FPR_CALLEE_SAVED } else { [GPR_LIMIT, FPR_LIMIT][slot] };
            // With a call the held float needs `d8`-`d15`: the values the loop
            // names compete for them. Without one, what is live at once does.
            let held = if slot == 1 && self.calls { self.named[slot] } else { self.peak[slot] + self.hoisted[slot] };
            if held + 1 >= limit {
                return false;
            }
            self.named[slot] += 1;
            self.hoisted[slot] += 1;
            return true;
        }
        (inst.rematerializable || (inst.may_load && inst.invariant_load))
            // `CheapInstr && CreatesCopy`: a constant a phi takes would need a
            // copy.
            && !(inst.cheap_as_move && one.defs.iter().any(|value| facts.copied.contains(value)))
            && !one.physical
            && !one.defs.is_empty()
            && one.defs.iter().all(|value| facts.defs.get(value) == Some(&1))
            && inst.ties.is_empty()
            && !(inst.call
                || inst.terminator
                || inst.barrier
                || (inst.may_load && !inst.invariant_load)
                || inst.may_store
                || inst.side_effects
                || inst.sets_flags
                || inst.reads_flags
                || inst.early_clobber)
    }
}

/// The blocks a block can go to: its branch targets and, unless it ends in a
/// jump nothing falls out of, the next block.
fn successors<C>(
    function: &MFunction<C>,
    at: usize,
) -> Vec<i64> {
    let block = &function.blocks[at];
    let mut succ: Vec<i64> = block
        .insts
        .iter()
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| match operand {
            Operand::Block(to) => Some(*to as i64),
            _ => None,
        })
        .collect();
    let falls = !block.insts.last().is_some_and(|last| last.barrier);
    if falls && let Some(next) = function.blocks.get(at + 1) {
        succ.push(next.id as i64);
    }
    succ.sort_unstable();
    succ.dedup();
    succ
}

/// The values live out of each block, by the usual backward dataflow.
fn live_out(blocks: &[Block<Wrapped>]) -> HashMap<i64, BTreeSet<u32>> {
    let place: HashMap<i64, usize> = blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
    let mut live_in: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); blocks.len()];
    let mut out: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); blocks.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for number in (0..blocks.len()).rev() {
            let mut now: BTreeSet<u32> = BTreeSet::new();
            for to in &blocks[number].succ {
                if let Some(&next) = place.get(to) {
                    now.extend(live_in[next].iter().copied());
                }
            }
            let mut before = now.clone();
            for one in blocks[number].insns.iter().rev() {
                for value in &one.defs {
                    before.remove(value);
                }
                before.extend(one.uses.iter().copied());
            }
            if before != live_in[number] {
                live_in[number] = before;
                changed = true;
            }
            out[number] = now;
        }
    }
    blocks.iter().zip(out).map(|(block, live)| (block.at, live)).collect()
}

/// Moves what the loops of `function` recompute to their preheaders; whether
/// anything moved.
pub fn hoist(function: &mut MFunction<Class>) -> bool {
    let Some(first) = function.blocks.first() else { return false };
    let blocks: Vec<Block<Wrapped>> = (0..function.blocks.len())
        .map(|at| Block {
            at: function.blocks[at].id as i64,
            succ: successors(function, at),
            phis: Vec::new(),
            insns: function.blocks[at].insts.iter().cloned().map(Wrapped::new).collect(),
        })
        .collect();
    let Some(moved) = licm_core::hoist(
        &blocks,
        first.id as i64,
        &mut Arm64 {
            classes: &function.classes,
            named: [0, 0],
            calls: false,
            live: live_out(&blocks),
            peak: [0, 0],
            hoisted: [0, 0],
        },
    ) else {
        return false;
    };
    for (block, new) in function.blocks.iter_mut().zip(moved) {
        block.insts = new.insns.into_iter().map(|one| one.inst).collect();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(
        dst: u32,
        name: &str,
    ) -> Inst {
        let mut inst = Inst::new(
            Kind::Op("Adr"),
            vec![Operand::Def(Reg::Virt(dst)), Operand::Symbol(name.into(), 0), Operand::Imm(7)],
        );
        inst.rematerializable = true;
        inst
    }

    fn jump(to: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("Branch"), vec![Operand::Block(to)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }

    fn block(
        id: u32,
        insts: Vec<Inst>,
    ) -> llrm_spike_mf::mfunc::Block {
        llrm_spike_mf::mfunc::Block { id, insts, phis: Vec::new() }
    }

    /// Two arms of a loop each made the page of the same global (2 `adrp`
    /// where the pin makes 1, `EliminateCSE`): the page is made once in the
    /// preheader and the arms read it.
    #[test]
    fn equal_pages_made_in_a_loops_arms_are_made_once_in_the_preheader() {
        let mut cond = Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(3)]);
        cond.terminator = true;
        let user = |reg: u32| Inst::new(Kind::Op("Store"), vec![Operand::Use(Reg::Virt(reg))]);
        let mut function = MFunction::<Class>::new("f");
        function.blocks = vec![
            // preheader, falls into 1
            block(0, vec![]),
            // header: to 3 or falls to 2
            block(1, vec![cond]),
            // arm
            block(2, vec![page(10, "g"), user(10), jump(4)]),
            // arm, falls to 4
            block(3, vec![page(11, "g"), user(11)]),
            // latch
            block(4, vec![Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(1)])]),
            // exit
            block(5, vec![]),
        ];
        assert!(hoist(&mut function));
        let pages = |at: usize| function.blocks[at].insts.iter().filter(|inst| inst.rematerializable).count();
        assert_eq!((pages(0), pages(2), pages(3)), (1, 0, 0), "{function:?}");
        let made = function.blocks[0].insts[0].operands[0].clone();
        let Operand::Def(made) = made else { panic!("not a def") };
        for arm in [2, 3] {
            assert!(function.blocks[arm].insts.iter().any(|inst| inst.operands == vec![Operand::Use(made)]), "{arm}");
        }
    }

    /// A load of the constant pool in a loop stayed in it (`ldr d17, [x10,
    /// lCPI@PAGEOFF]` every trip) where the pin loads it once before the loop
    /// (`isDereferenceableInvariantLoad`): a load of memory nothing writes
    /// moves with its page; one of other memory does not.
    #[test]
    fn a_load_of_the_constant_pool_moves_out_of_the_loop_and_other_loads_stay() {
        let load = |dst: u32, invariant: bool| {
            let mut inst = Inst::new(
                Kind::Op("Load"),
                vec![Operand::Def(Reg::Virt(dst)), Operand::Use(Reg::Virt(10)), Operand::Imm(3)],
            );
            inst.may_load = true;
            inst.invariant_load = invariant;
            inst
        };
        let mut back = Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(1)]);
        back.terminator = true;
        let mut function = MFunction::<Class>::new("f");
        function.blocks = vec![
            block(0, vec![page(10, "lCPI")]),
            block(1, vec![load(11, true), load(12, false), back]),
            block(2, vec![]),
        ];
        assert!(hoist(&mut function));
        let loads = |at: usize| function.blocks[at].insts.iter().filter(|inst| inst.may_load).count();
        assert_eq!((loads(0), loads(1)), (1, 1), "{function:?}");
    }

    fn constant(dst: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("MOVi32imm"), vec![Operand::Def(Reg::Virt(dst)), Operand::Imm(0x3fb3_74bc)]);
        inst.rematerializable = true;
        inst.cheap_as_move = true;
        inst
    }

    /// The `fmov s, w` of a float constant, as the view names it.
    fn copy(
        dst: u32,
        src: u32,
    ) -> Inst {
        Inst::new(Kind::Op(FMOV_TO_FPR), vec![Operand::Def(Reg::Virt(dst)), Operand::Use(Reg::Virt(src))])
    }

    /// A float constant made in the loop (`mov w, #imm` then `fmov s, w`) had
    /// only the `mov` hoisted: the held GPR constant was then split and made
    /// again on each latch (jpeg_single -O1: 3 `mov`/`movk` pairs where the pin
    /// makes 1, `IsProfitableToHoist` hoisting the `COPY` too).
    #[test]
    fn the_copy_of_a_hoisted_constant_into_a_float_register_is_hoisted_with_it() {
        let mut back = Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(1)]);
        back.terminator = true;
        let user = Inst::new(Kind::Op("Store"), vec![Operand::Use(Reg::Virt(1))]);
        let mut function = MFunction::<Class>::new("f");
        function.classes = vec![Class::Gpr, Class::Fpr];
        function.blocks = vec![block(0, vec![]), block(1, vec![constant(0), copy(1, 0), user, back]), block(2, vec![])];
        assert!(hoist(&mut function));
        let copies =
            |at: usize| function.blocks[at].insts.iter().filter(|inst| inst.kind == Kind::Op(FMOV_TO_FPR)).count();
        assert_eq!((copies(0), copies(1)), (1, 0), "{function:?}");
    }

    /// A loop that already names every float register keeps the copy: the
    /// held constant would only be spilled (`CanCauseHighRegPressure`).
    #[test]
    fn the_copy_stays_in_a_loop_that_names_every_float_register() {
        let mut back = Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(1)]);
        back.terminator = true;
        let mut function = MFunction::<Class>::new("f");
        let held = FPR_LIMIT as u32;
        function.classes = vec![Class::Fpr; held as usize + 2];
        function.classes[0] = Class::Gpr;
        let target = held + 1;
        let mut body = vec![constant(0), copy(target, 0)];
        // The loop reads `held` float values made before it.
        for value in 1..=held {
            body.push(Inst::new(Kind::Op("Store"), vec![Operand::Use(Reg::Virt(value))]));
        }
        body.push(back);
        function.blocks = vec![block(0, vec![]), block(1, body), block(2, vec![])];
        hoist(&mut function);
        let copies =
            |at: usize| function.blocks[at].insts.iter().filter(|inst| inst.kind == Kind::Op(FMOV_TO_FPR)).count();
        assert_eq!((copies(0), copies(1)), (0, 1), "{function:?}");
    }
}
