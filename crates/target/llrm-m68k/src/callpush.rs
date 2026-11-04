//! Outgoing arguments as pushes: `move.l %d0, -(%sp)` for a store into the
//! frame's call area, and the stack pointer restored after the call.
//!
//! # How it works
//! A call sequence is `ADJCALLSTACKDOWN n`, stores of the arguments to
//! `(k, %sp)` for `k` from `n - 4` down to 0, the call and `ADJCALLSTACKUP`.
//! [`plan`] finds the sequences whose stores fill the area in that order with
//! nothing else naming the stack pointer in between. Pushing is priced against
//! the stores (`encode` and `timing::cost` on the cpu, the cleanup `adda`
//! included) three ways: no sequence, every sequence, and the sequences that
//! are cheaper alone. The frame is priced with each choice's largest call area
//! left: when no store remains the prologue and epilogue lose their `suba` and
//! `adda`, which is where a pair of two-word calls gains.
//!
//! DEVIATE: the pin keeps the reserved call frame and stores
//! (`M68kFrameLowering` has no `X86CallFrameOptimization`); gcc pushes. Reason:
//! best of gcc and clang is the goal. Exit: none.
//!
//! # See also
//! `X86CallFrameOptimization::runOnMachineFunction` (`isProfitable`).

use std::collections::{HashMap, HashSet};

use crate::dagisel::hooks::Cpu;
use crate::mfunc::{Inst, Kind, Operand, Reg};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, mc, reg};

/// Which instructions of a function are rewritten.
#[derive(Default)]
pub struct Plan {
    /// Argument stores that become pushes.
    pushes: HashSet<usize>,
    /// `ADJCALLSTACKDOWN`s that vanish.
    downs: HashSet<usize>,
    /// `ADJCALLSTACKUP`s that become `adda.l #bytes, %sp`.
    ups: HashMap<usize, i64>,
}

impl Plan {
    pub fn is_push(
        &self,
        at: usize,
    ) -> bool {
        self.pushes.contains(&at)
    }

    pub fn is_down(
        &self,
        at: usize,
    ) -> bool {
        self.downs.contains(&at)
    }

    pub fn cleanup(
        &self,
        at: usize,
    ) -> Option<i64> {
        self.ups.get(&at).copied()
    }

    /// Whether the call sequence starting at `down` stays in the frame.
    pub fn keeps_frame(
        &self,
        down: usize,
    ) -> bool {
        !self.downs.contains(&down)
    }
}

/// One convertible sequence: where it is, how big, and the two prices.
struct Sequence {
    down: usize,
    up: usize,
    bytes: i64,
    stores: Vec<usize>,
    kept: (usize, u32),
    pushed: (usize, u32),
}

fn physical(
    register: Reg,
    assignment: &Assignment,
) -> llrm_lir::registers::RegId {
    match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    }
}

fn names_sp(
    inst: &Inst,
    assignment: &Assignment,
) -> bool {
    inst.operands
        .iter()
        .any(
            |operand| match operand {
                Operand::Def(r) | Operand::Use(r) => REGISTER_INFO.root(physical(*r, assignment)) == reg::SP,
                _ => false,
            },
        )
}

/// What a call-area store writes.
#[derive(Clone, Copy)]
enum Source {
    Register(llrm_lir::registers::RegId),
    Immediate(i64),
}

/// The call-area store `inst` is: its offset and what it stores.
fn argument_store(
    inst: &Inst,
    assignment: &Assignment,
) -> Option<(i64, Source)> {
    let (offset, base, source) = match (inst.kind, &inst.operands[..]) {
        (Kind::Op("MOV32pr"), [Operand::Imm(k), Operand::Use(base), Operand::Use(src)]) => {
            (*k, base, Source::Register(physical(*src, assignment)))
        }
        (Kind::Op("MOV32jr"), [Operand::Use(base), Operand::Use(src)]) => {
            (0, base, Source::Register(physical(*src, assignment)))
        }
        (Kind::Op("MOV32pi"), [Operand::Imm(k), Operand::Use(base), Operand::Imm(v)]) => {
            (*k, base, Source::Immediate(*v))
        }
        (Kind::Op("MOV32ji"), [Operand::Use(base), Operand::Imm(v)]) => (0, base, Source::Immediate(*v)),
        _ => return None,
    };
    (REGISTER_INFO.root(physical(*base, assignment)) == reg::SP).then_some((offset, source))
}

/// `move.l value, -(%sp)`: `value` is a register or an immediate.
pub(crate) fn push_of(value: &mc::Operand) -> mc::Inst {
    let opcode = match value {
        mc::Operand::Reg(src) if crate::expand::is_address(*src) => "MOV32ea",
        mc::Operand::Reg(_) => "MOV32ed",
        _ => "MOV32ei",
    };
    mc::Inst { opcode, operands: vec![mc::Operand::Reg(reg::SP), value.clone()] }
}

fn cost(
    inst: &mc::Inst,
    cpu: Cpu,
) -> Result<(usize, u32), String> {
    let size = crate::encode::encode(inst)?.bytes.len();
    Ok((size, crate::timing::cost(cpu, &mc::print(inst)?, size as u32, false)?.clocks))
}

