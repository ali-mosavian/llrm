//! Spilling around [`GraphColoring`]: a register nothing fits is kept in a
//! frame slot, stored after each definition and loaded before each use into a
//! short-lived register of its own.

use llrm_support::hash::HashSet;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, GraphColoring, Target, Uncolourable, call_crossings, successors};

/// How much more a use inside a loop costs per level of nesting.
const LOOP_WEIGHT: u64 = 10;

/// The loop depth of each block: the blocks from a branch's target back to the
/// branch, for every branch that goes backwards.
fn loop_depths<C>(function: &MFunction<C>) -> Vec<u32> {
    let mut depth = vec![0u32; function.blocks.len()];
    for (at, next) in successors(function).iter().enumerate() {
        for head in next.iter().filter(|head| **head <= at) {
            depth[*head..=at].iter_mut().for_each(|d| *d += 1);
        }
    }
    depth
}

/// What spilling each register would cost: its definitions and uses, each
/// weighted by the loop it is in.
pub(crate) fn costs<C>(function: &MFunction<C>) -> Vec<u64> {
    let mut cost = vec![0u64; function.classes.len()];
    for (block, depth) in function.blocks.iter().zip(loop_depths(function)) {
        let weight = LOOP_WEIGHT.pow(depth.min(4));
        for operand in block.insts.iter().flat_map(|inst| &inst.operands) {
            if let Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) = operand {
                cost[*v as usize] += weight;
            }
        }
    }
    cost
}

/// How many instructions `mentions` has looked at: the work that made the
/// allocation of a long function cost registers times instructions. The tests
/// count it; nothing reads it otherwise.
pub static MENTION_SCANS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn mentions(
    inst: &Inst,
    v: u32,
) -> (bool, bool) {
    MENTION_SCANS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let has = |wanted: fn(&Operand) -> Option<Reg>| inst.operands.iter().any(|o| wanted(o) == Some(Reg::Virt(v)));
    (
        has(|o| if let Operand::Use(r) = o { Some(*r) } else { None }),
        has(|o| if let Operand::Def(r) = o { Some(*r) } else { None }),
    )
}

/// Keep `v` in `slot`: each instruction that reads it loads it first, each that
/// writes it stores afterwards, through a new register of `v`'s class. A copy
/// to or from `v` becomes the load or the store itself. Returns the new
/// registers.
fn spill<T: Target>(
    function: &mut MFunction<T::Class>,
    v: u32,
    slot: i32,
) -> Result<Vec<u32>, String> {
    let class = function.classes[v as usize];
    let refuse = || format!("the target cannot spill v{v} ({class:?})");
    let mut made = Vec::new();
    for at in 0..function.blocks.len() {
        let mut out = Vec::with_capacity(function.blocks[at].insts.len());
        for mut inst in std::mem::take(&mut function.blocks[at].insts) {
            let (reads, writes) = mentions(&inst, v);
            if !reads && !writes {
                out.push(inst);
                continue;
            }
            if inst.kind == Kind::Copy
                && let [Operand::Def(to), Operand::Use(from)] = inst.operands[..]
                && to != from
            {
                if from == Reg::Virt(v) {
                    out.push(T::spill_load(class, to, slot).ok_or_else(refuse)?);
                } else {
                    out.push(T::spill_store(class, from, slot).ok_or_else(refuse)?);
                }
                continue;
            }
            if reads
                && !writes
                && let Some(folded) = T::fold_spill(&inst, Reg::Virt(v), slot)
            {
                out.push(folded);
                continue;
            }
            let temporary = Reg::Virt(function.classes.len() as u32);
            function.classes.push(class);
            made.push(temporary_index(temporary));
            for operand in &mut inst.operands {
                if let Operand::Def(r) | Operand::Use(r) = operand
                    && *r == Reg::Virt(v)
                {
                    *r = temporary;
                }
            }
            if reads {
                let load = T::spill_load(class, temporary, slot).ok_or_else(refuse)?;
                // a load may set the condition codes: it goes before the
                // compare a flag reader depends on, not between them
                let at = if inst.reads_flags { flag_sequence_start(&out) } else { out.len() };
                out.insert(at, load);
            }
            out.push(inst);
            if writes {
                out.push(T::spill_store(class, temporary, slot).ok_or_else(refuse)?);
            }
        }
        function.blocks[at].insts = out;
    }
    Ok(made)
}

