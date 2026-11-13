//! The function as the shared allocator sees it: an [`MFunction`] over
//! [`Class`]es, and the way back. Registers are the only thing that changes
//! between the two, so the view keeps the original [`MFunc`] and writes the
//! allocator's registers into it.

use std::sync::{Mutex, OnceLock};

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::mfunc::{Block, Inst as LirInst, Kind, MFunction, Operand, Reg as LirReg};
use llrm_spike_mf::regalloc::{Assignment, Target as AllocTarget};
use llrm_spike_mf::tolir::allocate_greedy;
use llrm_support::hash::HashMap;

use crate::mc::inst::{FpConvert, Gpr, Inst, Shift, Target, Vreg, Wide};
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op};
use crate::regalloc::{Allocation, spill_access};

/// A virtual register's class: the width of a use is the instruction's.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Class {
    Gpr,
    Fpr,
    /// A vector register read whole: a call keeps only the low 64 bits of
    /// `v8`-`v15`, so it never takes one.
    Quad,
}

/// The register file's id of machine register `n` of a class: `x0..x30` and
/// `xzr` (31, also `sp`), `v0..v31`.
pub(crate) fn id(
    class: Class,
    n: u32,
) -> RegId {
    match (class, n) {
        (Class::Gpr, 31) => crate::reg::XZR,
        (Class::Gpr, n) => RegId::from_raw(crate::reg::X0.index() as u8 + n as u8),
        (Class::Fpr | Class::Quad, n) => RegId::from_raw(crate::reg::V0.index() as u8 + n as u8),
    }
}

/// The register file's id of a physical machine register.
pub(crate) fn lir_id(register: Reg) -> RegId {
    match to_lir(register) {
        LirReg::Phys(register) => register,
        LirReg::Virt(_) => unreachable!("a virtual register after allocation"),
    }
}

/// The registers a call destroys by the convention.
pub(crate) fn caller_saved() -> Vec<RegId> {
    <Arm64 as AllocTarget>::caller_saved()
}

pub(crate) fn to_lir(register: Reg) -> LirReg {
    match register {
        Reg::Gpr(Gpr(n)) if n < Gpr::FIRST_VIRTUAL => LirReg::Phys(id(Class::Gpr, n)),
        Reg::Fpr(Vreg(n)) if n < Gpr::FIRST_VIRTUAL => LirReg::Phys(id(Class::Fpr, n)),
        Reg::Gpr(Gpr(n)) | Reg::Fpr(Vreg(n)) => LirReg::Virt(n - Gpr::FIRST_VIRTUAL),
    }
}

pub(crate) fn from_lir(
    register: LirReg,
    class: Class,
) -> Reg {
    let number = |base: RegId, id: RegId| (id.index() - base.index()) as u32;
    match (register, class) {
        (LirReg::Phys(r), _) if r == crate::reg::XZR || r == crate::reg::SP => Reg::Gpr(Gpr(31)),
        (LirReg::Phys(r), _) if r.index() >= crate::reg::V0.index() => Reg::Fpr(Vreg(number(crate::reg::V0, r))),
        (LirReg::Phys(r), _) => Reg::Gpr(Gpr(number(crate::reg::X0, r))),
        (LirReg::Virt(n), Class::Gpr) => Reg::Gpr(Gpr(n + Gpr::FIRST_VIRTUAL)),
        (LirReg::Virt(n), Class::Fpr | Class::Quad) => Reg::Fpr(Vreg(n + Gpr::FIRST_VIRTUAL)),
    }
}

/// The instruction's name for the allocator: its variant (`AddSubImm`).
pub fn name_of(inst: &Inst) -> &'static str {
    // The variant is told from its debug text once; every later instruction of
    // it is found by discriminant.
    static NAMES: OnceLock<Mutex<HashMap<std::mem::Discriminant<Inst>, &'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap();
    names
        .entry(std::mem::discriminant(inst))
        .or_insert_with(
            || {
                let debug = format!("{inst:?}");
                Box::leak(debug.split([' ', '(', '{']).next().unwrap_or_default().to_owned().into_boxed_str())
            },
        )
}

fn label_of(inst: &Inst) -> Option<&str> {
    match inst {
        Inst::Branch { link: false, target: Target::Label(label) }
        | Inst::BranchCond { target: Target::Label(label), .. }
        | Inst::CompareBranch { target: Target::Label(label), .. }
        | Inst::TestBranch { target: Target::Label(label), .. } => Some(label),
        _ => None,
    }
}

/// What an operation is apart from its registers, as a number: the allocator
/// sees the instruction by name and this, and `rebuild` finds the operation
/// again by it.
/// `mov xd, xzr`: the register-31 source of a copy is the zero register.
/// The name of the definition of zero (`mov xd, xzr`).
const ZERO: &str = "Zero";

/// `isReMaterializable && isAsCheapAsAMove` for an `ADD`/`SUB`: the pin flags
/// every form of `AddSub` (AArch64InstrFormats.td#L3098), and
/// `isAsCheapAsAMove` keeps the immediate and the unshifted register forms
/// (`SUBWrs`/`ADDWrs` only with `hasALULSLFast`, the extended forms not). A
/// flag-setting form has an `$nzcv` def and is not.
fn cheap_alu(op: &Op) -> bool {
    match op {
        Op::I(Inst::AddSubImm { op, .. }) => !op.set_flags,
        Op::I(Inst::AddSubShifted { op, shift: Shift::Lsl, amount: 0, .. }) => !op.set_flags,
        _ => false,
    }
}
/// A constant one instruction makes (`movz`, `movn` or a logical immediate):
/// the size of the copy it replaces.
const ONE_INSN_IMM: &str = "Imm1";

/// `fmov s, w`: the pin's cross-class `COPY`, which early `MachineLICM`
/// hoists with the constant it reads.
pub(crate) const FMOV_TO_FPR: &str = "FmovToFpr";

fn is_zero_copy(op: &Op) -> bool {
    matches!(op, Op::Copy { dst: Reg::Gpr(_), src: Reg::Gpr(Gpr(31)) })
}

