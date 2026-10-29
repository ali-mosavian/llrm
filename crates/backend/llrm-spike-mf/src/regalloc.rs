//! Phi elimination, two-address rewriting and the graph-colouring allocator,
//! over a [`Target`]'s register classes.
//!
//! [`GraphColoring`] is a spike allocator, not the pin's greedy one: precise
//! liveness, an interference graph, aggressive coalescing of copies (it never
//! joins registers that interfere), greedy colouring, no spilling. The emitter
//! and expander read only [`Assignment`].

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::marker::PhantomData;

use llrm_lir::registers::RegId;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::sink::{Jump, split_critical_edge};

/// What the allocator needs to know of a register file.
pub trait Target {
    /// A virtual register's class (`TargetRegisterClass`).
    type Class: Copy + Debug + Eq;

    /// The class both allow (`constrainRegClass`), if any.
    fn meet(
        one: Self::Class,
        other: Self::Class,
    ) -> Option<Self::Class>;
    /// The class that allows every allocatable register.
    fn any() -> Self::Class;
    /// The registers (roots) a class may use, in allocation order.
    fn candidates(class: Self::Class) -> Vec<RegId>;
    /// `root`'s view for `class` (the byte or word of a register).
    fn view(
        root: RegId,
        class: Self::Class,
    ) -> RegId;
    /// The register a (sub)register is a part of.
    fn root(register: RegId) -> RegId;
    /// A root no allocator hands out (the frame and stack pointers).
    fn reserved(root: RegId) -> bool;
    /// The roots a call leaves unspecified.
    fn caller_saved() -> Vec<RegId>;
    /// Whether two allocatable registers share storage (a pair and its
    /// bytes); a register overlaps itself.
    fn overlaps(
        one: RegId,
        other: RegId,
    ) -> bool {
        one == other
    }
    /// The class the instruction `name` names for operand `index`, where it
    /// does (`MCInstrDesc::operands`).
    fn operand_class(
        _name: &str,
        _index: usize,
    ) -> Option<Self::Class> {
        None
    }
    /// The instruction that stores `register` (of `class`) to frame `slot`;
    /// `None` when the target cannot spill.
    fn spill_store(
        _class: Self::Class,
        _register: Reg,
        _slot: i32,
    ) -> Option<Inst> {
        None
    }
    /// The instruction that loads frame `slot` into `register`.
    fn spill_load(
        _class: Self::Class,
        _register: Reg,
        _slot: i32,
    ) -> Option<Inst> {
        None
    }
}

/// No register fits `vreg`; `crowd` are the virtual registers (itself
/// included) that interfere with it or were joined to it.
#[derive(Clone, Debug)]
pub struct Uncolourable {
    pub vreg: u32,
    pub crowd: Vec<u32>,
    /// The virtual registers already coloured with a register that covers one
    /// `vreg` could have used: giving one up makes room.
    pub blockers: Vec<u32>,
}

/// The physical register of each virtual register.
#[derive(Clone, Debug, Default)]
pub struct Assignment(pub Vec<RegId>);

/// An allocator: what the rest of the backend reads.
pub trait Allocate<C> {
    fn allocate(
        &self,
        function: &MFunction<C>,
    ) -> Result<Assignment, String>;
}

/// Where a predecessor's copy of `source` for a successor's phi goes: before
/// the terminators, and before the instruction that sets the flags a
/// terminator reads, since a copy may set them too; but not above an
/// instruction that defines `source`, which must run first.
fn copy_point(
    insts: &[Inst],
    source: Reg,
) -> usize {
    let first = insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
    if !insts[first..].iter().any(|inst| inst.reads_flags) {
        return first;
    }
    let Some(setter) = insts[..first].iter().rposition(|inst| inst.sets_flags) else { return first };
    let defines_source = insts[setter..first]
        .iter()
        .any(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(reg) if *reg == source)));
    if defines_source { first } else { setter }
}

