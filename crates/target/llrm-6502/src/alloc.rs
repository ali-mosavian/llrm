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
use crate::emit::{MInst, Operand as Machine, address, spill};
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
    /// The block each label names.
    block_of: HashMap<String, u32>,
    /// How many times each cell or register has been defined so far.
    versions: HashMap<String, u32>,
    /// The compare whose flags the next branch reads, and the ones saved to
    /// cells for later.
    pending: Option<Compare>,
    saved: HashMap<String, Compare>,
    /// Compares are redone where the flags are read: nothing keeps `P` alive.
    /// The block instructions go to.
    current: usize,
}

/// A compare not yet emitted: the operands and the version of each when it
/// was made, so a later redo can tell they have not changed.
#[derive(Clone, Debug)]
struct Compare {
    a: (String, u32),
    b: (String, u32),
    /// Over words: `lda; cmp` on the low bytes, then `lda; sbc` on the high.
    word: bool,
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

    /// `name` is defined here.
    fn define(
        &mut self,
        name: &str,
    ) {
        *self.versions.entry(name.to_owned()).or_default() += 1;
    }

    fn version(
        &self,
        name: &str,
    ) -> u32 {
        self.versions.get(name).copied().unwrap_or(0)
    }

    /// `COPY a, lhs; CMP a, rhs` again, as the flags' producer: the operands
    /// must hold what they held at the first compare.
    fn redo_compare(&mut self) -> Result<(), Unsupported> {
        let Compare { a, b, word } = self.pending.clone().ok_or_else(|| Unsupported("flags with no compare".into()))?;
        if self.version(&a.0) != a.1 || self.version(&b.0) != b.1 {
            return Err(Unsupported("a compare's operand changed before its branch".into()));
        }
        let acc = Reg::Phys(reg::A);
        let (lhs, rhs) = if word {
            let ((al, ah), (bl, bh)) = (self.word(&a.0)?, self.word(&b.0)?);
            for r in [bl, bh] {
                self.require(r, Class::MEM)?;
            }
            ([al, ah], [Operand::Use(bl), Operand::Use(bh)])
        } else {
            let lhs = self.byte(&a.0)?;
            let rhs = match self.constants.get(&b.0) {
                Some(value) => Operand::Imm(*value & 0xFF),
                None => {
                    let r = self.byte(&b.0)?;
                    self.require(r, Class::MEM)?;
                    Operand::Use(r)
                }
            };
            ([lhs, lhs], [rhs.clone(), rhs])
        };
        self.copy(acc, lhs[0]);
        let mut cmp =
            Inst::new(Kind::Op("CMP"), vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(acc), rhs[0].clone()]);
        cmp.sets_flags = true;
        self.push(cmp);
        if word {
            self.copy(acc, lhs[1]);
            let mut sbc = Inst::new(
                Kind::Op("SBC"),
                vec![Operand::Def(acc), Operand::Use(acc), rhs[1].clone(), Operand::Def(Reg::Phys(reg::FLAGS))],
            );
            sbc.sets_flags = true;
            self.push(sbc);
        }
        Ok(())
    }

    fn block(
        &self,
        label: &Machine,
    ) -> Result<Operand, Unsupported> {
        match label {
            Machine::Label(l) => self
                .block_of
                .get(l)
                .map(|id| Operand::Block(*id))
                .ok_or_else(|| Unsupported(format!("a branch to {l}, outside the function"))),
            other => Err(Unsupported(format!("operand {other:?}"))),
        }
    }

    fn push(
        &mut self,
        inst: Inst,
    ) {
        self.function.blocks[self.current].insts.push(inst);
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
        self.define(d);
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
        self.define(d);
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
        if *opcode == op::STZ8 {
            values.insert(d.clone(), 0);
        }
    }
    values.retain(|cell, _| defs[cell.as_str()] == 1 && cell.starts_with("v_"));
    values
}

