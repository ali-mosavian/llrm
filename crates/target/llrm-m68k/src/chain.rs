//! Accesses through one address register that step by their own size are
//! `(An)+`: `move.l (%a0), (%a1); move.l (4,%a0), (4,%a1); move.l (8,%a0),
//! (8,%a1)` is three `move.l (%a0)+, (%a1)+`, 2 bytes each where the later two
//! were 6. An inline `memcpy`, a struct assignment or an unrolled walk makes
//! them.
//!
//! # How it works
//! [`post_increment_chains`] runs after allocation. A base is a virtual
//! register defined once and used only later in its own block, by memory
//! operands. Its accesses in order, the first at displacement 0 and each next
//! at the end of the one before, become the `(An)+` form of the pin's row (the
//! mode letter `p` or `j` of the operand is `o`). The register then holds the
//! end of the chain, so every later access through it takes the displacement
//! less the chain's size. Each base is priced on the CPU, the clocks first and
//! the bytes at the size level, and left alone when it does not win.
//!
//! The copies of an inline `memcpy` are in address order for this
//! (`ordered_copies`). The pass runs before the copies of an allocated
//! register are dropped, so a copy of the base is a use it refuses; a physical
//! register an instruction reads (a returned pointer) is never a base.
//!
//! DEVIATE: the pin's `CombineToPostIndexedLoadStore` refuses a base whose
//! other users are `add base, c` that fold into displacements, which is every
//! inline copy, and its M68k target sets no indexed addressing at all. gcc's
//! `auto_inc_dec` forms these. Reason: bytes and clocks on every struct copy.
//! Exit: none. Off for `Options::PINNED`.
//!
//! # See also
//! `AArch64LoadStoreOpt::findMatchingUpdateInsnForward`, which merges a
//! separate update instead, and `auto_inc_dec` in gcc.

use std::collections::HashSet;

use llrm_lir::registers::RegId;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::dagisel::hooks::Cpu;
use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, mc, reg};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Ari,
    Arid,
    Arii,
}

/// A memory operand of an instruction: its mode, where its operands start,
/// how many bytes it accesses, and where in the instruction's name the letter
/// of its mode is, if it has one.
#[derive(Clone, Copy, Debug)]
struct Slot {
    mode: Mode,
    at: usize,
    bytes: i64,
    letter_at: Option<usize>,
}

impl Slot {
    /// The operand that names the base register.
    fn base_at(self) -> usize {
        self.at + usize::from(self.mode != Mode::Ari)
    }
}

/// `MOVSXd32p8`: the extending load, a pseudo with no printed operands that
/// `ExpandMOVSZX_RM` turns into a move and an extension. The mode letter
/// follows the two digits of the destination width.
fn extending_load(name: &str) -> Option<Slot> {
    let rest = name.strip_prefix("MOVSXd").or_else(|| name.strip_prefix("MOVZXd"))?;
    let (mode, width) = match rest.get(2..3)? {
        "j" => (Mode::Ari, rest.get(3..)?),
        "p" => (Mode::Arid, rest.get(3..)?),
        "f" => (Mode::Arii, rest.get(3..)?),
        _ => return None,
    };
    let bytes = match width {
        "8" => 1,
        "16" => 2,
        _ => return None,
    };
    Some(Slot { mode, at: 1, bytes, letter_at: Some(name.len() - width.len() - 1) })
}

fn slots(name: &str) -> Vec<Slot> {
    if let Some(slot) = extending_load(name) {
        return vec![slot];
    }
    let Some(&(_, _, printed)) = local::text(name) else { return Vec::new() };
    let digits_end = name.rfind(|c: char| c.is_ascii_digit()).map_or(0, |at| at + 1);
    printed
        .iter()
        .filter_map(|one| {
            let (mode, rest) = if let Some(rest) = one.method.strip_prefix("printARII") {
                (Mode::Arii, rest)
            } else if let Some(rest) = one.method.strip_prefix("printARID") {
                (Mode::Arid, rest)
            } else if let Some(rest) = one.method.strip_prefix("printARI") {
                (Mode::Ari, rest)
            } else {
                return None;
            };
            let bytes = match rest {
                "8Mem" => 1,
                "16Mem" => 2,
                "32Mem" => 4,
                _ => return None,
            };
            let letter_at = match name.len() - digits_end {
                1 => Some(digits_end),
                2 => Some(digits_end + usize::from(one.name != "dst")),
                _ => None,
            };
            Some(Slot { mode, at: usize::from(one.at), bytes, letter_at })
        })
        .collect()
}

