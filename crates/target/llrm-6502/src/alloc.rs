//! The selected function on virtual registers, allocated by the shared
//! [`llrm_spike_mf`] allocator, and emitted as ca65.
//!
//! The byte cells of `select.rs` become virtual registers; `a`, `x`, `y` and
//! `rcN` stay physical. An accumulator operation is `COPY a, s; ADC a, b;
//! COPY d, a`: a value lives in `a` only where the allocator joins it to the
//! physical register, so two results can be in flight without both wanting it.
//! A word is two bytes, so a pair never needs a class of its own.
//!
//! DEVIATE: a function with an opcode `build` does not know is not handled
//! here; `select::function_text` falls back to the cell path. Exit: every
//! opcode built.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::spill::allocate_spilling;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Reg, Target};

use crate::dagisel::generated::ids::opcode as op;
use crate::emit::{MInst, Operand as Machine};
use crate::reg;
use crate::select::Item;

/// The registers a value may live in, as a set: `A`, `X`, `Y`, and the
/// zero-page bytes (`MEM`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Class(u8);

impl Class {
    const A: u8 = 1;
    const X: u8 = 2;
    const Y: u8 = 4;
    const M: u8 = 8;
    pub const ANY: Class = Class(15);
    /// What an ALU instruction reads as its second operand.
    pub const MEM: Class = Class(Class::M);
}

fn rc(n: u8) -> RegId {
    RegId::from_raw(reg::RC0.index() as u8 + n)
}

/// The zero-page bytes a value may live in: the ones a call clobbers.
fn cells() -> impl Iterator<Item = RegId> {
    (2..8).map(rc)
}

fn is_cell(register: RegId) -> bool {
    register.index() >= reg::RC0.index()
}

pub struct Mos;

impl Target for Mos {
    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        let both = one.0 & other.0;
        (both != 0).then_some(Class(both))
    }

    fn any() -> Class {
        Class::ANY
    }

    /// Index registers first: moving a value in or out of one is a transfer,
    /// the cheapest copy; `a` last, since most code wants it.
    fn candidates(class: Class) -> Vec<RegId> {
        let mut out = Vec::new();
        if class.0 & Class::X != 0 {
            out.push(reg::X);
        }
        if class.0 & Class::Y != 0 {
            out.push(reg::Y);
        }
        if class.0 & Class::M != 0 {
            out.extend(cells());
        }
        if class.0 & Class::A != 0 {
            out.push(reg::A);
        }
        out
    }

    fn view(
        root: RegId,
        _: Class,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    /// `rc0` and `rc1` are the software stack pointer.
    fn reserved(root: RegId) -> bool {
        root == reg::RC0 || root == reg::RC1
    }

    fn caller_saved() -> Vec<RegId> {
        [reg::A, reg::X, reg::Y].into_iter().chain(cells()).collect()
    }

    fn spill_store(
        _: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("SPILL"), vec![Operand::Use(register), Operand::Frame(slot)]))
    }

    fn spill_load(
        _: Class,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("RELOAD"), vec![Operand::Def(register), Operand::Frame(slot)]))
    }
}

/// An opcode `build` does not know.
#[derive(Debug)]
pub struct Unsupported(pub String);

struct Builder {
    function: MFunction<Class>,
    bytes: HashMap<String, Reg>,
    /// Cells defined once, by an immediate load: they fold into the user.
    constants: HashMap<String, i64>,
}

impl Builder {
    fn fresh(&mut self) -> Reg {
        self.function.classes.push(Class::ANY);
        Reg::Virt(self.function.classes.len() as u32 - 1)
    }

    /// The register a byte operand names: `a`, `x`, `y`, `rcN`, or a cell.
    fn byte(
        &mut self,
        name: &str,
    ) -> Result<Reg, Unsupported> {
        let named = |n: &str| match n {
            "a" => Some(reg::A),
            "x" => Some(reg::X),
            "y" => Some(reg::Y),
            _ => n.strip_prefix("rc").and_then(|n| n.parse::<u8>().ok()).filter(|n| *n < 16).map(rc),
        };
        if let Some(r) = named(name) {
            return Ok(Reg::Phys(r));
        }
        if !name.starts_with("v_") && !name.starts_with("w_") {
            return Err(Unsupported(format!("operand {name}")));
        }
        if let Some(r) = self.bytes.get(name) {
            return Ok(*r);
        }
        let r = self.fresh();
        self.bytes.insert(name.to_owned(), r);
        Ok(r)
    }