/// Where the copy for a phi goes within its block: before the last read of
/// `source` there when `point` is later, so that read is the one that ends
/// `source`'s register and the instruction may reuse it in place (the phi's
/// copy of a value a later `add` consumes is then not the last use that keeps
/// the register). Not past a flag setter its reader needs.
///
/// DEVIATE from `PHIElimination`, which copies at the end of the block.
/// Reason: with the copy last, a value carried to the next iteration stays in
/// its register to the end and the loop's rotation of values costs a move
/// more (fib). Exit: never; a choice, off where the pin's output is wanted.
fn hoisted_point(
    insts: &[Inst],
    source: Reg,
    point: usize,
) -> usize {
    let uses_source = |inst: &Inst| inst.operands.iter().any(|o| matches!(o, Operand::Use(reg) if *reg == source));
    let Some(last) = insts[..point].iter().rposition(uses_source) else { return point };
    let strands_reader = (last..point).any(|at| {
        insts[at].reads_flags
            && !insts[at].sets_flags
            && insts[..at].iter().rposition(|i| i.sets_flags).is_some_and(|setter| setter < last)
    });
    if strands_reader { point } else { last }
}

/// A forward critical edge into a phi gets a block of its own where the
/// copy for the phi would otherwise sit above the instruction that sets the
/// flags its branch reads (see [`copy_point`]): there the copy's register is
/// live across the compare, next to the register the compare reads, and the
/// two cannot share. On the edge the copy follows the compare and the branch.
///
/// DEVIATE from `PHIElimination::SplitPHIEdges`, which splits only an edge
/// whose source is live out past the phis, because the pin puts the copy after
/// the compare and saves `sr` around it. Reason: `move.w %sr` is privileged
/// from the 68010 on. Exit: never; a target choice.
pub fn split_flag_edges<T: Jump, C>(function: &mut MFunction<C>) {
    'again: loop {
        let succs = successors(function);
        for to in 0..function.blocks.len() {
            let preds: Vec<usize> = (0..function.blocks.len()).filter(|p| succs[*p].contains(&to)).collect();
            if preds.len() < 2 {
                continue;
            }
            for phi in function.blocks[to].phis.clone() {
                for (pred, source) in phi.incoming {
                    let insts = &function.blocks[pred].insts;
                    let first = insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
                    if pred >= to || succs[pred].len() < 2 || copy_point(insts, source) == first {
                        continue;
                    }
                    if split_critical_edge(function, pred, to, &T::jump).is_some() {
                        continue 'again;
                    }
                }
            }
        }
        return;
    }
}

/// `PHIElimination`: a phi becomes a copy of each incoming value into a new
/// register at the end of its predecessor, and a copy of that register into
/// the phi's at the top of its block, so parallel phis do not overwrite each
/// other. The allocator joins the registers by their copy hints.
pub fn eliminate_phis<C: Copy>(function: &mut MFunction<C>) {
    eliminate_phis_with(function, false)
}

/// [`eliminate_phis`], with each copy hoisted before the last read of its
/// source in the block where `hoist` (see [`hoisted_point`]).
pub fn eliminate_phis_with<C: Copy>(
    function: &mut MFunction<C>,
    hoist: bool,
) {
    for at in 0..function.blocks.len() {
        let mut heads = Vec::new();
        for phi in std::mem::take(&mut function.blocks[at].phis) {
            function
                .classes
                .push(
                    function.classes[match phi.result {
                        Reg::Virt(v) => v as usize,
                        Reg::Phys(_) => 0,
                    }],
                );
            let temporary = Reg::Virt(function.classes.len() as u32 - 1);
            for (pred, incoming) in phi.incoming {
                let insts = &mut function.blocks[pred].insts;
                let point = copy_point(insts, incoming);
                let point = if hoist { hoisted_point(insts, incoming, point) } else { point };
                insts.insert(point, Inst::copy(temporary, incoming));
            }
            heads.push(Inst::copy(phi.result, temporary));
        }
        function.blocks[at].insts.splice(0..0, heads);
    }
}