/// `items` as a machine function on virtual registers, with the labels of each
/// block; or an error naming what is not built.
pub fn build(
    name_of_function: &str,
    items: &[Item],
) -> Result<(MFunction<Class>, Vec<Vec<String>>), Unsupported> {
    // A block starts at a label that follows an instruction (or the first).
    let mut labels: Vec<Vec<String>> = vec![Vec::new()];
    let mut block_of = HashMap::new();
    let mut after_inst = false;
    for item in items {
        match item {
            Item::Label(l) => {
                if after_inst {
                    labels.push(Vec::new());
                    after_inst = false;
                }
                block_of.insert(l.clone(), labels.len() as u32 - 1);
                labels.last_mut().expect("a block").push(l.clone());
            }
            _ => after_inst = true,
        }
    }
    let mut b = Builder {
        function: MFunction::new(name_of_function),
        bytes: HashMap::new(),
        constants: constants(items),
        block_of,
        versions: HashMap::new(),
        pending: None,
        saved: HashMap::new(),
        current: 0,
    };
    for id in 0..labels.len() {
        b.function.blocks.push(Block { id: id as u32, ..Block::default() });
    }
    let mut after_inst = false;
    for item in items {
        let inst = match item {
            Item::Label(_) => {
                if after_inst {
                    b.current += 1;
                    after_inst = false;
                    b.pending = None;
                }
                continue;
            }
            Item::Text(_) => return Err(Unsupported("raw text".into())),
            Item::Inst(inst) => inst,
        };
        after_inst = true;
        let o = &inst.operands;
        match inst.opcode {
            op::COPY8 => {
                b.define(name(&o[0])?);
                let (d, s) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                b.copy(d, s);
            }
            op::COPY16 => {
                b.define(name(&o[0])?);
                let ((dl, dh), (sl, sh)) = (b.word(name(&o[0])?)?, b.word(name(&o[1])?)?);
                b.copy(dl, sl);
                b.copy(dh, sh);
            }
            op::LDI8 => {
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                b.push(Inst::new(Kind::Op("LDI"), vec![Operand::Def(d), Operand::Imm(immediate(&o[1])? & 0xFF)]));
            }
            op::LDI16 if matches!(&o[1], Machine::Label(_)) => {
                let Machine::Label(symbol) = &o[1] else { unreachable!() };
                b.define(name(&o[0])?);
                let (low, high) = b.word(name(&o[0])?)?;
                for (d, half) in [(low, 0), (high, 1)] {
                    b.push(Inst::new(Kind::Op("LDS"), vec![Operand::Def(d), Operand::Symbol(symbol.clone(), half)]));
                }
            }
            op::LDI16 => {
                b.define(name(&o[0])?);
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
            op::SHL8 | op::SRL8 | op::SRA8 => {
                let step = match inst.opcode {
                    op::SHL8 => "ASL",
                    op::SRL8 => "LSR",
                    _ => "ASR",
                };
                let acc = Reg::Phys(reg::A);
                b.define(name(&o[0])?);
                let (d, a) = (b.byte(name(&o[0])?)?, b.byte(name(&o[1])?)?);
                b.copy(acc, a);
                for _ in 0..immediate(&o[2])? {
                    b.push(Inst::new(Kind::Op(step), vec![Operand::Def(acc), Operand::Use(acc)]));
                }
                b.copy(d, acc);
            }
            op::STZ8 => {
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                b.require(d, Class::MEM)?;
                b.push(Inst::new(Kind::Op("STZ"), vec![Operand::Def(d)]));
            }
            op::STZABS => {
                let symbol = address(&o[0], &o[1]).map_err(Unsupported)?;
                b.push(Inst::new(Kind::Op("STZM"), vec![Operand::Symbol(symbol, 0)]));
            }
            op::LDABS | op::STABS => {
                let symbol = address(&o[1], &o[2]).map_err(Unsupported)?;
                let acc = Reg::Phys(reg::A);
                if inst.opcode == op::LDABS {
                    b.define(name(&o[0])?);
                    let d = b.byte(name(&o[0])?)?;
                    b.push(Inst::new(Kind::Op("LDM"), vec![Operand::Def(acc), Operand::Symbol(symbol, 0)]));
                    b.copy(d, acc);
                } else {
                    let value = b.byte(name(&o[0])?)?;
                    b.copy(acc, value);
                    b.push(Inst::new(Kind::Op("STM"), vec![Operand::Use(acc), Operand::Symbol(symbol, 0)]));
                }
            }
            op::LDAX | op::STAX => {
                let Machine::Label(symbol) = &o[1] else { return Err(Unsupported("an indexed access".into())) };
                let (acc, x) = (Reg::Phys(reg::A), Reg::Phys(reg::X));
                let index = b.byte(name(&o[2])?)?;
                b.copy(x, index);
                if inst.opcode == op::LDAX {
                    b.define(name(&o[0])?);
                    let d = b.byte(name(&o[0])?)?;
                    b.push(Inst::new(
                        Kind::Op("LDAX"),
                        vec![Operand::Def(acc), Operand::Symbol(symbol.clone(), 0), Operand::Use(x)],
                    ));
                    b.copy(d, acc);
                } else {
                    let value = b.byte(name(&o[0])?)?;
                    b.copy(acc, value);
                    b.push(Inst::new(
                        Kind::Op("STAX"),
                        vec![Operand::Use(acc), Operand::Symbol(symbol.clone(), 0), Operand::Use(x)],
                    ));
                }
            }
            // The flags are made where they are read.
            op::CMP8 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending =
                    Some(Compare { a: (a.to_owned(), b.version(a)), b: (c.to_owned(), b.version(c)), word: false });
            }
            op::CMP16 => {
                let (a, c) = (name(&o[1])?, name(&o[2])?);
                b.pending =
                    Some(Compare { a: (a.to_owned(), b.version(a)), b: (c.to_owned(), b.version(c)), word: true });
            }
            spill_op if spill_op == spill::SAVE_FLAGS => {
                let compare = b.pending.clone().ok_or_else(|| Unsupported("saved flags with no compare".into()))?;
                b.saved.insert(name(&o[0])?.to_owned(), compare);
            }
            spill_op if spill_op == spill::LOAD_FLAGS => {
                b.pending = Some(
                    b.saved.get(name(&o[0])?).cloned().ok_or_else(|| Unsupported("loaded flags never saved".into()))?,
                );
            }
            code @ (op::BEQ | op::BNE | op::BCC | op::BCS | op::BMI | op::BPL) => {
                b.redo_compare()?;
                let mnemonic = match code {
                    op::BEQ => "BEQ",
                    op::BNE => "BNE",
                    op::BCC => "BCC",
                    op::BCS => "BCS",
                    op::BMI => "BMI",
                    _ => "BPL",
                };
                let target = b.block(&o[0])?;
                let mut branch = Inst::new(Kind::Op(mnemonic), vec![target, Operand::Use(Reg::Phys(reg::FLAGS))]);
                branch.terminator = true;
                branch.reads_flags = true;
                b.push(branch);
            }
            op::JMP | op::BRA => {
                let target = b.block(&o[0])?;
                let mut jump = Inst::new(Kind::Op(if inst.opcode == op::BRA { "BRA" } else { "JMP" }), vec![target]);
                jump.terminator = true;
                jump.barrier = true;
                b.push(jump);
            }
            op::CMOV8 => {
                b.redo_compare()?;
                b.define(name(&o[0])?);
                let d = b.byte(name(&o[0])?)?;
                let value = |b: &mut Builder, operand: &Machine| -> Result<Operand, Unsupported> {
                    match b.constants.get(name(operand)?) {
                        Some(v) => Ok(Operand::Imm(*v & 0xFF)),
                        None => {
                            let r = b.byte(name(operand)?)?;
                            b.require(r, Class(Class::X | Class::Y | Class::M))?;
                            Ok(Operand::Use(r))
                        }
                    }
                };
                let (if_false, if_true) = (value(&mut b, &o[1])?, value(&mut b, &o[2])?);
                let mut cmov = Inst::new(
                    Kind::Op("CMOV"),
                    vec![
                        Operand::Def(Reg::Phys(reg::A)),
                        if_false,
                        if_true,
                        Operand::Imm(immediate(&o[3])?),
                        Operand::Use(Reg::Phys(reg::FLAGS)),
                    ],
                );
                cmov.reads_flags = true;
                b.push(cmov);
                b.copy(d, Reg::Phys(reg::A));
            }
            op::CMOV16 => {
                b.redo_compare()?;
                b.define(name(&o[0])?);
                let ((dl, dh), (fl, fh), (tl, th)) =
                    (b.word(name(&o[0])?)?, b.word(name(&o[1])?)?, b.word(name(&o[2])?)?);
                for r in [fl, fh, tl, th] {
                    b.require(r, Class(Class::X | Class::Y | Class::M))?;
                }
                let mut operands = vec![Operand::Def(dl), Operand::Def(dh), Operand::Def(Reg::Phys(reg::A))];
                operands.extend([fl, fh, tl, th].map(Operand::Use));
                operands.extend([Operand::Imm(immediate(&o[3])?), Operand::Use(Reg::Phys(reg::FLAGS))]);
                let mut cmov = Inst::new(Kind::Op("CMOV16"), operands);
                cmov.reads_flags = true;
                cmov.early_clobber = true;
                b.push(cmov);
            }
            op::RET => {
                let results = results(&b.function.blocks[b.current].insts);
                let mut ret =
                    Inst::new(Kind::Op("RTS"), results.into_iter().map(|r| Operand::Use(Reg::Phys(r))).collect());
                ret.terminator = true;
                ret.barrier = true;
                b.push(ret);
            }
            other => return Err(Unsupported(format!("opcode {other}"))),
        }
    }
    fold_branches(&mut b.function);
    remove_dead(&mut b.function);
    Ok((b.function, labels))
}