    /// The low and high byte of a pair: `rsN`, or a word cell.
    fn word(
        &mut self,
        name: &str,
    ) -> Result<(Reg, Reg), Unsupported> {
        if let Some(n) = name.strip_prefix("rs").and_then(|n| n.parse::<u8>().ok()).filter(|n| *n < 8) {
            return Ok((Reg::Phys(rc(2 * n)), Reg::Phys(rc(2 * n + 1))));
        }
        let name = name.strip_suffix("+1").unwrap_or(name);
        let low = self.byte(name)?;
        let high = self.byte(&format!("{name}+1"))?;
        Ok((low, high))
    }

    fn require(
        &mut self,
        register: Reg,
        class: Class,
    ) -> Result<(), Unsupported> {
        match register {
            Reg::Virt(v) => {
                let met = Mos::meet(self.function.classes[v as usize], class)
                    .ok_or_else(|| Unsupported("a value needed in two places".into()))?;
                self.function.classes[v as usize] = met;
                Ok(())
            }
            Reg::Phys(p) if is_cell(p) => Ok(()),
            Reg::Phys(_) => Err(Unsupported("a register as a memory operand".into())),
        }
    }

    fn push(
        &mut self,
        inst: Inst,
    ) {
        self.function.blocks.last_mut().expect("a block").insts.push(inst);
    }

    fn copy(
        &mut self,
        to: Reg,
        from: Reg,
    ) {
        self.push(Inst::copy(to, from));
    }

    /// `d := a <mnemonic> b`, through the accumulator; the second operand is
    /// memory or an immediate.
    fn alu(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        b: &str,
    ) -> Result<(), Unsupported> {
        let acc = Reg::Phys(reg::A);
        let (d, a) = (self.byte(d)?, self.byte(a)?);
        let second = match self.constants.get(b) {
            Some(value) => Operand::Imm(*value & 0xFF),
            None => {
                let b = self.byte(b)?;
                self.require(b, Class::MEM)?;
                Operand::Use(b)
            }
        };
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        self.copy(acc, a);
        self.push(Inst::new(Kind::Op(mnemonic), vec![Operand::Def(acc), Operand::Use(acc), second]));
        self.copy(d, acc);
        Ok(())
    }

    /// The same on a pair, the carry running from the low byte to the high.
    fn alu16(
        &mut self,
        mnemonic: &'static str,
        carry: Option<&'static str>,
        d: &str,
        a: &str,
        b: &str,
    ) -> Result<(), Unsupported> {
        let ((dl, dh), (al, ah), (bl, bh)) = (self.word(d)?, self.word(a)?, self.word(b)?);
        let acc = Reg::Phys(reg::A);
        if let Some(carry) = carry {
            self.push(Inst::new(Kind::Op(carry), vec![]));
        }
        for ((d, a), b) in [(dl, al), (dh, ah)].into_iter().zip([bl, bh]) {
            self.require(b, Class::MEM)?;
            self.copy(acc, a);
            self.push(Inst::new(Kind::Op(mnemonic), vec![Operand::Def(acc), Operand::Use(acc), Operand::Use(b)]));
            self.copy(d, acc);
        }
        Ok(())
    }
}

fn name(operand: &Machine) -> Result<&str, Unsupported> {
    match operand {
        Machine::Reg(name) => Ok(name),
        other => Err(Unsupported(format!("operand {other:?}"))),
    }
}

fn immediate(operand: &Machine) -> Result<i64, Unsupported> {
    match operand {
        Machine::Imm(value) => Ok(*value),
        other => Err(Unsupported(format!("operand {other:?}"))),
    }
}

/// The cells defined by exactly one `LDI8`, with their values.
fn constants(items: &[Item]) -> HashMap<String, i64> {
    let mut defs: HashMap<&str, usize> = HashMap::new();
    let mut values = HashMap::new();
    for item in items {
        let Item::Inst(MInst { opcode, operands }) = item else { continue };
        let Some(Machine::Reg(d)) = operands.first() else { continue };
        *defs.entry(d).or_default() += 1;
        if *opcode == op::LDI8
            && let Some(Machine::Imm(value)) = operands.get(1)
        {
            values.insert(d.clone(), *value);
        }
    }
    values.retain(|cell, _| defs[cell.as_str()] == 1 && cell.starts_with("v_"));
    values
}

