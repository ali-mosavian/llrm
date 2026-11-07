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

use llrm_codegen::backend::frameobjects::{Sorting, order_frame_objects};
use llrm_lir::registers::RegId;
use llrm_spike_mf::livephys::PhysLiveness;
use llrm_spike_mf::regalloc::Assignment;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::Z80Registers;
use crate::{REGISTER_INFO, reg};

/// A line of the function: a label or an instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    Label(String),
    Inst(String),
    /// The source line of the instructions that follow (`-g`); no code.
    Loc(u32),
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
    offsets: llrm_support::hash::HashMap<i32, i64>,
    /// Whether the function sets IX up at all.
    active: bool,
}

impl Frame {
    /// How far the prologue leaves SP below IX.
    fn below_ix(&self) -> i64 {
        let pushes = (self.size + 1) / 2;
        if pushes <= 3 { 2 * pushes } else { self.size }
    }
}

/// Bytes of return address and saved IX before the first argument
/// (`calling.toml`'s `first_argument_offset`).
const FIRST_ARGUMENT: i64 = 4;

struct Printer<'a> {
    assignment: &'a Assignment,
    number: usize,
    out: Vec<Line>,
    frame: Frame,
    /// The blocks that are only a `ret`.
    returns_bare: Vec<u32>,
    /// How many times each virtual register is read.
    reads: llrm_support::hash::HashMap<u32, usize>,
    /// A reload the next instruction reads from the frame itself: the register
    /// it would have gone to, and the cell.
    folded: Option<(RegId, String)>,
    /// Instructions after the current one that were printed with it.
    skip: usize,
    /// Bytes of stack arguments reserved below the frame, while a call is being
    /// set up.
    call_bytes: i64,
    /// Bytes of spills pushed (`PUSHP`) and not yet popped: they lie above the
    /// stack arguments of a call.
    spill_bytes: i64,
    /// The live lanes of the allocated function, and the block and
    /// instruction being printed.
    liveness: llrm_spike_mf::livephys::PhysLiveness,
    function: &'a MFunction,
    at: (usize, usize),
}

fn name(register: RegId) -> &'static str {
    REGISTER_INFO.name(register).expect("a register of the file")
}

/// The low and high byte of a pair.
pub(crate) fn lanes(pair: RegId) -> Result<(RegId, RegId), String> {
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

/// SDCC's register list of a value held in `register`: `[l,h]` for a pair.
pub(crate) fn register_list(register: RegId) -> String {
    match lanes(register) {
        Ok((low, high)) => format!("[{},{}]", name(low), name(high)),
        Err(_) => format!("[{}]", name(register)),
    }
}

/// `hl`, `bc` or `de` as the pointer of an `ld a,(rr)`.
fn pointer(register: RegId) -> Result<&'static str, String> {
    if is_pair(register) { Ok(name(register)) } else { Err(format!("{} is no pointer pair", name(register))) }
}