/// The blocks control can go to from each block: the targets its branches
/// name, and the next block unless the last instruction never falls through.
pub fn successors<C>(function: &MFunction<C>) -> Vec<Vec<usize>> {
    function
        .blocks
        .iter()
        .enumerate()
        .map(|(at, block)| {
            let mut out: Vec<usize> = block
                .insts
                .iter()
                .flat_map(|inst| &inst.operands)
                .filter_map(|operand| match operand {
                    Operand::Block(id) => function.blocks.iter().position(|other| other.id == *id),
                    _ => None,
                })
                .collect();
            let stops = block.insts.last().is_some_and(|inst| inst.barrier);
            if !stops && at + 1 < function.blocks.len() {
                out.push(at + 1);
            }
            out
        })
        .collect()
}

/// `two_address`: a tied use is copied into the register the instruction
/// defines, so it reads and writes one (`TwoAddressInstructionPass`). The copy
/// disappears when the source dies there and the allocator gives both one
/// register.
pub fn two_address<C>(function: &mut MFunction<C>) {
    for block in &mut function.blocks {
        let mut out = Vec::with_capacity(block.insts.len());
        for mut inst in std::mem::take(&mut block.insts) {
            for (def, used) in std::mem::take(&mut inst.ties) {
                let (Operand::Def(destination), Operand::Use(source)) =
                    (inst.operands[def].clone(), inst.operands[used].clone())
                else {
                    continue;
                };
                if destination == source {
                    continue;
                }
                out.push(Inst::copy(destination, source));
                inst.operands[used] = Operand::Use(destination);
            }
            out.push(inst);
        }
        block.insts = out;
    }
}

/// A copy of a reserved register into a register defined once is dropped and
/// the register read in its place, so a stack address is `(4,%sp)` and not
/// through an address register (`RegisterCoalescer::joinReservedPhysReg`).
pub fn fold_reserved_copies<T: Target>(function: &mut MFunction<T::Class>) {
    let mut defs = vec![0usize; function.classes.len()];
    for operand in function.blocks.iter().flat_map(|block| &block.insts).flat_map(|inst| &inst.operands) {
        if let Operand::Def(Reg::Virt(v)) = operand {
            defs[*v as usize] += 1;
        }
    }
    let mut replace: HashMap<u32, RegId> = HashMap::new();
    for block in &mut function.blocks {
        block
            .insts
            .retain(
                |inst| {
                    if let (Kind::Copy, [Operand::Def(Reg::Virt(v)), Operand::Use(Reg::Phys(p))]) =
                        (inst.kind, &inst.operands[..])
                        && defs[*v as usize] == 1
                        && T::reserved(T::root(*p))
                    {
                        replace.insert(*v, *p);
                        return false;
                    }
                    true
                },
            );
    }
    for operand in function.blocks.iter_mut().flat_map(|block| &mut block.insts).flat_map(|inst| &mut inst.operands) {
        if let Operand::Use(Reg::Virt(v)) = operand
            && let Some(p) = replace.get(v)
        {
            *operand = Operand::Use(Reg::Phys(*p));
        }
    }
}

/// A node of the interference graph: a virtual register, or a physical one by
/// its root.
struct Nodes {
    count: usize,
    physical: Vec<RegId>,
}

impl Nodes {
    fn of<T: Target>(
        &mut self,
        register: Reg,
    ) -> usize {
        match register {
            Reg::Virt(v) => v as usize,
            Reg::Phys(p) => {
                let root = T::root(p);
                let at = self
                    .physical
                    .iter()
                    .position(|one| *one == root)
                    .unwrap_or_else(
                        || {
                            self.physical.push(root);
                            self.physical.len() - 1
                        },
                    );
                self.count + at
            }
        }
    }
}

