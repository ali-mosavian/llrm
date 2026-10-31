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
        _ => return tst_memory_rows(name).map(|row| row.asm),
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

// `divu.l`/`divs.l %src, %dq` (32 / 32 -> 32): the pin's rows leave the
// extension word's remainder register at 0, which makes the form that stores
// the remainder in `%d0`. Dr = Dq stores none.
const fn divide_long(signed: u64) -> [Piece; 9] {
    [
        field(0, 3, 2, 0),
        bits(3, 3, 0),
        bits(6, 10, 305),
        field(16, 3, 0, 0),
        bits(19, 7, 0),
        bits(26, 1, 0),
        bits(27, 1, signed),
        field(28, 3, 0, 0),
        bits(31, 1, 0),
    ]
}

static DIVL: [[Piece; 9]; 2] = [divide_long(0), divide_long(1)];

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
        "UDIVd32d32" => (32, &DIVL[0]),
        "SDIVd32d32" => (32, &DIVL[1]),
        _ => return tst_memory_rows(name).map(|row| row.encoding),
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

type AsmRow = (&'static str, &'static [AsmOperand]);
type EncodingRow = (u16, &'static [Piece]);

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