pub fn content(op: &Op) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut blank = op.clone();
    blank.visit(&mut |register, _| match register {
        RegRef::Gpr(register) if register.is_virtual() => *register = Gpr(Gpr::FIRST_VIRTUAL),
        RegRef::Fpr(register) if register.is_virtual() => *register = Vreg(Gpr::FIRST_VIRTUAL),
        _ => {}
    });
    let mut hasher = rustc_hash::FxHasher::default();
    blank.hash(&mut hasher);
    hasher.finish() as i64
}

/// The `adrp` and `add :lo12:` the pin keeps as one `MOVaddr` until after
/// allocation (`AArch64ExpandPseudo`): a page register defined once, read by
/// one `add` of the same symbol in the same block. By block: the index of the
/// `add` and of its `adrp`. As one instruction the allocator can make the
/// address again where it is needed instead of spilling it.
thread_local! {
    static PAIR_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has looked for fused address pairs.
pub fn address_pair_runs() -> usize {
    PAIR_RUNS.with(std::cell::Cell::get)
}

fn address_pairs(function: &MFunc) -> Vec<HashMap<usize, usize>> {
    PAIR_RUNS.with(|runs| runs.set(runs.get() + 1));
    let mut seen: HashMap<Reg, (u32, u32)> = HashMap::default();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        let mut op = op.clone();
        op.visit(&mut |register, role| {
            let count = seen.entry(register.key()).or_default();
            match role {
                Role::Def => count.0 += 1,
                Role::Use => count.1 += 1,
                Role::UseDef => {
                    count.0 += 1;
                    count.1 += 1;
                }
            }
        });
    }
    function
        .blocks
        .iter()
        .map(|block| {
            let mut pages: HashMap<Reg, (usize, &Target)> = HashMap::default();
            let mut pairs = HashMap::default();
            for (at, op) in block.ops.iter().enumerate() {
                match op {
                    Op::I(Inst::Adr { page: true, rd, target, got: false })
                        if seen.get(&Reg::Gpr(*rd)) == Some(&(1, 1)) =>
                    {
                        pages.insert(Reg::Gpr(*rd), (at, target));
                    }
                    Op::I(Inst::AddPageOffset { rn, target, .. }) => {
                        if let Some(&(adrp, page_target)) = pages.get(&Reg::Gpr(*rn))
                            && page_target == target
                        {
                            pairs.insert(at, adrp);
                        }
                    }
                    _ => {}
                }
            }
            pairs
        })
        .collect()
}

/// The allocator's function for `function`, registers of both classes numbered
/// from zero.
pub fn view(function: &MFunc) -> MFunction<Class> {
    view_with(function, &address_pairs(function))
}

