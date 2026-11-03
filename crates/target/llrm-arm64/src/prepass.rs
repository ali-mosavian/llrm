//! Machine-level redundancy removal before registers are assigned: dead
//! operations (`DeadMachineInstructionElim`), common
//! subexpression elimination over the dominator tree (`MachineCSE`) and the
//! hoisting of loop-invariant definitions to the loop's preheader
//! (`MachineLICM`), for the values selection makes at each use: constants,
//! frame and global addresses. Only side-effect-free operations whose result is
//! a virtual register defined once are touched. A loop hoists only as many
//! values as the registers left over once what it already holds is counted
//! (`MachineLICM`'s `CanCauseHighRegPressure`): a value hoisted past that is
//! spilled for the whole loop, dearer than making it at each use. The most
//! used go first. What the loop already holds is what is live into its header,
//! and equal values made in several blocks count once and are one value in the
//! preheader (a second `MachineCSE` after the hoist).
//!
//! A flag-setting add or subtract defines a register as the pin's `SUBSWri`
//! does, so equal ones merge: one whose flags are dead is shared with an
//! earlier equal one (`MachineCSE`). Afterwards one whose register nothing
//! reads defines the zero register (`AArch64DeadRegisterDefinitions`, a
//! compare) and one whose flags nothing reads is the plain instruction
//! (`optimizeCompareInstr`, `convertToNonFlagSettingOpc`). A copy of a
//! single-definition register is read through when expressions are compared
//! (`PerformTrivialCopyPropagation`).
//!
//! A pure value, or a copy of the zero register, that is read only below its
//! block is moved to the start of the nearest block dominating its readers
//! (`MachineSink`), so a phi's constant is made on its edge and not on the
//! paths that skip it. The pin pays the same extra jump where that edge block
//! does not fall through to its target.
//!
//! DEVIATE SD530: LLVM runs these on the shared machine IR; this is a small
//! version over the arm64 selector's operations, standing in until the core
//! machine passes run on arm64 (docs/architecture/arm64-spike.md).

use std::collections::{HashMap, HashSet};

use crate::mc::inst::*;
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op, successors_of};

/// Whether `op` only computes a virtual register from virtual registers and
/// constants, with no other effect.
fn pure(op: &Op) -> bool {
    match op {
        Op::Imm { .. } | Op::FrameAddress { .. } => true,
        Op::Copy { .. } => false,
        // A sum of two registers, as an address makes: equal sums of virtual
        // registers are one value.
        Op::I(
            Inst::AddSubShifted { op: AddSub { set_flags: false, .. }, .. }
            | Inst::AddSubExtended { op: AddSub { set_flags: false, .. }, .. },
        ) => {
            let (defs, uses) = op.defs_and_uses();
            defs.iter().chain(&uses).all(|register| register.is_virtual())
        }
        Op::I(inst) => {
            matches!(
                inst,
                Inst::MoveWide { op: Wide::Not | Wide::Zero, .. }
                    | Inst::LogicImm { op: Logic::And | Logic::Orr | Logic::Eor, .. }
                    | Inst::AddSubImm { op: AddSub { set_flags: false, .. }, .. }
                    | Inst::AddPageOffset { .. }
                    | Inst::Adr { .. }
                    | Inst::FpMoveImm { .. }
                    | Inst::VectorFill { fill: Fill::Zero | Fill::Byte(_), .. }
                    | Inst::FpInt { op: FpConvert::Bits { to_fp: true }, .. }
            ) && !matches!(inst, Inst::AddSubImm { rd, .. } if !rd.is_virtual())
                // Reading the stack pointer (stacksave, a dynamic alloca's
                // object) depends on where it is.
                && !matches!(inst, Inst::AddSubImm { rn, .. } if rn.0 == 31)
        }
        _ => false,
    }
}

/// The one register `op` defines, if it is virtual.
fn defined(op: &Op) -> Option<Reg> {
    let (defs, _) = op.defs_and_uses();
    match defs.as_slice() {
        [one] if one.is_virtual() => Some(*one),
        _ => None,
    }
}

/// `op` with its destination register made a fixed one, so that equal
/// computations compare equal.
fn keyed(op: &Op) -> String {
    let mut copy = op.clone();
    copy.visit(&mut |register, role| {
        if role == Role::Def {
            match register {
                RegRef::Gpr(slot) => *slot = Gpr(0),
                RegRef::Fpr(slot) => *slot = Vreg(0),
            }
        }
    });
    format!("{copy:?}")
}

struct Cfg {
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    /// Immediate dominators; the entry's is itself.
    idom: Vec<usize>,
    /// Reachable blocks in reverse post-order.
    order: Vec<usize>,
}

