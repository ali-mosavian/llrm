//! Instructions the pin's M68k `.td` does not define, and the rewrites that
//! use them: `addq`/`subq` for a constant 1 to 8 and `tst` for a compare with
//! zero. Each is smaller and faster on the 68000 than the pin's text
//! (`add.l #1, %d1` is 6 bytes and 16 clocks, `addq.l #1, %d1` 2 and 8;
//! `cmpi.l #0, %d1` 6 and 14, `tst.l %d1` 2 and 4), with the same condition
//! codes: `addq`/`subq` on a data register set them as `add`/`sub` do, `tst`
//! as `cmpi #0` does. On an address register `addq`/`subq` leave them alone,
//! as `adda`/`suba` do.
//!
//! DEVIATE from the pin's output (`M68kInstrArithmetic.td` marks `ADDQ` and
//! `SUBQ` as not implemented, and there is no `TST`). Reason: the user's
//! target is code smaller and faster than the reference. Exit: never; a
//! target choice, switched off by [`crate::compile::Options::overlay`] for the
//! tests that compare with `llc`.
//!
//! The `(An)+` fold is one too: LLVM forms post-increment accesses in the
//! indexed-load/store combines when the target marks them legal, and the pin's
//! M68k marks none. Same exit, same switch.
//!
//! Encodings are written here by hand from the 68000 manual and checked
//! against binutils `m68k-linux-gnu-as` in `tests/overlay.rs`.

use crate::REGISTER_INFO;
use crate::dagisel::asm::AsmOperand;
use crate::dagisel::enc::{Enc, Piece};
use crate::mc::{Inst, Operand};

const IMM_THEN_DST: &[AsmOperand] = &[
    AsmOperand { name: "imm", at: 1, method: "printImmediate" },
    AsmOperand { name: "dst", at: 0, method: "printOperand" },
];
const CMP_OPERANDS: &[AsmOperand] = &[
    AsmOperand { name: "lhs", at: 0, method: "printOperand" },
    AsmOperand { name: "rhs", at: 1, method: "printOperand" },
];
const SRC: &[AsmOperand] = &[AsmOperand { name: "src", at: 0, method: "printOperand" }];
const MASK: &[AsmOperand] = &[AsmOperand { name: "mask", at: 0, method: "printMoveMask" }];
const DST: &[AsmOperand] = &[AsmOperand { name: "dst", at: 0, method: "printOperand" }];

/// `(name, asm string, operands)`, as the pin's `asm` rows.
pub fn asm(name: &str) -> Option<(&'static str, &'static [AsmOperand])> {
    Some(match name {
        "ADDQ8d" => ("addq.b\t$imm, $dst", IMM_THEN_DST),
        "ADDQ16d" => ("addq.w\t$imm, $dst", IMM_THEN_DST),
        "ADDQ32d" | "ADDQ32a" => ("addq.l\t$imm, $dst", IMM_THEN_DST),
        "SUBQ8d" => ("subq.b\t$imm, $dst", IMM_THEN_DST),
        "SUBQ16d" => ("subq.w\t$imm, $dst", IMM_THEN_DST),
        "SUBQ32d" | "SUBQ32a" => ("subq.l\t$imm, $dst", IMM_THEN_DST),
        "PUSH32" => ("move.l\t$src, -(%sp)", SRC),
        "POP32" => ("move.l\t(%sp)+, $dst", DST),
        "PUSHM32" => ("movem.l\t$mask, -(%sp)", MASK),
        "POPM32" => ("movem.l\t(%sp)+, $mask", MASK),
        "TST8d" => ("tst.b\t$dst", DST),
        "TST16d" => ("tst.w\t$dst", DST),
        "TST32d" => ("tst.l\t$dst", DST),
        "CMPA32" => ("cmpa.l\t$lhs, $rhs", CMP_OPERANDS),
        _ => return post_increment_rows(name).map(|row| row.asm).or_else(|| tst_memory_rows(name).map(|row| row.asm)),
    })
}

const fn field(
    lo: u16,
    width: u8,
    at: u8,
    from: u8,
) -> Piece {
    Piece::Operand { lo, width, at, from, enc: Enc::Plain }
}

const fn bits(
    lo: u16,
    width: u8,
    value: u64,
) -> Piece {
    Piece::Const { lo, width, value }
}