/// Keep each of `values` in its slot across the calls it is live across, and
/// nowhere else: stored before the call, loaded back after it, so no range of
/// it spans a call and it can take a register the call clobbers. A value
/// already stored since it was last written is not stored again.
///
/// DEVIATE (row S1): this is `RAGreedy`'s split of a range around the calls
/// that clobber every register it could take (`SplitEditor`, regmask
/// interference), done on a register that keeps its name instead of a new one
/// per piece, so both sides of a call get one register. Exit: retire when the
/// 6502 moves onto a target-generic greedy allocator.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L1216>
fn split_around_calls<T: Target>(
    function: &mut MFunction<T::Class>,
    values: &[(u32, i32)],
    crossings: &[Vec<(usize, usize)>],
) -> Result<(), String> {
    let mut at_call: std::collections::BTreeMap<(usize, usize), Vec<(u32, i32)>> = Default::default();
    for &(v, slot) in values {
        for &site in &crossings[v as usize] {
            at_call.entry(site).or_default().push((v, slot));
        }
    }
    let mut blocks: Vec<usize> = at_call.keys().map(|&(block, _)| block).collect();
    blocks.dedup();
    for block in blocks {
        let insts = std::mem::take(&mut function.blocks[block].insts);
        let mut out = Vec::with_capacity(insts.len());
        // the values whose slot holds what they hold now
        let mut clean: HashSet<u32> = HashSet::new();
        for (i, inst) in insts.into_iter().enumerate() {
            clean.retain(|&v| !mentions(&inst, v).1);
            if let Some(kept) = at_call.get(&(block, i)) {
                for &(v, slot) in kept {
                    if clean.insert(v) {
                        let class = function.classes[v as usize];
                        let store = T::spill_store(class, Reg::Virt(v), slot)
                            .ok_or_else(|| format!("the target cannot spill v{v} ({class:?})"))?;
                        out.push(store);
                    }
                }
                out.push(inst);
                for &(v, slot) in kept {
                    let class = function.classes[v as usize];
                    let load = T::spill_load(class, Reg::Virt(v), slot)
                        .ok_or_else(|| format!("the target cannot spill v{v} ({class:?})"))?;
                    out.push(load);
                }
            } else {
                out.push(inst);
            }
        }
        function.blocks[block].insts = out;
    }
    Ok(())
}

fn temporary_index(register: Reg) -> u32 {
    match register {
        Reg::Virt(v) => v,
        Reg::Phys(_) => unreachable!("a temporary is virtual"),
    }
}

/// Where the sequence that produced the flags a reader is about to consume
/// begins: the run of flag setters and copies into physical registers (the
/// accumulator loads of a multi-step compare) ending at `insts`' end.
fn flag_sequence_start(insts: &[Inst]) -> usize {
    let copies_to_physical =
        |inst: &Inst| inst.kind == Kind::Copy && matches!(inst.operands.first(), Some(Operand::Def(Reg::Phys(_))));
    let mut start = insts.len();
    while start > 0
        && !insts[start - 1].terminator
        && (insts[start - 1].sets_flags || copies_to_physical(&insts[start - 1]))
    {
        start -= 1;
    }
    start
}

/// Whether the condition codes are read at or after `at` before an instruction
/// sets them again: an instruction inserted there that sets them would change
/// what that reader sees.
fn flags_live_at(
    insts: &[Inst],
    at: usize,
) -> bool {
    for inst in &insts[at..] {
        if inst.reads_flags {
            return true;
        }
        if inst.sets_flags {
            return false;
        }
    }
    false
}