/// `name` with the mode letter of `slot` made `to`, when the pin has that row.
fn with_mode(
    name: &str,
    slot: Slot,
    to: char,
) -> Option<&'static str> {
    let at = slot.letter_at?;
    let renamed = format!("{}{to}{}", name.get(..at)?, name.get(at + 1..)?);
    local::name_of(u32::from(local::code_of(&renamed)?))
}

fn base_of(
    inst: &Inst,
    slot: Slot,
) -> Option<u32> {
    match inst.operands.get(slot.base_at())? {
        Operand::Use(Reg::Virt(v)) => Some(*v),
        _ => None,
    }
}

fn displacement(
    inst: &Inst,
    slot: Slot,
) -> Option<i64> {
    match slot.mode {
        Mode::Ari => Some(0),
        Mode::Arid | Mode::Arii => match inst.operands.get(slot.at)? {
            Operand::Imm(disp) => Some(*disp),
            _ => None,
        },
    }
}

/// Where a virtual register is defined and used.
struct Life {
    /// The block of every mention.
    block: usize,
    /// The instruction that defines it, when that is the first mention and
    /// the only def.
    def: Option<usize>,
    /// The instruction of each use, in order.
    uses: Vec<usize>,
    /// Every mention is in `block`.
    local: bool,
}

fn lives(function: &MFunction) -> Vec<Option<Life>> {
    let mut all: Vec<Option<Life>> = (0..function.classes.len()).map(|_| None).collect();
    for (b, block) in function.blocks.iter().enumerate() {
        for (at, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                let (Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v))) = operand else { continue };
                let is_def = matches!(operand, Operand::Def(_));
                let life = all[*v as usize].get_or_insert(Life {
                    block: b,
                    def: is_def.then_some(at),
                    uses: Vec::new(),
                    local: true,
                });
                life.local &= life.block == b;
                if is_def {
                    // a second def is not a chain's base
                    if life.def != Some(at) {
                        life.def = None;
                        life.local = false;
                    }
                } else {
                    life.uses.push(at);
                }
            }
        }
    }
    all
}

/// The clocks and bytes of `inst` on `cpu`, once allocated.
fn price(
    inst: &Inst,
    assignment: &Assignment,
    cpu: Cpu,
) -> Option<(u32, u32)> {
    let Kind::Op(opcode) = inst.kind else { return None };
    // the pseudo costs what its move does; the extensions cost the same either
    // way
    let opcode = match extending_load(opcode) {
        Some(slot) => {
            let letter = &opcode[slot.letter_at?..=slot.letter_at?];
            local::name_of(u32::from(local::code_of(&format!("MOV{}d{letter}", slot.bytes * 8))?))?
        }
        None => opcode,
    };
    let mut operands = Vec::new();
    for operand in &inst.operands {
        operands.push(match operand {
            Operand::Def(r) | Operand::Use(r) => mc::Operand::Reg(match r {
                Reg::Virt(v) => assignment.0[*v as usize],
                Reg::Phys(p) => *p,
            }),
            Operand::Imm(value) => mc::Operand::Imm(*value),
            _ => return None,
        });
    }
    let one = mc::Inst { opcode, operands };
    let bytes = crate::encode::encode(&one).ok()?.bytes.len() as u32;
    let clocks = crate::timing::cost(cpu, &mc::print(&one).ok()?, bytes, false).ok()?.clocks;
    Some((clocks, bytes))
}

/// The access in `slot` of `inst` as `(An)+`.
fn stepped(
    inst: &Inst,
    slot: Slot,
) -> Option<Inst> {
    let Kind::Op(name) = inst.kind else { return None };
    if slot.mode == Mode::Arii {
        return None;
    }
    let renamed = with_mode(name, slot, 'o')?;
    let desc = local::code_of(renamed).and_then(local::desc)?;
    let mut operands = inst.operands.clone();
    if slot.mode == Mode::Arid {
        operands.remove(slot.at);
    }
    Some(op_inst(renamed, desc, operands))
}