const fn quick(
    kind: u64,
    size: u64,
) -> [Piece; 6] {
    // `0101 ddd k ss mmm rrr`: `k` 0 for add, 1 for subtract; the data field
    // holds 8 as 0 (the mask drops the high bit)
    [bits(12, 4, 0b0101), field(9, 3, 1, 0), bits(8, 1, kind), bits(6, 2, size), field(3, 3, 0, 3), field(0, 3, 0, 0)]
}

const fn test(size: u64) -> [Piece; 4] {
    // `0100 1010 ss mmm rrr`
    [bits(8, 8, 0b0100_1010), bits(6, 2, size), field(3, 3, 0, 3), field(0, 3, 0, 0)]
}

// `move.l reg, -(%sp)`: 0010 111 100 mmm rrr
static PUSH: [Piece; 5] = [bits(12, 4, 0b0010), bits(9, 3, 7), bits(6, 3, 0b100), field(3, 3, 0, 3), field(0, 3, 0, 0)];
// `move.l (%sp)+, reg`: 0010 rrr mmm 011 111
static POP: [Piece; 5] = [bits(12, 4, 0b0010), field(9, 3, 0, 0), field(6, 3, 0, 3), bits(3, 3, 0b011), bits(0, 3, 7)];
// `movem.l regs, -(%sp)`: 0x48E7, then the mask with d0 as bit 15
static PUSHM: [Piece; 2] =
    [bits(0, 16, 0x48E7), Piece::Operand { lo: 16, width: 16, at: 0, from: 0, enc: Enc::InverseMoveMask }];
// `movem.l (%sp)+, regs`: 0x4CDF, then the mask with d0 as bit 0
static POPM: [Piece; 2] = [bits(0, 16, 0x4CDF), field(16, 16, 0, 0)];

// `cmpa.l ea, An`: 1011 aaa 111 mmm rrr (`cmp.l` has 010 in the middle)
static CMPA: [Piece; 5] = [field(0, 4, 0, 0), bits(4, 2, 0), bits(6, 3, 7), field(9, 3, 1, 0), bits(12, 4, 0b1011)];

static ADDQ: [[Piece; 6]; 3] = [quick(0, 0), quick(0, 1), quick(0, 2)];
static SUBQ: [[Piece; 6]; 3] = [quick(1, 0), quick(1, 1), quick(1, 2)];
static TST: [[Piece; 4]; 3] = [test(0), test(1), test(2)];

/// `(width in bits, pieces)`, as the pin's `enc` rows.
pub fn encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    Some(match name {
        "ADDQ8d" => (16, &ADDQ[0]),
        "ADDQ16d" => (16, &ADDQ[1]),
        "ADDQ32d" | "ADDQ32a" => (16, &ADDQ[2]),
        "SUBQ8d" => (16, &SUBQ[0]),
        "SUBQ16d" => (16, &SUBQ[1]),
        "SUBQ32d" | "SUBQ32a" => (16, &SUBQ[2]),
        "PUSH32" => (16, &PUSH),
        "POP32" => (16, &POP),
        "PUSHM32" => (32, &PUSHM),
        "POPM32" => (32, &POPM),
        "TST8d" => (16, &TST[0]),
        "TST16d" => (16, &TST[1]),
        "TST32d" => (16, &TST[2]),
        "CMPA32" => (16, &CMPA),
        _ => {
            return post_increment_rows(name)
                .map(|row| row.encoding)
                .or_else(|| tst_memory_rows(name).map(|row| row.encoding));
        }
    })
}

fn is_address(register: llrm_lir::registers::RegId) -> bool {
    let root = REGISTER_INFO.root(register);
    [
        crate::reg::A0,
        crate::reg::A1,
        crate::reg::A2,
        crate::reg::A3,
        crate::reg::A4,
        crate::reg::A5,
        crate::reg::A6,
        crate::reg::SP,
    ]
    .contains(&root)
}

