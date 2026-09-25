//! The QB runtime's string, `STR$` and `PRINT` routines, so the reference
//! executor can run QB HIR.
//!
//! A string argument is the near address of a four-byte descriptor. A string
//! the runtime makes is `[length][data pointer]`, the QB 4.5 layout, which
//! the frontend's QB 4.5 literals also use. A VBDOS literal's descriptor
//! instead bridges to a far descriptor in its payload, which points at
//! `[length][bytes]`.
//!
//! Number text follows `tools/qbprint.py`: an integer has a sign or a space,
//! and a DOUBLE has 15 significant digits and no zero before the point, as
//! measured on VBDOS. SINGLE's 7 digits and the change to exponent form
//! (`E` for SINGLE, `D` for DOUBLE) follow `%g` and are not measured.

use std::cell::RefCell;
use std::rc::Rc;

use super::{fail, memory, Address, Cells, Machine, Outcome, Scalar};
use crate::support::hash::HashMap;

/// PRINT's comma advances to the next zone of this many columns.
const ZONE: usize = 14;

fn word(cells: &Cells, at: i64) -> Outcome<usize> {
    let at = usize::try_from(at).map_err(|_| super::ExecutionError("negative string address".into()))?;
    match cells.bytes.get(at..at + 2) {
        Some(bytes) => Ok(u16::from_le_bytes([bytes[0], bytes[1]]) as usize),
        None => fail("string descriptor outside its object"),
    }
}

fn bytes_at(address: &Address, length: usize) -> Outcome<Vec<u8>> {
    let cells = address.memory.borrow();
    let start = address.offset as usize;
    match cells.bytes.get(start..start + length) {
        Some(bytes) => Ok(bytes.to_vec()),
        None => fail("string data outside its object"),
    }
}

/// The bytes of the string whose descriptor `argument` addresses.
fn string(argument: &Scalar) -> Outcome<Vec<u8>> {
    let Scalar::Address(descriptor) = argument else {
        return fail("string argument is not a descriptor address");
    };
    let cells = descriptor.memory.borrow();
    if let Some(bridge) = cells.pointers.get(&(descriptor.offset, 2)) {
        // A VBDOS literal: bridge -> far descriptor -> [length][bytes].
        let far = bridge.memory.borrow();
        let Some(data) = far.pointers.get(&(bridge.offset, 2)) else {
            return fail("VBDOS literal descriptor has no data");
        };
        let length = word(&data.memory.borrow(), data.offset)?;
        let bytes = Address { offset: data.offset + 2, ..data.clone() };
        return bytes_at(&bytes, length);
    }
    let length = word(&cells, descriptor.offset)?;
    if length == 0 {
        return Ok(Vec::new());
    }
    match cells.pointers.get(&(descriptor.offset + 2, 2)) {
        Some(data) => bytes_at(data, length),
        None => fail("string descriptor has a length but no data"),
    }
}

/// Writes `bytes` as the string of the descriptor at `address`.
fn assign(address: &Address, bytes: &[u8]) -> Outcome<()> {
    let data = Address {
        memory: Rc::new(RefCell::new(Cells { bytes: bytes.to_vec(), pointers: HashMap::default(), dead: false })),
        offset: 0,
        length: None,
        capacity: None,
    };
    let mut cells = address.memory.borrow_mut();
    let at = address.offset;
    let Some(slot) = usize::try_from(at).ok().and_then(|at| cells.bytes.get_mut(at..at + 2)) else {
        return fail("string descriptor outside its object");
    };
    slot.copy_from_slice(&(bytes.len() as u16).to_le_bytes());
    cells.pointers.retain(|(offset, width), _| !(at < offset + width && *offset < at + 4));
    cells.pointers.insert((at + 2, 2), data);
    Ok(())
}

/// A new temporary string, as the runtime's string functions return one.
fn temporary(bytes: &[u8]) -> Outcome<Scalar> {
    let descriptor = Address { memory: memory(4), offset: 0, length: None, capacity: None };
    assign(&descriptor, bytes)?;
    Ok(Scalar::Address(descriptor))
}

fn count(argument: &Scalar) -> Outcome<usize> {
    let value = argument.whole()?;
    usize::try_from(value).map_err(|_| super::ExecutionError(format!("Illegal function call: {value}")))
}

