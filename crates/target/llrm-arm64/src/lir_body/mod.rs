//! The selector's output as the shared flow's [`LirBody`], and the way back
//! (gap 1 of `docs/architecture/arm64-core-passes.md`). A form with no mapping
//! is [`Unmapped`], named by its variant, so the first missing form of a
//! program is the next one to write.
//!
//! Mapping: a virtual register is `Loc::Held` (value = its number), a machine
//! register `Loc::Reg`; the instruction's name is LLVM's opcode (`ADDXri`), its
//! width the letter before the form. A symbol is an `Addr` of `Space::External`
//! (a label of `Space::Segment`) whose `index` is its place in
//! [`Lowered::symbols`]; a block is its position in the function.
//! The class of a `COPY`'s registers is in its name (`COPY`, `COPY_FPR`) until
//! `LirBody::class_like` carries it.

mod control;
mod float;
mod integer;
mod memory;

use std::collections::HashMap;
use std::sync::Arc;

use llrm_codegen::model::lir::{Insn, LirBlock, LirBody};
use llrm_codegen::support::hash::IndexMap;
use llrm_lir::registers::RegId;
use llrm_lir::{Addr as LirAddr, AddressRef, Held, Imm, Loc, Mem, Operation, Reg as LirRegLoc, Semantics, Space};
use llrm_spike_mf::mfunc::Reg as MfReg;

use crate::lir_view::{from_lir as machine_register, to_lir as lir_register};
use crate::mc::inst::{Gpr, Inst, MemOperand, Shift, Target, Vreg, Width};
use crate::mc::visit::Reg;
use crate::mfunc::{MBlock, MFunc, Op, successors_of};

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

fn gpr_loc(
    register: Gpr,
    width: Width,
) -> Loc {
    loc(Reg::Gpr(register), width.bits())
}