/// The smaller instruction for `inst`, if it has one.
pub fn rewrite(inst: Inst) -> Inst {
    let quickly = |name: &'static str, register, amount: i64| Inst {
        opcode: name,
        operands: vec![Operand::Reg(register), Operand::Imm(amount)],
    };
    if let [Operand::Imm(0), memory @ ..] = &inst.operands[..]
        && let Some((name, _)) = tst_memory_form(inst.opcode)
    {
        return Inst { opcode: name, operands: memory.to_vec() };
    }
    match (inst.opcode, &inst.operands[..]) {
        ("CMP32dr", [lhs, Operand::Reg(rhs)]) if is_address(*rhs) => {
            Inst { opcode: "CMPA32", operands: vec![lhs.clone(), Operand::Reg(*rhs)] }
        }
        (
            "ADD8di" | "ADD16di" | "ADD32di" | "ADD32ai" | "SUB8di" | "SUB16di" | "SUB32di" | "SUB32ai",
            &[Operand::Reg(dst), Operand::Reg(src), Operand::Imm(value @ (-8..=-1 | 1..=8))],
        ) if dst == src => {
            // adding -n is subtracting n and the reverse: the quick forms
            // reach 1..=8 only, and `N`, `Z` and `V` come out the same (`C`
            // does not, which no branch the compiler emits after them reads)
            let subtract = inst.opcode.starts_with("SUB") != (value < 0);
            // the name is `ADD`/`SUB`, the size, the register kind, then `i`
            let size = &inst.opcode[3..inst.opcode.len() - 2];
            let register = inst.opcode.as_bytes()[inst.opcode.len() - 2] as char;
            let name = match (subtract, size, register) {
                (false, "8", _) => "ADDQ8d",
                (false, "16", _) => "ADDQ16d",
                (false, _, 'a') => "ADDQ32a",
                (false, _, _) => "ADDQ32d",
                (true, "8", _) => "SUBQ8d",
                (true, "16", _) => "SUBQ16d",
                (true, _, 'a') => "SUBQ32a",
                (true, _, _) => "SUBQ32d",
            };
            quickly(name, dst, value.abs())
        }
        ("CMP8di", &[Operand::Imm(0), Operand::Reg(register)]) => {
            Inst { opcode: "TST8d", operands: vec![Operand::Reg(register)] }
        }
        ("CMP16di", &[Operand::Imm(0), Operand::Reg(register)]) => {
            Inst { opcode: "TST16d", operands: vec![Operand::Reg(register)] }
        }
        ("CMP32di", &[Operand::Imm(0), Operand::Reg(register)]) => {
            Inst { opcode: "TST32d", operands: vec![Operand::Reg(register)] }
        }
        _ => inst,
    }
}

/// What an instruction that writes a data register leaves in the condition
/// codes, from its mnemonic: `N` and `Z` from the result, and whether `V` is
/// cleared as `tst` clears it.
fn writes_flags(inst: &Inst) -> Option<(crate::mc::Operand, bool)> {
    let text =
        asm(inst.opcode).map(|(text, _)| text).or_else(|| crate::dagisel::asm::asm(inst.opcode).map(|row| row.1))?;
    let mnemonic = text.split('\t').next()?;
    let (name, size) = mnemonic.split_once('.').unwrap_or((mnemonic, "l"));
    if size != "l" && name != "moveq" {
        return None;
    }
    let clears_overflow = match name {
        "move" | "moveq" | "and" | "or" | "eor" | "not" | "lsr" | "asr" | "ext" | "clr" => true,
        "add" | "sub" | "addq" | "subq" | "neg" | "lsl" | "asl" => false,
        _ => return None,
    };
    let Some(Operand::Reg(dst)) = inst.operands.first() else { return None };
    // an address register is not changed in its codes (`movea`, `adda`)
    use crate::reg;
    let data = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7]
        .contains(&crate::REGISTER_INFO.root(*dst));
    data.then(|| (Operand::Reg(*dst), clears_overflow))
}