/// `items` as a machine function on virtual registers: one block, or an error
/// naming what is not built.
pub fn build(
    name_of_function: &str,
    items: &[Item],
) -> Result<MFunction<Class>, Unsupported> {
    let mut b =
        Builder { function: MFunction::new(name_of_function), bytes: HashMap::new(), constants: constants(items) };
    b.function.blocks.push(Block { id: 0, ..Block::default() });
    let mut labels = 0;
    for item in items {
        let inst = match item {
            Item::Label(_) => {
                labels += 1;
                if labels > 2 {
                    return Err(Unsupported("more than one block".into()));
                }
                continue;
            }
            Item::Text(_) => return Err(Unsupported("raw text".into())),
            Item::Inst(inst) => inst,
        };
        let o = &inst.operands;
        match inst.opcode {
            op::COPY8 => {
                let (d, s) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                b.copy(d, s);
            }
            op::COPY16 => {
                let ((dl, dh), (sl, sh)) = (b.word(name(&o[0])?)?, b.word(name(&o[1])?)?);
                b.copy(dl, sl);
                b.copy(dh, sh);
            }
            op::LDI8 => {
                let d = b.byte(name(&o[0])?)?;
                b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(immediate(&o[1])? & 0xFF)]));
            }
            op::LDI16 => {
                let (low, high) = b.word(name(&o[0])?)?;
                let value = immediate(&o[1])?;
                for (d, byte) in [(low, value & 0xFF), (high, (value >> 8) & 0xFF)] {
                    b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(byte)]));
                }
            }
            op::ADD8 => b.alu("ADC", Some("CLC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::SUB8 => b.alu("SBC", Some("SEC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::AND8 => b.alu("AND", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::OR8 => b.alu("ORA", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::XOR8 => b.alu("EOR", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::ADD16 => b.alu16("ADC", Some("CLC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::SUB16 => b.alu16("SBC", Some("SEC"), name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::AND16 => b.alu16("AND", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::OR16 => b.alu16("ORA", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::XOR16 => b.alu16("EOR", None, name(&o[0])?, name(&o[1])?, name(&o[2])?)?,
            op::RET => {
                let results = results(&b.function);
                let mut ret =
                    Inst::new(Kind::Op("RTS"), results.into_iter().map(|r| Operand::Use(Reg::Phys(r))).collect());
                ret.terminator = true;
                ret.barrier = true;
                b.push(ret);
            }
            other => return Err(Unsupported(format!("opcode {other}"))),
        }
    }
    remove_dead(&mut b.function);
    Ok(b.function)
}

/// The physical registers the run of copies just before the return sets: the
/// result.
fn results(function: &MFunction<Class>) -> Vec<RegId> {
    let mut out = Vec::new();
    for inst in function.blocks[0].insts.iter().rev() {
        match (inst.kind, inst.operands.first()) {
            (Kind::Copy, Some(Operand::Def(Reg::Phys(p)))) => out.push(*p),
            _ => break,
        }
    }
    out
}

/// Copies and loads of a value nothing reads.
fn remove_dead(function: &mut MFunction<Class>) {
    loop {
        let mut read: HashSet<u32> = HashSet::new();
        for operand in function.blocks.iter().flat_map(|b| &b.insts).flat_map(|i| &i.operands) {
            if let Operand::Use(Reg::Virt(v)) = operand {
                read.insert(*v);
            }
        }
        let mut changed = false;
        for block in &mut function.blocks {
            let before = block.insts.len();
            block
                .insts
                .retain(
                    |inst| {
                        let pure = inst.kind == Kind::Copy || inst.kind == Kind::Op("LDI");
                        !(pure
                            && matches!(
                                inst.operands.first(),
                                Some(Operand::Def(Reg::Virt(v))) if !read.contains(v)
                            ))
                    },
                );
            changed |= block.insts.len() != before;
        }
        if !changed {
            return;
        }
    }
}

/// The ca65 name of an allocated register.
fn register_name(register: RegId) -> String {
    if register == reg::A {
        "a".into()
    } else if register == reg::X {
        "x".into()
    } else if register == reg::Y {
        "y".into()
    } else {
        format!("rc{}", register.index() - reg::RC0.index())
    }
}

/// Registers live after each instruction of each block, by their final names.
fn live_after(
    function: &MFunction<Class>,
    at: &dyn Fn(Reg) -> RegId,
) -> Vec<Vec<HashSet<RegId>>> {
    let blocks = function.blocks.len();
    let next = successors(function);
    let effects = |inst: &Inst| {
        let mut defs = Vec::new();
        let mut reads = Vec::new();
        for operand in &inst.operands {
            match operand {
                Operand::Def(r) => defs.push(at(*r)),
                Operand::Use(r) => reads.push(at(*r)),
                _ => {}
            }
        }
        (defs, reads)
    };
    let mut live_in: Vec<HashSet<RegId>> = vec![HashSet::new(); blocks];
    let mut after: Vec<Vec<HashSet<RegId>>> =
        function.blocks.iter().map(|b| vec![HashSet::new(); b.insts.len()]).collect();
    loop {
        let mut changed = false;
        for b in (0..blocks).rev() {
            let mut live: HashSet<RegId> = next[b].iter().flat_map(|n| live_in[*n].iter().copied()).collect();
            for (i, inst) in function.blocks[b].insts.iter().enumerate().rev() {
                after[b][i] = live.clone();
                let (defs, reads) = effects(inst);
                for d in &defs {
                    live.remove(d);
                }
                live.extend(reads);
            }
            if live != live_in[b] {
                live_in[b] = live;
                changed = true;
            }
        }
        if !changed {
            return after;
        }
    }
}

/// A move that needs the accumulator when neither side is it: bracketed by
/// `pha`/`pla` when it is live.
fn through_a(
    out: &mut String,
    a_live: bool,
    body: &str,
) {
    if a_live {
        out.push_str("  pha\n");
    }
    out.push_str(body);
    if a_live {
        out.push_str("  pla\n");
    }
}

fn move_byte(
    out: &mut String,
    to: RegId,
    from: RegId,
    a_live: bool,
) {
    let (d, s) = (register_name(to), register_name(from));
    if to == from {
        return;
    }
    let reg3 = |r: RegId| r == reg::A || r == reg::X || r == reg::Y;
    match (reg3(to), reg3(from)) {
        (true, true) if to == reg::A || from == reg::A => {
            let mnemonic = if to == reg::A { format!("t{s}a") } else { format!("ta{d}") };
            writeln!(out, "  {mnemonic}").unwrap();
        }
        // x to y or y to x: through `a`
        (true, true) => through_a(out, a_live, &format!("  t{s}a\n  ta{d}\n")),
        (true, false) => writeln!(out, "  ld{d} {s}").unwrap(),
        (false, true) => writeln!(out, "  st{s} {d}").unwrap(),
        (false, false) => through_a(out, a_live, &format!("  lda {s}\n  sta {d}\n")),
    }
}

/// The function as ca65 lines (no label), with the zero-page slots the spills
/// used, or an error from the allocator.
pub fn emit(mut function: MFunction<Class>) -> Result<(String, Vec<String>), String> {
    let (assignment, slots) = allocate_spilling::<Mos>(&mut function, 0)?;
    let at = |r: Reg| match r {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    };
    let after = live_after(&function, &at);
    let slot_name = |slot: i32| format!("s_{}_{}", function.name, slot);
    let mut out = String::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            let a_live = after[b][i].contains(&reg::A);
            let regs: Vec<RegId> = inst
                .operands
                .iter()
                .filter_map(|o| match o {
                    Operand::Def(r) | Operand::Use(r) => Some(at(*r)),
                    _ => None,
                })
                .collect();
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, _) => move_byte(&mut out, regs[0], regs[1], a_live),
                (Kind::Op("LDI"), [_, Operand::Imm(value)]) => {
                    let d = regs[0];
                    if d == reg::A || d == reg::X || d == reg::Y {
                        writeln!(out, "  ld{} #{value}", register_name(d)).unwrap();
                    } else {
                        through_a(&mut out, a_live, &format!("  lda #{value}\n  sta {}\n", register_name(d)));
                    }
                }
                (Kind::Op("CLC"), _) => out.push_str("  clc\n"),
                (Kind::Op("SEC"), _) => out.push_str("  sec\n"),
                (Kind::Op(mnemonic @ ("ADC" | "SBC" | "AND" | "ORA" | "EOR")), [_, _, second]) => {
                    let text = match second {
                        Operand::Imm(value) => format!("#{value}"),
                        _ => register_name(regs[2]),
                    };
                    writeln!(out, "  {} {text}", mnemonic.to_lowercase()).unwrap();
                }
                (Kind::Op("RTS"), _) => out.push_str("  rts\n"),
                (Kind::Op("SPILL"), [_, Operand::Frame(slot)]) => {
                    if regs[0] == reg::A || regs[0] == reg::X || regs[0] == reg::Y {
                        writeln!(out, "  st{} {}", register_name(regs[0]), slot_name(*slot)).unwrap();
                    } else {
                        through_a(
                            &mut out,
                            a_live,
                            &format!("  lda {}\n  sta {}\n", register_name(regs[0]), slot_name(*slot)),
                        );
                    }
                }
                (Kind::Op("RELOAD"), [_, Operand::Frame(slot)]) => {
                    if regs[0] == reg::A || regs[0] == reg::X || regs[0] == reg::Y {
                        writeln!(out, "  ld{} {}", register_name(regs[0]), slot_name(*slot)).unwrap();
                    } else {
                        through_a(
                            &mut out,
                            a_live,
                            &format!("  lda {}\n  sta {}\n", slot_name(*slot), register_name(regs[0])),
                        );
                    }
                }
                (other, _) => return Err(format!("{other:?} has no expansion")),
            }
        }
    }
    Ok((out, slots.iter().map(|(slot, _)| slot_name(*slot)).collect()))
}