/// `isTriviallyReMaterializable`: the block and index of the one instruction
/// that defines `v` from nothing but constants and incoming-argument slots
/// (immutable), with no side effect, so it can run again wherever `v` is read.
fn remat_source<C>(
    function: &MFunction<C>,
    v: u32,
) -> Option<(usize, usize)> {
    let mut defs = function.blocks.iter().enumerate().flat_map(|(at, block)| {
        block.insts.iter().enumerate().filter(move |(_, inst)| mentions(inst, v).1).map(move |(i, _)| (at, i))
    });
    let (block, index) = defs.next()?;
    if defs.next().is_some() || function.blocks.iter().any(|b| b.phis.iter().any(|phi| phi.result == Reg::Virt(v))) {
        return None;
    }
    let inst = &function.blocks[block].insts[index];
    // a copy of a rematerializable value is one too (the coalescer would have
    // joined them): look through it to the instruction that makes the value
    if inst.kind == Kind::Copy
        && let [Operand::Def(_), Operand::Use(Reg::Virt(source))] = inst.operands[..]
        && source != v
    {
        return remat_source(function, source);
    }
    let invariant = |inst: &Inst| {
        !inst.may_load
            || inst.operands.iter().any(|o| matches!(o, Operand::Frame(slot) if function.fixed.contains_key(slot)))
    };
    let defines_only_v = inst.operands.iter().filter(|o| matches!(o, Operand::Def(_))).count() == 1;
    let reads_nothing = !inst.operands.iter().any(|o| matches!(o, Operand::Use(_)));
    (inst.kind != Kind::Copy
        && inst.rematerializable
        && !inst.call
        && !inst.terminator
        && !inst.may_store
        && !inst.side_effects
        && !inst.early_clobber
        && inst.ties.is_empty()
        && defines_only_v
        && reads_nothing
        && invariant(inst))
    .then_some((block, index))
}

/// Run `v`'s defining instruction again at each read instead of keeping it in
/// a register or a slot (`InlineSpiller::reMaterializeFor`); the definition
/// goes. `None` when `v` cannot be, or when a read sits where the condition
/// codes are live and the instruction sets them. Returns the new registers.
fn rematerialize<C: Copy>(
    function: &mut MFunction<C>,
    v: u32,
) -> Option<Vec<u32>> {
    let (def_block, def_index) = remat_source(function, v)?;
    let source = function.blocks[def_block].insts[def_index].clone();
    for (at, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            let reads = mentions(inst, v).0;
            if reads && source.sets_flags && flags_live_at(&block.insts, i) && !(at == def_block && i == def_index) {
                return None;
            }
        }
    }
    let class = function.classes[v as usize];
    let again = |destination: Reg| {
        let mut copy = source.clone();
        for operand in &mut copy.operands {
            if let Operand::Def(r) = operand {
                *r = destination;
            }
        }
        copy
    };
    let mut made = Vec::new();
    for at in 0..function.blocks.len() {
        let mut out = Vec::with_capacity(function.blocks[at].insts.len());
        for mut inst in std::mem::take(&mut function.blocks[at].insts) {
            let (reads, writes) = mentions(&inst, v);
            if writes {
                continue;
            }
            if !reads {
                out.push(inst);
                continue;
            }
            if inst.kind == Kind::Copy
                && let [Operand::Def(to), Operand::Use(from)] = inst.operands[..]
                && from == Reg::Virt(v)
                && to != from
            {
                out.push(again(to));
                continue;
            }
            let temporary = Reg::Virt(function.classes.len() as u32);
            function.classes.push(class);
            made.push(temporary_index(temporary));
            for operand in &mut inst.operands {
                if let Operand::Use(r) = operand
                    && *r == Reg::Virt(v)
                {
                    *r = temporary;
                }
            }
            out.push(again(temporary));
            out.push(inst);
        }
        function.blocks[at].insts = out;
    }
    remove_dead_defs(function);
    Some(made)
}

