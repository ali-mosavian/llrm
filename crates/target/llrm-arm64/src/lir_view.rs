//! The function as the shared allocator sees it: an [`MFunction`] over
//! [`Class`]es, and the way back. Registers are the only thing that changes
//! between the two, so the view keeps the original [`MFunc`] and writes the
//! allocator's registers into it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::mfunc::{Block, Inst as LirInst, Kind, MFunction, Operand, Reg as LirReg};
use llrm_spike_mf::regalloc::{Assignment, Target as AllocTarget};
use llrm_spike_mf::tolir::allocate_greedy;

use crate::flagcse::writes_flags;
use crate::mc::inst::{Gpr, Inst, Target, Vreg, Wide};
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
fn id(
    class: Class,
    n: u32,
) -> RegId {
    match (class, n) {
        (Class::Gpr, 31) => crate::reg::XZR,
        (Class::Gpr, n) => RegId::from_raw(crate::reg::X0.index() as u8 + n as u8),
        (Class::Fpr | Class::Quad, n) => RegId::from_raw(crate::reg::V0.index() as u8 + n as u8),
    }
}

fn to_lir(register: Reg) -> LirReg {
    match register {
        Reg::Gpr(Gpr(n)) if n < Gpr::FIRST_VIRTUAL => LirReg::Phys(id(Class::Gpr, n)),
        Reg::Fpr(Vreg(n)) if n < Gpr::FIRST_VIRTUAL => LirReg::Phys(id(Class::Fpr, n)),
        Reg::Gpr(Gpr(n)) | Reg::Fpr(Vreg(n)) => LirReg::Virt(n - Gpr::FIRST_VIRTUAL),
    }
}

fn from_lir(
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
fn name_of(inst: &Inst) -> &'static str {
    static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let debug = format!("{inst:?}");
    let variant = debug.split([' ', '(', '{']).next().unwrap_or_default().to_owned();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap();
    names.entry(variant.clone()).or_insert_with(|| Box::leak(variant.into_boxed_str()))
}

fn reads_flags(op: &Op) -> bool {
    matches!(
        op,
        Op::I(Inst::BranchCond { .. } | Inst::CondSelect { .. } | Inst::CondCompare { .. } | Inst::FpSelect { .. })
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
fn content(op: &Op) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut blank = op.clone();
    blank.visit(&mut |register, _| match register {
        RegRef::Gpr(register) => *register = Gpr(0),
        RegRef::Fpr(register) => *register = Vreg(0),
    });
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{blank:?}").hash(&mut hasher);
    hasher.finish() as i64
}

/// The `adrp` and `add :lo12:` the pin keeps as one `MOVaddr` until after
/// allocation (`AArch64ExpandPseudo`): a page register defined once, read by
/// one `add` of the same symbol in the same block. By block: the index of the
/// `add` and of its `adrp`. As one instruction the allocator can make the
/// address again where it is needed instead of spilling it.
fn address_pairs(function: &MFunc) -> Vec<HashMap<usize, usize>> {
    let mut seen: HashMap<Reg, (u32, u32)> = HashMap::new();
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
            let mut pages: HashMap<Reg, (usize, &Target)> = HashMap::new();
            let mut pairs = HashMap::new();
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
    let mut lir = MFunction::<Class>::new(&function.name);
    lir.classes = vec![Class::Gpr; (function.next_register - Gpr::FIRST_VIRTUAL) as usize];
    lir.locals =
        function.slots.iter().enumerate().map(|(at, slot)| (at as i32, slot.size as i64, slot.align as i64)).collect();
    let wide = crate::regalloc::wide_registers(function);
    let labels: HashMap<&str, u32> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at as u32)).collect();
    let mut origin = 0u32;
    let pairs = address_pairs(function);
    for (at, block) in function.blocks.iter().enumerate() {
        let mut insts = Vec::new();
        let first_halves: Vec<usize> = pairs[at].values().copied().collect();
        for (index, op) in block.ops.iter().enumerate() {
            origin += 1;
            if first_halves.contains(&index) {
                continue;
            }
            let fused = pairs[at].contains_key(&index);
            let (kind, terminator, barrier, call) = match op {
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
                Op::Imm { .. } => (Kind::Op("Imm"), false, false, false),
                Op::FrameAddress { .. } => (Kind::Op("FrameAddress"), false, false, false),
                Op::JumpTable { .. } => (Kind::Op("JumpTable"), true, true, false),
                Op::Call { .. } => (Kind::Op("Call"), false, false, true),
                Op::Return { .. } => (Kind::Op("Return"), true, true, false),
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
            inst.ties = ties;
            inst.origin = origin;
            inst.terminator = terminator;
            inst.barrier = barrier;
            inst.call = call;
            inst.sets_flags = writes_flags(&op);
            inst.reads_flags = reads_flags(&op);
            inst.may_load = matches!(
                op,
                Op::I(Inst::Load { .. } | Inst::LoadPair { .. } | Inst::Atomic { .. } | Inst::Prefetch { .. })
            );
            inst.may_store = matches!(
                op,
                Op::I(Inst::Store { .. } | Inst::StorePair { .. } | Inst::Atomic { .. })
            );
            inst.side_effects = matches!(
                op,
                Op::I(Inst::Svc { .. } | Inst::Dmb { .. } | Inst::Atomic { .. })
            );
            inst.early_clobber = matches!(op, Op::I(Inst::Atomic { .. }) | Op::JumpTable { .. });
            inst.rematerializable = fused
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
            inst.cheap_as_move = matches!(op, Op::Imm { .. } | Op::I(Inst::MoveWide { .. }));
            insts.push(inst);
        }
        lir.blocks.push(Block { id: at as u32, insts, phis: Vec::new() });
    }
    lir
}

