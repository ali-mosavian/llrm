//! The selector's output as the shared flow's [`LirBody`], and the way back
//! (gap 1 of `docs/architecture/arm64-core-passes.md`). Skeleton: the forms
//! below are mapped; any other is [`Unmapped`], named by its variant, so the
//! first missing form of a program is the next one to write.
//!
//! Mapping: a virtual register is `Loc::Held` (value = its number), a machine
//! register `Loc::Reg`; the instruction's name is LLVM's opcode (`ADDXri`).
//! The class of a `COPY`'s registers is in its name (`COPY`, `COPY_FPR`) until
//! `LirBody::class_like` carries it.

use std::sync::Arc;

use llrm_codegen::model::lir::{Insn, LirBlock, LirBody};
use llrm_codegen::support::hash::IndexMap;
use llrm_lir::{Held, Imm, Loc, Operation, Reg as LirRegLoc, Semantics};
use llrm_spike_mf::mfunc::Reg as MfReg;

use crate::lir_view::{from_lir as machine_register, to_lir as lir_register};
use crate::mc::inst::{AddSub, Gpr, Inst, Target, Width};
use crate::mc::visit::Reg;
use crate::mfunc::{MBlock, MFunc, Op};

/// An operation the producer has no mapping for yet: its variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unmapped(pub String);

fn loc(
    register: Reg,
    bits: u32,
) -> Loc {
    match lir_register(register) {
        MfReg::Phys(register) => Loc::Reg(LirRegLoc { register, width: bits }),
        MfReg::Virt(value) => Loc::Held(Held { value, width: bits, offset: 0 }),
    }
}

fn imm(value: i64) -> Loc {
    Loc::Imm(Imm { value, width: 64, address: None })
}

fn value_of(loc: &Loc) -> i64 {
    match loc {
        Loc::Imm(imm) => imm.value,
        other => panic!("an immediate operand, found {other:?}"),
    }
}

fn gpr_of(loc: &Loc) -> Gpr {
    match loc {
        Loc::Reg(LirRegLoc { register, .. }) => from_gpr(MfReg::Phys(*register)),
        Loc::Held(held) => from_gpr(MfReg::Virt(held.value)),
        other => panic!("a register operand, found {other:?}"),
    }
}

fn from_gpr(register: MfReg) -> Gpr {
    match machine_register(register, crate::lir_view::Class::Gpr) {
        Reg::Gpr(gpr) => gpr,
        Reg::Fpr(_) => panic!("a general register"),
    }
}

fn reg_of(
    loc: &Loc,
    fpr: bool,
) -> Reg {
    let class = if fpr { crate::lir_view::Class::Fpr } else { crate::lir_view::Class::Gpr };
    match loc {
        Loc::Reg(LirRegLoc { register, .. }) => machine_register(MfReg::Phys(*register), class),
        Loc::Held(held) => machine_register(MfReg::Virt(held.value), class),
        other => panic!("a register operand, found {other:?}"),
    }
}

fn add_sub_name(
    op: AddSub,
    width: Width,
) -> String {
    let w = if width == Width::X { 'X' } else { 'W' };
    format!("{}{}{w}ri", if op.subtract { "SUB" } else { "ADD" }, if op.set_flags { "S" } else { "" })
}

fn semantics(
    op: Operation,
    name: impl AsRef<str>,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    target: Option<i64>,
) -> Semantics {
    Semantics {
        op,
        name: Some(Arc::from(name.as_ref())),
        dests,
        sources,
        target,
        indirect: false,
        table: Vec::new(),
        table_id: None,
        repeat: false,
        pops: None,
    }
}

fn block_of(
    function: &MFunc,
    target: &Target,
) -> Result<i64, Unmapped> {
    match target {
        Target::Label(label) => function
            .blocks
            .iter()
            .position(|block| &block.label == label)
            .map(|at| at as i64)
            .ok_or_else(|| Unmapped(format!("branch to unknown label {label}"))),
        other => Err(Unmapped(format!("Target::{other:?}"))),
    }
}

fn one(
    function: &MFunc,
    op: &Op,
) -> Result<Semantics, Unmapped> {
    Ok(match op {
        Op::Copy { dst, src } => {
            let fpr = matches!(dst, Reg::Fpr(_));
            semantics(
                Operation::Move,
                if fpr { "COPY_FPR" } else { "COPY" },
                vec![loc(*dst, 64)],
                vec![loc(*src, 64)],
                None,
            )
        }
        Op::Imm { dst, bits, value } => {
            semantics(Operation::Move, "MOVImm", vec![loc(Reg::Gpr(*dst), *bits)], vec![imm(*value as i64)], None)
        }
        Op::I(Inst::Ret { rn }) => semantics(Operation::Return, "RET", vec![], vec![loc(Reg::Gpr(*rn), 64)], None),
        Op::I(Inst::Branch { link: false, target }) => {
            semantics(Operation::Jump, "B", vec![], vec![], Some(block_of(function, target)?))
        }
        Op::I(Inst::AddSubImm { op, width, rd, rn, imm12, shift12 }) => semantics(
            Operation::Binary,
            add_sub_name(*op, *width),
            vec![loc(Reg::Gpr(*rd), width.bits())],
            vec![loc(Reg::Gpr(*rn), width.bits()), imm(i64::from(*imm12)), imm(i64::from(*shift12))],
            None,
        ),
        Op::Return { uses, tail: None } => {
            semantics(Operation::Return, "RET_ReallyLR", vec![], uses.iter().map(|r| loc(*r, 64)).collect(), None)
        }
        Op::I(inst) => return Err(Unmapped(crate::lir_view::name_of(inst).to_owned())),
        Op::Call { .. } => return Err(Unmapped("Call".into())),
        Op::Return { .. } => return Err(Unmapped("Return(tail)".into())),
        Op::FrameAddress { .. } => return Err(Unmapped("FrameAddress".into())),
        Op::JumpTable { .. } => return Err(Unmapped("JumpTable".into())),
        Op::DbgValue { .. } => return Err(Unmapped("DbgValue".into())),
    })
}