/// [`view`] with the address pairs of `function` already found.
fn view_with(
    function: &MFunc,
    pairs: &[HashMap<usize, usize>],
) -> MFunction<Class> {
    let mut lir = MFunction::<Class>::new(&function.name);
    lir.odds = function.alloc_odds.clone();
    lir.classes = vec![Class::Gpr; (function.next_register - Gpr::FIRST_VIRTUAL) as usize];
    lir.locals =
        function.slots.iter().enumerate().map(|(at, slot)| (at as i32, slot.size as i64, slot.align as i64)).collect();
    let wide = crate::regalloc::wide_registers(function);
    let wide_list: Vec<Reg> = wide.iter().copied().collect();
    let labels: HashMap<&str, u32> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at as u32)).collect();
    let mut origin = 0u32;
    // The names the memory operands of this function refer to, by index.
    let mut symbols: Vec<String> = Vec::new();
    for (at, block) in function.blocks.iter().enumerate() {
        let mut insts = Vec::new();
        let first_halves: Vec<usize> = pairs[at].values().copied().collect();
        for (index, op) in block.ops.iter().enumerate() {
            origin += 1;
            // A debug note is no instruction to the allocator (`isDebugInstr`):
            // `rebuild` puts it back after the operation that made its value.
            if op.is_debug() || first_halves.contains(&index) {
                continue;
            }
            let fused = pairs[at].contains_key(&index);
            let zero = is_zero_copy(op);
            let (kind, terminator, barrier, call) = match op {
                // A copy of the zero register is a move of 0: made again where
                // it is read (`reMaterializeTrivialDef`), not copied from one
                // value.
                Op::Copy { .. } if zero => (Kind::Op(ZERO), false, false, false),
                Op::I(Inst::FpInt { op: FpConvert::Bits { to_fp: true }, .. }) => {
                    (Kind::Op(FMOV_TO_FPR), false, false, false)
                }
                Op::I(inst) => (
                    Kind::Op(name_of(inst)),
                    matches!(
                        inst,
                        Inst::Branch { link: false, .. }
                            | Inst::BranchCond { .. }
                            | Inst::CompareBranch { .. }
                            | Inst::TestBranch { .. }
                            | Inst::BranchReg { link: false, .. }
                            | Inst::Ret { .. }
                            | Inst::Brk { .. }
                    ),
                    matches!(
                        inst,
                        Inst::Branch { link: false, .. }
                            | Inst::BranchReg { link: false, .. }
                            | Inst::Ret { .. }
                            | Inst::Brk { .. }
                    ),
                    matches!(
                        inst,
                        Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. }
                    ),
                ),
                Op::Copy { .. } => (Kind::Copy, false, false, false),
                Op::Imm { bits, value, dst } => (
                    Kind::Op(if crate::expand_imm::mov_imm(*value, *bits, *dst).len() == 1 {
                        ONE_INSN_IMM
                    } else {
                        "Imm"
                    }),
                    false,
                    false,
                    false,
                ),
                Op::FrameAddress { .. } => (Kind::Op("FrameAddress"), false, false, false),
                Op::JumpTable { .. } => (Kind::Op("JumpTable"), true, true, false),
                Op::Call { .. } => (Kind::Op("Call"), false, false, true),
                Op::Return { .. } => (Kind::Op("Return"), true, true, false),
                Op::DbgValue { .. } => unreachable!("a debug note is skipped above"),
                Op::Phi { .. } => unreachable!("phis are copies before allocation"),
            };
            let mut operands = Vec::new();
            let mut ties = Vec::new();
            let mut op = op.clone();
            let mut page_read = fused;
            op.visit(&mut |register, role| {
                let key = register.key();
                // The page register of a fused `MOVaddr` is not a value.
                if fused && role == Role::Use && std::mem::take(&mut page_read) {
                    return;
                }
                if zero && role == Role::Use {
                    return;
                }
                let reg = to_lir(key);
                if let LirReg::Virt(n) = reg {
                    lir.classes[n as usize] = match key {
                        Reg::Gpr(_) => Class::Gpr,
                        _ if wide.contains(&key) => Class::Quad,
                        _ => Class::Fpr,
                    };
                }
                match role {
                    Role::Def => operands.push(Operand::Def(reg)),
                    Role::Use => operands.push(Operand::Use(reg)),
                    Role::UseDef => {
                        ties.push((operands.len(), operands.len() + 1));
                        operands.push(Operand::Def(reg));
                        operands.push(Operand::Use(reg));
                    }
                }
            });
            // Register 31 is the zero register and the stack pointer by slot;
            // a write of the stack pointer is named so, for the scheduler's
            // boundary (`modifiesRegister(SP)`).
            if let Op::I(inst) = &op
                && crate::ldst_update::writes_sp(inst)
            {
                for operand in &mut operands {
                    if matches!(
                        operand,
                        Operand::Def(LirReg::Phys(register)) if *register == crate::reg::XZR
                    ) {
                        *operand = Operand::Def(LirReg::Phys(crate::reg::SP));
                    }
                }
            }
            if let Op::I(inst) = &op
                && let Some(target) = label_of(inst).and_then(|label| labels.get(label))
            {
                operands.push(Operand::Block(*target));
            }
            if let Op::JumpTable { targets, .. } = &op {
                operands
                    .extend(targets.iter().filter_map(|label| labels.get(label.as_str())).map(|&b| Operand::Block(b)));
            }
            if kind != Kind::Copy {
                operands.push(Operand::Imm(if fused { content(&op).wrapping_add(1) } else { content(&op) }));
            }
            let mut inst = LirInst::new(kind, operands);
            if !fused && !zero && matches!(op, Op::I(_) | Op::Copy { .. } | Op::Imm { .. }) {
                inst.form = crate::lir_body::form_of(function, &mut symbols, &wide_list, &op).map(std::sync::Arc::new);
                inst.form_operands = inst.operands.clone();
            }
            inst.ties = ties;
            inst.origin = origin;
            inst.terminator = terminator;
            inst.barrier = barrier;
            inst.call = call;
            if let Op::Call { callee: crate::mfunc::Callee::Symbol(name), .. } = &op {
                inst.call_clobbers = crate::ipra::known(name);
            }
            let facts = crate::facts::of(&op);
            inst.sets_flags = facts.sets_flags;
            inst.reads_flags = facts.reads_flags;
            inst.may_load = facts.may_load;
            inst.may_store = facts.may_store;
            inst.side_effects = facts.side_effects;
            inst.early_clobber = facts.early_clobber;
            inst.rematerializable = fused
                || zero
                || cheap_alu(&op)
                || matches!(
                    op,
                    Op::Imm { .. }
                        | Op::I(Inst::FpMoveImm { .. } | Inst::Adr { .. })
                        | Op::I(Inst::MoveWide { op: Wide::Zero | Wide::Not, .. })
                );
            inst.invariant_load = matches!(
                &op,
                Op::I(Inst::Load { mmo, .. }) if mmo.object == crate::mc::inst::MemObject::ConstantPool
            );
            if let Op::I(access @ (Inst::Load { mem, mmo, .. } | Inst::Store { mem, mmo, .. })) = &op {
                inst.memory =
                    Some(crate::lir_body::memref_of(mem, mmo, matches!(access, Inst::Store { .. }), &mut symbols));
            }
            inst.cheap_as_move = zero || cheap_alu(&op) || matches!(op, Op::Imm { .. } | Op::I(Inst::MoveWide { .. }));
            insts.push(inst);
        }
        lir.blocks.push(Block { id: at as u32, insts, phis: Vec::new() });
    }
    lir
}

/// `function`, its registers the physical ones greedy chose in `lir`
/// (`assignment` by virtual number) and the spills it wrote: each operation is
/// the original one found by its [`content`], the spills and copies new.
static NOTES_SEEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static NOTES_PLACED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The debug notes `rebuild` was given and the ones it put back after the
/// operation that made their value, so far in the process: the rest said
/// nothing, and a test that finds locations must also see how many were lost.
pub fn note_counts() -> (u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (NOTES_SEEN.load(Relaxed), NOTES_PLACED.load(Relaxed))
}

pub fn rebuild(
    function: &mut MFunc,
    lir: &MFunction<Class>,
    assignment: &Assignment,
    spill_slots: &[(i32, Class)],
) {
    let pairs = address_pairs(function);
    rebuild_with(function, lir, &pairs, assignment, spill_slots);
}