fn cfg(function: &MFunc) -> Cfg {
    let count = function.blocks.len();
    let labels: HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, one)| (one.label.as_str(), at)).collect();
    let successors: Vec<Vec<usize>> =
        function.blocks.iter().enumerate().map(|(at, block)| successors_of(block, at, count, &labels)).collect();
    let mut predecessors = vec![Vec::new(); count];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }
    // Reverse post-order from the entry.
    let mut seen = vec![false; count];
    let mut post = Vec::new();
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some((block, next)) = stack.last_mut() {
        if *next < successors[*block].len() {
            let successor = successors[*block][*next];
            *next += 1;
            if !seen[successor] {
                seen[successor] = true;
                stack.push((successor, 0));
            }
        } else {
            post.push(*block);
            stack.pop();
        }
    }
    post.reverse();
    let position: HashMap<usize, usize> = post.iter().enumerate().map(|(at, block)| (*block, at)).collect();
    // Cooper, Harvey and Kennedy's iterative dominators.
    let mut idom = vec![usize::MAX; count];
    idom[0] = 0;
    let mut changed = true;
    while changed {
        changed = false;
        for &block in post.iter().skip(1) {
            let mut new = usize::MAX;
            for &predecessor in &predecessors[block] {
                if idom[predecessor] == usize::MAX {
                    continue;
                }
                new = if new == usize::MAX {
                    predecessor
                } else {
                    let (mut a, mut b) = (predecessor, new);
                    while a != b {
                        while position[&a] > position[&b] {
                            a = idom[a];
                        }
                        while position[&b] > position[&a] {
                            b = idom[b];
                        }
                    }
                    a
                };
            }
            if new != idom[block] {
                idom[block] = new;
                changed = true;
            }
        }
    }
    Cfg { successors, predecessors, idom, order: post }
}

fn dominates(
    cfg: &Cfg,
    a: usize,
    mut b: usize,
) -> bool {
    loop {
        if a == b {
            return true;
        }
        if b == 0 || cfg.idom[b] == usize::MAX {
            return false;
        }
        b = cfg.idom[b];
    }
}

/// How many definitions each virtual register has.
fn definitions(function: &MFunc) -> HashMap<Reg, usize> {
    let mut count: HashMap<Reg, usize> = HashMap::new();
    for block in &function.blocks {
        for op in &block.ops {
            for register in op.defs_and_uses().0 {
                *count.entry(register).or_default() += 1;
            }
        }
    }
    count
}

/// Whether `op` only computes registers, so that it can go when none is read.
/// Loads stay (a volatile load is not told apart), and so does anything that
/// sets the flags.
fn removable(op: &Op) -> bool {
    match op {
        Op::Imm { .. } | Op::FrameAddress { .. } | Op::Copy { .. } => true,
        Op::I(inst) => match inst {
            Inst::AddSubImm { op, .. } | Inst::AddSubShifted { op, .. } | Inst::AddSubExtended { op, .. } => {
                !op.set_flags
            }
            Inst::LogicImm { op, .. } | Inst::LogicShifted { op, .. } => *op != Logic::Ands,
            Inst::MoveWide { op, .. } => *op != Wide::Keep,
            Inst::AddPageOffset { .. }
            | Inst::BitfieldMove { .. }
            | Inst::Extract { .. }
            | Inst::Divide { .. }
            | Inst::OneSource { .. }
            | Inst::ThreeSource { .. }
            | Inst::CondSelect { .. }
            | Inst::Adr { .. }
            | Inst::FpUnary { .. }
            | Inst::FpBinary { .. }
            | Inst::FpFused { .. }
            | Inst::FpSelect { .. }
            | Inst::FpMoveImm { .. }
            | Inst::FpToFp { .. }
            | Inst::FpInt { .. } => true,
            _ => false,
        },
        _ => false,
    }
}

/// Operations whose every result is a virtual register nothing reads, until
/// none is left.
fn dead_code(function: &mut MFunc) {
    loop {
        let read: HashSet<Reg> =
            function.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.defs_and_uses().1).collect();
        let mut changed = false;
        for block in &mut function.blocks {
            let before = block.ops.len();
            block.ops.retain(|op| {
                let (defs, _) = op.defs_and_uses();
                !(removable(op) && !defs.is_empty() && defs.iter().all(|one| one.is_virtual() && !read.contains(one)))
            });
            changed |= block.ops.len() != before;
        }
        if !changed {
            return;
        }
    }
}

pub fn run(function: &mut MFunc) {
    dead_code(function);
    unsplit_edges(function);
    if function.blocks.len() < 2 && function.blocks.iter().all(|block| block.ops.len() < 3) {
        return;
    }
    cse(function);
    hoist(function);
    // What was hoisted from several blocks is one value in the preheader.
    cse(function);
    sink(function);
    dead_definitions(function);
    non_flag_setting(function);
    redundant_zero_extends(function);
}

/// A flag-setting instruction whose register nothing reads defines the zero
/// register (`AArch64DeadRegisterDefinitions`): that is a compare.
fn dead_definitions(function: &mut MFunc) {
    let read: HashSet<Reg> =
        function.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.defs_and_uses().1).collect();
    for block in &mut function.blocks {
        for op in &mut block.ops {
            if let Op::I(Inst::AddSubImm { rd, .. } | Inst::AddSubShifted { rd, .. } | Inst::AddSubExtended { rd, .. }) =
                op
                && rd.is_virtual()
                && !read.contains(&Reg::Gpr(*rd))
            {
                *rd = Gpr::R31;
            }
        }
    }
}

