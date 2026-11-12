//! The data registers whose value an executed trace has fixed, so a shift by a
//! register can be priced at its real count.
//!
//! The 68000 and 68010 charge `2n` clocks for a shift of `n` places, `n` taken
//! from the count register modulo 64. A trace is the executed instruction
//! sequence, so a register set by `moveq` or `move.l #imm` and not written
//! since still holds that value. Any instruction this module cannot show leaves
//! a register alone forgets it: a wrong "known" would price a shift too low
//! without saying so.

use super::{is_shift, split_operands};

/// Bases that write no data register named in their operands.
const NO_WRITE: [&str; 5] = ["cmp", "cmpa", "cmpi", "tst", "btst"];

/// Bases whose first operand is a pure source.
const SOURCE_FIRST: [&str; 10] = ["move", "movea", "add", "adda", "sub", "suba", "and", "or", "eor", "mulu"];

/// Bases that run code this trace does not see or reload several registers.
const CLOBBERS: [&str; 7] = ["jsr", "bsr", "rts", "rte", "trap", "movem", "link"];

#[derive(Clone, Copy, Debug, Default)]
pub struct DataRegisters {
    known: [Option<u32>; 8],
}

impl DataRegisters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget everything: a gap in the trace, a call.
    pub fn forget_all(&mut self) {
        self.known = [None; 8];
    }

    /// The data register that holds the places of the shift `text`, when it
    /// shifts by a register.
    pub fn count_register(text: &str) -> Option<usize> {
        let (base, _, operands) = parts(text);
        if !is_shift(base) || operands.len() != 2 {
            return None;
        }
        data_register(operands[0])
    }

    /// The places the shift `text` moves, if it shifts by a register whose
    /// value is known.
    pub fn places_of(
        &self,
        text: &str,
    ) -> Option<u32> {
        self.known[Self::count_register(text)?].map(|value| value % 64)
    }

    /// Account for the registers `text` writes; call after it executed.
    pub fn execute(
        &mut self,
        text: &str,
    ) {
        let (base, _, operands) = parts(text);
        if CLOBBERS.contains(&base) {
            self.forget_all();
            return;
        }
        if NO_WRITE.contains(&base) {
            return;
        }
        let immediate = |operand: &str| {
            let digits = operand.strip_prefix('#')?;
            let (sign, digits) = digits.strip_prefix('-').map_or((1i64, digits), |rest| (-1, rest));
            let value = match digits.strip_prefix("0x") {
                Some(hex) => i64::from_str_radix(hex, 16).ok()?,
                None => digits.parse::<i64>().ok()?,
            };
            Some((sign * value) as u32)
        };
        match (base, operands.as_slice()) {
            ("moveq" | "move", [value, target]) if base == "moveq" || is_long(text) => {
                if let (Some(value), Some(register)) = (immediate(value), data_register(target)) {
                    self.known[register] = Some(value);
                    return;
                }
            }
            ("clr", [target]) if is_long(text) => {
                if let Some(register) = data_register(target) {
                    self.known[register] = Some(0);
                    return;
                }
            }
            _ => {}
        }
        for (index, operand) in operands.iter().enumerate() {
            if index == 0 && operands.len() == 2 && SOURCE_FIRST.contains(&base) {
                continue;
            }
            // `%d1:%d2` is a register pair
            for register in operand.split(':') {
                if let Some(register) = data_register(register) {
                    self.known[register] = None;
                }
            }
        }
    }
}

fn parts(text: &str) -> (&str, Option<&str>, Vec<&str>) {
    let text = text.trim();
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let (base, suffix) = match name.split_once('.') {
        Some((base, suffix)) => (base, Some(suffix)),
        None => (name, None),
    };
    (base, suffix, split_operands(rest))
}

fn is_long(text: &str) -> bool {
    text.trim().split_whitespace().next().is_some_and(|name| name.ends_with(".l"))
}

/// `%d3` is register 3; `(%a0,%d3.l)` and every other form are not a direct
/// data register.
fn data_register(operand: &str) -> Option<usize> {
    let number = operand.trim().strip_prefix("%d")?;
    if number.len() == 1 { number.parse().ok().filter(|n| *n < 8) } else { None }
}
