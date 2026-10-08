//! What a listing does at a call boundary, read by running its straight-line
//! code over symbolic bytes: where each argument's bytes are pushed and read,
//! where a result is left and taken from, who pops how much, and what a
//! callee restores. It reads BCC's `-S`, BC's `/A` and llrm's own listings
//! alike, so a reference and llrm's lowering are measured by one instrument.

use std::collections::{BTreeMap};
use crate::support::hash::HashMap;
use std::fmt;

/// One byte of a value, by where it came from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Byte {
    Const(u8),
    /// Byte `1` of the object `0`, as it was on entry.
    Global(String, i32),
    /// The byte at entry SP + `0`: a callee's argument area.
    Incoming(i32),
    /// A register's byte as it was on entry: a callee's to keep.
    Entry(&'static str, u8),
    /// Byte `2` of the address `0` + `1`.
    Address(Box<Base>, i32, u8),
    /// Byte `1` of what call `0` left in a register or in st(0).
    Returned(String, &'static str, u8),
    /// Byte `2` of what call `0` wrote through a pointer argument, at offset `1`.
    Written(String, i32),
    /// The byte at offset `1` through the pointer `0`, as it was when read.
    Pointed(Vec<Byte>, i32),
    /// Byte `2` of the x87 value whose own bytes are `0`, stored `1` bytes wide.
    Converted(Vec<Byte>, u8, u8),
    /// The sign of a byte, filling a wider value.
    Sign(Box<Byte>),
    Unknown,
}

/// What an address is an offset from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Base {
    /// Entry SP.
    Stack,
    Global(String),
    /// A segment register's value on entry.
    Segment(&'static str),
    /// A pointer held in these bytes.
    Pointer(Vec<Byte>),
}

impl fmt::Display for Byte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Byte::Const(value) => write!(f, "{value:02X}"),
            Byte::Global(name, at) => write!(f, "{name}[{at}]"),
            Byte::Incoming(at) => write!(f, "in+{at}"),
            Byte::Entry(register, at) => write!(f, "{register}.{at}"),
            Byte::Address(base, at, part) => write!(f, "&{base:?}{at:+}.{part}"),
            Byte::Returned(call, register, at) => write!(f, "{call}:{register}.{at}"),
            Byte::Written(call, at) => write!(f, "{call}:*{at}"),
            Byte::Converted(source, width, at) => {
                let bits = u32::from(*width) * 8;
                match source.first() {
                    Some(Byte::Global(name, 0)) if source.iter().all(|one| matches!(one, Byte::Global(other, _) if other == name)) => write!(f, "{name}:f{bits}[{at}]"),
                    _ => write!(f, "({}):f{bits}[{at}]", source.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")),
                }
            }
            Byte::Pointed(pointer, at) => write!(f, "*({}){at:+}", pointer.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")),
            Byte::Sign(byte) => write!(f, "sign({byte})"),
            Byte::Unknown => write!(f, "?"),
        }
    }
}

/// One call a procedure makes.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub target: String,
    pub far: bool,
    /// Its arguments as they lie at the call, lowest address first.
    pub stack: Vec<Byte>,
    /// What the caller pops after it.
    pub popped: i32,
}

/// What one procedure did, run from entry to its return.
#[derive(Clone, Debug, PartialEq)]
pub struct Procedure {
    /// The listing line it starts on, from 1.
    pub line: usize,
    pub far: bool,
    /// What its return pops.
    pub popped: i32,
    pub calls: Vec<Call>,
    /// Every global it wrote, byte by byte.
    pub globals: BTreeMap<String, Vec<Byte>>,
    /// Every write through a pointer argument, by the pointer's bytes.
    pub through: BTreeMap<String, Vec<Byte>>,
    /// The registers at the return.
    pub registers: HashMap<&'static str, Vec<Byte>>,
    /// st(0) at the return, if the x87 stack is not empty.
    pub top: Option<Vec<Byte>>,
    /// Whether it returns with the direction flag clear.
    pub forward: bool,
    /// A branch ended the run before any return: only what came before it is here.
    pub cut: bool,
    /// It returned by `iret`, as an interrupt handler does.
    pub iret: bool,
}

impl Procedure {
    /// `register`'s bytes at the return: al, ax, dx:ax (as `dxax`), eax...
    pub fn register(&self, name: &str) -> Vec<Byte> {
        match name {
            "dxax" => [&self.registers["eax"][..2], &self.registers["edx"][..2]].concat(),
            _ => {
                let (full, from, width) = view(name).unwrap_or_else(|| panic!("no register {name}"));
                self.registers[full][from..from + width].to_vec()
            }
        }
    }
}

/// Each procedure in `listing`, by name.
pub fn procedures(listing: &str) -> BTreeMap<String, Procedure> {
    if listing.contains("Source Line") && listing.contains(" ** ") {
        return bc_procedures(listing);
    }
    let constants = data(listing);
    let mut found = BTreeMap::new();
    let mut open: Option<(String, bool, usize, Vec<Line>)> = None;
    for (number, raw) in listing.lines().enumerate() {
        let text = clean(raw);
        let words: Vec<&str> = text.split_whitespace().collect();
        match words.as_slice() {
            [name, "proc", rest @ ..] => open = Some((name.to_string(), rest.first() == Some(&"far"), number + 1, Vec::new())),
            [name, "endp"] => {
                if let Some((open_name, far, line, lines)) = open.take() {
                    assert_eq!(&open_name, name, "{name} ends another procedure");
                    found.insert(open_name.clone(), Procedure { line, ..run(&open_name, far, &lines, &constants) });
                }
            }
            _ => {
                if let (Some((_, _, _, lines)), Some(line)) = (open.as_mut(), parse(&text)) {
                    lines.push(line);
                }
            }
        }
    }
    found
}

