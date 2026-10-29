//! Accesses through one address register that step by their own size are
//! `(An)+`: `move.l (%a0), (%a1); move.l (4,%a0), (4,%a1); move.l (8,%a0),
//! (8,%a1)` is three `move.l (%a0)+, (%a1)+`, 2 bytes each where the later two
//! were 6, and 4 clocks less an operand. The accesses are the ones of an
//! inline memcpy, a struct assignment, or an unrolled walk.
//!
//! A chain is accesses of one register, in program order, the first at
//! displacement 0 and each next one at the end of the one before; nothing
//! between them mentions the register, and no label or branch comes between
//! them and the point where the register is dead. Later accesses through the
//! register, which now holds the end of the chain, take the displacement less
//! the chain's size; any other mention of it, or a register that may be live
//! past the end, keeps the chain as it was.
//!
//! DEVIATE from the pin's output, and beyond LLVM: its `CombineToPostIndexedLoadStore`
//! refuses a base whose other users are `add base, c` that fold into
//! displacements (`shouldCombineToPostInc`), which is every inline copy, and the
//! M68k target sets no indexed addressing at all. gcc's `auto_inc_dec` forms
//! these. Reason: bytes and clocks on every struct copy. Exit: never; a target
//! choice, off with `Options::overlay`.

use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;
use crate::dagisel::asm::AsmOperand;
use crate::expand::Line;
use crate::mc::{Inst, Operand};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Ari,
    Arid,
    Arii,
}

/// A memory operand of an instruction: where its operands start, how many
/// bytes it accesses, and which letter of the instruction's name tells its mode.
#[derive(Clone, Copy, Debug)]
struct Slot {
    mode: Mode,
    at: usize,
    bytes: i64,
    letter: Option<usize>,
}

fn slots(opcode: &str) -> Vec<Slot> {
    let Some((_, _, operands)) = crate::dagisel::asm::asm(opcode) else { return Vec::new() };
    let digits_end = opcode.rfind(|c: char| c.is_ascii_digit()).map_or(0, |at| at + 1);
    let letters = &opcode[digits_end..];
    operands
        .iter()
        .filter_map(|one: &AsmOperand| {
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
            let letter = match letters.len() {
                1 => Some(0),
                2 => Some(usize::from(one.name != "dst")),
                _ => None,
            };
            Some(Slot { mode, at: usize::from(one.at), bytes, letter })
        })
        .collect()
}

/// `opcode` with the mode letter of `slot` made `to`, if the pin has that instruction.
fn with_mode(
    opcode: &str,
    slot: Slot,
    to: char,
) -> Option<&'static str> {
    let digits_end = opcode.rfind(|c: char| c.is_ascii_digit())? + 1;
    let (head, letters) = opcode.split_at(digits_end);
    let mut letters: Vec<char> = letters.chars().collect();
    *letters.get_mut(slot.letter?)? = to;
    let name = format!("{head}{}", letters.into_iter().collect::<String>());
    crate::dagisel::asm::ASM.binary_search_by_key(&name.as_str(), |row| row.0).ok().map(|at| crate::dagisel::asm::ASM[at].0)
}