/// A `tst.l Dn` right after the instruction that wrote `Dn` is dropped when
/// the branches after it read only what that instruction left the same:
/// `eq`/`ne`/`mi`/`pl` always, and the signed orderings when `V` is cleared.
/// `lsr.l #1, %d1; tst.l %d1; bne` is `lsr.l #1, %d1; bne`.
pub fn drop_redundant_tests(lines: Vec<crate::expand::Line>) -> Vec<crate::expand::Line> {
    use crate::expand::Line;
    let readers_after = |from: usize| {
        lines[from..]
            .iter()
            .map_while(|line| match line {
                Line::Inst(inst) if is_branch_on_flags(inst.opcode) => Some(inst.opcode),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut drop = vec![false; lines.len()];
    for at in 1..lines.len() {
        let (Line::Inst(before), Line::Inst(test)) = (&lines[at - 1], &lines[at]) else { continue };
        if test.opcode != "TST32d" || drop[at - 1] {
            continue;
        }
        let Some((written, clears_overflow)) = writes_flags(before) else { continue };
        if test.operands.first() != Some(&written) {
            continue;
        }
        let branches = readers_after(at + 1);
        let safe = |name: &str| match name {
            "Bne8" | "Beq8" | "Bmi8" | "Bpl8" => true,
            "Bgt8" | "Bge8" | "Blt8" | "Ble8" => clears_overflow,
            _ => false,
        };
        // the codes must be read by a branch at all, and by nothing else
        let next_is_branch = matches!(
            lines.get(at + 1),
            Some(Line::Inst(next)) if is_branch_on_flags(next.opcode)
        );
        if next_is_branch && branches.iter().all(|name| safe(name)) {
            drop[at] = true;
        }
    }
    lines.into_iter().zip(drop).filter(|(_, dropped)| !dropped).map(|(line, _)| line).collect()
}

/// A conditional branch in its 8-bit form (the form the compiler emits; the
/// assembler relaxes it later).
fn is_branch_on_flags(opcode: &str) -> bool {
    matches!(
        opcode,
        "Bhi8"
            | "Bls8"
            | "Bcc8"
            | "Bcs8"
            | "Bne8"
            | "Beq8"
            | "Bvc8"
            | "Bvs8"
            | "Bpl8"
            | "Bmi8"
            | "Bge8"
            | "Blt8"
            | "Bgt8"
            | "Ble8"
    )
}

type AsmRow = (&'static str, &'static [AsmOperand]);
type EncodingRow = (u16, &'static [Piece]);

struct PostIncrementRow {
    asm: AsmRow,
    encoding: EncodingRow,
    /// The operand number of the `(An)` it reads or writes through.
    operand: usize,
    bytes: i64,
}

/// The `(An)+` form of every pin instruction that takes an `(An)` operand in
/// the effective-address field: the same bits with the mode 2 made 3, the text
/// with `(An)` made `(An)+`. Named after the `(An)` instruction with a `+`.
/// Where the pin has the form itself (`move`), the pin's is used.
fn post_increment_table() -> &'static std::collections::HashMap<&'static str, PostIncrementRow> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<&'static str, PostIncrementRow>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = std::collections::HashMap::new();
        let mut add = |name: &str, text: &'static str, operands: &[AsmOperand], width: u16, pieces: &[Piece]| {
            let Some(ari) =
                operands.iter().find(|one| one.method.starts_with("printARI") && one.method.ends_with("Mem"))
            else {
                return;
            };
            let bytes = match ari.method {
                "printARI8Mem" => 1,
                "printARI16Mem" => 2,
                "printARI32Mem" => 4,
                _ => return,
            };
            let mode = |piece: &Piece| matches!(piece, Piece::Const { lo: 3, width: 3, value: 2 });
            if pieces.iter().filter(|piece| mode(piece)).count() != 1 {
                return;
            }
            let pieces: Vec<Piece> = pieces
                .iter()
                .map(|piece| if mode(piece) { Piece::Const { lo: 3, width: 3, value: 3 } } else { *piece })
                .collect();
            let operands: Vec<AsmOperand> = operands
                .iter()
                .map(|one| AsmOperand {
                    name: one.name,
                    at: one.at,
                    method: if one.at != ari.at {
                        one.method
                    } else {
                        match one.method {
                            "printARI8Mem" => "printARIPI8Mem",
                            "printARI16Mem" => "printARIPI16Mem",
                            "printARI32Mem" => "printARIPI32Mem",
                            other => other,
                        }
                    },
                })
                .collect();
            let key: &'static str = Box::leak(format!("{name}+").into_boxed_str());
            let row = PostIncrementRow {
                asm: (text, Box::leak(operands.into_boxed_slice())),
                encoding: (width, Box::leak(pieces.into_boxed_slice())),
                operand: usize::from(ari.at),
                bytes,
            };
            table.insert(key, row);
        };
        for &(name, text, operands) in crate::dagisel::asm::ASM.iter() {
            let Some(&(_, width, pieces)) = crate::dagisel::enc::encoding(name) else { continue };
            add(name, text, operands, width, pieces);
        }
        // `tst` of memory, which the overlay derives from `cmpi #0`
        for (name, row) in tst_memory_table().values() {
            add(name, row.asm.0, row.asm.1, row.encoding.0, row.encoding.1);
        }
        table
    })
}

fn post_increment_rows(name: &str) -> Option<&'static PostIncrementRow> {
    post_increment_table().get(name)
}

