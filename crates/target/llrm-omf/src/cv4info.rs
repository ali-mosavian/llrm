//! The CodeView 4 `cv4` writes and Microsoft's C7-era tools write, read back from an OMF object: its procedures
//! with their parameters and locals, its data and its type names, as the text a test compares. The reader is
//! `cvinfo`'s counterpart for the dialect BASIC's compilers do not write.

use std::rc::Rc;

use crate::cvinfo;
use crate::omf::Record;

/// A table's records as (code, data), each past its length; the signature skipped.
fn records(image: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    let mut at = 4;
    while at + 4 <= image.len() {
        let length = usize::from(u16::from_le_bytes([image[at], image[at + 1]]));
        let end = (at + 2 + length).min(image.len());
        out.push((u16::from_le_bytes([image[at + 2], image[at + 3]]), &image[at + 4..end]));
        at = end;
    }
    out
}

fn u16_at(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

fn pascal(data: &[u8], at: usize) -> (String, usize) {
    let length = usize::from(data[at]);
    (data[at + 1..at + 1 + length].iter().map(|&one| one as char).collect(), at + 1 + length)
}

/// A leaf's number: itself under 0x8000, else the tagged integer that follows.
fn numeric(data: &[u8], at: usize) -> (i64, usize) {
    let tag = u16_at(data, at);
    if tag < 0x8000 {
        return (i64::from(tag), at + 2);
    }
    let width = match tag {
        0x8000 => 1,
        0x8001 | 0x8002 => 2,
        0x8003 | 0x8004 => 4,
        _ => 8,
    };
    let mut bytes = [0u8; 8];
    bytes[..width].copy_from_slice(&data[at + 2..at + 2 + width]);
    let signed = matches!(tag, 0x8000 | 0x8001 | 0x8003);
    let value = i64::from_le_bytes(bytes);
    let value = if signed && width < 8 { value << (64 - 8 * width) >> (64 - 8 * width) } else { value };
    (value, at + 2 + width)
}

pub struct Types<'a> {
    records: Vec<(u16, &'a [u8])>,
}

impl Types<'_> {
    fn record(&self, index: u16) -> Option<(u16, &[u8])> {
        self.records.get(usize::from(index).checked_sub(0x1000)?).copied()
    }

    /// The name of a primitive: its type's code, its size, and the mode of a pointer to it.
    fn primitive(index: u16) -> String {
        let base = match index & 0xFF {
            0x03 => "VOID",
            0x10 => "CHAR",
            0x20 => "UNSIGNED CHAR",
            0x70 => "RCHAR",
            0x11 => "SHORT",
            0x21 => "UNSIGNED SHORT",
            0x12 => "LONG",
            0x22 => "UNSIGNED LONG",
            0x13 => "QUAD",
            0x23 => "UNSIGNED QUAD",
            0x76 => "INT8",
            0x77 => "UINT8",
            0x74 => "INT4",
            0x75 => "UINT4",
            0x30 => "BOOL8",
            0x40 => "REAL32",
            0x41 => "REAL64",
            0x42 => "REAL80",
            other => return format!("primitive {other:#x}"),
        };
        match index >> 8 {
            0 => base.to_owned(),
            1 => format!("NEAR {base} *"),
            2 => format!("FAR {base} *"),
            3 => format!("HUGE {base} *"),
            4 => format!("NEAR32 {base} *"),
            _ => format!("FAR32 {base} *"),
        }
    }

    /// A type's text; a struct met again inside itself is its name alone.
    pub fn name(&self, index: u16) -> String {
        self.describe(index, &mut Vec::new())
    }

    fn describe(&self, index: u16, open: &mut Vec<u16>) -> String {
        if index < 0x1000 {
            return Self::primitive(index);
        }
        let Some((leaf, data)) = self.record(index) else { return format!("type {index:#x} is not in the table") };
        match leaf {
            0x0001 => {
                let attr = u16_at(data, 0);
                format!("{}{}{}", if attr & 1 != 0 { "const " } else { "" }, if attr & 2 != 0 { "volatile " } else { "" }, self.describe(u16_at(data, 2), open))
            }
            0x0002 => {
                let kind = u16_at(data, 0) & 0x1F;
                let mode = (u16_at(data, 0) >> 5) & 7;
                let way = match kind {
                    0 => "NEAR",
                    1 => "FAR",
                    2 => "HUGE",
                    10 => "NEAR32",
                    11 => "FAR32",
                    _ => "?",
                };
                format!("{way} {} {}", if mode == 1 { "&" } else { "*" }, self.describe(u16_at(data, 2), open))
            }
            0x0003 => {
                let (bytes, _) = numeric(data, 4);
                format!("{bytes} BYTES OF {}", self.describe(u16_at(data, 0), open))
            }
            0x0005 | 0x0006 => {
                let union = leaf == 0x0006;
                let (count, field) = (u16_at(data, 0), u16_at(data, 2));
                let (_, after) = numeric(data, if union { 6 } else { 10 });
                let (name, _) = pascal(data, after);
                let kind = if union { "union" } else { "struct" };
                if open.contains(&index) || count == 0 {
                    return format!("{kind} {name}");
                }
                open.push(index);
                let members = self.members(field, open);
                open.pop();
                format!("{kind} {name} {{{}}}", members.join(", "))
            }
            0x0007 => {
                let (name, _) = pascal(data, 8);
                let list = self.record(u16_at(data, 4)).map(|(_, list)| enumerators(list)).unwrap_or_default();
                format!("enum {name} {{{}}}", list.join(", "))
            }
            0x0206 => format!("BITFIELD {} {} @{}", data[0], self.describe(u16_at(data, 2), open), data[1]),
            0x0008 => self.procedure(data, open),
            other => format!("leaf {other:#x}"),
        }
    }

    fn procedure(&self, data: &[u8], open: &mut Vec<u16>) -> String {
        let parameters = self.record(u16_at(data, 6)).map(|(_, list)| (0..usize::from(u16_at(list, 0))).map(|one| self.describe(u16_at(list, 2 + 2 * one), open)).collect::<Vec<_>>()).unwrap_or_default();
        format!("({}) -> {}", parameters.join(", "), self.describe(u16_at(data, 0), open))
    }

    fn members(&self, field: u16, open: &mut Vec<u16>) -> Vec<String> {
        let Some((_, list)) = self.record(field) else { return Vec::new() };
        let mut out = Vec::new();
        let mut at = 0;
        while at + 2 <= list.len() {
            if list[at] >= 0xF0 {
                at += 1;
                continue;
            }
            if u16_at(list, at) != 0x0406 {
                break;
            }
            let r#type = u16_at(list, at + 2);
            let (offset, next) = numeric(list, at + 6);
            let (name, end) = pascal(list, next);
            out.push(format!("{name} +{offset} {}", self.describe(r#type, open)));
            at = end;
        }
        out
    }
}

fn enumerators(list: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 2 <= list.len() {
        if list[at] >= 0xF0 {
            at += 1;
            continue;
        }
        if u16_at(list, at) != 0x0403 {
            break;
        }
        let (value, next) = numeric(list, at + 4);
        let (name, end) = pascal(list, next);
        out.push(format!("{name}={value}"));
        at = end;
    }
    out
}

/// A procedure as its record says it, with the frame variables up to its end.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub length: u32,
    /// Where its body starts and ends, from its first byte.
    pub debug_start: u32,
    pub debug_end: u32,
    /// Each BP-relative variable by name and offset, in the order written.
    pub variables: Vec<(String, i64)>,
}

/// The procedures of `records`' CodeView 4, 16- or 32-bit.
pub fn procedures(records_of_object: &[Rc<Record>]) -> Vec<Procedure> {
    let symbols = cvinfo::symbols(records_of_object);
    let mut out: Vec<Procedure> = Vec::new();
    for (code, data) in if symbols.len() >= 4 { records(&symbols) } else { Vec::new() } {
        let wide = code >= 0x0200;
        match code {
            0x0104 | 0x0105 | 0x0204 | 0x0205 => {
                let at = |field: usize| -> u32 { if wide { u32::from_le_bytes(data[12 + 4 * field..16 + 4 * field].try_into().unwrap()) } else { u32::from(u16_at(&data, 12 + 2 * field)) } };
                let (lengths, address) = if wide { (12, 6) } else { (6, 4) };
                let (name, _) = pascal(&data, 12 + lengths + address + 3);
                out.push(Procedure { name, length: at(0), debug_start: at(1), debug_end: at(2), variables: Vec::new() });
            }
            0x0100 | 0x0200 => {
                let width = if wide { 4 } else { 2 };
                let disp = if wide { i64::from(i32::from_le_bytes(data[..4].try_into().unwrap())) } else { i64::from(i16::from_le_bytes([data[0], data[1]])) };
                let (name, _) = pascal(&data, width + 2);
                if let Some(last) = out.last_mut() {
                    last.variables.push((name, disp));
                }
            }
            _ => {}
        }
    }
    out
}

/// What `records`' CodeView 4 says, one line each and sorted: `PROC name far|near (parameters) -> result`,
/// `PARAM` and `LOCAL name.variable: type`, `REGISTER name.variable: type in register N`, `DATA name: type` and
/// `UDT name: type`.
pub fn shape(records_of_object: &[Rc<Record>]) -> Vec<String> {
    let (types, symbols) = (cvinfo::types(records_of_object), cvinfo::symbols(records_of_object));
    let types = Types { records: if types.len() >= 4 { records(&types) } else { Vec::new() } };
    let mut out = Vec::new();
    let mut procedure = String::new();
    let wide = |code: u16| code >= 0x0200;
    for (code, data) in if symbols.len() >= 4 { records(&symbols) } else { Vec::new() } {
        match code {
            0x0104 | 0x0105 | 0x0204 | 0x0205 => {
                // The links, three lengths, the offset and segment, then the type, the flags and the name.
                let (lengths, address) = if wide(code) { (12, 6) } else { (6, 4) };
                let at = 12 + lengths + address;
                let r#type = u16_at(data, at);
                let flags = data[at + 2];
                let (name, _) = pascal(data, at + 3);
                let signature = types.record(r#type).map_or_else(|| "no signature".to_owned(), |(_, procedure)| types.procedure(procedure, &mut Vec::new()));
                out.push(format!("PROC {name} {} {signature}", if flags & 4 != 0 { "far" } else { "near" }));
                procedure = name;
            }
            0x0100 | 0x0200 => {
                let width = if wide(code) { 4 } else { 2 };
                let disp = if wide(code) { i64::from(i32::from_le_bytes(data[..4].try_into().unwrap())) } else { i64::from(i16::from_le_bytes([data[0], data[1]])) };
                let (name, _) = pascal(data, width + 2);
                out.push(format!("{} {procedure}.{name}: {}", if disp > 0 { "PARAM" } else { "LOCAL" }, types.name(u16_at(data, width))));
            }
            0x0002 => {
                let (name, _) = pascal(data, 4);
                out.push(format!("REGISTER {procedure}.{name}: {} in register {}", types.name(u16_at(data, 0)), u16_at(data, 2)));
            }
            0x0101 | 0x0102 | 0x0201 | 0x0202 => {
                let at = if wide(code) { 6 } else { 4 };
                let (name, _) = pascal(data, at + 2);
                out.push(format!("DATA {name}: {}", types.name(u16_at(data, at))));
            }
            0x0004 => {
                let (name, _) = pascal(data, 2);
                out.push(format!("UDT {name}: {}", types.name(u16_at(data, 0))));
            }
            _ => {}
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omf;

    fn shape_of(object: &str) -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/cv4").join(object);
        shape(&omf::parse(&std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))).unwrap())
    }

    /// Objects ML 6.11 wrote under /Zi (tests/inputs/cv4, with their sources) read as the programs they are: a struct
    /// with its members, a procedure and its arguments, its parameters and local, data of each primitive size and a
    /// pointer, named by a UDT. ML writes its debug segments as SEGDEF32, whose four-byte length the segment reader
    /// took for two: it found no $$SYMBOLS and read nothing.
    #[test]
    fn objects_ml_wrote_read_as_the_programs_they_are() {
        let point = "struct point {px +0 UNSIGNED SHORT, py +2 UNSIGNED SHORT}";
        let p1 = shape_of("p1.obj");
        for expected in [
            "PROC f near (UNSIGNED SHORT, UNSIGNED SHORT) -> VOID".to_owned(),
            "PARAM f.a: UNSIGNED SHORT".to_owned(),
            "PARAM f.b: UNSIGNED SHORT".to_owned(),
            "LOCAL f.x: UNSIGNED SHORT".to_owned(),
            format!("LOCAL f.q: {point}"),
            format!("DATA pt: {point}"),
            "DATA big: UNSIGNED LONG".to_owned(),
            format!("UDT point: {point}"),
        ] {
            assert!(p1.contains(&expected), "no {expected:?} in {p1:#?}");
        }
        let p3 = shape_of("p3.obj");
        for expected in ["DATA v1: NEAR UNSIGNED SHORT *", "DATA v2: FAR UNSIGNED CHAR *", "DATA v4: REAL32", "DATA v5: REAL64", "DATA v7: CHAR", "DATA v8: LONG", "DATA v6: INT8", "UDT pw: NEAR UNSIGNED SHORT *"] {
            assert!(p3.iter().any(|one| one == expected), "no {expected:?} in {p3:#?}");
        }
        // Flat 32-bit: a four-byte member is at 4, and a pointer to a dword is NEAR32.
        let p2 = shape_of("p2.obj");
        assert!(p2.contains(&"DATA pt: struct point {px +0 UNSIGNED LONG, py +4 UNSIGNED LONG}".to_owned()), "{p2:#?}");
        assert!(shape_of("p4.obj").contains(&"DATA v1: NEAR32 * UNSIGNED LONG".to_owned()));
    }
}