thread_local! {
    /// Set while a variant is priced: the printer leaves its peepholes out.
    pub static PRICING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn is_pair(register: RegId) -> bool {
    register == reg::BC || register == reg::DE || register == reg::HL
}

impl Printer<'_> {
    fn push(
        &mut self,
        text: impl Into<String>,
    ) {
        let text = text.into();
        // `ld r, a` then `ld a, r`: `a` still holds what `r` was just given and
        // neither load touches the flags (an `and`/`or` chain on one byte went
        // `ld c, a; ld a, c` between its steps). Registers only: a memory
        // cell may be volatile. Not while pricing: the price at the speed
        // levels is taken before the peepholes.
        if let Some(from) = text.strip_prefix("ld a, ").filter(|from| from.len() == 1)
            && matches!(self.out.last(), Some(Line::Inst(last)) if *last == format!("ld {from}, a"))
            && !PRICING.with(|one| one.get())
        {
            return;
        }
        self.out.push(Line::Inst(text));
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
        rest: &[Inst],
    ) -> Result<(), String> {
        if to == reg::HL && is_pair(from) && self.through_own_pair(from, rest)? {
            return Ok(());
        }
        match (is_pair(to), is_pair(from)) {
            // iy has no halves the Z80 can move: through the stack
            _ if to == reg::IY && is_pair(from) => {
                self.push(format!("push {}", name(from)));
                self.push("pop iy");
            }
            _ if from == reg::IY && is_pair(to) => {
                self.push("push iy");
                self.push(format!("pop {}", name(to)));
            }
            (false, false) => self.byte_move(to, from),
            (true, true) if to == from => {}
            // `de` and `hl` trade places in one byte; the source is dead, so
            // what lands in it is too
            (true, true) if to != reg::BC && from != reg::BC && self.pair_dead_after(from, rest) => {
                self.push("ex de, hl");
            }
            (true, true) => {
                let ((dl, dh), (sl, sh)) = (lanes(to)?, lanes(from)?);
                // a half nothing reads before it is written again is not moved
                let after = self
                    .liveness
                    .live_before::<Z80Registers>(
                        self.function,
                        self.assignment,
                        &REGISTER_INFO,
                        self.at.0,
                        self.at.1 + 1,
                    );
                if after.is_live(&REGISTER_INFO, dl) {
                    self.byte_move(dl, sl);
                }
                if after.is_live(&REGISTER_INFO, dh) {
                    self.byte_move(dh, sh);
                }
            }
            _ => return Err(format!("a copy of {} to {}", name(from), name(to))),
        }
        Ok(())
    }

    /// A pointer copied into `hl` only for the next instruction to load into
    /// or store from `a`, with `hl` dead after it, is read through its own
    /// pair (`ld a, (bc)`): no copy. Emits the access and skips the
    /// instruction; false when the pattern does not hold.
    fn through_own_pair(
        &mut self,
        from: RegId,
        rest: &[Inst],
    ) -> Result<bool, String> {
        let Some(next) = rest.first() else { return Ok(false) };
        let load = match next.kind {
            Kind::Op("LD8rm") => true,
            Kind::Op("LD8mr") => false,
            _ => return Ok(false),
        };
        if self.register(next, 0)? != reg::A || self.register(next, 1)? != reg::HL {
            return Ok(false);
        }
        let after = self
            .liveness
            .live_before::<Z80Registers>(self.function, self.assignment, &REGISTER_INFO, self.at.0, self.at.1 + 2);
        if after.is_live(&REGISTER_INFO, reg::HL) {
            return Ok(false);
        }
        let pair = pointer(from)?;
        self.push(if load { format!("ld a, ({pair})") } else { format!("ld ({pair}), a") });
        self.skip = 1;
        Ok(true)
    }

    /// `PACK pair, lo, hi`: two byte moves, ordered so that neither reads a
    /// byte the other wrote.
    fn pack(
        &mut self,
        pair: RegId,
        lo: RegId,
        hi: RegId,
        rest: &[Inst],
    ) -> Result<(), String> {
        let (dl, dh) = lanes(pair)?;
        if dl == hi && dh == lo && lo != hi {
            // the two bytes of a pair trade places: through `a`, if nothing
            // needs it
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
            return self.copy(to, from, rest);
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
            "SUBZX8" => {
                // the subtraction's low byte, then its borrow as the high byte
                let (pair, from) = (self.register(inst, 0)?, self.source(inst, 2)?);
                let (dl, dh) = lanes(pair)?;
                self.push(format!("sub a, {from}"));
                self.push(format!("ld {}, a", name(dl)));
                self.push("sbc a, a");
                self.push(format!("ld {}, a", name(dh)));
            }
            "ADD8r" | "SUB8r" | "AND8r" | "OR8r" | "XOR8r" => {
                let mnemonic = op[..op.len() - 2].to_lowercase();
                let source = self.source(inst, 2)?;
                self.push(format!("{mnemonic} a, {source}"));
            }
            "ADD8m" | "SUB8m" | "AND8m" | "OR8m" | "XOR8m" => {
                let mnemonic = op[..op.len() - 2].to_lowercase();
                self.push(format!("{mnemonic} a, (hl)"));
            }
            "CP8m" => self.push("cp a, (hl)"),
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
            // the word stepped when the condition holds: `jr` over the step
            // on its opposite (a step leaves the flags alone)
            "INC16c" | "DEC16c" => {
                let Some(Operand::Imm(cond)) = inst.operands.get(2) else {
                    return Err("a conditional step without a condition".to_owned());
                };
                let opposite =
                    ["z", "nz", "c", "nc"].get(*cond as usize).ok_or("a conditional step on an unknown condition")?;
                let to = name_of(self, 0)?;
                let skip = format!("{}$", self.out.iter().filter(|line| !matches!(line, Line::Loc(_))).count());
                self.push(format!("jr {opposite}, {skip}"));
                self.push(format!("{} {to}", op[..3].to_lowercase()));
                self.out.push(Line::Label(skip));
            }
            "INC16x2" | "INC16x3" | "DEC16x2" | "DEC16x3" => {
                let mnemonic = op[..3].to_lowercase();
                let to = name_of(self, 0)?;
                for _ in 0..op[6..].parse::<u32>().map_err(|_| format!("{op} is not a step"))? {
                    self.push(format!("{mnemonic} {to}"));
                }
            }
            "ADD16" => {
                let source = self.register(inst, 2)?;
                if !self.add_small_constant(source, rest) && !self.step_pair_through_hl(source, rest) {
                    self.push(format!("add hl, {}", name(source)));
                }
            }
            "ADC16" | "SBC16" => {
                let source = name_of(self, 2)?;
                self.push(format!("{} hl, {source}", op[..3].to_lowercase()));
            }
            "SUB16" => {
                let source = name_of(self, 2)?;
                self.push("or a, a");
                self.push(format!("sbc hl, {source}"));
            }
            // in `a` a left shift is `add a, a` (1 B / 4 T a bit against `sla
            // a`'s 2 B / 8 T), and a logical right shift by two or more, where
            // nothing reads the flags, rotates the bits round (the shorter way)
            // and masks: `rlca` x3, `and #7` for `>> 5`
            "SHL8" if name_of(self, 0)? == "a" => {
                for _ in 0..shift_count(inst)? {
                    self.push("add a, a");
                }
            }
            "SRL8" if name_of(self, 0)? == "a" && (2..8).contains(&shift_count(inst)?) && self.flags_dead(rest) => {
                let count = shift_count(inst)?;
                let (rotate, turns) = if count <= 4 { ("rrca", count) } else { ("rlca", 8 - count) };
                for _ in 0..turns {
                    self.push(rotate);
                }
                self.push(format!("and a, #{}", 0xFFu32 >> count));
            }
            // the sign of the accumulator: its bit 7 into the carry and
            // out again (`sra a` seven times is 14 B)
            "SRA8" if shift_count(inst)? == 7 && name_of(self, 0)? == "a" => {
                self.push("add a, a");
                self.push("sbc a, a");
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
            "SHL16" | "SRL16" if shift_count(inst)? >= 8 => {
                // a whole byte moves across: the other byte is cleared, the
                // rest is a byte shift
                let count = shift_count(inst)?;
                let (lo, hi) = lanes(self.register(inst, 0)?)?;
                let (kept, cleared, shifted) = if op == "SHL16" { (lo, hi, "sla") } else { (hi, lo, "srl") };
                self.push(format!("ld {}, {}", name(cleared), name(kept)));
                self.push(format!("ld {}, #0", name(kept)));
                if count > 8 {
                    for _ in 0..count - 8 {
                        self.push(format!("{shifted} {}", name(cleared)));
                    }
                }
            }
            // `add hl, hl` is a doubling: 1 B / 11 T a bit against `sla l; rl
            // h`'s 4 B / 16 T
            "SHL16" if self.register(inst, 0)? == reg::HL => {
                for _ in 0..shift_count(inst)? {
                    self.push("add hl, hl");
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
                // `xor a, a` is a byte and three clocks less, and sets the
                // flags
                if to == "a" && value == "#0" && self.flags_dead(rest) {
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
                // a byte widened at once is loaded into the low half of the
                // pair: no byte register, no copy
                let widened = match (inst.operands.first(), rest.first()) {
                    (Some(Operand::Def(Reg::Virt(loaded))), Some(next))
                        if next.kind == Kind::Op("ZEXT8")
                            && next.operands.get(1) == Some(&Operand::Use(Reg::Virt(*loaded)))
                            && self.reads.get(loaded) == Some(&1) =>
                    {
                        Some(self.register(next, 0)?)
                    }
                    _ => None,
                };
                if let Some(pair) = widened {
                    let (lo, hi) = lanes(pair)?;
                    self.push(format!("ld {}, (hl)", name(lo)));
                    self.push(format!("ld {}, #0", name(hi)));
                    self.skip = 1;
                    return Ok(());
                }
                let to = name_of(self, 0)?;
                self.push(format!("ld {to}, (hl)"));
            }
            "LD8Arp" => {
                let p = self.register(inst, 1)?;
                self.push(format!("ld a, ({})", pointer(p)?));
            }
            "LD8prA" => {
                let p = self.register(inst, 1)?;
                self.push(format!("ld ({}), a", pointer(p)?));
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
                    // the pointer is overwritten: the low byte waits in a,
                    // which the pseudo clobbers
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
            "LD8am" | "LD16rg" => {
                let (to, at) = (name_of(self, 0)?, self.address(inst, 1)?);
                self.push(format!("ld {to}, ({at})"));
            }
            "LD8ma" | "LD16gr" => {
                let (from, at) = (name_of(self, 0)?, self.address(inst, 1)?);
                self.push(format!("ld ({at}), {from}"));
            }
            "BIT7_8" => {
                let source = name_of(self, 1)?;
                self.push(format!("bit 7, {source}"));
            }
            "BIT7_16" => {
                let (_, hi) = lanes(self.register(inst, 1)?)?;
                self.push(format!("bit 7, {}", name(hi)));
            }
            "CP8r" => {
                let source = self.source(inst, 2)?;
                self.push(format!("cp a, {source}"));
            }
            "CP8i" => {
                let value = self.immediate(inst, 2, 0xFF)?;
                // `or a, a` is a byte and three clocks less. It differs from
                // `cp a, 0` in the parity/overflow and
                // half-carry flags, which no branch here reads.
                self.push(if value == "#0" { "or a, a".to_owned() } else { format!("cp a, {value}") });
            }
            "CP16" | "CP16i" | "CP16C" | "CP16Ci" => self.compare16(inst, rest)?,
            // `ld` leaves the flags alone: the byte is 1, and 0 when the
            // condition fails
            "SETCC8" => {
                let to = name_of(self, 0)?;
                let Some(Operand::Imm(cond)) = inst.operands.get(1) else {
                    return Err("SETCC8 without a condition".to_owned());
                };
                let cond = ["nz", "z", "nc", "c"].get(*cond as usize).ok_or("SETCC8 with an unknown condition")?;
                let skip = format!("{}$", self.out.iter().filter(|line| !matches!(line, Line::Loc(_))).count());
                self.push(format!("ld {to}, #1"));
                self.push(format!("jr {cond}, {skip}"));
                // `dec` would change the zero flag a later reader of the same
                // compare wants
                self.push(if self.flags_dead(rest) { format!("dec {to}") } else { format!("ld {to}, #0") });
                self.out.push(Line::Label(skip));
            }
            // destination hl, source de, count bc: `ex de,hl` makes them
            // `ldir`'s. A count of zero is 65536 here; `LDIRG` skips it.
            "LDIR" => {
                self.push("ex de, hl");
                self.push("ldir");
            }
            "LDIRG" => {
                let skip = format!("{}$", self.out.iter().filter(|line| !matches!(line, Line::Loc(_))).count());
                self.push("ld a, b");
                self.push("or a, c");
                self.push(format!("jr z, {skip}"));
                self.push("ex de, hl");
                self.push("ldir");
                self.out.push(Line::Label(skip));
            }
            "AND16i" | "OR16i" | "XOR16i" => {
                let mnemonic = op[..op.len() - 3].to_lowercase();
                let (to, a) = (self.register(inst, 0)?, self.register(inst, 1)?);
                let Some(Operand::Imm(mask)) = inst.operands.get(2) else {
                    return Err("a word logic op with no mask".to_owned());
                };
                let ((dl, dh), (al, ah)) = (lanes(to)?, lanes(a)?);
                for (d, x, byte) in [(dl, al, mask & 0xFF), (dh, ah, (mask >> 8) & 0xFF)] {
                    match (mnemonic.as_str(), byte) {
                        ("and", 0xFF) | ("or", 0) | ("xor", 0) => self.byte_move(d, x),
                        ("and", 0) => self.push(format!("ld {}, #0", name(d))),
                        ("or", 0xFF) => self.push(format!("ld {}, #255", name(d))),
                        _ => {
                            self.push(format!("ld a, {}", name(x)));
                            self.push(format!("{mnemonic} a, #{byte}"));
                            self.push(format!("ld {}, a", name(d)));
                        }
                    }
                }
            }
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
                // a branch to a bare `ret` is a `ret` (a byte, not three)
                if self.returns_bare.contains(target) && !self.frame.active {
                    if op == "JP" {
                        self.push("ret");
                    } else {
                        self.push(format!("ret {}", op[2..].to_lowercase()));
                    }
                    return Ok(());
                }
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
            "CALLiy" => self.push("call __call_iy"),
            "TAILCALL" => {
                let Some(Operand::Symbol(symbol, _)) = inst.operands.first() else {
                    return Err("a tail call that names no symbol".to_owned());
                };
                if self.frame.active {
                    self.push("ld sp, ix");
                    self.push("pop ix");
                }
                self.push(format!("jp {symbol}"));
            }
            // The area is `push bc`s (bc is only saved and restored: no
            // register is lost, and no result lives in it); a store into it is
            // addressed from IX, so the frame is set up.
            "ADJCALLSTACKDOWN" | "ADJCALLSTACKUP" => {
                let Some(Operand::Imm(bytes)) = inst.operands.first() else { return Ok(()) };
                if *bytes == 0 {
                    return Ok(());
                }
                let pushes = (*bytes + 1) / 2;
                if pushes > 8 {
                    return Err(format!("{bytes} bytes of stack arguments"));
                }
                let down = op == "ADJCALLSTACKDOWN";
                for _ in 0..pushes {
                    self.push(if down { "push bc" } else { "pop bc" });
                }
                self.call_bytes = if down { 2 * pushes } else { 0 };
            }
            "PUSHARG" => {
                let pair = self.register(inst, 0)?;
                self.push(format!("push {}", name(pair)));
                self.call_bytes += 2;
            }
            "STSP8" | "STSP16" => {
                let Some(Operand::Imm(offset)) = inst.operands.get(1) else {
                    return Err("a stack store without an offset".to_owned());
                };
                // The area is addressed from IX when its bytes are within the
                // displacement; a frame past that sets IY to the area's
                // first byte (`ld iy, #n; add iy, sp`), when nothing holds IY.
                let width = if op == "STSP8" { 1 } else { 2 };
                let from_ix: Vec<i64> = (0..width)
                    .map(|extra| offset + extra - self.frame.below_ix() - self.call_bytes - self.spill_bytes)
                    .collect();
                let places: Vec<String> = if from_ix.iter().all(|at| (-128..=127).contains(at)) {
                    from_ix.iter().map(|at| format!("{at}(ix)")).collect()
                } else if self.iy_is_live() {
                    return Err(format!("a stack argument {} from ix and iy is live", from_ix[0]));
                } else {
                    self.iy_to_stack(*offset);
                    (0..width).map(|byte| format!("{byte}(iy)")).collect()
                };
                if op == "STSP8" {
                    let value = name_of(self, 0)?;
                    self.push(format!("ld {}, {value}", places[0]));
                } else {
                    let (lo, hi) = lanes(self.register(inst, 0)?)?;
                    self.push(format!("ld {}, {}", places[0], name(lo)));
                    self.push(format!("ld {}, {}", places[1], name(hi)));
                }
            }
            "LEAFI" => {
                // the stack pointer sits at the bottom of the frame the
                // prologue made, and the stack arguments a call has pushed
                let offset = self
                    .frame
                    .offsets
                    .get(&self.frame_index(inst, 1)?)
                    .copied()
                    .ok_or("a frame object with no place")?;
                self.push(format!("ld hl, #{}", offset + self.frame.below_ix() + self.call_bytes + self.spill_bytes));
                self.push("add hl, sp");
            }
            "LD8rf" | "LD16rf" | "LD8fr" | "LD16fr" | "LD8fi" => {
                // register first for a load, the stored value first for a
                // store; the frame object and its offset last
                let load = op.ends_with("rf");
                let at = 1;
                let index = self.frame_index(inst, at)?;
                let extra = match inst.operands.get(at + 1) {
                    Some(Operand::Imm(extra)) => *extra,
                    other => return Err(format!("a frame offset that is {other:?}")),
                };
                if op == "LD8fi" {
                    let value = self.immediate(inst, 0, 0xFF)?;
                    let slot = self.frame_slots(index, extra, 1)?.remove(0);
                    self.push(format!("ld {slot}, {value}"));
                } else if op.starts_with("LD8") {
                    let register = name_of(self, 0)?;
                    let slot = self.frame_slots(index, extra, 1)?.remove(0);
                    self.push(if load { format!("ld {register}, {slot}") } else { format!("ld {slot}, {register}") });
                } else {
                    let (lo, hi) = lanes(self.register(inst, 0)?)?;
                    let slots = self.frame_slots(index, extra, 2)?;
                    for (byte, slot) in [lo, hi].into_iter().zip(slots) {
                        self.push(if load {
                            format!("ld {}, {slot}", name(byte))
                        } else {
                            format!("ld {slot}, {}", name(byte))
                        });
                    }
                }
            }
            "PUSHP" | "POPP" | "PEEKP" => {
                if self.call_bytes != 0 {
                    return Err("a pushed spill inside the stack arguments of a call".to_owned());
                }
                let register = self.register(inst, 0)?;
                let pair = [reg::BC, reg::DE, reg::HL]
                    .into_iter()
                    .find(|pair| REGISTER_INFO.root(register) == *pair)
                    .ok_or("a push of something that is no pair")?;
                if op == "PEEKP" {
                    // the slot on top stays: it is read again
                    self.push(format!("pop {}", name(pair)));
                    self.push(format!("push {}", name(pair)));
                    return Ok(());
                }
                self.push(format!("{} {}", if op == "PUSHP" { "push" } else { "pop" }, name(pair)));
                self.spill_bytes += if op == "PUSHP" { 2 } else { -2 };
            }
            "SPILL8" | "RELOAD8" => {
                let (index, register) =
                    if op == "SPILL8" { (self.frame_index(inst, 0)?, 1) } else { (self.frame_index(inst, 1)?, 0) };
                let register = name_of(self, register)?;
                if op == "RELOAD8" && self.updated_in_place(inst, rest)? {
                    return Ok(());
                }
                let slot = self.frame_slots(index, 0, 1)?.remove(0);
                if op == "RELOAD8" && self.held_since_stored(&[(slot.clone(), register.to_owned())]) {
                    return Ok(());
                }
                if op == "SPILL8" && just_reloaded(&self.out, &[(slot.clone(), register.to_owned())]) {
                    return Ok(());
                }
                if op == "RELOAD8" && self.reload_is_operand(inst, rest) {
                    self.folded = Some((self.register(inst, 0)?, slot));
                    return Ok(());
                }
                self.push(if op == "SPILL8" {
                    format!("ld {slot}, {register}")
                } else {
                    format!("ld {register}, {slot}")
                });
            }
            "SPILL8L" => {
                let index = self.frame_index(inst, 0)?;
                let register = name_of(self, 1)?;
                let Some(Operand::Imm(offset)) = inst.operands.get(2) else {
                    return Err("a lane spill with no offset".to_owned());
                };
                let slot = self.frame_slots(index, *offset, 1)?.remove(0);
                self.push(format!("ld {slot}, {register}"));
            }
            "RELOAD8L" => {
                let index = self.frame_index(inst, 1)?;
                let register = name_of(self, 0)?;
                let Some(Operand::Imm(offset)) = inst.operands.get(2) else {
                    return Err("a lane reload with no offset".to_owned());
                };
                let slot = self.frame_slots(index, *offset, 1)?.remove(0);
                self.push(format!("ld {register}, {slot}"));
            }
            "SPILL16" | "RELOAD16" => {
                let (index, register) =
                    if op == "SPILL16" { (self.frame_index(inst, 0)?, 1) } else { (self.frame_index(inst, 1)?, 0) };
                let (lo, hi) = lanes(self.register(inst, register)?)?;
                if op == "RELOAD16"
                    && self.call_bytes == 0
                    && self.spill_bytes == 0
                    && self.frame.offsets.get(&index) == Some(&-self.frame.below_ix())
                {
                    // the slot at SP: `pop` then `push` is 2 B where two loads
                    // are 6
                    let pair = name(self.register(inst, register)?);
                    self.push(format!("pop {pair}"));
                    self.push(format!("push {pair}"));
                    return Ok(());
                }
                let slots = self.frame_slots(index, 0, 2)?;
                if op == "RELOAD16" {
                    let stores: Vec<(String, String)> =
                        slots.iter().zip([lo, hi]).map(|(slot, byte)| (slot.clone(), name(byte).to_owned())).collect();
                    if self.held_since_stored(&stores) {
                        return Ok(());
                    }
                } else {
                    let loaded: Vec<(String, String)> =
                        slots.iter().zip([lo, hi]).map(|(slot, byte)| (slot.clone(), name(byte).to_owned())).collect();
                    if just_reloaded(&self.out, &loaded) {
                        return Ok(());
                    }
                }
                for (byte, slot) in [lo, hi].into_iter().zip(slots) {
                    self.push(if op == "SPILL16" {
                        format!("ld {slot}, {}", name(byte))
                    } else {
                        format!("ld {}, {slot}", name(byte))
                    });
                }
            }
            "RETNZ" | "RETZ" | "RETNC" | "RETC" => {
                if self.frame.active {
                    return Err("a conditional return in a function with a frame".to_owned());
                }
                self.push(format!("ret {}", op[3..].to_lowercase()));
            }
            "UNREACHABLE" => {}
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

    /// `ld rr, #k` then `add hl, rr` with k in 2..=3 or -3..=-2, `rr` and the
    /// carry dead after: `inc hl` / `dec hl` k times, 2-3 B against 4 B. It is
    /// done here, not in selection, where a pointer that stays live needs a
    /// copy and the allocation gets worse.
    fn add_small_constant(
        &mut self,
        source: RegId,
        rest: &[Inst],
    ) -> bool {
        if PRICING.with(|one| one.get()) {
            return false;
        }
        let Some(Line::Inst(last)) = self.out.last() else { return false };
        let Some(value) = last
            .strip_prefix("ld ")
            .and_then(|text| text.strip_prefix(name(source)))
            .and_then(|text| text.strip_prefix(", #"))
            .and_then(|text| text.parse::<i32>().ok())
        else {
            return false;
        };
        let value = if value >= 0x8000 { value - 0x1_0000 } else { value };
        if !(2..=3).contains(&value.abs()) || !is_pair(source) || source == reg::HL {
            return false;
        }
        if !self.flags_dead(rest) || !self.pair_dead_after(source, rest) {
            return false;
        }
        self.out.pop();
        for _ in 0..value.abs() {
            self.push(if value > 0 { "inc hl" } else { "dec hl" });
        }
        true
    }

    /// `ld hl, #k; add hl, rr; ex de, hl` (the sum copied back into `rr`, `hl`
    /// dead after, the carry too) with k in 1..=3 or -3..=-1: `inc rr` / `dec
    /// rr` k times, 1-3 B against 5. A pointer kept in `de` or `bc` was
    /// stepped through `hl`.
    fn step_pair_through_hl(
        &mut self,
        source: RegId,
        rest: &[Inst],
    ) -> bool {
        if PRICING.with(|one| one.get()) {
            return false;
        }
        let Some(Line::Inst(last)) = self.out.last() else { return false };
        let Some(value) = last.strip_prefix("ld hl, #").and_then(|text| text.parse::<i32>().ok()) else {
            return false;
        };
        let value = if value >= 0x8000 { value - 0x1_0000 } else { value };
        if !(1..=3).contains(&value.abs()) || source == reg::HL || !is_pair(source) {
            return false;
        }
        let Some(copy) = rest.first().filter(|inst| inst.kind == Kind::Copy) else { return false };
        let [Operand::Def(to), Operand::Use(from)] = copy.operands[..] else { return false };
        if self.physical(to) != source || self.physical(from) != reg::HL || !self.flags_dead(rest) {
            return false;
        }
        self.at.1 += 1;
        let hl_dead = self.pair_dead_after(reg::HL, &rest[1..]);
        self.at.1 -= 1;
        if !hl_dead {
            return false;
        }
        self.out.pop();
        for _ in 0..value.abs() {
            self.push(format!("{} {}", if value > 0 { "inc" } else { "dec" }, name(source)));
        }
        self.skip = 1;
        true
    }

    /// [`held_since_stored`] over what is printed so far.
    fn held_since_stored(
        &self,
        stores: &[(String, String)],
    ) -> bool {
        held_since_stored(&self.out, stores)
    }

    /// [`Self::dead_after`] for both bytes of `pair`, and the pair itself.
    fn pair_dead_after(
        &self,
        pair: RegId,
        rest: &[Inst],
    ) -> bool {
        let Ok((lo, hi)) = lanes(pair) else { return false };
        for inst in rest {
            let named = |r: &Reg| {
                let r = self.physical(*r);
                r == pair || r == lo || r == hi
            };
            let reads = inst.operands.iter().any(|o| matches!(o, Operand::Use(r) if named(r)));
            if reads {
                return false;
            }
            // a def of the whole pair ends it; one lane alone does not
            if inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if self.physical(*r) == pair)) {
                return true;
            }
            if inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if named(r))) {
                return false;
            }
        }
        if matches!(rest.last().map(|inst| inst.kind), Some(Kind::Op("RET"))) {
            return true;
        }
        // the block ends with the pair unmentioned: what the blocks it goes on
        // to read of it (the loop head of digits defines `de` first thing)
        let after = self
            .liveness
            .live_before::<Z80Registers>(self.function, self.assignment, &REGISTER_INFO, self.at.0, self.at.1 + 1);
        ![pair, lo, hi].into_iter().any(|one| after.is_live(&REGISTER_INFO, one))
    }

    /// Whether the byte register `register` holds nothing a later instruction
    /// of the block reads: the first mention after this point defines it
    /// without reading it. A block that ends with `ret` before any mention
    /// is dead too (what the function returns is a `ret` operand); any
    /// other end is not known.
    fn dead_after(
        &self,
        register: RegId,
        rest: &[Inst],
    ) -> bool {
        for inst in rest {
            let mentions = |want_def: bool| {
                inst.operands
                    .iter()
                    .any(
                        |operand| match operand {
                            Operand::Def(r) if want_def => self.physical(*r) == register,
                            Operand::Use(r) if !want_def => self.physical(*r) == register,
                            _ => false,
                        },
                    )
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

    /// `CP16 a, b`: the flags of `a - b` through `a`, which the pseudo
    /// clobbers. What follows says which flag is read: `sub` and `sbc` give
    /// the carry of the word; zero needs both bytes, so the second compare
    /// is skipped when the first differs.
    fn compare16(
        &mut self,
        inst: &Inst,
        rest: &[Inst],
    ) -> Result<(), String> {
        let (al, ah) = lanes(self.register(inst, 1)?)?;
        // the other word: a pair, or two immediate bytes
        let (bl, bh) = match inst.operands.get(2) {
            Some(Operand::Imm(value)) => (format!("#{}", value & 0xFF), format!("#{}", (value >> 8) & 0xFF)),
            Some(Operand::Use(_)) => {
                let (lo, hi) = lanes(self.register(inst, 2)?)?;
                (name(lo).to_owned(), name(hi).to_owned())
            }
            other => return Err(format!("CP16 against {other:?}")),
        };
        // every reader of the compare's flags, up to the next instruction that
        // sets them: a carry chain reads the carry (`ult`) and the zero flag
        // (`eq`) of the one compare
        let (mut carry, mut zero, mut other) = (false, false, None);
        for one in &self.flag_readers(rest) {
            {
                // a conditional step names its condition after the word
                let condition = if matches!(one.kind, Kind::Op("INC16c" | "DEC16c")) { 2 } else { 1 };
                match (one.kind, one.operands.get(condition)) {
                    (Kind::Op("SETCC8" | "INC16c" | "DEC16c"), Some(Operand::Imm(2 | 3)))
                    | (Kind::Op("JPC" | "JPNC" | "RETC" | "RETNC" | "CP16C" | "CP16Ci"), _) => carry = true,
                    (Kind::Op("SETCC8" | "INC16c" | "DEC16c"), Some(Operand::Imm(0 | 1)))
                    | (Kind::Op("JPZ" | "JPNZ" | "RETZ" | "RETNZ"), _) => zero = true,
                    (Kind::Op(name), _) => other = Some(name),
                    (Kind::Copy, _) => {}
                }
            }
        }
        if other.is_some() {
            return Err(format!("CP16 read by {other:?}"));
        }
        // no reader: a wide compare whose later word compare decides on its own
        if !(carry || zero) {
            return Ok(());
        }
        // the next word of a long: the borrow of the one before comes in
        if matches!(inst.kind, Kind::Op("CP16C" | "CP16Ci")) {
            if zero {
                return Err("CP16C read for zero".into());
            }
            self.push(format!("ld a, {}", name(al)));
            self.push(format!("sbc a, {bl}"));
            self.push(format!("ld a, {}", name(ah)));
            self.push(format!("sbc a, {bh}"));
            // signed: the sign, corrected where the subtract overflowed, into
            // the carry
            if matches!(inst.operands.get(4), Some(Operand::Imm(1))) {
                let fixed = format!("{}$", self.out.iter().filter(|line| !matches!(line, Line::Loc(_))).count() + 1);
                self.push(format!("jp po, {fixed}"));
                self.push("xor a, #128");
                self.out.push(Line::Label(fixed));
                self.push("rlca");
            }
            return Ok(());
        }
        let skip = format!("{}$", self.out.iter().filter(|line| !matches!(line, Line::Loc(_))).count());
        match (carry, zero) {
            // both flags of the word: the high bytes decide unless they are
            // equal, then the low bytes do (a borrow chain gives only the
            // carry)
            (true, true) => {
                self.push(format!("ld a, {}", name(ah)));
                self.push(format!("cp a, {bh}"));
                self.push(format!("jr nz, {skip}"));
                self.push(format!("ld a, {}", name(al)));
                self.push(format!("cp a, {bl}"));
                self.out.push(Line::Label(skip));
            }
            (true, false) => {
                self.push(format!("ld a, {}", name(al)));
                self.push(format!("sub a, {bl}"));
                self.push(format!("ld a, {}", name(ah)));
                self.push(format!("sbc a, {bh}"));
            }
            // a word against zero: its two bytes or-ed, which is zero only when
            // both are
            (false, true) if bl == "#0" && bh == "#0" => {
                self.push(format!("ld a, {}", name(ah)));
                self.push(format!("or a, {}", name(al)));
            }
            // two pairs, one of them `hl`: `sbc hl, rr` sets zero for the whole
            // word (`add hl, rr` leaves it alone and puts `hl` back)
            (false, true)
                if al == reg::L
                    && matches!(inst.operands.get(2), Some(Operand::Use(_)))
                    && self.register(inst, 2)? != reg::HL =>
            {
                let other = name(self.register(inst, 2)?);
                self.push("or a, a");
                self.push(format!("sbc hl, {other}"));
                if !self.pair_dead_after(reg::HL, rest) {
                    self.push(format!("add hl, {other}"));
                }
            }
            (false, true) => {
                self.push(format!("ld a, {}", name(al)));
                self.push(format!("sub a, {bl}"));
                self.push(format!("jr nz, {skip}"));
                self.push(format!("ld a, {}", name(ah)));
                self.push(format!("sub a, {bh}"));
                self.out.push(Line::Label(skip));
            }
            (false, false) => unreachable!("a reader was found"),
        }
        Ok(())
    }

    /// The byte operand `at` of an ALU or compare as printed: the frame cell a
    /// reload was folded into, or the register.
    fn source(
        &mut self,
        inst: &Inst,
        at: usize,
    ) -> Result<String, String> {
        let register = self.register(inst, at)?;
        match self.folded.take() {
            Some((one, slot)) if one == register => Ok(slot),
            _ => Ok(name(register).to_owned()),
        }
    }

    /// A byte reloaded, optionally stepped by one, and spilled back to the same
    /// cell, with nothing else reading it, is `inc d(ix)` / `dec d(ix)` or no
    /// instruction at all (`foldMemoryOperand` of the frame cell). Prints it
    /// and reports whether it did.
    fn updated_in_place(
        &mut self,
        reload: &Inst,
        rest: &[Inst],
    ) -> Result<bool, String> {
        let (Some(Operand::Def(loaded @ Reg::Virt(_))), Some(Operand::Frame(cell))) =
            (reload.operands.first(), reload.operands.get(1))
        else {
            return Ok(false);
        };
        let register = self.physical(*loaded);
        // `inst` spills `register` back to the cell
        let spills = |inst: &Inst| {
            inst.kind == Kind::Op("SPILL8")
                && matches!(
                    &inst.operands[..],
                    [Operand::Frame(one), Operand::Use(r)] if one == cell && self.physical(*r) == register
                )
        };
        let steps = |inst: &Inst| {
            matches!(inst.kind, Kind::Op("INC8" | "DEC8"))
                && matches!(
                    &inst.operands[..],
                    [Operand::Def(d), Operand::Use(u)] if self.physical(*d) == register && self.physical(*u) == register
                )
        };
        let (taken, step) = match rest {
            [back, ..] if spills(back) => (1, None),
            [step, back, ..] if steps(step) && spills(back) => (2, Some(step)),
            _ => return Ok(false),
        };
        // nothing but these reads the registers' virtual values
        let mut inside: llrm_support::hash::HashMap<u32, usize> = llrm_support::hash::HashMap::default();
        for inst in &rest[..taken] {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    *inside.entry(*v).or_default() += 1;
                }
            }
        }
        // the reload feeds one read (a copy to the stepped register that the
        // allocator dropped, or the step), and the stepped value only the spill
        let loaded_reads = match loaded {
            Reg::Virt(v) => self.reads.get(v).copied().unwrap_or(0),
            Reg::Phys(_) => 2,
        };
        let step_made = step.and_then(|one| match one.operands.first() {
            Some(Operand::Def(Reg::Virt(d))) => Some(*d),
            _ => None,
        });
        let others = inside.keys().chain(step_made.as_ref());
        if loaded_reads > 1
            || others
                .into_iter()
                .any(|v| self.reads.get(v).copied().unwrap_or(0) != inside.get(v).copied().unwrap_or(0))
        {
            return Ok(false);
        }
        if let Some(step) = step {
            let Kind::Op(op) = step.kind else { return Ok(false) };
            let slot = self.slot(*cell, 0)?;
            self.push(format!("{} {slot}", op[..3].to_lowercase()));
        }
        self.skip = taken;
        Ok(true)
    }

    /// Whether the instruction after a `RELOAD8` is an ALU op or compare that
    /// reads the reloaded byte, which nothing else reads: it may take the frame
    /// cell as its operand (`sub a, d(ix)`) and the reload goes.
    fn reload_is_operand(
        &self,
        reload: &Inst,
        rest: &[Inst],
    ) -> bool {
        let (Some(Operand::Def(Reg::Virt(loaded))), Some(next)) = (reload.operands.first(), rest.first()) else {
            return false;
        };
        matches!(
            next.kind,
            Kind::Op("ADD8r" | "SUB8r" | "AND8r" | "OR8r" | "XOR8r" | "CP8r")
        )
            && matches!(
                next.operands.get(2),
                Some(Operand::Use(Reg::Virt(v))) if v == loaded
            )
            && self.reads.get(loaded) == Some(&1)
            && !matches!(
                next.operands.get(1),
                Some(Operand::Use(Reg::Virt(v))) if v == loaded
            )
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

    /// The operands of `count` bytes of frame object `index` from `extra`.
    /// Within the byte displacement of IX they are `d(ix)`; beyond it they
    /// are `d(iy)` with IY set to the first byte (`ld iy, #n; add iy, sp`),
    /// when nothing holds IY at this instruction.
    fn frame_slots(
        &mut self,
        index: i32,
        extra: i64,
        count: i64,
    ) -> Result<Vec<String>, String> {
        let offset = *self.frame.offsets.get(&index).ok_or_else(|| format!("frame object {index} has no place"))?;
        let at = offset + extra;
        if (-128..=127).contains(&at) && (-128..=127).contains(&(at + count - 1)) {
            return Ok((0..count).map(|byte| format!("{}(ix)", at + byte)).collect());
        }
        if self.iy_is_live() {
            return Err(format!("frame object {index} is {at} from ix and iy is live"));
        }
        self.iy_to_stack(at + self.frame.below_ix() + self.call_bytes + self.spill_bytes);
        Ok((0..count).map(|byte| format!("{byte}(iy)")).collect())
    }

    /// `ld iy, #offset; add iy, sp`: IY at `offset` bytes above the stack
    /// pointer. The sum sets the carry, so flags that a later instruction
    /// reads are kept across it; `a` is saved with them, and the `push` moves
    /// the stack pointer by two.
    fn iy_to_stack(
        &mut self,
        offset: i64,
    ) {
        let keep = !self.flags_dead(&self.function.blocks[self.at.0].insts[self.at.1 + 1..]);
        if keep {
            self.push("push af");
        }
        self.push(format!("ld iy, #{}", offset + if keep { 2 } else { 0 }));
        self.push("add iy, sp");
        if keep {
            self.push("pop af");
        }
    }

    /// The instructions that read the flags the instruction before `rest` set:
    /// in `rest` and, when nothing in it sets them again, in the blocks it
    /// goes on to.
    fn flag_readers(
        &self,
        rest: &[Inst],
    ) -> Vec<Inst> {
        let mut readers: Vec<Inst> = Vec::new();
        let mut scan = |insts: &[Inst]| {
            for inst in insts {
                if inst.reads_flags {
                    readers.push(inst.clone());
                }
                if inst.sets_flags {
                    return true;
                }
            }
            false
        };
        if scan(rest) {
            return readers;
        }
        let successors = llrm_spike_mf::regalloc::successors(self.function);
        let mut seen = vec![false; self.function.blocks.len()];
        let mut todo = successors[self.at.0].clone();
        while let Some(block) = todo.pop() {
            if std::mem::replace(&mut seen[block], true) {
                continue;
            }
            if !scan(&self.function.blocks[block].insts) {
                todo.extend(&successors[block]);
            }
        }
        readers
    }

    /// Whether nothing reads the flags before something sets them again, in
    /// `rest` and in the blocks it goes on to: a conditional return in the
    /// block after a compare reads the compare's flags, so an `xor a, a` or a
    /// `dec` in between would change what it sees.
    fn flags_dead(
        &self,
        rest: &[Inst],
    ) -> bool {
        if let Some(dead) = flags_first(rest) {
            return dead;
        }
        let successors = llrm_spike_mf::regalloc::successors(self.function);
        let mut seen = vec![false; self.function.blocks.len()];
        let mut todo = successors[self.at.0].clone();
        while let Some(block) = todo.pop() {
            if std::mem::replace(&mut seen[block], true) {
                continue;
            }
            match flags_first(&self.function.blocks[block].insts) {
                Some(false) => return false,
                Some(true) => {}
                None => todo.extend(&successors[block]),
            }
        }
        true
    }

    /// Whether something holds IY at this instruction.
    fn iy_is_live(&self) -> bool {
        self.liveness
            .live_before::<Z80Registers>(self.function, self.assignment, &REGISTER_INFO, self.at.0, self.at.1)
            .is_live(&REGISTER_INFO, reg::IY)
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

/// What `insts` do with the flags first: `Some(true)` if they set them before
/// anything reads them, `Some(false)` if they read them first, none if they do
/// neither.
fn flags_first(insts: &[Inst]) -> Option<bool> {
    for inst in insts {
        if inst.reads_flags {
            return Some(false);
        }
        if inst.sets_flags {
            return Some(true);
        }
    }
    None
}

fn shift_count(inst: &Inst) -> Result<i64, String> {
    match inst.operands.last() {
        Some(Operand::Imm(count)) => Ok(*count),
        other => Err(format!("a shift by {other:?}")),
    }
}

/// Whether `function` with spill `slots` keeps an IX frame.
pub(crate) fn needs_frame(function: &MFunction, slots: &[(i32, usize)]) -> bool {
    layout(function, slots).active
}

/// The frame of `function`: its static allocas, then the spill `slots`.
fn layout(
    function: &MFunction,
    slots: &[(i32, usize)],
) -> Frame {
    let mut frame = Frame::default();
    // Slots alone: bytes first, then the words, so the last word lies at SP and
    // each word is one `push`.
    let aligned = function.locals.is_empty() && slots.iter().all(|&(_, size)| size == 1 || size == 2);
    if aligned {
        let mut ordered = slots.to_vec();
        ordered.sort_by_key(|&(_, size)| size);
        for (index, size) in ordered {
            if size == 2 && frame.size % 2 == 1 {
                frame.size += 1;
            }
            frame.size += size as i64;
            frame.offsets.insert(index, -frame.size);
        }
    } else {
        // The densest objects (most uses per byte) lie nearest IX, as the pin's
        // `orderFrameObjects` lays a frame addressed from the frame pointer:
        // `ld -n(ix), r` reaches 128 bytes below it, and past that every access
        // is `ld iy, #n; add iy, sp`. Spill slots and scalars came after a local
        // array and lay beyond its 8 KB (tests/run/c/fill_nest); by size alone a
        // hot array lay beyond a cold smaller one (tests/frame_density.rs).
        let mut objects: Vec<(i32, i64)> = function.locals.iter().map(|&(index, size, _)| (index, size)).collect();
        objects.extend(slots.iter().map(|&(index, size)| (index, size as i64)));
        // A frame within IX's reach keeps its locals first and its slots last,
        // the last slot lying at SP (`pop` then `push` for a reload).
        if objects.iter().map(|&(_, size)| size).sum::<i64>() > 127 {
            let mut uses: llrm_support::hash::HashMap<i32, u64> = llrm_support::hash::HashMap::default();
            for inst in function.blocks.iter().flat_map(|block| &block.insts) {
                for operand in &inst.operands {
                    if let Operand::Frame(index) = operand {
                        *uses.entry(*index).or_default() += 1;
                    }
                }
            }
            let sorting: Vec<_> = objects
                .iter()
                .map(|&(index, size)| Sorting { size: size as u64, align: 1, uses: uses.get(&index).copied().unwrap_or(0) })
                .collect();
            objects = order_frame_objects(&sorting, true).into_iter().map(|at| objects[at]).collect();
        }
        for (index, size) in objects {
            frame.size += size;
            frame.offsets.insert(index, -frame.size);
        }
    }
    for (&index, &offset) in &function.fixed {
        frame.offsets.insert(index, FIRST_ARGUMENT + offset);
    }
    let stack_arguments = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .any(
            |inst| inst.kind == Kind::Op("ADJCALLSTACKDOWN")
                && matches!(
                    inst.operands.first(),
                    Some(Operand::Imm(bytes)) if *bytes != 0
                ),
        );
    frame.active = frame.size > 0 || !function.fixed.is_empty() || stack_arguments;
    frame
}

/// `push ix; ld ix, 0; add ix, sp`, then room for the frame: a `push` per two
/// bytes (`pushed` names the pair a word of the frame, from the top, is made
/// by, `af` for the rest), or `iy` as the scratch for a large frame. Nothing
/// here touches the registers that arguments arrive in.
fn prologue(
    frame: &Frame,
    pushed: &[Option<&'static str>],
) -> Vec<String> {
    let mut lines = vec!["push ix".to_owned(), "ld ix, #0".to_owned(), "add ix, sp".to_owned()];
    let pushes = (frame.size + 1) / 2;
    if pushes <= 3 {
        lines.extend(
            (0..pushes as usize).map(|word| format!("push {}", pushed.get(word).copied().flatten().unwrap_or("af"))),
        );
    } else {
        lines.extend([
            "ld iy, #".to_owned() + &format!("{}", -frame.size),
            "add iy, sp".to_owned(),
            "ld sp, iy".to_owned(),
        ]);
    }
    lines
}

/// The entry block's leading spills a `push` of the prologue makes, taken out
/// of the block: a pair into the word it fills, `a` or the high half of a pair
/// into the high byte of a word (`push af` stores `a` there, and flags that
/// nothing holds in the low byte). By word from the top of the frame.
fn entry_pushes(
    function: &mut MFunction,
    assignment: &Assignment,
    frame: &Frame,
) -> Vec<Option<&'static str>> {
    let mut pushed: Vec<Option<&'static str>> = vec![None; (frame.size as usize + 1) / 2];
    if !frame.active || frame.size > 6 || !function.locals.is_empty() {
        return pushed;
    }
    let mut at = 0;
    while at < function.blocks[0].insts.len() {
        let inst = &function.blocks[0].insts[at];
        let (op, [Operand::Frame(slot), Operand::Use(source)]) = (inst.kind, &inst.operands[..]) else { break };
        let register = match source {
            Reg::Virt(v) => assignment.0[*v as usize],
            Reg::Phys(id) => *id,
        };
        let Some(&offset) = frame.offsets.get(slot) else { break };
        let word = ((-offset - 1) / 2) as usize;
        let pair = [reg::BC, reg::DE, reg::HL].into_iter().find(|pair| REGISTER_INFO.root(register) == *pair);
        let by = match (op, pair) {
            (Kind::Op("SPILL16"), Some(pair)) if offset == -2 * word as i64 - 2 && register == pair => Some(name(pair)),
            (Kind::Op("SPILL8"), _) if offset == -2 * word as i64 - 1 && register == reg::A => Some("af"),
            (Kind::Op("SPILL8"), Some(pair))
                if offset == -2 * word as i64 - 1 && lanes(pair).is_ok_and(|(_, hi)| hi == register) =>
            {
                Some(name(pair))
            }
            (Kind::Op("SPILL8" | "SPILL16"), _) => None,
            _ => break,
        };
        match by {
            Some(by) if pushed[word].is_none() => {
                pushed[word] = Some(by);
                function.blocks[0].insts.remove(at);
            }
            _ => at += 1,
        }
    }
    pushed
}

/// The instructions of `function` with the registers `assignment` gave them.
/// `slots` are the spill slots and their sizes in bytes.
/// How many times each virtual register is read, copies included.
pub fn reads(function: &MFunction) -> llrm_support::hash::HashMap<u32, usize> {
    let mut reads = llrm_support::hash::HashMap::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Use(Reg::Virt(v)) = operand {
                *reads.entry(*v).or_default() += 1;
            }
        }
    }
    reads
}

/// The bytes of stack arguments a call has reserved when each block is
/// entered. Branch folding hoists the reservation of a call that both arms of
/// a branch make into the block ahead of it, so a count kept in the order
/// blocks are printed was the one the arm before had left: the second arm
/// stored its arguments 6 B off (a `u64` shifted one way or the other, -O2).
pub(crate) fn call_bytes_at_entry(function: &MFunction) -> Vec<(i64, i64)> {
    let index: llrm_support::hash::HashMap<u32, usize> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.id, at)).collect();
    let mut entry: Vec<Option<(i64, i64)>> = vec![None; function.blocks.len()];
    let mut work = Vec::new();
    if !entry.is_empty() {
        entry[0] = Some((0, 0));
        work.push(0);
    }
    while let Some(at) = work.pop() {
        let block = &function.blocks[at];
        let (mut bytes, mut spilled) = entry[at].unwrap_or((0, 0));
        for inst in &block.insts {
            match (inst.kind, inst.operands.first()) {
                (Kind::Op("ADJCALLSTACKDOWN"), Some(Operand::Imm(size))) if *size != 0 => bytes = 2 * ((size + 1) / 2),
                (Kind::Op("ADJCALLSTACKUP"), Some(Operand::Imm(size))) if *size != 0 => bytes = 0,
                (Kind::Op("PUSHARG"), _) => bytes += 2,
                (Kind::Op("PUSHP"), _) => spilled += 2,
                (Kind::Op("POPP"), _) => spilled -= 2,
                _ => {}
            }
        }
        let falls = !block.insts.last().is_some_and(|inst| inst.barrier);
        let targets = block
            .insts
            .iter()
            .flat_map(|inst| &inst.operands)
            .filter_map(
                |operand| match operand {
                    Operand::Block(id) => index.get(id).copied(),
                    _ => None,
                },
            );
        for to in targets.chain(falls.then_some(at + 1).filter(|next| *next < function.blocks.len())) {
            if entry[to].is_none() {
                entry[to] = Some((bytes, spilled));
                work.push(to);
            }
        }
    }
    entry.into_iter().map(|bytes| bytes.unwrap_or((0, 0))).collect()
}

/// `reads` is [`reads`] of the function as the allocator left it: the copies
/// it dropped as identities were reads too.
pub fn finish(
    function: &MFunction,
    assignment: &Assignment,
    number: usize,
    slots: &[(i32, usize)],
    reads: llrm_support::hash::HashMap<u32, usize>,
) -> Result<Vec<Line>, String> {
    let mut function = tail_calls(function);
    push_arguments(&mut function, assignment);
    let frame = layout(&function, slots);
    let pushed = entry_pushes(&mut function, assignment, &frame);
    let function = &function;
    let returns_bare = function
        .blocks
        .iter()
        .filter(|block| matches!(&block.insts[..], [one] if one.kind == Kind::Op("RET")))
        .map(|block| block.id)
        .collect();
    let liveness = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
    let mut printer = Printer {
        assignment,
        number,
        out: Vec::new(),
        frame,
        returns_bare,
        reads,
        folded: None,
        skip: 0,
        call_bytes: 0,
        spill_bytes: 0,
        liveness,
        function,
        at: (0, 0),
    };
    if printer.frame.active {
        for line in prologue(&printer.frame, &pushed) {
            printer.push(line);
        }
    }
    let call_bytes = call_bytes_at_entry(function);
    for (at, block) in function.blocks.iter().enumerate() {
        (printer.call_bytes, printer.spill_bytes) = call_bytes[at];
        printer.out.push(Line::Label(label(number, block.id)));
        let mut shown = None;
        let next = function.blocks.get(at + 1).map(|block| block.id);
        for (index, inst) in block.insts.iter().enumerate() {
            if printer.skip > 0 {
                printer.skip -= 1;
                continue;
            }
            printer.at = (at, index);
            // Place 0 is line 0, the pin's row for an instruction with no
            // location.
            if shown != Some(inst.origin) {
                shown = Some(inst.origin);
                let line = match inst.origin {
                    0 => Some(0),
                    origin => function.places.get(origin as usize - 1).map(|place| place.line),
                };
                if let Some(line) = line {
                    printer.out.push(Line::Loc(line));
                }
            }
            printer.instruction(inst, next, &block.insts[index + 1..])?;
        }
    }
    Ok(drop_unreachable(forward_high_bytes(cancel_steps(printer.out))))
}

/// Whether `text` ends the flow: `ret`, `jp L` or `jr L` with no condition.
fn ends_flow(text: &str) -> bool {
    text == "ret"
        || text.strip_prefix("jp ").or_else(|| text.strip_prefix("jr ")).is_some_and(|rest| !rest.contains(','))
}

/// Whether the last lines printed are the loads `ld register, slot` of `loads`,
/// in order: the slots already hold what the registers do, and a store of them
/// back is 3 B and 19 T a byte for nothing. The allocator splits a range round
/// a reload and spills the value it had not changed (idiv_loop_os).
fn just_reloaded(
    out: &[Line],
    loads: &[(String, String)],
) -> bool {
    // newest first; a label on the way is a join, whose other path has not
    // loaded
    let mut lines: Vec<&str> = Vec::new();
    for line in out.iter().rev() {
        match line {
            Line::Loc(_) => {}
            Line::Label(_) => return false,
            Line::Inst(text) => lines.push(text),
        }
        if lines.len() == loads.len() {
            break;
        }
    }
    lines.len() == loads.len()
        && lines.iter().rev().zip(loads).all(|(text, (slot, register))| **text == format!("ld {register}, {slot}"))
}

/// Whether the registers still hold what was just stored to their slots:
/// the stores `ld slot, reg` are among the last lines printed, and nothing
/// since mentions a register or a slot, or is a label, a jump, a call or an
/// instruction with registers of its own (`ldir`, `djnz`). The allocator
/// splits a range round an `inc hl; inc hl` and reloads what never left.
fn held_since_stored(
    out: &[Line],
    stores: &[(String, String)],
) -> bool {
    let Some((last_slot, last_register)) = stores.last() else { return false };
    if stores.iter().any(|(_, register)| register == "a") {
        return false;
    }
    let want = |(slot, register): &(String, String)| format!("ld {slot}, {register}");
    let mut lines: Vec<&str> = Vec::new();
    for line in out.iter().rev() {
        match line {
            Line::Loc(_) => {}
            Line::Label(_) => return false,
            Line::Inst(text) => lines.push(text),
        }
        if lines.len() > 24 {
            return false;
        }
        if lines.last().is_some_and(|text| *text == format!("ld {last_slot}, {last_register}")) {
            break;
        }
    }
    // `lines` is newest first, ending at the last store; the stores before
    // it are the lines directly behind
    let Some(at) = lines.len().checked_sub(1) else { return false };
    let stored: Vec<&str> = out
        .iter()
        .filter_map(|line| if let Line::Inst(text) = line { Some(text.as_str()) } else { None })
        .rev()
        .skip(at)
        .take(stores.len())
        .collect();
    let in_order = stored.iter().rev().zip(stores).all(|(text, store)| **text == want(store));
    if !in_order {
        return false;
    }
    let touched = |word: &str| {
        stores.iter().any(|(_, register)| {
            word == register || (word.len() == 2 && word.contains(register.as_str()) && word != "ix" && word != "iy")
        })
    };
    lines[..at].iter().all(|text| {
        let mut words = text.split([' ', ',', '(', ')', '+']).filter(|word| !word.is_empty());
        let first = words.next().unwrap_or("");
        !HIDDEN.contains(&first) && !words.any(touched) && !stores.iter().any(|(slot, _)| text.contains(slot.as_str()))
    })
}

/// Instructions that leave, or use registers of their own.
const HIDDEN: [&str; 14] =
    ["call", "ret", "jp", "jr", "djnz", "ldir", "lddr", "ldi", "ldd", "cpir", "cpdr", "exx", "rst", "halt"];

/// Whether `text` names `pair` or a byte of it.
fn mentions(
    text: &str,
    pair: &str,
) -> bool {
    text.split([' ', ',', '(', ')', '+'])
        .filter(|word| !word.is_empty())
        .skip(1)
        .any(|word| word == pair || (word.len() == 1 && pair.contains(word)))
}

/// `ld R1, S1; ld R2, S2; ld R1, R2; ld R2, #0`: a pair copied in order to read
/// the high byte of a copy, widened. The first load is overwritten and the
/// second is read once: `ld R1, S2; ld R2, #0`, 2 B less.
fn forward_high_bytes(lines: Vec<Line>) -> Vec<Line> {
    fn load(line: &Line) -> Option<(&str, &str)> {
        let Line::Inst(text) = line else { return None };
        let (to, from) = text.strip_prefix("ld ")?.split_once(", ")?;
        let byte = |r: &str| matches!(r, "b" | "c" | "d" | "e" | "h" | "l");
        (byte(to) && (byte(from) || from == "#0")).then_some((to, from))
    }
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines {
        let fused = (|| {
            let (r2, zero) = load(&line)?;
            if zero != "#0" || out.len() < 3 {
                return None;
            }
            let at = out.len() - 3;
            let (r1, s1) = load(&out[at])?;
            let (r2b, s2) = load(&out[at + 1])?;
            let (r1b, r2c) = load(&out[at + 2])?;
            (r2b == r2 && r2c == r2 && r1b == r1 && r1 != r2 && s1 != "#0" && s2 != "#0" && s2 != r1 && s2 != r2)
                .then(|| (at, r1.to_owned(), s2.to_owned()))
        })();
        match fused {
            Some((at, r1, s2)) => {
                out.truncate(at);
                out.push(Line::Inst(format!("ld {r1}, {s2}")));
                out.push(line);
            }
            None => out.push(line),
        }
    }
    out
}

/// `dec rr` then `inc rr` of the same pair, or the reverse, is nothing: neither
/// touches the flags, and nothing between may name the pair. A word stored
/// through a pointer leaves it where it was (`inc hl; ld (hl), d; dec hl`), and
/// a pointer stepped by one after that undoes the restore.
fn cancel_steps(lines: Vec<Line>) -> Vec<Line> {
    fn step(line: &Line) -> Option<(i32, &str)> {
        let Line::Inst(text) = line else { return None };
        text.strip_prefix("inc ")
            .map(|pair| (1, pair))
            .or_else(|| text.strip_prefix("dec ").map(|pair| (-1, pair)))
            .filter(|(_, pair)| matches!(*pair, "bc" | "de" | "hl"))
    }
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines {
        // the step it undoes: `dec hl` ... `inc hl` with nothing between that
        // names the pair, a label, or a jump
        let undone = step(&line).and_then(|(sign, pair)| {
            for (at, earlier) in out.iter().enumerate().rev().take(12) {
                match earlier {
                    Line::Loc(_) => continue,
                    Line::Label(_) => return None,
                    Line::Inst(text) => {
                        if step(earlier) == Some((-sign, pair)) {
                            return Some(at);
                        }
                        if mentions(text, pair) || HIDDEN.contains(&text.split(' ').next().unwrap_or("")) {
                            return None;
                        }
                    }
                }
            }
            None
        });
        match undone {
            Some(at) => {
                out.remove(at);
            }
            None => out.push(line),
        }
    }
    out
}

/// `ret z` stands in for a branch to a block that is only a `ret`, which
/// leaves the block with no reader: the code after an unconditional `ret` or
/// jump, up to the next label something names, never runs.
fn drop_unreachable(lines: Vec<Line>) -> Vec<Line> {
    let named: llrm_support::hash::HashSet<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(text) => Some(text.split([' ', ',']).filter(|word| !word.is_empty())),
            _ => None,
        })
        .flatten()
        .collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut dead = false;
    for line in &lines {
        match line {
            Line::Label(name) if named.contains(name.as_str()) => dead = false,
            Line::Label(_) | Line::Loc(_) if dead => continue,
            Line::Inst(_) if dead => continue,
            Line::Inst(text) => dead = ends_flow(text),
            _ => {}
        }
        out.push(line.clone());
    }
    out
}

