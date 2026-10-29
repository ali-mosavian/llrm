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

/// The lines of one basic block, and where control goes after it.
struct BlockLines {
    start: usize,
    end: usize,
    /// `None`: it leaves the function or goes somewhere the parse does not follow.
    succs: Option<Vec<usize>>,
}

/// Basic blocks of `lines`: a label starts one, a branch or return ends one.
fn parse_blocks(lines: &[Line]) -> Vec<BlockLines> {
    let mut starts = vec![0];
    for (at, line) in lines.iter().enumerate() {
        match line {
            Line::Label(_) if at > 0 => starts.push(at),
            Line::Inst(inst) if leaves_straight_line(inst.opcode) || inst.opcode == "RTS" => starts.push(at + 1),
            _ => {}
        }
    }
    starts.sort_unstable();
    starts.dedup();
    starts.retain(|&at| at < lines.len());
    let label_block = |label: usize| {
        starts.iter().position(|&at| matches!(lines[at], Line::Label(one) if one == label))
    };
    let mut blocks: Vec<BlockLines> = Vec::new();
    for (n, &start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(lines.len());
        let last = lines[start..end].iter().rev().find_map(|line| match line {
            Line::Inst(inst) => Some(inst),
            Line::Label(_) => None,
        });
        let next = (n + 1 < starts.len()).then_some(n + 1);
        let succs = match last {
            None => Some(next.into_iter().collect()),
            Some(inst) if crate::overlay::is_branch_on_flags(inst.opcode) => {
                crate::overlay::branch_target(inst).and_then(label_block).map(|to| next.into_iter().chain([to]).collect())
            }
            Some(inst) if inst.opcode.starts_with("BRA") => {
                crate::overlay::branch_target(inst).and_then(label_block).map(|to| vec![to])
            }
            Some(inst) if leaves_straight_line(inst.opcode) && !["JSR", "BSR", "CALL"].iter().any(|p| inst.opcode.starts_with(p)) => {
                None
            }
            Some(inst) if inst.opcode == "RTS" => None,
            Some(_) => Some(next.into_iter().collect()),
        };
        blocks.push(BlockLines { start, end, succs });
    }
    blocks
}

fn is_step(
    inst: &Inst,
    root: RegId,
    bytes: i64,
) -> bool {
    matches!(inst.opcode, "ADDQ32a" | "ADD32ai")
        && matches!(inst.operands.first(), Some(Operand::Reg(r)) if REGISTER_INFO.root(*r) == root)
        && matches!(inst.operands.last(), Some(Operand::Imm(n)) if *n == bytes)
}

/// Edits for the mentions of `root` in `lines[range]`, each an access that
/// can take its displacement `bytes` less; none if one cannot.
fn adjustments(
    lines: &[Line],
    range: std::ops::Range<usize>,
    root: RegId,
    bytes: i64,
    out: &mut Vec<Edit>,
) -> bool {
    for at in range {
        let Line::Inst(inst) = &lines[at] else { continue };
        if mention_count(inst, root) == 0 {
            continue;
        }
        if mention_count(inst, root) != 1 {
            return false;
        }
        let Some(slot) = slots(inst.opcode).into_iter().find(|slot| base_register(inst, *slot).is_some_and(|r| REGISTER_INFO.root(r) == root))
        else {
            return false;
        };
        let Some(disp) = displacement(inst, slot) else { return false };
        let moved = disp - bytes;
        let fits = match slot.mode {
            Mode::Ari => with_mode(inst.opcode, slot, 'p').is_some() && i16::try_from(moved).is_ok(),
            Mode::Arid => i16::try_from(moved).is_ok(),
            Mode::Arii => i8::try_from(moved).is_ok(),
        };
        if !fits {
            return false;
        }
        out.push(Edit::Back(at, slot, bytes));
    }
    true
}

/// The access at line `at` of block `home` steps the register `step` lines
/// later in block `tail`, with nothing but adjustable accesses between: the
/// edits, and the line of the step to drop.
fn across(
    lines: &[Line],
    blocks: &[BlockLines],
    home: usize,
    at: usize,
    first: Slot,
    tail: usize,
    step: usize,
) -> Option<(Vec<Edit>, usize)> {
    let Line::Inst(head) = &lines[at] else { return None };
    let root = REGISTER_INFO.root(base_register(head, first)?);
    let bytes = first.bytes;
    // the blocks control passes through from the access to the step
    let mut between: Vec<usize> = Vec::new();
    let mut work: Vec<usize> = blocks[home].succs.clone()?;
    let mut reached = false;
    while let Some(block) = work.pop() {
        if block == tail {
            reached = true;
        } else if block == home {
            // the access would run twice before the step
            return None;
        } else if !between.contains(&block) {
            between.push(block);
            work.extend(blocks[block].succs.clone()?);
        }
    }
    if !reached {
        return None;
    }
    // nothing else enters: every way into the step and the blocks before it
    // comes from the access
    let mut preds = vec![Vec::new(); blocks.len()];
    for (from, one) in blocks.iter().enumerate() {
        for &to in one.succs.iter().flatten() {
            preds[to].push(from);
        }
    }
    for &block in between.iter().chain([&tail]) {
        if block == 0 || preds[block].iter().any(|from| *from != home && !between.contains(from)) {
            return None;
        }
    }
    let mut edits = vec![Edit::Step(at, first)];
    if !adjustments(lines, at + 1..blocks[home].end, root, bytes, &mut edits) {
        return None;
    }
    for &block in &between {
        if !adjustments(lines, blocks[block].start..blocks[block].end, root, bytes, &mut edits) {
            return None;
        }
    }
    if !adjustments(lines, blocks[tail].start..step, root, bytes, &mut edits) {
        return None;
    }
    Some((edits, step))
}

/// The step of a pointer in a later block becomes the post-increment of its
/// first access: `move.l (%a1), %d5` ... `move.l %d6, (%a1)` ...
/// `addq.l #4, %a1` in the latch is `move.l (%a1)+, %d5` ... `move.l %d6,
/// (-4,%a1)`, with the step gone. The step must be reached from the access on
/// every path, by nothing else, and the register mentioned in between only by
/// accesses that can take the displacement; a way out of that span, a second
/// step or a second pass over the access keeps the code as it was.
///
/// DEVIATE from the pin, as [`chain_post_increment`]. Exit: never.
pub fn loop_post_increment(mut lines: Vec<Line>) -> Vec<Line> {
    'again: loop {
        let blocks = parse_blocks(&lines);
        for (home, block) in blocks.iter().enumerate() {
            for at in block.start..block.end {
                let Line::Inst(inst) = &lines[at] else { continue };
                for first in slots(inst.opcode) {
                    let Some(base) = base_register(inst, first) else { continue };
                    let root = REGISTER_INFO.root(base);
                    if !is_address_register(root)
                        || mention_count(inst, root) != 1
                        || displacement(inst, first) != Some(0)
                        || first.mode == Mode::Arii
                        || post_increment(inst.opcode, first).is_none()
                    {
                        continue;
                    }
                    for (tail, other) in blocks.iter().enumerate() {
                        if tail == home {
                            continue;
                        }
                        for step in other.start..other.end {
                            let Line::Inst(candidate) = &lines[step] else { continue };
                            if !is_step(candidate, root, first.bytes) {
                                continue;
                            }
                            if let Some((edits, drop)) = across(&lines, &blocks, home, at, first, tail, step) {
                                apply(&mut lines, edits);
                                lines.remove(drop);
                                continue 'again;
                            }
                        }
                    }
                }
            }
        }
        return lines;
    }
}
