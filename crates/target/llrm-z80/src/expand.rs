//! After allocation: the pseudos become Z80 instructions, printed in sdasz80
//! syntax.
//!
//! # How it works
//! Every expansion is `ld` only where the pseudo is a move (`LO`, `HI`,
//! `PACK`, a copy, a spill), so none touches the flags a branch reads. A pseudo
//! that needs a scratch register the allocator has no way to name is refused
//! (`Err`), not expanded into whatever register happens to be free.
//!
//! DEVIATE: the text is written by hand per instruction, not from the `.td` asm
//! strings (they name no operands after allocation). Reason: nothing else reads
//! them yet. Exit: the MC printer.

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Assignment;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::{REGISTER_INFO, reg};

/// A line of the function: a label or an instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    Label(String),
    Inst(String),
}

/// The label of block `id` of the function numbered `number`.
pub fn label(
    number: usize,
    id: u32,
) -> String {
    format!("L{number}_{id}")
}

/// The stack frame addressed through IX: IX points at the saved IX, the
/// return address is above it, the arguments above that, and the frame objects
/// and spill slots below.
#[derive(Default)]
struct Frame {
    /// Bytes below IX.
    size: i64,
    /// The offset from IX of each object, by frame index.
    offsets: std::collections::HashMap<i32, i64>,
    /// Whether the function sets IX up at all.
    active: bool,
}

/// Bytes of return address and saved IX before the first argument
/// (`calling.toml`'s `first_argument_offset`).
const FIRST_ARGUMENT: i64 = 4;

struct Printer<'a> {
    assignment: &'a Assignment,
    number: usize,
    out: Vec<Line>,
    frame: Frame,
}

fn name(register: RegId) -> &'static str {
    REGISTER_INFO.name(register).expect("a register of the file")
}

/// The low and high byte of a pair.
fn lanes(pair: RegId) -> Result<(RegId, RegId), String> {
    if pair == reg::BC {
        Ok((reg::C, reg::B))
    } else if pair == reg::DE {
        Ok((reg::E, reg::D))
    } else if pair == reg::HL {
        Ok((reg::L, reg::H))
    } else {
        Err(format!("{} has no byte halves", name(pair)))
    }
}

fn is_pair(register: RegId) -> bool {
    register == reg::BC || register == reg::DE || register == reg::HL
}