/// The `timing/z80.csv` key of an instruction as printed here: operands by
/// kind (`b` any byte register, `n` an 8-bit immediate, `nn` a 16-bit one or an
/// address, `(ix+d)` an indexed cell).
fn form(text: &str) -> String {
    let (mnemonic, rest) = text.split_once(' ').unwrap_or((text, ""));
    let mut operands: Vec<String> =
        rest.split(',').map(|one| one.trim().to_owned()).filter(|one| !one.is_empty()).collect();
    // sub, and, or, xor and cp name the accumulator only implicitly in the
    // table
    if matches!(mnemonic, "sub" | "and" | "or" | "xor" | "cp") && operands.first().is_some_and(|one| one == "a") {
        operands.remove(0);
    }
    let wide = operands.first().is_some_and(|one| matches!(one.as_str(), "bc" | "de" | "hl" | "ix" | "iy" | "sp"));
    let kind = |one: &str, first: bool| -> String {
        match one {
            // every condition times alike: the table has `nz`
            "nz" | "z" | "nc" | "c" | "po" | "pe" | "p" | "m"
                if first
                    && matches!(mnemonic, "jp" | "jr" | "call" | "ret")
                    && (operands.len() == 2 || mnemonic == "ret") =>
            {
                "nz".to_owned()
            }
            "de" if mnemonic == "ex" => "de".to_owned(),
            "a" if first && matches!(mnemonic, "add" | "adc" | "sbc") => "a".to_owned(),
            "a" | "b" | "c" | "d" | "e" | "h" | "l" => "b".to_owned(),
            "bc" | "de" | "af" => "bc".to_owned(),
            "hl" | "ix" | "iy" | "sp" => one.to_owned(),
            "(hl)" => "(hl)".to_owned(),
            "(bc)" | "(de)" => one.to_owned(),
            _ if one.ends_with("(ix)") || one.ends_with("(iy)") => "(ix+d)".to_owned(),
            _ if one.starts_with('(') => "(nn)".to_owned(),
            _ if one.starts_with('#') => {
                if wide || matches!(mnemonic, "jp" | "call") {
                    "nn".to_owned()
                } else {
                    "n".to_owned()
                }
            }
            other if mnemonic == "jr" && other.starts_with('L') => "e".to_owned(),
            other if mnemonic == "djnz" => {
                let _ = other;
                "e".to_owned()
            }
            other => {
                if matches!(mnemonic, "jp" | "call") || other.starts_with('L') {
                    "nn".to_owned()
                } else {
                    other.to_owned()
                }
            }
        }
    };
    let mut kinds: Vec<String> = operands.iter().enumerate().map(|(at, one)| kind(one, at == 0)).collect();
    // the table spells a register copy `ld b;c`
    if mnemonic == "ld" && kinds == ["b", "b"] {
        kinds[1] = "c".to_owned();
    }
    // a conditional jump or call names its condition first
    let mut key = mnemonic.to_owned();
    if !kinds.is_empty() {
        key.push(' ');
        key.push_str(&kinds.join(";"));
    }
    // `jp nz, L` is `jp nz;nn`; a bare condition on ret is `ret nz`
    key
}

