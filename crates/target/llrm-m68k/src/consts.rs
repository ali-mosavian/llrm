//! `move.l #65535, %d0` is `moveq #0, %d0; not.w %d0`: 4 bytes for 6 and 8
//! clocks for 12 on the 68000. `move.l #196608, %d0` is `moveq #3, %d0; swap
//! %d0`.
//!
//! # How it works
//! [`materialize_constants`] runs after allocation. For a long constant
//! loaded into a data register it looks up the cheapest `moveq` followed by
//! one register operation ([`steps`]: `not.b`, `not.w`, `neg`, `swap`,
//! shifts and rotates by 1 to 8) that makes the value, priced on the CPU by
//! encoding and [`crate::timing::cost`]: clocks then bytes, bytes then clocks
//! at the size level. It is used when it beats `move.l #imm`. The table of what
//! each `moveq` and step makes is built once per CPU and level, by running the
//! steps' own semantics over the 256 `moveq` values. A step the timing tables
//! do not price is left out, never counted as free.
//!
//! Only where no instruction reads the codes after the load
//! ([`crate::cmpelim::flags_read_after`]): `not.w` and `lsl.l` set N and Z of
//! their own width and carry.
//!
//! DEVIATE: the pin's `ExpandMOVI` knows `moveq` and `moveq; not.b` only, and
//! tests the value by range. gcc's `m68k_const_method` picks among a fixed
//! list; `RISCVMatInt::generateInstSeq` and `AArch64_IMM::expandMOVImm` are the
//! pin's priced searches over a mnemonic set. Reason: best of gcc and clang is
//! the goal. One step after the `moveq` is the cap: two cost 6 bytes, as
//! `move.l` does. Exit: none. Off for `Options::PINNED`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::cmpelim::flags_read_after;
use crate::dagisel::hooks::Cpu;
use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, mc, reg};

/// A register operation on a data register: its row, the immediate count of
/// a shift or rotate, and what it makes of the register.
struct Step {
    row: &'static str,
    count: Option<i64>,
    apply: fn(u32, u32) -> u32,
}

fn low_byte(
    x: u32,
    byte: u32,
) -> u32 {
    (x & !0xFF) | (byte & 0xFF)
}

fn low_word(
    x: u32,
    word: u32,
) -> u32 {
    (x & !0xFFFF) | (word & 0xFFFF)
}

/// Every step the search may use; the shifts are expanded by count in
/// [`steps`]. `not.l` and `asr.l` make another `moveq` value, so they are not
/// here.
fn steps() -> Vec<Step> {
    let mut all = vec![
        Step { row: "NOT8d", count: None, apply: |x, _| low_byte(x, !x) },
        Step { row: "NOT16d", count: None, apply: |x, _| low_word(x, !x) },
        Step { row: "NEG8d", count: None, apply: |x, _| low_byte(x, (x & 0xFF).wrapping_neg()) },
        Step { row: "NEG16d", count: None, apply: |x, _| low_word(x, (x & 0xFFFF).wrapping_neg()) },
        Step { row: "NEG32d", count: None, apply: |x, _| x.wrapping_neg() },
        Step { row: "SWAP", count: None, apply: |x, _| x.rotate_left(16) },
    ];
    let shifts: [(&str, fn(u32, u32) -> u32); 4] = [
        ("SHL32di", |x, k| x << k),
        ("LSR32di", |x, k| x >> k),
        ("ROL32di", |x, k| x.rotate_left(k)),
        ("ROR32di", |x, k| x.rotate_right(k)),
    ];
    for (row, apply) in shifts {
        for count in 1..=8 {
            all.push(Step { row, count: Some(count), apply });
        }
    }
    all
}

/// What one instruction costs on `cpu`: clocks and bytes; `None` where the
/// tables or the encoder do not know it.
fn price(
    opcode: &'static str,
    operands: Vec<mc::Operand>,
    cpu: Cpu,
) -> Option<(u32, u32)> {
    let one = mc::Inst { opcode, operands };
    let bytes = crate::encode::encode(&one).ok()?.bytes.len() as u32;
    let clocks = crate::timing::cost(cpu, &mc::print(&one).ok()?, bytes, false).ok()?.clocks;
    Some((clocks, bytes))
}

/// The cheapest `moveq` and step per value, and `move.l #imm` for comparison.
struct Table {
    made: HashMap<u32, (i64, usize)>,
    steps: Vec<Step>,
}