/// The branch with the opposite condition.
fn opposite(kind: Kind) -> Option<Kind> {
    Some(Kind::Op(match kind {
        Kind::Op("BEQ") => "BNE",
        Kind::Op("BNE") => "BEQ",
        Kind::Op("BCC") => "BCS",
        Kind::Op("BCS") => "BCC",
        Kind::Op("BMI") => "BPL",
        Kind::Op("BPL") => "BMI",
        _ => return None,
    }))
}

/// `BranchFolder`'s simple cases: a branch to a block that only jumps goes to
/// where it jumps, a block nothing reaches goes, and a conditional branch over
/// the jump that follows it, to the next block, becomes the opposite branch.
///
/// DEVIATE: spike-local, in the 6502 only. Reason: it needs the opposite of a
/// branch. Exit: a `Target` hook, in `llrm-spike-mf`.
fn fold_branches(function: &mut MFunction<Class>) {
    // threading, to a fixed point (a loop of empty jumps stays)
    loop {
        let only_jump: HashMap<u32, u32> = function
            .blocks
            .iter()
            .filter_map(|block| match &block.insts[..] {
                [Inst { kind: Kind::Op("JMP" | "BRA"), operands, .. }] => match operands[..] {
                    [Operand::Block(to)] if to != block.id => Some((block.id, to)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let mut changed = false;
        for operand in function.blocks.iter_mut().flat_map(|b| &mut b.insts).flat_map(|i| &mut i.operands) {
            if let Operand::Block(to) = operand
                && let Some(next) = only_jump.get(to)
            {
                *to = *next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // unreachable blocks
    let reached = {
        let next = successors(function);
        let mut seen = vec![false; function.blocks.len()];
        let mut work = vec![0usize];
        while let Some(at) = work.pop() {
            if !std::mem::replace(&mut seen[at], true) {
                work.extend(next[at].iter().copied());
            }
        }
        seen
    };
    let mut at = 0;
    function
        .blocks
        .retain(
            |_| {
                at += 1;
                reached[at - 1]
            },
        );
    // the condition over a jump, to the block that follows
    for at in 0..function.blocks.len() {
        let next_id = function.blocks.get(at + 1).map(|b| b.id);
        let insts = &mut function.blocks[at].insts;
        let n = insts.len();
        if n < 2 {
            continue;
        }
        let (Some(flipped), [Operand::Block(skipped), _], [Operand::Block(to)]) =
            (opposite(insts[n - 2].kind), &insts[n - 2].operands[..], &insts[n - 1].operands[..])
        else {
            continue;
        };
        if matches!(insts[n - 1].kind, Kind::Op("JMP" | "BRA")) && Some(*skipped) == next_id {
            let to = *to;
            insts[n - 2].kind = flipped;
            insts[n - 2].operands[0] = Operand::Block(to);
            insts.pop();
        }
    }
}

/// The physical registers the run of copies just before the return sets: the
/// result.
fn results(insts: &[Inst]) -> Vec<RegId> {
    let mut out = Vec::new();
    for inst in insts.iter().rev() {
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
                        let pure = matches!(inst.kind, Kind::Copy | Kind::Op("LDI" | "LDS" | "STZ"));
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
pub fn emit(
    mut function: MFunction<Class>,
    labels: &[Vec<String>],
) -> Result<(String, Vec<String>), String> {
    let (assignment, slots) = allocate_spilling::<Mos>(&mut function, 0)?;
    let at = |r: Reg| match r {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    };
    // copies the allocator joined are gone; blocks that held only those now
    // just jump
    for block in &mut function.blocks {
        block
            .insts
            .retain(
                |inst| !(inst.kind == Kind::Copy
                    && matches!(
                        inst.operands[..],
                        [Operand::Def(d), Operand::Use(s)] if at(d) == at(s)
                    )),
            );
    }
    fold_branches(&mut function);
    let after = live_after(&function, &at);
    let slot_name = |slot: i32| format!("s_{}_{}", function.name, slot);
    let mut out = String::new();
    let mut made = 0;
    let target = |operand: &Operand| match operand {
        Operand::Block(id) => labels[*id as usize][0].clone(),
        other => unreachable!("{other:?} is not a block"),
    };
    for (b, block) in function.blocks.iter().enumerate() {
        for label in &labels[block.id as usize] {
            writeln!(out, "{label}:").unwrap();
        }
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
                (Kind::Op("LDS"), [_, Operand::Symbol(symbol, half)]) => {
                    let d = regs[0];
                    let part = if *half == 0 { '<' } else { '>' };
                    if d == reg::A || d == reg::X || d == reg::Y {
                        writeln!(out, "  ld{} #{part}{symbol}", register_name(d)).unwrap();
                    } else {
                        through_a(&mut out, a_live, &format!("  lda #{part}{symbol}\n  sta {}\n", register_name(d)));
                    }
                }
                // both bytes of d := cc ? t : f; every use is read into `a`
                // before a def is written, and no def shares a register with a
                // use
                (Kind::Op("CMOV16"), [.., Operand::Imm(code), _]) => {
                    made += 1;
                    let (taken, done) = (format!("_cmt{made}"), format!("_cmd{made}"));
                    let branch = ["beq", "bne", "bcc", "bcs", "bmi", "bpl"][*code as usize];
                    let load = |out: &mut String, r: RegId| match r {
                        r if r == reg::X => out.push_str("  txa\n"),
                        r if r == reg::Y => out.push_str("  tya\n"),
                        r => writeln!(out, "  lda {}", register_name(r)).unwrap(),
                    };
                    writeln!(out, "  {branch} {taken}").unwrap();
                    for (n, pair) in [[regs[3], regs[4]], [regs[5], regs[6]]].into_iter().enumerate() {
                        for (d, from) in [regs[0], regs[1]].into_iter().zip(pair) {
                            load(&mut out, from);
                            move_byte(&mut out, d, reg::A, false);
                        }
                        if n == 0 {
                            writeln!(out, "  jmp {done}\n{taken}:").unwrap();
                        }
                    }
                    writeln!(out, "{done}:").unwrap();
                }
                (Kind::Op("STZ"), _) => writeln!(out, "  stz {}", register_name(regs[0])).unwrap(),
                (Kind::Op("STZM"), [Operand::Symbol(symbol, _)]) => writeln!(out, "  stz {symbol}").unwrap(),
                (Kind::Op("LDM"), [_, Operand::Symbol(symbol, _)]) => writeln!(out, "  lda {symbol}").unwrap(),
                (Kind::Op("STM"), [_, Operand::Symbol(symbol, _)]) => writeln!(out, "  sta {symbol}").unwrap(),
                (Kind::Op("ASL"), _) => out.push_str("  asl a\n"),
                (Kind::Op("LSR"), _) => out.push_str("  lsr a\n"),
                (Kind::Op("ASR"), _) => out.push_str("  cmp #$80\n  ror a\n"),
                (Kind::Op("LDAX"), [_, Operand::Symbol(symbol, _), _]) => writeln!(out, "  lda {symbol},x").unwrap(),
                (Kind::Op("STAX"), [_, Operand::Symbol(symbol, _), _]) => writeln!(out, "  sta {symbol},x").unwrap(),
                (Kind::Op("CLC"), _) => out.push_str("  clc\n"),
                (Kind::Op("SEC"), _) => out.push_str("  sec\n"),
                (Kind::Op(mnemonic @ ("ADC" | "SBC" | "AND" | "ORA" | "EOR")), [_, _, second, ..]) => {
                    let text = match second {
                        Operand::Imm(value) => format!("#{value}"),
                        _ => register_name(regs[2]),
                    };
                    writeln!(out, "  {} {text}", mnemonic.to_lowercase()).unwrap();
                }
                (Kind::Op("RTS"), _) => out.push_str("  rts\n"),
                (Kind::Op("CMP"), [_, _, second]) => {
                    let text = match second {
                        Operand::Imm(value) => format!("#{value}"),
                        _ => register_name(regs[2]),
                    };
                    writeln!(out, "  cmp {text}").unwrap();
                }
                (Kind::Op(branch @ ("BEQ" | "BNE" | "BCC" | "BCS" | "BMI" | "BPL")), [to, _]) => {
                    writeln!(out, "  {} {}", branch.to_lowercase(), target(to)).unwrap();
                }
                // a jump to the block that follows is the layout
                (Kind::Op(jump @ ("JMP" | "BRA")), [to @ Operand::Block(id)]) => {
                    if function.blocks.get(b + 1).map(|next| next.id) != Some(*id) {
                        writeln!(out, "  {} {}", jump.to_lowercase(), target(to)).unwrap();
                    }
                }
                // a := cc ? if_true : if_false, the branch first: loads set N
                // and Z
                (Kind::Op("CMOV"), [_, if_false, if_true, Operand::Imm(code), _]) => {
                    made += 1;
                    let (taken, done) = (format!("_cmt{made}"), format!("_cmd{made}"));
                    let branch = ["beq", "bne", "bcc", "bcs", "bmi", "bpl"][*code as usize];
                    let load = |out: &mut String, value: &Operand, at: Option<RegId>| match (value, at) {
                        (Operand::Imm(v), _) => writeln!(out, "  lda #{v}").unwrap(),
                        (_, Some(r)) if r == reg::X => out.push_str("  txa\n"),
                        (_, Some(r)) if r == reg::Y => out.push_str("  tya\n"),
                        (_, Some(r)) => writeln!(out, "  lda {}", register_name(r)).unwrap(),
                        (_, None) => unreachable!(),
                    };
                    let mut read = inst
                        .operands
                        .iter()
                        .filter_map(
                            |o| match o {
                                Operand::Use(Reg::Virt(_)) => Some(at(match o {
                                    Operand::Use(r) => *r,
                                    _ => unreachable!(),
                                })),
                                _ => None,
                            },
                        );
                    let false_at = matches!(if_false, Operand::Use(_)).then(|| read.next()).flatten();
                    let true_at = matches!(if_true, Operand::Use(_)).then(|| read.next()).flatten();
                    writeln!(out, "  {branch} {taken}").unwrap();
                    load(&mut out, if_false, false_at);
                    writeln!(out, "  jmp {done}\n{taken}:").unwrap();
                    load(&mut out, if_true, true_at);
                    writeln!(out, "{done}:").unwrap();
                }
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
    Ok((crate::emit::relax_bra(&relax(&out)), slots.iter().map(|(slot, _)| slot_name(*slot)).collect()))
}

/// A conditional branch reaches 127 bytes. An instruction is at most 3 bytes,
/// so one whose target is within 40 instruction lines stays and any other
/// becomes the opposite branch over a `jmp`.
///
/// DEVIATE: line counting, not sizes (as `emit::relax_bra`). Exit: the shared
/// branch relaxation.
fn relax(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_instruction = |l: &str| l.starts_with("  ") && !l.trim_start().starts_with('.');
    let mut out = String::new();
    let mut far = 0;
    for (at, line) in lines.iter().enumerate() {
        let parts: Vec<&str> = line.trim_start().split(' ').collect();
        let opposite = match parts[..] {
            ["beq", _] => "bne",
            ["bne", _] => "beq",
            ["bcc", _] => "bcs",
            ["bcs", _] => "bcc",
            ["bmi", _] => "bpl",
            ["bpl", _] => "bmi",
            _ => {
                writeln!(out, "{line}").unwrap();
                continue;
            }
        };
        let label = format!("{}:", parts[1]);
        let near = lines
            .iter()
            .position(|l| *l == label)
            .is_some_and(
                |to| {
                    let (from, until) = if to > at { (at + 1, to) } else { (to, at) };
                    lines[from..until].iter().filter(|l| is_instruction(l)).count() <= 40
                },
            );
        if near {
            writeln!(out, "{line}").unwrap();
        } else {
            far += 1;
            writeln!(out, "  {opposite} _far{far}\n  jmp {}\n_far{far}:", parts[1]).unwrap();
        }
    }
    out
}
