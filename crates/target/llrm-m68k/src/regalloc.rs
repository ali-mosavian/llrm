//! Register allocation for the m68k machine function, behind one interface.
//!
//! DEVIATE: [`LinearScan`] is a spike allocator, not the pin's greedy
//! allocator: one pass over live intervals, copy hints taken from the copy
//! graph, a value that lives across a call in a callee-saved register, no
//! spilling. Reason: the shared greedy allocator takes the x86-shaped
//! `llrm-lir` function, which m68k does not have yet. Exit: the neutral-LIR
//! task; the emitter and expander read only [`Assignment`].
//!
//! The classes are `DR8`/`DR16`/`DR32` (the views of `d0` to `d7`), `AR32`
//! (`a0` to `a5`; `a6` and `sp` are the frame and stack) and `XR32` (both).

use std::collections::HashMap;

use llrm_lir::registers::RegId;

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

/// `CSR_STD`: the registers a call leaves alone.
fn callee_saved(root: RegId) -> bool {
    ![reg::D0, reg::D1, reg::A0, reg::A1].contains(&root)
}

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

/// The linear scan allocator.
pub struct LinearScan;

#[derive(Clone, Copy, Debug)]
struct Interval {
    start: usize,
    end: usize,
}

fn conflict(
    a: Interval,
    b: Interval,
) -> bool {
    a.start < b.end && b.start < a.end
}

impl Allocate for LinearScan {
    fn allocate(
        &self,
        function: &MFunction,
    ) -> Result<Assignment, String> {
        let insts: Vec<&Inst> = function.blocks.iter().flat_map(|block| block.insts.iter()).collect();
        let count = function.classes.len();
        let mut intervals: Vec<Option<Interval>> = vec![None; count];
        let mut busy: HashMap<RegId, Vec<Interval>> = HashMap::new();
        let mut calls = Vec::new();
        let mut last_def: HashMap<RegId, usize> = HashMap::new();
        for (at, inst) in insts.iter().enumerate() {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) => {
                        let one = intervals[*v as usize].get_or_insert(Interval { start: at, end: at });
                        one.end = at;
                    }
                    _ => {}
                }
            }
            for operand in &inst.operands {
                if let Operand::Use(Reg::Phys(p)) = operand {
                    let root = REGISTER_INFO.root(*p);
                    let start = last_def.get(&root).copied().unwrap_or(0);
                    busy.entry(root).or_default().push(Interval { start, end: at });
                }
            }
            for operand in &inst.operands {
                if let Operand::Def(Reg::Phys(p)) = operand {
                    last_def.insert(REGISTER_INFO.root(*p), at);
                }
            }
            if inst.call {
                calls.push(at);
                // a call defines what it returns and clobbers the rest
                for root in [reg::D0, reg::D1, reg::A0, reg::A1] {
                    last_def.insert(root, at);
                }
            }
        }
        let hints = hints(&insts, count);
        let mut order: Vec<usize> = (0..count).filter(|v| intervals[*v].is_some()).collect();
        order.sort_by_key(|v| intervals[*v].map(|one| (one.start, *v)));
        let mut taken: HashMap<RegId, Vec<Interval>> = HashMap::new();
        let mut assigned = vec![reg::D0; count];
        for v in order {
            let one = intervals[v].expect("an interval");
            let class = function.classes[v];
            let crosses = calls.iter().any(|&call| one.start < call && call < one.end);
            let free = |root: RegId| {
                (!crosses || callee_saved(root))
                    && !busy.get(&root).is_some_and(|all| all.iter().any(|other| conflict(one, *other)))
                    && !taken.get(&root).is_some_and(|all| all.iter().any(|other| conflict(one, *other)))
            };
            let all = candidates(class);
            let chosen = hints[v]
                .filter(|root| all.contains(root) && free(*root))
                .or_else(|| all.iter().copied().find(|root| free(*root)))
                .ok_or_else(|| format!("no register for v{v} ({class:?}); spilling is not translated"))?;
            taken.entry(chosen).or_default().push(one);
            assigned[v] = view(chosen, class);
        }
        Ok(Assignment(assigned))
    }
}

/// The register each virtual register is joined to by copies, if any: copies
/// make components, and a component touching one physical register wants it.
fn hints(
    insts: &[&Inst],
    count: usize,
) -> Vec<Option<RegId>> {
    let mut parent: Vec<usize> = (0..count).collect();
    fn find(
        parent: &mut Vec<usize>,
        mut at: usize,
    ) -> usize {
        while parent[at] != at {
            parent[at] = parent[parent[at]];
            at = parent[at];
        }
        at
    }
    let mut wanted: HashMap<usize, RegId> = HashMap::new();
    let mut physical = Vec::new();
    for inst in insts.iter().filter(|inst| inst.kind == Kind::Copy) {
        match (&inst.operands[0], &inst.operands[1]) {
            (Operand::Def(Reg::Virt(a)), Operand::Use(Reg::Virt(b))) => {
                let (a, b) = (find(&mut parent, *a as usize), find(&mut parent, *b as usize));
                parent[a] = b;
            }
            (Operand::Def(Reg::Virt(v)), Operand::Use(Reg::Phys(p)))
            | (Operand::Def(Reg::Phys(p)), Operand::Use(Reg::Virt(v))) => {
                physical.push((*v as usize, REGISTER_INFO.root(*p)));
            }
            _ => {}
        }
    }
    for (v, root) in physical {
        let at = find(&mut parent, v);
        wanted.entry(at).or_insert(root);
    }
    (0..count).map(|v| wanted.get(&find(&mut parent, v)).copied()).collect()
}