/// Each SUB and FUNCTION in a BC `/A` listing, from its entry label to its
/// return. Every BASIC procedure is far; BC lists its `retf` as `ret`.
fn bc_procedures(listing: &str) -> BTreeMap<String, Procedure> {
    // VBDOS lists a pool constant as its value, high byte first; QuickBASIC
    // 4.5 and PDS 7.1 list its bytes in memory order.
    let memory_order = !listing.contains("Visual Basic");
    let mut found = BTreeMap::new();
    let (mut pending, mut open): (Option<String>, Option<(String, usize, Vec<Line>)>) = (None, None);
    let mut prefix: Option<&'static str> = None;
    let mut emulator: Option<(u8, Vec<String>)> = None;
    for (row, raw) in listing.lines().enumerate() {
        let raw = raw.trim_end_matches('\r');
        let Some(at) = raw.find(" ** ") else {
            // A source line: `sub name (...)` or `function name (...)` opens one.
            let source = raw.get(16..).unwrap_or("").trim().to_lowercase();
            let mut words = source.split(|one: char| one.is_whitespace() || one == '(');
            if let (Some("sub" | "function"), Some(name)) = (words.next(), words.next()) {
                pending = Some(name.to_uppercase());
            }
            continue;
        };
        let code = raw[at + 4..].trim();
        let (label, instruction) = match code.split_once(':') {
            Some((label, rest)) if is_label(label.trim()) && !label.trim().contains(' ') => (Some(label.trim().to_uppercase()), rest.trim()),
            _ => (None, code),
        };
        if label.is_some() && label == pending {
            open = Some((pending.take().expect("a name"), row + 1, Vec::new()));
        }
        let Some((_, _, lines)) = open.as_mut() else { continue };
        let mut text = instruction.to_lowercase();
        // /FPi: an emulator interrupt stands for an ESC opcode, its operand in
        // the db and dw lines after it.
        if let Some(interrupt) = text.strip_prefix("int").map(str::trim).and_then(|one| number(one)).filter(|one| (0x34..=0x3B).contains(one)) {
            emulator = Some((0xD8 + (interrupt - 0x34) as u8, Vec::new()));
            continue;
        }
        if let Some((opcode, operand)) = emulator.as_mut() {
            if let Some(rest) = text.strip_prefix("db").or_else(|| text.strip_prefix("dw")) {
                operand.push(rest.trim().to_owned());
                if let Some(decoded) = escape(*opcode, operand) {
                    emulator = None;
                    text = decoded;
                } else {
                    continue;
                }
            } else {
                emulator = None;
            }
        }
        if text == "int     3dh" || text.split_whitespace().collect::<Vec<_>>() == ["int", "3dh"] {
            continue;
        }
        // A segment override on a line of its own belongs to the next instruction.
        if let Some(segment) = ["es:", "cs:", "ss:", "ds:"].iter().find(|one| text == **one) {
            prefix = register(&segment[..2]);
            continue;
        }
        if memory_order {
            text = reordered(&text);
        }
        let Some(mut line) = parse(&text) else { continue };
        // BC lists its far calls as plain `call`: every call it makes is far.
        if let (true, [Operand::Target(_, far @ None)]) = (line.op == "call", line.operands.as_mut_slice()) {
            *far = Some(true);
        }
        if let Some(segment) = prefix.take() {
            for one in &mut line.operands {
                if let Operand::Memory { segment: none @ None, .. } = one {
                    *none = Some(segment);
                }
            }
        }
        // A constant pushed a word at a time is listed twice by one name:
        // the first push is its high word.
        if let (Some(previous), "push", [Operand::Memory { name: Some(name), registers, .. }]) = (lines.last_mut(), line.op.as_str(), line.operands.as_slice()) {
            if let ("push", [Operand::Memory { name: Some(before), registers: none, displacement, .. }]) = (previous.op.as_str(), previous.operands.as_mut_slice()) {
                if before == name && name.starts_with('<') && registers.is_empty() && none.is_empty() && *displacement == 0 {
                    *displacement = 2;
                }
            }
        }
        let returns = matches!(line.op.as_str(), "ret" | "retf");
        lines.push(line);
        if returns {
            let (name, line, lines) = open.take().expect("an open procedure");
            found.insert(name.clone(), Procedure { line, ..run(&name, true, &lines, &HashMap::default()) });
        }
    }
    found
}

/// `text` with each `<hex>` constant turned from memory order to value order.
fn reordered(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else { break };
        let hex = &rest[start + 1..start + end];
        let flipped: String = (0..hex.len()).step_by(2).rev().filter_map(|at| hex.get(at..at + 2)).collect();
        out += &rest[..start];
        out += &format!("<{flipped}>");
        rest = &rest[start + end + 1..];
    }
    out + rest
}

/// The x87 instruction `opcode` and the listed operand bytes spell, once
/// they are all there: its ModRM byte, then any displacement (BC lists a
/// constant's as `<value>`).
fn escape(opcode: u8, operand: &[String]) -> Option<String> {
    let modrm = number(&operand[0].to_lowercase())? as u8;
    let (mode, reg, rm) = (modrm >> 6, (modrm >> 3) & 7, modrm & 7);
    let displacement = match (mode, rm) {
        (0, 6) | (2, _) => Some(operand.get(1)?.clone()),
        (1, _) => Some(operand.get(1)?.clone()),
        _ => None,
    };
    if mode == 3 {
        return Some(format!("f{opcode:02x}{modrm:02x}"));
    }
    let base = ["bx+si", "bx+di", "bp+si", "bp+di", "si", "di", "bp", "bx"][rm as usize];
    let place = match (mode, displacement) {
        (0, Some(constant)) => format!("[{}]", constant.trim_matches(['<', '>'])),
        (_, Some(disp)) => format!("[{base}+{}]", number(&disp).map_or(disp.clone(), |one| if mode == 1 { (one as i8).to_string() } else { (one as i16).to_string() })),
        (_, None) => format!("[{base}]"),
    };
    let (name, width) = match (opcode, reg) {
        (0xD9, 0) => ("fld", "dword"),
        (0xDD, 0) => ("fld", "qword"),
        (0xDB, 5) => ("fld", "tbyte"),
        (0xD9, 2) => ("fst", "dword"),
        (0xDD, 2) => ("fst", "qword"),
        (0xD9, 3) => ("fstp", "dword"),
        (0xDD, 3) => ("fstp", "qword"),
        (0xDB, 7) => ("fstp", "tbyte"),
        (0xDF, 0) | (0xDB, 0) | (0xDF, 5) => ("fild", "word"),
        _ => ("farith", "dword"),
    };
    // A constant's displacement is a pool object: name it as one.
    let place = if place.starts_with('[') && !place.contains('+') && !place.contains("bx") && !place.contains("si") && !place.contains("di") && !place.contains("bp") { format!("<{}>", place.trim_matches(['[', ']'])) } else { place };
    Some(format!("{name} {width} ptr {place}"))
}

