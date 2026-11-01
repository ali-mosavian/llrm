//! The arm64 machine function as the spike machine function the shared greedy
//! allocator reads (`llrm_spike_mf::tolir::allocate_greedy`), the allocator run
//! on it, and its result written back into the [`MFunc`].
//!
//! # How it works
//! Each [`Op`] becomes one spike instruction whose operands are its defined
//! and used registers in visit order, then an [`Operand::Imm`] holding the
//! op's number over the whole function, the key back to it (greedy moves a
//! rematerializable instruction to its users). A register that both reads and
//! writes (`movk`, a pre-indexed base) is a def and a use tied to each other.
//! The zero register and `sp` are no register operand. A virtual register `n`
//! is `n - FIRST_VIRTUAL`, whichever class; its class is the one its first
//! mention has. After allocation each original op takes the registers greedy
//! left on its spike instruction, in the same order; a spill or reload is a
//! frame-slot store or load; a callee-saved register written is recorded for
//! the frame lowering.
//!
//! `LLRM_ARM64_GREEDY=1` allocates with it; a function it refuses goes to the
//! linear scan, reported as `greedy: @fn: reason`. `LLRM_ARM64_GREEDY_SHADOW=1`
//! only runs it and reports.
//!
//! DEVIATE 1: opt-in. The linear scan stays the default. Exit: the greedy
//! output is at least as good as the linear scan's on the corpus (instructions,
//! memory ops, stack-relative ops against the pin), then it becomes the default.
//!
//! DEVIATE 2: two classes, `Gpr` (the 64-bit roots; a `w` view is the same
//! register) and `Fpr` (`v0`..`v31`, one root per register). No `GPR64sp`
//! class, no sub-register classes for `s`/`d`/`q`, no register pairs. An `Fpr`
//! spill is always a 16-byte `q` store. Exit: those classes, with `ldp`/`stp`
//! pair spills.
//!
//! DEVIATE 3: the pool is the linear scan's (`regalloc::pool`), with its three
//! scratch registers left to the frame lowering; no `reg_cost`, no CSR first
//! use cost (the shared defaults). Exit: the pin's allocation order and
//! `getCSRFirstUseCost` for AArch64.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::mfunc::{Block, Inst as SInst, Kind, MFunction, Operand, Reg as SReg};
use llrm_spike_mf::regalloc::Target;

use crate::mc::inst::{Gpr, Inst, Target as BranchTarget, Vreg};
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op, Slot};
use crate::regalloc::Allocation;

/// The register class of a virtual register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    Gpr,
    Fpr,
}

/// The arm64 register file for the spike allocator.
pub struct Arm64;

fn by_name(name: String) -> RegId {
    RegId::values().find(|&id| crate::REGISTER_INFO.name(id) == Some(name.as_str())).expect("a register of the file")
}

fn x_registers() -> &'static [RegId] {
    static IDS: OnceLock<Vec<RegId>> = OnceLock::new();
    IDS.get_or_init(|| (0..31).map(|n| by_name(format!("x{n}"))).collect())
}

fn v_registers() -> &'static [RegId] {
    static IDS: OnceLock<Vec<RegId>> = OnceLock::new();
    IDS.get_or_init(|| (0..32).map(|n| by_name(format!("v{n}"))).collect())
}

/// `x18` is the platform register, `x29` the frame pointer, `x30` the link
/// register.
fn reserved_x(number: usize) -> bool {
    matches!(number, 18 | 29 | 30)
}