/// The order a level prefers: `(clocks, bytes)`, or `(bytes, clocks)` for size.
fn key(
    cost: (u32, u32),
    size: bool,
) -> (u32, u32) {
    if size { (cost.1, cost.0) } else { cost }
}

fn build(
    cpu: Cpu,
    size: bool,
) -> Option<Table> {
    use mc::Operand::{Imm, Reg as R};
    let moveq = price("MOVQ", vec![R(reg::D0), Imm(1)], cpu);
    let long = price("MOV32ri", vec![R(reg::D0), Imm(0x1234_5678)], cpu);
    let (moveq, long) = (moveq?, long?);
    let steps = steps();
    let mut costs = Vec::new();
    for step in &steps {
        let mut operands = vec![R(reg::D0), R(reg::D0)];
        if let Some(count) = step.count {
            operands.push(Imm(count));
        }
        let cost = price(step.row, operands, cpu);
        costs.push(cost);
    }
    let mut made: HashMap<u32, ((u32, u32), (i64, usize))> = HashMap::new();
    for m in -128..=127i64 {
        for (at, step) in steps.iter().enumerate() {
            let Some(cost) = costs[at] else { continue };
            let total = (moveq.0 + cost.0, moveq.1 + cost.1);
            let value = (step.apply)(m as i32 as u32, step.count.unwrap_or(0) as u32);
            let entry = made.entry(value).or_insert((total, (m, at)));
            if key(total, size) < key(entry.0, size) {
                *entry = (total, (m, at));
            }
        }
    }
    let made = made
        .into_iter()
        .filter(|(value, (cost, _))| !(-128..=127).contains(&(*value as i32)) && key(*cost, size) < key(long, size))
        .map(|(value, (_, how))| (value, how))
        .collect();
    Some(Table { made, steps })
}

fn table(
    cpu: Cpu,
    size: bool,
) -> Option<std::sync::Arc<Table>> {
    static TABLES: OnceLock<Mutex<HashMap<(bool, bool), Option<std::sync::Arc<Table>>>>> = OnceLock::new();
    let mut tables = TABLES.get_or_init(Default::default).lock().unwrap();
    tables.entry((cpu == Cpu::M68020, size)).or_insert_with(|| build(cpu, size).map(std::sync::Arc::new)).clone()
}

/// The instructions that make `value` in a data register when that beats
/// `move.l #value`: the `moveq` and its step.
pub fn sequence(
    value: u32,
    cpu: Cpu,
    size: bool,
) -> Option<(i64, &'static str, Option<i64>)> {
    let table = table(cpu, size)?;
    let &(m, at) = table.made.get(&value)?;
    let step = &table.steps[at];
    Some((m, step.row, step.count))
}

/// Every value the table makes without `move.l`, with its `moveq` value,
/// step row and count: what the tests check against the hardware.
pub fn made(
    cpu: Cpu,
    size: bool,
) -> Vec<(u32, i64, &'static str, Option<i64>)> {
    let Some(table) = table(cpu, size) else { return Vec::new() };
    let mut all: Vec<_> =
        table.made.iter().map(|(&value, &(m, at))| (value, m, table.steps[at].row, table.steps[at].count)).collect();
    all.sort();
    all
}

pub fn materialize_constants(
    function: &mut MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    size: bool,
) {
    let data = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let here = at;
            at += 1;
            let Kind::Op("MOVI32ri") = block.insts[here].kind else { continue };
            let [Operand::Def(def), Operand::Imm(value)] = block.insts[here].operands[..] else { continue };
            let root = match def {
                Reg::Virt(v) => REGISTER_INFO.root(assignment.0[v as usize]),
                Reg::Phys(p) => REGISTER_INFO.root(p),
            };
            let value = value as u32;
            if !data.contains(&root) || (-128..=127).contains(&(value as i32)) {
                continue;
            }
            if flags_read_after(&block.insts, here) != Some(0) {
                continue;
            }
            let Some((m, row, count)) = sequence(value, cpu, size) else { continue };
            let Some(second) = local::code_of(row).and_then(local::desc) else { continue };
            let mut operands = vec![Operand::Def(def), Operand::Use(def)];
            if let Some(count) = count {
                operands.push(Operand::Imm(count));
            }
            block.insts[here].operands[1] = Operand::Imm(m);
            block.insts.insert(here + 1, op_inst(row, second, operands));
            at += 1;
        }
    }
}