/// The table's cost of a form as `form` writes it. Every byte register is `b`
/// there, so a copy between two of them is `ld b;b`, which the table spells
/// `ld b;c`.
fn table_cost(key: &str) -> Option<crate::timing::Cost> {
    crate::timing::cost(key)
        .or_else(|| crate::timing::cost(&key.replace("hl", "bc")))
        .or_else(|| crate::timing::cost(&key.replace("ix", "bc")))
        .or_else(|| crate::timing::cost(&key.replace("iy", "ix")))
        .or_else(|| (key == "ld b;b").then(|| crate::timing::cost("ld b;c")).flatten())
}

/// Bytes and T-states of the lines, summed over every instruction (a loop body
/// counts once): what two variants of one function are compared by. A form the
/// table lacks costs two bytes and eight clocks.
pub fn price(lines: &[Line]) -> (u32, u32) {
    let (mut bytes, mut clocks) = (0, 0);
    for line in lines {
        let Line::Inst(text) = line else { continue };
        let key = form(text);
        let cost = crate::timing::cost(&key)
            .or_else(|| crate::timing::cost(&key.replace("hl", "bc")))
            .or_else(|| crate::timing::cost(&key.replace("ix", "bc")));
        match cost {
            Some(cost) => {
                bytes += cost.bytes;
                clocks += cost.taken;
            }
            None => {
                bytes += 2;
                clocks += 8;
            }
        }
    }
    (bytes, clocks)
}

