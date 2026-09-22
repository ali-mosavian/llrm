//! Port of `qbopt/backend/schedule.py`: latency-aware ordering of safe,
//! already-allocated machine instructions.
//!
//! This is deliberately narrower than an instruction scheduler in a flat
//! 32-bit compiler.  The medium-model output has segment state, far calls,
//! source-map anchors, and precise x87 exception ordering.  Until all of
//! those have a complete dependency model, they are boundaries.  Within the
//! remaining integer register/immediate windows, physical register and flag
//! lanes are the complete dependency graph, so a later independent operation
//! can fill the latency of an earlier producer without changing memory or
//! ABI behaviour.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::peephole::{_lanes, _register_effects, Lanes};
use crate::backend::select;
use crate::model::ir::{Loc, Operation, Space};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

const _GENERAL: [Register; 7] = [
    Register::EAX,
    Register::EBX,
    Register::ECX,
    Register::EDX,
    Register::ESI,
    Register::EDI,
    Register::EBP,
];

/// Hide measured dependency latency where the complete hardware state is known.
pub struct Scheduler {
    pub cpu: Profile,
}

impl Scheduler {
    pub fn new<'a>(cpu: impl Into<ProfileOrName<'a>>) -> Result<Self, String> {
        Ok(Self { cpu: targets::profile(cpu)?.clone() })
    }
}

impl LIRTransform for Scheduler {
    fn class_name(&self) -> &'static str {
        "Scheduler"
    }

    fn name(&self) -> &str {
        "schedule"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        scheduled(&body, &self.cpu)
    }
}

/// Return physical reads/writes for a freely movable integer occurrence.
///
/// Memory, stack/segment state, symbolic operands, calls, control transfer,
/// x87, and source-map-sensitive allocator artifacts are boundaries.  This
/// is a proof boundary, not a list of currently inconvenient cases: every
/// form left inside has only GPR/flag state represented by `_effects`.
pub fn _safe(one: &Insn) -> Option<(Lanes, Lanes)> {
    let what = one.what.as_ref()?;
    if ![
        Operation::Move,
        Operation::Binary,
        Operation::Unary,
        Operation::Multiply,
        Operation::Compare,
        Operation::Extend,
        Operation::Funnel,
        Operation::Address,
    ]
    .contains(&what.op)
        || what.dests.is_empty()
        || what.dests.iter().any(|r#where| !matches!(r#where, Loc::Reg(_)))
        || (what.op != Operation::Address
            && what.sources.iter().any(|r#where| !matches!(r#where, Loc::Reg(_) | Loc::Imm(_))))
        || (what.op == Operation::Address
            && (what.sources.len() != 1
                || !matches!(
                    &what.sources[0],
                    Loc::Address(address) if address.addr.is_some_and(|addr| addr.space == Space::Frame)
                )))
        || what.sources.iter().any(|r#where| matches!(r#where, Loc::Imm(imm) if imm.address.is_some()))
        || !one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.symbol.is_some()
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || one.rematerialized
        || one.op.as_ref().is_some_and(|op| op.barrier())
    {
        return None;
    }
    let mut registers: Vec<Register> = what
        .dests
        .iter()
        .chain(&what.sources)
        .filter_map(|r#where| match r#where {
            Loc::Reg(reg) => Some(reg.register),
            _ => None,
        })
        .collect();
    if what.op == Operation::Address {
        let Loc::Address(address) = &what.sources[0] else {
            unreachable!("checked above")
        };
        registers.extend([address.through, address.index].into_iter().filter(|register| *register != Register::None));
    }
    if registers.iter().any(|register| !_GENERAL.contains(&register.full_register32())) {
        return None;
    }
    let (reads, writes) = _register_effects(one, false, true)?;
    if reads
        .union(&writes)
        .any(|lane| lane.0 != Register::None && !_lanes(lane.0).contains(lane))
    {
        return None;
    }
    Some((reads, writes))
}

/// The audited profile key for a safe selected form, or `unknown`.
pub fn _form(one: &Insn) -> &'static str {
    let what = one.what.as_ref().expect("a safe form has semantics");
    let name = what.name.as_deref();
    if what.op == Operation::Address {
        return "lea";
    }
    if name == Some("imul") {
        // `_safe` admits only Reg and Imm operands here.
        let wide = what.dests.iter().chain(&what.sources).any(|r#where| match r#where {
            Loc::Reg(reg) => reg.width == 4,
            Loc::Imm(imm) => imm.width == 4,
            _ => false,
        });
        return if wide { "imul_r32" } else { "mul_r16" };
    }
    if matches!(name, Some("mov" | "movsx" | "movzx")) {
        if name == Some("movzx") {
            return "movzx";
        }
        return if what.sources.iter().any(|r#where| matches!(r#where, Loc::Imm(_))) { "mov_ri" } else { "mov_rr" };
    }
    if matches!(name, Some("shl" | "shr" | "sar" | "rol" | "ror")) || what.op == Operation::Funnel {
        return "shift_ri";
    }
    if matches!(name, Some("cwd" | "cdq")) {
        return "cdq";
    }
    if [Operation::Binary, Operation::Unary, Operation::Compare].contains(&what.op) {
        return "alu_rr";
    }
    "unknown"
}

pub fn _latency(one: &Insn, cpu: &Profile) -> i64 {
    let form = _form(one);
    match cpu.latency(form) {
        Ok(latency) => 1.max(latency),
        Err(_) => 1,
    }
}

/// The profile's 16/8-to-32-bit merge delay on one real dependency edge.
///
/// A full write of the same root in between replaces the partial value, so
/// the later 32-bit read does not need the old upper bytes and has no merge
/// dependency.  The narrow operand check is intentionally syntactic: this
/// post-allocation phase knows exact physical roots, not source values.
pub fn _partial_merge_delay(window: &[Arc<Insn>], producer: usize, consumer: usize, cpu: &Profile) -> i64 {
    if cpu.partial_register_stall == 0 {
        return 0;
    }
    let before = window[producer].what.as_ref().expect("a safe form has semantics");
    let after = window[consumer].what.as_ref().expect("a safe form has semantics");
    let partial: BTreeSet<Register> = before
        .dests
        .iter()
        .filter_map(|r#where| match r#where {
            Loc::Reg(reg) if reg.width < 4 && _GENERAL.contains(&reg.register.full_register32()) => {
                Some(reg.register.full_register32())
            }
            _ => None,
        })
        .collect();
    let wide: BTreeSet<Register> = after
        .sources
        .iter()
        .filter_map(|r#where| match r#where {
            Loc::Reg(reg) if reg.width == 4 && partial.contains(&reg.register.full_register32()) => {
                Some(reg.register.full_register32())
            }
            _ => None,
        })
        .collect();
    if wide.is_empty() {
        return 0;
    }
    for crossed in &window[producer + 1..consumer] {
        if let Some(what) = &crossed.what {
            if what.dests.iter().any(|r#where| {
                matches!(r#where, Loc::Reg(reg) if reg.width == 4 && wide.contains(&reg.register.full_register32()))
            }) {
                return 0;
            }
        }
    }
    cpu.partial_register_stall
}

