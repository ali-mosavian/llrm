//! LIR as text, one stage at a time: what `ISEL_DUMP` prints after each
//! machine phase.

use std::fmt::Write as _;

use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::pyrepr::Repr;

/// `=== stage`, then each body's instructions and operands, short enough to diff.
pub fn lir_stage(stage: &str, bodies: &[(String, LirBody)]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== {stage}");
    for (name, one) in bodies {
        _lir_body(&mut out, name, one);
    }
    out
}

fn _lir_body(out: &mut String, name: &str, body: &LirBody) {
    let count: usize = body.blocks.iter().map(|block| block.insns.len()).sum();
    let _ = writeln!(out, "  {name}: {count} instructions");
    for block in &body.blocks {
        let _ = writeln!(out, "    block {}", hex6(block.at));
        for phi in &block.phis {
            let arms = phi.incoming.iter().map(|(at, value)| format!("{}:v{value}", hex6(*at))).collect::<Vec<_>>().join(" ");
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
                let parts = one.requires.iter().map(|(held, register)| format!("v{}@{}", held.value, _name_of(*register))).collect::<Vec<_>>();
                notes.push(format!("requires {}", parts.join(",")));
            }
            if !one.delivers.is_empty() {
                let parts = one.delivers.iter().map(|(held, register)| format!("v{}@{}", held.value, _name_of(*register))).collect::<Vec<_>>();
                notes.push(format!("delivers {}", parts.join(",")));
            }
            if !one.clobbers.is_empty() {
                let parts = one.clobbers.iter().map(|register| _name_of(*register)).collect::<Vec<_>>();
                notes.push(format!("clobbers {}", parts.join(",")));
            }
            let annotation = if notes.is_empty() { String::new() } else { format!("  ; {}", notes.join("; ")) };
            let name = what.name.clone().unwrap_or_else(|| "None".to_owned());
            let line = format!("      {}  {said}{name} {sources}{annotation}", hex6(one.at));
            let _ = writeln!(out, "{}", line.trim_end());
        }
    }
}

fn _name_of(register: iced_x86::Register) -> String {
    format!("{register:?}").to_uppercase()
}

fn _operand(one: &Loc) -> String {
    match one {
        Loc::Reg(reg) => _name_of(reg.register),
        Loc::Held(held) => format!("v{}", held.value),
        Loc::Imm(imm) => {
            if imm.value >= 0 { format!("{:#x}", imm.value) } else { imm.value.to_string() }
        }
        Loc::Mem(mem) => {
            let addr = mem.addr.map_or_else(|| "None".to_owned(), |addr| addr.repr());
            match mem.base {
                None => format!("[{addr}]"),
                Some(base) => {
                    let placed = if mem.through == iced_x86::Register::None { "unplaced".to_owned() } else { _name_of(mem.through) };
                    format!("[{addr} v{}@{placed}]", base.value)
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