/// An add or subtract that sets flags nothing reads is the plain instruction
/// (`optimizeCompareInstr`, `convertToNonFlagSettingOpc`).
fn non_flag_setting(function: &mut MFunc) {
    for block in &mut function.blocks {
        for at in 0..block.ops.len() {
            if flag_setting(&block.ops[at]) && flags_dead(&block.ops, at) {
                if let Op::I(
                    Inst::AddSubImm { op, .. } | Inst::AddSubShifted { op, .. } | Inst::AddSubExtended { op, .. },
                ) = block.ops.op_mut(at)
                {
                    op.set_flags = false;
                }
            }
        }
    }
}

/// A pure value that is read only below the block making it is made at the
/// start of the nearest block that dominates its readers, so the paths that
/// skip them do not pay for it (`MachineSink`; a phi's constant is made on its
/// edge). A block in a loop the maker is not in is no target. A value moves one
/// step at a time: what it reads goes next, in the following round.
fn sink(function: &mut MFunc) {
    for _ in 0..16 {
        let cfg = cfg(function);
        let definitions = definitions(function);
        let loops = natural_loops(function, &cfg);
        let depth = |block: usize| loops.iter().filter(|(_, body)| body.contains(&block)).count();
        let mut readers: HashMap<Reg, Vec<usize>> = HashMap::new();
        for (at, block) in function.blocks.iter().enumerate() {
            for op in &block.ops {
                for register in op.defs_and_uses().1 {
                    readers.entry(register).or_default().push(at);
                }
            }
        }
        let mut moves: Vec<(usize, usize, usize)> = Vec::new();
        for &from in &cfg.order {
            for (index, op) in function.blocks[from].ops.iter().enumerate() {
                // The zero register is the same value everywhere.
                let zero = matches!(
                    op,
                    Op::Copy { dst, src } if dst.is_virtual() && *src == Reg::Gpr(Gpr::R31)
                );
                if !(pure(op) || zero) {
                    continue;
                }
                let Some(register) = defined(op) else { continue };
                let once = |one: &Reg| definitions.get(one) == Some(&1);
                if !once(&register) || !(zero || op.defs_and_uses().1.iter().all(once)) {
                    continue;
                }
                let Some(blocks) = readers.get(&register) else { continue };
                let mut target = blocks[0];
                for &reader in &blocks[1..] {
                    while !dominates(&cfg, target, reader) {
                        target = cfg.idom[target];
                    }
                }
                // "Don't sink into a deeper cycle": the work would repeat on
                // each trip of an inner loop.
                if target != from
                    && cfg.order.contains(&target)
                    && dominates(&cfg, from, target)
                    && depth(target) <= depth(from)
                {
                    moves.push((from, index, target));
                }
            }
        }
        if moves.is_empty() {
            return;
        }
        let mut sunk: HashMap<usize, Vec<Op>> = HashMap::new();
        for &(from, index, target) in &moves {
            sunk.entry(target).or_default().push(function.blocks[from].ops[index].clone());
        }
        for &(from, index, _) in moves.iter().rev() {
            function.blocks[from].ops.remove(index);
        }
        for (target, ops) in sunk {
            for op in ops.into_iter().rev() {
                function.blocks[target].ops.insert(0, op);
            }
        }
    }
}

/// The virtual registers that are plain copies of another, each defined once:
/// reading the copy is reading its source (`PerformTrivialCopyPropagation`).
fn single_copies(
    function: &MFunc,
    definitions: &HashMap<Reg, usize>,
) -> HashMap<Reg, Reg> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|op| match op {
            Op::Copy { dst, src } if dst.is_virtual() && src.is_virtual() => Some((*dst, *src)),
            _ => None,
        })
        .filter(|(dst, src)| definitions.get(dst) == Some(&1) && definitions.get(src) == Some(&1))
        .collect()
}

/// `op` with the uses that earlier removals renamed, and copies read through.
fn renamed(
    op: &Op,
    rename: &HashMap<Reg, Reg>,
    copies: &HashMap<Reg, Reg>,
) -> Op {
    let mut copy = op.clone();
    copy.visit(&mut |slot, role| {
        if role == Role::Use {
            let mut key = slot.key();
            while let Some(&found) = rename.get(&key).or_else(|| copies.get(&key)) {
                key = found;
            }
            match (slot, key) {
                (RegRef::Gpr(slot), Reg::Gpr(new)) => *slot = new,
                (RegRef::Fpr(slot), Reg::Fpr(new)) => *slot = new,
                _ => {}
            }
        }
    });
    copy
}

/// Whether `op` sets the flags and defines a register, as the pin's `SUBSWri`
/// with a `$nzcv` def that may be dead.
fn flag_setting(op: &Op) -> bool {
    match op {
        Op::I(
            Inst::AddSubImm { op, rd, .. } | Inst::AddSubShifted { op, rd, .. } | Inst::AddSubExtended { op, rd, .. },
        ) => op.set_flags && rd.is_virtual(),
        _ => false,
    }
}

/// Whether the flags `block.ops[at]` sets are overwritten before anything reads
/// them (`$nzcv` is `dead`). The flags do not live out of a block.
fn flags_dead(
    ops: &[Op],
    at: usize,
) -> bool {
    for op in &ops[at + 1..] {
        if matches!(
            op,
            Op::I(Inst::BranchCond { .. } | Inst::CondSelect { .. } | Inst::CondCompare { .. })
        ) {
            return false;
        }
        if crate::flagcse::writes_flags(op) {
            return true;
        }
    }
    true
}