/// `%.{digits}g` with BASIC's spelling: no zero before the point, and `E`
/// or `D` exponents with a sign and at least two digits.
fn float_text(value: f64, digits: usize, exponent: char) -> String {
    let text = format!("{:.*e}", digits - 1, value);
    let (mantissa, power) = text.split_once('e').expect("exponent form");
    let power: i32 = power.parse().expect("an exponent");
    let fixed = if power < -4 || power >= digits as i32 {
        let mantissa = trim_fraction(mantissa);
        let sign = if power < 0 { '-' } else { '+' };
        format!("{mantissa}{exponent}{sign}{:02}", power.abs())
    } else {
        let decimals = (digits as i32 - 1 - power).max(0) as usize;
        trim_fraction(&format!("{value:.decimals$}"))
    };
    let (sign, magnitude) = match fixed.strip_prefix('-') {
        Some(magnitude) => ("-", magnitude.to_owned()),
        None => (" ", fixed),
    };
    let magnitude = magnitude.strip_prefix("0.").map_or(magnitude.clone(), |fraction| format!(".{fraction}"));
    format!("{sign}{magnitude}")
}

fn trim_fraction(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text.to_owned()
    }
}

/// What `STR$` makes of a number: a sign or a space, then the digits.
fn number_text(name: &str, value: &Scalar) -> Outcome<String> {
    Ok(match &name[name.len() - 2..] {
        "I2" | "I4" => {
            let value = value.whole()?;
            format!("{}{}", if value < 0 { "" } else { " " }, value)
        }
        "R4" => float_text(value.float()? as f32 as f64, 7, 'E'),
        "R8" => float_text(value.float()?, 15, 'D'),
        _ => return fail(format!("{name}: no number type")),
    })
}