impl Target for Arm64 {
    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        (one == other).then_some(one)
    }

    fn any() -> Class {
        Class::Gpr
    }

    /// The linear scan's pool, in its order: caller-saved first, then callee-saved.
    fn candidates(class: Class) -> Vec<RegId> {
        use crate::regalloc::pool;
        match class {
            Class::Gpr => pool::GPR_CALLER_SAVED.iter().chain(&pool::GPR_CALLEE_SAVED).map(|n| x_registers()[*n as usize]).collect(),
            Class::Fpr => pool::FPR_CALLER_SAVED.iter().chain(&pool::FPR_CALLEE_SAVED).map(|n| v_registers()[*n as usize]).collect(),
        }
    }

    fn view(
        root: RegId,
        _: Class,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        crate::REGISTER_INFO.root(register)
    }

    /// What the pool does not hand out: the scratch registers frame lowering uses, the
    /// platform register, the frame pointer and the link register.
    fn reserved(root: RegId) -> bool {
        use crate::regalloc::pool;
        let among = |ids: &[RegId], numbers: &[u32]| numbers.iter().any(|n| ids[*n as usize] == root);
        among(x_registers(), &pool::GPR_SCRATCH) || among(v_registers(), &pool::FPR_SCRATCH) || {
            x_registers().iter().enumerate().any(|(n, id)| *id == root && reserved_x(n))
        }
    }

    fn spill_store(
        _: Class,
        register: SReg,
        slot: i32,
    ) -> Option<SInst> {
        Some(SInst::new(Kind::Op("SPILL"), vec![Operand::Use(register), Operand::Frame(slot)]))
    }

    fn spill_load(
        _: Class,
        register: SReg,
        slot: i32,
    ) -> Option<SInst> {
        Some(SInst::new(Kind::Op("RELOAD"), vec![Operand::Def(register), Operand::Frame(slot)]))
    }

    /// The pool's caller-saved registers: a call destroys them.
    fn caller_saved() -> Vec<RegId> {
        use crate::regalloc::pool;
        let gprs = pool::GPR_CALLER_SAVED.iter().map(|n| x_registers()[*n as usize]);
        let fprs = pool::FPR_CALLER_SAVED.iter().map(|n| v_registers()[*n as usize]);
        gprs.chain(fprs).collect()
    }
}

/// The spike register of an arm64 one, or `None` for the zero register and `sp`.
fn register(
    reg: Reg,
    dense: &mut impl FnMut(Reg) -> u32,
) -> Option<SReg> {
    match reg {
        Reg::Gpr(Gpr(31)) => None,
        Reg::Gpr(Gpr(number)) if number < Gpr::FIRST_VIRTUAL => Some(SReg::Phys(x_registers()[number as usize])),
        Reg::Fpr(Vreg(number)) if number < Gpr::FIRST_VIRTUAL => Some(SReg::Phys(v_registers()[number as usize])),
        virtual_register => Some(SReg::Virt(dense(virtual_register))),
    }
}

fn is_branch(op: &Op) -> Option<(&BranchTarget, bool)> {
    match op {
        Op::I(Inst::Branch { link: false, target }) => Some((target, true)),
        Op::I(Inst::BranchCond { target, .. } | Inst::CompareBranch { target, .. } | Inst::TestBranch { target, .. }) => {
            Some((target, false))
        }
        _ => None,
    }
}