/// Equal pure computations of a dominating block are used instead of repeated.
fn cse(function: &mut MFunc) {
    let cfg = cfg(function);
    let definitions = definitions(function);
    // The table, scoped by the dominator tree: a block sees what its
    // dominators computed.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); function.blocks.len()];
    for &block in cfg.order.iter().skip(1) {
        children[cfg.idom[block]].push(block);
    }
    let mut rename: HashMap<Reg, Reg> = HashMap::new();
    let mut removed: HashSet<(usize, usize)> = HashSet::new();
    let mut table: HashMap<String, Reg> = HashMap::new();
    let copies = single_copies(function, &definitions);
    let mut stack: Vec<(usize, usize, Vec<String>)> = vec![(0, 0, Vec::new())];
    // Depth-first over the tree, undoing a block's entries when it is left.
    while let Some((block, next, added)) = stack.last_mut() {
        if *next == 0 {
            let mut entries = Vec::new();
            for (index, op) in function.blocks[*block].ops.iter().enumerate() {
                let setter = flag_setting(op);
                if !(pure(op) || setter) {
                    continue;
                }
                let Some(register) = defined(op) else { continue };
                if definitions.get(&register) != Some(&1) {
                    continue;
                }
                let key = keyed(&renamed(op, &rename, &copies));
                match table.get(&key) {
                    // A flag-setting instruction is shared where its own flags
                    // are dead: the earlier one still sets them
                    // (`MachineCSE`, `PhysRegDefsReach`).
                    Some(&earlier) if !setter || flags_dead(&function.blocks[*block].ops, index) => {
                        rename.insert(register, earlier);
                        removed.insert((*block, index));
                    }
                    Some(_) => {}
                    None => {
                        table.insert(key.clone(), register);
                        entries.push(key);
                    }
                }
            }
            *added = entries;
        }
        if *next < children[*block].len() {
            let child = children[*block][*next];
            *next += 1;
            stack.push((child, 0, Vec::new()));
        } else {
            for key in added.iter() {
                table.remove(key);
            }
            stack.pop();
        }
    }
    if removed.is_empty() {
        return;
    }
    for (index, block) in function.blocks.iter_mut().enumerate() {
        let mut at = 0;
        block
            .ops
            .retain(
                |_| {
                    let keep = !removed.contains(&(index, at));
                    at += 1;
                    keep
                },
            );
        for op in &mut block.ops {
            op.visit(&mut |slot, _| {
                let key = slot.key();
                if let Some(found) = rename.get(&key) {
                    match (slot, found) {
                        (RegRef::Gpr(slot), Reg::Gpr(new)) => *slot = *new,
                        (RegRef::Fpr(slot), Reg::Fpr(new)) => *slot = *new,
                        _ => {}
                    }
                }
            });
        }
    }
}

/// The loop's invariant pure definitions, taken out of its `blocks` in the
/// order they can go (each after what it reads); with `only`, just those
/// defining one of its registers.
fn invariants(
    function: &mut MFunc,
    blocks: &[usize],
    definitions: &HashMap<Reg, usize>,
    mut inside_defs: HashSet<Reg>,
    only: Option<&HashSet<Reg>>,
) -> Vec<Op> {
    let mut hoisted: Vec<Op> = Vec::new();
    let mut changed = true;
    while changed {
        changed = false;
        for &block in blocks {
            let ops = std::mem::take(&mut function.blocks[block].ops);
            let mut kept = crate::mfunc::OpList::default();
            for (op, loc) in ops {
                let invariant = pure(&op)
                    && defined(&op).is_some_and(|register| {
                        definitions.get(&register) == Some(&1) && only.is_none_or(|chosen| chosen.contains(&register))
                    })
                    && op
                        .defs_and_uses()
                        .1
                        .iter()
                        .all(|used| !inside_defs.contains(used) || !used.is_virtual() && false)
                    && op.defs_and_uses().1.iter().all(|used| used.is_virtual());
                if invariant {
                    inside_defs.remove(&defined(&op).expect("a definition"));
                    hoisted.push(op);
                    changed = true;
                } else {
                    kept.push_at(op, loc);
                }
            }
            function.blocks[block].ops = kept;
        }
    }
    hoisted
}

/// The registers among `candidates` (what `probe`, the loop without them,
/// would have hoisted) worth a register for the whole loop: those the loop
/// reads most, as many as the registers not already held by what the loop
/// reads from outside, plus what they are made from.
/// The virtual registers live into each block.
fn live_in(
    function: &MFunc,
    cfg: &Cfg,
) -> Vec<HashSet<Reg>> {
    let count = function.blocks.len();
    let mut gen_kill: Vec<(HashSet<Reg>, HashSet<Reg>)> = Vec::with_capacity(count);
    for block in &function.blocks {
        let (mut gen_set, mut kill) = (HashSet::new(), HashSet::new());
        for op in &block.ops {
            let (defs, uses) = op.defs_and_uses();
            gen_set.extend(uses.into_iter().filter(|used| used.is_virtual() && !kill.contains(used)));
            kill.extend(defs.into_iter().filter(|def| def.is_virtual()));
        }
        gen_kill.push((gen_set, kill));
    }
    let mut live: Vec<HashSet<Reg>> = vec![HashSet::new(); count];
    let mut changed = true;
    while changed {
        changed = false;
        for &block in cfg.order.iter().rev() {
            let mut input: HashSet<Reg> =
                cfg.successors[block].iter().flat_map(|&next| live[next].iter().copied()).collect();
            input.retain(|register| !gen_kill[block].1.contains(register));
            input.extend(gen_kill[block].0.iter().copied());
            if input != live[block] {
                live[block] = input;
                changed = true;
            }
        }
    }
    live
}

