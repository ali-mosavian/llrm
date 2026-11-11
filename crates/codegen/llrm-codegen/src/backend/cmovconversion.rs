//! Conditional moves turned into branches where that is faster: LLVM's
//! `X86CmovConversion`
//! ([X86CmovConversion.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CmovConversion.cpp)).
//!
//! # How it works
//! A group is consecutive `cmovcc` in a block that read the same flags and
//! test one condition or its opposite (`collect_candidates`). Every group with
//! a load is converted, anywhere (`ForceMemOperand`: a `cmov` makes the load
//! wait for the condition). In each innermost loop the other groups are
//! converted when the loop's critical path shrinks enough and every `cmov`
//! waits longer for its condition than for its likelier value
//! (`profitable_groups`). A conversion is a diamond: the block ends in a
//! `jcc` over a block holding the loads, and the join starts with a phi per
//! `cmov` (`convert_group`).
//!
//! ```text
//!   b:  flags := cmp ...                  b:     flags := cmp ...
//!       v := cmovne f, t            =>           jne join   ; jmp skip
//!       rest                          skip: (loads)
//!                                     join:  v := phi [f, skip], [t, b]
//!                                            rest
//! ```
//!
//! # Example
//! The unit tests `a_cmov_of_a_load_in_a_loop_is_a_branch` and
//! `a_register_cmov_outside_a_loop_stays`.
//!
//! # Runs
//! On SSA, after `combiner` and before `machinelicm` (the pin's `addILPOpts`),
//! at every -O level but -O0.
//!
//! # Limits
//! DEVIATE CMC1 (owner w2, exit 2026-12-05: latencies from a per-target
//! scheduling model, else the row stays; audited 2026-11-08,
//! [permalink](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CmovConversion.cpp#L487)):
//! the latency of an instruction is the target description's row for its
//! form (`form_of`), not the scheduling model's; a copy takes 0, a form with
//! no row 1. The misprediction penalty is the target's `mispredict_penalty`;
//! with none stated only the load groups convert.
//!
//! DEVIATE CMC2 (owner w2, exit 2026-12-05: the LIR carries `SUBREG_TO_REG`,
//! else the row stays; audited 2026-11-08,
//! [permalink](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CmovConversion.cpp#L342)):
//! the `SUBREG_TO_REG` that relies on a 32-bit `cmov` zeroing the upper half
//! is a reader of the result at a wider width.
//!
//! Not ported: the unpredictable flag (`MIFlag::Unpredictable`, which nothing
//! sets here), and the debug-instruction numbers (`setDebugInstrNum`).

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::analysis::loops as loopy;
use crate::backend::cpu::Profile;
use crate::backend::flagscopy::{cond, opposite};
use crate::backend::liveness;
use crate::backend::precse::sets_flags;
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashMap;

/// `x86-cmov-converter-threshold`: the fewest cycles a loop must gain.
const GAIN_CYCLE_THRESHOLD: i64 = 4;
/// The iterations whose depth is worked out (`LoopIterations`).
const LOOP_ITERATIONS: usize = 2;

/// The phase.
pub struct CmovConversion<'a> {
    pub cpu: &'a Profile,
}

impl LIRTransform for CmovConversion<'_> {
    fn class_name(&self) -> &'static str {
        "CmovConversion"
    }

    fn name(&self) -> &str {
        "cmovconversion"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(converted(&body, self.cpu))
    }
}

/// Consecutive `cmov`s of one flags def.
type Group = Vec<Arc<Insn>>;

/// The identity of an instruction in a table.
fn key(one: &Arc<Insn>) -> *const Insn {
    Arc::as_ptr(one)
}

/// The canonical condition of a `cmovcc` with a register or a cell as its
/// second source (`getCondFromCMov`).
fn cmov_condition(one: &Insn) -> Option<&'static str> {
    let what = one.what.as_ref()?;
    let shaped = matches!(
        (what.dests.as_slice(), what.sources.as_slice()),
        ([Loc::Held(_)], [Loc::Held(_), Loc::Held(_) | Loc::Mem(_)])
    );
    if what.op != Operation::Select || !shaped {
        return None;
    }
    cond(what.name.as_deref()?.strip_prefix("cmov")?)
}