/// Drops the side-effect-free definitions nothing reads any more, which
/// rematerializing a copy's source leaves behind.
fn remove_dead_defs<C>(function: &mut MFunction<C>) {
    loop {
        let mut used = HashSet::default();
        for block in &function.blocks {
            for inst in &block.insts {
                for operand in &inst.operands {
                    if let Operand::Use(Reg::Virt(u)) = operand {
                        used.insert(*u);
                    }
                }
            }
            for phi in &block.phis {
                used.extend(
                    phi.incoming.iter().filter_map(|(_, r)| if let Reg::Virt(u) = r { Some(*u) } else { None }),
                );
            }
        }
        let mut removed = false;
        for block in &mut function.blocks {
            let mut at = block.insts.len();
            while at > 0 {
                at -= 1;
                let inst = &block.insts[at];
                let defs: Vec<u32> = inst
                    .operands
                    .iter()
                    .filter_map(|o| if let Operand::Def(Reg::Virt(d)) = o { Some(*d) } else { None })
                    .collect();
                let pure = !inst.call
                    && !inst.terminator
                    && !inst.may_store
                    && !inst.side_effects
                    && inst.ties.is_empty()
                    && !(inst.sets_flags && flags_live_at(&block.insts, at + 1));
                if pure && !defs.is_empty() && defs.iter().all(|d| !used.contains(d)) {
                    block.insts.remove(at);
                    removed = true;
                }
            }
        }
        if !removed {
            return;
        }
    }
}

/// For each virtual register, whether every definition and use of it is in one
/// run of instructions of one block: spilling it leaves the same run, so frees
/// nothing. One pass over the function; a test per register made the colouring
/// and the choice of victim cost registers times instructions.
pub fn minimal_vregs<C>(function: &MFunction<C>) -> Vec<bool> {
    // (block, last instruction) of the run so far
    let mut run: Vec<Option<(usize, usize)>> = vec![None; function.classes.len()];
    let mut minimal = vec![true; function.classes.len()];
    let mut seen = Vec::new();
    for (at, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            seen.clear();
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) | Operand::Def(Reg::Virt(v)) = operand
                    && !seen.contains(v)
                {
                    seen.push(*v);
                }
            }
            for &v in &seen {
                let v = v as usize;
                match run[v] {
                    None => run[v] = Some((at, i)),
                    Some((block, last)) if block == at && i == last + 1 => run[v] = Some((at, i)),
                    Some(_) => minimal[v] = false,
                }
            }
        }
    }
    minimal
}

/// The register to spill for `failed`: the cheapest that spilling can free
/// (not a spill temporary, not already as short as a range gets) of those
/// holding a register `failed` could have used and `failed` itself; failing
/// that, of the crowd. `cost` is what each costs to relieve.
fn victim<T: Target>(
    function: &MFunction<T::Class>,
    failed: &Uncolourable,
    temporaries: &HashSet<u32>,
    cost: &[u64],
) -> Option<u32> {
    let minimal = minimal_vregs(function);
    let cheapest = |from: &[u32]| {
        from.iter()
            .copied()
            .filter(|u| !temporaries.contains(u) && !minimal[*u as usize])
            .min_by_key(|u| (cost[*u as usize], std::cmp::Reverse(*u)))
    };
    let near: Vec<u32> = failed.blockers.iter().copied().chain([failed.vreg]).collect();
    cheapest(&near).or_else(|| cheapest(&failed.crowd))
}

/// What keeping each register in a slot across its calls costs instead of in
/// one all along: a store and a load at each call it is live across, weighed
/// by the loops the call is in. None where it is live across no call.
fn split_costs<C>(
    function: &MFunction<C>,
    crossings: &[Vec<(usize, usize)>],
) -> Vec<Option<u64>> {
    let depths = loop_depths(function);
    crossings
        .iter()
        .map(|sites| {
            (!sites.is_empty()).then(|| sites.iter().map(|&(block, _)| 2 * LOOP_WEIGHT.pow(depths[block].min(4))).sum())
        })
        .collect()
}

