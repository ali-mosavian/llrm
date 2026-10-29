//! Register allocation for the m68k machine function, behind one interface.
//!
//! DEVIATE: [`GraphColoring`] is a spike allocator, not the pin's greedy
//! allocator: precise liveness, an interference graph, aggressive coalescing
//! of copies (it never joins registers that interfere), greedy colouring, no
//! spilling. Reason: the shared greedy allocator takes the x86-shaped
//! `llrm-lir` function, which m68k does not have yet. Exit: the neutral-LIR
//! task; the emitter and expander read only [`Assignment`].
//!
//! The classes are `DR8`/`DR16`/`DR32` (the views of `d0` to `d7`), `AR32`
//! (`a0` to `a5`; `a6` and `sp` are the frame and stack) and `XR32` (both).

use std::collections::{HashMap, HashSet};

use llrm_lir::registers::RegId;

use crate::dagisel::generated::instrinfo::flag;
use crate::mfunc::{Class, Inst, Kind, MFunction, Operand, Reg};
use crate::{REGISTER_INFO, reg};

/// The physical register of each virtual register.
#[derive(Clone, Debug, Default)]
pub struct Assignment(pub Vec<RegId>);

/// An allocator: what the rest of the backend reads.
pub trait Allocate {
    fn allocate(
        &self,
        function: &MFunction,
    ) -> Result<Assignment, String>;
}

const DATA: [RegId; 8] = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
const ADDRESS: [RegId; 6] = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5];

/// The registers (roots) a class may use, in allocation order.
fn candidates(class: Class) -> Vec<RegId> {
    match class {
        Class::Dr8 | Class::Dr16 | Class::Dr32 => DATA.to_vec(),
        Class::Ar32 => ADDRESS.to_vec(),
        Class::Xr32 => DATA.iter().chain(&ADDRESS).copied().collect(),
    }
}

/// `root`'s view for `class`: `bd0` for a byte class, `wd0` for a word.
fn view(
    root: RegId,
    class: Class,
) -> RegId {
    let bits = match class {
        Class::Dr8 => 8,
        Class::Dr16 => 16,
        _ => 32,
    };
    REGISTER_INFO.view(root, bits).unwrap_or(root)
}

/// Where a predecessor's copies for its successors' phis go: before the
/// terminators, and before the instruction that sets the condition codes a
/// terminator reads, since a `move` sets them too.
fn copy_point(insts: &[Inst]) -> usize {
    let first = insts.iter().rposition(|inst| !inst.is_terminator()).map_or(0, |last| last + 1);
    let reads_ccr =
        insts[first..].iter().any(|inst| inst.desc().is_some_and(|d| d.implicit_uses.contains(&flag_ccr())));
    if !reads_ccr {
        return first;
    }
    insts[..first]
        .iter()
        .rposition(|inst| inst.desc().is_some_and(|d| d.implicit_defs.contains(&flag_ccr())))
        .unwrap_or(first)
}

fn flag_ccr() -> u16 {
    crate::dagisel::generated::ids::reg::CCR
}

/// `PHIElimination`: a phi becomes a copy of each incoming value into a new
/// register at the end of its predecessor, and a copy of that register into
/// the phi's at the top of its block, so parallel phis do not overwrite each
/// other. The allocator joins the registers by their copy hints.
pub fn eliminate_phis(function: &mut MFunction) {
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
                let point = copy_point(insts);
                insts.insert(
                    point,
                    Inst {
                        kind: Kind::Copy,
                        operands: vec![Operand::Def(temporary), Operand::Use(incoming)],
                        call: false,
                        ties: vec![],
                    },
                );
            }
            heads.push(Inst {
                kind: Kind::Copy,
                operands: vec![Operand::Def(phi.result), Operand::Use(temporary)],
                call: false,
                ties: vec![],
            });
        }
        function.blocks[at].insts.splice(0..0, heads);
    }
}

/// The blocks control can go to from each block: the targets its branches
/// name, and the next block unless the last instruction never falls through.
pub fn successors(function: &MFunction) -> Vec<Vec<usize>> {
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
            let stops = block.insts.last().and_then(Inst::desc).is_some_and(|desc| desc.has(flag::BARRIER));
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
pub fn two_address(function: &mut MFunction) {
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
                out.push(Inst {
                    kind: Kind::Copy,
                    operands: vec![Operand::Def(destination), Operand::Use(source)],
                    call: false,
                    ties: vec![],
                });
                inst.operands[used] = Operand::Use(destination);
            }
            out.push(inst);
        }
        block.insts = out;
    }
}

