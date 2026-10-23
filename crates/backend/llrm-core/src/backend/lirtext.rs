//! LIR as text, one stage at a time: what `ISEL_DUMP` prints after each
//! machine phase.
//!
//! # How it works
//! `lir_stage` prints a `=== stage` line, then for each body its name and
//! instruction count, each block by address, its phis (`v3 := phi ...`) and
//! its instructions as `dests := name sources`, with `; notes` for what a phase
//! attached (`inserted`, `parallel-copy N`, `spill reload`, `frame adjust`,
//! `requires`, `delivers`, `clobbers`).
//! Addresses are hex with at least four digits, and the format is short enough
//! to diff between stages.
//!
//! ```text
//! === PhiElimination
//!   f: 2 instructions
//!     block 0x0000
//!       0x0004  v5 := mov v2  ; inserted; parallel-copy 1
//! ```
//!
//! # Example
//! The first block of `main` after phi elimination:
//!
//! ```lir
//! === PhiElimination
//!   main: 18 instructions
//!     block 0x0000
//!       0x0001  [[bp-0x4]] := mov 0x30000  ; inserted
//!       ; ...
//!       0x0002  v2 := mov v3  ; inserted
//!       0x0003  v1 := mov -7  ; inserted
//!       0x0003  v5 := mov v1  ; inserted; parallel-copy 1
//!       0x0003  jmp   ; inserted
//! ```
//!
//! The `v5 := mov v1` is the copy that replaced the loop header's phi.
//!
//! From `ISEL_DUMP=1` of a nib program (a struct method with a `while` loop,
//! called from `main`) at `-m16 -O2`.
//!
//! # Used by
//! `assemble` after each machine phase when `ISEL_DUMP` is set; the register
//! allocator's tests print input and result with it.

use std::fmt::Write as _;

use llrm_lir::registers::RegId;

use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::pyrepr::Repr;

/// `=== stage`, then each body's instructions and operands, short enough to
/// diff.
pub fn lir_stage(
    stage: &str,
    bodies: &[(String, LirBody)],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== {stage}");
    for (name, one) in bodies {
        _lir_body(&mut out, name, one);
    }
    out
}

fn _lir_body(
    out: &mut String,
    name: &str,
    body: &LirBody,
) {
    let count: usize = body.blocks.iter().map(|block| block.insns.len()).sum();
    let _ = writeln!(out, "  {name}: {count} instructions");
    for block in &body.blocks {
        let _ = writeln!(out, "    block {}", hex6(block.at));
        for phi in &block.phis {
            let arms =
                phi.incoming.iter().map(|(at, value)| format!("{}:v{value}", hex6(*at))).collect::<Vec<_>>().join(" ");
            let _ = writeln!(out, "      v{} := phi {arms}", phi.result);
        }
        for one in &block.insns {
            let Some(what) = &one.what else {
                let covers = one.covers.map_or_else(|| "None".to_owned(), |(a, b)| format!("({a}, {b})"));
                let _ = writeln!(out, "      {}  (carried, {covers})", hex6(one.at));
                continue;
            };
            let dests = what.dests.iter().map(_operand).collect::<Vec<_>>().join(", ");
            let sources = what.sources.iter().map(_operand).collect::<Vec<_>>().join(", ");
            let said = if dests.is_empty() { String::new() } else { format!("{dests} := ") };
            let mut notes: Vec<String> = Vec::new();
            if one.covers.is_some_and(|(a, b)| a == b) {
                notes.push("inserted".to_owned());
            }
            if let Some(group) = one.group {
                notes.push(format!("parallel-copy {group}"));
            }
            if one.spill_reload {
                notes.push("spill reload".to_owned());
            }
            if one.frame_adjust {
                notes.push("frame adjust".to_owned());
            }
            if !one.requires.is_empty() {
                let parts = one
                    .requires
                    .iter()
                    .map(|(held, register)| format!("v{}@{}", held.value, _name_of(*register)))
                    .collect::<Vec<_>>();
                notes.push(format!("requires {}", parts.join(",")));
            }
            if !one.delivers.is_empty() {
                let parts = one
                    .delivers
                    .iter()
                    .map(|(held, register)| format!("v{}@{}", held.value, _name_of(*register)))
                    .collect::<Vec<_>>();
                notes.push(format!("delivers {}", parts.join(",")));
            }
            if !one.clobbers.is_empty() {
                let parts = one.clobbers.iter().map(|register| _name_of(*register)).collect::<Vec<_>>();
                notes.push(format!("clobbers {}", parts.join(",")));
            }
            let annotation = if notes.is_empty() { String::new() } else { format!("  ; {}", notes.join("; ")) };
            let name = what.name.as_deref().map_or_else(|| "None".to_owned(), str::to_owned);
            let line = format!("      {}  {said}{name} {sources}{annotation}", hex6(one.at));
            let _ = writeln!(out, "{}", line.trim_end());
        }
    }
}

fn _name_of(register: RegId) -> String {
    format!("{register:?}").to_uppercase()
}

fn _operand(one: &Loc) -> String {
    match one {
        Loc::Reg(reg) => _name_of(reg.register),
        Loc::Held(held) => format!("v{}", held.value),
        Loc::Imm(imm) => {
            if imm.value >= 0 {
                format!("{:#x}", imm.value)
            } else {
                imm.value.to_string()
            }
        }
        Loc::Mem(mem) => {
            let addr = mem.addr.map_or_else(|| "None".to_owned(), |addr| addr.repr());
            let index = mem.index.map_or_else(String::new, |held| {
                let placed =
                    if mem.index_through == RegId::None { "unplaced".to_owned() } else { _name_of(mem.index_through) };
                format!(" + v{}*{}@{placed}", held.value, mem.scale)
            });
            match mem.base {
                None => format!("[{addr}{index}]"),
                Some(base) => {
                    let placed = if mem.through == RegId::None { "unplaced".to_owned() } else { _name_of(mem.through) };
                    format!("[{addr} v{}@{placed}{index}]", base.value)
                }
            }
        }
        other => ir_repr(other),
    }
}

fn ir_repr(one: &Loc) -> String {
    one.repr()
}

/// `value` in hex, at least four digits.
fn hex6(value: i64) -> String {
    if value < 0 { format!("-{:#05x}", value.unsigned_abs()) } else { format!("{value:#06x}") }
}
