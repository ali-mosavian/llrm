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
use llrm_lir::registers::RegId;
use llrm_lir::{Addr as LirAddr, Held, Imm, Loc, Mem, Operation, Reg as LirRegLoc, Semantics, Space};
use llrm_spike_mf::mfunc::Reg as MfReg;

use crate::lir_view::{from_lir as machine_register, to_lir as lir_register};
use crate::mc::inst::{AddSub, Addr, Extend, Gpr, Inst, MemOperand, Memory, Shift, Target, Vreg, Width};
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

const SHIFTS: [Shift; 4] = [Shift::Lsl, Shift::Lsr, Shift::Asr, Shift::Ror];
const EXTENDS: [Extend; 8] =
    [Extend::Uxtb, Extend::Uxth, Extend::Uxtw, Extend::Uxtx, Extend::Sxtb, Extend::Sxth, Extend::Sxtw, Extend::Sxtx];

fn position<T: PartialEq>(
    all: &[T],
    one: T,
) -> i64 {
    all.iter().position(|each| *each == one).expect("a listed variant") as i64
}

/// A general register as a memory operand's base or index: a machine register
/// is the cell's `through`, a virtual one its `Held` value.
fn place(register: Gpr) -> (RegId, Option<Held>) {
    match lir_register(Reg::Gpr(register)) {
        MfReg::Phys(register) => (register, None),
        MfReg::Virt(value) => (RegId::None, Some(Held { value, width: 64, offset: 0 })),
    }
}

fn unplace(
    through: RegId,
    held: Option<Held>,
) -> Gpr {
    gpr_of(&match held {
        Some(held) => Loc::Held(held),
        None => Loc::Reg(LirRegLoc { register: through, width: 64 }),
    })
}

/// The cell an address names, the suffix of the opcode name that says its
/// form, and the base register a pre/post form writes back.
fn cell(addr: &Addr) -> Result<(Mem, String, Option<Gpr>), Unmapped> {
    let mut cell = Mem::new(None, 64);
    let mut base_of = |cell: &mut Mem, base: Gpr| {
        (cell.through, cell.base) = place(base);
    };
    Ok(match addr {
        Addr::Offset { base, imm } => {
            base_of(&mut cell, *base);
            cell.offset = *imm;
            (cell, "ui".into(), None)
        }
        Addr::Pre { base, imm } => {
            base_of(&mut cell, *base);
            cell.offset = *imm;
            (cell, "pre".into(), Some(*base))
        }
        Addr::Post { base, imm } => {
            base_of(&mut cell, *base);
            cell.offset = *imm;
            (cell, "post".into(), Some(*base))
        }
        Addr::Register { base, index, extend, shifted } => {
            base_of(&mut cell, *base);
            (cell.index_through, cell.index) = place(*index);
            (cell, format!("ro{}{}", position(&EXTENDS, *extend), u8::from(*shifted)), None)
        }
        Addr::Slot { slot, offset } => {
            cell.addr = Some(LirAddr::new(Space::Frame, *offset).in_slot(i64::from(*slot)));
            (cell, "fi".into(), None)
        }
        Addr::PageOffset { .. } => return Err(Unmapped("Addr::PageOffset".into())),
    })
}

fn uncell(
    cell: &Mem,
    suffix: &str,
) -> Addr {
    let base = || unplace(cell.through, cell.base);
    match suffix {
        "ui" => Addr::Offset { base: base(), imm: cell.offset },
        "pre" => Addr::Pre { base: base(), imm: cell.offset },
        "post" => Addr::Post { base: base(), imm: cell.offset },
        "fi" => {
            let at = cell.addr.expect("a frame cell");
            Addr::Slot { slot: at.slot_home().expect("a slot") as u32, offset: at.disp }
        }
        ro => {
            let digits = ro.strip_prefix("ro").expect("a register form").as_bytes();
            Addr::Register {
                base: base(),
                index: unplace(cell.index_through, cell.index),
                extend: EXTENDS[usize::from(digits[0] - b'0')],
                shifted: digits[1] == b'1',
            }
        }
    }
}