/// [`rebuild`] with the address pairs `view` used: `function` has not changed
/// since.
fn rebuild_with(
    function: &mut MFunc,
    lir: &MFunction<Class>,
    pairs: &[HashMap<usize, usize>],
    assignment: &Assignment,
    spill_slots: &[(i32, Class)],
) {
    let mut originals: HashMap<i64, Op> = HashMap::default();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        originals.entry(content(op)).or_insert_with(|| op.clone());
    }
    // The fused `MOVaddr`s stand under the content `view` gave them.
    let mut fused: HashMap<i64, Op> = HashMap::default();
    for (block, pairs) in function.blocks.iter().zip(pairs) {
        for &add in pairs.keys() {
            fused.entry(content(&block.ops[add]).wrapping_add(1)).or_insert_with(|| block.ops[add].clone());
        }
    }
    // The debug notes, by the number `view` gave the operation that made their
    // value, with the position of that value among the operation's registers
    // (`registers` below): the note is put back after it, naming the register
    // allocation left there. A note whose value the operation before it did not
    // make says nothing.
    let mut notes: HashMap<u32, Vec<(llrm_mir::MetadataId, usize, Option<Reg>)>> = HashMap::default();
    let mut arrived: Vec<(llrm_mir::MetadataId, Reg)> = Vec::new();
    // Variables with a note the converter cannot place (a value from another
    // block, a float, nothing): the earlier notes would stay standing, saying
    // more than the code does, so such a variable says nothing at all.
    let mut lost: std::collections::HashSet<llrm_mir::MetadataId> = std::collections::HashSet::new();
    let mut number = 0u32;
    for (block_at, block) in function.blocks.iter().enumerate() {
        let is_entry = block_at == 0;
        // Where each register was last written in this block: the operation's
        // number and the register's position among its registers.
        let mut made: HashMap<Reg, (u32, usize, Option<Reg>)> = HashMap::default();
        for op in block.ops.iter() {
            number += 1;
            if op.is_debug() {
                NOTES_SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if let Op::DbgValue { variable, reg } = op {
                match reg.and_then(|reg| made.get(&reg)) {
                    Some(&(anchor, position, comes_in)) => {
                        notes.entry(anchor).or_default().push((*variable, position, comes_in));
                    }
                    None => {
                        lost.insert(*variable);
                    }
                }
                continue;
            }
            if op.is_debug() || is_zero_copy(op) {
                continue;
            }
            // A copy of an argument register in the entry block: if allocation
            // removes it, the value is in that register from the entry.
            let comes_in = match op {
                Op::Copy { dst, src: src @ Reg::Gpr(from) } if is_entry && !from.is_virtual() && *dst != *src => {
                    Some(*src)
                }
                _ => None,
            };
            let mut position = 0usize;
            op.clone().visit(&mut |register, role| {
                if role.writes() {
                    made.insert(register.key(), (number, position, comes_in));
                }
                position += if role == Role::UseDef { 2 } else { 1 };
            });
        }
    }
    // The place of each operation, by the number `view` gave it.
    let places: Vec<_> =
        function.blocks.iter().flat_map(|block| (0..block.ops.len()).map(|at| block.ops.loc(at))).collect();
    // Registers the selector made 128 bits wide, physical ones (an `f128`
    // argument or result, copied between `v0`..`v7`) included.
    let wide = crate::regalloc::wide_registers(function);
    let mut slots: HashMap<i32, u32> = HashMap::default();
    for (slot, class) in spill_slots {
        let (size, align) = if *class == Class::Quad { (16, 16) } else { (8, 8) };
        slots.insert(*slot, function.slot(size, align));
    }
    let physical = |register: LirReg| match register {
        LirReg::Virt(v) => LirReg::Phys(assignment.0[v as usize]),
        phys => phys,
    };
    let class_of = |register: LirReg| match register {
        LirReg::Virt(v) => lir.classes[v as usize],
        LirReg::Phys(id) if id.index() >= crate::reg::V0.index() => Class::Fpr,
        LirReg::Phys(_) => Class::Gpr,
    };
    // A note whose operation allocation removed (a copy of an argument register
    // the value was coalesced with) stands at the entry, naming that register.
    let present: std::collections::HashSet<u32> =
        lir.blocks.iter().flat_map(|block| block.insts.iter().map(|inst| inst.origin)).collect();
    for (anchor, list) in &notes {
        if !present.contains(anchor) {
            for &(variable, _, comes_in) in list {
                match comes_in {
                    Some(reg) => arrived.push((variable, reg)),
                    None => {
                        lost.insert(variable);
                    }
                }
            }
        }
    }
    for (block_at, (block, lir_block)) in function.blocks.iter_mut().zip(&lir.blocks).enumerate() {
        let mut ops = Vec::with_capacity(lir_block.insts.len());
        let mut locs = Vec::with_capacity(lir_block.insts.len());
        if block_at == 0 {
            for &(variable, reg) in &arrived {
                ops.push(Op::DbgValue { variable, reg: Some(reg) });
                locs.push(None);
            }
        }
        for inst in &lir_block.insts {
            let before = ops.len();
            let registers: Vec<(LirReg, Class)> = inst
                .operands
                .iter()
                .filter_map(|operand| match operand {
                    Operand::Def(reg) | Operand::Use(reg) => Some((*reg, class_of(*reg))),
                    _ => None,
                })
                .collect();
            let reg_at = |n: usize| {
                let (register, class) = registers[n];
                from_lir(physical(register), class)
            };
            match inst.kind {
                Kind::Copy => {
                    let (dst, src) = (reg_at(0), reg_at(1));
                    if dst != src {
                        // A copy of 128 bits moves all of them.
                        match (dst, src) {
                            (Reg::Fpr(rd), Reg::Fpr(rn))
                                if registers[0].1 == Class::Quad
                                    || registers[1].1 == Class::Quad
                                    || wide.contains(&dst)
                                    || wide.contains(&src) =>
                            {
                                ops.push(Op::I(Inst::VectorMov { rd, rn }));
                            }
                            _ => ops.push(Op::Copy { dst, src }),
                        }
                    }
                }
                Kind::Op(name @ ("SpillStore" | "SpillLoad")) => {
                    let Some(Operand::Frame(slot)) = inst.operands.last() else { panic!("a spill names no slot") };
                    let slot = slots[slot];
                    let quad = registers[0].1 == Class::Quad;
                    ops.push(spill_access(reg_at(0), slot, name == "SpillLoad", quad));
                }
                Kind::Op(_)
                    if matches!(
                        inst.operands.last(),
                        Some(Operand::Imm(hash)) if fused.contains_key(hash)
                    ) =>
                {
                    // `AArch64ExpandPseudo`: `adrp rd` and `add rd, rd,
                    // :lo12:`.
                    let Some(Operand::Imm(hash)) = inst.operands.last() else { unreachable!() };
                    let Op::I(Inst::AddPageOffset { target, .. }) = &fused[hash] else { unreachable!() };
                    let Reg::Gpr(rd) = reg_at(0) else { panic!("a page offset into a vector register") };
                    ops.push(Op::I(Inst::Adr { page: true, rd, target: target.clone(), got: false }));
                    ops.push(Op::I(Inst::AddPageOffset { rd, rn: rd, target: target.clone() }));
                }
                Kind::Op(_) => {
                    let Some(Operand::Imm(hash)) = inst.operands.last() else { panic!("an operation with no content") };
                    let mut op = originals[hash].clone();
                    let zero = is_zero_copy(&op);
                    let mut at = 0;
                    op.visit(&mut |register, role| {
                        if zero && role == Role::Use {
                            return;
                        }
                        let new = reg_at(at);
                        at += if role == Role::UseDef { 2 } else { 1 };
                        match (register, new) {
                            (RegRef::Gpr(slot), Reg::Gpr(new)) => *slot = new,
                            (RegRef::Fpr(slot), Reg::Fpr(new)) => *slot = new,
                            _ => panic!("a register moved between classes"),
                        }
                    });
                    ops.push(op);
                }
            }
            for &(variable, position, _) in notes.get(&inst.origin).map_or(&[][..], Vec::as_slice) {
                ops.push(Op::DbgValue { variable, reg: Some(reg_at(position)) });
            }
            for _ in before..ops.len() {
                locs.push(if inst.origin == 0 { None } else { places[inst.origin as usize - 1] });
            }
        }
        // What the allocator added stands where the operation after it does
        // (`OpList::insert`), the last one's at the end.
        let mut next = None;
        for loc in locs.iter_mut().rev() {
            match loc {
                Some(_) => next = *loc,
                None => *loc = next,
            }
        }
        let mut last = None;
        for loc in locs.iter_mut() {
            match loc {
                Some(_) => last = *loc,
                None => *loc = last,
            }
        }
        let kept: Vec<bool> =
            ops.iter().map(|op| !matches!(op, Op::DbgValue { variable, .. } if lost.contains(variable))).collect();
        let mut at = 0;
        ops.retain(|_| (kept[at], at += 1).0);
        at = 0;
        locs.retain(|_| (kept[at], at += 1).0);
        NOTES_PLACED
            .fetch_add(ops.iter().filter(|op| op.is_debug()).count() as u64, std::sync::atomic::Ordering::Relaxed);
        block.ops = ops.into_iter().zip(locs).collect();
    }
    if llrm_support::env_var("LLRM_NOTE_COUNTS").is_some() {
        let (seen, placed) = note_counts();
        eprintln!("debug notes: {seen} seen, {placed} placed so far ({})", function.name);
    }
}