/// The post-increment form of an instruction with an `(An)` operand.
fn post_increment_form(opcode: &str) -> Option<(&'static str, &'static PostIncrementRow)> {
    post_increment_table().get_key_value(format!("{opcode}+").as_str()).map(|(name, row)| (*name, row))
}

/// The pin's own `move` forms with a post-incremented side: `MOV8jj` is
/// `move.b (An), (An)`, named destination then source, `o` for `(An)+`.
/// Returns the form that steps the destination (operand 0) or the source
/// (operand 1), and the access size.
fn move_post_increment(
    opcode: &str,
    operand: usize,
) -> Option<(&'static str, i64)> {
    let rest = opcode.strip_prefix("MOV")?;
    let (bits, modes) = rest.split_at(rest.len().checked_sub(2)?);
    let bytes = match bits {
        "8" => 1,
        "16" => 2,
        "32" => 4,
        _ => return None,
    };
    let mut modes: Vec<char> = modes.chars().collect();
    if modes[operand] != 'j' {
        return None;
    }
    modes[operand] = 'o';
    let name = format!("MOV{bits}{}{}", modes[0], modes[1]);
    crate::dagisel::asm::ASM.iter().find(|row| row.0 == name).map(|row| (row.0, bytes))
}

/// The block a branch names, by layout index (`.LBB<function>_<index>`).
fn branch_target(inst: &Inst) -> Option<usize> {
    let [Operand::Symbol(name, 0)] = &inst.operands[..] else { return None };
    name.strip_prefix(".LBB")?.rsplit('_').next()?.parse().ok()
}

/// The lines that branch to block `label`.
fn jumps_to(
    lines: &[crate::expand::Line],
    label: usize,
) -> impl Iterator<Item = usize> + '_ {
    lines
        .iter()
        .enumerate()
        .filter_map(
            move |(at, line)| match line {
                crate::expand::Line::Inst(inst) if branch_target(inst) == Some(label) => Some(at),
                _ => None,
            },
        )
}

/// An access through `(An)` followed by the `addq.l #size, An` that steps the
/// register past it is one access through `(An)+` (the increment sets no
/// condition codes, so moving it up changes none): `add.l (%a0), %d0 ...
/// addq.l #4, %a0` is `add.l (%a0)+, %d0`. The step may be past a forward
/// conditional branch, when every way from the access reaches it and nothing
/// else does: a loop whose select skips one move.
pub fn fold_post_increment(lines: Vec<crate::expand::Line>) -> Vec<crate::expand::Line> {
    let mut lines = lines;
    let mut at = 0;
    while at < lines.len() {
        // a `move` may step both of its pointers
        while fold_one(&mut lines, at) {}
        at += 1;
    }
    lines
}