/// `LDR`/`STR` with the access: `S` sign-extending, `B`/`H`/`W`/`X` the bytes
/// of an integer, `F1`..`F16` of a float register.
fn access_name(
    load: bool,
    memory: &Memory,
    suffix: &str,
) -> String {
    let (mnemonic, kind) = match memory {
        Memory::Int { bytes, sign, .. } => (
            if load { "LDR" } else { "STR" },
            format!(
                "{}{}",
                if *sign { "S" } else { "" },
                match bytes {
                    1 => "B",
                    2 => "H",
                    4 => "W",
                    _ => "X",
                }
            ),
        ),
        Memory::Fp { bytes, .. } => (if load { "LDR" } else { "STR" }, format!("F{bytes}")),
    };
    format!("{mnemonic}{kind}.{suffix}")
}

fn memory_loc(memory: &Memory) -> Loc {
    match memory {
        Memory::Int { width, reg, .. } => loc(Reg::Gpr(*reg), width.bits()),
        Memory::Fp { bytes, reg } => loc(Reg::Fpr(*reg), u32::from(*bytes) * 8),
    }
}

/// The register a load or store moves, rebuilt from its name and operand.
fn memory_of(
    kind: &str,
    place: &Loc,
) -> Memory {
    if let Some(bytes) = kind.strip_prefix('F') {
        let Reg::Fpr(reg) = reg_of(place, true) else { panic!("a float register") };
        return Memory::Fp { bytes: bytes.parse().expect("float bytes"), reg };
    }
    let sign = kind.starts_with('S');
    let bytes = match kind.trim_start_matches('S') {
        "B" => 1,
        "H" => 2,
        "W" => 4,
        _ => 8,
    };
    let width = match place {
        Loc::Reg(r) => r.width,
        Loc::Held(h) => h.width,
        other => panic!("a register, found {other:?}"),
    };
    Memory::Int { width: if width == 64 { Width::X } else { Width::W }, bytes, sign, reg: gpr_of(place) }
}

fn add_sub_name(
    op: AddSub,
    width: Width,
    form: &str,
) -> String {
    let w = if width == Width::X { 'X' } else { 'W' };
    format!("{}{}{w}{form}", if op.subtract { "SUB" } else { "ADD" }, if op.set_flags { "S" } else { "" })
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
            add_sub_name(*op, *width, "ri"),
            vec![loc(Reg::Gpr(*rd), width.bits())],
            vec![loc(Reg::Gpr(*rn), width.bits()), imm(i64::from(*imm12)), imm(i64::from(*shift12))],
            None,
        ),
        Op::I(Inst::AddSubShifted { op, width, rd, rn, rm, shift, amount }) => semantics(
            Operation::Binary,
            add_sub_name(*op, *width, "rs"),
            vec![loc(Reg::Gpr(*rd), width.bits())],
            vec![
                loc(Reg::Gpr(*rn), width.bits()),
                loc(Reg::Gpr(*rm), width.bits()),
                imm(position(&SHIFTS, *shift)),
                imm(i64::from(*amount)),
            ],
            None,
        ),
        Op::I(Inst::CompareBranch { nonzero, width, rt, target }) => semantics(
            Operation::Branch,
            format!("CB{}Z{}", if *nonzero { "N" } else { "" }, if *width == Width::X { 'X' } else { 'W' }),
            vec![],
            vec![loc(Reg::Gpr(*rt), width.bits())],
            Some(block_of(function, target)?),
        ),
        Op::I(Inst::Load { mem, addr, .. }) => {
            let (cell, suffix, written) = cell(addr)?;
            let mut dests = vec![memory_loc(mem)];
            dests.extend(written.map(|base| loc(Reg::Gpr(base), 64)));
            semantics(Operation::Move, access_name(true, mem, &suffix), dests, vec![Loc::Mem(cell)], None)
        }
        Op::I(Inst::Store { mem, addr, .. }) => {
            let (cell, suffix, written) = cell(addr)?;
            let dests = written.map(|base| loc(Reg::Gpr(base), 64)).into_iter().collect();
            semantics(
                Operation::Move,
                access_name(false, mem, &suffix),
                dests,
                vec![memory_loc(mem), Loc::Mem(cell)],
                None,
            )
        }
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

