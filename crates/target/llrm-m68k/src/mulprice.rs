//! Whether the shifts and the add of `x * c` beat the multiply on a cpu.
//!
//! The 68000 and 68010 have no 32-bit multiply: it is a call of `__mulsi3`
//! (`pea c; move.l x, -(%sp); jsr; addq.l #8, %sp`, 14 B and over 100 clocks).
//! The pieces cost
//! a copy and a shift for each term and an add or subtract; both are priced
//! from the encodings and the tables under the level's [`Price`]. From the
//! 68020 the multiply is one instruction and is kept, as before.
//!
//! DEVIATE: the pin's `M68kTargetLowering::decomposeMulByConstant` answers yes
//! for a multiply of 32 bits or fewer on every cpu, and for 64 bits from the
//! 68020. Reason: from the 68020 the multiply is one instruction that
//! takes memory operands; the pieces lost in context (+3760 B over 516 cells,
//! 112 of them worse; priced in isolation they tie). Owner: m68k. Exit:
//! 2026-11-14, the shared flow prices a multiply expansion against its pieces.

use llrm_codegen::backend::selectiondag::dag_combiner::MulPieces;

use crate::dagisel::hooks::Cpu;
use crate::timing::Price;
use crate::{mc, reg};

fn price_of(
    opcode: &'static str,
    operands: Vec<mc::Operand>,
    cpu: Cpu,
) -> Option<(usize, u32)> {
    let one = mc::Inst { opcode, operands };
    let bytes = crate::encode::encode(&one).ok()?.bytes.len();
    let clocks = crate::timing::cost(cpu, &mc::print(&one).ok()?, bytes as u32, false).ok()?.clocks;
    Some((bytes, clocks))
}

fn sum(parts: impl IntoIterator<Item = Option<(usize, u32)>>) -> Option<(usize, u32)> {
    let mut total = (0, 0);
    for part in parts {
        let (bytes, clocks) = part?;
        total = (total.0 + bytes, total.1 + clocks);
    }
    Some(total)
}

/// A copy of `d0` into `d1` shifted left by `amount` (`moveq` and a register
/// shift past 8).
fn shifted(
    amount: u32,
    cpu: Cpu,
) -> Option<(usize, u32)> {
    use mc::Operand::{Imm, Reg as R};
    let copy = price_of("MOV32rr", vec![R(reg::D1), R(reg::D0)], cpu);
    if amount == 0 {
        return copy;
    }
    if amount <= 8 {
        return sum([copy, price_of("SHL32di", vec![R(reg::D1), R(reg::D1), Imm(i64::from(amount))], cpu)]);
    }
    sum([
        copy,
        price_of("MOVQ", vec![R(reg::D2), Imm(i64::from(amount))], cpu),
        price_of("SHL32dd", vec![R(reg::D1), R(reg::D1), R(reg::D2)], cpu),
    ])
}

/// The multiply by the signed 32-bit constant `c`.
fn multiply(
    c: i64,
    cpu: Cpu,
) -> Option<(usize, u32)> {
    use mc::Operand::{Imm, Reg as R};
    let push = if crate::dagisel::pea::fits(c) {
        price_of(crate::dagisel::pea::NAME, vec![Imm(c)], cpu)
    } else {
        price_of("MOV32ei", vec![R(reg::SP), Imm(c)], cpu)
    };
    sum([
        push,
        price_of("MOV32ed", vec![R(reg::SP), R(reg::D0)], cpu),
        price_of("CALLb", vec![mc::Operand::Symbol("__mulsi3".to_owned(), 0)], cpu),
        Some(price_of_inst(&crate::expand::sp_adjust(8, true), cpu)?),
    ])
}

fn price_of_inst(
    inst: &mc::Inst,
    cpu: Cpu,
) -> Option<(usize, u32)> {
    price_of(inst.opcode, inst.operands.clone(), cpu)
}

/// Whether `pieces` of a multiply by `c` (a signed value of at most 32 bits)
/// beat the multiply under `price`; an instruction the tables cannot price
/// keeps the multiply.
pub fn pieces_beat_multiply(
    cpu: Cpu,
    price: Price,
    pieces: MulPieces,
    c: i64,
) -> bool {
    use mc::Operand::Reg as R;
    // `muls.l` is one instruction, and it takes its source from memory, which
    // the pieces cannot: priced in isolation they tie or win by 2 B and
    // lose in context (+1040 B over 516 cells)
    if cpu.at_least(Cpu::M68020) {
        return false;
    }
    let parts = sum([
        shifted(pieces.high, cpu),
        if pieces.low > 0 { shifted(pieces.low, cpu) } else { Some((0, 0)) },
        price_of(if pieces.add { "ADD32dd" } else { "SUB32dd" }, vec![R(reg::D1), R(reg::D1), R(reg::D0)], cpu),
        if pieces.negative { price_of("NEG32d", vec![R(reg::D1), R(reg::D1)], cpu) } else { Some((0, 0)) },
    ]);
    match (parts, multiply(c, cpu)) {
        (Some(parts), Some(multiply)) => price.beats(cpu, parts, multiply),
        _ => false,
    }
}