/// The callee-saved registers `function` writes, for the frame to save.
pub fn written_callee_saved(function: &MFunc) -> Allocation {
    let mut allocation = Allocation::default();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        let (defs, _) = op.defs_and_uses();
        for register in defs {
            match register {
                Reg::Gpr(Gpr(n))
                    if ((19..=28).contains(&n) || (n == 29 && frame_pointer_free()))
                        && !allocation.saved_gprs.contains(&n) =>
                {
                    allocation.saved_gprs.push(n)
                }
                Reg::Fpr(Vreg(n)) if (8..=15).contains(&n) && !allocation.saved_fprs.contains(&n) => {
                    allocation.saved_fprs.push(n)
                }
                _ => {}
            }
        }
    }
    allocation.saved_gprs.sort_unstable();
    allocation.saved_fprs.sort_unstable();
    allocation
}

/// `function` with every register physical, by the shared greedy allocator.
pub fn allocate(
    function: &mut MFunc,
    size: bool,
) -> Result<Allocation, String> {
    let pairs = address_pairs(function);
    let mut lir = view_with(function, &pairs);
    crate::licm::hoist(&mut lir);
    let (assignment, slots) = allocate_greedy::<Arm64>(&mut lir, Regs(&crate::REGISTER_INFO), size)?;
    rebuild_with(function, &lir, &pairs, &assignment, &slots);
    Ok(written_callee_saved(function))
}

/// The 64-bit and vector registers the allocator hands out, in the pin's
/// order (`AArch64RegisterInfo::getCalleeSavedRegs` aside: callee-saved
/// registers come last).
pub struct Arm64;