fn may_load(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.sources.iter().any(|place| matches!(place, Loc::Mem(_))))
}

/// The values `one` reads.
fn reads(one: &Insn) -> Vec<u32> {
    let mut seen: Vec<u32> = one.uses.clone();
    if let Some(what) = &one.what {
        let addresses = what.dests.iter().filter(|place| matches!(place, Loc::Mem(_)));
        seen.extend(
            what.sources
                .iter()
                .chain(addresses)
                .flat_map(|place| llrm_lir::values(place).into_iter().map(|held| held.value)),
        );
    }
    seen.extend(one.requires.iter().map(|(held, _)| held.value));
    seen.sort_unstable();
    seen.dedup();
    seen
}

/// Who reads each value: the instructions (a phi is `None`) and the widest
/// width any of them reads it at.
struct Readers {
    of: HashMap<u32, Vec<(Option<Arc<Insn>>, u32)>>,
}

impl Readers {
    fn of(body: &LirBody) -> Self {
        let mut of: HashMap<u32, Vec<(Option<Arc<Insn>>, u32)>> = HashMap::default();
        for block in &body.blocks {
            for phi in block.phis.iter() {
                for &(_, value) in &phi.incoming {
                    of.entry(value).or_default().push((None, 0));
                }
            }
            for one in block.insns.iter() {
                let mut widest: HashMap<u32, u32> = HashMap::default();
                if let Some(what) = &one.what {
                    for place in &what.sources {
                        if let Loc::Held(held) = place {
                            let width = widest.entry(held.value).or_default();
                            *width = (*width).max(held.width);
                        }
                    }
                }
                for value in reads(one) {
                    of.entry(value)
                        .or_default()
                        .push((Some(Arc::clone(one)), widest.get(&value).copied().unwrap_or(0)));
                }
            }
        }
        Readers { of }
    }

    /// `SUBREG_TO_REG` of the result of a 32-bit `cmov` (DEVIATE CMC2).
    fn relies_on_zero_extension(
        &self,
        one: &Insn,
        bits: u32,
    ) -> bool {
        let Some(Loc::Held(result)) = one.what.as_ref().and_then(|what| what.dests.first()) else { return false };
        bits == 64
            && result.width == 4
            && self.of.get(&result.value).is_some_and(|readers| readers.iter().any(|(_, width)| *width > 4))
    }

    /// The one instruction that reads `value`, if exactly one does.
    fn only(
        &self,
        value: u32,
    ) -> Option<&Arc<Insn>> {
        match self.of.get(&value)?.as_slice() {
            [(Some(one), _)] => Some(one),
            _ => None,
        }
    }
}

/// `collectCmovCandidates`: the groups of `blocks`.
fn collect_candidates(
    blocks: &[&Block],
    regs: Regs,
    bits: u32,
    readers: &Readers,
    include_loads: bool,
) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for block in blocks {
        let mut group: Group = Vec::new();
        let (mut first, mut first_opposite, mut memory_condition) = ("", "", None::<&str>);
        let (mut found_other, mut skip) = (false, false);
        for one in block.insns.iter() {
            if one.is_meta() {
                continue;
            }
            if let Some(code) = cmov_condition(one)
                && (include_loads || !may_load(one))
                && !one.volatile
            {
                if group.is_empty() {
                    first = code;
                    first_opposite = opposite(code);
                    memory_condition = None;
                    found_other = false;
                    skip = false;
                }
                group.push(Arc::clone(one));
                if found_other || (code != first && code != first_opposite) {
                    skip = true;
                }
                if may_load(one) {
                    match memory_condition {
                        None => memory_condition = Some(code),
                        Some(seen) if seen != code => skip = true,
                        Some(_) => {}
                    }
                }
                if !skip && readers.relies_on_zero_extension(one, bits) {
                    skip = true;
                }
                continue;
            }
            if group.is_empty() {
                continue;
            }
            found_other = true;
            if sets_flags(regs, bits, one) {
                if !skip {
                    groups.push(std::mem::take(&mut group));
                }
                group.clear();
            }
        }
        if !group.is_empty() && !skip {
            groups.push(group);
        }
    }
    groups
}