thread_local! {
    /// The frequency of each block of the function being compiled, relative to
    /// its entry, by block id: the pin's `BlockFrequencyInfo` of the IR
    /// function (`Selection::frequencies`).
    static FREQUENCIES: std::cell::RefCell<llrm_support::hash::HashMap<u32, f64>> = std::cell::RefCell::default();
}

/// Sets the block frequencies the prices of the function being compiled weigh by.
pub fn set_frequencies(frequencies: &[(u32, f64)]) {
    FREQUENCIES.with(|one| *one.borrow_mut() = frequencies.iter().copied().collect());
}

/// The block a label names (`L<function>_<block>`).
fn block_of(label: &str) -> Option<u32> {
    label.strip_prefix('L')?.rsplit_once('_')?.1.parse().ok()
}

/// Each line with its weight: the frequency of the block it is in. A block the
/// passes made after selection (a `select`'s halves, a block split off or
/// merged by branch folding, an edge split) has no frequency of its own: it
/// takes what flows into it from the blocks that jump or fall to it. The pin
/// keeps the machine blocks' frequencies in step with each split
/// (`MachineBlockFrequencyInfo::onEdgeSplit`,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockFrequencyInfo.cpp#L284>;
/// `BranchFolder::SplitMBBAt`, `BranchFolding.cpp#L513`; a merged tail,
/// `#L1283`). DEVIATE Z80-BLOCK-FLOW: the flow is worked out here from the
/// jumps of the printed lines; a branch with one known side gives the other
/// what is left, otherwise the sides split evenly (the pin's unweighted
/// branch). Without it the block took the weight of the label before it, the
/// arm of a diamond when the block is the join (pattern_fill -O1: the loop's
/// join, 163 T a trip, priced at the arm's 240 for 384). Owner z80, exit
/// 2026-11-20: the per-site updates above replace this reconstruction.
pub fn weighted(
    lines: &[Line],
    frequencies: &llrm_support::hash::HashMap<u32, f64>,
) -> Vec<(Line, f64)> {
    // blocks: the entry (before the first label) and one per `L<fn>_<id>`
    let mut starts = vec![0usize];
    let mut ids: Vec<Option<u32>> = vec![None];
    for (at, line) in lines.iter().enumerate() {
        if let Line::Label(name) = line {
            if let Some(id) = block_of(name) {
                starts.push(at);
                ids.push(Some(id));
            }
        }
    }
    let count = starts.len();
    let end = |block: usize| starts.get(block + 1).copied().unwrap_or(lines.len());
    let index_of: llrm_support::hash::HashMap<u32, usize> =
        ids.iter().enumerate().filter_map(|(block, id)| id.map(|id| (id, block))).collect();
    // successors of each block: the global labels it jumps to, and the next
    // block when it can fall through
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); count];
    for block in 0..count {
        let mut falls = true;
        for line in &lines[starts[block]..end(block)] {
            let Line::Inst(text) = line else { continue };
            let (mnemonic, rest) = text.split_once(' ').unwrap_or((text.as_str(), ""));
            let target = rest.rsplit(',').next().unwrap_or("").trim();
            falls = true;
            match mnemonic {
                "jp" | "jr" | "djnz" => {
                    let conditional = rest.contains(',') || mnemonic == "djnz";
                    if let Some(&to) = block_of(target).and_then(|id| index_of.get(&id)) {
                        if !succs[block].contains(&to) {
                            succs[block].push(to);
                        }
                    }
                    if !conditional {
                        falls = false;
                    }
                }
                "ret" if rest.is_empty() => falls = false,
                "halt" => falls = false,
                _ => {}
            }
        }
        if falls && block + 1 < count && !succs[block].contains(&(block + 1)) {
            succs[block].push(block + 1);
        }
    }
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (block, to) in succs.iter().enumerate() {
        for &s in to {
            preds[s].push(block);
        }
    }
    let known: Vec<Option<f64>> = ids
        .iter()
        .enumerate()
        .map(|(block, id)| if block == 0 { Some(1.0) } else { id.and_then(|id| frequencies.get(&id).copied()) })
        .collect();
    let mut freq: Vec<f64> = known.iter().map(|one| one.unwrap_or(0.0)).collect();
    for _ in 0..4 {
        for u in 0..count {
            if known[u].is_some() {
                continue;
            }
            let mut total = 0.0;
            for &p in &preds[u] {
                let sides = &succs[p];
                total += if sides.len() == 1 {
                    freq[p]
                } else {
                    // a side that is known and entered only from here takes its own
                    // frequency; the rest of the block's is shared by the others
                    let (mut taken, mut open) = (0.0, 0usize);
                    for &s in sides {
                        if known[s].is_some() && preds[s].len() == 1 {
                            taken += freq[s];
                        } else {
                            open += 1;
                        }
                    }
                    let share = (freq[p] - taken).max(0.0) / open.max(1) as f64;
                    share
                };
            }
            // no way in (unreachable, or only a back edge not yet worked out):
            // the block before it
            freq[u] = if preds[u].is_empty() { freq[u - 1] } else { total };
        }
    }
    let mut out = Vec::with_capacity(lines.len());
    let mut block = 0;
    for (at, line) in lines.iter().enumerate() {
        while block + 1 < count && starts[block + 1] <= at {
            block += 1;
        }
        out.push((line.clone(), freq[block]));
    }
    out
}