/// Whether `inst` stores to (`true`) or loads from a spill slot in `slots`: a
/// store reads its register first, a load defines it.
fn slot_access<C>(
    inst: &Inst,
    slots: &[(i32, C)],
) -> Option<(usize, bool)> {
    let slot = inst.operands.iter().find_map(|o| if let Operand::Frame(s) = o { Some(*s) } else { None })?;
    let at = slots.iter().position(|(s, _)| *s == slot)?;
    match inst.operands.iter().find(|o| matches!(o, Operand::Use(_) | Operand::Def(_)))? {
        Operand::Use(_) => Some((at, true)),
        _ => Some((at, false)),
    }
}

/// Test profile: check every slot sharing with [`verify_sharing`] and panic on
/// a wrong one.
pub static VERIFY_SLOTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

type Sites = std::collections::BTreeSet<(usize, usize)>;

/// What each read of a spill slot can see: the stores that may have written the
/// value it reads. A store of a register that a reload of another slot filled
/// stores that slot's value, wherever the two sit in the block, so it takes
/// that slot's stores. Slots shared wrongly, or a read or copy taken for
/// another, change the answer. `origin` names each instruction's place in the
/// function it came from.
fn slot_sources<C>(
    function: &MFunction<C>,
    slots: &[(i32, C)],
    origin: &[Vec<usize>],
) -> Vec<((usize, usize), Sites)> {
    use llrm_support::hash::HashMap;
    type State = HashMap<i32, Sites>;
    let next = successors(function);
    let mut entry: Vec<Option<State>> = vec![None; function.blocks.len()];
    entry[0] = Some(State::default());
    let step = |state: &mut State, b: usize, reads: &mut Vec<((usize, usize), Sites)>| {
        // the stores a register's value came from, while it holds a reloaded
        // slot
        let mut loaded: HashMap<Reg, Sites> = HashMap::default();
        for (i, inst) in function.blocks[b].insts.iter().enumerate() {
            let access = slot_access(inst, slots);
            let reload = access.is_some_and(|(_, stores)| !stores) && is_reload(inst);
            match access {
                Some((slot, true)) => {
                    let register = match inst.operands.first() {
                        Some(Operand::Use(r)) => Some(*r),
                        _ => None,
                    };
                    let sources = register
                        .and_then(|r| loaded.get(&r).cloned())
                        .unwrap_or_else(|| Sites::from([(b, origin[b][i])]));
                    state.insert(slots[slot].0, sources);
                }
                Some((slot, false)) => {
                    let held = state.get(&slots[slot].0).cloned().unwrap_or_default();
                    reads.push(((b, origin[b][i]), held.clone()));
                    if let (true, Some(Operand::Def(r))) = (reload, inst.operands.first()) {
                        loaded.insert(*r, held);
                    }
                }
                None => {}
            }
            if inst.call || inst.barrier {
                loaded.clear();
            }
            for (at, operand) in inst.operands.iter().enumerate() {
                if let Operand::Def(r) = operand
                    && !(reload && at == 0)
                {
                    loaded.remove(r);
                }
            }
        }
    };
    let mut work: Vec<usize> = vec![0];
    while let Some(b) = work.pop() {
        let mut state = entry[b].clone().unwrap_or_default();
        step(&mut state, b, &mut Vec::new());
        for &s in &next[b] {
            let merged = match &entry[s] {
                None => state.clone(),
                Some(old) => {
                    let mut merged = old.clone();
                    for (k, v) in &state {
                        merged.entry(*k).or_default().extend(v.iter().copied());
                    }
                    merged
                }
            };
            if entry[s].as_ref() != Some(&merged) {
                entry[s] = Some(merged);
                work.push(s);
            }
        }
    }
    let mut reads = Vec::new();
    for b in 0..function.blocks.len() {
        if let Some(state) = &entry[b] {
            step(&mut state.clone(), b, &mut reads);
        }
    }
    reads.sort();
    reads
}