/// `getDepthOfOptCmov`: the depth of a `cmov` as a branch, at 75% for one value
/// and 25% for the other, the larger of the two.
fn depth_of_branch(
    true_depth: i64,
    false_depth: i64,
) -> i64 {
    let quarter = |a: i64, b: i64| (a * 3 + b + 3) / 4;
    quarter(true_depth, false_depth).max(quarter(false_depth, true_depth))
}

/// The row of the target's latency table that prices `one` (DEVIATE CMC1).
fn form_of(one: &Insn) -> Option<&'static str> {
    let what = one.what.as_ref()?;
    let (loads, stores) = (
        what.sources.iter().any(|place| matches!(place, Loc::Mem(_))),
        what.dests.iter().any(|place| matches!(place, Loc::Mem(_))),
    );
    Some(match what.op {
        Operation::Move | Operation::Extend if loads => "mov_rm",
        Operation::Move if stores => "mov_mr",
        Operation::Move if what.sources.iter().any(|place| matches!(place, Loc::Imm(_))) => "mov_ri",
        // A copy of a register is transient.
        Operation::Move if matches!(what.sources.as_slice(), [Loc::Held(_)]) => return None,
        Operation::Move | Operation::Extend => "movzx",
        Operation::Select => "cmov",
        Operation::Multiply if loads => "imul_m32",
        Operation::Multiply => "imul_r32",
        Operation::Divide if loads => "idiv_m32",
        Operation::Divide => "idiv_r32",
        Operation::Address => "lea",
        Operation::Funnel => "shift_ri",
        Operation::Push => "push_r",
        Operation::Pop => "pop_r",
        Operation::Call => "call_near",
        Operation::Branch => "jcc",
        Operation::Jump => "jmp_short",
        Operation::Return => "ret_near",
        _ if loads => "alu_rm",
        _ if stores => "alu_mr",
        _ => "alu_rr",
    })
}

fn latency_of(
    one: &Insn,
    cpu: &Profile,
) -> i64 {
    match form_of(one) {
        None => 0,
        Some(form) => match cpu.latency(form) {
            Ok(clocks) => clocks.max(1),
            Err(_) => {
                llrm_support::debug!("cmov", "no latency row for {form}");
                1
            }
        },
    }
}

/// One instruction (or phi) of the loop, as the depth walk reads it.
struct Node {
    defines: Vec<u32>,
    uses: Vec<u32>,
    reads_flags: bool,
    sets_flags: bool,
    latency: i64,
    /// For a `cmov` of a candidate group: its false value and its true value.
    cmov: Option<(u32, u32)>,
}

fn reads_flags(
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> bool {
    liveness::effect(regs, bits, one)
        .is_some_and(|effect| effect.reads.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None))
}

/// The depth of an instruction and of the same loop with the groups as
/// branches.
#[derive(Clone, Copy, Default)]
struct Depth {
    plain: i64,
    branch: i64,
}