/// `function` as a spike machine function, and what each of its instructions
/// came from.
pub fn to_spike(function: &MFunc) -> Result<MFunction<Class>, String> {
    let labels: HashMap<&str, u32> = function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at as u32)).collect();
    let mut out = MFunction::<Class>::new(&function.name);
    let mut numbers: BTreeMap<u32, u32> = BTreeMap::new();
    let mut classes: Vec<Class> = Vec::new();
    let mut index = 0usize;
    for (at, block) in function.blocks.iter().enumerate() {
        let mut converted = Block { id: at as u32, ..Block::default() };
        for op in block.ops.iter() {
            index += 1;
            let index = index - 1;
            let mut operands: Vec<Operand> = Vec::new();
            let mut ties: Vec<(usize, usize)> = Vec::new();
            op.clone().visit(&mut |slot: RegRef<'_>, role: Role| {
                let key = slot.key();
                let class = if matches!(key, Reg::Fpr(_)) { Class::Fpr } else { Class::Gpr };
                let mut dense = |_: Reg| {
                    let raw = match key {
                        Reg::Gpr(Gpr(n)) | Reg::Fpr(Vreg(n)) => n,
                    };
                    *numbers.entry(raw).or_insert_with(|| {
                        classes.push(class);
                        classes.len() as u32 - 1
                    })
                };
                let Some(reg) = register(key, &mut dense) else { return };
                if role.writes() {
                    operands.push(Operand::Def(reg));
                }
                if role.reads() {
                    operands.push(Operand::Use(reg));
                    if role.writes() {
                        let last = operands.len() - 1;
                        ties.push((last - 1, last));
                    }
                }
            });
            if let Some((target, always)) = is_branch(op) {
                let BranchTarget::Label(label) = target else { return Err("a branch to a symbol".into()) };
                operands.push(Operand::Block(*labels.get(label.as_str()).ok_or("a branch to a label outside the function")?));
                operands.push(Operand::Imm(index as i64));
                let mut branch = SInst::new(Kind::Op("BRANCH"), operands);
                branch.terminator = true;
                branch.barrier = always;
                branch.reads_flags = matches!(op, Op::I(Inst::BranchCond { .. }));
                converted.insts.push(branch);
                continue;
            }
            let mut inst = match op {
                Op::Copy { .. } if operands.len() == 2 => SInst::copy(def_of(&operands[0]), use_of(&operands[1])),
                other => {
                    operands.push(Operand::Imm(index as i64));
                    SInst::new(Kind::Op(name_of(other)), operands)
                }
            };
            inst.ties = ties;
            inst.sets_flags = crate::flagcse::writes_flags(op) && !matches!(op, Op::Call { .. } | Op::Return { .. });
            inst.reads_flags = matches!(op, Op::I(Inst::CondSelect { .. } | Inst::CondCompare { .. }));
            match op {
                Op::Call { .. } => inst.call = true,
                Op::Return { .. } | Op::I(Inst::Ret { .. } | Inst::Brk { .. } | Inst::BranchReg { .. }) => {
                    inst.terminator = true;
                    inst.barrier = true;
                }
                Op::Imm { .. } | Op::FrameAddress { .. } | Op::Copy { .. } => {
                    inst.rematerializable = true;
                    inst.cheap_as_move = true;
                }
                Op::I(Inst::Load { .. } | Inst::LoadPair { .. }) => inst.may_load = true,
                Op::I(Inst::Store { .. } | Inst::StorePair { .. }) => inst.may_store = true,
                Op::I(Inst::Atomic { .. }) => {
                    inst.may_load = true;
                    inst.may_store = true;
                }
                Op::I(Inst::Svc { .. } | Inst::Dmb { .. }) => inst.side_effects = true,
                _ => {}
            }
            converted.insts.push(inst);
        }
        out.blocks.push(converted);
    }
    out.classes = classes;
    Ok(out)
}

fn def_of(operand: &Operand) -> SReg {
    match operand {
        Operand::Def(reg) => *reg,
        _ => unreachable!("a def"),
    }
}

fn use_of(operand: &Operand) -> SReg {
    match operand {
        Operand::Use(reg) => *reg,
        _ => unreachable!("a use"),
    }
}

fn name_of(op: &Op) -> &'static str {
    match op {
        Op::I(_) => "INST",
        // A copy of the zero register defines a constant.
        Op::Copy { .. } => "ZERO",
        Op::Imm { .. } => "IMM",
        Op::Call { .. } => "CALL",
        Op::Return { .. } => "RETURN",
        Op::FrameAddress { .. } => "FRAMEADDR",
    }
}

/// Runs greedy on `function` as a shadow: `Ok` with the number of spill slots it
/// would add, or the reason it refused. `function` is not changed.
pub fn shadow(function: &MFunc) -> Result<usize, String> {
    let mut spike = to_spike(function)?;
    let (_, slots) = llrm_spike_mf::tolir::allocate_greedy::<Arm64>(&mut spike, Regs(&crate::REGISTER_INFO), false)?;
    Ok(slots.len())
}

/// The arm64 register an id of the file is: `x5`, `w5` and `v5`, `d5` are 5.
fn arm64_reg(id: RegId) -> Reg {
    let name = crate::REGISTER_INFO.name(id).expect("a register of the file");
    let number = name[1..].parse::<u32>().expect("a numbered register");
    if name.starts_with('v') { Reg::Fpr(Vreg(number)) } else { Reg::Gpr(Gpr(number)) }
}