/// [`price_emitted`]'s measure: bytes, and the clocks of each instruction times
/// the frequency of its block (the entry's is 1), what two variants of one
/// function are compared by. DEVIATE Z80-BLOCK-WEIGHT: the pin's frequencies
/// are of the IR; the machine blocks of this stage take them by block id.
/// Owner z80, exit 2026-11-20: `MachineBlockFrequencyInfo` over the machine function.
pub fn price_in_loops(lines: &[Line]) -> (u32, u64) {
    let frequencies = FREQUENCIES.with(|one| one.borrow().clone());
    let (mut bytes, mut clocks) = (0, 0.0);
    for (line, weight) in weighted(lines, &frequencies) {
        let Line::Inst(text) = line else { continue };
        let cost = table_cost(&form(&text)).map_or((2, 8), |cost| (cost.bytes, cost.taken));
        bytes += cost.0;
        clocks += f64::from(cost.1) * weight;
    }
    (bytes, clocks.round() as u64)
}

/// [`price_in_loops`] of the lines as emitted: after branch relaxation, which
/// fuses `dec b; jr nz` into `djnz` (2 B, 13 T for 3 B, 16 T) only with the
/// counter in `b`. Priced before it, a counter in `c` cost the same as one in
/// `b`, and the pick took the allocation 3 T a trip slower (byval_struct -O1:
/// +83 T for -1 B).
pub fn price_emitted(
    lines: &[Line],
    speed: bool,
) -> (u32, u64) {
    price_in_loops(&relax(lines.to_vec(), speed))
}

