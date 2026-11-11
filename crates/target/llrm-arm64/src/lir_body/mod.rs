//! The selector's output as the shared flow's [`LirBody`], and the way back
//! (gap 1 of `docs/architecture/arm64-core-passes.md`). A form with no mapping
//! is [`Unmapped`], named by its variant, so the first missing form of a
//! program is the next one to write.
//!
//! Mapping: a virtual register is `Loc::Held` (value = its number), a machine
//! register `Loc::Reg`; the instruction's name is LLVM's opcode (`ADDXri`), its
//! width the letter before the form. A symbol is an `Addr` of `Space::External`
//! (a label of `Space::Segment`) whose `index` is its place in
//! [`LirBody::symbols`]; a block is its position in the function. A load or
//! store carries what it reaches as [`Insn::memory`] (its `MachineMemOperand`).
//! The class of a `COPY`'s registers is in its name (`COPY`, `COPY_FPR`) until
//! `LirBody::class_like` carries it. A location's width is in bytes, as the
//! shared phases read it (the verifier compares it with the register's bytes);
//! the producer's `bits` are converted at `loc`, `imm` and `bits_of`.

mod control;
mod float;
mod integer;
mod memory;
#[cfg(test)]
mod opcode_tests;
pub mod stages;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use llrm_codegen::backend::lanes::{Lanes, enter};
use llrm_analysis::placealias::Object;
use llrm_codegen::model::lir::{Insn, LirBlock, LirBody};
use llrm_codegen::model::memref::MemRef;
use llrm_codegen::support::hash::IndexMap;
use llrm_lir::registers::{RegId, Regs};
use llrm_lir::{Addr as LirAddr, AddressRef, Held, Imm, Loc, Mem, Operation, Reg as LirRegLoc, Semantics, Space};
use llrm_spike_mf::mfunc::Reg as MfReg;

use crate::lir_view::{from_lir as machine_register, to_lir as lir_register};
use crate::mc::inst::{Gpr, Inst, MemObject, MemOperand, Memory, Shift, Target, Vreg, Width};
use crate::mc::visit::{Reg, RegList};
use crate::mfunc::{MBlock, MFunc, Op, successors_of};

/// An operation the producer has no mapping for yet: its variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unmapped(pub String);

fn loc(
    register: Reg,
    bits: u32,
) -> Loc {
    match lir_register(register) {
        MfReg::Phys(register) => Loc::Reg(LirRegLoc { register: viewed(register, bits), width: bits / 8 }),
        MfReg::Virt(value) => Loc::Held(Held { value, width: bits / 8, offset: 0 }),
    }
}

/// The register of `root` that is `bits` wide (`w3` for `x3` at 32): the
/// file has one id per width, as the pin's `W3` is its own register.
fn viewed(
    root: llrm_lir::registers::RegId,
    bits: u32,
) -> llrm_lir::registers::RegId {
    Regs(&crate::REGISTER_INFO).view(root, bits).unwrap_or(root)
}

/// The register a view is of: the id the machine register numbers go by.
fn rooted(register: llrm_lir::registers::RegId) -> llrm_lir::registers::RegId {
    Regs(&crate::REGISTER_INFO).root(register)
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
    Loc::Imm(Imm { value, width: 8, address: None })
}

fn value_of(loc: &Loc) -> i64 {
    match loc {
        Loc::Imm(imm) => imm.value,
        other => panic!("an immediate operand, found {other:?}"),
    }
}

fn bits_of(loc: &Loc) -> u32 {
    match loc {
        Loc::Reg(register) => register.width * 8,
        Loc::Held(held) => held.width * 8,
        other => panic!("a register, found {other:?}"),
    }
}

fn gpr_of(loc: &Loc) -> Gpr {
    match loc {
        Loc::Reg(LirRegLoc { register, .. }) => from_gpr(MfReg::Phys(rooted(*register))),
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
        Loc::Reg(LirRegLoc { register, .. }) => machine_register(MfReg::Phys(rooted(*register)), class),
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
        tied: None,
    }
}