/// The most virtual registers (general, floating point) live at once at a
/// point of `blocks`, leaving out `skip` (`MachineLICM`'s `RegPressure` over
/// the loop: its temporaries count, not only what it holds across trips).
fn peak_pressure(
    function: &MFunc,
    blocks: &[usize],
    cfg: &Cfg,
    live: &[HashSet<Reg>],
    skip: &HashMap<Reg, &Op>,
) -> (usize, usize) {
    let count = |set: &HashSet<Reg>| {
        let gprs = set.iter().filter(|register| matches!(register, Reg::Gpr(_))).count();
        (gprs, set.len() - gprs)
    };
    let (mut gprs, mut fprs) = (0, 0);
    for &block in blocks {
        let mut now: HashSet<Reg> = cfg.successors[block]
            .iter()
            .flat_map(|&next| live[next].iter().copied())
            .filter(|register| register.is_virtual() && !skip.contains_key(register))
            .collect();
        let (g, f) = count(&now);
        (gprs, fprs) = (gprs.max(g), fprs.max(f));
        for op in function.blocks[block].ops.iter().rev() {
            let (defs, uses) = op.defs_and_uses();
            for register in defs {
                now.remove(&register);
            }
            now.extend(uses.into_iter().filter(|register| register.is_virtual() && !skip.contains_key(register)));
            let (g, f) = count(&now);
            (gprs, fprs) = (gprs.max(g), fprs.max(f));
        }
    }
    (gprs, fprs)
}

fn within_pressure(
    probe: &MFunc,
    blocks: &[usize],
    candidates: &[Op],
    cfg: &Cfg,
    live: &[HashSet<Reg>],
) -> HashSet<Reg> {
    use crate::regalloc::pool;
    let made: HashMap<Reg, &Op> =
        candidates.iter().filter_map(|op| defined(op).map(|register| (register, op))).collect();
    let (mut reads, mut calls) = (HashMap::<Reg, usize>::new(), false);
    for op in blocks.iter().flat_map(|&block| &probe.blocks[block].ops) {
        calls |= matches!(op, Op::Call { .. });
        for used in op.defs_and_uses().1 {
            if made.contains_key(&used) {
                *reads.entry(used).or_default() += 1;
            }
        }
    }
    let (held_gprs, held_fprs) = peak_pressure(probe, blocks, cfg, live, &made);
    // Across a call only the callee-saved registers keep a value.
    let room = |gpr: bool| {
        let (kept, rest) = if gpr {
            (pool::GPR_CALLEE_SAVED.len(), pool::GPR_CALLER_SAVED.len())
        } else {
            (pool::FPR_CALLEE_SAVED.len(), pool::FPR_CALLER_SAVED.len())
        };
        (if calls { kept } else { rest }).saturating_sub(if gpr { held_gprs } else { held_fprs })
    };
    // Equal values made in different blocks are one value once hoisted (the
    // `cse` after it joins them): they cost one register, and are read by all.
    let mut groups: HashMap<String, (Vec<Reg>, usize)> = HashMap::new();
    for (register, count) in reads {
        let group = groups.entry(keyed(made[&register])).or_default();
        group.0.push(register);
        group.1 += count;
    }
    let mut roots: Vec<(Vec<Reg>, usize)> = groups.into_values().map(|(registers, count)| (registers, count)).collect();
    for (registers, _) in &mut roots {
        registers.sort();
    }
    roots.sort_by(|(a, x), (b, y)| y.cmp(x).then(a.cmp(b)));
    let mut chosen: HashSet<Reg> = HashSet::new();
    let (mut gprs, mut fprs) = (room(true), room(false));
    for (registers, _) in roots {
        let gpr = matches!(registers[0], Reg::Gpr(_));
        let left = if gpr { &mut gprs } else { &mut fprs };
        if *left == 0 {
            continue;
        }
        *left -= 1;
        // What it is made from comes along, and lives only until it is made.
        let mut work = registers;
        while let Some(register) = work.pop() {
            if chosen.insert(register) {
                work.extend(made[&register].defs_and_uses().1.into_iter().filter(|used| made.contains_key(used)));
            }
        }
    }
    chosen
}

/// The natural loops, each as its header and body: a back edge's header
/// dominates its tail.
fn natural_loops(
    function: &MFunc,
    cfg: &Cfg,
) -> Vec<(usize, HashSet<usize>)> {
    let mut loops: Vec<(usize, HashSet<usize>)> = Vec::new();
    for tail in 0..function.blocks.len() {
        for &header in &cfg.successors[tail] {
            if cfg.idom[tail] != usize::MAX && dominates(cfg, header, tail) {
                let mut body: HashSet<usize> = HashSet::from([header, tail]);
                let mut work = vec![tail];
                while let Some(block) = work.pop() {
                    if block == header {
                        continue;
                    }
                    for &predecessor in &cfg.predecessors[block] {
                        if body.insert(predecessor) {
                            work.push(predecessor);
                        }
                    }
                }
                match loops.iter_mut().find(|(known, _)| *known == header) {
                    Some((_, existing)) => existing.extend(body),
                    None => loops.push((header, body)),
                }
            }
        }
    }
    loops
}