/// The registers no allocator hands out: the frame and stack pointers.
fn reserved(root: RegId) -> bool {
    root == reg::SP || root == reg::A6
}

/// A copy of a reserved register into a register defined once is dropped and
/// the register read in its place, so a stack address is `(4,%sp)` and not
/// through an address register (`RegisterCoalescer::joinReservedPhysReg`).
pub fn fold_reserved_copies(function: &mut MFunction) {
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
                        && reserved(REGISTER_INFO.root(*p))
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

/// The registers a call leaves unspecified (`CSR_STD` keeps the rest).
const CALLER_SAVED: [RegId; 4] = [reg::D0, reg::D1, reg::A0, reg::A1];

/// A node of the interference graph: a virtual register, or a physical one by
/// its root.
struct Nodes {
    count: usize,
    physical: Vec<RegId>,
}

impl Nodes {
    fn of(
        &mut self,
        register: Reg,
    ) -> usize {
        match register {
            Reg::Virt(v) => v as usize,
            Reg::Phys(p) => {
                let root = REGISTER_INFO.root(p);
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
fn effects(
    inst: &Inst,
    nodes: &mut Nodes,
) -> (Vec<usize>, Vec<usize>) {
    let mut defs = Vec::new();
    let mut uses = Vec::new();
    for operand in &inst.operands {
        match operand {
            Operand::Def(r) if !matches!(r, Reg::Phys(p) if reserved(REGISTER_INFO.root(*p))) => {
                defs.push(nodes.of(*r))
            }
            Operand::Use(r) if !matches!(r, Reg::Phys(p) if reserved(REGISTER_INFO.root(*p))) => {
                uses.push(nodes.of(*r))
            }
            _ => {}
        }
    }
    if inst.call {
        defs.extend(CALLER_SAVED.iter().map(|root| nodes.of(Reg::Phys(*root))));
    }
    (defs, uses)
}

/// The nodes live into and out of each block, to a fixed point.
fn liveness(
    function: &MFunction,
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
pub struct GraphColoring;

impl Allocate for GraphColoring {
    fn allocate(
        &self,
        function: &MFunction,
    ) -> Result<Assignment, String> {
        let count = function.classes.len();
        let mut nodes = Nodes { count, physical: Vec::new() };
        let effects_of: Vec<Vec<(Vec<usize>, Vec<usize>)>> = function
            .blocks
            .iter()
            .map(|block| block.insts.iter().map(|inst| effects(inst, &mut nodes)).collect())
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
        let mut classes: Vec<Class> = function.classes.clone();
        classes.resize(total, Class::Xr32);
        // `RegisterCoalescer`, aggressively: copies joined in program order
        // unless the two interfere
        for inst in function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Copy) {
            let (Operand::Def(a), Operand::Use(b)) = (&inst.operands[0], &inst.operands[1]) else { continue };
            if matches!(a, Reg::Phys(p) if reserved(REGISTER_INFO.root(*p)))
                || matches!(b, Reg::Phys(p) if reserved(REGISTER_INFO.root(*p)))
            {
                continue;
            }
            let (a, b) = (find(&mut parent, nodes.of(*a)), find(&mut parent, nodes.of(*b)));
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
                if !candidates(classes[drop]).contains(&root) {
                    continue;
                }
                classes[keep]
            } else {
                match classes[keep].meet(classes[drop]) {
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
        let mut assigned = vec![reg::D0; count];
        for v in 0..count {
            let node = find(&mut parent, v);
            if node >= count {
                assigned[v] = view(colour[&node], function.classes[v]);
                continue;
            }
            if !colour.contains_key(&node) {
                let taken: HashSet<RegId> = edges[node].iter().filter_map(|n| colour.get(n).copied()).collect();
                let chosen = candidates(classes[node])
                    .into_iter()
                    .find(|root| !taken.contains(root))
                    .ok_or_else(|| format!("no register for v{v} ({:?}); spilling is not translated", classes[node]))?;
                colour.insert(node, chosen);
            }
            assigned[v] = view(colour[&node], function.classes[v]);
        }
        Ok(Assignment(assigned))
    }
}