/// The body and what it does not carry yet: each load and store's
/// `MachineMemOperand`, by instruction.
pub struct Lowered {
    pub body: LirBody,
    pub memory_operands: std::collections::HashMap<i64, MemOperand>,
}

/// The function's operations as a body: block `n` is `at` = `n`.
pub fn to_lir(function: &MFunc) -> Result<Lowered, Unmapped> {
    let mut memory_operands = std::collections::HashMap::new();
    let mut at = 0_i64;
    let mut blocks = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::new();
        for op in block.ops.iter() {
            let what = one(function, op)?;
            if let Op::I(Inst::Load { mmo, .. } | Inst::Store { mmo, .. }) = op {
                memory_operands.insert(at, mmo.clone());
            }
            insns.push(Arc::new(Insn::new(at, None, Some(what), vec![], vec![])));
            at += 1;
        }
        blocks.push(LirBlock::new(index as i64, insns).into());
    }
    let body = LirBody::new(function.name.clone(), 0, blocks, IndexMap::default(), IndexMap::default());
    Ok(Lowered { body, memory_operands })
}

fn back(
    template: &MFunc,
    what: &Semantics,
    mmo: Option<&MemOperand>,
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
        (Operation::Binary, name) if name.ends_with("rs") && (name.starts_with("ADD") || name.starts_with("SUB")) => {
            let width = if name.ends_with("Xrs") { Width::X } else { Width::W };
            let op = AddSub { subtract: name.starts_with("SUB"), set_flags: name.as_bytes()[3] == b'S' };
            Op::I(Inst::AddSubShifted {
                op,
                width,
                rd: gpr_of(dest.unwrap()),
                rn: gpr_of(source.unwrap()),
                rm: gpr_of(&what.sources[1]),
                shift: SHIFTS[value_of(&what.sources[2]) as usize],
                amount: value_of(&what.sources[3]) as u8,
            })
        }
        (Operation::Branch, name) if name.starts_with("CB") => {
            let label = template.blocks[what.target.unwrap() as usize].label.clone();
            Op::I(Inst::CompareBranch {
                nonzero: name.starts_with("CBN"),
                width: if name.ends_with('X') { Width::X } else { Width::W },
                rt: gpr_of(source.unwrap()),
                target: Target::Label(label),
            })
        }
        (Operation::Move, name) if name.starts_with("LDR") || name.starts_with("STR") => {
            let load = name.starts_with("LDR");
            let (kind, suffix) = name[3..].split_once('.').expect("a form suffix");
            let Some(Loc::Mem(cell)) = what.sources.last() else { panic!("a memory operand") };
            let place = if load { dest.unwrap() } else { source.unwrap() };
            let (mem, addr, mmo) =
                (memory_of(kind, place), uncell(cell, suffix), mmo.expect("a memory operand").clone());
            Op::I(if load { Inst::Load { mem, addr, mmo } } else { Inst::Store { mem, addr, mmo } })
        }
        _ => return Err(Unmapped(name.to_owned())),
    })
}

/// `template` with its blocks' operations replaced by the body's: everything
/// that is not an operation (labels, frame objects, pool) is the template's.
pub fn from_lir(
    lowered: &Lowered,
    template: &MFunc,
) -> Result<MFunc, Unmapped> {
    let mut function = template.clone();
    function.blocks = Vec::new();
    for (block, original) in lowered.body.blocks.iter().zip(&template.blocks) {
        let mut rebuilt = MBlock { label: original.label.clone(), ops: Default::default() };
        for insn in block.insns.iter() {
            let what = insn.what.as_ref().expect("a selected instruction");
            rebuilt.ops.push(back(template, what, lowered.memory_operands.get(&insn.at))?);
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
    let lowered = to_lir(function).map_err(|Unmapped(form)| format!("@{}: unmapped form {form}", function.name))?;
    let again =
        from_lir(&lowered, function).map_err(|Unmapped(form)| format!("@{}: no way back for {form}", function.name))?;
    for (a, b) in function.blocks.iter().zip(&again.blocks) {
        if a.ops[..] != b.ops[..] {
            return Err(format!("@{}: block {} differs after the round trip", function.name, a.label));
        }
    }
    Ok(())
}