/// `_graph`'s result: each occurrence's lanes, then its `needs` and `users`.
pub type Graph = (Vec<(Lanes, Lanes)>, Vec<BTreeSet<usize>>, Vec<BTreeSet<usize>>);

pub fn _graph(window: &[Arc<Insn>]) -> Graph {
    let effects: Vec<(Lanes, Lanes)> =
        window.iter().map(|one| _safe(one).expect("every window occurrence is safe")).collect();
    let mut needs: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); window.len()];
    let mut users: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); window.len()];
    for (left, (reads, writes)) in effects.iter().enumerate() {
        for right in left + 1..window.len() {
            let (later_reads, later_writes) = &effects[right];
            // RAW, WAR and WAW including FLAGS.  The direction is the
            // original program order; no independence is inferred from a
            // mnemonic or from a source-level value that no longer exists.
            if writes.iter().any(|lane| later_reads.contains(lane) || later_writes.contains(lane))
                || !reads.is_disjoint(later_writes)
            {
                users[left].insert(right);
                needs[right].insert(left);
            }
        }
    }
    (effects, needs, users)
}

/// The audited non-MMX Pentium U/V category for a safe register form.
///
/// GCC's local Pentium model supplies the rule, not an instruction listing:
/// operand/address-size prefixes use U only, immediate/displacement forms
/// and multiply use neither pairing slot, and the remaining register ALU or
/// move forms can issue in either pipe.  `_safe` has already ruled out memory
/// and all forms whose category is not complete here.
pub fn _pair_class(one: &Insn) -> &'static str {
    let what = one.what.as_ref().expect("a safe form has semantics");
    let Some(encoded) = select::emit(what, 0, None, false, false, None) else {
        return "np";
    };
    // GCC's Pentium description marks scalar SHLD/SHRD `pent_pair=np` even
    // though their 32-bit form also carries the otherwise-U-only 66h prefix.
    if what.op == Operation::Funnel {
        return "np";
    }
    if encoded.code.first().is_some_and(|byte| [0x66, 0x67, 0xf2, 0xf3].contains(byte)) {
        return "u";
    }
    if what.name.as_deref() == Some("imul") || what.sources.iter().any(|r#where| matches!(r#where, Loc::Imm(_))) {
        return "np";
    }
    if matches!(what.name.as_deref(), Some("shl" | "shr" | "sar" | "rol" | "ror")) {
        return "u";
    }
    "uv"
}