/// `checkForProfitableCmovCandidates`: the groups of the loop `blocks` that
/// are worth converting.
fn profitable_groups(
    body: &LirBody,
    blocks: &[&Block],
    groups: Vec<Group>,
    readers: &Readers,
    cpu: &Profile,
    penalty: i64,
) -> Vec<Group> {
    let (regs, bits) = (body.regs(), body.bits);
    let candidates: HashMap<*const Insn, (u32, u32)> = groups
        .iter()
        .flatten()
        .filter_map(|one| match one.what.as_ref()?.sources.as_slice() {
            [Loc::Held(no), Loc::Held(yes)] => Some((key(one), (no.value, yes.value))),
            _ => None,
        })
        .collect();

    // Step 1: instruction depth and loop depth.
    let mut nodes: Vec<Node> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut index_of: HashMap<*const Insn, usize> = HashMap::default();
    for block in blocks {
        let start = nodes.len();
        for phi in block.phis.iter() {
            nodes.push(Node {
                defines: vec![phi.result],
                uses: phi.incoming.iter().map(|&(_, value)| value).collect(),
                reads_flags: false,
                sets_flags: false,
                latency: 0,
                cmov: None,
            });
        }
        for one in block.insns.iter().filter(|one| !one.is_meta()) {
            index_of.insert(key(one), nodes.len());
            nodes.push(Node {
                defines: one.defines.clone(),
                uses: reads(one),
                reads_flags: reads_flags(regs, bits, one),
                sets_flags: sets_flags(regs, bits, one),
                latency: latency_of(one, cpu),
                cmov: candidates.get(&key(one)).copied(),
            });
        }
        spans.push((start, nodes.len()));
    }

    let mut depth = vec![Depth::default(); nodes.len()];
    let mut defined_by: HashMap<u32, usize> = HashMap::default();
    // Where a `cmov` found its false value, true value and flags.
    let mut operand_def: Vec<[Option<usize>; 3]> = vec![[None; 3]; nodes.len()];
    let mut loop_depth = [Depth::default(); LOOP_ITERATIONS];
    for max in &mut loop_depth {
        for &(start, end) in &spans {
            let mut flags_def: Option<usize> = None;
            for at in start..end {
                let (mut plain, mut branch) = (0, 0);
                let mut take = |def: usize| {
                    plain = plain.max(depth[def].plain);
                    if nodes[at].cmov.is_none() {
                        branch = branch.max(depth[def].branch);
                    }
                };
                for &used in &nodes[at].uses {
                    if let Some(&def) = defined_by.get(&used) {
                        if let Some((no, yes)) = nodes[at].cmov {
                            if used == no {
                                operand_def[at][0] = Some(def);
                            }
                            if used == yes {
                                operand_def[at][1] = Some(def);
                            }
                        }
                        take(def);
                    }
                }
                if nodes[at].reads_flags
                    && let Some(def) = flags_def
                {
                    operand_def[at][2] = Some(def);
                    take(def);
                }
                if nodes[at].cmov.is_some() {
                    let branch_of = |slot: usize| operand_def[at][slot].map_or(0, |def| depth[def].branch);
                    branch = depth_of_branch(branch_of(0), branch_of(1));
                }
                for &defined in &nodes[at].defines {
                    defined_by.insert(defined, at);
                }
                if nodes[at].sets_flags {
                    flags_def = Some(at);
                }
                depth[at] = Depth { plain: plain + nodes[at].latency, branch: branch + nodes[at].latency };
                max.plain = max.plain.max(depth[at].plain);
                max.branch = max.branch.max(depth[at].branch);
            }
        }
    }
    let diff = [loop_depth[0].plain - loop_depth[0].branch, loop_depth[1].plain - loop_depth[1].branch];

    llrm_support::debug!(
        "cmov",
        "loop of {} blocks: depth plain {:?} branch {:?}, diff {diff:?}, {} groups",
        blocks.len(),
        loop_depth.map(|one| one.plain),
        loop_depth.map(|one| one.branch),
        groups.len()
    );
    // Step 2: is the loop worth it.
    if diff[1] < GAIN_CYCLE_THRESHOLD {
        return Vec::new();
    }
    let worth = if diff[1] == diff[0] {
        diff[0] * 8 >= loop_depth[0].plain
    } else if diff[1] > diff[0] {
        (diff[1] - diff[0]) * 2 >= loop_depth[1].plain - loop_depth[0].plain && diff[1] * 8 >= loop_depth[1].plain
    } else {
        false
    };
    if !worth {
        return Vec::new();
    }

    // Step 3: is each group worth it.
    let plain_of = |at: usize, slot: usize| operand_def[at][slot].map_or(0, |def| depth[def].plain);
    groups
        .into_iter()
        .filter(|group| {
            group.iter().all(|one| {
                // A result that is an address: a search tree's branch, which
                // nothing predicts.
                let result = one.defines.first().copied();
                let loaded_from = result.and_then(|value| readers.only(value)).is_some_and(|reader| {
                    reader.what.as_ref().is_some_and(|what| {
                        what.op == Operation::Move
                            && what.dests.iter().any(|place| matches!(place, Loc::Held(held) if held.width >= 4))
                    }) && may_load(reader)
                });
                if loaded_from {
                    return false;
                }
                let at = index_of[&key(one)];
                let condition = plain_of(at, 2);
                let value = depth_of_branch(plain_of(at, 0), plain_of(at, 1));
                llrm_support::debug!("cmov", "  cmov condition {condition} value {value} penalty {penalty}");
                value <= condition && (condition - value) * 4 >= penalty
            })
        })
        .collect()
}

