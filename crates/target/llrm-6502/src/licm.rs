//! Early `MachineLICM` for the 6502: a constant or a symbol's address a loop
//! makes on every trip is made once before it.
//!
//! # How it works
//! The walk is the shared [`licm_core`] one, over the allocator's
//! [`MFunction`] (as arm64's). The policy is the pin's `IsProfitableToHoist`
//! for a rematerializable instruction (`LDI`, `LDS`): it moves while the loop
//! keeps a register for it. A hoisted constant a cell holds costs a load of the
//! cell where the constant was an immediate, so the loop must have `a`, `x` or
//! `y` to spare: the values it holds at once, with the constants already moved,
//! are fewer than the three. A loop with a call keeps none (the call clobbers
//! all three).
//!
//! A frame address (`LEA`: `rs0` plus a distance, the pointer of an object past
//! the 255 `(zp),Y` reaches) is hoisted too, as the pin's MachineLICM hoists
//! the `LEA` of a stack slot. It clobbers only `a` and the pair it makes lives
//! in zero page, so it is no pressure on `a`, `x` and `y`: the pin's
//! `CanCauseHighRegPressure` is asked of the zero-page pairs, `pressure +
//! weight >= limit` with the loop's peak as the pressure and the pairs the
//! smallest zero page leaves ([`PAIR_LIMIT`]) as the limit.
//!
//! # Example
//! ```text
//!   pre:  jmp loop                        pre:  v4 := ldi 255 ; jmp loop
//!   loop: v4 := ldi 255             =>    loop: sta junk,x
//!         sta junk,x
//! ```
//!
//! # See also
//! `llrm_codegen::backend::licm_core` (the walk); `llrm-arm64`'s `licm` (the
//! other policy over an `MFunction`).

use std::collections::BTreeSet;

use llrm_codegen::backend::licm_core::{self, Block, Facts, Inst as CoreInst, Policy};
use llrm_spike_mf::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use llrm_support::hash::HashMap;

use crate::alloc::Class;

/// `a`, `x`, `y`: where a loop can hold a value for an immediate-speed read.
const REGISTERS: usize = 3;

/// The zero-page pairs of the smallest zero page, less `rs0`'s: the limit of
/// the pin's `RegLimit` for the class a hoisted pointer lives in.
const PAIR_LIMIT: usize = crate::alloc::BASE_CELLS as usize / 2 - 1;

/// What a loop's values (words in pairs, bytes in `a`, `x`, `y`) are counted
/// against: one slot each.
const SLOTS: usize = PAIR_LIMIT + REGISTERS;

#[derive(Clone)]
struct Wrapped {
    inst: Inst,
    defs: Vec<u32>,
    uses: Vec<u32>,
    /// It names a physical register: the allocator's input, not a value.
    physical: bool,
    /// The only physical register it names is a defined `a`.
    clobbers_only_a: bool,
    /// It reads `a` by name (a return, a call's argument), not as a value.
    reads_a: bool,
    /// It defines `a` by name.
    writes_a: bool,
    /// Its immediate is zero: a frame address at distance 0 is `rs0` itself.
    zero_distance: bool,
}

impl Wrapped {
    fn new(inst: Inst) -> Wrapped {
        let (mut defs, mut uses, mut physical, mut clobbers_only_a, mut reads_a, mut writes_a, mut zero_distance) =
            (Vec::new(), Vec::new(), false, true, false, false, false);
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(n)) => defs.push(*n),
                Operand::Use(Reg::Virt(n)) => uses.push(*n),
                Operand::Def(Reg::Phys(r)) => {
                    physical = true;
                    clobbers_only_a &= *r == crate::reg::A;
                    writes_a |= *r == crate::reg::A;
                }
                Operand::Use(Reg::Phys(r)) => {
                    physical = true;
                    clobbers_only_a = false;
                    reads_a |= *r == crate::reg::A;
                }
                Operand::Imm(0) => zero_distance = true,
                _ => {}
            }
        }
        Wrapped { inst, defs, uses, physical, clobbers_only_a, reads_a, writes_a, zero_distance }
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

struct Mos {
    /// The values live out of each block.
    live: HashMap<i64, BTreeSet<u32>>,
    /// The value a register copy stands for: the coalescer joins the copy with
    /// its source, so the two hold one register.
    alias: HashMap<u32, u32>,
    /// The most values live at once in the loop, and the constants this walk
    /// has moved out of it.
    peak: usize,
    hoisted: usize,
    pairs: usize,
    /// The loop reads `a` by name: what the preheader leaves in it may be
    /// wanted, and the hoisted `LEA` would end the preheader with `a`
    /// clobbered.
    reads_a: bool,
    /// The loop moves `rs0`: no address made from it is the same on every trip.
    moves_sp: bool,
    /// The loop calls: a call clobbers `a`, `x` and `y`.
    calls: bool,
}

