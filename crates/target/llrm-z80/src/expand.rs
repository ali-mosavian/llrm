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
    offsets: std::collections::HashMap<i32, i64>,
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
    reads: std::collections::HashMap<u32, usize>,
    /// A reload the next instruction reads from the frame itself: the register
    /// it would have gone to, and the cell.
    folded: Option<(RegId, String)>,
    /// Instructions after the current one that were printed with it.
    skip: usize,
    /// Bytes of stack arguments reserved below the frame, while a call is being
    /// set up.
    call_bytes: i64,
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
            "CP16" | "CP16i" => self.compare16(inst, rest)?,
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
            "STSP8" | "STSP16" => {
                let Some(Operand::Imm(offset)) = inst.operands.get(1) else {
                    return Err("a stack store without an offset".to_owned());
                };
                // The area is addressed from IX when its bytes are within the
                // displacement; a frame past that sets IY to the area's
                // first byte (`ld iy, #n; add iy, sp`), when nothing holds IY.
                let width = if op == "STSP8" { 1 } else { 2 };
                let from_ix: Vec<i64> =
                    (0..width).map(|extra| offset + extra - self.frame.below_ix() - self.call_bytes).collect();
                let places: Vec<String> = if from_ix.iter().all(|at| (-128..=127).contains(at)) {
                    from_ix.iter().map(|at| format!("{at}(ix)")).collect()
                } else if self.iy_is_live() {
                    return Err(format!("a stack argument {} from ix and iy is live", from_ix[0]));
                } else {
                    self.push(format!("ld iy, #{offset}"));
                    self.push("add iy, sp");
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
                self.push(format!("ld hl, #{}", offset + self.frame.below_ix() + self.call_bytes));
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
            "PUSHP" | "POPP" => {
                if self.call_bytes != 0 {
                    return Err("a pushed spill inside the stack arguments of a call".to_owned());
                }
                let register = self.register(inst, 0)?;
                let pair = [reg::BC, reg::DE, reg::HL]
                    .into_iter()
                    .find(|pair| REGISTER_INFO.root(register) == *pair)
                    .ok_or("a push of something that is no pair")?;
                self.push(format!("{} {}", if op == "PUSHP" { "push" } else { "pop" }, name(pair)));
            }
            "SPILL8" | "RELOAD8" => {
                let (index, register) =
                    if op == "SPILL8" { (self.frame_index(inst, 0)?, 1) } else { (self.frame_index(inst, 1)?, 0) };
                let register = name_of(self, register)?;
                if op == "RELOAD8" && self.updated_in_place(inst, rest)? {
                    return Ok(());
                }
                let slot = self.frame_slots(index, 0, 1)?.remove(0);
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
            "SPILL16" | "RELOAD16" => {
                let (index, register) =
                    if op == "SPILL16" { (self.frame_index(inst, 0)?, 1) } else { (self.frame_index(inst, 1)?, 0) };
                let (lo, hi) = lanes(self.register(inst, register)?)?;
                if op == "RELOAD16"
                    && self.call_bytes == 0
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
                match (one.kind, one.operands.get(1)) {
                    (Kind::Op("SETCC8"), Some(Operand::Imm(2 | 3)))
                    | (Kind::Op("JPC" | "JPNC" | "RETC" | "RETNC"), _) => carry = true,
                    (Kind::Op("SETCC8"), Some(Operand::Imm(0 | 1)))
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
        let mut inside: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
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
        self.push(format!("ld iy, #{}", at + self.frame.below_ix() + self.call_bytes));
        self.push("add iy, sp");
        Ok((0..count).map(|byte| format!("{byte}(iy)")).collect())
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
    // Slots alone: bytes first, then the words, so the last word lies at SP and
    // each word is one `push`.
    let aligned = function.locals.is_empty() && slots.iter().all(|&(_, size)| size == 1 || size == 2);
    let mut ordered = slots.to_vec();
    if aligned {
        ordered.sort_by_key(|&(_, size)| size);
    }
    for (index, size) in ordered {
        if aligned && size == 2 && frame.size % 2 == 1 {
            frame.size += 1;
        }
        frame.size += size as i64;
        frame.offsets.insert(index, -frame.size);
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
pub fn reads(function: &MFunction) -> std::collections::HashMap<u32, usize> {
    let mut reads = std::collections::HashMap::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Use(Reg::Virt(v)) = operand {
                *reads.entry(*v).or_default() += 1;
            }
        }
    }
    reads
}

/// `reads` is [`reads`] of the function as the allocator left it: the copies
/// it dropped as identities were reads too.
pub fn finish(
    function: &MFunction,
    assignment: &Assignment,
    number: usize,
    slots: &[(i32, usize)],
    reads: std::collections::HashMap<u32, usize>,
) -> Result<Vec<Line>, String> {
    let mut function = tail_calls(function);
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
        liveness,
        function,
        at: (0, 0),
    };
    if printer.frame.active {
        for line in prologue(&printer.frame, &pushed) {
            printer.push(line);
        }
    }
    for (at, block) in function.blocks.iter().enumerate() {
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
    Ok(printer.out)
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
            "a" if first && matches!(mnemonic, "add" | "adc" | "sbc") => "a".to_owned(),
            "a" | "b" | "c" | "d" | "e" | "h" | "l" => {
                if mnemonic == "ld" && one == "a" && !first {
                    "b".to_owned()
                } else {
                    "b".to_owned()
                }
            }
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
    let kinds: Vec<String> = operands.iter().enumerate().map(|(at, one)| kind(one, at == 0)).collect();
    // a conditional jump or call names its condition first
    let mut key = mnemonic.to_owned();
    if !kinds.is_empty() {
        key.push(' ');
        key.push_str(&kinds.join(";"));
    }
    // `jp nz, L` is `jp nz;nn`; a bare condition on ret is `ret nz`
    key
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

/// The bytes of one printed line, jumps being the three-byte form.
fn bytes_of(line: &Line) -> u32 {
    let Line::Inst(text) = line else { return 0 };
    let key = form(text);
    crate::timing::cost(&key)
        .or_else(|| crate::timing::cost(&key.replace("hl", "bc")))
        .or_else(|| crate::timing::cost(&key.replace("ix", "bc")))
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
/// against 10); SDCC's choice, kept for the byte. Exit: block frequencies at
/// this stage.
pub fn relax(lines: Vec<Line>) -> Vec<Line> {
    const REACH: i64 = 100;
    let mut at = 0;
    let mut starts = Vec::with_capacity(lines.len());
    let mut labels = std::collections::HashMap::new();
    for line in &lines {
        starts.push(at);
        if let Line::Label(name) = line {
            labels.insert(name.clone(), at);
        }
        at += i64::from(bytes_of(line));
    }
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
            let distance = labels.get(label)? - (starts[index] + 2);
            (distance.abs() <= REACH).then(|| (cc, label))
        });
        match relaxed {
            Some((cc, label)) => {
                // `dec b; jr nz, L` is `djnz L`
                // a row between the two does not part them
                let before = out.iter().rposition(|line| !matches!(line, Line::Loc(_)));
                let counted = cc == Some("nz")
                    && before.is_some_and(|at| matches!(&out[at], Line::Inst(last) if last == "dec b"));
                if counted {
                    out.remove(before.expect("a dec b"));
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

/// `call f; ret` is `jp f`: the callee returns to our caller with its result
/// in the registers ours would be returned in. Only a call that put nothing on
/// the stack (the caller cleans up after it).
fn tail_calls(function: &MFunction) -> MFunction {
    let mut function = function.clone();
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
                    let mut tail = Inst::new(Kind::Op("TAILCALL"), vec![symbol]);
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