/// Loop-invariant pure definitions move to the end of the loop's preheader.
fn hoist(function: &mut MFunc) {
    let cfg = cfg(function);
    let definitions = definitions(function);
    let live = live_in(function, &cfg);
    let mut loops = natural_loops(function, &cfg);
    // Innermost first, so a value hoisted out of an inner loop can go on out of
    // the outer one.
    loops.sort_by_key(|(_, body)| body.len());
    for (header, body) in loops {
        let outside: Vec<usize> = cfg.predecessors[header].iter().copied().filter(|one| !body.contains(one)).collect();
        let [preheader] = outside[..] else { continue };
        // What the loop defines.
        let mut inside_defs: HashSet<Reg> = HashSet::new();
        for &block in &body {
            for op in &function.blocks[block].ops {
                inside_defs.extend(op.defs_and_uses().0);
            }
        }
        let mut order: Vec<usize> = body.iter().copied().collect();
        order.sort_by_key(|block| cfg.order.iter().position(|one| one == block));
        // What the loop would hoist, on a copy, and how much of it fits.
        let mut probe = function.clone();
        let candidates = invariants(&mut probe, &order, &definitions, inside_defs.clone(), None);
        let chosen = within_pressure(&probe, &order, &candidates, &cfg, &live);
        let hoisted = invariants(function, &order, &definitions, inside_defs, Some(&chosen));
        if hoisted.is_empty() {
            continue;
        }
        // Before the preheader's branches.
        let ops = &mut function.blocks[preheader].ops;
        let at = ops
            .iter()
            .rposition(|op| {
                !matches!(
                    op,
                    Op::I(
                        Inst::Branch { link: false, .. }
                            | Inst::BranchCond { .. }
                            | Inst::CompareBranch { .. }
                            | Inst::TestBranch { .. }
                    )
                )
            })
            .map_or(0, |last| last + 1);
        for (offset, op) in hoisted.into_iter().enumerate() {
            ops.insert(at + offset, op);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mfunc::{MBlock, branch_to};

    fn movz(
        rd: u32,
        value: u16,
    ) -> Op {
        Op::I(Inst::MoveWide { op: Wide::Zero, width: Width::X, rd: Gpr(rd), imm16: value, lsl: 0 })
    }

    fn add(
        rd: u32,
        rn: u32,
        rm: u32,
    ) -> Op {
        Op::I(Inst::AddSubShifted {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: Gpr(rd),
            rn: Gpr(rn),
            rm: Gpr(rm),
            shift: Shift::Lsl,
            amount: 0,
        })
    }

    /// entry -> loop -> loop | exit, where the loop makes the constant 5000 and
    /// the entry makes it too.
    fn function() -> MFunc {
        let mut function = MFunc::new("f");
        let (a, b, c, d) = (64, 65, 66, 67);
        function.next_register = 70;
        function.blocks = vec![
            MBlock { label: "entry".into(), ops: vec![movz(a, 5000), movz(b, 0), branch_to("loop")].into() },
            MBlock {
                label: "loop".into(),
                ops: vec![
                    movz(c, 5000),
                    add(d, b, c),
                    Op::I(Inst::CompareBranch {
                        nonzero: true,
                        width: Width::X,
                        rt: Gpr(d),
                        target: Target::Label("loop".into()),
                    }),
                    branch_to("exit"),
                ]
                .into(),
            },
            MBlock { label: "exit".into(), ops: vec![Op::Return { uses: vec![], tail: None }].into() },
        ];
        function
    }

    /// The loop made 5000 again on each trip although the entry had it in a
    /// register: `executed` was 1.4x of `llc` on loops of this shape.
    #[test]
    fn a_constant_the_loop_remakes_is_the_entrys() {
        let mut function = function();
        run(&mut function);
        let movzs = |block: usize| {
            function.blocks[block].ops.iter().filter(|op| matches!(op, Op::I(Inst::MoveWide { .. }))).count()
        };
        assert_eq!((movzs(0), movzs(1)), (2, 0), "{function:#?}");
    }

    /// A value made in an outer loop was sunk into the inner loop that reads it
    /// and made on every trip of it (tile at -O1: `add x13, x11, x13, lsl #7`
    /// in the inner loop, +975 executed, 3%). `MachineSink` does not sink into
    /// a deeper cycle.
    #[test]
    fn a_value_is_not_sunk_into_a_deeper_loop() {
        let back = |rt: u32, label: &str| {
            Op::I(Inst::CompareBranch {
                nonzero: true,
                width: Width::X,
                rt: Gpr(rt),
                target: Target::Label(label.into()),
            })
        };
        let mut function = MFunc::new("f");
        function.next_register = 80;
        function.blocks = vec![
            MBlock { label: "entry".into(), ops: vec![movz(0, 0), branch_to("outer")].into() },
            // outer loop: makes a constant, runs the inner loop
            MBlock { label: "outer".into(), ops: vec![movz(71, 7), branch_to("inner")].into() },
            // inner loop reads the copy
            MBlock { label: "inner".into(), ops: vec![add(72, 71, 0), back(72, "inner"), branch_to("latch")].into() },
            MBlock { label: "latch".into(), ops: vec![back(0, "outer"), branch_to("exit")].into() },
            MBlock { label: "exit".into(), ops: vec![Op::Return { uses: vec![], tail: None }].into() },
        ];
        sink(&mut function);
        let made = |block: usize| {
            function.blocks[block].ops.iter().filter(|op| matches!(op, Op::I(Inst::MoveWide { .. }))).count()
        };
        assert_eq!((made(1), made(2)), (1, 0), "{function:#?}");
    }

    /// `sp + 0` before a dynamic allocation and after it are different values:
    /// a `stacksave` and the object were made one (the array read the saved
    /// stack pointer, and a C variable-length array's loop printed
    /// garbage).
    #[test]
    fn two_reads_of_the_stack_pointer_are_not_one_value() {
        let read = |rd: u32| {
            Op::I(Inst::AddSubImm {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rd: Gpr(rd),
                rn: Gpr::R31,
                imm12: 0,
                shift12: false,
            })
        };
        let mut function = function();
        let keep = |rd: u32| {
            Op::I(Inst::Store {
                mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(rd) },
                addr: Addr::Slot { slot: 0, offset: 0 },
                mmo: MemOperand::default(),
            })
        };
        function.blocks[0].ops =
            vec![read(64), movz(65, 1), read(66), keep(64), keep(65), keep(66), branch_to("loop")].into();
        run(&mut function);
        let reads = function.blocks[0].ops.iter().filter(|op| matches!(op, Op::I(Inst::AddSubImm { .. }))).count();
        assert_eq!(reads, 2, "{function:#?}");
    }

    /// A constant only the loop makes goes to the preheader.
    #[test]
    fn a_constant_only_the_loop_makes_is_hoisted() {
        let mut function = function();
        function.blocks[0].ops.remove(0);
        run(&mut function);
        let count = |block: usize| {
            function.blocks[block]
                .ops
                .iter()
                .filter(|op| matches!(op, Op::I(Inst::MoveWide { imm16: 5000, .. })))
                .count()
        };
        assert_eq!((count(0), count(1)), (1, 0), "{function:#?}");
    }
}

/// Argument and result copies are a parallel copy (`x0 <- v`, `x1 <- v`), and
/// any order is correct when no source is a register the run writes; the
/// order decides which of them the allocator can make a no-op. A value that
/// came in on a register (`v <- x0`) stays there when the copy into that
/// register is the run's last for it, so it goes last: `x1 <- v; x0 <- v`
/// is one move, `x0 <- v; x1 <- v` two (the register coalescer's answer, by
/// ordering).
pub fn order_copies(function: &mut MFunc) {
    let mut home: HashMap<Reg, Reg> = HashMap::new();
    for block in &function.blocks {
        for op in &block.ops {
            if let Op::Copy { dst, src } = op
                && dst.is_virtual()
                && !src.is_virtual()
            {
                home.entry(*dst).or_insert(*src);
            }
        }
    }
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.ops.len() {
            let end = block.ops[at..]
                .iter()
                .position(|op| {
                    !matches!(
                        op,
                        Op::Copy { dst, src } if !dst.is_virtual() && src.is_virtual()
                    )
                })
                .map_or(block.ops.len(), |n| at + n);
            // Stable: a copy into its source's home register moves behind the
            // others.
            block
                .ops
                .sort_by_key(
                    at..end,
                    |op| match op {
                        Op::Copy { dst, src } => home.get(src) == Some(dst),
                        _ => false,
                    },
                );
            at = end.max(at + 1);
        }
    }
}