impl Machine<'_> {
    /// `Some(result)` when `name` is a modelled QB runtime routine.
    pub(super) fn qb_runtime(&mut self, name: &str, arguments: &[Scalar]) -> Outcome<Option<Option<Scalar>>> {
        let text = |index: usize| string(&arguments[index]);
        let result = match name {
            "B$SASS" => {
                let Scalar::Address(destination) = &arguments[1] else {
                    return fail("B$SASS destination is not a descriptor address");
                };
                assign(destination, &text(0)?)?;
                None
            }
            "B$STDL" => None,
            "B$CEND" => {
                self.ended = true;
                return fail("END");
            }
            "B$SCPF" => Some(temporary(&text(0)?)?),
            "B$SCAT" => Some(temporary(&[text(0)?, text(1)?].concat())?),
            "B$FLEN" => Some(Scalar::Int(text(0)?.len() as i128)),
            "B$FASC" => match text(0)?.first() {
                Some(byte) => Some(Scalar::Int(i128::from(*byte))),
                None => return self.panic("Illegal function call"),
            },
            "B$FCHR" => Some(temporary(&[u8::try_from(arguments[0].whole()?)
                .map_err(|_| super::ExecutionError("Illegal function call".into()))?])?),
            "B$LEFT" => {
                let bytes = text(0)?;
                Some(temporary(&bytes[..count(&arguments[1])?.min(bytes.len())])?)
            }
            "B$RGHT" => {
                let bytes = text(0)?;
                Some(temporary(&bytes[bytes.len() - count(&arguments[1])?.min(bytes.len())..])?)
            }
            "B$FMID" => {
                let bytes = text(0)?;
                let start = count(&arguments[1])?;
                if start == 0 {
                    return self.panic("Illegal function call");
                }
                let from = (start - 1).min(bytes.len());
                let to = from + count(&arguments[2])?.min(bytes.len() - from);
                Some(temporary(&bytes[from..to])?)
            }
            "B$INS2" | "B$INS3" => {
                let (start, haystack, needle) = if name == "B$INS2" {
                    (1, text(0)?, text(1)?)
                } else {
                    (count(&arguments[0])?, text(1)?, text(2)?)
                };
                if start == 0 {
                    return self.panic("Illegal function call");
                }
                let found = if start > haystack.len() {
                    0
                } else if needle.is_empty() {
                    start
                } else {
                    haystack[start - 1..]
                        .windows(needle.len())
                        .position(|window| window == needle.as_slice())
                        .map_or(0, |at| at + start)
                };
                Some(Scalar::Int(found as i128))
            }
            "B$LTRM" => {
                let bytes = text(0)?;
                let from = bytes.iter().position(|one| *one != b' ').unwrap_or(bytes.len());
                Some(temporary(&bytes[from..])?)
            }
            "B$RTRM" => {
                let bytes = text(0)?;
                let to = bytes.iter().rposition(|one| *one != b' ').map_or(0, |at| at + 1);
                Some(temporary(&bytes[..to])?)
            }
            "B$UCAS" => Some(temporary(&text(0)?.to_ascii_uppercase())?),
            "B$LCAS" => Some(temporary(&text(0)?.to_ascii_lowercase())?),
            "B$SPAC" => Some(temporary(&vec![b' '; count(&arguments[0])?])?),
            "B$STRI" => {
                let byte = u8::try_from(arguments[1].whole()?)
                    .map_err(|_| super::ExecutionError("Illegal function call".into()))?;
                Some(temporary(&vec![byte; count(&arguments[0])?])?)
            }
            "B$STRS" => {
                let Some(byte) = text(1)?.first().copied() else {
                    return self.panic("Illegal function call");
                };
                Some(temporary(&vec![byte; count(&arguments[0])?])?)
            }
            "B$FHEX" | "B$FOCT" => {
                let value = arguments[0].whole()? as i32 as u32;
                let digits = if name == "B$FHEX" { format!("{value:X}") } else { format!("{value:o}") };
                Some(temporary(digits.as_bytes())?)
            }
            "B$STI2" | "B$STI4" | "B$STR4" | "B$STR8" => {
                Some(temporary(number_text(name, &arguments[0])?.as_bytes())?)
            }
            "B$SCMP" => Some(Scalar::Int(text(0)?.cmp(&text(1)?) as i128)),
            "B$PEOS" => None,
            "B$FTAB" => {
                let column = count(&arguments[0])?.max(1) - 1;
                if column < self.column {
                    self.qb_print("\n");
                }
                let missing = column.saturating_sub(self.column);
                self.qb_print(&" ".repeat(missing));
                None
            }
            _ if name.len() == 6 && name.starts_with("B$P") => {
                let (term, kind) = (name.as_bytes()[3], &name[4..]);
                let item = match kind {
                    "SD" => match &arguments[0] {
                        // A bare PRINT passes the null descriptor.
                        Scalar::Int(0) => String::new(),
                        _ => super::cp437(&text(0)?),
                    },
                    "I2" | "I4" | "R4" | "R8" => number_text(name, &arguments[0])? + " ",
                    _ => return Ok(None),
                };
                self.qb_print(&item);
                match term {
                    b'E' => self.qb_print("\n"),
                    b'C' => {
                        let next = (self.column / ZONE + 1) * ZONE;
                        self.qb_print(&" ".repeat(next - self.column));
                    }
                    b'S' => {}
                    _ => return Ok(None),
                }
                None
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// The string operations' comparison of two descriptors.
    pub(super) fn qb_string_order(left: &Scalar, right: &Scalar) -> Outcome<std::cmp::Ordering> {
        Ok(string(left)?.cmp(&string(right)?))
    }

    fn qb_print(&mut self, text: &str) {
        for character in text.chars() {
            self.column = if character == '\n' { 0 } else { self.column + 1 };
        }
        self.output.push_str(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_as_qbprint_measured_them() {
        assert_eq!(number_text("B$STI2", &Scalar::Int(5)).unwrap(), " 5");
        assert_eq!(number_text("B$STI4", &Scalar::Int(-70000)).unwrap(), "-70000");
        // tools/qbprint.py: 15 digits, no leading zero, no point when whole.
        assert_eq!(number_text("B$STR8", &Scalar::Float(0.999984741210938)).unwrap(), " .999984741210938");
        assert_eq!(number_text("B$STR8", &Scalar::Float(2.0)).unwrap(), " 2");
        assert_eq!(number_text("B$STR8", &Scalar::Float(-0.5)).unwrap(), "-.5");
        assert_eq!(number_text("B$STR4", &Scalar::Float(1e7)).unwrap(), " 1E+07");
        assert_eq!(number_text("B$STR8", &Scalar::Float(1e16)).unwrap(), " 1D+16");
    }
}