/// The loop-carried facts `convert_group` keeps current.
struct Work {
    blocks: Vec<Block>,
    odds: crate::model::lir::BlockOdds,
    next_at: i64,
    next_value: u32,
    /// The value a made value is of the class of.
    like: Vec<(u32, u32)>,
    /// The loops, by block, that new blocks join with the block they came from.
    loops: Vec<BTreeSet<i64>>,
    /// The block of each instruction of a group, kept through splits.
    home: HashMap<*const Insn, i64>,
}

impl Work {
    fn position(
        &self,
        at: i64,
    ) -> usize {
        self.blocks.iter().position(|block| block.at == at).expect("a block of the body")
    }
}

fn held_of(place: &Loc) -> Held {
    match place {
        Loc::Held(held) => *held,
        other => unreachable!("a cmov source is a register here: {other:?}"),
    }
}

fn jump(
    name: &str,
    op: Operation,
    at: i64,
    target: i64,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), target: Some(target), ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

/// `convertCmovInstsToBranches`.
fn convert_group(
    work: &mut Work,
    group: &Group,
) {
    let first = &group[0];
    let last = &group[group.len() - 1];
    let mbb_at = work.home[&key(first)];
    let at_block = work.position(mbb_at);

    // `packCmovGroup`: markers between the first and last go after the last.
    let mut insns: Vec<Arc<Insn>> = work.blocks[at_block].insns.to_vec();
    let from = insns.iter().position(|one| Arc::ptr_eq(one, first)).expect("the group's first");
    let to = insns.iter().position(|one| Arc::ptr_eq(one, last)).expect("the group's last");
    let (cmovs, markers): (Vec<_>, Vec<_>) = insns.drain(from..=to).partition(|one| !one.is_meta());
    let tail_start = from;

    let mut code = cmov_condition(first).expect("a cmov");
    let mut opposite_code = opposite(code);
    // A cell goes on the path that skips the branch, so its `cmov` tests the
    // opposite of the branch.
    if cmovs.iter().any(|one| may_load(one) && cmov_condition(one) == Some(code)) {
        std::mem::swap(&mut code, &mut opposite_code);
    }

    let false_at = work.next_at + 1;
    let sink_at = work.next_at + 2;
    work.next_at += 2;

    // Loads on the false path, and the value each `cmov` leaves there.
    let mut false_path: Vec<Arc<Insn>> = Vec::new();
    let mut false_value: HashMap<u32, u32> = HashMap::default();
    let mut rewritten: Vec<Arc<Insn>> = Vec::new();
    for one in &cmovs {
        let what = one.what.as_ref().expect("a cmov");
        let dest = held_of(&what.dests[0]);
        if !may_load(one) {
            let mut value = held_of(&what.sources[usize::from(cmov_condition(one) != Some(code))]).value;
            while let Some(&earlier) = false_value.get(&value) {
                value = earlier;
            }
            false_value.insert(dest.value, value);
            rewritten.push(Arc::clone(one));
            continue;
        }
        let Loc::Mem(cell) = &what.sources[1] else { unreachable!("a loading cmov has a cell") };
        let tmp = work.next_value;
        work.next_value += 1;
        work.like.push((tmp, dest.value));
        let cell = {
            let renamed = llrm_lir::mapped(&Loc::Mem(cell.clone()), |held| Held {
                value: false_value.get(&held.value).copied().unwrap_or(held.value),
                ..*held
            });
            match renamed {
                Loc::Mem(cell) => cell,
                _ => unreachable!("a cell maps to a cell"),
            }
        };
        let tmp_held = Held { value: tmp, ..dest };
        let load = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(tmp_held)],
            sources: vec![Loc::Mem(cell.clone())],
            ..Semantics::new(Operation::Move)
        };
        let uses: Vec<u32> = llrm_lir::values(&Loc::Mem(cell)).into_iter().map(|held| held.value).collect();
        let mut made = (**one).clone();
        made.what = Some(load);
        made.defines = vec![tmp];
        made.uses = uses;
        made.covers = None;
        false_path.push(Arc::new(made));
        // The `cmov` now takes the loaded value.
        let mut sources = what.sources.clone();
        sources[1] = Loc::Held(tmp_held);
        let mut changed = (**one).clone();
        let mut new_what = what.clone();
        new_what.sources = sources;
        changed.what = Some(new_what);
        changed.uses = vec![held_of(&what.sources[0]).value, tmp];
        false_value.insert(dest.value, tmp);
        rewritten.push(Arc::new(changed));
    }

    // The phis: later ones name the inputs of earlier ones, not their results.
    let mut phis: Vec<Phi> = Vec::new();
    let mut inputs: HashMap<u32, (u32, u32)> = HashMap::default();
    for one in &rewritten {
        let what = one.what.as_ref().expect("a cmov");
        let dest = held_of(&what.dests[0]).value;
        let (mut no, mut yes) = (held_of(&what.sources[0]).value, held_of(&what.sources[1]).value);
        if cmov_condition(one) == Some(opposite_code) {
            std::mem::swap(&mut no, &mut yes);
        }
        let no = inputs.get(&no).map_or(no, |&(first, _)| first);
        let yes = inputs.get(&yes).map_or(yes, |&(_, second)| second);
        phis.push(Phi { result: dest, incoming: vec![(false_at, no), (mbb_at, yes)] });
        inputs.insert(dest, (no, yes));
    }

    let mbb = &mut work.blocks[at_block];
    let succ: Vec<i64> = mbb.succ.to_vec();
    let tail: Vec<Arc<Insn>> = insns.split_off(tail_start);
    let at = last.at;
    insns.push(jump(&format!("j{code}"), Operation::Branch, at, sink_at));
    insns.push(jump("jmp", Operation::Jump, at, false_at));
    let cold = mbb.cold;
    mbb.succ = vec![false_at, sink_at].into();
    *mbb = mbb.with_insns(insns);

    // The join: the bytes of the moved `cmov`s, then the rest of the block.
    let mut sink_insns: Vec<Arc<Insn>> =
        cmovs.iter().chain(markers.iter()).map(crate::model::lir::bytes_only).collect();
    for one in &tail {
        if work.home.contains_key(&key(one)) {
            work.home.insert(key(one), sink_at);
        }
    }
    sink_insns.extend(tail);
    let mut sink = LirBlock::new(sink_at, sink_insns);
    sink.succ = succ.clone().into();
    sink.phis = phis.into();
    sink.cold = cold;
    let mut skip = LirBlock::new(false_at, {
        let mut list = false_path;
        list.push(jump("jmp", Operation::Jump, at, sink_at));
        list
    });
    skip.succ = vec![sink_at].into();
    skip.cold = cold;

    // `transferSuccessorsAndUpdatePHIs`.
    for &next in succ.iter().collect::<BTreeSet<_>>() {
        let place = if next == mbb_at { at_block } else { work.position(next) };
        let phis: Vec<Phi> = work.blocks[place]
            .phis
            .iter()
            .map(|phi| Phi {
                result: phi.result,
                incoming: phi
                    .incoming
                    .iter()
                    .map(|&(from, v)| (if from == mbb_at { sink_at } else { from }, v))
                    .collect(),
            })
            .collect();
        work.blocks[place].phis = phis.into();
    }
    let moved: Vec<((i64, i64), u32)> =
        work.odds.taken.iter().filter(|((from, _), _)| *from == mbb_at).map(|(&edge, &odds)| (edge, odds)).collect();
    for ((from, to), odds) in moved {
        work.odds.taken.shift_remove(&(from, to));
        work.odds.taken.insert((sink_at, to), odds);
    }
    let place = work.position(mbb_at) + 1;
    work.blocks.insert(place, skip.into());
    work.blocks.insert(place + 1, sink.into());
    for set in &mut work.loops {
        if set.contains(&mbb_at) {
            set.insert(false_at);
            set.insert(sink_at);
        }
    }
}