/// Slot sharing and the removal of the copies it makes into no-ops leave every
/// read that remains seeing the stores it saw before: no two values that are
/// live at once share a cell, and a read folded into an instruction reads the
/// cell it names. `kept[b][i]` is where instruction `i` of block `b` of `after`
/// was in `before`.
pub fn verify_sharing<C>(
    before: &MFunction<C>,
    slots_before: &[(i32, C)],
    after: &MFunction<C>,
    slots_after: &[(i32, C)],
    kept: &[Vec<usize>],
) -> Result<(), String> {
    let identity: Vec<Vec<usize>> = before.blocks.iter().map(|b| (0..b.insts.len()).collect()).collect();
    let was = slot_sources(before, slots_before, &identity);
    let is = slot_sources(after, slots_after, kept);
    // a removed self-copy was a read too; every read that is left must see what
    // it saw
    let mut now = is.iter().peekable();
    for (site, one) in &was {
        if let Some((at, other)) = now.peek()
            && at == site
        {
            if one != other {
                return Err(format!(
                    "the read at block {} instruction {} saw the stores {one:?}, now {other:?}",
                    site.0, site.1
                ));
            }
            now.next();
        }
    }
    if let Some((at, _)) = now.peek() {
        return Err(format!("a read of a slot appeared at {at:?}"));
    }
    Ok(())
}

