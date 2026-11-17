// SPDX-License-Identifier: GPL-3.0-or-later
//! `bset`, `bclr` and `bchg #k,Dn`: a single-bit `or`, `and` and `xor`, on the
//! 68000 and 68010 (from the 68020 `or.l #imm,Dn` takes fewer clocks).
//!
//! # How it works
//! Three copies of the pin's `ADD32di` row (result, tied source, immediate)
//! with the encoding of the pin's `BSET32di`, `BCLR32di` and `BCHG32di`, whose
//! operand list has no result and so is never selected. [`select`] turns an
//! `i32` `or x, 2^k` into `bset`, `and x, ~2^k` into `bclr` and `xor x, 2^k`
//! into `bchg`. On the 68000 that is 12-14 clocks in 4 bytes against 16 in 6
//! for `or.l`/`and.l`/`eor.l` with an immediate.
//!
//! The codes are Z of the old bit only: [`crate::ccr::like_test`] has no row
//! for them, so no pass reads them as a test of the result.
//!
//! Translation: gcc's `bsetsi_internal`, `bclrsi_internal` and `bchgsi_internal`
//! patterns (`config/m68k/m68k.md`).
//!
//! DEVIATE M68K-BITOP: the pin's M68k selects none of the three
//! (`M68kInstrBits.td` defines them with no pattern). Reason: gcc 15 sets,
//! clears and flips single bits with them. Owner w8. Exit: 2026-11-24, or when
//! the pin selects them.

use std::sync::LazyLock;

use llrm_codegen::backend::selectiondag::SDValue as Value;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::instr::InstrDesc;
use llrm_dagisel::interp::Outcome;

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::instrinfo;

/// Code of the first row, above the `bfextu` row.
pub const FIRST: u16 = 0xFA00;

/// The operands: the result, the register it is tied to, the bit number.
const DESTINATION: u8 = 0;
const BIT: u8 = 2;

/// The rows in code order: the name, the mnemonic and the opmode of the pin's
/// immediate form (bits 6-8 of the first word).
const ROWS: [(&str, &str, u64); 3] = [("BSET32ti", "bset", 3), ("BCLR32ti", "bclr", 2), ("BCHG32ti", "bchg", 1)];

pub fn code_of(name: &str) -> Option<u16> {
    ROWS.iter().position(|row| row.0 == name).map(|index| FIRST + index as u16)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    let index = usize::try_from(code.checked_sub(u32::from(FIRST))?).ok()?;
    ROWS.get(index).map(|row| row.0)
}

static DESC: LazyLock<&'static InstrDesc> =
    LazyLock::new(|| instrinfo::desc(super::opcodes::code_of("ADD32di").expect("a pin row")).expect("a described row"));

/// The `ADD32di` description itself: a result, a source tied to it, an
/// immediate.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    name_of(u32::from(code)).map(|_| *DESC)
}

static ASM: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    ROWS.iter()
        .map(|&(name, mnemonic, _)| {
            let operands: &'static [AsmOperand] = Box::leak(Box::new([
                AsmOperand { name: "bitno", at: BIT, method: "printImmediate" },
                AsmOperand { name: "dst", at: DESTINATION, method: "printOperand" },
            ]));
            let text: &'static str = Box::leak(format!("{mnemonic}\t$bitno, $dst").into_boxed_str());
            (name, text, operands)
        })
        .collect()
});

pub fn bitop_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    ASM.iter().find(|row| row.0 == name)
}

const fn encoding_of(opmode: u64) -> [Piece; 6] {
    [
        Piece::Operand { lo: 0, width: 4, at: DESTINATION, from: 0, enc: Enc::Plain },
        Piece::Const { lo: 4, width: 2, value: 0 },
        Piece::Const { lo: 6, width: 3, value: opmode },
        Piece::Const { lo: 9, width: 7, value: 4 },
        Piece::Operand { lo: 16, width: 8, at: BIT, from: 0, enc: Enc::Plain },
        Piece::Const { lo: 24, width: 8, value: 0 },
    ]
}

static ENCODINGS: [[Piece; 6]; 3] = [encoding_of(ROWS[0].2), encoding_of(ROWS[1].2), encoding_of(ROWS[2].2)];

pub fn bitop_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    let index = ROWS.iter().position(|row| row.0 == name)?;
    Some((32, &ENCODINGS[index][..]))
}

fn operand(
    adapter: &DagisAdapter<'_>,
    node: u32,
    index: usize,
) -> SDValue {
    Dag::operand(adapter, node, index)
}

/// The row and bit number of `node`: an `i32` `or` or `xor` with a power of
/// two, or an `and` with the complement of one.
fn bit_of(
    adapter: &DagisAdapter<'_>,
    node: u32,
) -> Option<(u16, i64)> {
    let here = Value { node, res: 0 };
    if adapter.dag.value_type(here) != EVT::I32 {
        return None;
    }
    let row = match adapter.dag.opcode(here) {
        ISD::Or => 0,
        ISD::And => 1,
        ISD::Xor => 2,
        _ => return None,
    };
    let mask = adapter.dag.constant(adapter.dag.operand(here, 1))?;
    let mask = u32::try_from(mask & 0xFFFF_FFFF).ok()?;
    let single = if row == 1 { !mask } else { mask };
    // the pin selects a disjoint `or` as an add (`or_is_add`), and `addq` of 1 to 8 is 2 bytes and 8 clocks
    if row == 0
        && single <= 8
        && (Dag::has_disjoint_flag(adapter, node)
            || Dag::have_no_common_bits_set(adapter, operand(adapter, node, 0), operand(adapter, node, 1)))
    {
        return None;
    }
    single.is_power_of_two().then(|| (FIRST + row, i64::from(single.trailing_zeros())))
}

/// A single-bit `or`, `and` or `xor` as `bset`, `bclr` or `bchg`.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let (code, bit) = bit_of(adapter, node)?;
    let value = Dag::operand(adapter, node, 0);
    let vt = adapter.value_type(SDValue { node, res: 0 });
    let bit = adapter.target_constant(bit, vt);
    adapter.morph_node_to(node, code, &[vt], &[value, bit]);
    Some(Outcome::Selected)
}