/// The function's operations as a body: block `n` is `at` = `n`.
pub fn to_lir(function: &MFunc) -> Result<LirBody, Unmapped> {
    let mut at = 0_i64;
    let mut blocks = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::new();
        for op in block.ops.iter() {
            let what = one(function, op)?;
            insns.push(Arc::new(Insn::new(at, None, Some(what), vec![], vec![])));
            at += 1;
        }
        blocks.push(LirBlock::new(index as i64, insns).into());
    }
    Ok(LirBody::new(function.name.clone(), 0, blocks, IndexMap::default(), IndexMap::default()))
}

fn back(
    template: &MFunc,
    what: &Semantics,
) -> Result<Op, Unmapped> {
    let (dest, source) = (what.dests.first(), what.sources.first());
    let name = what.name.as_deref().unwrap_or("");
    Ok(match (what.op, name) {
        (Operation::Move, "COPY" | "COPY_FPR") => {
            let fpr = name == "COPY_FPR";
            Op::Copy { dst: reg_of(dest.unwrap(), fpr), src: reg_of(source.unwrap(), fpr) }
        }
        (Operation::Move, "MOVImm") => {
            let bits = match dest.unwrap() {
                Loc::Reg(r) => r.width,
                Loc::Held(h) => h.width,
                other => panic!("a register, found {other:?}"),
            };
            Op::Imm { dst: gpr_of(dest.unwrap()), bits, value: value_of(source.unwrap()) as u64 }
        }
        (Operation::Return, "RET_ReallyLR") => {
            Op::Return { uses: what.sources.iter().map(|s| reg_of(s, false)).collect(), tail: None }
        }
        (Operation::Return, "RET") => Op::I(Inst::Ret { rn: gpr_of(source.unwrap()) }),
        (Operation::Jump, "B") => {
            let label = template.blocks[what.target.unwrap() as usize].label.clone();
            Op::I(Inst::Branch { link: false, target: Target::Label(label) })
        }
        (Operation::Binary, name) if name.ends_with("ri") && (name.starts_with("ADD") || name.starts_with("SUB")) => {
            let width = if name.ends_with("Xri") { Width::X } else { Width::W };
            let op = AddSub { subtract: name.starts_with("SUB"), set_flags: name.as_bytes()[3] == b'S' };
            Op::I(Inst::AddSubImm {
                op,
                width,
                rd: gpr_of(dest.unwrap()),
                rn: gpr_of(source.unwrap()),
                imm12: value_of(&what.sources[1]) as u16,
                shift12: value_of(&what.sources[2]) != 0,
            })
        }
        _ => return Err(Unmapped(name.to_owned())),
    })
}

/// `template` with its blocks' operations replaced by the body's: everything
/// that is not an operation (labels, frame objects, pool) is the template's.
pub fn from_lir(
    body: &LirBody,
    template: &MFunc,
) -> Result<MFunc, Unmapped> {
    let mut function = template.clone();
    function.blocks = Vec::new();
    for (block, original) in body.blocks.iter().zip(&template.blocks) {
        let mut rebuilt = MBlock { label: original.label.clone(), ops: Default::default() };
        for insn in block.insns.iter() {
            rebuilt.ops.push(back(template, insn.what.as_ref().expect("a selected instruction"))?);
        }
        function.blocks.push(rebuilt);
    }
    Ok(function)
}

thread_local! {
    static CHECK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes the driver check the round trip of every function before allocation.
pub fn set_check(on: bool) {
    CHECK.with(|check| check.set(on));
}

pub(crate) fn checking() -> bool {
    CHECK.with(std::cell::Cell::get)
}

/// Checks MFunc -> LirBody -> MFunc is the identity (operations and their
/// places aside, which the body does not carry yet).
pub fn check_round_trip(function: &MFunc) -> Result<(), String> {
    let body = to_lir(function).map_err(|Unmapped(form)| format!("@{}: unmapped form {form}", function.name))?;
    let again =
        from_lir(&body, function).map_err(|Unmapped(form)| format!("@{}: no way back for {form}", function.name))?;
    for (a, b) in function.blocks.iter().zip(&again.blocks) {
        if a.ops[..] != b.ops[..] {
            return Err(format!("@{}: block {} differs after the round trip", function.name, a.label));
        }
    }
    Ok(())
}