/// `StackSlotColoring` with copy coalescing: slots that are never live
/// together share one, and a copy from a slot to another that ends up the same
/// slot moves nothing, so it goes. A copy is a load into a register followed
/// at once by a store of that register.
fn share_slots<C: Copy + PartialEq>(
    function: &mut MFunction<C>,
    slots: &mut Vec<(i32, C)>,
) {
    let n = slots.len();
    if n < 2 {
        return;
    }
    let next = successors(function);
    // slots live into each block, by backward dataflow
    let mut live_in = vec![vec![false; n]; function.blocks.len()];
    let mut interferes = vec![vec![false; n]; n];
    let mut copies = Vec::new();
    for round in 0.. {
        let mut changed = false;
        for b in (0..function.blocks.len()).rev() {
            let mut live = vec![false; n];
            for s in &next[b] {
                for (x, l) in live_in[*s].iter().enumerate() {
                    live[x] |= l;
                }
            }
            let insts = &function.blocks[b].insts;
            for i in (0..insts.len()).rev() {
                match slot_access(&insts[i], slots) {
                    Some((slot, true)) => {
                        // the copy's source holds the same value: no clash
                        let loaded = |p: usize| match (slot_access(&insts[p], slots), insts[p].operands.first()) {
                            // a load only: a read of the slot folded into an
                            // instruction is not a copy source
                            (Some((from, false)), Some(Operand::Def(r))) if is_reload(&insts[p]) => Some((from, *r)),
                            _ => None,
                        };
                        let source = match (i.checked_sub(1).and_then(loaded), insts[i].operands.first()) {
                            (Some((from, r)), Some(Operand::Use(stored))) if r == *stored => {
                                if round == 0 {
                                    copies.push((from, slot));
                                }
                                Some(from)
                            }
                            _ => None,
                        };
                        for x in (0..n).filter(|x| live[*x] && *x != slot && Some(*x) != source) {
                            interferes[slot][x] = true;
                            interferes[x][slot] = true;
                        }
                        live[slot] = false;
                    }
                    Some((slot, false)) => live[slot] = true,
                    None => {}
                }
            }
            if live != live_in[b] {
                live_in[b] = live;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    copies.sort_unstable();
    copies.dedup();
    let mut rep: Vec<usize> = (0..n).collect();
    let find = |rep: &Vec<usize>, mut x: usize| {
        while rep[x] != x {
            x = rep[x];
        }
        x
    };
    for (from, to) in copies {
        let (a, b) = (find(&rep, from), find(&rep, to));
        if a == b || slots[a].1 != slots[b].1 || interferes[a][b] {
            continue;
        }
        rep[b] = a;
        for x in 0..n {
            if interferes[b][x] {
                interferes[a][x] = true;
                interferes[x][a] = true;
            }
        }
    }
    // `StackSlotColoring`: each slot that is still its own takes the first
    // lower one it never shares a moment with, so a function's spills use
    // as many cells as are live at once, not one per spill
    for b in 0..n {
        if rep[b] != b {
            continue;
        }
        let Some(a) = (0..b).find(|a| rep[*a] == *a && slots[*a].1 == slots[b].1 && !interferes[*a][b]) else {
            continue;
        };
        rep[b] = a;
        for x in 0..n {
            if interferes[b][x] {
                interferes[a][x] = true;
                interferes[x][a] = true;
            }
        }
    }
    if (0..n).all(|x| rep[x] == x) {
        return;
    }
    let verify = VERIFY_SLOTS.load(std::sync::atomic::Ordering::Relaxed).then(|| (function.clone(), slots.clone()));
    let numbers: Vec<i32> = (0..n).map(|x| slots[find(&rep, x)].0).collect();
    let originals: Vec<i32> = slots.iter().map(|(s, _)| *s).collect();
    let renumber = |slot: i32| numbers[originals.iter().position(|s| *s == slot).unwrap()];
    for block in &mut function.blocks {
        for inst in &mut block.insts {
            if slot_access(inst, slots).is_some() {
                for operand in &mut inst.operands {
                    if let Operand::Frame(s) = operand {
                        *s = renumber(*s);
                    }
                }
            }
        }
    }
    slots.retain(|(s, _)| renumber(*s) == *s);
    let kept = drop_self_copies(function, slots);
    if let Some((before, slots_before)) = verify
        && let Err(why) = verify_sharing(&before, &slots_before, function, slots, &kept)
    {
        panic!("slot sharing changed what a read sees: {why}");
    }
}

/// Whether `inst` only loads a slot into a register: `[Def(register),
/// Frame(slot), ..]`. A read of the slot folded into another instruction (`ora
/// slot`) is not one, and neither it nor the store of its result is a copy.
fn is_reload(inst: &Inst) -> bool {
    matches!(inst.operands.get(1), Some(Operand::Frame(_)))
}

/// A load of a slot followed by a store of the register back to it: the store
/// goes, and the load with it when nothing else reads the register.
/// `kept[b][i]` says where instruction `i` of block `b` was before.
fn drop_self_copies<C>(
    function: &mut MFunction<C>,
    slots: &[(i32, C)],
) -> Vec<Vec<usize>> {
    let mut mentions = llrm_support::hash::HashMap::<Reg, usize>::default();
    for inst in function.blocks.iter().flat_map(|b| &b.insts) {
        for operand in &inst.operands {
            if let Operand::Def(r) | Operand::Use(r) = operand {
                *mentions.entry(*r).or_default() += 1;
            }
        }
    }
    let frame =
        |inst: &Inst| inst.operands.iter().find_map(|o| if let Operand::Frame(s) = o { Some(*s) } else { None });
    let mut kept: Vec<Vec<usize>> = Vec::with_capacity(function.blocks.len());
    for block in &mut function.blocks {
        let mut out: Vec<Inst> = Vec::with_capacity(block.insts.len());
        let mut origin: Vec<usize> = Vec::with_capacity(block.insts.len());
        for (at, inst) in std::mem::take(&mut block.insts).into_iter().enumerate() {
            if let Some(load) = out.last()
                && slot_access(load, slots).is_some_and(|(_, stores)| !stores)
                && is_reload(load)
                && slot_access(&inst, slots).is_some_and(|(_, stores)| stores)
                && frame(load) == frame(&inst)
                && let (Some(Operand::Def(d)), Some(Operand::Use(r))) = (load.operands.first(), inst.operands.first())
                && d == r
            {
                if mentions[r] == 2 {
                    out.pop();
                    origin.pop();
                }
                continue;
            }
            out.push(inst);
            origin.push(at);
        }
        block.insts = out;
        kept.push(origin);
    }
    kept
}

/// Allocate `function`, spilling until it fits; slot numbers start at
/// `first_slot`. Returns the assignment and each slot with its register's
/// class. The function must be out of phis.
pub fn allocate_spilling<T: Target>(
    function: &mut MFunction<T::Class>,
    first_slot: i32,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    allocate_spilling_ordered::<T>(function, first_slot, false)
}

/// [`allocate_spilling`], joining the copies a loop runs on every trip first
/// where `by_frequency` (see [`GraphColoring::colour`]).
pub fn allocate_spilling_ordered<T: Target>(
    function: &mut MFunction<T::Class>,
    first_slot: i32,
    by_frequency: bool,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    allocate_spilling_splitting::<T>(function, first_slot, by_frequency, false)
}

/// [`allocate_spilling_ordered`], where `split_calls` keeps a register that is
/// live across calls in a slot across them only, where that costs less than
/// keeping it there all along (see [`split_around_calls`]).
pub fn allocate_spilling_splitting<T: Target>(
    function: &mut MFunction<T::Class>,
    first_slot: i32,
    by_frequency: bool,
    split_calls: bool,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    let mut slots = Vec::new();
    let mut temporaries = HashSet::default();
    loop {
        let failures =
            match GraphColoring::<T>::by_frequency(by_frequency).with_unspillable(&temporaries).colour(function) {
                Ok(assignment) => {
                    share_slots(function, &mut slots);
                    return Ok((assignment, slots));
                }
                Err(failures) => failures,
            };
        // A victim relieves the failures near it, so a victim per failure can
        // spill more than the function needs; one per round made the
        // cost the square of the spills. The round spills
        // at most a quarter more than the function has spilled so far: the
        // first spills are chosen one at a time, a long function's in
        // O(log) rounds.
        let cost = costs(function);
        let has_calls = split_calls && function.blocks.iter().any(|block| block.insts.iter().any(|inst| inst.call));
        let crossings = if has_calls { call_crossings::<T>(function) } else { Vec::new() };
        let split = if has_calls { split_costs(function, &crossings) } else { Vec::new() };
        let cheaper_split = |v: u32| split.get(v as usize).copied().flatten().filter(|&c| c < cost[v as usize]);
        // what relieving a register costs: splitting it where that is cheaper
        let relief: Vec<u64> = (0..cost.len() as u32).map(|v| cheaper_split(v).unwrap_or(cost[v as usize])).collect();
        let mut victims: Vec<u32> = Vec::new();
        let mut excluded = temporaries.clone();
        for failed in failures.iter().take(slots.len() / 4 + 1) {
            match victim::<T>(function, failed, &excluded, &relief) {
                Some(v) => {
                    excluded.insert(v);
                    victims.push(v);
                }
                None if victims.is_empty() => {
                    return Err(format!(
                        "no register for v{} ({:?}) and nothing to spill",
                        failed.vreg, function.classes[failed.vreg as usize]
                    ));
                }
                None => {}
            }
        }
        // The registers kept in a slot only across their calls, in one pass
        // before anything else changes the instructions' places.
        let (splitting, victims): (Vec<u32>, Vec<u32>) =
            victims.into_iter().partition(|&v| cheaper_split(v).is_some() && remat_source(function, v).is_none());
        let mut kept = Vec::new();
        for v in splitting {
            let slot = first_slot + slots.len() as i32;
            slots.push((slot, function.classes[v as usize]));
            kept.push((v, slot));
        }
        split_around_calls::<T>(function, &kept, &crossings)?;
        for v in victims {
            if let Some(made) = rematerialize(function, v) {
                temporaries.extend(made);
                continue;
            }
            let slot = first_slot + slots.len() as i32;
            slots.push((slot, function.classes[v as usize]));
            temporaries.extend(spill::<T>(function, v, slot)?);
        }
    }
}
