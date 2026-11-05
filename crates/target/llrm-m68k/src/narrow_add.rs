//! Counts the `add.l #imm, %dN` rows that a word add would do: the sites of
//! the byte-width add gcc emits (`addi.b #48, %d1` for 6 bytes and 16 clocks of
//! `add.l`). Counting only: nothing is rewritten.
//!
//! # How it works
//! [`sites`] walks each block with the upper bound of every data register's
//! unsigned value: `moveq`, `and.l`, `lsr.l`, a zero extension and a copy give
//! one, anything else that writes the register forgets it, a call forgets all.
//! A block with one predecessor starts from that predecessor's end (to
//! [`CHAIN_DEPTH`] blocks back); any other starts with nothing known. The row
//! is a site when `bound + immediate` fits the word: the upper half is zero
//! before and after, so `addi.w` leaves the register as `add.l` does. The word
//! form sets the codes from bit 15: when the sum stays below 32768 they are the
//! long form's (N and V clear, C and X clear, Z the same), otherwise the site
//! needs no reader of the codes after it ([`flags_read_after`], inside the
//! block, as `narrow.rs` asks).
//!
//! DEVIATE (when it rewrites): the pin keeps the 32-bit add; gcc's `addhi3`
//! narrowing prints the byte form. Reason: best of gcc and clang is the goal.

use std::collections::HashMap;

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::REGISTER_INFO;
use crate::cmpelim::flags_read_after;
use crate::mfunc::{Class, MFunction};
use crate::regalloc::Assignment;

/// Predecessor blocks the walk looks back through.
pub const CHAIN_DEPTH: usize = 4;

/// Upper bound of the unsigned value of each data register, by root.
type State = HashMap<RegId, u32>;

/// A row that could be a word add.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Site {
    pub block: usize,
    pub at: usize,
    pub bound: u32,
    pub immediate: u32,
    /// The word form sets the same codes (the sum is below 32768).
    pub codes_equal: bool,
    /// What later rows of the block read of the codes (`None`: not known).
    pub codes_read: Option<u32>,
}

impl Site {
    /// The rewrite would be sound: the codes are the same, or nothing reads them.
    pub fn sound(&self) -> bool {
        self.codes_equal || self.codes_read == Some(0)
    }
}

fn root(
    assignment: &Assignment,
    reg: &Reg,
) -> RegId {
    match reg {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[*v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(*p),
    }
}

fn bound_of(
    state: &State,
    assignment: &Assignment,
    reg: &Reg,
) -> u32 {
    state.get(&root(assignment, reg)).copied().unwrap_or(u32::MAX)
}

/// A 32-bit data register: a copy of or into a byte or word view leaves the
/// rest of the register as it was.
fn long(
    classes: &[Class],
    reg: &Reg,
) -> bool {
    matches!(reg, Reg::Virt(v) if classes[*v as usize] == Class::Dr32)
}

/// The bound after `inst`, updating `state`.
fn step(
    state: &mut State,
    assignment: &Assignment,
    classes: &[Class],
    inst: &Inst,
) {
    if inst.call {
        state.clear();
        return;
    }
    let new = match (inst.kind, &inst.operands[..]) {
        (Kind::Op("MOVQ"), [Operand::Def(_), Operand::Imm(k)]) if *k >= 0 => Some(*k as u32),
        (Kind::Op("MOV32ri"), [Operand::Def(_), Operand::Imm(k)]) if (0..=i64::from(u32::MAX)).contains(k) => {
            Some(*k as u32)
        }
        (Kind::Op("AND32di"), [Operand::Def(_), Operand::Use(s), Operand::Imm(k)]) => {
            Some(bound_of(state, assignment, s).min(*k as u32))
        }
        (Kind::Op("AND32dd"), [Operand::Def(_), Operand::Use(a), Operand::Use(b)]) => {
            Some(bound_of(state, assignment, a).min(bound_of(state, assignment, b)))
        }
        (Kind::Op("LSR32di"), [Operand::Def(_), Operand::Use(s), Operand::Imm(k)]) if (0..32).contains(k) => {
            Some(bound_of(state, assignment, s) >> *k)
        }
        (Kind::Op("MOVZXd32d8"), [Operand::Def(_), Operand::Use(_)]) => Some(0xFF),
        (Kind::Op("MOVZXd32d16"), [Operand::Def(_), Operand::Use(_)]) => Some(0xFFFF),
        (Kind::Copy, [Operand::Def(d), Operand::Use(s)]) if long(classes, d) && long(classes, s) => {
            Some(bound_of(state, assignment, s))
        }
        (Kind::Op("ADD32di"), [Operand::Def(_), Operand::Use(s), Operand::Imm(k)]) if *k > 0 => {
            bound_of(state, assignment, s).checked_add(*k as u32)
        }
        _ => None,
    };
    for operand in &inst.operands {
        if let Operand::Def(reg) = operand {
            match new {
                Some(bound) => state.insert(root(assignment, reg), bound),
                None => state.remove(&root(assignment, reg)),
            };
        }
    }
}

fn end_state(
    function: &MFunction,
    assignment: &Assignment,
    predecessors: &[Vec<usize>],
    block: usize,
    depth: usize,
) -> State {
    let mut state = start_state(function, assignment, predecessors, block, depth);
    for inst in &function.blocks[block].insts {
        step(&mut state, assignment, &function.classes, inst);
    }
    state
}

fn start_state(
    function: &MFunction,
    assignment: &Assignment,
    predecessors: &[Vec<usize>],
    block: usize,
    depth: usize,
) -> State {
    match predecessors[block][..] {
        [only] if depth > 0 && only != block => end_state(function, assignment, predecessors, only, depth - 1),
        _ => State::new(),
    }
}

/// The `add.l #imm` rows (an immediate above `addq`'s) that a word add would do.
pub fn sites(
    function: &MFunction,
    assignment: &Assignment,
) -> Vec<Site> {
    let following = successors(function);
    let mut predecessors = vec![Vec::new(); function.blocks.len()];
    for (from, targets) in following.iter().enumerate() {
        for &to in targets {
            if !predecessors[to].contains(&from) {
                predecessors[to].push(from);
            }
        }
    }
    let mut out = Vec::new();
    for (block, body) in function.blocks.iter().enumerate() {
        let mut state = start_state(function, assignment, &predecessors, block, CHAIN_DEPTH);
        for (at, inst) in body.insts.iter().enumerate() {
            if let (Kind::Op("ADD32di"), [Operand::Def(def), Operand::Use(base), Operand::Imm(k)]) =
                (inst.kind, &inst.operands[..])
                && (9..=0xFFFF).contains(k)
                && root(assignment, def) == root(assignment, base)
            {
                let bound = bound_of(&state, assignment, base);
                let sum = u64::from(bound) + *k as u64;
                if sum <= 0xFFFF {
                    out.push(Site {
                        block,
                        at,
                        bound,
                        immediate: *k as u32,
                        codes_equal: sum < 0x8000,
                        codes_read: flags_read_after(&body.insts, at),
                    });
                }
            }
            step(&mut state, assignment, &function.classes, inst);
        }
    }
    out
}