/// Folds one step into the access at `at`; whether it did.
fn fold_one(
    lines: &mut Vec<crate::expand::Line>,
    at: usize,
) -> bool {
    use crate::expand::Line;
    let Line::Inst(access) = &lines[at] else { return false };
    let mut candidates: Vec<(&'static str, usize, i64)> = Vec::new();
    for operand in [1, 0] {
        if let Some((form, bytes)) = move_post_increment(access.opcode, operand) {
            candidates.push((form, operand, bytes));
        }
    }
    if let Some((form, row)) = post_increment_form(access.opcode) {
        candidates.push((form, row.operand, row.bytes));
    }
    for (form, operand, bytes) in candidates {
        let Some(&Operand::Reg(base)) = access.operands.get(operand) else { continue };
        let root = REGISTER_INFO.root(base);
        let mentions =
            |inst: &Inst| inst.operands.iter().any(|o| matches!(o, Operand::Reg(r) if REGISTER_INFO.root(*r) == root));
        let others = access
            .operands
            .iter()
            .enumerate()
            .any(|(n, o)| n != operand && matches!(o, Operand::Reg(r) if REGISTER_INFO.root(*r) == root));
        if others || (bytes == 1 && base == crate::reg::SP) {
            continue;
        }
        let mut step = None;
        // forward branches taken inside the span, whose labels are still ahead
        let mut pending: Vec<usize> = Vec::new();
        for (later, line) in lines.iter().enumerate().skip(at + 1) {
            let inst = match line {
                Line::Label(label) => {
                    // a way in from outside the span would reach the step
                    // without the access
                    if jumps_to(lines, *label).any(|from| from <= at || from >= later) {
                        break;
                    }
                    pending.retain(|one| one != label);
                    continue;
                }
                Line::Inst(inst) => inst,
            };
            let is_step = pending.is_empty()
                && matches!(inst.opcode, "ADDQ32a" | "ADD32ai")
                && matches!(
                    inst.operands[..],
                    [Operand::Reg(to), ..] if REGISTER_INFO.root(to) == root
                )
                && matches!(inst.operands.last(), Some(Operand::Imm(n)) if *n == bytes);
            if is_step {
                step = Some(later);
                break;
            }
            if is_branch_on_flags(inst.opcode)
                && let Some(label) = branch_target(inst)
                && !mentions(inst)
            {
                pending.push(label);
                continue;
            }
            if mentions(inst)
                || is_branch_on_flags(inst.opcode)
                || inst.opcode.starts_with("BRA")
                || inst.opcode.starts_with("JSR")
                || inst.opcode.starts_with("BSR")
                || inst.opcode.starts_with("CALL")
                || inst.opcode == "RTS"
            {
                break;
            }
        }
        if let Some(step) = step {
            lines.remove(step);
            if let Line::Inst(access) = &mut lines[at] {
                access.opcode = form;
            }
            return true;
        }
    }
    false
}

/// An instruction whose `dr` form takes a register of either kind as its
/// source commutes when its `dd` form (data registers only) does: `ADD32dr`
/// from `ADD32dd`'s `isCommutable`. The swap is checked against the register
/// classes by `commute_tied`.
///
/// DEVIATE from the pin, which selects `ADD32dr` for a value in an `xr32`
/// register (every phi is) and so never swaps its sources. Reason: swapping
/// is what lets the result take the register of the source that lives on, and
/// saves a move on a loop that carries a sum. Exit: never; a target choice,
/// off with [`crate::compile::Options::overlay`].
pub fn mark_commutable(function: &mut crate::mfunc::MFunction) {
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        if let llrm_spike_mf::Kind::Op(name) = inst.kind
            && let Some(stem) = name.strip_suffix("dr")
            && crate::mfunc::pin_commutable(&format!("{stem}dd"))
        {
            inst.commutable = true;
        }
    }
}
/// The second register of a `cmp.l` may be an address register (`cmpa.l`,
/// the same codes): a value read from memory only to be compared and moved
/// need not take a data register. A register nothing else restricts to data
/// registers gets the class of either; its other mentions (the pin's operand
/// classes) say.
///
/// DEVIATE from the pin, which has no `CMPA`. Reason: with the data
/// registers taken by the loop's counter and maximum, the loaded value
/// took callee-saved `d2` and a push and a pop (4 bytes); gcc uses `a1`.
/// Exit: never; a target choice, off with
/// [`crate::compile::Options::overlay`].
pub fn widen_compare_operands(function: &mut crate::mfunc::MFunction) {
    use crate::mfunc::{Class, Kind};
    let mut widen: Vec<u32> = Vec::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if let Kind::Op("CMP32dr") = inst.kind
            && let [_, llrm_spike_mf::Operand::Use(llrm_spike_mf::Reg::Virt(rhs))] = inst.operands[..]
            && function.classes[rhs as usize] == Class::Dr32
        {
            widen.push(rhs);
        }
    }
    for v in widen {
        let mut class = Some(Class::Xr32);
        'mentions: for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            for (at, operand) in inst.operands.iter().enumerate() {
                let (llrm_spike_mf::Operand::Def(llrm_spike_mf::Reg::Virt(r))
                | llrm_spike_mf::Operand::Use(llrm_spike_mf::Reg::Virt(r))) = operand
                else {
                    continue;
                };
                if *r != v {
                    continue;
                }
                match inst.kind {
                    Kind::Copy => {}
                    Kind::Op("CMP32dr") if at == 1 => {}
                    Kind::Op(name) => match crate::mfunc::operand_class(name, at) {
                        Some(wanted) => class = class.and_then(|have| have.meet(wanted)),
                        None => {
                            class = None;
                            break 'mentions;
                        }
                    },
                }
            }
        }
        if let Some(class) = class {
            function.classes[v as usize] = class;
        }
    }
}