/// What an instruction defines and reads, as nodes; a call defines the
/// registers it clobbers.
fn effects<T: Target>(
    inst: &Inst,
    nodes: &mut Nodes,
) -> (Vec<usize>, Vec<usize>) {
    let mut defs = Vec::new();
    let mut uses = Vec::new();
    for operand in &inst.operands {
        match operand {
            Operand::Def(r) if !matches!(r, Reg::Phys(p) if T::reserved(T::root(*p))) => defs.push(nodes.of::<T>(*r)),
            Operand::Use(r) if !matches!(r, Reg::Phys(p) if T::reserved(T::root(*p))) => uses.push(nodes.of::<T>(*r)),
            _ => {}
        }
    }
    if inst.call {
        defs.extend(T::caller_saved().iter().map(|root| nodes.of::<T>(Reg::Phys(*root))));
    }
    (defs, uses)
}

/// The nodes live into and out of each block, to a fixed point.
fn liveness<C>(
    function: &MFunction<C>,
    effects_of: &[Vec<(Vec<usize>, Vec<usize>)>],
) -> (Vec<HashSet<usize>>, Vec<HashSet<usize>>) {
    let blocks = function.blocks.len();
    let mut reads = vec![HashSet::new(); blocks];
    let mut writes = vec![HashSet::new(); blocks];
    for (at, block) in effects_of.iter().enumerate() {
        for (defs, uses) in block {
            for u in uses {
                if !writes[at].contains(u) {
                    reads[at].insert(*u);
                }
            }
            writes[at].extend(defs.iter().copied());
        }
    }
    let successors = successors(function);
    let mut live_in = reads.clone();
    let mut live_out = vec![HashSet::new(); blocks];
    loop {
        let mut changed = false;
        for at in (0..blocks).rev() {
            let out: HashSet<usize> = successors[at].iter().flat_map(|next| live_in[*next].iter().copied()).collect();
            let into: HashSet<usize> =
                reads[at].iter().copied().chain(out.iter().copied().filter(|v| !writes[at].contains(v))).collect();
            changed |= out != live_out[at] || into != live_in[at];
            live_out[at] = out;
            live_in[at] = into;
        }
        if !changed {
            return (live_in, live_out);
        }
    }
}

/// The graph colouring allocator.
pub struct GraphColoring<T> {
    /// Join copies that run on every trip of a loop before the ones in a
    /// branch of it (see [`GraphColoring::colour`]).
    pub by_frequency: bool,
    marker: PhantomData<T>,
}

impl<T> Default for GraphColoring<T> {
    fn default() -> Self {
        GraphColoring { by_frequency: false, marker: PhantomData }
    }
}

impl<T: Target> Allocate<T::Class> for GraphColoring<T> {
    fn allocate(
        &self,
        function: &MFunction<T::Class>,
    ) -> Result<Assignment, String> {
        self.colour(function).map_err(|failed| {
            format!(
                "no register for v{} ({:?}); spilling is not translated",
                failed.vreg, function.classes[failed.vreg as usize]
            )
        })
    }
}

impl<T: Target> GraphColoring<T> {
    /// A colourer that joins in the order [`GraphColoring::colour`] describes
    /// where `by_frequency`.
    pub fn by_frequency(by_frequency: bool) -> Self {
        GraphColoring { by_frequency, marker: PhantomData }
    }