/// The access in `slot` of `inst`, the register `size` bytes on.
fn moved_back(
    inst: &Inst,
    slot: Slot,
    size: i64,
) -> Option<Inst> {
    let Kind::Op(name) = inst.kind else { return None };
    let disp = displacement(inst, slot)? - size;
    match slot.mode {
        Mode::Ari => {
            let renamed = with_mode(name, slot, 'p')?;
            let desc = local::code_of(renamed).and_then(local::desc)?;
            let mut operands = inst.operands.clone();
            operands.insert(slot.at, Operand::Imm(disp));
            i16::try_from(disp).ok()?;
            Some(op_inst(renamed, desc, operands))
        }
        Mode::Arid | Mode::Arii => {
            if slot.mode == Mode::Arid {
                i16::try_from(disp).ok()?;
            } else {
                i8::try_from(disp).ok()?;
            }
            let mut again = inst.clone();
            again.operands[slot.at] = Operand::Imm(disp);
            Some(again)
        }
    }
}

/// The instructions to replace for the chain through `base`: their numbers
/// and what they become.
fn plan(
    insts: &[Inst],
    life: &Life,
    base: u32,
) -> Option<Vec<(usize, Inst)>> {
    let mut edits: Vec<(usize, Inst)> = Vec::new();
    let mut expected = 0;
    let mut chaining = true;
    for (n, &at) in life.uses.iter().enumerate() {
        // one mention of the base in an instruction
        if life.uses.get(n + 1) == Some(&at) || n > 0 && life.uses[n - 1] == at {
            return None;
        }
        let inst = &insts[at];
        let Kind::Op(name) = inst.kind else { return None };
        let slot = slots(name).into_iter().find(|slot| base_of(inst, *slot) == Some(base))?;
        let disp = displacement(inst, slot)?;
        if chaining && disp == expected {
            if let Some(step) = stepped(inst, slot) {
                edits.push((at, step));
                expected += slot.bytes;
                continue;
            }
        }
        if edits.is_empty() {
            return None;
        }
        chaining = false;
        edits.push((at, moved_back(inst, slot, expected)?));
    }
    Some(edits)
}

/// Makes each chain of the function `(An)+` where that costs less on `cpu`.
/// `size` puts the bytes before the clocks.
pub fn post_increment_chains(
    function: &mut MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
) {
    if function.blocks.iter().any(|block| !block.phis.is_empty()) {
        return;
    }
    let read: HashSet<RegId> = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .flat_map(|inst| &inst.operands)
        .filter_map(|operand| match operand {
            Operand::Use(Reg::Phys(p)) => Some(REGISTER_INFO.root(*p)),
            _ => None,
        })
        .collect();
    let lives = lives(function);
    let addresses = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5, reg::A6];
    for (base, life) in lives.iter().enumerate() {
        let Some(life) = life.as_ref().filter(|life| life.local && life.def.is_some()) else { continue };
        let root = REGISTER_INFO.root(assignment.0[base]);
        if !addresses.contains(&root) || read.contains(&root) {
            continue;
        }
        let insts = &function.blocks[life.block].insts;
        let Some(edits) = plan(insts, life, base as u32) else { continue };
        let mut before = (0, 0);
        let mut after = (0, 0);
        let mut priced = true;
        for (at, new) in &edits {
            match (price(&insts[*at], assignment, cpu), price(new, assignment, cpu)) {
                (Some(old), Some(now)) => {
                    before = (before.0 + old.0, before.1 + old.1);
                    after = (after.0 + now.0, after.1 + now.1);
                }
                _ => priced = false,
            }
        }
        let key = |(clocks, bytes): (u32, u32)| if size { (bytes, clocks) } else { (clocks, bytes) };
        if !priced || key(after) >= key(before) {
            continue;
        }
        let insts = &mut function.blocks[life.block].insts;
        for (at, new) in edits {
            insts[at] = new;
        }
    }
}