// Per thread: a compile on another thread must not change this one's target.
thread_local! {
    static DARWIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FP_FREE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SCRATCH_FREE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Sets the target the next allocations are for.
pub fn set_darwin(darwin: bool) {
    DARWIN.with(|flag| flag.set(darwin));
}

/// Whether the target is Apple's: `x18` is the platform register there
/// (`AArch64RegisterInfo::getReservedRegs`, `isTargetDarwin`), an ordinary
/// caller-saved register on Linux.
fn darwin() -> bool {
    DARWIN.with(std::cell::Cell::get)
}

/// Sets whether the next function has no frame record, so `x29` is an ordinary
/// callee-saved register (`getReservedRegs` reserves it when `hasFP`).
pub fn set_frame_pointer_free(free: bool) {
    FP_FREE.with(|flag| flag.set(free));
}

/// Sets whether `x16` and `x17` may be allocated: the frame needs no scratch.
pub fn set_scratch_free(free: bool) {
    SCRATCH_FREE.with(|flag| flag.set(free));
}

fn scratch_free() -> bool {
    SCRATCH_FREE.with(std::cell::Cell::get)
}

fn frame_pointer_free() -> bool {
    FP_FREE.with(std::cell::Cell::get)
}

/// The caller-saved general registers the allocator hands out.
pub fn gpr_caller_saved_count() -> usize {
    15 + 1 + usize::from(!darwin())
}

fn gpr_order(
    darwin: bool,
    scratch_free: bool,
    frame_pointer_free: bool,
) -> Vec<RegId> {
    // The caller-saved registers first, then x19..x28. x16 and x17 are the
    // frame lowerer's scratch registers; x15 and, off Darwin, x18 are
    // allocatable (the pin's `getReservedRegs` reserves x18 on Darwin only).
    let mut extra: Vec<u32> = vec![15];
    if scratch_free {
        extra.extend([16, 17]);
    }
    if !darwin {
        extra.push(18);
    }
    (9..=14u32)
        .chain((0..=8).rev())
        .chain(extra)
        .chain(19..=28)
        .chain(frame_pointer_free.then_some(29))
        .map(|n| id(Class::Gpr, n))
        .collect()
}

type Flags = (Class, bool, bool, bool);

thread_local! {
    static ORDERS: std::cell::RefCell<Vec<(Flags, Vec<RegId>)>> = const { std::cell::RefCell::new(Vec::new()) };
    static ORDERS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many allocation orders this thread has built.
pub fn candidate_orders_built() -> usize {
    ORDERS_BUILT.with(std::cell::Cell::get)
}

/// The registers the allocator tries for `class`, in order. The order depends
/// on the target and the frame's needs (`darwin`, the scratch registers, the
/// frame pointer) and is asked for by every instruction converted and every
/// interval allocated, 4,274 times for shellsort's two functions: it is built
/// once per such state.
pub fn candidate_order(class: Class) -> Vec<RegId> {
    cached_order((class, darwin(), scratch_free(), frame_pointer_free()))
}

/// [`candidate_order`] for an explicit state; the cache is keyed by all of it.
fn cached_order(state: Flags) -> Vec<RegId> {
    ORDERS.with(|orders| {
        if let Some((_, order)) = orders.borrow().iter().find(|(known, _)| *known == state) {
            return order.clone();
        }
        ORDERS_BUILT.with(|built| built.set(built.get() + 1));
        let order = built_order(state);
        orders.borrow_mut().push((state, order.clone()));
        order
    })
}

/// The order itself: a function of the class and the three flags only.
fn built_order((class, darwin, scratch_free, frame_pointer_free): Flags) -> Vec<RegId> {
    match class {
        Class::Gpr => gpr_order(darwin, scratch_free, frame_pointer_free),
        Class::Fpr => fpr_order(),
        Class::Quad => fpr_order().into_iter().filter(|id| !callee_saved_fpr(*id)).collect(),
    }
}

fn callee_saved_fpr(id: RegId) -> bool {
    (8..=15).any(|n| self::id(Class::Fpr, n) == id)
}

fn fpr_order() -> Vec<RegId> {
    (16..=31u32).chain(0..=7).chain(8..=15).map(|n| id(Class::Fpr, n)).collect()
}

impl AllocTarget for Arm64 {
    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        match (one, other) {
            (one, other) if one == other => Some(one),
            (Class::Fpr, Class::Quad) | (Class::Quad, Class::Fpr) => Some(Class::Quad),
            _ => None,
        }
    }

    fn any() -> Class {
        Class::Gpr
    }

    fn schedules_before_allocation() -> bool {
        true
    }

    /// `-mcpu=generic` on Linux: the Cortex-A510 model. DEVIATE SB8: Darwin
    /// (`apple-m1`) keeps the generic model until the Apple A14 model's data
    /// (S1) lands.
    fn scheduling_model() -> std::rc::Rc<dyn llrm_codegen::backend::schedmodel::SchedulingModel> {
        if darwin() {
            std::rc::Rc::new(crate::schedmodel::DarwinModel)
        } else {
            std::rc::Rc::new(crate::schedmodel::Arm64Model::generic())
        }
    }

    /// The coalescer makes a definition again for a copy only if it is
    /// `isAsCheapAsAMove`; `MOVaddr` is not (the pair is two instructions),
    /// so a copy of an address stays a `mov`. The spiller takes it still.
    fn literal_move_wins(
        literal: &llrm_codegen::model::lir::Insn,
        _source: &[RegId],
        _destination: &[RegId],
        _size: bool,
    ) -> bool {
        literal.what.as_ref().is_none_or(|what| what.name.as_deref() != Some("AddPageOffset"))
    }

    /// Mirrors `RegisterCoalescer::reMaterializeDef`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1298>):
    /// the pin makes a trivially rematerializable def again for a copy with no
    /// size guard. The zero register's `mov xd, xzr` and a constant one
    /// `movz`/`movn`/logical immediate makes are each one four-byte
    /// instruction, the size of the copy they replace. A constant that takes
    /// several instructions stays a copy (`mul_constants` -Os grew 124 B when
    /// every constant was made again); its operand here is an identity, so
    /// the length is carried in the name.
    fn literal_move_is_copy_sized(literal: &llrm_codegen::model::lir::Insn) -> bool {
        literal
            .what
            .as_ref()
            .and_then(|what| what.name.as_deref())
            .is_some_and(|name| Self::is_zero_def(name) || name == ONE_INSN_IMM || name.starts_with("AddSub"))
    }

    fn remat_reads_registers() -> bool {
        true
    }

    fn candidates(class: Class) -> Vec<RegId> {
        candidate_order(class)
    }

    fn view(
        root: RegId,
        _class: Class,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        Regs(&crate::REGISTER_INFO).root(register)
    }

    fn reserved(root: RegId) -> bool {
        // sp/xzr, the frame pointer, the link register and the platform
        // register.
        [crate::reg::X30, crate::reg::SP, crate::reg::XZR].contains(&root)
            || (root == crate::reg::X29 && !frame_pointer_free())
            || (darwin() && root == crate::reg::X18)
    }

    fn caller_saved() -> Vec<RegId> {
        let gprs = (0..=18u32).map(|n| id(Class::Gpr, n));
        let fprs = (0..=7u32).chain(16..=31).map(|n| id(Class::Fpr, n));
        gprs.chain(fprs).collect()
    }

    fn csr_first_use_cost() -> u32 {
        1
    }

    /// `stp`/`ldp` save two registers in 4 bytes; a spill is one 4-byte
    /// `str`/`ldr`; a made-again constant is a `movz` or an `adrp`+`add`.
    fn size_bytes() -> Option<llrm_codegen::greedy::targetinfo::SizeBytes> {
        Some(llrm_codegen::greedy::targetinfo::SizeBytes {
            push: 2,
            pop: 2,
            load: 4,
            store: 4,
            constant: 4,
            extended_prefix: 0,
        })
    }

    fn spill_store(
        _class: Class,
        register: LirReg,
        slot: i32,
    ) -> Option<LirInst> {
        let mut inst = LirInst::new(Kind::Op("SpillStore"), vec![Operand::Use(register), Operand::Frame(slot)]);
        inst.may_store = true;
        Some(inst)
    }

    fn is_zero_def(name: &str) -> bool {
        name == ZERO
    }

    /// `str xzr, [slot]`.
    fn zero_store(
        _class: Class,
        slot: i32,
    ) -> Option<LirInst> {
        Self::spill_store(Class::Gpr, LirReg::Phys(crate::reg::XZR), slot)
    }

    fn spill_load(
        _class: Class,
        register: LirReg,
        slot: i32,
    ) -> Option<LirInst> {
        let mut inst = LirInst::new(Kind::Op("SpillLoad"), vec![Operand::Def(register), Operand::Frame(slot)]);
        inst.may_load = true;
        Some(inst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::inst::{AddSub, Cond, Wide, Width};
    use crate::mfunc::{MBlock, OpList};

    fn sample() -> MFunc {
        let mut function = MFunc::new("f");
        let (a, b, v) = (function.gpr(), function.gpr(), function.fpr());
        let add = Op::I(Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: true },
            width: Width::X,
            rd: b,
            rn: a,
            imm12: 1,
            shift12: false,
        });
        let keep = Op::I(Inst::MoveWide { op: Wide::Keep, width: Width::X, rd: b, imm16: 1, lsl: 16 });
        let ops = vec![
            Op::Copy { dst: Reg::Gpr(a), src: Reg::Gpr(Gpr(0)) },
            add,
            keep,
            Op::Copy { dst: Reg::Fpr(v), src: Reg::Fpr(Vreg(0)) },
            Op::I(Inst::BranchCond { cond: Cond::Ne, target: Target::Label("L1".into()) }),
        ];
        function.blocks.push(MBlock { label: "L0".into(), ops: OpList::from(ops) });
        function
            .blocks
            .push(
                MBlock {
                    label: "L1".into(),
                    ops: OpList::from(vec![Op::Return { uses: vec![Reg::Gpr(Gpr(0))], tail: None }]),
                },
            );
        function
    }

    /// The allocator's function carried no branch odds on arm64, so greedy
    /// weighed every branch but a loop's even: the zero-compare 20:12 the pin
    /// uses was 16:16, and idioms `main` -O1 ranked its loop counter below an
    /// address constant (block frequencies 16/512 against the pin's 20/640).
    #[test]
    fn the_irs_branch_odds_reach_the_allocators_function() {
        use llrm_spike_mf::placement::Odds;
        let mut function = sample();
        let odds: Odds = [((0, 1), (llrm_mir::blockfrequency::BranchProbability::new(5, 8), 0))].into_iter().collect();
        function.alloc_odds = Some(odds.clone());
        assert_eq!(view(&function).odds, Some(odds));
    }

    /// `view` and `rebuild` each looked for the fused address pairs of the
    /// function (a pass over every operation, 0.4 M each on shellsort's two
    /// functions): `allocate` finds them once, for both.
    #[test]
    fn allocate_finds_the_address_pairs_once() {
        let mut function = sample();
        let before = address_pair_runs();
        allocate(&mut function, false).expect("allocates");
        assert_eq!(address_pair_runs() - before, 1);
    }

    /// `candidates` rebuilt its register order (a chain of ranges mapped to ids
    /// into a vector) on every call: 3.5 M of shellsort's 583 M, 4,274 calls.
    /// The order is built once per target state and class. The state flags are
    /// process-wide, so the test passes each state itself.
    #[test]
    fn the_candidate_order_is_built_once_per_state() {
        let before = candidate_orders_built();
        let base = (Class::Gpr, false, false, false);
        let first = cached_order(base);
        for _ in 0..50 {
            assert_eq!(cached_order(base), first);
        }
        assert_eq!(candidate_orders_built() - before, 1, "built again for the same state");
        assert!(!first.is_empty(), "the premise: an order");
    }

    /// Each input of the order is in the cache's key: flipping one gives the
    /// order that input makes, never a cached one of another state.
    #[test]
    fn every_input_of_the_candidate_order_is_in_the_cache_key() {
        let order = |state| cached_order(state);
        let base = order((Class::Gpr, false, false, false));
        let (darwin, scratch, fp) = (
            order((Class::Gpr, true, false, false)),
            order((Class::Gpr, false, true, false)),
            order((Class::Gpr, false, false, true)),
        );
        let x = |n: u32| id(Class::Gpr, n);
        assert!(base.contains(&x(18)) && !darwin.contains(&x(18)), "x18 is the platform register on Darwin");
        assert!(
            !base.contains(&x(16)) && scratch.contains(&x(16)) && scratch.contains(&x(17)),
            "x16, x17 when the frame needs no scratch"
        );
        assert!(!base.contains(&x(29)) && fp.contains(&x(29)), "x29 when there is no frame record");
        // Asking again returns each state's own order, not the last one's.
        assert_eq!(order((Class::Gpr, false, false, false)), base);
        assert_eq!(order((Class::Gpr, true, false, false)), darwin);
        assert_eq!(order((Class::Gpr, false, false, true)), fp);
        let (fpr, quad) = (order((Class::Fpr, false, false, false)), order((Class::Quad, false, false, false)));
        assert!(quad.len() < fpr.len(), "the classes have their own orders");
    }

    /// `movk` reads and writes its register: the view needs a def, a use and a
    /// tie, or the allocator may give the two different registers.
    #[test]
    fn a_keeping_move_is_tied() {
        let lir = view(&sample());
        let keep = &lir.blocks[0].insts[2];
        assert_eq!(keep.ties, vec![(0, 1)], "{keep:?}");
        assert_eq!(lir.classes, vec![Class::Gpr, Class::Gpr, Class::Fpr]);
        assert_eq!(lir.blocks[0].insts[4].operands[0], Operand::Block(1));
    }

    /// Greedy rebuilt the operations without their source places, so `-g` with
    /// the shared allocator gave a function one line row. Each operation keeps
    /// its place; a copy that goes leaves the neighbours' alone.
    #[test]
    fn source_places_go_back_with_the_operations() {
        use llrm_codegen::model::lir::SrcLoc;
        let mut function = sample();
        let ops = std::mem::take(&mut function.blocks[0].ops);
        function.blocks[0].ops =
            ops.into_iter().zip([7, 8, 9, 10, 11]).map(|((op, _), line)| (op, Some(SrcLoc::line(line)))).collect();
        let lir = view(&function);
        let mut back = function.clone();
        let (x0, x5, v4) = (crate::reg::X0, crate::reg::X5, RegId::from_raw(crate::reg::V0.index() as u8 + 4));
        rebuild(&mut back, &lir, &Assignment(vec![x0, x5, v4]), &[]);
        let lines: Vec<_> =
            (0..back.blocks[0].ops.len()).map(|at| back.blocks[0].ops.loc(at).map(|l| l.line)).collect();
        assert_eq!(lines, [Some(8), Some(9), Some(10), Some(11)], "the copy of x0 into itself (line 7) goes");
    }

    /// The pin keeps `adrp` + `add :lo12:` of one symbol as a single `MOVaddr`
    /// until after allocation, so the allocator can make the address again
    /// where it is read (jpeg_single -O3: the address was stored and
    /// reloaded across a call, +252 B). Here the pair is one rematerializable
    /// instruction that defines one value, and it expands into the register
    /// greedy chose.
    #[test]
    fn an_address_pair_is_one_rematerializable_instruction_until_after_allocation() {
        let mut function = MFunc::new("f");
        let (page, address) = (function.gpr(), function.gpr());
        let target = Target::Symbol { name: "_msg".into(), addend: 0 };
        let ops = vec![
            Op::I(Inst::Adr { page: true, rd: page, target: target.clone(), got: false }),
            Op::I(Inst::AddPageOffset { rd: address, rn: page, target: target.clone() }),
            Op::Return { uses: vec![Reg::Gpr(address)], tail: None },
        ];
        function.blocks.push(MBlock { label: "L0".into(), ops: OpList::from(ops) });
        let lir = view(&function);
        let insts = &lir.blocks[0].insts;
        assert_eq!(insts.len(), 2, "the pair is one instruction and the return: {insts:?}");
        assert!(insts[0].rematerializable, "{:?}", insts[0]);
        let defs = insts[0].operands.iter().filter(|o| matches!(o, Operand::Def(_))).count();
        let uses = insts[0].operands.iter().filter(|o| matches!(o, Operand::Use(_))).count();
        assert_eq!((defs, uses), (1, 0), "{:?}", insts[0]);
        let mut back = function.clone();
        rebuild(&mut back, &lir, &Assignment(vec![crate::reg::X5, crate::reg::X6]), &[]);
        let ops = &back.blocks[0].ops;
        assert_eq!(ops[0], Op::I(Inst::Adr { page: true, rd: Gpr(6), target: target.clone(), got: false }), "{ops:?}");
        assert_eq!(ops[1], Op::I(Inst::AddPageOffset { rd: Gpr(6), rn: Gpr(6), target }), "{ops:?}");
    }

    /// `sub sp, sp, #16` defined register 31, the zero register, so the
    /// scheduler saw no stack-pointer write and moved the stores below it
    /// (`vla_call` printed 918, not 1213).
    #[test]
    fn a_write_of_the_stack_pointer_defines_sp_not_the_zero_register() {
        let (sp, minus) = (Gpr(31), AddSub { subtract: true, set_flags: false });
        let mut function = sample();
        function.blocks[0].ops = OpList::from(vec![
            Op::I(Inst::AddSubImm { op: minus, width: Width::X, rd: sp, rn: sp, imm12: 16, shift12: false }),
            Op::I(Inst::AddSubImm { op: minus, width: Width::X, rd: Gpr(9), rn: sp, imm12: 16, shift12: false }),
            Op::Return { uses: vec![], tail: None },
        ]);
        let lir = view(&function);
        let defined = |at: usize| {
            lir.blocks[0]
                .insts[at]
                .operands
                .iter()
                .find_map(
                    |o| match o {
                        Operand::Def(LirReg::Phys(register)) => Some(*register),
                        _ => None,
                    },
                )
        };
        assert_eq!(defined(0), Some(crate::reg::SP), "{:?}", lir.blocks[0].insts[0]);
        assert_ne!(defined(1), Some(crate::reg::SP), "a read of sp defines nothing");
    }

    /// The registers greedy chose go back into the original operations, a
    /// copy between the same register goes, and a tied register stays one.
    #[test]
    fn registers_go_back_into_the_function() {
        let function = sample();
        let lir = view(&function);
        let mut back = function.clone();
        let (x0, x5, v4) = (crate::reg::X0, crate::reg::X5, RegId::from_raw(crate::reg::V0.index() as u8 + 4));
        rebuild(&mut back, &lir, &Assignment(vec![x0, x5, v4]), &[]);
        let ops = &back.blocks[0].ops;
        assert_eq!(ops.len(), 4, "the copy of x0 into itself goes: {ops:?}");
        let Op::I(Inst::AddSubImm { rd, rn, .. }) = &ops[0] else { panic!("{ops:?}") };
        assert_eq!((*rd, *rn), (Gpr(5), Gpr(0)));
        let Op::I(Inst::MoveWide { rd, .. }) = &ops[1] else { panic!("{ops:?}") };
        assert_eq!(*rd, Gpr(5));
        assert_eq!(ops[2], Op::Copy { dst: Reg::Fpr(Vreg(4)), src: Reg::Fpr(Vreg(0)) });
    }

    /// Machine registers are the register file's ids: raw numbers made `x5`
    /// the file's `x4`, and every physical register the allocator named wrong.
    #[test]
    fn machine_registers_are_the_register_files_ids() {
        let lir = view(&sample());
        let copy = &lir.blocks[0].insts[3];
        assert_eq!(copy.operands[1], Operand::Use(LirReg::Phys(crate::reg::V0)));
        assert_eq!(from_lir(LirReg::Phys(crate::reg::X5), Class::Gpr), Reg::Gpr(Gpr(5)));
        assert_eq!(from_lir(LirReg::Phys(crate::reg::V3), Class::Fpr), Reg::Fpr(Vreg(3)));
    }

    /// An `Imm 1` into the fixed `w0` and one into a virtual register had one
    /// content (every register blanked to 0): the allocator took the fixed one
    /// as the template of the other, and jpeg -Os read `w10` that `movz w0`
    /// had been emitted for (a wrong checksum).
    #[test]
    fn a_fixed_register_and_a_virtual_one_do_not_share_content() {
        let into = |dst| Op::Imm { dst: Gpr(dst), bits: 32, value: 1 };
        assert_ne!(content(&into(0)), content(&into(Gpr::FIRST_VIRTUAL + 7)));
        assert_eq!(content(&into(Gpr::FIRST_VIRTUAL + 1)), content(&into(Gpr::FIRST_VIRTUAL + 7)));
    }
}