impl Mos {
    fn root(
        &self,
        mut value: u32,
    ) -> u32 {
        // copies can form a cycle (a loop-carried swap): the walk ends where it
        // would revisit, on the least value of the walk, the same for every
        // start
        let mut seen = vec![value];
        while let Some(&next) = self.alias.get(&value) {
            if seen.contains(&next) {
                return *seen.iter().min().unwrap();
            }
            seen.push(next);
            value = next;
        }
        value
    }

    /// The registers `live` holds at once, copies counted with their source.
    fn held(
        &self,
        live: &BTreeSet<u32>,
    ) -> usize {
        live.iter().map(|&value| self.root(value)).collect::<BTreeSet<u32>>().len()
    }
}

impl Policy<Wrapped> for Mos {
    fn begin(
        &mut self,
        facts: &Facts<'_, Wrapped>,
    ) {
        self.calls = facts.inside_blocks().any(|block| block.insns.iter().any(|one| one.inst.call));
        self.moves_sp = facts
            .inside_blocks()
            .any(|block| block.insns.iter().any(|one| crate::alloc::moves_stack_pointer(&one.inst)));
        // `a` read before any write of it in its block comes from before the
        // block: from the preheader or an earlier trip, which the hoisted
        // `LEA` would overwrite.
        self.reads_a = facts
            .inside_blocks()
            .any(
                |block| {
                    let mut written = false;
                    block
                        .insns
                        .iter()
                        .any(
                            |one| {
                                let exposed = one.reads_a && !written;
                                written |= one.writes_a;
                                exposed
                            },
                        )
                },
            );
        self.hoisted = 0;
        self.pairs = 0;
        self.peak = 0;
        for block in facts.inside_blocks() {
            let mut now: BTreeSet<u32> = self.live.get(&block.at).cloned().unwrap_or_default();
            self.peak = self.peak.max(self.held(&now));
            for one in block.insns.iter().rev() {
                for value in &one.defs {
                    now.remove(value);
                }
                now.extend(one.uses.iter().copied());
                self.peak = self.peak.max(self.held(&now));
            }
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
        let constant = inst.rematerializable
            && one.defs.len() == 1
            && one.uses.is_empty()
            && !one.physical
            && facts.defs.get(&one.defs[0]) == Some(&1)
            && !facts.copied.contains(&one.defs[0])
            && inst.ties.is_empty()
            && !(inst.call || inst.terminator || inst.barrier || inst.may_store || inst.side_effects);
        // `rs0` plus a distance (not zero: that is a copy of `rs0`, free where
        // it is, a pair held for the loop once moved): nothing is read
        // but `rs0`, which the loop does not move (`moves_sp`), and `a`
        // is the only register it clobbers
        let frame_address = matches!(inst.kind, Kind::Op("LEA"))
            && one.defs.len() == 2
            && one.uses.is_empty()
            && one.physical
            && one.clobbers_only_a
            && !one.zero_distance
            && one.defs.iter().all(|value| facts.defs.get(value) == Some(&1) && !facts.copied.contains(value))
            && inst.ties.is_empty()
            && !(inst.call || inst.terminator || inst.barrier || inst.may_store || inst.side_effects);
        if frame_address {
            // `CanCauseHighRegPressure`: the peak and the pairs moved, the one
            // more
            if self.calls
                || self.moves_sp
                || self.reads_a
                || !(facts.every_trip)(at)
                || self.peak + self.pairs + 1 >= SLOTS
            {
                return false;
            }
            self.pairs += 1;
            return true;
        }
        // a constant a phi takes would need a copy; one in a block that does
        // not run on every trip is not made speculatively
        if !constant || self.calls || !(facts.every_trip)(at) {
            return false;
        }
        if self.peak + self.hoisted + 1 > REGISTERS {
            return false;
        }
        self.hoisted += 1;
        true
    }
}

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

/// Each virtual register a register copy defines, by the one it copies.
fn copies(blocks: &[Block<Wrapped>]) -> HashMap<u32, u32> {
    blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter(|one| one.inst.kind == Kind::Copy && !one.physical)
        .filter_map(|one| Some((*one.defs.first()?, *one.uses.first()?)))
        .collect()
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

/// Moves what the loops of `function` make on every trip to their
/// preheaders; whether anything moved.
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
        &mut Mos {
            live: live_out(&blocks),
            alias: copies(&blocks),
            peak: 0,
            hoisted: 0,
            pairs: 0,
            reads_a: false,
            moves_sp: false,
            calls: false,
        },
    ) else {
        return false;
    };
    for (block, new) in function.blocks.iter_mut().zip(moved) {
        block.insts = new.insns.into_iter().map(|one| one.inst).collect();
    }
    true
}