/// The bytes of one printed line, jumps being the three-byte form.
fn bytes_of(line: &Line) -> u32 {
    let Line::Inst(text) = line else { return 0 };
    let key = form(text);
    table_cost(&key)
        // a form the table lacks: longer than any estimate
        .map_or(4, |cost| cost.bytes)
}

/// The label a `jp` names, and its condition if it has one.
fn jump_of(text: &str) -> Option<(Option<&str>, &str)> {
    let rest = text.strip_prefix("jp ")?;
    match rest.split_once(", ") {
        Some((cc, label)) => Some((Some(cc), label)),
        None => Some((None, rest)),
    }
}

/// Branch relaxation: a `jp` whose target is within reach of `jr` (two bytes)
/// becomes one, and `dec b; jr nz` becomes `djnz`. Reach is judged with every
/// jump at its long size, so it only shrinks; a margin covers a form the table
/// does not price.
///
/// DEVIATE: `jr` is slower taken (12 T against 10) and faster not taken (7
/// against 10). At the size levels every jump within reach is a `jr`, SDCC's
/// choice. Where speed comes first, an always-taken jump inside a loop stays a
/// `jp`: a byte longer, 2 T faster each trip. A conditional jump is a `jr`
/// (a back edge not taken once per loop costs 3 T there, which a short loop
/// does not win back), and so is a jump outside every loop. Exit: block
/// frequencies at this stage.
pub fn relax(
    lines: Vec<Line>,
    speed: bool,
) -> Vec<Line> {
    const REACH: i64 = 100;
    let mut at = 0;
    let mut starts = Vec::with_capacity(lines.len());
    let mut labels = llrm_support::hash::HashMap::default();
    for line in &lines {
        starts.push(at);
        if let Line::Label(name) = line {
            labels.insert(name.clone(), at);
        }
        at += i64::from(bytes_of(line));
    }
    // the loops, as the spans of the back edges
    let back_edges: Vec<(i64, i64)> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Line::Inst(text) => {
                let target = *labels.get(jump_of(text)?.1)?;
                (target <= starts[index]).then_some((target, starts[index]))
            }
            _ => None,
        })
        .collect();
    let in_loop = |at: i64| back_edges.iter().any(|&(head, edge)| head <= at && at <= edge);
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let Line::Inst(text) = line else {
            out.push(line.clone());
            continue;
        };
        let relaxed = jump_of(text).and_then(|(cc, label)| {
            let reach = |name: &str| matches!(name, "nz" | "z" | "nc" | "c");
            if cc.is_some_and(|cc| !reach(cc)) {
                return None;
            }
            let target = *labels.get(label)?;
            if speed && cc.is_none() && in_loop(starts[index]) {
                return None;
            }
            let distance = target - (starts[index] + 2);
            (distance.abs() <= REACH).then(|| (cc, label))
        });
        match relaxed {
            Some((cc, label)) => {
                // `dec b; jr nz, L` is `djnz L`, also past instructions that
                // neither touch the flags nor use b; a row
                // between does not part them
                let counter = (cc == Some("nz"))
                    .then(|| {
                        out.iter().rposition(|line| match line {
                            Line::Inst(text) => text == "dec b" || !flag_and_b_free(text),
                            Line::Loc(_) => false,
                            _ => true,
                        })
                    })
                    .flatten()
                    .filter(|&at| matches!(&out[at], Line::Inst(last) if last == "dec b"));
                if let Some(at) = counter {
                    out.remove(at);
                    out.push(Line::Inst(format!("djnz {label}")));
                } else {
                    out.push(Line::Inst(match cc {
                        Some(cc) => format!("jr {cc}, {label}"),
                        None => format!("jr {label}"),
                    }));
                }
            }
            None => out.push(line.clone()),
        }
    }
    out
}