/// Each `name label byte` the listing follows with `db` bytes.
fn data(listing: &str) -> HashMap<String, Vec<u8>> {
    let mut found: HashMap<String, Vec<u8>> = HashMap::default();
    let mut open: Option<String> = None;
    for line in listing.lines().map(|one| one.trim().to_lowercase()) {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [name, "label", _] => open = Some((*name).to_owned()),
            ["db", ..] if open.is_some() => {
                let bytes: Option<Vec<u8>> = line[2..].split(',').map(|one| number(one).map(|value| value as u8)).collect();
                match bytes {
                    Some(bytes) => found.entry(open.clone().expect("a label")).or_default().extend(bytes),
                    None => open = None,
                }
            }
            _ => open = None,
        }
    }
    found
}

/// A constant's bytes: data the listing defines, or BC's `<hex>` operand,
/// its value written high byte first.
fn data_constant(constants: &HashMap<String, Vec<u8>>, name: &str) -> Option<Vec<u8>> {
    if let Some(hex) = name.strip_prefix('<').and_then(|rest| rest.strip_suffix('>')).or_else(|| name.strip_prefix("k<").and_then(|rest| rest.strip_suffix('>'))) {
        return (0..hex.len()).step_by(2).rev().map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok()).collect();
    }
    constants.get(name).cloned()
}

/// A line without its comment and BC's listing columns, lowercased.
fn clean(raw: &str) -> String {
    let text = raw.split(';').next().unwrap_or("").trim_end();
    // BC /A: offset, `**`, an optional label, then the instruction.
    let text = match text.find(" ** ") {
        Some(at) => &text[at + 4..],
        None => text,
    };
    let text = text.trim();
    let text = match text.split_once(':') {
        Some((label, rest)) if !label.contains(' ') && !label.contains('[') && !rest.starts_with('[') && is_label(label) => rest.trim(),
        _ => text,
    };
    text.to_lowercase()
}

fn is_label(word: &str) -> bool {
    !word.is_empty() && !matches!(word.to_lowercase().as_str(), "es" | "cs" | "ss" | "ds" | "fs" | "gs" | "dgroup") && word.chars().all(|one| one.is_alphanumeric() || "_@$?!#%&".contains(one))
}

#[derive(Clone, Debug)]
struct Line {
    op: String,
    operands: Vec<Operand>,
}

#[derive(Clone, Debug, PartialEq)]
enum Operand {
    Register(&'static str),
    Immediate(i64),
    /// `offset name+at`.
    Offset(String, i32),
    Memory { width: Option<usize>, segment: Option<&'static str>, name: Option<String>, registers: Vec<&'static str>, displacement: i32 },
    Float(usize),
    Target(String, Option<bool>),
}

fn parse(text: &str) -> Option<Line> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('.') || text.ends_with(':') {
        return None;
    }
    let (op, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let op = op.trim().to_owned();
    if matches!(op.as_str(), "public" | "extrn" | "assume" | "db" | "dw" | "dd" | "label" | "end" | "align" | "even") {
        return None;
    }
    let operands = if op.starts_with("call") || op.starts_with('j') {
        vec![target(rest.trim())]
    } else {
        split(rest).iter().map(|one| operand(one)).collect()
    };
    Some(Line { op, operands })
}

fn target(text: &str) -> Operand {
    let words: Vec<&str> = text.split_whitespace().collect();
    let far = match words.first() {
        Some(&"far") => Some(true),
        Some(&"near") => Some(false),
        _ => None,
    };
    Operand::Target(words.last().copied().unwrap_or("").trim_start_matches("dgroup:").to_owned(), far)
}

fn split(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (mut depth, mut current) = (0, String::new());
    for one in text.chars() {
        match one {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(current.trim().to_owned());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(one);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_owned());
    }
    parts
}

const REGISTERS: [&str; 30] = [
    "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "ax", "bx", "cx", "dx", "si", "di", "bp", "sp", "al", "bl", "cl", "dl", "ah", "bh", "ch", "dh", "es", "cs", "ss", "ds", "fs", "gs",
];

fn register(text: &str) -> Option<&'static str> {
    REGISTERS.iter().copied().find(|one| *one == text)
}

fn number(text: &str) -> Option<i64> {
    let text = text.trim();
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let value = if let Some(hex) = text.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(hex) = text.strip_suffix('h') {
        i64::from_str_radix(hex, 16).ok()?
    } else {
        text.parse().ok()?
    };
    Some(if negative { -value } else { value })
}

fn width(word: &str) -> Option<usize> {
    Some(match word {
        "byte" => 1,
        "word" => 2,
        "dword" => 4,
        "qword" => 8,
        "tbyte" => 10,
        _ => return None,
    })
}