/// Two masks of one register in a row are one: `and.l #255, %d1; and.l #1,
/// %d1` is `and.l #1, %d1` (the second leaves the same value and the same
/// codes). The pin's selection leaves the pair after `sgt` and a `zext`.
pub fn merge_masks(lines: Vec<crate::expand::Line>) -> Vec<crate::expand::Line> {
    use crate::expand::Line;
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines {
        if let (Some(Line::Inst(before)), Line::Inst(after)) = (out.last_mut(), &line)
            && before.opcode == "AND32di"
            && after.opcode == "AND32di"
            && let [Operand::Reg(d1), Operand::Reg(s1), Operand::Imm(m1)] = before.operands[..]
            && let [Operand::Reg(d2), Operand::Reg(s2), Operand::Imm(m2)] = after.operands[..]
            && d1 == s1
            && d2 == s2
            && d1 == d2
        {
            before.operands[2] = Operand::Imm(m1 & m2);
            continue;
        }
        out.push(line);
    }
    out
}

/// `move.l %a, %d; and.l #c, %d` with `c` in `moveq`'s range is `moveq #c,
/// %d; and.l %a, %d`: the same value and codes in 4 bytes, not 8. Likewise
/// `or` and `add` (an address register may be the source of an `add`).
/// `x & 1` of a copy is popcount's loop.
///
/// DEVIATE from the pin, whose patterns take the immediate form. Reason: bytes
/// and a clock (`and.l #1` is 6 bytes and 16 clocks, `moveq` and `and.l d,d`
/// 4 and 12). Exit: never; a target choice, off with
/// [`crate::compile::Options::overlay`].
pub fn constant_into_destination(lines: Vec<crate::expand::Line>) -> Vec<crate::expand::Line> {
    use crate::expand::Line;
    use crate::reg;
    let data = |register| {
        [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7]
            .contains(&crate::REGISTER_INFO.root(register))
    };
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines {
        if let (Some(Line::Inst(copy)), Line::Inst(op)) = (out.last(), &line)
            && copy.opcode == "MOV32rr"
            && let [Operand::Reg(dst), Operand::Reg(source)] = copy.operands[..]
            && let [Operand::Reg(d1), Operand::Reg(d2), Operand::Imm(value)] = op.operands[..]
            && d1 == dst
            && d2 == dst
            && data(dst)
            && (-128..=127).contains(&value)
            && let Some(register_form) = match (op.opcode, data(source)) {
                ("AND32di", true) => Some("AND32dd"),
                ("OR32di", true) => Some("OR32dd"),
                // 1..=8 is an `addq`, which is already 4 bytes
                ("ADD32di", true) if !(-8..=8).contains(&value) => Some("ADD32dd"),
                ("ADD32di", false) if !(-8..=8).contains(&value) => Some("ADD32dr"),
                _ => None,
            }
        {
            out.pop();
            out.push(Line::Inst(Inst { opcode: "MOVQ", operands: vec![Operand::Reg(dst), Operand::Imm(value)] }));
            out.push(Line::Inst(Inst {
                opcode: register_form,
                operands: vec![Operand::Reg(dst), Operand::Reg(dst), Operand::Reg(source)],
            }));
            continue;
        }
        out.push(line);
    }
    out
}

/// `cmp; scc %d; and.l #1, %d` is `moveq #0, %d; cmp; scc %d; neg.b %d`: the
/// byte `scc` leaves is 0 or 255, and `neg.b` makes it 0 or 1 over the zeros
/// `moveq` put in the rest. The `moveq` goes before the compare, which sets
/// the codes `scc` reads (`moveq` would clear them) and does not touch `%d`.
/// 4 bytes and 8 clocks for 6 and 16 (`and.l #imm, Dn` 16 clocks, `moveq` 4,
/// `neg.b Dn` 4: the 68000 manual's instruction timing tables), one
/// instruction more. The codes after differ from the `and`'s in `C`, which a
/// branch on the boolean (`ne`, `eq`) does not read; `drop_redundant_tests`
/// does not know `neg`, so a test after it stays.
///
/// DEVIATE from the pin, whose patterns mask. Exit: never; a target choice,
/// off with [`crate::compile::Options::overlay`].
pub fn boolean_by_negation(lines: Vec<crate::expand::Line>) -> Vec<crate::expand::Line> {
    use crate::expand::Line;
    let mut lines = lines;
    let mut at = 2;
    while at < lines.len() {
        let (Line::Inst(compare), Line::Inst(set), Line::Inst(mask)) = (&lines[at - 2], &lines[at - 1], &lines[at])
        else {
            at += 1;
            continue;
        };
        let mentions = |inst: &Inst, root| {
            inst.operands.iter().any(|o| matches!(o, Operand::Reg(r) if REGISTER_INFO.root(*r) == root))
        };
        if (compare.opcode.starts_with("CMP") || compare.opcode.starts_with("TST"))
            && set.opcode.starts_with("SETd8")
            && mask.opcode == "AND32di"
            && let [Operand::Reg(byte)] = set.operands[..]
            && let [Operand::Reg(to), Operand::Reg(from), Operand::Imm(1)] = mask.operands[..]
            && to == from
            && REGISTER_INFO.root(to) == REGISTER_INFO.root(byte)
            && !mentions(compare, REGISTER_INFO.root(to))
        {
            let negate = Inst { opcode: "NEG8d", operands: vec![Operand::Reg(byte), Operand::Reg(byte)] };
            let clear = Inst { opcode: "MOVQ", operands: vec![Operand::Reg(to), Operand::Imm(0)] };
            lines[at] = Line::Inst(negate);
            lines.insert(at - 2, Line::Inst(clear));
            at += 2;
            continue;
        }
        at += 1;
    }
    lines
}