/// A back edge's phi copies go to the end of the latch, not into a block of
/// their own: `PHIElimination::SplitPHIEdges` never splits one
/// ([PHIElimination.cpp#L827-L832](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L827-L832)).
/// `recfloat`'s loop ran `fmov; b` in an edge block each trip, the pin one
/// `fmov` before the branch.
///
/// DEVIATE PE1: the DAG path splits every critical edge as it emits, before
/// any liveness exists, and the pin decides after it. This pass is that
/// decision for back edges, with the check the pin's per-phi temporary gives
/// it (the copy's destinations are dead on the other successor). Exit: phi
/// elimination onto the shared machine function with liveness (dated 11-20),
/// then this pass goes.
fn unsplit_edges(function: &mut MFunc) {
    let is_edge = |label: &str| {
        label.rsplit_once(".e").is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    let is_branch = |op: &Op| {
        matches!(
            op,
            Op::I(Inst::Branch { .. } | Inst::BranchCond { .. } | Inst::CompareBranch { .. } | Inst::TestBranch { .. })
        )
    };
    loop {
        let graph = cfg(function);
        let live = live_in(function, &graph);
        let found = (0..function.blocks.len()).find_map(|edge| {
            let block = &function.blocks[edge];
            if !is_edge(&block.label) || graph.predecessors[edge].len() != 1 {
                return None;
            }
            let from = graph.predecessors[edge][0];
            let [Op::I(Inst::Branch { link: false, target: Target::Label(head) }), ..] =
                &block.ops.iter().rev().take(1).cloned().collect::<Vec<_>>()[..]
            else {
                return None;
            };
            let copies = &block.ops[..block.ops.len() - 1];
            if copies.is_empty() || !copies.iter().all(|op| matches!(op, Op::Copy { .. })) {
                return None;
            }
            // The other successors must not need what the copies read or write,
            // and the branches must not read what they write.
            let others: Vec<usize> = graph.successors[from].iter().copied().filter(|&one| one != edge).collect();
            if others.len() + 1 != graph.successors[from].len() || from == edge {
                return None;
            }
            let defined: Vec<Reg> = copies.iter().flat_map(|op| op.defs_and_uses().0).collect();
            // Only a back edge: `SplitPHIEdges` never splits one, and
            // `MachineSink` never breaks it for a sink. Any other edge may
            // still be one the sink lands a value on.
            let back = function
                .blocks
                .iter()
                .position(|one| one.label == *head)
                .is_some_and(|header| dominates(&graph, header, from));
            if !back {
                return None;
            }
            let touched = defined.clone();
            if others.iter().any(|&one| touched.iter().any(|reg| live[one].contains(reg))) {
                return None;
            }
            let reads_defined = function.blocks[from]
                .ops
                .iter()
                .filter(|op| is_branch(op))
                .any(|op| op.defs_and_uses().1.iter().any(|reg| defined.contains(reg)));
            if reads_defined {
                return None;
            }
            Some((edge, from, head.clone()))
        });
        let Some((edge, from, head)) = found else { return };
        let label = function.blocks[edge].label.clone();
        let mut copies = std::mem::take(&mut function.blocks[edge].ops).into_iter().collect::<Vec<_>>();
        copies.pop();
        let ops = &mut function.blocks[from].ops;
        for index in 0..ops.len() {
            if let Op::I(
                Inst::Branch { target, .. }
                | Inst::BranchCond { target, .. }
                | Inst::CompareBranch { target, .. }
                | Inst::TestBranch { target, .. },
            ) = ops.op_mut(index)
            {
                if *target == Target::Label(label.clone()) {
                    *target = Target::Label(head.clone());
                }
            }
        }
        let at = ops.iter().rposition(|op| !is_branch(op)).map_or(0, |last| last + 1);
        for (offset, copy) in copies.into_iter().enumerate() {
            ops.insert(at + offset, copy.0);
        }
        function.blocks.remove(edge);
    }
}

/// `AArch64MIPeepholeOpt::visitORR`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64MIPeepholeOpt.cpp#L290>):
/// `orr w, wzr, w` is how `zext i32 to i64` is made, and a 32-bit instruction
/// already clears the upper half of its result, so the copy goes where the
/// register it copies is the single result of such an instruction. Not
/// translated: a source that is a copy of a float register (made an `fmov`).
fn redundant_zero_extends(function: &mut MFunc) {
    let definitions = definitions(function);
    let narrow_defs: HashSet<Reg> = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| clears_upper_half(op))
        .filter_map(defined)
        .filter(|register| definitions.get(register) == Some(&1))
        .collect();
    let mut rename: HashMap<Reg, Reg> = HashMap::new();
    for block in &mut function.blocks {
        block.ops.retain(|op| {
            let Op::I(Inst::LogicShifted {
                op: Logic::Orr,
                invert: false,
                width: Width::W,
                rd,
                rn: Gpr::R31,
                rm,
                shift: Shift::Lsl,
                amount: 0,
            }) = op
            else {
                return true;
            };
            let (from, to) = (Reg::Gpr(*rd), Reg::Gpr(*rm));
            if rd.is_virtual() && rm.is_virtual() && definitions.get(&from) == Some(&1) && narrow_defs.contains(&to) {
                rename.insert(from, to);
                return false;
            }
            true
        });
    }
    if rename.is_empty() {
        return;
    }
    for block in &mut function.blocks {
        for op in &mut block.ops {
            op.visit(&mut |slot, _| {
                if let Some(Reg::Gpr(new)) = rename.get(&slot.key())
                    && let RegRef::Gpr(slot) = slot
                {
                    *slot = *new;
                }
            });
        }
    }
}

/// Whether `op` is a 32-bit instruction (or a 32-bit constant) whose result
/// register has its upper 32 bits cleared by the architecture.
fn clears_upper_half(op: &Op) -> bool {
    match op {
        Op::Imm { bits: 32, .. } => true,
        Op::I(
            Inst::AddSubImm { width: Width::W, .. }
            | Inst::AddSubShifted { width: Width::W, .. }
            | Inst::AddSubExtended { width: Width::W, .. }
            | Inst::LogicImm { width: Width::W, .. }
            | Inst::LogicShifted { width: Width::W, .. }
            | Inst::MoveWide { width: Width::W, .. }
            | Inst::BitfieldMove { width: Width::W, .. }
            | Inst::Extract { width: Width::W, .. }
            | Inst::Divide { width: Width::W, .. }
            | Inst::OneSource { width: Width::W, .. }
            | Inst::ThreeSource { width: Width::W, .. }
            | Inst::CondSelect { width: Width::W, .. },
        ) => true,
        _ => false,
    }
}