fn base_register(
    inst: &Inst,
    slot: Slot,
) -> Option<RegId> {
    match (slot.mode, inst.operands.get(slot.at + usize::from(slot.mode != Mode::Ari))?) {
        (_, Operand::Reg(register)) => Some(*register),
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

fn mention_count(
    inst: &Inst,
    root: RegId,
) -> usize {
    inst.operands.iter().filter(|o| matches!(o, Operand::Reg(r) if REGISTER_INFO.root(*r) == root)).count()
}

fn is_address_register(root: RegId) -> bool {
    use crate::reg::*;
    [A0, A1, A2, A3, A4, A5, A6].contains(&root)
}

/// The `(An)+` form of the access at `slot`, and whether it takes the
/// displacement operand away.
fn post_increment(
    opcode: &str,
    slot: Slot,
) -> Option<&'static str> {
    if slot.mode == Mode::Arii {
        return None;
    }
    if opcode.starts_with("MOV") {
        return with_mode(opcode, slot, 'o');
    }
    let plain = if slot.mode == Mode::Arid { with_mode(opcode, slot, 'j')? } else { opcode };
    let (name, row) = crate::overlay::post_increment_form(plain)?;
    (row.operand == slot.at).then_some(name)
}

/// An instruction that writes `root` and reads nothing of it.
fn overwrites(
    inst: &Inst,
    root: RegId,
) -> bool {
    (inst.opcode.starts_with("MOV32a") || inst.opcode.starts_with("LEA32"))
        && matches!(inst.operands.first(), Some(Operand::Reg(r)) if REGISTER_INFO.root(*r) == root)
        && mention_count(inst, root) == 1
}

fn leaves_straight_line(opcode: &str) -> bool {
    crate::overlay::is_branch_on_flags(opcode)
        || ["BRA", "JSR", "BSR", "CALL", "JMP", "DBcc", "RTD", "RTE", "TRAP"].iter().any(|p| opcode.starts_with(p))
}

/// What to do to the line `at` when the chain applies.
enum Edit {
    /// The access at `slot` becomes `(An)+`.
    Step(usize, Slot),
    /// The access at `slot` goes back by `size`.
    Back(usize, Slot, i64),
}

/// The edits of the chain that starts at the access in `slot` of line `start`.
fn plan(
    lines: &[Line],
    start: usize,
    first: Slot,
    returned: &[RegId],
) -> Option<Vec<Edit>> {
    let Line::Inst(head) = &lines[start] else { return None };
    let root = REGISTER_INFO.root(base_register(head, first)?);
    if !is_address_register(root) || mention_count(head, root) != 1 || displacement(head, first)? != 0 {
        return None;
    }
    post_increment(head.opcode, first)?;
    let mut edits = vec![Edit::Step(start, first)];
    let mut expected = first.bytes;
    let mut chaining = true;
    for (at, line) in lines.iter().enumerate().skip(start + 1) {
        let Line::Inst(inst) = line else { return None };
        if mention_count(inst, root) == 0 {
            if inst.opcode == "RTS" {
                return (edits.len() >= 2 && !returned.iter().any(|r| REGISTER_INFO.root(*r) == root)).then_some(edits);
            }
            if leaves_straight_line(inst.opcode) {
                return None;
            }
            continue;
        }
        if overwrites(inst, root) {
            return (edits.len() >= 2).then_some(edits);
        }
        if mention_count(inst, root) != 1 {
            return None;
        }
        let slot = slots(inst.opcode).into_iter().find(|slot| base_register(inst, *slot).is_some_and(|r| REGISTER_INFO.root(r) == root))?;
        let disp = displacement(inst, slot)?;
        if chaining && disp == expected && slot.mode != Mode::Arii && post_increment(inst.opcode, slot).is_some() {
            edits.push(Edit::Step(at, slot));
            expected += slot.bytes;
            continue;
        }
        chaining = false;
        // the register is `expected` bytes on
        let moved = disp - expected;
        let fits = match slot.mode {
            Mode::Ari => with_mode(inst.opcode, slot, 'p').is_some() && i16::try_from(moved).is_ok(),
            Mode::Arid => i16::try_from(moved).is_ok(),
            Mode::Arii => i8::try_from(moved).is_ok(),
        };
        if !fits {
            return None;
        }
        edits.push(Edit::Back(at, slot, expected));
    }
    // the end of the function with no `rts`: nothing follows
    None
}

fn apply(
    lines: &mut [Line],
    edits: Vec<Edit>,
) {
    for edit in edits {
        match edit {
            Edit::Step(at, slot) => {
                let Line::Inst(inst) = &mut lines[at] else { continue };
                let form = post_increment(inst.opcode, slot).expect("planned");
                if slot.mode == Mode::Arid {
                    inst.operands.remove(slot.at);
                }
                inst.opcode = form;
            }
            Edit::Back(at, slot, size) => {
                let Line::Inst(inst) = &mut lines[at] else { continue };
                match slot.mode {
                    Mode::Ari => {
                        inst.opcode = with_mode(inst.opcode, slot, 'p').expect("planned");
                        inst.operands.insert(slot.at, Operand::Imm(-size));
                    }
                    Mode::Arid | Mode::Arii => {
                        if let Operand::Imm(disp) = &mut inst.operands[slot.at] {
                            *disp -= size;
                        }
                    }
                }
            }
        }
    }
}

/// Folds every chain of the function. `returned` are the registers the
/// function's `rts` leaves its result in.
pub fn chain_post_increment(
    mut lines: Vec<Line>,
    returned: &[RegId],
) -> Vec<Line> {
    let mut at = 0;
    while at < lines.len() {
        let Line::Inst(inst) = &lines[at] else {
            at += 1;
            continue;
        };
        let candidates = slots(inst.opcode);
        let mut done = false;
        for slot in candidates {
            if let Some(edits) = plan(&lines, at, slot, returned) {
                apply(&mut lines, edits);
                done = true;
                break;
            }
        }
        // the same line may have a second pointer to chain
        if !done {
            at += 1;
        }
    }
    lines
}