impl Printer<'_> {
    fn push(
        &mut self,
        text: impl Into<String>,
    ) {
        self.out.push(Line::Inst(text.into()));
    }

    fn physical(
        &self,
        register: Reg,
    ) -> RegId {
        match register {
            Reg::Phys(id) => id,
            Reg::Virt(v) => self.assignment.0[v as usize],
        }
    }

    /// The register of operand `at`, a def or a use.
    fn register(
        &self,
        inst: &Inst,
        at: usize,
    ) -> Result<RegId, String> {
        match inst.operands.get(at) {
            Some(Operand::Def(r) | Operand::Use(r)) => Ok(self.physical(*r)),
            other => Err(format!("operand {at} of {:?} is {other:?}, not a register", inst.kind)),
        }
    }

    fn immediate(
        &self,
        inst: &Inst,
        at: usize,
        mask: i64,
    ) -> Result<String, String> {
        match inst.operands.get(at) {
            Some(Operand::Imm(value)) => Ok(format!("#{}", value & mask)),
            Some(Operand::Symbol(symbol, 0)) => Ok(format!("#{symbol}")),
            Some(Operand::Symbol(symbol, offset)) => Ok(format!("#{symbol}+{offset}")),
            other => Err(format!("operand {at} of {:?} is {other:?}, not an immediate", inst.kind)),
        }
    }

    fn byte_move(
        &mut self,
        to: RegId,
        from: RegId,
    ) {
        if to != from {
            self.push(format!("ld {}, {}", name(to), name(from)));
        }
    }

    /// A copy: bytes with `ld`, a pair as its two bytes.
    fn copy(
        &mut self,
        to: RegId,
        from: RegId,
    ) -> Result<(), String> {
        match (is_pair(to), is_pair(from)) {
            (false, false) => self.byte_move(to, from),
            (true, true) if to == from => {}
            (true, true) => {
                let ((dl, dh), (sl, sh)) = (lanes(to)?, lanes(from)?);
                self.byte_move(dl, sl);
                self.byte_move(dh, sh);
            }
            _ => return Err(format!("a copy of {} to {}", name(from), name(to))),
        }
        Ok(())
    }

    /// `PACK pair, lo, hi`: two byte moves, ordered so that neither reads a byte
    /// the other wrote.
    fn pack(
        &mut self,
        pair: RegId,
        lo: RegId,
        hi: RegId,
        rest: &[Inst],
    ) -> Result<(), String> {
        let (dl, dh) = lanes(pair)?;
        if dl == hi && dh == lo && lo != hi {
            // the two bytes of a pair trade places: through `a`, if nothing needs it
            if !self.dead_after(reg::A, rest) {
                return Err(format!("PACK swapping {} and {} needs a, which is live", name(lo), name(hi)));
            }
            self.push(format!("ld a, {}", name(dl)));
            self.push(format!("ld {}, {}", name(dl), name(dh)));
            self.push(format!("ld {}, a", name(dh)));
            return Ok(());
        }
        if dl == hi {
            self.byte_move(dh, hi);
            self.byte_move(dl, lo);
        } else {
            self.byte_move(dl, lo);
            self.byte_move(dh, hi);
        }
        Ok(())
    }

    fn instruction(
        &mut self,
        inst: &Inst,
        next: Option<u32>,
        rest: &[Inst],
    ) -> Result<(), String> {
        let name_of = |this: &Self, at| this.register(inst, at).map(name);
        let Kind::Op(op) = inst.kind else {
            let (to, from) = (self.register(inst, 0)?, self.register(inst, 1)?);
            return self.copy(to, from);
        };
        match op {
            "LO" | "HI" => {
                let (lo, hi) = lanes(self.register(inst, 1)?)?;
                let to = self.register(inst, 0)?;
                self.byte_move(to, if op == "LO" { lo } else { hi });
            }
            "PACK" => {
                let (pair, lo, hi) = (self.register(inst, 0)?, self.register(inst, 1)?, self.register(inst, 2)?);
                self.pack(pair, lo, hi, rest)?;
            }
            "ZEXT8" => {
                let (pair, from) = (self.register(inst, 0)?, self.register(inst, 1)?);
                let (dl, dh) = lanes(pair)?;
                // the byte is read before the high half is cleared
                self.byte_move(dl, from);
                self.push(format!("ld {}, #0", name(dh)));
            }
            "SEXT8" => {
                let (pair, _) = (self.register(inst, 0)?, self.register(inst, 1)?);
                let (dl, dh) = lanes(pair)?;
                self.push(format!("ld {}, a", name(dl)));
                self.push("rla");
                self.push("sbc a, a");
                self.push(format!("ld {}, a", name(dh)));
            }
            "ADD8r" | "SUB8r" | "AND8r" | "OR8r" | "XOR8r" => {
                let mnemonic = op[..op.len() - 2].to_lowercase();
                let source = name_of(self, 2)?;
                self.push(format!("{mnemonic} a, {source}"));
            }
            "ADD8i" | "SUB8i" | "AND8i" | "OR8i" | "XOR8i" => {
                let mnemonic = op[..op.len() - 2].to_lowercase();
                let value = self.immediate(inst, 2, 0xFF)?;
                self.push(format!("{mnemonic} a, {value}"));
            }
            "INC8" | "DEC8" | "INC16" | "DEC16" => {
                let mnemonic = op[..3].to_lowercase();
                let to = name_of(self, 0)?;
                self.push(format!("{mnemonic} {to}"));
            }
            "ADD16" => {
                let source = name_of(self, 2)?;
                self.push(format!("add hl, {source}"));
            }
            "SUB16" => {
                let source = name_of(self, 2)?;
                self.push("or a, a");
                self.push(format!("sbc hl, {source}"));
            }
            "SHL8" | "SRL8" | "SRA8" => {
                let to = name_of(self, 0)?;
                let mnemonic = match op {
                    "SHL8" => "sla",
                    "SRL8" => "srl",
                    _ => "sra",
                };
                for _ in 0..shift_count(inst)? {
                    self.push(format!("{mnemonic} {to}"));
                }
            }
            "SHL16" | "SRL16" | "SRA16" => {
                let (lo, hi) = lanes(self.register(inst, 0)?)?;
                let (lo, hi) = (name(lo), name(hi));
                for _ in 0..shift_count(inst)? {
                    match op {
                        "SHL16" => {
                            self.push(format!("sla {lo}"));
                            self.push(format!("rl {hi}"));
                        }
                        _ => {
                            self.push(format!("{} {hi}", if op == "SRL16" { "srl" } else { "sra" }));
                            self.push(format!("rr {lo}"));
                        }
                    }
                }
            }
            "LD8ri" => {
                let (to, value) = (name_of(self, 0)?, self.immediate(inst, 1, 0xFF)?);
                // `xor a, a` is a byte and three clocks less, and sets the flags
                if to == "a" && value == "#0" && flags_dead(rest) {
                    self.push("xor a, a");
                } else {
                    self.push(format!("ld {to}, {value}"));
                }
            }
            "LD16ri" => {
                let (to, value) = (name_of(self, 0)?, self.immediate(inst, 1, 0xFFFF)?);
                self.push(format!("ld {to}, {value}"));
            }
            "LD8rm" => {
                let to = name_of(self, 0)?;
                self.push(format!("ld {to}, (hl)"));
            }
            "LD8mr" => {
                let from = name_of(self, 0)?;
                self.push(format!("ld (hl), {from}"));
            }
            "LD8mi" => {
                let value = self.immediate(inst, 0, 0xFF)?;
                self.push(format!("ld (hl), {value}"));
            }
            "LD16rm" => {
                let to = self.register(inst, 0)?;
                if to == reg::HL {
                    // the pointer is overwritten: the low byte waits in a, which the pseudo clobbers
                    self.push("ld a, (hl)");
                    self.push("inc hl");
                    self.push("ld h, (hl)");
                    self.push("ld l, a");
                } else {
                    let (lo, hi) = lanes(to)?;
                    self.push(format!("ld {}, (hl)", name(lo)));
                    self.push("inc hl");
                    self.push(format!("ld {}, (hl)", name(hi)));
                    self.push("dec hl");
                }
            }
            "LD16mr" => {
                let from = self.register(inst, 0)?;
                if from == reg::HL {
                    return Err("LD16mr of hl through hl".to_owned());
                }
                let (lo, hi) = lanes(from)?;
                self.push(format!("ld (hl), {}", name(lo)));
                self.push("inc hl");
                self.push(format!("ld (hl), {}", name(hi)));
                self.push("dec hl");
            }
            "LD8am" | "LD16hm" => {
                let (to, at) = (name_of(self, 0)?, self.address(inst, 1)?);
                self.push(format!("ld {to}, ({at})"));
            }
            "LD8ma" | "LD16mh" => {
                let (from, at) = (name_of(self, 0)?, self.address(inst, 1)?);
                self.push(format!("ld ({at}), {from}"));
            }
            "CP8r" => {
                let source = name_of(self, 2)?;
                self.push(format!("cp a, {source}"));
            }
            "CP8i" => {
                let value = self.immediate(inst, 2, 0xFF)?;
                // `or a, a` is a byte and three clocks less. It differs from `cp a, 0` in
                // the parity/overflow and half-carry flags, which no branch here reads.
                self.push(if value == "#0" { "or a, a".to_owned() } else { format!("cp a, {value}") });
            }
            "CP16" => self.compare16(inst, rest)?,
            "AND16" | "OR16" | "XOR16" => {
                let mnemonic = op[..op.len() - 2].to_lowercase();
                let (to, a, b) = (self.register(inst, 0)?, self.register(inst, 1)?, self.register(inst, 2)?);
                let ((dl, dh), (al, ah), (bl, bh)) = (lanes(to)?, lanes(a)?, lanes(b)?);
                for (d, x, y) in [(dl, al, bl), (dh, ah, bh)] {
                    self.push(format!("ld a, {}", name(x)));
                    self.push(format!("{mnemonic} a, {}", name(y)));
                    self.push(format!("ld {}, a", name(d)));
                }
            }
            "JP" | "JPNZ" | "JPZ" | "JPNC" | "JPC" => {
                let Some(Operand::Block(target)) = inst.operands.first() else {
                    return Err(format!("{op} without a target"));
                };
                if op == "JP" && Some(*target) == next {
                    return Ok(());
                }
                let target = label(self.number, *target);
                match op {
                    "JP" => self.push(format!("jp {target}")),
                    _ => self.push(format!("jp {}, {target}", op[2..].to_lowercase())),
                }
            }
            "CALL" => {
                let Some(Operand::Symbol(symbol, _)) = inst.operands.first() else {
                    return Err("a CALL that names no symbol".to_owned());
                };
                self.push(format!("call {symbol}"));
            }
            "ADJCALLSTACKDOWN" | "ADJCALLSTACKUP" => {
                if matches!(inst.operands.first(), Some(Operand::Imm(bytes)) if *bytes != 0) {
                    return Err("stack arguments are not expanded yet".to_owned());
                }
            }
            "SPILL8" | "RELOAD8" => {
                let (index, register) = if op == "SPILL8" {
                    (self.frame_index(inst, 0)?, 1)
                } else {
                    (self.frame_index(inst, 1)?, 0)
                };
                let (register, slot) = (name_of(self, register)?, self.slot(index, 0)?);
                self.push(if op == "SPILL8" { format!("ld {slot}, {register}") } else { format!("ld {register}, {slot}") });
            }
            "SPILL16" | "RELOAD16" => {
                let (index, register) = if op == "SPILL16" {
                    (self.frame_index(inst, 0)?, 1)
                } else {
                    (self.frame_index(inst, 1)?, 0)
                };
                let (lo, hi) = lanes(self.register(inst, register)?)?;
                for (byte, extra) in [(lo, 0), (hi, 1)] {
                    let slot = self.slot(index, extra)?;
                    self.push(if op == "SPILL16" {
                        format!("ld {slot}, {}", name(byte))
                    } else {
                        format!("ld {}, {slot}", name(byte))
                    });
                }
            }
            "RET" => {
                if self.frame.active {
                    self.push("ld sp, ix");
                    self.push("pop ix");
                }
                self.push("ret");
            }
            other => return Err(format!("{other} is not expanded yet")),
        }
        Ok(())
    }

    /// Whether the byte register `register` holds nothing a later instruction of
    /// the block reads: the first mention after this point defines it without
    /// reading it. A block that ends with `ret` before any mention is dead too
    /// (what the function returns is a `ret` operand); any other end is not
    /// known.
    fn dead_after(
        &self,
        register: RegId,
        rest: &[Inst],
    ) -> bool {
        for inst in rest {
            let mentions = |want_def: bool| {
                inst.operands.iter().any(|operand| match operand {
                    Operand::Def(r) if want_def => self.physical(*r) == register,
                    Operand::Use(r) if !want_def => self.physical(*r) == register,
                    _ => false,
                })
            };
            if mentions(false) {
                return false;
            }
            if mentions(true) {
                return true;
            }
        }
        matches!(rest.last().map(|inst| inst.kind), Some(Kind::Op("RET")))
    }

    /// `CP16 a, b`: the flags of `a - b` through `a`, which the pseudo clobbers.
    /// What follows says which flag is read: `sub` and `sbc` give the carry of the
    /// word; zero needs both bytes, so the second compare is skipped when the first
    /// differs.
    fn compare16(
        &mut self,
        inst: &Inst,
        rest: &[Inst],
    ) -> Result<(), String> {
        let ((al, ah), (bl, bh)) = (lanes(self.register(inst, 1)?)?, lanes(self.register(inst, 2)?)?);
        let reader = rest.iter().find(|one| one.reads_flags).and_then(|one| match one.kind {
            Kind::Op(name) => Some(name),
            Kind::Copy => None,
        });
        match reader {
            Some("JPC" | "JPNC") => {
                self.push(format!("ld a, {}", name(al)));
                self.push(format!("sub a, {}", name(bl)));
                self.push(format!("ld a, {}", name(ah)));
                self.push(format!("sbc a, {}", name(bh)));
            }
            Some("JPZ" | "JPNZ") => {
                let skip = format!("{}$", self.out.len());
                self.push(format!("ld a, {}", name(al)));
                self.push(format!("sub a, {}", name(bl)));
                self.push(format!("jr nz, {skip}"));
                self.push(format!("ld a, {}", name(ah)));
                self.push(format!("sub a, {}", name(bh)));
                self.out.push(Line::Label(skip));
            }
            other => return Err(format!("CP16 read by {other:?}")),
        }
        Ok(())
    }

    /// `d(ix)` for byte `extra` of frame object `index`.
    fn slot(
        &self,
        index: i32,
        extra: i64,
    ) -> Result<String, String> {
        let offset = *self.frame.offsets.get(&index).ok_or_else(|| format!("frame object {index} has no place"))?;
        let at = offset + extra;
        if !(-128..=127).contains(&at) {
            return Err(format!("frame object {index} is {at} from ix, out of the byte displacement"));
        }
        Ok(format!("{at}(ix)"))
    }

    fn frame_index(
        &self,
        inst: &Inst,
        at: usize,
    ) -> Result<i32, String> {
        match inst.operands.get(at) {
            Some(Operand::Frame(index)) => Ok(*index),
            other => Err(format!("operand {at} of {:?} is {other:?}, not a frame object", inst.kind)),
        }
    }

    /// `(nn)`'s address: a symbol and offset, or a number.
    fn address(
        &self,
        inst: &Inst,
        at: usize,
    ) -> Result<String, String> {
        Ok(self.immediate(inst, at, 0xFFFF)?.trim_start_matches('#').to_owned())
    }
}