    /// The copies in the order the coalescer tries them. `RegisterCoalescer`
    /// goes by loop depth, deeper first, then block number
    /// (`compareMBBPriority`, RegisterCoalescer.cpp L4058). With
    /// `by_frequency`, within a depth the blocks every trip passes through
    /// (those that post-dominate the loop's header, or the entry) come before
    /// the others.
    ///
    /// DEVIATE: the second key. Reason: the pin's greedy allocator splits and
    /// hints around a join that blocks a better one; this one cannot, so a
    /// copy in one arm of a select joined first (the loaded value into the
    /// result) keeps the loop-carried copy, run every trip, from joining.
    /// Exit: the neutral-LIR task.
    fn copies_in_join_order<'a>(
        &self,
        function: &'a MFunction<T::Class>,
    ) -> Vec<&'a Inst> {
        let mut keyed: Vec<((std::cmp::Reverse<usize>, bool, usize), &Inst)> = Vec::new();
        let cfg = crate::cfg::Cfg::new(function);
        for (at, block) in function.blocks.iter().enumerate() {
            let depth = cfg.cycle_depth(at);
            let always = self.by_frequency
                && match cfg.cycle(at) {
                    Some(header) => cfg.post_dominates(at, header),
                    None => cfg.post_dominates(at, 0),
                };
            for inst in block.insts.iter().filter(|inst| inst.kind == Kind::Copy) {
                keyed.push(((std::cmp::Reverse(depth), !always, at), inst));
            }
        }
        if self.by_frequency {
            keyed.sort_by_key(|(key, _)| *key);
        }
        keyed.into_iter().map(|(_, inst)| inst).collect()
    }

    /// Colour every register, or name the first one nothing fits.
    pub fn colour(
        &self,
        function: &MFunction<T::Class>,
    ) -> Result<Assignment, Uncolourable> {
        let count = function.classes.len();
        let mut nodes = Nodes { count, physical: Vec::new() };
        let effects_of: Vec<Vec<(Vec<usize>, Vec<usize>)>> = function
            .blocks
            .iter()
            .map(|block| block.insts.iter().map(|inst| effects::<T>(inst, &mut nodes)).collect())
            .collect();
        let total = count + nodes.physical.len();
        let (_, live_out) = liveness(function, &effects_of);
        let mut edges: Vec<HashSet<usize>> = vec![HashSet::new(); total];
        let connect = |a: usize, b: usize, edges: &mut Vec<HashSet<usize>>| {
            if a != b {
                edges[a].insert(b);
                edges[b].insert(a);
            }
        };
        // A register that holds the same value as the one being defined does
        // not interfere with it (`JoinVals`): within a block, a copy gives the
        // value of its source, anything else a new one; a register not yet
        // written in the block holds its own entry value.
        let mut fresh = total as u32;
        for (at, block) in function.blocks.iter().enumerate() {
            let mut current: HashMap<usize, u32> = HashMap::new();
            let mut before = Vec::with_capacity(block.insts.len());
            let mut made: Vec<Vec<u32>> = Vec::with_capacity(block.insts.len());
            for (inst, (defs, uses)) in block.insts.iter().zip(&effects_of[at]) {
                before.push(current.clone());
                let value_of =
                    |node: usize, current: &HashMap<usize, u32>| current.get(&node).copied().unwrap_or(node as u32);
                let copied = (inst.kind == Kind::Copy).then(|| uses.first().map(|u| value_of(*u, &current))).flatten();
                let new: Vec<u32> = defs
                    .iter()
                    .map(|_| {
                        copied.unwrap_or_else(|| {
                            fresh += 1;
                            fresh
                        })
                    })
                    .collect();
                for (d, value) in defs.iter().zip(&new) {
                    current.insert(*d, *value);
                }
                made.push(new);
            }
            let mut live = live_out[at].clone();
            for (i, (defs, uses)) in effects_of[at].iter().enumerate().rev() {
                for (d, value) in defs.iter().zip(&made[i]) {
                    for l in live.iter().filter(|l| !defs.contains(l)) {
                        let held = before[i].get(l).copied().unwrap_or(*l as u32);
                        if held != *value {
                            connect(*d, *l, &mut edges);
                        }
                    }
                    for other in defs {
                        connect(*d, *other, &mut edges);
                    }
                }
                if block.insts[i].early_clobber {
                    for (d, u) in defs.iter().flat_map(|d| uses.iter().map(move |u| (d, u))) {
                        connect(*d, *u, &mut edges);
                    }
                }
                for d in defs {
                    live.remove(d);
                }
                live.extend(uses.iter().copied());
            }
        }
        let mut parent: Vec<usize> = (0..total).collect();
        fn find(
            parent: &mut [usize],
            mut at: usize,
        ) -> usize {
            while parent[at] != at {
                parent[at] = parent[parent[at]];
                at = parent[at];
            }
            at
        }
        let mut classes: Vec<T::Class> = function.classes.clone();
        classes.resize(total, T::any());
        // `RegisterCoalescer`, aggressively: copies joined in program order
        // unless the two interfere
        for inst in self.copies_in_join_order(function) {
            let (Operand::Def(a), Operand::Use(b)) = (&inst.operands[0], &inst.operands[1]) else { continue };
            if matches!(a, Reg::Phys(p) if T::reserved(T::root(*p)))
                || matches!(b, Reg::Phys(p) if T::reserved(T::root(*p)))
            {
                continue;
            }
            let (a, b) = (find(&mut parent, nodes.of::<T>(*a)), find(&mut parent, nodes.of::<T>(*b)));
            if a == b || edges[a].contains(&b) || (a >= count && b >= count) {
                continue;
            }
            let (keep, drop) = if a >= count {
                (a, b)
            } else if b >= count {
                (b, a)
            } else {
                (a, b)
            };
            let class = if keep >= count {
                let root = nodes.physical[keep - count];
                if !T::candidates(classes[drop]).contains(&root) {
                    continue;
                }
                classes[keep]
            } else {
                match T::meet(classes[keep], classes[drop]) {
                    Some(class) => class,
                    None => continue,
                }
            };
            classes[keep] = class;
            parent[drop] = keep;
            let moved: Vec<usize> = edges[drop].drain().collect();
            for n in moved {
                edges[n].remove(&drop);
                if n != keep {
                    edges[n].insert(keep);
                    edges[keep].insert(n);
                }
            }
        }
        let mut colour: HashMap<usize, RegId> = HashMap::new();
        for (at, root) in nodes.physical.iter().enumerate() {
            colour.insert(count + at, *root);
        }
        let mut assigned: Vec<Option<RegId>> = vec![None; count];
        // the most constrained class first, so a word is not left with what
        // the bytes took
        let mut order: Vec<usize> = (0..count).collect();
        let reach: Vec<usize> = (0..count)
            .map(|v| {
                let node = find(&mut parent, v);
                T::candidates(classes[node]).len()
            })
            .collect();
        order.sort_by_key(|v| reach[*v]);
        for v in order {
            let node = find(&mut parent, v);
            if node >= count {
                assigned[v] = Some(T::view(colour[&node], function.classes[v]));
                continue;
            }
            if !colour.contains_key(&node) {
                let taken: HashSet<RegId> = edges[node].iter().filter_map(|n| colour.get(n).copied()).collect();
                let Some(chosen) = T::candidates(classes[node])
                    .into_iter()
                    .find(|root| !taken.iter().any(|one| T::overlaps(*one, *root)))
                else {
                    let near: Vec<usize> = edges[node].iter().copied().chain([node]).collect();
                    let crowd = (0..count as u32).filter(|u| near.contains(&find(&mut parent, *u as usize))).collect();
                    let wanted = T::candidates(classes[node]);
                    let blockers = (0..count as u32)
                        .filter(|u| {
                            let other = find(&mut parent, *u as usize);
                            other != node
                                && edges[node].contains(&other)
                                && colour
                                    .get(&other)
                                    .is_some_and(|c| wanted.iter().any(|candidate| T::overlaps(*c, *candidate)))
                        })
                        .collect();
                    return Err(Uncolourable { vreg: v as u32, crowd, blockers });
                };
                colour.insert(node, chosen);
            }
            assigned[v] = Some(T::view(colour[&node], function.classes[v]));
        }
        Ok(Assignment(assigned.into_iter().map(|one| one.expect("every register is coloured")).collect()))
    }
}