/// What the way into the body needs to know of the function.
struct To<'a> {
    function: &'a MFunc,
    symbols: &'a mut Vec<String>,
    /// The float registers that hold 128 bits.
    wide: &'a [Reg],
}

impl To<'_> {
    /// The bits a copy, call or return moves of `register`.
    fn bits(
        &self,
        register: Reg,
    ) -> u32 {
        if self.wide.contains(&register) { 128 } else { 64 }
    }

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
        intern(self.symbols, name)
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

/// The number of `name` in `symbols`, added if it has none.
pub(crate) fn intern(
    symbols: &mut Vec<String>,
    name: &str,
) -> i64 {
    let at = symbols.iter().position(|each| each == name).unwrap_or_else(|| {
        symbols.push(name.to_owned());
        symbols.len() - 1
    });
    at as i64
}

/// What a load or store reaches, as the body carries it (`Insn::memory`).
pub(crate) fn memref_of(
    mem: &Memory,
    mmo: &MemOperand,
    store: bool,
    symbols: &mut Vec<String>,
) -> MemRef {
    let object = match &mmo.object {
        MemObject::Unknown => None,
        MemObject::Global(name) => Some(Object::Global(intern(symbols, name) as u32)),
        MemObject::Slot(slot) => Some(Object::Frame { id: i64::from(*slot), fixed: false }),
        MemObject::ConstantPool => Some(Object::Pool),
    };
    let (Memory::Int { bytes, .. } | Memory::Fp { bytes, .. }) = mem;
    MemRef { object, offset: mmo.offset, bytes: Some(u64::from(*bytes)), store }
}

