//! The function as the shared allocator sees it: an [`MFunction`] over
//! [`Class`]es, and the way back. Registers are the only thing that changes
//! between the two, so the view keeps the original [`MFunc`] and writes the
//! allocator's registers into it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::mfunc::{Block, Inst as LirInst, Kind, MFunction, Operand, Reg as LirReg};
use llrm_spike_mf::regalloc::Target as AllocTarget;

use crate::flagcse::writes_flags;
use crate::mc::inst::{Gpr, Inst, Target, Vreg};
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op};

/// A virtual register's class: the width of a use is the instruction's.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Class {
    Gpr,
    Fpr,
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
        (Class::Fpr, n) => RegId::from_raw(crate::reg::V0.index() as u8 + n as u8),
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
        (LirReg::Virt(n), Class::Fpr) => Reg::Fpr(Vreg(n + Gpr::FIRST_VIRTUAL)),
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

/// The allocator's function for `function`, registers of both classes numbered
/// from zero.
pub fn view(function: &MFunc) -> MFunction<Class> {
    let mut lir = MFunction::<Class>::new(&function.name);
    lir.classes = vec![Class::Gpr; (function.next_register - Gpr::FIRST_VIRTUAL) as usize];
    let labels: HashMap<&str, u32> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at as u32)).collect();
    for (at, block) in function.blocks.iter().enumerate() {
        let mut insts = Vec::new();
        for op in &block.ops {
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
                Op::Call { .. } => (Kind::Op("Call"), false, false, true),
                Op::Return { .. } => (Kind::Op("Return"), true, true, false),
            };
            let mut operands = Vec::new();
            let mut ties = Vec::new();
            let mut op = op.clone();
            op.visit(&mut |register, role| {
                let key = register.key();
                let (reg, class) = (to_lir(key), if matches!(key, Reg::Gpr(_)) { Class::Gpr } else { Class::Fpr });
                if let LirReg::Virt(n) = reg {
                    lir.classes[n as usize] = class;
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
            let mut inst = LirInst::new(kind, operands);
            inst.ties = ties;
            inst.terminator = terminator;
            inst.barrier = barrier;
            inst.call = call;
            inst.sets_flags = writes_flags(&op);
            inst.reads_flags = reads_flags(&op);
            insts.push(inst);
        }
        lir.blocks.push(Block { id: at as u32, insts, phis: Vec::new() });
    }
    lir
}

/// `function` with the registers of `lir`, which has the instructions of
/// [`view`] in the same places.
pub fn apply(
    function: &mut MFunc,
    lir: &MFunction<Class>,
) {
    for (block, lir_block) in function.blocks.iter_mut().zip(&lir.blocks) {
        for (op, inst) in (&mut block.ops).into_iter().zip(&lir_block.insts) {
            let mut registers = inst
                .operands
                .iter()
                .filter_map(
                    |operand| match operand {
                        Operand::Def(reg) | Operand::Use(reg) => Some(*reg),
                        _ => None,
                    },
                );
            op.visit(&mut |register, role| {
                let reg = registers.next().expect("an operand for each register");
                if role == Role::UseDef {
                    registers.next();
                }
                let class = if matches!(register.key(), Reg::Gpr(_)) { Class::Gpr } else { Class::Fpr };
                match (register, from_lir(reg, class)) {
                    (RegRef::Gpr(slot), Reg::Gpr(new)) => *slot = new,
                    (RegRef::Fpr(slot), Reg::Fpr(new)) => *slot = new,
                    _ => panic!("a register moved between classes"),
                }
            });
        }
    }
}

/// The 64-bit and vector registers the allocator hands out, in the pin's
/// order (`AArch64RegisterInfo::getCalleeSavedRegs` aside: callee-saved
/// registers come last).
pub struct Arm64;

fn gpr_order() -> Vec<RegId> {
    // x9..x15 and x0..x8 (caller-saved) first, then x16, x17 and x19..x28.
    (9..=15u32).chain(0..=8).chain(16..=17).chain(19..=28).map(|n| id(Class::Gpr, n)).collect()
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
        (one == other).then_some(one)
    }

    fn any() -> Class {
        Class::Gpr
    }

    fn candidates(class: Class) -> Vec<RegId> {
        match class {
            Class::Gpr => gpr_order(),
            Class::Fpr => fpr_order(),
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
        [crate::reg::X18, crate::reg::X29, crate::reg::X30, crate::reg::SP, crate::reg::XZR].contains(&root)
    }

    fn caller_saved() -> Vec<RegId> {
        let gprs = (0..=17u32).map(|n| id(Class::Gpr, n));
        let fprs = (0..=7u32).chain(16..=31).map(|n| id(Class::Fpr, n));
        gprs.chain(fprs).collect()
    }

    fn csr_first_use_cost() -> u32 {
        1
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
        assert_eq!(lir.blocks[0].insts[4].operands, vec![Operand::Block(1)]);
    }

    /// The view and back is the same function; a register the allocator
    /// changed shows up in the instruction.
    #[test]
    fn registers_go_back_into_the_function() {
        let function = sample();
        let mut lir = view(&function);
        let mut same = function.clone();
        apply(&mut same, &lir);
        assert_eq!(same.blocks, function.blocks);
        let x5 = LirReg::Phys(crate::reg::X5);
        for operand in &mut lir.blocks[0].insts[1].operands {
            if let Operand::Def(reg) = operand {
                *reg = x5;
            }
        }
        let mut changed = function.clone();
        apply(&mut changed, &lir);
        let Op::I(Inst::AddSubImm { rd, .. }) = &changed.blocks[0].ops[1] else { panic!() };
        assert_eq!(*rd, Gpr(5));
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