fn add(
    a: (usize, u32),
    b: (usize, u32),
) -> (usize, u32) {
    (a.0 + b.0, a.1 + b.1)
}

/// Whether `a` is the cheaper: bytes then clocks at the size level, clocks
/// then bytes otherwise.
fn cheaper(
    a: (usize, u32),
    b: (usize, u32),
    size: bool,
) -> bool {
    if size { a < b } else { (a.1, a.0) < (b.1, b.0) }
}

/// The sequences of `insts` whose stores can be pushes, with their prices.
fn sequences(
    insts: &[&Inst],
    assignment: &Assignment,
    cpu: Cpu,
    short: bool,
) -> Result<(Vec<Sequence>, Vec<i64>), String> {
    let mut found = Vec::new();
    // the call areas of the sequences that cannot be pushed
    let mut others = Vec::new();
    let mut at = 0;
    while at < insts.len() {
        let Kind::Op("ADJCALLSTACKDOWN") = insts[at].kind else {
            at += 1;
            continue;
        };
        let down = at;
        let bytes = match insts[down].operands.first() {
            Some(Operand::Imm(bytes)) => *bytes,
            _ => 0,
        };
        let call = (down + 1..insts.len()).find(|&i| insts[i].call);
        let up = call
            .and_then(|call| (call + 1..insts.len()).find(|&i| matches!(insts[i].kind, Kind::Op("ADJCALLSTACKUP"))));
        let (Some(call), Some(up)) = (call, up) else {
            others.push(bytes);
            at += 1;
            continue;
        };
        at = up + 1;
        let mut stores = Vec::new();
        let mut offsets = Vec::new();
        let mut priced = ((0usize, 0u32), (0usize, 0u32));
        let mut convertible = bytes > 0 && bytes % 4 == 0;
        for i in down + 1..call {
            if let Some((offset, source)) = argument_store(insts[i], assignment) {
                let sp = mc::Operand::Reg(reg::SP);
                let (value, store_rows) = match source {
                    Source::Register(src) => (mc::Operand::Reg(src), ("MOV32jr", "MOV32pr")),
                    Source::Immediate(v) => (mc::Operand::Imm(v), ("MOV32ji", "MOV32pi")),
                };
                let store = if offset == 0 {
                    mc::Inst { opcode: store_rows.0, operands: vec![sp.clone(), value.clone()] }
                } else {
                    mc::Inst {
                        opcode: store_rows.1,
                        operands: vec![mc::Operand::Imm(offset), sp.clone(), value.clone()],
                    }
                };
                let push = push_of(&value);
                priced.0 = add(priced.0, cost(&store, cpu)?);
                priced.1 = add(priced.1, cost(&push, cpu)?);
                stores.push(i);
                offsets.push(offset);
            } else if names_sp(insts[i], assignment) || matches!(insts[i].kind, Kind::Op("ADJCALLSTACKDOWN")) {
                convertible = false;
            }
        }
        let wanted: Vec<i64> = (0..bytes / 4).map(|n| bytes - 4 - 4 * n).collect();
        if !convertible || offsets != wanted {
            others.push(bytes);
            continue;
        }
        let cleanup = crate::expand::sp_adjust(bytes, short);
        found.push(Sequence { down, up, bytes, stores, kept: priced.0, pushed: add(priced.1, cost(&cleanup, cpu)?) });
    }
    Ok((found, others))
}

/// The pushes to make in `insts`. `frame_price(n)` is the price of the
/// prologue and epilogue when the largest call area is `n` bytes.
pub fn plan(
    insts: &[&Inst],
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
    short: bool,
    frame_price: impl Fn(i64) -> Result<(usize, u32), String>,
) -> Result<Plan, String> {
    let (found, others) = sequences(insts, assignment, cpu, short)?;
    if found.is_empty() {
        return Ok(Plan::default());
    }
    let widest = |chosen: &[bool]| {
        found
            .iter()
            .zip(chosen)
            .filter(|(_, pushed)| !**pushed)
            .map(|(one, _)| one.bytes)
            .chain(others.iter().copied())
            .max()
            .unwrap_or(0)
    };
    let total = |chosen: &[bool]| -> Result<(usize, u32), String> {
        let mut sum = frame_price(widest(chosen))?;
        for (one, pushed) in found.iter().zip(chosen) {
            sum = add(sum, if *pushed { one.pushed } else { one.kept });
        }
        Ok(sum)
    };
    let none = vec![false; found.len()];
    let all = vec![true; found.len()];
    let alone: Vec<bool> = found.iter().map(|one| cheaper(one.pushed, one.kept, size)).collect();
    let mut best = (none.clone(), total(&none)?);
    for choice in [all, alone] {
        let price = total(&choice)?;
        if cheaper(price, best.1, size) {
            best = (choice, price);
        }
    }
    let mut plan = Plan::default();
    for (one, pushed) in found.iter().zip(&best.0) {
        if *pushed {
            plan.pushes.extend(one.stores.iter().copied());
            plan.downs.insert(one.down);
            plan.ups.insert(one.up, one.bytes);
        }
    }
    Ok(plan)
}