#[cfg(test)]
#[path = "cmovconversion_tests.rs"]
mod tests;

/// `runOnMachineFunction`.
pub fn converted(
    body: &LirBody,
    cpu: &Profile,
) -> LirBody {
    if !body.iter_insns().any(|one| cmov_condition(one).is_some()) {
        return body.clone();
    }
    let (regs, bits) = (body.regs(), body.bits);
    let readers = Readers::of(body);
    let mut work = Work {
        blocks: body.blocks.clone(),
        odds: body.odds.clone(),
        next_at: body.blocks.iter().map(|block| block.at).max().unwrap_or(0),
        next_value: body.next_value(),
        like: Vec::new(),
        loops: Vec::new(),
        home: HashMap::default(),
    };
    for block in &body.blocks {
        for one in block.insns.iter().filter(|one| cmov_condition(one).is_some()) {
            work.home.insert(key(one), block.at);
        }
    }
    let found = loopy::loops(&body.blocks, Some(body.entry));
    work.loops = found.iter().map(|one| one.body.to_set()).collect();
    let mut changed = false;

    // `ForceMemOperand`: a `cmov` that loads waits for the condition.
    let everywhere: Vec<&Block> = body.blocks.iter().collect();
    for group in collect_candidates(&everywhere, regs, bits, &readers, true) {
        if group.iter().any(|one| may_load(one)) {
            convert_group(&mut work, &group);
            changed = true;
        }
    }

    // The groups of registers, in the innermost loops.
    llrm_support::debug!("cmov", "penalty {:?}, {} loops", cpu.mispredict_penalty_here(), found.len());
    if let Some(penalty) = cpu.mispredict_penalty_here() {
        let headers: Vec<i64> = found.iter().map(|one| one.header).collect();
        let first_sets = work.loops.clone();
        for (number, header) in headers.iter().enumerate() {
            let nested = headers
                .iter()
                .enumerate()
                .any(|(other, inner)| other != number && inner != header && first_sets[number].contains(inner));
            if nested {
                continue;
            }
            // The blocks as the conversions above left them.
            let inside: Vec<Block> =
                work.blocks.iter().filter(|block| work.loops[number].contains(&block.at)).cloned().collect();
            let inside: Vec<&Block> = inside.iter().collect();
            let groups = collect_candidates(&inside, regs, bits, &readers, false);
            if groups.is_empty() {
                continue;
            }
            for group in profitable_groups(body, &inside, groups, &readers, cpu, penalty) {
                convert_group(&mut work, &group);
                changed = true;
            }
        }
    }
    if !changed {
        return body.clone();
    }
    let mut out = body.with_blocks(std::mem::take(&mut work.blocks));
    out.odds = work.odds;
    let table = Arc::make_mut(&mut out.class_like);
    for (value, old) in work.like {
        table.insert(value, old);
    }
    out
}