/// The operand a load or store was made from, back from what the body carries.
fn mmo_of(
    insn: &Insn,
    symbols: &[String],
) -> Option<MemOperand> {
    let memory = insn.memory.as_ref()?;
    let object = match memory.object {
        None => MemObject::Unknown,
        Some(Object::Global(at)) => MemObject::Global(symbols[at as usize].clone()),
        Some(Object::Frame { id, .. }) => MemObject::Slot(id as u32),
        Some(Object::Pool) => MemObject::ConstantPool,
    };
    Some(MemOperand { object, offset: memory.offset, volatile: insn.volatile })
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

/// The selected form of `op`, as the shared scheduler reads an instruction
/// (`one`): none for an operation with no mapping yet.
pub(crate) fn form_of(
    function: &MFunc,
    symbols: &mut Vec<String>,
    wide: &[Reg],
    op: &Op,
) -> Option<Semantics> {
    one(&mut To { function, symbols, wide }, op).ok()
}

fn one(
    cx: &mut To,
    op: &Op,
) -> Result<Semantics, Unmapped> {
    if let Some(what) = common(cx, op) {
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
    let refused = found
        .as_ref()
        .and_then(|what| what.name.as_deref())
        .is_some_and(
            |name| REFUSED.with(|refused| refused.borrow().as_deref().is_some_and(|prefix| name.starts_with(prefix))),
        );
    found
        .filter(|_| !refused)
        .ok_or_else(
            || match op {
                Op::I(inst) => Unmapped(crate::lir_view::name_of(inst).to_owned()),
                other => Unmapped(format!("{other:?}").split([' ', '{']).next().unwrap_or_default().to_owned()),
            },
        )
}

/// `COPY` and the immediate move: pseudo operations of no group.
fn common(
    cx: &To,
    op: &Op,
) -> Option<Semantics> {
    Some(match op {
        Op::Copy { dst, src } => {
            let fpr = matches!(dst, Reg::Fpr(_));
            semantics(
                Operation::Move,
                if fpr { "COPY_FPR" } else { "COPY" },
                vec![loc(*dst, cx.bits(*dst))],
                vec![loc(*src, cx.bits(*src))],
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

/// The body and what it does not carry yet: each debug note's variable, by
/// instruction (`at`).
pub struct Lowered {
    pub body: LirBody,
    pub debug_values: HashMap<i64, (llrm_mir::MetadataId, Option<Reg>)>,
    /// A writeback's new base value, by number, and the register it is the
    /// updated value of (`$wback = LDRXpost %base`): the way back puts it back.
    pub tied: HashMap<u32, u32>,
}

/// A base register the function sets by a copy and then updates by a
/// pre/post-indexed access in the same block, with nothing reading it between:
/// it is one register twice defined. The body gives the access's result, and
/// every later reader, a value of its own (the pin's `$wback`, tied to the
/// base). By register number, the number of the new value.
fn writeback_ties(function: &MFunc) -> HashMap<u32, u32> {
    use crate::mc::inst::Addr;
    let number = |register: Reg| match lir_register(register) {
        MfReg::Virt(value) => Some(value),
        MfReg::Phys(_) => None,
    };
    let mut definitions: HashMap<u32, usize> = HashMap::new();
    for op in function.blocks.iter().flat_map(|block| block.ops.iter()) {
        for register in op.defs_and_uses().0.into_iter().filter_map(number) {
            *definitions.entry(register).or_default() += 1;
        }
    }
    let mut next = function.next_register - Gpr::FIRST_VIRTUAL;
    let mut ties = HashMap::new();
    for block in &function.blocks {
        // The registers a copy set earlier in this block that nothing has read
        // or written since.
        let mut fresh: std::collections::HashSet<u32> = Default::default();
        for op in block.ops.iter() {
            let base = match op {
                Op::I(
                    Inst::Load { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. }
                    | Inst::Store { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. }
                    | Inst::LoadPair { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. }
                    | Inst::StorePair { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. },
                ) => number(Reg::Gpr(*base)),
                _ => None,
            };
            let (defs, uses) = op.defs_and_uses();
            if let Some(base) = base
                && fresh.contains(&base)
                && definitions.get(&base) == Some(&2)
                && uses.iter().filter_map(|one| number(*one)).filter(|one| *one == base).count() == 1
            {
                ties.insert(base, next);
                next += 1;
            }
            for register in defs.iter().chain(&uses).filter_map(|one| number(*one)) {
                fresh.remove(&register);
            }
            if let Op::Copy { dst, .. } = op
                && let Some(register) = number(*dst)
            {
                fresh.insert(register);
            }
        }
    }
    ties
}

fn renumber(
    place: &mut Loc,
    map: &HashMap<u32, u32>,
) {
    let held = |held: &mut Held| {
        if let Some(&to) = map.get(&held.value) {
            held.value = to;
        }
    };
    match place {
        Loc::Held(one) => held(one),
        Loc::Mem(cell) => {
            cell.base.iter_mut().for_each(held);
            cell.index.iter_mut().for_each(held);
        }
        _ => {}
    }
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

/// What an operation reads and writes, as the shared passes' lanes (the file's
/// lanes must be entered): the machine registers it names, the status flags
/// (`NZCV`, one lane as LLVM's one physical register) where it reads or sets
/// them, and memory (one more lane) where it loads or stores. DEVIATE MS6's
/// model, the one the allocator's view states; exit with it (gap 3).
fn effects(
    op: &Op,
    facts: &crate::facts::Facts,
) -> (Lanes, Lanes) {
    let registers = Regs(&crate::REGISTER_INFO);
    let lanes = |list: RegList| {
        list.into_iter()
            .filter_map(|register| match lir_register(register) {
                // The zero register is no lane: nothing writes it.
                MfReg::Phys(id) if id != crate::reg::XZR && registers.known(id) => Some(Lanes::span(id, 0, 1)),
                _ => None,
            })
            .fold(Lanes::new(), |all, one| all.or(&one))
    };
    let (defs, uses) = op.defs_and_uses();
    let (mut reads, mut writes) = (lanes(uses), lanes(defs));
    let (flags, memory) = (Lanes::flags(1), Lanes::flags(Lanes::MEMORY));
    if facts.reads_flags {
        reads.extend(flags.iter().copied());
    }
    if facts.sets_flags {
        writes.extend(flags.iter().copied());
    }
    if facts.may_load || facts.may_store || facts.call {
        reads.extend(memory.iter().copied());
    }
    if facts.may_store || facts.call {
        writes.extend(memory.iter().copied());
    }
    (reads, writes)
}

/// The function's operations as a body: block `n` is `at` = `n`.
pub fn to_lir(function: &MFunc) -> Result<Lowered, Unmapped> {
    let mut debug_values = HashMap::new();
    let mut symbols = Vec::new();
    let labels: rustc_hash::FxHashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let wide: Vec<Reg> = crate::regalloc::wide_registers(function).into_iter().collect();
    let _file = enter(Regs(&crate::REGISTER_INFO));
    let ties = writeback_ties(function);
    // The copy that makes the base and the access that updates it keep the
    // register's number (the value before); all else gets the new value.
    let mut keeps: std::collections::HashSet<(usize, usize)> = Default::default();
    let mut updates: std::collections::HashSet<(usize, usize)> = Default::default();
    if !ties.is_empty() {
        for (index, block) in function.blocks.iter().enumerate() {
            let mut copied: HashMap<u32, usize> = HashMap::new();
            for (place, op) in block.ops.iter().enumerate() {
                let (defs, uses) = op.defs_and_uses();
                let numbers = |list: &[Reg]| -> Vec<u32> {
                    list.iter()
                        .filter_map(|one| match lir_register(*one) {
                            MfReg::Virt(value) => Some(value),
                            MfReg::Phys(_) => None,
                        })
                        .collect()
                };
                match op {
                    Op::Copy { .. } => {
                        if let Some(&value) = numbers(&defs).first().filter(|value| ties.contains_key(value)) {
                            copied.insert(value, place);
                        }
                    }
                    _ => {
                        for value in numbers(&defs).into_iter().filter(|value| ties.contains_key(value)) {
                            if let Some(copy) = copied.remove(&value)
                                && numbers(&uses).contains(&value)
                            {
                                keeps.insert((index, copy));
                                updates.insert((index, place));
                            }
                        }
                    }
                }
            }
        }
    }
    let mut at = 0_i64;
    let mut blocks = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::new();
        let mut phis = Vec::new();
        for (place, op) in block.ops.iter().enumerate() {
            // A phi is the block's, not an instruction (`LirBlock::phis`): its
            // result and each incoming value are held values, the predecessor
            // a block's position.
            if let Op::Phi { dst, incoming } = op {
                let held = |register: Reg| match lir_register(register) {
                    MfReg::Virt(value) => Ok(value),
                    MfReg::Phys(_) => Err(Unmapped("Phi of a machine register".to_owned())),
                };
                let incoming = incoming
                    .iter()
                    .map(|(value, from)| {
                        let from = *labels
                            .get(from.as_str())
                            .ok_or_else(|| Unmapped(format!("phi from unknown label {from}")))?;
                        Ok((from as i64, held(*value)?))
                    })
                    .collect::<Result<Vec<_>, Unmapped>>()?;
                phis.push(llrm_codegen::model::lir::Phi { result: held(*dst)?, incoming });
                continue;
            }
            let mut what = one(&mut To { function, symbols: &mut symbols, wide: &wide }, op)?;
            if !ties.is_empty() && !keeps.contains(&(index, place)) {
                what.dests.iter_mut().for_each(|place| renumber(place, &ties));
                if !updates.contains(&(index, place)) {
                    what.sources.iter_mut().for_each(|place| renumber(place, &ties));
                }
            }
            let (defines, uses) = values_of(&what);
            let facts = crate::facts::of(op);
            let mut insn = Insn::new(at, None, Some(what), defines.clone(), uses);
            insn.line = block.ops.loc(place);
            insn.volatile = facts.side_effects || facts.call;
            if facts.early_clobber {
                insn.early_clobbers = defines;
            }
            insn.declared = Some(effects(op, &facts));
            match op {
                Op::I(access @ (Inst::Load { mem, mmo, .. } | Inst::Store { mem, mmo, .. })) => {
                    insn.volatile |= mmo.volatile;
                    insn.invariant = mmo.object == MemObject::ConstantPool;
                    insn.memory = Some(memref_of(mem, mmo, matches!(access, Inst::Store { .. }), &mut symbols));
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
        lowered.phis.extend(phis);
        lowered.succ.extend(successors_of(block, index, function.blocks.len(), &labels).into_iter().map(|b| b as i64));
        blocks.push(lowered.into());
    }
    let mut body = LirBody::new(function.name.clone(), 0, blocks, IndexMap::default(), IndexMap::default());
    body.registers = Some(Regs(&crate::REGISTER_INFO));
    body.symbols = Arc::new(symbols);
    // The edge odds, by block position (`at`); the body keeps the numerator over 2^31.
    for (&(from, to), &(probability, _)) in function.alloc_odds.iter().flatten() {
        body.odds.taken.insert((i64::from(from), i64::from(to)), probability.get_numerator());
    }
    let tied = ties.iter().map(|(&old, &new)| (new, old)).collect();
    Ok(Lowered { body, debug_values, tied })
}

fn show(
    place: &Loc,
    symbols: &[String],
) -> String {
    let registers = Regs(&crate::REGISTER_INFO);
    let name = |id: RegId| registers.name(id).map_or_else(|| format!("{id:?}"), str::to_owned);
    let held = |held: &Held| format!("%{}", held.value);
    match place {
        Loc::Reg(register) => name(register.register),
        Loc::Held(value) => held(value),
        Loc::Imm(value) => format!("#{}", value.value),
        Loc::Address(AddressRef { addr: Some(at), .. }) => match at.space {
            Space::Frame => format!("fi{}+{}", at.slot_home().unwrap_or(-1), at.disp),
            _ => format!("@{}+{}", symbols[at.index as usize], at.disp),
        },
        Loc::Mem(cell) => {
            let base = cell.base.as_ref().map_or_else(|| name(cell.through), held);
            let index = cell.index.as_ref().map(|value| format!("+{}", held(value))).unwrap_or_default();
            match cell.addr {
                Some(at) if at.space == Space::Frame => format!("[fi{}+{}]", at.slot_home().unwrap_or(-1), at.disp),
                Some(at) => format!("[{base}, @{}+{}]", symbols[at.index as usize], at.disp),
                None => format!("[{base}{index}, #{}]", cell.offset),
            }
        }
        other => format!("{other:?}"),
    }
}

/// The body, one instruction a line: `NAME dests <- sources`, the way an
/// instruction reads when a pass looks at it. With `ISEL_DUMP=1` the driver
/// prints it.
pub fn text(lowered: &Lowered) -> String {
    let mut out = String::new();
    for block in lowered.body.blocks.iter() {
        out.push_str(&format!("bb{}:\n", block.at));
        for insn in block.insns.iter() {
            let Some(what) = insn.what.as_ref() else { continue };
            let list = |places: &[Loc]| {
                places.iter().map(|place| show(place, &lowered.body.symbols)).collect::<Vec<_>>().join(", ")
            };
            let target = what.target.map(|at| format!(" -> bb{at}")).unwrap_or_default();
            out.push_str(&format!(
                "  {} {} <- {}{target}\n",
                what.name.as_deref().unwrap_or("?"),
                list(&what.dests),
                list(&what.sources)
            ));
        }
    }
    out
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
                rebuilt.ops.push_at(Op::DbgValue { variable, reg }, insn.line);
                continue;
            }
            let mmo = mmo_of(insn, &lowered.body.symbols);
            let cx = Back { template, symbols: &lowered.body.symbols, mmo: mmo.as_ref() };
            let mut again;
            let what = if lowered.tied.is_empty() {
                what
            } else {
                again = what.clone();
                again.dests.iter_mut().chain(again.sources.iter_mut()).for_each(|place| renumber(place, &lowered.tied));
                &again
            };
            rebuilt.ops.push_at(back(&cx, what)?, insn.line);
        }
        function.blocks.push(rebuilt);
    }
    Ok(function)
}

thread_local! {
    static CHECK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CHECKED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static REFUSED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static FORMS: std::cell::RefCell<std::collections::BTreeSet<String>> = const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

/// Makes the producer refuse every form whose name starts with `prefix` on this
/// thread, as if it had no mapping: the way a test shows the check goes red
/// when a form is dropped.
pub fn refuse_form(prefix: Option<&str>) {
    REFUSED.with(|refused| *refused.borrow_mut() = prefix.map(str::to_owned));
}

/// The opcode names [`round_trip`] has seen on this thread.
pub fn forms_seen() -> Vec<String> {
    FORMS.with(|forms| forms.borrow().iter().cloned().collect())
}

/// Makes the driver replace every function by its round trip before
/// allocation (`LLRM_CHECK_ARM64_LIR` does it for a whole run).
pub fn set_check(on: bool) {
    CHECK.with(|check| check.set(on));
}

pub(crate) fn checking() -> bool {
    CHECK.with(std::cell::Cell::get) || llrm_support::env_flag!("LLRM_CHECK_ARM64_LIR")
}

/// The operations whose round trip [`round_trip`] has compared on this thread:
/// a check that compares nothing passes, so a test reads this.
pub fn checked_operations() -> u64 {
    CHECKED.with(std::cell::Cell::get)
}

/// The virtual registers (by number) an operation defines and reads, from the
/// operation itself: the oracle for the body's `defines` and `uses`.
fn oracle(op: &Op) -> (Vec<u32>, Vec<u32>) {
    let (defs, uses) = op.defs_and_uses();
    let values = |list: RegList| {
        let mut out: Vec<u32> = list
            .into_iter()
            .filter(|register| register.is_virtual())
            .map(|register| match lir_register(register) {
                MfReg::Virt(value) => value,
                MfReg::Phys(_) => unreachable!("a virtual register"),
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    };
    (values(defs), values(uses))
}

/// `function` as the body and back, checked: every operation with its
/// registers and place comes back as it went, and the body's `defines` and
/// `uses` are the operation's own.
pub fn round_trip(function: &MFunc) -> Result<MFunc, String> {
    let lowered = to_lir(function).map_err(|Unmapped(form)| format!("@{}: unmapped form {form}", function.name))?;
    if llrm_support::env_flag!("ISEL_DUMP") {
        eprintln!("=== Lir @{}\n{}", function.name, text(&lowered));
    }
    let again =
        from_lir(&lowered, function).map_err(|Unmapped(form)| format!("@{}: no way back for {form}", function.name))?;
    for ((a, b), block) in function.blocks.iter().zip(&again.blocks).zip(&lowered.body.blocks) {
        let count = a.ops.len().max(b.ops.len());
        if let Some(at) = (0..count).find(|&at| a.ops.get(at) != b.ops.get(at) || a.ops.loc(at) != b.ops.loc(at)) {
            return Err(format!(
                "@{}: block {} differs at operation {at} after the round trip: {:?} became {:?}",
                function.name,
                a.label,
                a.ops.get(at),
                b.ops.get(at)
            ));
        }
        for (op, insn) in a.ops.iter().zip(block.insns.iter()) {
            // A writeback's new value is the register it updates.
            let old =
                |list: &[u32]| list.iter().map(|value| lowered.tied.get(value).copied().unwrap_or(*value)).collect();
            let (mut defines, mut uses): (Vec<u32>, Vec<u32>) = (old(&insn.defines), old(&insn.uses));
            defines.sort_unstable();
            defines.dedup();
            uses.sort_unstable();
            uses.dedup();
            if (defines.clone(), uses.clone()) != oracle(op) {
                return Err(format!(
                    "@{}: block {}: {op:?} defines {defines:?} and reads {uses:?} in the body, {:?} by the operation",
                    function.name,
                    a.label,
                    oracle(op)
                ));
            }
        }
    }
    FORMS.with(|forms| {
        let mut forms = forms.borrow_mut();
        for insn in lowered.body.blocks.iter().flat_map(|block| block.insns.iter()) {
            forms.extend(insn.what.iter().filter_map(|what| what.name.as_deref().map(str::to_owned)));
        }
    });
    let total: usize = function.blocks.iter().map(|block| block.ops.len()).sum();
    CHECKED.with(|checked| checked.set(checked.get() + total as u64));
    Ok(again)
}