/// Whether `text` leaves the flags alone and does not read or write `b`: a
/// load (`ld` sets no flag) or a 16-bit `inc`/`dec` (neither does), so a
/// `dec b` before it may move behind it.
fn flag_and_b_free(text: &str) -> bool {
    let (mnemonic, operands) = text.split_once(' ').unwrap_or((text, ""));
    let names_b = |operand: &str| {
        let bare = operand.trim_matches(|c| c == '(' || c == ')');
        bare == "b" || bare == "bc"
    };
    match mnemonic {
        "ld" => !operands.split(", ").any(names_b),
        "inc" | "dec" => matches!(operands, "hl" | "de" | "ix" | "iy"),
        _ => false,
    }
}

/// The stack arguments of a call, all words in a register pair, are pushed in
/// place of reserving the area with `push bc` and storing into it: `ld hl, #0;
/// push hl` is 4 B where `push bc; ld hl, #0; ld -32(ix), l; ld -31(ix), h`
/// is 10. `X86CallFrameOptimization`, which turns the stores of a reserved
/// call frame into pushes, for the stores that run from the top word down.
fn push_arguments(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let pair_of = |operand: Option<&Operand>| {
        let register = match operand {
            Some(Operand::Use(Reg::Virt(v))) => assignment.0[*v as usize],
            Some(Operand::Use(Reg::Phys(id))) => *id,
            _ => return None,
        };
        [reg::BC, reg::DE, reg::HL].into_iter().find(|pair| register == *pair)
    };
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let Kind::Op("ADJCALLSTACKDOWN") = block.insts[at].kind else {
                at += 1;
                continue;
            };
            let Some(Operand::Imm(bytes @ 2..=16)) = block.insts[at].operands.first().cloned() else {
                at += 1;
                continue;
            };
            let mut stores = Vec::new();
            let mut next = bytes - 2;
            let mut legal = bytes % 2 == 0;
            let mut end = at + 1;
            while legal && end < block.insts.len() && !block.insts[end].call {
                let inst = &block.insts[end];
                match inst.kind {
                    Kind::Op("STSP16") => {
                        legal = matches!(
                            inst.operands.get(1),
                            Some(Operand::Imm(offset)) if *offset == next
                        ) && pair_of(inst.operands.first()).is_some();
                        stores.push(end);
                        next -= 2;
                    }
                    Kind::Op("STSP8" | "ADJCALLSTACKDOWN" | "ADJCALLSTACKUP" | "PUSHP" | "POPP" | "PEEKP") => {
                        legal = false
                    }
                    _ => {}
                }
                end += 1;
            }
            let called = end < block.insts.len() && block.insts[end].call;
            if !(legal && called && next == -2) {
                at += 1;
                continue;
            }
            for &store in &stores {
                block.insts[store].kind = Kind::Op("PUSHARG");
            }
            block.insts.remove(at);
        }
    }
}

/// `call f; ret` is `jp f`: the callee returns to our caller with its result
/// in the registers ours would be returned in. Only a call that put nothing on
/// the stack (the caller cleans up after it).
fn tail_calls(function: &MFunction) -> MFunction {
    let mut function = function.clone();
    // The frame is gone when the callee runs, so a local whose address was
    // taken may be read by it; LLVM marks no such call `tail`.
    let escapes = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .any(
            |inst| inst.kind == Kind::Op("LEAFI")
                && matches!(
                    inst.operands.get(1),
                    Some(Operand::Frame(index)) if function.locals.iter().any(|&(local, ..)| local == *index)
                ),
        );
    if escapes {
        return function;
    }
    for block in &mut function.blocks {
        let insts = &block.insts;
        let n = insts.len();
        let bare_adjust = |inst: &Inst| {
            matches!(inst.kind, Kind::Op("ADJCALLSTACKUP")) && matches!(inst.operands.first(), Some(Operand::Imm(0)))
        };
        if n >= 2 && insts[n - 1].kind == Kind::Op("RET") {
            let at = if bare_adjust(&insts[n - 2]) { n - 3 } else { n - 2 };
            if n >= 2 && at < n && insts[at].call && insts[at].kind == Kind::Op("CALL") && (at == n - 2 || at == n - 3)
            {
                let symbol = insts[at].operands.first().cloned();
                if let Some(symbol @ Operand::Symbol(..)) = symbol {
                    // the call's argument registers stay read: the copies that
                    // fill them are not dead
                    let mut operands = insts[at].operands.clone();
                    operands[0] = symbol;
                    let mut tail = Inst::new(Kind::Op("TAILCALL"), operands);
                    tail.terminator = true;
                    tail.barrier = true;
                    block.insts.truncate(at);
                    block.insts.push(tail);
                }
            }
        }
    }
    function
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `form` spelled a register copy `ld b;b`, a key the table lacks, so
    /// every copy was priced as an unknown form (2 B, 8 T) instead of
    /// 1 B, 4 T, and the choice between allocations over-counted copies.
    #[test]
    fn a_register_copy_is_priced_from_the_table() {
        for text in ["ld a, b", "ld h, l", "ld b, b"] {
            assert_eq!(price(&[Line::Inst(text.to_owned())]), (1, 4), "{text}");
        }
        // a store of a register keeps its own key
        assert_eq!(price(&[Line::Inst("ld (hl), b".to_owned())]), (1, 7));
    }

    /// A `ret` behind `ret z` that nothing names was printed: 1 B dead, and
    /// the price of the variant that had it was 6 T too high.
    #[test]
    fn code_after_a_return_that_nothing_names_is_dropped() {
        let inst = |text: &str| Line::Inst(text.to_owned());
        let label = |name: &str| Line::Label(name.to_owned());
        let lines = vec![
            inst("ret z"),
            inst("jr L1"),
            label("L2"),
            inst("ret"),
            label("L3"),
            inst("ret"),
            label("L1"),
            inst("ld a, b"),
            inst("ret"),
        ];
        let kept = drop_unreachable(lines);
        assert_eq!(kept, vec![inst("ret z"), inst("jr L1"), label("L1"), inst("ld a, b"), inst("ret")]);
    }

    /// `dec hl; ...; inc hl` round lines that do not name the pair was kept:
    /// the copy loop of loop_fusion -O3 ran 15 instructions a trip.
    #[test]
    fn a_step_is_undone_across_lines_that_leave_the_pair_alone() {
        let inst = |text: &str| Line::Inst(text.to_owned());
        let kept = cancel_steps(vec![inst("dec hl"), inst("ld -2(ix), c"), inst("inc hl")]);
        assert_eq!(kept, vec![inst("ld -2(ix), c")]);
        let kept = cancel_steps(vec![inst("dec hl"), inst("ld (hl), d"), inst("inc hl")]);
        assert_eq!(kept.len(), 3, "a store through the pointer reads it: {kept:?}");
    }

    /// A pair stored to its slot and reloaded after lines that leave it alone
    /// was loaded again (loop_fusion -O3: 4 B and 34 T a trip).
    #[test]
    fn a_reload_of_what_was_just_stored_is_not_printed() {
        let inst = |text: &str| Line::Inst(text.to_owned());
        let stores = vec![("-2(ix)".to_owned(), "c".to_owned()), ("-1(ix)".to_owned(), "b".to_owned())];
        let out = vec![inst("ld -2(ix), c"), inst("ld -1(ix), b"), inst("inc hl"), inst("inc de")];
        assert!(held_since_stored(&out, &stores));
        let out = vec![inst("ld -2(ix), c"), inst("ld -1(ix), b"), inst("add hl, bc")];
        assert!(!held_since_stored(&out, &stores), "bc is read, not changed, but stays out of the proof");
        let out = vec![inst("ld -2(ix), c"), inst("ld -1(ix), b"), inst("ld c, 5")];
        assert!(!held_since_stored(&out, &stores));
        let out = vec![inst("ld -2(ix), c"), inst("ld -1(ix), b"), inst("call _f")];
        assert!(!held_since_stored(&out, &stores));
    }

    /// A pair copied, its high byte read from the copy and widened was four
    /// loads (switch_table_holes -Os: 5 B over the allocation before).
    #[test]
    fn the_high_byte_of_a_copied_pair_is_read_from_the_source() {
        let inst = |text: &str| Line::Inst(text.to_owned());
        let lines = vec![inst("ld c, l"), inst("ld b, h"), inst("ld c, b"), inst("ld b, #0")];
        assert_eq!(forward_high_bytes(lines), vec![inst("ld c, h"), inst("ld b, #0")]);
        // the copy read again is kept
        let lines = vec![inst("ld c, l"), inst("ld b, h"), inst("ld c, b"), inst("ld b, #0"), inst("add hl, bc")];
        assert_eq!(forward_high_bytes(lines).len(), 3);
    }

    /// A loop with two jumps back to its head was counted as two nested
    /// loops, 8 times too heavy: lz4 -O2 priced a candidate with 40% fewer
    /// real clocks as 40% dearer.
    #[test]
    fn a_loop_with_two_jumps_back_is_one_loop() {
        let inst = |text: &str| Line::Inst(text.to_owned());
        let lines = vec![Line::Label("L1".into()), inst("ld a, b"), inst("jr nz, L1"), inst("jr z, L1")];
        assert_eq!(price_in_loops(&lines).1, price(&lines).1 as u64 * 8);
    }
}