/// `function`, its registers the physical ones greedy chose in `lir`
/// (`assignment` by virtual number) and the spills it wrote: each operation is
/// the original one found by its [`content`], the spills and copies new.
pub fn rebuild(
    function: &mut MFunc,
    lir: &MFunction<Class>,
    assignment: &Assignment,
    spill_slots: &[(i32, Class)],
) {
    let mut originals: HashMap<i64, Op> = HashMap::new();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        originals.entry(content(op)).or_insert_with(|| op.clone());
    }
    // The fused `MOVaddr`s stand under the content `view` gave them.
    let pairs = address_pairs(function);
    let mut fused: HashMap<i64, Op> = HashMap::new();
    for (block, pairs) in function.blocks.iter().zip(&pairs) {
        for &add in pairs.keys() {
            fused.entry(content(&block.ops[add]).wrapping_add(1)).or_insert_with(|| block.ops[add].clone());
        }
    }
    // The place of each operation, by the number `view` gave it.
    let places: Vec<_> =
        function.blocks.iter().flat_map(|block| (0..block.ops.len()).map(|at| block.ops.loc(at))).collect();
    let mut slots: HashMap<i32, u32> = HashMap::new();
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
    for (block, lir_block) in function.blocks.iter_mut().zip(&lir.blocks) {
        let mut ops = Vec::with_capacity(lir_block.insts.len());
        let mut locs = Vec::with_capacity(lir_block.insts.len());
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
                        ops.push(Op::Copy { dst, src });
                    }
                }
                Kind::Op(name @ ("SpillStore" | "SpillLoad")) => {
                    let Some(Operand::Frame(slot)) = inst.operands.last() else { panic!("a spill names no slot") };
                    let slot = slots[slot];
                    let quad = registers[0].1 == Class::Quad;
                    ops.push(spill_access(reg_at(0), slot, name == "SpillLoad", quad));
                }
                Kind::Op(_) if matches!(inst.operands.last(), Some(Operand::Imm(hash)) if fused.contains_key(hash)) => {
                    // `AArch64ExpandPseudo`: `adrp rd` and `add rd, rd, :lo12:`.
                    let Some(Operand::Imm(hash)) = inst.operands.last() else { unreachable!() };
                    let Op::I(Inst::AddPageOffset { target, .. }) = &fused[hash] else { unreachable!() };
                    let Reg::Gpr(rd) = reg_at(0) else { panic!("a page offset into a vector register") };
                    ops.push(Op::I(Inst::Adr { page: true, rd, target: target.clone(), got: false }));
                    ops.push(Op::I(Inst::AddPageOffset { rd, rn: rd, target: target.clone() }));
                }
                Kind::Op(_) => {
                    let Some(Operand::Imm(hash)) = inst.operands.last() else { panic!("an operation with no content") };
                    let mut op = originals[hash].clone();
                    let mut at = 0;
                    op.visit(&mut |register, role| {
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
        block.ops = ops.into_iter().zip(locs).collect();
    }
}

/// The callee-saved registers `function` writes, for the frame to save.
pub fn written_callee_saved(function: &MFunc) -> Allocation {
    let mut allocation = Allocation::default();
    for op in function.blocks.iter().flat_map(|block| &block.ops) {
        let (defs, _) = op.defs_and_uses();
        for register in defs {
            match register {
                Reg::Gpr(Gpr(n)) if (19..=28).contains(&n) && !allocation.saved_gprs.contains(&n) => {
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
pub fn allocate(function: &mut MFunc) -> Result<Allocation, String> {
    let mut lir = view(function);
    crate::licm::hoist(&mut lir);
    let (assignment, slots) = allocate_greedy::<Arm64>(&mut lir, Regs(&crate::REGISTER_INFO), false)?;
    rebuild(function, &lir, &assignment, &slots);
    Ok(written_callee_saved(function))
}

/// The 64-bit and vector registers the allocator hands out, in the pin's
/// order (`AArch64RegisterInfo::getCalleeSavedRegs` aside: callee-saved
/// registers come last).
pub struct Arm64;

/// Whether the target is Apple's: `x18` is the platform register there
/// (`AArch64RegisterInfo::getReservedRegs`, `isTargetDarwin`), an ordinary
/// caller-saved register on Linux.
static DARWIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Sets the target the next allocations are for.
pub fn set_darwin(darwin: bool) {
    DARWIN.store(darwin, std::sync::atomic::Ordering::Relaxed);
}

fn darwin() -> bool {
    DARWIN.load(std::sync::atomic::Ordering::Relaxed)
}

/// The caller-saved general registers the allocator hands out.
pub fn gpr_caller_saved_count() -> usize {
    15 + 1 + usize::from(!darwin())
}

fn gpr_order() -> Vec<RegId> {
    // The caller-saved registers first, then x19..x28. x16 and x17 are the
    // frame lowerer's scratch registers; x15 and, off Darwin, x18 are
    // allocatable (the pin's `getReservedRegs` reserves x18 on Darwin only).
    let extra: Vec<u32> = if darwin() { vec![15] } else { vec![15, 18] };
    (9..=14u32).chain((0..=8).rev()).chain(extra).chain(19..=28).map(|n| id(Class::Gpr, n)).collect()
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

    fn candidates(class: Class) -> Vec<RegId> {
        match class {
            Class::Gpr => gpr_order(),
            Class::Fpr => fpr_order(),
            Class::Quad => fpr_order().into_iter().filter(|id| !callee_saved_fpr(*id)).collect(),
        }
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
        [crate::reg::X29, crate::reg::X30, crate::reg::SP, crate::reg::XZR].contains(&root)
            || (darwin() && root == crate::reg::X18)
    }

    fn coalescer_asks_target_for_remat() -> bool {
        true
    }

    fn caller_saved() -> Vec<RegId> {
        let gprs = (0..=18u32).map(|n| id(Class::Gpr, n));
        let fprs = (0..=7u32).chain(16..=31).map(|n| id(Class::Fpr, n));
        gprs.chain(fprs).collect()
    }

    fn csr_first_use_cost() -> u32 {
        1
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
}