struct TstRow {
    asm: AsmRow,
    encoding: EncodingRow,
}

/// `tst` of memory, derived from the pin's `cmpi #imm, ea` rows: the same
/// effective-address fields with the opcode `0100 1010` and the immediate's
/// words gone. Named `TST{size}{mode}` after `CMP{size}{mode}i`. The
/// pc-relative modes have no `tst`.
fn tst_memory_table() -> &'static std::collections::HashMap<&'static str, (&'static str, TstRow)> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<&'static str, (&'static str, TstRow)>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = std::collections::HashMap::new();
        for &(name, text, operands) in crate::dagisel::asm::ASM.iter() {
            let Some(rest) = name.strip_prefix("CMP") else { continue };
            let Some(rest) = rest.strip_suffix('i') else { continue };
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let mode = &rest[digits.len()..];
            let Ok(bits) = digits.parse::<u16>() else { continue };
            if mode.len() != 1 || mode == "d" || !text.starts_with("cmpi.") {
                continue;
            }
            let Some(memory) = operands.iter().find(|one| one.name != "imm") else { continue };
            if memory.method.starts_with("printPC") {
                continue;
            }
            let Some(&(_, width, pieces)) = crate::dagisel::enc::encoding(name) else { continue };
            let dropped: u16 = if bits == 32 { 32 } else { 16 };
            let moved = |piece: &Piece| -> Option<Piece> {
                let (lo, at_least) = match piece {
                    Piece::Const { lo, .. } | Piece::Operand { lo, .. } => (*lo, 16),
                };
                if lo >= at_least && lo < at_least + dropped {
                    return None;
                }
                let shifted = if lo >= at_least + dropped { lo - dropped } else { lo };
                Some(match *piece {
                    Piece::Const { width, value, .. } if lo == 8 && width == 8 && value == 12 => {
                        Piece::Const { lo: shifted, width, value: 0x4A }
                    }
                    Piece::Const { width, value, .. } => Piece::Const { lo: shifted, width, value },
                    Piece::Operand { width, at, from, enc, .. } => {
                        Piece::Operand { lo: shifted, width, at: at - 1, from, enc }
                    }
                })
            };
            let pieces: Vec<Piece> = pieces.iter().filter_map(moved).collect();
            let size = match bits {
                8 => "b",
                16 => "w",
                _ => "l",
            };
            let tst: &'static str = Box::leak(format!("TST{bits}{mode}").into_boxed_str());
            let text: &'static str = Box::leak(format!("tst.{size}\t$mem").into_boxed_str());
            let operands: &'static [AsmOperand] = Box::leak(
                vec![AsmOperand { name: "mem", at: memory.at - 1, method: memory.method }].into_boxed_slice(),
            );
            let row =
                TstRow { asm: (text, operands), encoding: (width - dropped, Box::leak(pieces.into_boxed_slice())) };
            table.insert(name, (tst, row));
        }
        // keyed by the name of the `cmpi` row; the `tst` rows by their own name
        let named: Vec<_> = table.values().map(|(tst, _)| *tst).collect();
        let _ = named;
        table
    })
}

/// The `tst` of memory that `cmpi.s #0, ea` is, and its rows.
fn tst_memory_form(cmpi: &str) -> Option<(&'static str, &'static TstRow)> {
    tst_memory_table().get(cmpi).map(|(name, row)| (*name, row))
}

fn tst_memory_rows(tst: &str) -> Option<&'static TstRow> {
    tst_memory_table().values().find(|(name, _)| *name == tst).map(|(_, row)| row)
}