fn operand(text: &str) -> Operand {
    let text = text.trim();
    if let Some(one) = register(text) {
        return Operand::Register(one);
    }
    if text == "st" {
        return Operand::Float(0);
    }
    if let Some(inside) = text.strip_prefix("st(").and_then(|rest| rest.strip_suffix(')')) {
        return Operand::Float(inside.parse().unwrap_or(0));
    }
    if let Some(value) = number(text) {
        return Operand::Immediate(value);
    }
    if let Some(rest) = text.strip_prefix("offset ") {
        let (name, at) = symbol(rest.trim().trim_start_matches("dgroup:"));
        return Operand::Offset(name, at);
    }
    let mut rest = text;
    let mut size = None;
    if let Some((word, after)) = rest.split_once(" ptr ") {
        size = width(word.trim());
        rest = after.trim();
    }
    let mut segment = None;
    for one in ["es:", "cs:", "ss:", "ds:"] {
        if let Some(after) = rest.strip_prefix(one) {
            segment = register(&one[..2]);
            rest = after;
        }
    }
    let rest = rest.trim_start_matches("dgroup:");
    let (outside, inside) = match rest.find('[') {
        Some(at) => (&rest[..at], rest[at + 1..].trim_end_matches(']')),
        None => (rest, ""),
    };
    let (mut name, mut displacement) = (None, 0);
    match number(outside) {
        // BC writes a displacement before the brackets, 16 bits wide:
        // 08h[bp], and 0FF30h[bp] for -208.
        Some(value) if !inside.is_empty() => displacement += i32::from(value as u16 as i16),
        _ if !outside.is_empty() => {
            let (symbol_name, at) = symbol(outside);
            name = Some(symbol_name);
            displacement += at;
        }
        _ => {}
    }
    let mut registers = Vec::new();
    for term in inside.replace('-', "+-").split('+').map(str::trim).filter(|one| !one.is_empty()) {
        match (register(term), number(term)) {
            (Some(one), _) => registers.push(one),
            (None, Some(value)) => displacement += value as i32,
            (None, None) => {
                let (symbol_name, at) = symbol(term);
                name = Some(symbol_name);
                displacement += at;
            }
        }
    }
    Operand::Memory { width: size, segment, name, registers, displacement }
}

/// `name+2+1` as its name and summed offset.
fn symbol(text: &str) -> (String, i32) {
    let mut parts = text.replace('-', "+-");
    parts.retain(|one| !one.is_whitespace());
    let mut terms = parts.split('+').filter(|one| !one.is_empty());
    let name = terms.next().unwrap_or("").to_owned();
    (name, terms.filter_map(number).map(|one| one as i32).sum())
}

/// A register's full name, and the bytes of it `name` covers.
fn view(name: &str) -> Option<(&'static str, usize, usize)> {
    Some(match name {
        "eax" => ("eax", 0, 4),
        "ebx" => ("ebx", 0, 4),
        "ecx" => ("ecx", 0, 4),
        "edx" => ("edx", 0, 4),
        "esi" => ("esi", 0, 4),
        "edi" => ("edi", 0, 4),
        "ebp" => ("ebp", 0, 4),
        "esp" => ("esp", 0, 4),
        "ax" => ("eax", 0, 2),
        "bx" => ("ebx", 0, 2),
        "cx" => ("ecx", 0, 2),
        "dx" => ("edx", 0, 2),
        "si" => ("esi", 0, 2),
        "di" => ("edi", 0, 2),
        "bp" => ("ebp", 0, 2),
        "sp" => ("esp", 0, 2),
        "al" => ("eax", 0, 1),
        "bl" => ("ebx", 0, 1),
        "cl" => ("ecx", 0, 1),
        "dl" => ("edx", 0, 1),
        "ah" => ("eax", 1, 1),
        "bh" => ("ebx", 1, 1),
        "ch" => ("ecx", 1, 1),
        "dh" => ("edx", 1, 1),
        "es" => ("es", 0, 2),
        "cs" => ("cs", 0, 2),
        "ss" => ("ss", 0, 2),
        "ds" => ("ds", 0, 2),
        "fs" => ("fs", 0, 2),
        "gs" => ("gs", 0, 2),
        _ => return None,
    })
}

const GENERAL: [&str; 8] = ["eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp"];
const SEGMENTS: [&str; 6] = ["es", "cs", "ss", "ds", "fs", "gs"];
/// What a Borland, Microsoft or llrm callee may leave changed.
const SCRATCH: [&str; 5] = ["eax", "ebx", "ecx", "edx", "es"];

/// Where a memory operand lands.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Place {
    Stack(i32),
    Global(String, i32),
    Through(Vec<Byte>, i32),
    Nowhere,
}

struct Machine<'a> {
    name: &'a str,
    /// Read-only data the listing defines, by lowercased name.
    constants: &'a HashMap<String, Vec<u8>>,
    registers: HashMap<&'static str, Vec<Byte>>,
    memory: HashMap<Place, Byte>,
    written: BTreeMap<String, Vec<Byte>>,
    through: BTreeMap<String, Vec<Byte>>,
    floats: Vec<Vec<Byte>>,
    /// SP as an offset from entry SP.
    sp: i32,
    forward: bool,
    calls: Vec<Call>,
    /// SP where the pushes for the next call begin.
    base: i32,
    /// The call whose caller cleanup is still being counted.
    pending: Option<usize>,
    cut: bool,
    iret: bool,
}

fn run(name: &str, far: bool, lines: &[Line], constants: &HashMap<String, Vec<u8>>) -> Procedure {
    let mut registers = HashMap::default();
    for one in GENERAL {
        registers.insert(one, (0..4).map(|at| Byte::Entry(one, at)).collect());
    }
    for one in SEGMENTS {
        registers.insert(one, (0..2).map(|at| Byte::Entry(one, at)).collect());
    }
    let mut machine = Machine {
        name,
        constants,
        registers,
        memory: HashMap::default(),
        written: BTreeMap::new(),
        through: BTreeMap::new(),
        floats: Vec::new(),
        sp: 0,
        forward: true,
        calls: Vec::new(),
        base: 0,
        pending: None,
        cut: false,
        iret: false,
    };
    machine.set("sp", address(Base::Stack, 0, 2));
    let mut in_prologue = true;
    for line in lines {
        // A frame's setup: an interrupt handler's saves and DGROUP loads too.
        let dgroup = |one: &Operand| matches!(one, Operand::Memory { name: Some(name), registers, .. } if name == "dgroup" && registers.is_empty());
        let setup = match (line.op.as_str(), line.operands.as_slice()) {
            ("push", [Operand::Register(_)]) | ("pushad" | "cld" | "enter", _) | ("pop", [Operand::Register("ds" | "es")]) => true,
            ("mov", [Operand::Register("bp"), Operand::Register("sp")]) | ("mov", [Operand::Register("ds"), Operand::Register("bp")]) => true,
            ("mov", [Operand::Register("bp"), one]) | ("push" | "pushw", [one]) => dgroup(one),
            ("sub", [Operand::Register("sp"), Operand::Immediate(_)]) => true,
            _ => false,
        };
        if in_prologue && !setup {
            in_prologue = false;
            machine.base = machine.sp;
        }
        if let Some(popped) = machine.step(line) {
            return machine.finished(far, popped);
        }
    }
    // It runs off its end, into what follows: as cut short as a branch.
    machine.cut = true;
    machine.finished(far, 0)
}

