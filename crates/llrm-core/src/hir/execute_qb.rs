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

/// B$DDIM and B$RDIM: `(lower, upper)` per dimension, the last record's
/// first, then the element width, the rank and allocation flags, and the
/// descriptor, which gets fresh zeroed storage in ARRAY.INC's layout: the
/// data at +0, the rank at +8, the offset adjusted for the lower bounds at
/// +0Ah, the width at +0Ch and a (count, lower) record per dimension at +0Eh.
fn dimension(name: &str, arguments: &[Scalar]) -> Outcome<()> {
    let [pairs @ .., width, flags, Scalar::Address(descriptor)] = arguments else {
        return fail(format!("{name} has no descriptor"));
    };
    let (width, rank) = (width.whole()? as i64, (flags.whole()? & 0xff) as usize);
    if pairs.len() != 2 * rank {
        return fail(format!("{name} has {} bounds for rank {rank}", pairs.len()));
    }
    let mut records = Vec::new();
    for pair in pairs.chunks(2).rev() {
        records.push((pair[0].whole()? as i64, pair[1].whole()? as i64));
    }
    let at = descriptor.offset;
    let mut cells = descriptor.memory.borrow_mut();
    if name == "B$DDIM" && cells.pointers.contains_key(&(at, 4)) {
        return fail("Array already dimensioned");
    }
    let mut elements = 1;
    let mut lower_linear = None;
    for (lower, upper) in &records {
        if upper < lower {
            return fail("Subscript out of range");
        }
        elements *= upper - lower + 1;
        lower_linear = Some(lower_linear.map_or(*lower, |previous: i64| previous * (upper - lower + 1) + lower));
    }
    let end = usize::try_from(at + 14 + 4 * rank as i64).unwrap_or(usize::MAX);
    if end > cells.bytes.len() {
        return fail("array descriptor outside its object");
    }
    let data = memory(elements * width);
    let bias = lower_linear.unwrap_or(0) * width;
    cells.pointers.insert((at, 4), Address { memory: data.clone(), offset: 0, length: None, capacity: None });
    cells.pointers.insert((at + 10, 2), Address { memory: data, offset: -bias, length: None, capacity: None });
    let base = at as usize;
    cells.bytes[base + 8] = rank as u8;
    cells.bytes[base + 12..base + 14].copy_from_slice(&(width as u16).to_le_bytes());
    for (record, (lower, upper)) in records.iter().enumerate() {
        let field = base + 14 + 4 * record;
        cells.bytes[field..field + 2].copy_from_slice(&((upper - lower + 1) as u16).to_le_bytes());
        cells.bytes[field + 2..field + 4].copy_from_slice(&(*lower as u16).to_le_bytes());
    }
    Ok(())
}

/// B$ERAS: the descriptor no longer holds storage.
fn erase(argument: &Scalar) -> Outcome<()> {
    let Scalar::Address(descriptor) = argument else {
        return fail("B$ERAS argument is not a descriptor address");
    };
    let at = descriptor.offset;
    let mut cells = descriptor.memory.borrow_mut();
    cells.pointers.remove(&(at, 4));
    cells.pointers.remove(&(at + 10, 2));
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

/// `VAL`: blanks are skipped anywhere, and the number is the longest
/// prefix that reads as one, `&H` and `&O` included.
fn value(bytes: &[u8]) -> f64 {
    let text: String = bytes.iter().filter(|one| !matches!(one, b' ' | b'\t' | b'\n')).map(|one| *one as char).collect();
    let upper = text.to_ascii_uppercase();
    for (prefix, radix) in [("&H", 16), ("&O", 8), ("&", 8)] {
        if let Some(digits) = upper.strip_prefix(prefix) {
            let digits: String = digits.chars().take_while(|one| one.is_digit(radix)).collect();
            return i64::from_str_radix(&digits, radix).map_or(0.0, |one| one as f64);
        }
    }
    let mut end = 0;
    let mut best = 0.0;
    while end < upper.len() {
        end += 1;
        let candidate = upper[..end].replace('D', "E");
        if let Ok(parsed) = candidate.parse::<f64>() {
            best = parsed;
        } else if !candidate.ends_with(['E', '+', '-', '.']) {
            break;
        }
    }
    best
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
            "B$DDIM" | "B$RDIM" => {
                dimension(name, arguments)?;
                None
            }
            "B$ERAS" | "B$ERS1" => {
                erase(&arguments[0])?;
                None
            }
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
            "B$FVAL" => {
                let accumulator = memory(8);
                accumulator.borrow_mut().bytes.copy_from_slice(&value(&text(0)?).to_le_bytes());
                Some(Scalar::Address(Address { memory: accumulator, offset: 0, length: None, capacity: None }))
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

    #[test]
    fn val_reads_the_longest_number() {
        assert_eq!(value(b" -12 .5x"), -12.5);
        assert_eq!(value(b"+04"), 4.0);
        assert_eq!(value(b"1D3"), 1000.0);
        assert_eq!(value(b"&HFF"), 255.0);
        assert_eq!(value(b"abc"), 0.0);
    }
}