/// Issue independent audited U/V pairs in an in-order Pentium listing.
pub fn _pentium_ordered(window: &[Arc<Insn>], cpu: &Profile) -> Vec<Arc<Insn>> {
    let (_effects, mut needs, users) = _graph(window);
    let mut ready_at = vec![0_i64; window.len()];
    let mut left: BTreeSet<usize> = (0..window.len()).collect();
    let mut emitted: Vec<Arc<Insn>> = Vec::new();
    let mut clock = 0_i64;
    while !left.is_empty() {
        let ready: Vec<usize> =
            left.iter().copied().filter(|index| needs[*index].is_empty() && ready_at[*index] <= clock).collect();
        if ready.is_empty() {
            clock = left
                .iter()
                .filter(|index| needs[**index].is_empty())
                .map(|index| ready_at[*index])
                .min()
                .expect("min() arg is an empty sequence");
            continue;
        }
        let classes: IndexMap<usize, &str> = ready.iter().map(|index| (*index, _pair_class(&window[*index]))).collect();
        // A U-only form can pair only as the first instruction, while an
        // ordinary form can be placed in U or V.  Prefer a candidate that
        // actually makes a pair; otherwise preserve source order.
        let pair_starters: Vec<usize> = ready
            .iter()
            .copied()
            .filter(|index| {
                ["u", "uv"].contains(&classes[index])
                    && ready.iter().any(|other| other != index && classes[other] == "uv")
            })
            .collect();
        let first = *if pair_starters.is_empty() { &ready } else { &pair_starters }
            .iter()
            .min()
            .expect("ready is not empty");
        emitted.push(Arc::clone(&window[first]));
        left.remove(&first);
        let done = clock + _latency(&window[first], cpu);
        for user in &users[first] {
            needs[*user].remove(&first);
            ready_at[*user] = ready_at[*user].max(done + _partial_merge_delay(window, first, *user, cpu));
        }

        // The V slot may only receive a fully pairable form.  Its dependencies
        // were already ready before the U-slot occurrence, so it cannot read
        // a same-cycle result from that occurrence.
        let seconds: Vec<usize> =
            ready.iter().copied().filter(|index| left.contains(index) && classes[index] == "uv").collect();
        if !seconds.is_empty() && ["u", "uv"].contains(&classes[&first]) {
            let second = *seconds.iter().min().expect("seconds is not empty");
            emitted.push(Arc::clone(&window[second]));
            left.remove(&second);
            let done = clock + _latency(&window[second], cpu);
            for user in &users[second] {
                needs[*user].remove(&second);
                ready_at[*user] = ready_at[*user].max(done + _partial_merge_delay(window, second, *user, cpu));
            }
        }
        clock += 1;
    }
    emitted
}

/// List-schedule one side-effect-free window by lanes and measured latency.
pub fn _ordered(window: &[Arc<Insn>], cpu: &Profile) -> Vec<Arc<Insn>> {
    let (_effects, mut needs, users) = _graph(window);
    let mut ready_at = vec![0_i64; window.len()];
    let mut left: BTreeSet<usize> = (0..window.len()).collect();
    let mut emitted: Vec<Arc<Insn>> = Vec::new();
    let mut clock = 0_i64;
    while !left.is_empty() {
        let ready: Vec<usize> =
            left.iter().copied().filter(|index| needs[*index].is_empty() && ready_at[*index] <= clock).collect();
        if ready.is_empty() {
            clock = left
                .iter()
                .filter(|index| needs[**index].is_empty())
                .map(|index| ready_at[*index])
                .min()
                .expect("min() arg is an empty sequence");
            continue;
        }
        // Long producers first.  Ties retain source order, making the
        // scheduler deterministic and avoiding invented P5 pairing claims.
        let chosen = ready
            .iter()
            .copied()
            .max_by_key(|index| (_latency(&window[*index], cpu), -(*index as i64)))
            .expect("ready is not empty");
        emitted.push(Arc::clone(&window[chosen]));
        left.remove(&chosen);
        let done = clock + _latency(&window[chosen], cpu);
        for user in &users[chosen] {
            needs[*user].remove(&chosen);
            ready_at[*user] = ready_at[*user].max(done + _partial_merge_delay(window, chosen, *user, cpu));
        }
        // The listing has no explicit no-ops.  One issue slot was consumed;
        // skipped cycles represent hardware waiting for a dependency.
        clock += 1;
    }
    emitted
}

/// Hide safe dependency gaps for an out-of-order profile, else retain order.
///
/// Python returns `body` itself when nothing moved; here that is a clone
/// sharing every `Arc<Insn>`.
pub fn scheduled<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let target = targets::profile(cpu)?;
    // 386/486 are in-order.  P5's U/V pairing is its own audited profile
    // property rather than an inference from issue width.
    if target.in_order && !target.pentium_pairing {
        return Ok(body.clone());
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut out: Vec<Arc<Insn>> = Vec::new();
        let mut window: Vec<Arc<Insn>> = Vec::new();

        let flush = |window: &mut Vec<Arc<Insn>>, out: &mut Vec<Arc<Insn>>, changed: &mut bool| {
            if !window.is_empty() {
                let ordered =
                    if target.pentium_pairing { _pentium_ordered(window, target) } else { _ordered(window, target) };
                *changed |= ordered != *window;
                out.extend(ordered);
                window.clear();
            }
        };

        for one in &block.insns {
            if _safe(one).is_none() {
                flush(&mut window, &mut out, &mut changed);
                out.push(Arc::clone(one));
            } else {
                window.push(Arc::clone(one));
            }
        }
        flush(&mut window, &mut out, &mut changed);
        blocks.push(LirBlock { insns: out, ..block.clone() });
    }
    Ok(if changed { LirBody { blocks, ..body.clone() } } else { body.clone() })
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