fn address(base: Base, at: i32, bytes: u8) -> Vec<Byte> {
    let base = Box::new(base);
    (0..bytes).map(|part| Byte::Address(base.clone(), at, part)).collect()
}

/// The constant `value` as `bytes` little-endian bytes.
fn constant(value: i64, bytes: usize) -> Vec<Byte> {
    (0..bytes).map(|at| Byte::Const((value >> (8 * at)) as u8)).collect()
}

fn known(bytes: &[Byte]) -> Option<i64> {
    bytes.iter().enumerate().try_fold(0i64, |sum, (at, one)| match one {
        Byte::Const(value) => Some(sum | i64::from(*value) << (8 * at)),
        _ => None,
    })
}

impl Machine<'_> {
    fn finished(self, far: bool, popped: i32) -> Procedure {
        Procedure {
            line: 0,
            far,
            popped,
            top: self.floats.last().cloned(),
            calls: self.calls,
            globals: self.written,
            through: self.through,
            forward: self.forward,
            cut: self.cut,
            iret: self.iret,
            registers: self.registers,
        }
    }

    fn get(&self, name: &str) -> Vec<Byte> {
        let (full, from, width) = view(name).unwrap_or_else(|| panic!("{}: no register {name}", self.name));
        self.registers[full][from..from + width].to_vec()
    }

    fn set(&mut self, name: &str, value: Vec<Byte>) {
        let (full, from, width) = view(name).unwrap_or_else(|| panic!("{}: no register {name}", self.name));
        assert_eq!(value.len(), width, "{}: {name} set from {} bytes", self.name, value.len());
        self.registers.get_mut(full).expect("a register")[from..from + width].clone_from_slice(&value);
        if full == "esp" {
            self.sp = self.stack_offset(&self.registers["esp"][..2]).unwrap_or(self.sp);
        }
    }

    fn stack_offset(&self, bytes: &[Byte]) -> Option<i32> {
        match bytes {
            [Byte::Address(base, at, 0), Byte::Address(same, other, 1)] if **base == Base::Stack && **same == Base::Stack && at == other => Some(*at),
            _ => None,
        }
    }

    /// Where a memory operand points.
    fn place(&self, operand: &Operand) -> Place {
        let Operand::Memory { name, registers, displacement, segment, .. } = operand else { panic!("{}: {operand:?} is not memory", self.name) };
        if let Some(name) = name {
            if registers.is_empty() {
                return Place::Global(name.clone(), *displacement);
            }
        }
        let mut offset = *displacement;
        let mut base = None;
        for one in registers {
            let value = self.get(one);
            match value.as_slice() {
                [Byte::Address(from, at, 0), Byte::Address(_, _, 1)] => {
                    offset += at;
                    base = Some((**from).clone());
                }
                _ => match known(&value) {
                    Some(value) => offset += value as i32,
                    None => base = Some(Base::Pointer(value)),
                },
            }
        }
        match base {
            Some(Base::Stack) => Place::Stack(offset),
            Some(Base::Global(name)) => Place::Global(name, offset),
            Some(Base::Pointer(bytes)) => {
                // A far pointer: its segment is the segment register the access
                // names, unless that holds DGROUP, as DS does.
                let dgroup: Vec<Byte> = (0..2).map(|at| Byte::Entry("ds", at)).collect();
                let segment = segment.map(|one| self.get(one)).filter(|one| *one != dgroup);
                Place::Through([bytes, segment.unwrap_or_default()].concat(), offset)
            }
            _ => Place::Nowhere,
        }
    }

    fn load(&self, place: &Place, bytes: usize) -> Vec<Byte> {
        (0..bytes as i32)
            .map(|at| {
                let one = match place {
                    Place::Stack(offset) => Place::Stack(offset + at),
                    Place::Global(name, offset) => Place::Global(name.clone(), offset + at),
                    Place::Through(pointer, offset) => Place::Through(pointer.clone(), offset + at),
                    Place::Nowhere => return Byte::Unknown,
                };
                self.memory.get(&one).cloned().unwrap_or_else(|| match &one {
                    Place::Stack(offset) if *offset >= 0 => Byte::Incoming(*offset),
                    Place::Through(pointer, offset) => Byte::Pointed(pointer.clone(), *offset),
                    Place::Global(name, offset) => match data_constant(self.constants, name).and_then(|bytes| bytes.get(*offset as usize).copied()) {
                        Some(value) => Byte::Const(value),
                        None => Byte::Global(name.clone(), *offset),
                    },
                    _ => Byte::Unknown,
                })
            })
            .collect()
    }

    fn store(&mut self, place: &Place, value: Vec<Byte>) {
        for (at, byte) in value.into_iter().enumerate() {
            let at = at as i32;
            let one = match place {
                Place::Stack(offset) => Place::Stack(offset + at),
                Place::Global(name, offset) => {
                    let bytes = self.written.entry(name.clone()).or_default();
                    let index = (offset + at) as usize;
                    if bytes.len() <= index {
                        bytes.resize(index + 1, Byte::Unknown);
                    }
                    bytes[index] = byte.clone();
                    Place::Global(name.clone(), offset + at)
                }
                Place::Through(pointer, offset) => {
                    let key = pointer.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ");
                    let bytes = self.through.entry(key).or_default();
                    let index = (offset + at) as usize;
                    if bytes.len() <= index {
                        bytes.resize(index + 1, Byte::Unknown);
                    }
                    bytes[index] = byte.clone();
                    Place::Through(pointer.clone(), offset + at)
                }
                Place::Nowhere => continue,
            };
            self.memory.insert(one, byte);
        }
    }

    fn operand_width(&self, operand: &Operand, other: Option<&Operand>) -> usize {
        match operand {
            Operand::Register(one) => view(one).map_or(2, |(_, _, width)| width),
            Operand::Memory { width: Some(width), .. } => *width,
            _ => other.map_or(2, |one| self.operand_width(one, None)),
        }
    }

    fn read(&self, operand: &Operand, bytes: usize) -> Vec<Byte> {
        match operand {
            Operand::Register(one) => self.get(one),
            Operand::Immediate(value) => constant(*value, bytes),
            Operand::Offset(name, at) => {
                let mut value = address(Base::Global(name.clone()), *at, 2);
                value.resize(bytes, Byte::Const(0));
                value
            }
            // DGROUP's selector, as DS holds it.
            Operand::Memory { name: Some(name), registers, .. } if name == "dgroup" && registers.is_empty() => self.get("ds"),
            Operand::Memory { .. } => self.load(&self.place(operand), bytes),
            _ => vec![Byte::Unknown; bytes],
        }
    }

    fn write(&mut self, operand: &Operand, value: Vec<Byte>) {
        match operand {
            Operand::Register(one) => self.set(one, value),
            Operand::Memory { .. } => {
                let place = self.place(operand);
                self.store(&place, value);
            }
            _ => panic!("{}: a write to {operand:?}", self.name),
        }
    }

    fn push(&mut self, value: Vec<Byte>) {
        let sp = self.sp - value.len() as i32;
        self.store(&Place::Stack(sp), value);
        self.set("sp", address(Base::Stack, sp, 2));
    }

    fn pop(&mut self, bytes: usize) -> Vec<Byte> {
        let value = self.load(&Place::Stack(self.sp), bytes);
        let sp = self.sp + bytes as i32;
        self.set("sp", address(Base::Stack, sp, 2));
        value
    }

    fn add_constant(value: &[Byte], amount: i64) -> Vec<Byte> {
        match value {
            [Byte::Address(base, at, 0), Byte::Address(_, _, 1), ..] => {
                let mut sum = address((**base).clone(), at + amount as i32, 2);
                sum.extend(value[2..].iter().map(|_| Byte::Unknown));
                sum
            }
            _ => match known(value) {
                Some(known) => constant(known + amount, value.len()),
                // A pointer from elsewhere, stepped: the same base, an offset on.
                None if value.len() == 2 && !value.contains(&Byte::Unknown) => address(Base::Pointer(value.to_vec()), amount as i32, 2),
                None => vec![Byte::Unknown; value.len()],
            },
        }
    }

    /// A call: it consumes the pushes since the last one and leaves what a
    /// callee may.
    fn call(&mut self, target: &str, far: bool) {
        let stack = self.load(&Place::Stack(self.sp), (self.base - self.sp).max(0) as usize);
        // What a pointer argument addresses is the callee's to write.
        for (at, pair) in stack.windows(2).enumerate() {
            if let Some(offset) = self.stack_offset(pair).filter(|_| at % 2 == 0) {
                for index in 0..16 {
                    self.memory.insert(Place::Stack(offset + index), Byte::Written(target.to_owned(), index));
                }
            }
            if let [Byte::Address(base, offset, 0), Byte::Address(_, _, 1)] = pair {
                if let Base::Global(name) = &**base {
                    for index in 0..16 {
                        let byte = Byte::Written(target.to_owned(), index);
                        self.memory.insert(Place::Global(name.clone(), offset + index), byte.clone());
                        let bytes = self.written.entry(name.clone()).or_default();
                        let place = (offset + index) as usize;
                        if bytes.len() <= place {
                            bytes.resize(place + 1, Byte::Unknown);
                        }
                        bytes[place] = byte;
                    }
                }
            }
        }
        self.calls.push(Call { target: target.to_owned(), far, stack, popped: 0 });
        self.pending = Some(self.calls.len() - 1);
        for one in SCRATCH {
            let bytes = self.registers[one].len();
            let value = (0..bytes as u8).map(|at| if bytes == 4 && at < 4 { Byte::Returned(target.to_owned(), one, at) } else { Byte::Unknown }).collect();
            self.registers.insert(one, value);
        }
        for one in ["esi", "edi", "ebp"] {
            for at in 2..4 {
                self.registers.get_mut(one).expect("a register")[at] = Byte::Unknown;
            }
        }
        // The x87 stack is empty across a call but for what it returns.
        self.floats = vec![(0..10).map(|at| Byte::Returned(target.to_owned(), "st", at)).collect()];
    }

    /// `count` moves of `op`'s width from DS:[SI] to ES:[DI], forward.
    fn string_move(&mut self, op: &str, count: i64) {
        let width = match op {
            "movsb" => 1,
            "movsw" => 2,
            _ => 4,
        };
        for _ in 0..count {
            let from = Operand::Memory { width: Some(width), segment: None, name: None, registers: vec!["si"], displacement: 0 };
            let to = Operand::Memory { width: Some(width), segment: Some("es"), name: None, registers: vec!["di"], displacement: 0 };
            let value = self.read(&from, width);
            self.write(&to, value);
            for one in ["si", "di"] {
                let moved = Self::add_constant(&self.get(one), width as i64);
                self.set(one, moved);
            }
        }
    }

    /// Borland's struct push: CX bytes from DX:AX, rounded to a word.
    fn struct_push(&mut self) {
        let size = known(&self.get("cx")).unwrap_or(0) as usize;
        let from = self.get("ax");
        let place = match self.stack_offset(&from) {
            Some(offset) => Place::Stack(offset),
            None => match from.as_slice() {
                [Byte::Address(base, at, 0), ..] => match &**base {
                    Base::Global(name) => Place::Global(name.clone(), *at),
                    _ => Place::Nowhere,
                },
                _ => Place::Nowhere,
            },
        };
        let mut value = self.load(&place, size);
        if size % 2 == 1 {
            value.push(Byte::Unknown);
        }
        self.push(value);
    }

    fn float_width(operand: &Operand) -> usize {
        match operand {
            Operand::Memory { width: Some(width), .. } => *width,
            _ => 8,
        }
    }

    /// Runs one instruction; at a return, what it pops.
    fn step(&mut self, line: &Line) -> Option<i32> {
        let operands = &line.operands;
        // A pop into a scratch register discards an argument; into BP, SI or DI it is the epilogue.
        let cleaning = matches!((line.op.as_str(), operands.as_slice()), ("add", [Operand::Register("sp"), _]) | ("pop", [Operand::Register("ax" | "bx" | "cx" | "dx" | "eax" | "ebx" | "ecx" | "edx")]));
        if self.pending.is_some() && !cleaning {
            // What the callee and the caller popped between them, the call's pushes.
            self.pending = None;
            let sp = self.base;
            self.set("sp", address(Base::Stack, sp, 2));
        }
        match (line.op.as_str(), operands.as_slice()) {
            // Straight-line code only: a branch ends the run.
            (op, [Operand::Target(..)]) if op.starts_with('j') || op.starts_with("loop") => {
                self.cut = true;
                return Some(0);
            }
            ("ret" | "retf" | "iret", rest) => {
                self.iret = line.op == "iret";
                let popped = match rest {
                    [Operand::Immediate(value)] => *value as i32,
                    _ => 0,
                };
                return Some(popped);
            }
            ("nop" | "fwait" | "wait" | "cld", _) => {
                if line.op == "cld" {
                    self.forward = true;
                }
            }
            ("std", _) => self.forward = false,
            ("pushf", []) => self.push(vec![Byte::Entry("flags", 0), Byte::Entry("flags", 1)]),
            ("pushad", []) => {
                for one in ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"] {
                    let value = self.get(one);
                    self.push(value);
                }
            }
            ("popad", []) => {
                for one in ["edi", "esi", "ebp", "esp", "ebx", "edx", "ecx", "eax"] {
                    let value = self.pop(4);
                    if one != "esp" {
                        self.set(one, value);
                    }
                }
            }
            ("movsb" | "movsw" | "movsd", []) => self.string_move(&line.op, 1),
            ("rep", [Operand::Memory { name: Some(op), registers, .. }]) if registers.is_empty() && op.starts_with("movs") => {
                let count = known(&self.get("cx")).unwrap_or(0);
                self.string_move(op, count);
                self.set("cx", constant(0, 2));
            }
            ("push", [one]) => {
                let bytes = self.operand_width(one, None).max(2);
                let value = self.read(one, bytes);
                self.push(value);
            }
            ("pushw", [one]) => {
                let value = self.read(one, 2);
                self.push(value);
            }
            ("pushd", [one]) => {
                let value = self.read(one, 4);
                self.push(value);
            }
            ("pop", [one]) => {
                let bytes = self.operand_width(one, None).max(2);
                let value = self.pop(bytes);
                if let (Some(call), true) = (self.pending, cleaning) {
                    self.calls[call].popped += bytes as i32;
                }
                self.write(one, value);
            }
            ("mov", [to, from]) => {
                let bytes = self.operand_width(to, Some(from));
                let value = self.read(from, bytes);
                self.write(to, value);
            }
            ("lea", [to, from]) => {
                let bytes = self.operand_width(to, None);
                let mut value = match self.place(from) {
                    Place::Stack(offset) => address(Base::Stack, offset, 2),
                    Place::Global(name, offset) => address(Base::Global(name), offset, 2),
                    _ => vec![Byte::Unknown; 2],
                };
                value.resize(bytes, Byte::Unknown);
                self.write(to, value);
            }
            ("les" | "lds", [to, from]) => {
                let value = self.read(from, 4);
                self.write(to, value[..2].to_vec());
                self.set(if line.op == "les" { "es" } else { "ds" }, value[2..].to_vec());
            }
            ("movzx" | "movsx", [to, from]) => {
                let bytes = self.operand_width(to, None);
                let narrow = self.operand_width(from, None);
                let value = self.read(from, narrow);
                let fill = if line.op == "movzx" { Byte::Const(0) } else { sign(value.last().expect("a byte")) };
                let mut wide = value;
                wide.resize(bytes, fill);
                self.write(to, wide);
            }
            ("cbw", []) => {
                let al = self.get("al");
                self.set("ah", vec![sign(&al[0])]);
            }
            ("cwd", []) => {
                let ax = self.get("ax");
                let fill = sign(&ax[1]);
                self.set("dx", vec![fill.clone(), fill]);
            }
            ("cwde", []) => {
                let ax = self.get("ax");
                let fill = sign(&ax[1]);
                self.set("eax", [ax, vec![fill.clone(), fill]].concat());
            }
            ("xor" | "sub", [Operand::Register(to), Operand::Register(from)]) if to == from => {
                let bytes = self.operand_width(&operands[0], None);
                self.write(&operands[0], constant(0, bytes));
            }
            ("add" | "sub", [Operand::Register("sp"), Operand::Immediate(amount)]) => {
                let amount = if line.op == "add" { *amount } else { -amount };
                let sp = self.sp + amount as i32;
                if let Some(call) = self.pending {
                    self.calls[call].popped += amount as i32;
                }
                self.set("sp", address(Base::Stack, sp, 2));
            }
            ("add" | "sub" | "inc" | "dec", [to, rest @ ..]) => {
                let bytes = self.operand_width(to, rest.first());
                let amount = match (line.op.as_str(), rest) {
                    ("inc", _) => Some(1),
                    ("dec", _) => Some(-1),
                    (op, [Operand::Immediate(value)]) => Some(if op == "add" { *value } else { -value }),
                    _ => None,
                };
                let value = self.read(to, bytes);
                let result = match amount {
                    Some(amount) => Self::add_constant(&value, amount),
                    None => vec![Byte::Unknown; bytes],
                };
                self.write(to, result);
            }
            ("shl", [Operand::Register(to), Operand::Immediate(16)]) if to.starts_with('e') => {
                let value = self.get(to);
                self.set(to, [vec![Byte::Const(0), Byte::Const(0)], value[..2].to_vec()].concat());
            }
            ("shr", [Operand::Register(to), Operand::Immediate(16)]) if to.starts_with('e') => {
                let value = self.get(to);
                self.set(to, [value[2..].to_vec(), vec![Byte::Const(0), Byte::Const(0)]].concat());
            }
            ("shrd", [Operand::Register(to), Operand::Register(from), Operand::Immediate(16)]) => {
                let (value, other) = (self.get(to), self.get(from));
                self.set(to, [value[2..].to_vec(), other[..2].to_vec()].concat());
            }
            ("shld", [Operand::Register(to), Operand::Register(from), Operand::Immediate(16)]) => {
                let (value, other) = (self.get(to), self.get(from));
                self.set(to, [other[2..].to_vec(), value[..2].to_vec()].concat());
            }
            ("or", [Operand::Register(to), Operand::Register(from)]) if to == from => {}
            ("xchg", [one, other]) => {
                let bytes = self.operand_width(one, Some(other));
                let (first, second) = (self.read(one, bytes), self.read(other, bytes));
                self.write(one, second);
                self.write(other, first);
            }
            ("leave", []) => {
                let bp = self.get("bp");
                self.set("sp", bp);
                let value = self.pop(2);
                self.set("bp", value);
            }
            // The runtime frame: B$ENRA saves BP as a prologue would and
            // points BP at it; B$EXSA restores BP, SI and DI and leaves SP
            // at the return address.
            ("call", [Operand::Target(target, _)]) if target == "b$enra" => {
                let bp = self.get("bp");
                self.store(&Place::Stack(-2), bp);
                self.set("bp", address(Base::Stack, -2, 2));
                // Below any frame a BASIC procedure has: its locals stay apart from what it pushes.
                self.set("sp", address(Base::Stack, -0x800, 2));
                self.base = self.sp;
            }
            ("call", [Operand::Target(target, _)]) if target == "b$exsa" => {
                for one in ["ebp", "esi", "edi"] {
                    let entry = (0..4).map(|at| Byte::Entry(one, at)).collect();
                    self.registers.insert(one, entry);
                }
                self.set("sp", address(Base::Stack, 0, 2));
            }
            ("call", [Operand::Target(target, far)]) => {
                if target == "f_spush@" || target == "n_spush@" {
                    self.struct_push();
                } else {
                    // `push cs` then a near call: a far call into the same segment.
                    let pushed_cs = far != &Some(true) && self.sp < self.base && self.load(&Place::Stack(self.sp), 2) == self.get("cs");
                    if pushed_cs {
                        let sp = self.sp + 2;
                        self.set("sp", address(Base::Stack, sp, 2));
                    }
                    self.call(target, far.unwrap_or(false) || pushed_cs);
                }
            }
            ("fld" | "fild", [one]) if matches!(one, Operand::Memory { .. }) => {
                let bytes = Self::float_width(one);
                let value = self.read(one, bytes);
                self.floats.push(if line.op == "fld" { float_value(value) } else { vec![Byte::Unknown; 10] });
            }
            ("fld", [Operand::Float(at)]) => {
                let value = self.floats.iter().rev().nth(*at).cloned().unwrap_or_default();
                self.floats.push(value);
            }
            ("fld1" | "fldz" | "fldpi", []) => self.floats.push(vec![Byte::Unknown; 10]),
            ("fstp" | "fst" | "fistp" | "fist", [one]) => {
                let top = self.floats.last().cloned().unwrap_or_default();
                if let Operand::Memory { .. } = one {
                    let bytes = Self::float_width(one);
                    let returned = top.iter().all(|one| matches!(one, Byte::Returned(..)));
                    let value = match () {
                        _ if line.op.starts_with("fist") => vec![Byte::Unknown; bytes],
                        _ if top.len() == bytes => top,
                        // A result in st(0) is stored at whatever width the caller keeps.
                        _ if returned => top[..bytes.min(top.len())].to_vec(),
                        _ => converted(&top, bytes),
                    };
                    self.write(one, value);
                } else if let Operand::Float(at) = one {
                    let length = self.floats.len();
                    if *at > 0 && *at < length {
                        self.floats[length - 1 - at] = top;
                    }
                }
                if line.op.ends_with('p') {
                    self.floats.pop();
                }
            }
            ("fxch", rest) => {
                let at = match rest {
                    [Operand::Float(at)] => *at,
                    _ => 1,
                };
                let length = self.floats.len();
                if at < length {
                    self.floats.swap(length - 1, length - 1 - at);
                }
            }
            (op, rest) if op.starts_with('f') => {
                // Arithmetic: a new value. A popping form drops one.
                if op.ends_with('p') && self.floats.len() > 1 {
                    self.floats.pop();
                }
                if let Some(top) = self.floats.last_mut() {
                    *top = vec![Byte::Unknown; 10];
                } else if rest.is_empty() {
                    self.floats.push(vec![Byte::Unknown; 10]);
                }
            }
            (_, [to, ..]) if matches!(to, Operand::Register(_) | Operand::Memory { .. }) => {
                let bytes = self.operand_width(to, operands.get(1));
                self.write(to, vec![Byte::Unknown; bytes]);
            }
            _ => panic!("{}: cannot run {line:?}", self.name),
        }
        None
    }
}

/// The x87 value `top` stored `bytes` wide.
fn converted(top: &[Byte], bytes: usize) -> Vec<Byte> {
    if top.iter().any(|one| matches!(one, Byte::Unknown)) {
        return vec![Byte::Unknown; bytes];
    }
    (0..bytes as u8).map(|at| Byte::Converted(top.to_vec(), bytes as u8, at)).collect()
}

/// The x87 value these stored bytes hold: what a converted store kept.
fn float_value(bytes: Vec<Byte>) -> Vec<Byte> {
    match bytes.first() {
        Some(Byte::Converted(source, width, 0)) if bytes.len() == *width as usize && bytes.iter().enumerate().all(|(at, one)| matches!(one, Byte::Converted(other, _, index) if other == source && *index as usize == at)) => source.clone(),
        _ => bytes,
    }
}

fn sign(byte: &Byte) -> Byte {
    match byte {
        Byte::Const(value) => Byte::Const(if *value & 0x80 != 0 { 0xFF } else { 0 }),
        Byte::Sign(inner) => Byte::Sign(inner.clone()),
        other => Byte::Sign(Box::new(other.clone())),
    }
}