/// Whether nothing in `rest` reads the flags before something sets them again.
/// A block ends with them dead: only a branch reads them, and it follows its
/// own compare in the block.
fn flags_dead(rest: &[Inst]) -> bool {
    for inst in rest {
        if inst.reads_flags {
            return false;
        }
        if inst.sets_flags {
            return true;
        }
    }
    true
}

fn shift_count(inst: &Inst) -> Result<i64, String> {
    match inst.operands.last() {
        Some(Operand::Imm(count)) => Ok(*count),
        other => Err(format!("a shift by {other:?}")),
    }
}

/// The frame of `function`: its static allocas, then the spill `slots`.
fn layout(
    function: &MFunction,
    slots: &[(i32, usize)],
) -> Frame {
    let mut frame = Frame::default();
    for &(index, size, _) in &function.locals {
        frame.size += size;
        frame.offsets.insert(index, -frame.size);
    }
    for &(index, size) in slots {
        frame.size += size as i64;
        frame.offsets.insert(index, -frame.size);
    }
    for (&index, &offset) in &function.fixed {
        frame.offsets.insert(index, FIRST_ARGUMENT + offset);
    }
    frame.active = frame.size > 0 || !function.fixed.is_empty();
    frame
}

/// `push ix; ld ix, 0; add ix, sp`, then room for the frame: a `push af` per
/// two bytes, or `iy` as the scratch for a large frame. Nothing here touches
/// the registers that arguments arrive in.
fn prologue(frame: &Frame) -> Vec<String> {
    let mut lines = vec!["push ix".to_owned(), "ld ix, #0".to_owned(), "add ix, sp".to_owned()];
    let pushes = (frame.size + 1) / 2;
    if pushes <= 3 {
        lines.extend((0..pushes).map(|_| "push af".to_owned()));
    } else {
        lines.extend(["ld iy, #".to_owned() + &format!("{}", -frame.size), "add iy, sp".to_owned(), "ld sp, iy".to_owned()]);
    }
    lines
}

/// The instructions of `function` with the registers `assignment` gave them.
/// `slots` are the spill slots and their sizes in bytes.
pub fn finish(
    function: &MFunction,
    assignment: &Assignment,
    number: usize,
    slots: &[(i32, usize)],
) -> Result<Vec<Line>, String> {
    let frame = layout(function, slots);
    let mut printer = Printer { assignment, number, out: Vec::new(), frame };
    if printer.frame.active {
        for line in prologue(&printer.frame) {
            printer.push(line);
        }
    }
    for (at, block) in function.blocks.iter().enumerate() {
        printer.out.push(Line::Label(label(number, block.id)));
        let next = function.blocks.get(at + 1).map(|block| block.id);
        for (index, inst) in block.insts.iter().enumerate() {
            printer.instruction(inst, next, &block.insts[index + 1..])?;
        }
    }
    Ok(printer.out)
}