fn vreg_loc(
    register: Vreg,
    bits: u32,
) -> Loc {
    loc(Reg::Fpr(register), bits)
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

fn bits_of(loc: &Loc) -> u32 {
    match loc {
        Loc::Reg(register) => register.width,
        Loc::Held(held) => held.width,
        other => panic!("a register, found {other:?}"),
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

fn vreg_of(loc: &Loc) -> Vreg {
    match reg_of(loc, true) {
        Reg::Fpr(register) => register,
        Reg::Gpr(_) => panic!("a float register"),
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

/// `one`'s place in `all`, the number the body carries a closed set's member
/// as.
fn position<T: PartialEq>(
    all: &[T],
    one: T,
) -> i64 {
    all.iter().position(|each| *each == one).expect("a listed variant") as i64
}

fn member<T: Copy>(
    all: &[T],
    loc: &Loc,
) -> T {
    all[value_of(loc) as usize]
}

/// `name` as its mnemonic, the width letter and the form: `ADDXri` is (`ADD`,
/// `X`, `ri`).
fn split(name: &str) -> (&str, char, &str) {
    let at = name.find(|c: char| c.is_ascii_lowercase()).unwrap_or(name.len());
    let (head, form) = name.split_at(at);
    let (mnemonic, last) = head.split_at(head.len().saturating_sub(1));
    (mnemonic, last.chars().next().unwrap_or(' '), form)
}

fn width_letter(width: Width) -> char {
    if width == Width::X { 'X' } else { 'W' }
}

fn width_named(letter: char) -> Width {
    if letter == 'X' { Width::X } else { Width::W }
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

/// What the way into the body needs to know of the function.
struct To<'a> {
    function: &'a MFunc,
    symbols: &'a mut Vec<String>,
}

impl To<'_> {
    fn block(
        &self,
        target: &Target,
    ) -> Result<i64, Unmapped> {
        match target {
            Target::Label(label) => self
                .function
                .blocks
                .iter()
                .position(|block| &block.label == label)
                .map(|at| at as i64)
                .ok_or_else(|| Unmapped(format!("branch to unknown label {label}"))),
            other => Err(Unmapped(format!("Target::{other:?}"))),
        }
    }

    fn symbol(
        &mut self,
        name: &str,
    ) -> i64 {
        let at = self
            .symbols
            .iter()
            .position(|each| each == name)
            .unwrap_or_else(
                || {
                    self.symbols.push(name.to_owned());
                    self.symbols.len() - 1
                },
            );
        at as i64
    }

    /// The place a symbol or label names, as an address: the symbol's number is
    /// the `index`, the addend the displacement.
    fn address(
        &mut self,
        target: &Target,
    ) -> Result<AddressRef, Unmapped> {
        let (space, name, addend) = match target {
            Target::Symbol { name, addend } => (Space::External, name, *addend),
            Target::Label(label) => (Space::Segment, label, 0),
            other => return Err(Unmapped(format!("Target::{other:?}"))),
        };
        let mut at = LirAddr::new(space, addend);
        at.index = self.symbol(name);
        Ok(AddressRef::new(Some(at)))
    }
}

/// What the way back needs: the function the body came from, its symbols and
/// what the body does not carry yet for the instruction at hand.
struct Back<'a> {
    template: &'a MFunc,
    symbols: &'a [String],
    mmo: Option<&'a MemOperand>,
}

impl Back<'_> {
    fn block(
        &self,
        at: i64,
    ) -> Target {
        Target::Label(self.template.blocks[at as usize].label.clone())
    }

    fn target(
        &self,
        address: &AddressRef,
    ) -> Target {
        let at = address.addr.expect("a symbol address");
        let name = self.symbols[at.index as usize].clone();
        match at.space {
            Space::Segment => Target::Label(name),
            _ => Target::Symbol { name, addend: at.disp },
        }
    }

    fn target_of(
        &self,
        loc: &Loc,
    ) -> Target {
        match loc {
            Loc::Address(address) => self.target(address),
            other => panic!("a symbol address, found {other:?}"),
        }
    }
}

fn one(
    cx: &mut To,
    op: &Op,
) -> Result<Semantics, Unmapped> {
    if let Some(what) = common(op) {
        return Ok(what);
    }
    let found = match op {
        Op::I(inst) => match integer::to(cx, inst)? {
            Some(what) => Some(what),
            None => match memory::to(cx, inst)? {
                Some(what) => Some(what),
                None => match float::to(inst) {
                    Some(what) => Some(what),
                    None => control::to_inst(cx, inst)?,
                },
            },
        },
        other => control::to_op(cx, other)?,
    };
    found.ok_or_else(|| match op {
        Op::I(inst) => Unmapped(crate::lir_view::name_of(inst).to_owned()),
        other => Unmapped(format!("{other:?}").split([' ', '{']).next().unwrap_or_default().to_owned()),
    })
}

/// `COPY` and the immediate move: pseudo operations of no group.
fn common(op: &Op) -> Option<Semantics> {
    Some(match op {
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
        _ => return None,
    })
}

fn back(
    cx: &Back,
    what: &Semantics,
) -> Result<Op, Unmapped> {
    let name = what.name.as_deref().unwrap_or("");
    if let Some(op) = common_back(what, name) {
        return Ok(op);
    }
    integer::back(cx, what)
        .map(Op::I)
        .or_else(|| memory::back(cx, what))
        .or_else(|| float::back(what).map(Op::I))
        .or_else(|| control::back(cx, what))
        .ok_or_else(|| Unmapped(name.to_owned()))
}

fn common_back(
    what: &Semantics,
    name: &str,
) -> Option<Op> {
    let (dest, source) = (what.dests.first(), what.sources.first());
    Some(match name {
        "COPY" | "COPY_FPR" => {
            let fpr = name == "COPY_FPR";
            Op::Copy { dst: reg_of(dest?, fpr), src: reg_of(source?, fpr) }
        }
        "MOVImm" => Op::Imm { dst: gpr_of(dest?), bits: bits_of(dest?), value: value_of(source?) as u64 },
        _ => return None,
    })
}

/// The body and what it does not carry yet: each load and store's
/// `MachineMemOperand`, the symbols its addresses name and each debug note's
/// variable, all by instruction (`at`).
pub struct Lowered {
    pub body: LirBody,
    pub memory_operands: HashMap<i64, MemOperand>,
    pub symbols: Vec<String>,
    pub debug_values: HashMap<i64, (llrm_mir::MetadataId, Option<Reg>)>,
}

/// The values (by number) an instruction defines and reads.
fn values_of(what: &Semantics) -> (Vec<u32>, Vec<u32>) {
    let held = |loc: &Loc, out: &mut Vec<u32>| match loc {
        Loc::Held(held) => out.push(held.value),
        Loc::Mem(cell) => out.extend(cell.base.iter().chain(&cell.index).map(|held| held.value)),
        _ => {}
    };
    let (mut defines, mut uses) = (Vec::new(), Vec::new());
    what.dests.iter().for_each(|loc| held(loc, &mut defines));
    what.sources.iter().for_each(|loc| held(loc, &mut uses));
    defines.dedup();
    uses.dedup();
    (defines, uses)
}

/// The function's operations as a body: block `n` is `at` = `n`.
pub fn to_lir(function: &MFunc) -> Result<Lowered, Unmapped> {
    let mut memory_operands = HashMap::new();
    let mut debug_values = HashMap::new();
    let mut symbols = Vec::new();
    let labels: rustc_hash::FxHashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let mut at = 0_i64;
    let mut blocks = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::new();
        for op in block.ops.iter() {
            let what = one(&mut To { function, symbols: &mut symbols }, op)?;
            let (defines, uses) = values_of(&what);
            let mut insn = Insn::new(at, None, Some(what), defines, uses);
            match op {
                Op::I(Inst::Load { mmo, .. } | Inst::Store { mmo, .. }) => {
                    insn.volatile = mmo.volatile;
                    insn.invariant = mmo.object == crate::mc::inst::MemObject::ConstantPool;
                    memory_operands.insert(at, mmo.clone());
                }
                Op::DbgValue { variable, reg } => {
                    debug_values.insert(at, (*variable, *reg));
                }
                Op::Call { .. } => insn.clobbers = crate::lir_view::caller_saved().into_iter().collect(),
                _ => {}
            }
            insns.push(Arc::new(insn));
            at += 1;
        }
        let mut lowered = LirBlock::new(index as i64, insns);
        lowered.succ.extend(successors_of(block, index, function.blocks.len(), &labels).into_iter().map(|b| b as i64));
        blocks.push(lowered.into());
    }
    let body = LirBody::new(function.name.clone(), 0, blocks, IndexMap::default(), IndexMap::default());
    Ok(Lowered { body, memory_operands, symbols, debug_values })
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
            if what.name.as_deref() == Some("DBG_VALUE") {
                let (variable, reg) = lowered.debug_values[&insn.at];
                rebuilt.ops.push(Op::DbgValue { variable, reg });
                continue;
            }
            let cx = Back { template, symbols: &lowered.symbols, mmo: lowered.memory_operands.get(&insn.at) };
            rebuilt.ops.push(back(&cx, what)?);
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

static CHECKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The operations whose round trip [`check_round_trip`] has compared, so far in
/// the process: a check that compares nothing passes, so a test reads this.
pub fn checked_operations() -> u64 {
    CHECKED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Checks MFunc -> LirBody -> MFunc is the identity: every operation, with its
/// registers, comes back as it went.
pub fn check_round_trip(function: &MFunc) -> Result<(), String> {
    let lowered = to_lir(function).map_err(|Unmapped(form)| format!("@{}: unmapped form {form}", function.name))?;
    let again =
        from_lir(&lowered, function).map_err(|Unmapped(form)| format!("@{}: no way back for {form}", function.name))?;
    let total: usize = function.blocks.iter().map(|block| block.ops.len()).sum();
    CHECKED.fetch_add(total as u64, std::sync::atomic::Ordering::Relaxed);
    for (a, b) in function.blocks.iter().zip(&again.blocks) {
        if let Some(at) = (0..a.ops.len().max(b.ops.len())).find(|&at| a.ops.get(at) != b.ops.get(at)) {
            return Err(format!(
                "@{}: block {} differs at operation {at} after the round trip: {:?} became {:?}",
                function.name,
                a.label,
                a.ops.get(at),
                b.ops.get(at)
            ));
        }
    }
    Ok(())
}