/// `function` with every register physical, allocated by greedy: its virtual registers
/// replaced, the spills and reloads greedy wrote placed as frame accesses.
pub fn allocate(function: &mut MFunc) -> Result<Allocation, String> {
    let mut spike = to_spike(function)?;
    let (assignment, slots) = llrm_spike_mf::tolir::allocate_greedy::<Arm64>(&mut spike, Regs(&crate::REGISTER_INFO), false)?;
    let mut frame: HashMap<i32, u32> = HashMap::new();
    for (slot, class) in &slots {
        let size = if *class == Class::Fpr { 16 } else { 8 };
        function.slots.push(Slot { size, align: size });
        frame.insert(*slot, function.slots.len() as u32 - 1);
    }
    let resolve = |reg: SReg| match reg {
        SReg::Virt(dense) => arm64_reg(assignment.0[dense as usize]),
        SReg::Phys(id) => arm64_reg(id),
    };
    let mut saved: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    // Greedy moves a rematerializable instruction to its users: an origin is a number over the whole function.
    let original: Vec<(Op, _)> = function.blocks.iter_mut().flat_map(|block| std::mem::take(&mut block.ops)).collect();
    for (at, block) in spike.blocks.iter().enumerate() {
        let mut ops: Vec<(Op, _)> = Vec::with_capacity(block.insts.len());
        for inst in &block.insts {
            let mut loc = ops.last().and_then(|(_, loc)| *loc);
            let op = match inst.kind {
                Kind::Copy => {
                    let [Operand::Def(dst), Operand::Use(src)] = inst.operands[..] else { return Err("a malformed copy".into()) };
                    Op::Copy { dst: resolve(dst), src: resolve(src) }
                }
                Kind::Op(kind @ ("SPILL" | "RELOAD")) => {
                    let [Operand::Def(reg) | Operand::Use(reg), Operand::Frame(slot)] = inst.operands[..] else {
                        return Err("a malformed spill".into());
                    };
                    let slot = *frame.get(&slot).ok_or("a spill to a slot greedy did not report")?;
                    crate::regalloc::spill_access(resolve(reg), slot, kind == "RELOAD", true)
                }
                Kind::Op(_) => {
                    let Some(Operand::Imm(index)) = inst.operands.last() else { return Err("an instruction without its origin".into()) };
                    let (mut op, origin) = original[*index as usize].clone();
                    loc = origin;
                    // The registers greedy gave, in the order `to_spike` listed them.
                    let mut given = inst.operands.iter().filter_map(|operand| match operand {
                        Operand::Def(reg) | Operand::Use(reg) => Some(resolve(*reg)),
                        _ => None,
                    });
                    op.visit(&mut |slot: RegRef<'_>, role: Role| {
                        if matches!(slot.key(), Reg::Gpr(Gpr(31))) {
                            return;
                        }
                        let chosen = given.next();
                        if role.reads() && role.writes() {
                            given.next();
                        }
                        match (slot, chosen) {
                            (RegRef::Gpr(slot), Some(Reg::Gpr(new))) => *slot = new,
                            (RegRef::Fpr(slot), Some(Reg::Fpr(new))) => *slot = new,
                            _ => {}
                        }
                    });
                    op
                }
            };
            ops.push((op, loc));
        }
        for (op, _) in &mut ops {
            op.visit(&mut |slot: RegRef<'_>, role: Role| {
                if !role.writes() {
                    return;
                }
                match slot.key() {
                    Reg::Gpr(Gpr(n)) if (19..=28).contains(&n) => {
                        saved[0].insert(n);
                    }
                    Reg::Fpr(Vreg(n)) if (8..=15).contains(&n) => {
                        saved[1].insert(n);
                    }
                    _ => {}
                }
            });
        }
        function.blocks[at].ops = ops.into_iter().collect();
    }
    let [gprs, fprs] = saved;
    Ok(Allocation { saved_gprs: gprs.into_iter().collect(), saved_fprs: fprs.into_iter().collect() })
}
