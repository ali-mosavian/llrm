//! Clock counts of the 68000 and 68020 for one instruction, from Motorola's
//! published tables. A measuring instrument: nothing in the compiler calls it.
//!
//! # How it works
//! [`cost`] reads an instruction in the pin's assembler syntax (what `asm`
//! prints and `llvm-objdump` shows) and returns its clock periods.
//! - 68000: MC68000UM section 8 ("16-bit instruction execution times"), Tables
//!   8-1 to 8-12. A move is `4 + source + destination`, which gives every cell
//!   of Tables 8-2 and 8-3.
//! - 68020: MC68020UM section 8, the cache case column (instruction cache hit,
//!   no wait states). The manual says its tables bound the real time and do not
//!   predict it, since overlap and alignment move it; the move matrix is copied
//!   from section 8.2.6.
//!
//! A branch is priced taken or not by the caller's `taken`. A [`Cost`] with
//! `partial` set leaves out a data-dependent term: a shift by a register
//! counts no places, a multiply or divide is the published maximum.
//!
//! Instructions the pin cannot assemble (`addq`, `tst`, `pea`...) are priced
//! too, for the reference compilers' code. Anything else is an error, never
//! zero clocks.
//!
//! # See also
//! `tests/timing.rs`: the cells it checks, with table and page.

mod m68010;
mod m68030;
mod m68040;
mod m68060;
mod table;

use crate::dagisel::hooks::Cpu as SelectCpu;
use crate::isa::Cpu;

/// Clock periods of one instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cost {
    pub clocks: u32,
    /// A data-dependent term is at its lower bound (shift by register) or its
    /// published maximum (multiply, divide).
    pub partial: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Dn,
    An,
    Ind,
    Post,
    Pre,
    Disp,
    Index,
    AbsW,
    AbsL,
    PcDisp,
    PcIndex,
    Imm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Size {
    B,
    W,
    L,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Operand {
    /// The mode and, for an immediate, its value.
    Mode(Mode, i64),
    /// A register list of `n` registers.
    List(u32),
    /// A branch or call target: a label or an address.
    Target,
    /// `%sr` or `%ccr`.
    Status,
    /// `%usp`.
    Usp,
    /// Any other control register (`%vbr`, `%cacr`, ...).
    Control,
}

fn split_operands(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (at, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(text[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    if !text[start..].trim().is_empty() {
        parts.push(text[start..].trim());
    }
    parts
}

fn is_data(register: &str) -> bool {
    register.len() == 3 && register.starts_with("%d") && register.as_bytes()[2].is_ascii_digit()
}

fn is_address(register: &str) -> bool {
    (register.len() == 3 && register.starts_with("%a") && register.as_bytes()[2].is_ascii_digit())
        || register == "%sp"
        || register == "%fp"
}

/// The number of registers in `%d2-%d4/%a2`.
fn list_size(text: &str) -> Option<u32> {
    let mut count = 0;
    for part in text.split('/') {
        match part.split_once('-') {
            Some((from, to)) => {
                let number = |register: &str| register.get(2..)?.parse::<u32>().ok();
                count += number(to)?.checked_sub(number(from)?)? + 1;
            }
            None if is_data(part) || is_address(part) => count += 1,
            None => return None,
        }
    }
    Some(count)
}

fn operand(text: &str) -> Result<Operand, String> {
    let unsupported = || format!("operand `{text}` is not in the tables");
    // a bit field's `{offset:width}` and a register pair `%d1:%d0` do not
    // change the operand's mode
    if let Some((inner, field)) = text.split_once('{').filter(|(_, field)| field.ends_with('}')) {
        let _ = field;
        return operand(inner);
    }
    if text.split_once(':').is_some_and(|(high, low)| is_data(high) && is_data(low)) {
        return Ok(Operand::Mode(Mode::Dn, 0));
    }
    if is_data(text) {
        return Ok(Operand::Mode(Mode::Dn, 0));
    }
    if is_address(text) {
        return Ok(Operand::Mode(Mode::An, 0));
    }
    if text == "%sr" || text == "%ccr" {
        return Ok(Operand::Status);
    }
    if text == "%usp" {
        return Ok(Operand::Usp);
    }
    if let Some(value) = text.strip_prefix('#') {
        return Ok(Operand::Mode(Mode::Imm, value.parse().unwrap_or(0)));
    }
    if text.starts_with('%') && (text.contains('/') || text.contains('-')) {
        return list_size(text).map(Operand::List).ok_or_else(unsupported);
    }
    if text.starts_with('%') && text[1..].chars().all(|c| c.is_ascii_lowercase()) {
        return Ok(Operand::Control);
    }
    if let Some(inner) = text.strip_prefix("-(").and_then(|rest| rest.strip_suffix(')')) {
        return if is_address(inner) { Ok(Operand::Mode(Mode::Pre, 0)) } else { Err(unsupported()) };
    }
    if let Some(inner) = text.strip_prefix('(').and_then(|rest| rest.strip_suffix(")+")) {
        return if is_address(inner) { Ok(Operand::Mode(Mode::Post, 0)) } else { Err(unsupported()) };
    }
    if let Some(inner) = text.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        return match parts.as_slice() {
            [base] if is_address(base) => Ok(Operand::Mode(Mode::Ind, 0)),
            [_, base] if is_address(base) => Ok(Operand::Mode(Mode::Disp, 0)),
            [_, "%pc"] => Ok(Operand::Mode(Mode::PcDisp, 0)),
            [_, base, _] if is_address(base) => Ok(Operand::Mode(Mode::Index, 0)),
            [_, "%pc", _] => Ok(Operand::Mode(Mode::PcIndex, 0)),
            _ => Err(unsupported()),
        };
    }
    if text.strip_suffix(":l").is_some_and(|address| address.starts_with("0x")) {
        return Ok(Operand::Mode(Mode::AbsL, 0));
    }
    if text.strip_suffix(":w").is_some_and(|address| address.starts_with("0x")) {
        return Ok(Operand::Mode(Mode::AbsW, 0));
    }
    let first = text.chars().next();
    if first == Some('$')
        || text.starts_with("0x")
        || text.chars().all(|c| c.is_ascii_hexdigit())
        || first == Some('.')
        || first.is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return Ok(Operand::Target);
    }
    Err(unsupported())
}

/// The mode of a register or memory operand.
fn mode(operand: &Operand) -> Option<Mode> {
    match operand {
        Operand::Mode(mode, _) => Some(*mode),
        _ => None,
    }
}

fn is_register_or_immediate(mode: Mode) -> bool {
    matches!(mode, Mode::Dn | Mode::An | Mode::Imm)
}

fn size_of(suffix: Option<&str>) -> Size {
    match suffix {
        Some("b") => Size::B,
        Some("l") => Size::L,
        _ => Size::W,
    }
}

/// The places a shift by an immediate moves; `llvm-objdump` prints the quick
/// count 8 as the encoded field 0.
fn places(operands: &[Operand]) -> Option<u32> {
    match operands {
        [Operand::Mode(Mode::Imm, count), _] => Some(if *count == 0 { 8 } else { *count as u32 }),
        _ => None,
    }
}

fn is_shift(base: &str) -> bool {
    matches!(
        base,
        "asl" | "asr" | "lsl" | "lsr" | "rol" | "ror" | "roxl" | "roxr"
    )
}

fn condition_branch(base: &str) -> bool {
    matches!(
        base,
        "bhi"
            | "bls"
            | "bcc"
            | "bcs"
            | "bne"
            | "beq"
            | "bvc"
            | "bvs"
            | "bpl"
            | "bmi"
            | "bge"
            | "blt"
            | "bgt"
            | "ble"
            | "bhs"
            | "blo"
    )
}

fn set_on_condition(base: &str) -> bool {
    matches!(
        base,
        "st" | "sf"
            | "shi"
            | "sls"
            | "scc"
            | "scs"
            | "sne"
            | "seq"
            | "svc"
            | "svs"
            | "spl"
            | "smi"
            | "sge"
            | "slt"
            | "sgt"
            | "sle"
    )
}

/// The clock periods of `text` on `cpu`. `bytes` is its encoded length (a
/// conditional branch has a byte or a word displacement by it) and `taken`
/// whether a branch was taken.
pub fn cost(
    cpu: SelectCpu,
    text: &str,
    bytes: u32,
    taken: bool,
) -> Result<Cost, String> {
    cost_on(
        match cpu {
            SelectCpu::M68000 => Cpu::M68000,
            SelectCpu::M68020 => Cpu::M68020,
        },
        text,
        bytes,
        taken,
    )
}

/// [`cost`] for any CPU of the family.
pub fn cost_on(
    cpu: Cpu,
    text: &str,
    bytes: u32,
    taken: bool,
) -> Result<Cost, String> {
    let text = text.trim();
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let (base, suffix) = match name.split_once('.') {
        Some((base, suffix)) => (base, Some(suffix)),
        None => (name, None),
    };
    let mut operands: Vec<Operand> = split_operands(rest).into_iter().map(operand).collect::<Result<_, _>>()?;
    if base == "movem" {
        // a list of one register reads as a register operand
        for one in &mut operands {
            if matches!(one, Operand::Mode(Mode::Dn | Mode::An, _)) {
                *one = Operand::List(1);
            }
        }
    }
    let clocks = match cpu {
        Cpu::M68000 => clocks68000(base, suffix, &operands, bytes, taken),
        Cpu::M68010 => m68010::clocks(base, suffix, &operands, bytes, taken),
        Cpu::M68020 => clocks68020(base, suffix, &operands, bytes, taken),
        Cpu::M68030 => m68030::clocks(base, suffix, &operands, bytes, taken),
        Cpu::M68040 => m68040::clocks(base, suffix, &operands, bytes, taken),
        Cpu::M68060 => m68060::clocks(base, suffix, &operands, rest, taken),
    }
    .ok_or_else(|| format!("`{text}` is not in the {cpu:?} tables"))?;
    let by_register = is_shift(base)
        && matches!(cpu, Cpu::M68000 | Cpu::M68010)
        && places(&operands).is_none()
        && operands.len() == 2;
    let partial = if cpu == Cpu::M68060 {
        m68060::partial(base, suffix)
    } else {
        matches!(base, "mulu" | "muls" | "divu" | "divs")
            || by_register
            || (set_on_condition(base) && cpu == Cpu::M68000)
    };
    Ok(Cost { clocks, partial })
}

/// MC68000UM Table 8-1 (p. 8-1): the effective address calculation.
fn ea68000(
    mode: Mode,
    size: Size,
) -> u32 {
    let (word, long) = match mode {
        Mode::Dn | Mode::An => (0, 0),
        Mode::Ind | Mode::Post => (4, 8),
        Mode::Pre => (6, 10),
        Mode::Disp | Mode::AbsW | Mode::PcDisp => (8, 12),
        Mode::Index | Mode::PcIndex => (10, 14),
        Mode::AbsL => (12, 16),
        Mode::Imm => (4, 8),
    };
    if size == Size::L { long } else { word }
}

fn clocks68000(
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    bytes: u32,
    taken: bool,
) -> Option<u32> {
    let size = size_of(suffix);
    let long = size == Size::L;
    let wide = |word: u32, long_time: u32| if long { long_time } else { word };
    match (base, operands) {
        ("move" | "movea", [source, Operand::Status]) => Some(12 + ea68000(mode(source)?, Size::W)),
        ("move", [Operand::Status, destination]) => match mode(destination)? {
            Mode::Dn => Some(6),
            memory => Some(8 + ea68000(memory, Size::W)),
        },
        ("move" | "movea", [source, destination]) => {
            // Tables 8-2, 8-3: `-(An)` as a destination costs what `(An)` does
            let destination = match mode(destination)? {
                Mode::Pre => Mode::Ind,
                other => other,
            };
            Some(4 + ea68000(mode(source)?, size) + ea68000(destination, size))
        }
        ("moveq", [_, _]) => Some(4),
        ("add" | "sub" | "adda" | "suba" | "and" | "or" | "eor" | "cmp" | "cmpa", [source, destination]) => {
            let (source, destination) = (mode(source)?, mode(destination)?);
            match (base, destination) {
                ("eor", Mode::Dn) => Some(wide(4, 8)),
                ("eor", memory) => Some(wide(8, 12) + ea68000(memory, size)),
                (_, Mode::An) => {
                    let table = match (base, long, is_register_or_immediate(source)) {
                        ("cmp" | "cmpa", _, _) => 6,
                        (_, false, _) | (_, true, true) => 8,
                        (_, true, false) => 6,
                    };
                    Some(table + ea68000(source, if long { Size::L } else { Size::W }))
                }
                // `ADDI`, `SUBI`, `ANDI`, `ORI`: Table 8-5 (p. 8-5)
                (_, Mode::Dn) if source == Mode::Imm && base != "cmp" => {
                    Some(wide(8, if base == "and" { 14 } else { 16 }))
                }
                (_, Mode::Dn) => {
                    let table = match (long, is_register_or_immediate(source)) {
                        (false, _) => 4,
                        (true, false) => 6,
                        (true, true) => 8,
                    };
                    Some(table + ea68000(source, size))
                }
                (_, memory) if source == Mode::Dn => Some(wide(8, 12) + ea68000(memory, size)),
                (_, memory) if source == Mode::Imm => Some(wide(12, 20) + ea68000(memory, size)),
                _ => None,
            }
        }
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => match mode(destination)? {
            Mode::Dn => Some(wide(8, if base == "andi" || base == "cmpi" { 14 } else { 16 })),
            memory if base == "cmpi" => Some(wide(8, 12) + ea68000(memory, size)),
            memory => Some(wide(12, 20) + ea68000(memory, size)),
        },
        ("addq" | "subq", [_, destination]) => match mode(destination)? {
            Mode::Dn => Some(wide(4, 8)),
            Mode::An => Some(8),
            memory => Some(wide(8, 12) + ea68000(memory, size)),
        },
        ("clr" | "neg" | "negx" | "not", [destination]) => match mode(destination)? {
            Mode::Dn => Some(wide(4, 6)),
            memory => Some(wide(8, 12) + ea68000(memory, size)),
        },
        ("tst", [destination]) => match mode(destination)? {
            Mode::Dn => Some(4),
            other => Some(4 + ea68000(other, size)),
        },
        ("ext" | "swap" | "nop", _) => Some(4),
        ("exg", _) => Some(6),
        (shift, [_, _]) if is_shift(shift) => Some(wide(6, 8) + 2 * places(operands).unwrap_or(0)),
        (shift, [memory]) if is_shift(shift) => Some(8 + ea68000(mode(memory)?, Size::W)),
        ("mulu" | "muls", [source, _]) => Some(70 + ea68000(mode(source)?, Size::W)),
        ("divu", [source, _]) => Some(140 + ea68000(mode(source)?, Size::W)),
        ("divs", [source, _]) => Some(158 + ea68000(mode(source)?, Size::W)),
        ("lea", [source, _]) => match mode(source)? {
            Mode::Ind => Some(4),
            Mode::Disp | Mode::AbsW | Mode::PcDisp => Some(8),
            Mode::Index | Mode::AbsL | Mode::PcIndex => Some(12),
            _ => None,
        },
        ("pea", [source]) => match mode(source)? {
            Mode::Ind => Some(12),
            Mode::Disp | Mode::AbsW | Mode::PcDisp => Some(16),
            Mode::Index | Mode::AbsL | Mode::PcIndex => Some(20),
            _ => None,
        },
        ("jmp" | "jsr", [target]) => {
            let (ind, short, index, absolute) = if base == "jmp" { (8, 10, 14, 12) } else { (16, 18, 22, 20) };
            match target {
                // a call to an address is absolute short in four bytes, long in
                // six
                Operand::Target => Some(if bytes == 4 { short } else { absolute }),
                other => match mode(other)? {
                    Mode::Ind => Some(ind),
                    Mode::Disp | Mode::AbsW | Mode::PcDisp => Some(short),
                    Mode::Index | Mode::PcIndex => Some(index),
                    Mode::AbsL => Some(absolute),
                    _ => None,
                },
            }
        }
        ("bra", [_]) => Some(10),
        ("bsr", [_]) => Some(18),
        (condition, [_]) if condition_branch(condition) => Some(match (taken, bytes) {
            (true, _) => 10,
            (false, 0..=2) => 8,
            (false, _) => 12,
        }),
        ("dbf" | "dbra" | "dbt" | "dbne" | "dbeq", [_, _]) => Some(if taken { 10 } else { 14 }),
        ("rts", []) => Some(16),
        ("link", [_, _]) => Some(16),
        ("unlk", [_]) => Some(12),
        (set, [destination]) if set_on_condition(set) => match mode(destination)? {
            Mode::Dn => Some(6),
            memory => Some(8 + ea68000(memory, Size::B)),
        },
        // Table 8-10 (p. 8-8): register to memory, then memory to register
        ("movem", [Operand::List(registers), memory]) => {
            let base = match mode(memory)? {
                Mode::Ind | Mode::Pre => 8,
                Mode::Disp | Mode::AbsW => 12,
                Mode::Index => 14,
                Mode::AbsL => 16,
                _ => return None,
            };
            Some(base + wide(4, 8) * registers)
        }
        ("movem", [memory, Operand::List(registers)]) => {
            let base = match mode(memory)? {
                Mode::Ind | Mode::Post => 12,
                Mode::Disp | Mode::AbsW | Mode::PcDisp => 16,
                Mode::Index | Mode::PcIndex => 18,
                Mode::AbsL => 20,
                _ => return None,
            };
            Some(base + wide(4, 8) * registers)
        }
        _ => None,
    }
}

/// MC68020UM 8.2.1 (p. 8-13), cache case: fetching an operand's address and
/// value.
fn fetch68020(
    mode: Mode,
    size: Size,
) -> u32 {
    match mode {
        Mode::Dn | Mode::An => 0,
        Mode::Ind | Mode::Post | Mode::AbsW | Mode::AbsL => 4,
        Mode::Pre | Mode::Disp | Mode::PcDisp => 5,
        Mode::Index | Mode::PcIndex => 7,
        Mode::Imm if size == Size::L => 4,
        Mode::Imm => 2,
    }
}

/// MC68020UM 8.2.2 (p. 8-14), cache case: fetching an immediate and the
/// address of its memory destination (a byte or word immediate is the `.W`
/// row).
fn fetch_immediate68020(
    mode: Mode,
    size: Size,
) -> Option<u32> {
    let long = size == Size::L;
    let (word, long_time) = match mode {
        Mode::Dn => (2, 4),
        Mode::Ind => (4, 4),
        Mode::Post => (6, 8),
        Mode::Pre => (5, 7),
        Mode::Disp | Mode::AbsW => (5, 7),
        Mode::AbsL => (6, 8),
        Mode::Index => (9, 11),
        _ => return None,
    };
    Some(if long { long_time } else { word })
}

/// MC68020UM 8.2.3 (p. 8-16), cache case: calculating an address only.
fn calculate68020(mode: Mode) -> Option<u32> {
    match mode {
        Mode::Ind | Mode::Post | Mode::Pre | Mode::Disp | Mode::PcDisp | Mode::AbsW => Some(2),
        Mode::AbsL | Mode::Index | Mode::PcIndex => Some(4),
        _ => None,
    }
}

/// MC68020UM 8.2.5 (p. 8-19), cache case: a jump address.
fn jump68020(mode: Mode) -> Option<u32> {
    match mode {
        Mode::Ind | Mode::AbsW | Mode::AbsL => Some(2),
        Mode::Disp | Mode::PcDisp => Some(4),
        Mode::Index | Mode::PcIndex => Some(6),
        _ => None,
    }
}

/// MC68020UM 8.2.4 (p. 8-17), cache case: the register mask word and the
/// address of a `movem`.
fn movem_address68020(mode: Mode) -> Option<u32> {
    match mode {
        Mode::Ind => Some(2),
        Mode::Post | Mode::Pre | Mode::Disp | Mode::PcDisp => Some(4),
        _ => None,
    }
}

/// MC68020UM 8.2.6 (pp. 8-23, 8-24), cache case: a move, by source row and
/// destination column.
fn move68020(
    source: Mode,
    size: Size,
    destination: Mode,
) -> Option<u32> {
    let row = match source {
        Mode::Dn | Mode::An => 0,
        Mode::Imm if size != Size::L => 1,
        Mode::Imm => 2,
        Mode::Ind => 3,
        Mode::Post => 4,
        Mode::Pre => 5,
        Mode::Disp | Mode::PcDisp => 6,
        Mode::AbsW => 7,
        Mode::AbsL => 8,
        Mode::Index | Mode::PcIndex => 9,
    };
    let column = match destination {
        Mode::An => 0,
        Mode::Dn => 1,
        Mode::Ind => 2,
        Mode::Post => 3,
        Mode::Pre => 4,
        Mode::Disp | Mode::PcDisp => 5,
        Mode::AbsW => 6,
        Mode::AbsL => 7,
        Mode::Index | Mode::PcIndex => 8,
        Mode::Imm => return None,
    };
    //     An  Dn  (An) (An)+ -(An) d16 .W .L d8
    const CELLS: [[u32; 9]; 10] = [
        [2, 2, 4, 4, 5, 5, 4, 6, 7],
        [4, 4, 6, 6, 7, 7, 6, 8, 7],
        [6, 6, 8, 8, 9, 9, 8, 10, 9],
        [6, 6, 7, 7, 7, 7, 7, 9, 9],
        [6, 6, 7, 7, 7, 7, 7, 9, 9],
        [7, 7, 8, 8, 8, 8, 8, 10, 10],
        [7, 7, 8, 8, 8, 8, 8, 10, 10],
        [6, 6, 7, 7, 7, 7, 7, 9, 9],
        [6, 6, 7, 7, 7, 7, 7, 9, 9],
        [9, 9, 10, 10, 10, 10, 10, 12, 12],
    ];
    Some(CELLS[row][column])
}

fn clocks68020(
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    bytes: u32,
    taken: bool,
) -> Option<u32> {
    let size = size_of(suffix);
    let long = size == Size::L;
    let wide = |word: u32, long_time: u32| if long { long_time } else { word };
    match (base, operands) {
        ("move" | "movea", [source, Operand::Status]) => Some(4 + fetch68020(mode(source)?, Size::W)),
        ("move", [Operand::Status, destination]) => match mode(destination)? {
            Mode::Dn => Some(4),
            memory => Some(5 + calculate68020(memory)?),
        },
        ("move" | "movea", [source, destination]) => move68020(mode(source)?, size, mode(destination)?),
        ("moveq", [_, _]) => Some(2),
        ("add" | "sub" | "adda" | "suba" | "and" | "or" | "eor" | "cmp" | "cmpa", [source, destination]) => {
            let (source, destination) = (mode(source)?, mode(destination)?);
            match (base, destination) {
                ("eor", Mode::Dn) => Some(2),
                (_, Mode::An) => Some(if base == "cmp" || base == "cmpa" { 4 } else { 2 } + fetch68020(source, size)),
                (_, Mode::Dn) => Some(2 + fetch68020(source, size)),
                (_, memory) if source == Mode::Dn => Some(4 + fetch68020(memory, size)),
                (_, memory) if source == Mode::Imm => Some(4 + fetch_immediate68020(memory, size)?),
                _ => None,
            }
        }
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => {
            let to_memory = if base == "cmpi" { 2 } else { 4 };
            match mode(destination)? {
                Mode::Dn => Some(2 + fetch_immediate68020(Mode::Dn, size)?),
                memory => Some(to_memory + fetch_immediate68020(memory, size)?),
            }
        }
        ("addq" | "subq", [_, destination]) => match mode(destination)? {
            Mode::Dn | Mode::An => Some(2),
            memory => Some(4 + fetch68020(memory, size)),
        },
        ("clr" | "neg" | "negx" | "not", [destination]) => match mode(destination)? {
            Mode::Dn => Some(2),
            memory => Some(4 + fetch68020(memory, size)),
        },
        ("tst", [destination]) => Some(2 + fetch68020(mode(destination)?, size)),
        ("ext" | "swap", _) => Some(4),
        ("nop" | "exg", _) => Some(2),
        ("lsl" | "lsr", [Operand::Mode(Mode::Imm, _), _]) => Some(4),
        ("lsl" | "lsr", [_, _]) => Some(6),
        ("asl", [_, _]) => Some(8),
        ("asr", [_, _]) => Some(6),
        ("rol" | "ror", [_, _]) => Some(8),
        ("roxl" | "roxr", [_, _]) => Some(12),
        ("mulu" | "muls", [source, _]) => Some(wide(27, 43) + fetch68020(mode(source)?, Size::W)),
        ("divu", [source, _]) => Some(wide(44, 78) + fetch68020(mode(source)?, Size::W)),
        ("divs", [source, _]) => Some(wide(56, 90) + fetch68020(mode(source)?, Size::W)),
        ("lea", [source, _]) => Some(2 + calculate68020(mode(source)?)?),
        ("pea", [source]) => Some(5 + calculate68020(mode(source)?)?),
        ("jmp" | "jsr", [target]) => {
            let address = match target {
                Operand::Target => 2,
                other => jump68020(mode(other)?)?,
            };
            Some(address + if base == "jmp" { 4 } else { 5 })
        }
        ("bra", [_]) => Some(6),
        ("bsr", [_]) => Some(7),
        (condition, [_]) if condition_branch(condition) => Some(match (taken, bytes) {
            (true, _) => 6,
            (false, 0..=2) => 4,
            (false, _) => 6,
        }),
        ("dbf" | "dbra" | "dbt" | "dbne" | "dbeq", [_, _]) => Some(if taken { 6 } else { 10 }),
        ("rts", []) => Some(10),
        ("link", [_, _]) => Some(wide(5, 6)),
        ("unlk", [_]) => Some(6),
        (set, [destination]) if set_on_condition(set) => match mode(destination)? {
            Mode::Dn => Some(4),
            memory => Some(6 + calculate68020(memory)?),
        },
        ("movem", [Operand::List(registers), memory]) => Some(4 + 3 * registers + movem_address68020(mode(memory)?)?),
        ("movem", [memory, Operand::List(registers)]) => Some(8 + 4 * registers + movem_address68020(mode(memory)?)?),
        _ => None,
    }
}

const BASES: [&str; 62] = [
    "movea", "move", "moveq", "movem", "adda", "addi", "addq", "addx", "add", "suba", "subi", "subq", "subx", "sub",
    "andi", "and", "ori", "or", "eori", "eor", "cmpa", "cmpi", "cmpm", "cmp", "clr", "negx", "neg", "not", "tst",
    "ext", "swap", "lsl", "lsr", "asl", "asr", "rol", "ror", "roxl", "roxr", "link", "unlk", "lea", "pea", "jsr",
    "jmp", "rts", "bra", "bsr", "mulu", "muls", "divu", "divs", "exg", "nop", "dbf", "dbra", "dbt", "dbne", "dbeq",
    "scc", "seq", "sne",
];

/// `name` of GNU syntax (`addql`, `bnew`) as the pin's `base.size`.
fn gnu_mnemonic(name: &str) -> String {
    if BASES.contains(&name) || set_on_condition(name) {
        return name.to_owned();
    }
    let split = |name: &str| -> Option<(String, char)> {
        let last = name.chars().last()?;
        let base = &name[..name.len() - 1];
        "bwls".contains(last).then_some((base.to_owned(), last))
    };
    match split(name) {
        Some((base, size)) if BASES.contains(&base.as_str()) || condition_branch(&base) => {
            // a branch's `s`, `w` or `l` is its displacement, not an operand
            // size
            if condition_branch(&base) || base == "bra" || base == "bsr" {
                base
            } else {
                format!("{base}.{}", if size == 's' { 'b' } else { size })
            }
        }
        _ => name.to_owned(),
    }
}

/// An operand of GNU syntax (`%a0@(12,%d2:l)`) as the pin's.
fn gnu_operand(text: &str) -> String {
    let Some((base, rest)) = text.split_once('@') else { return text.to_owned() };
    match rest {
        "" => format!("({base})"),
        "+" => format!("({base})+"),
        "-" => format!("-({base})"),
        _ => {
            let inner = rest.trim_start_matches('(').trim_end_matches(')');
            let mut parts = inner.split(',');
            let displacement = parts.next().unwrap_or("0");
            match parts.next() {
                None => format!("({displacement},{base})"),
                Some(index) => format!("({displacement},{base},{})", index.split(':').next().unwrap_or(index)),
            }
        }
    }
}

/// The pin's syntax for one line of binutils' `objdump -d` (GNU syntax:
/// `movel %a0@(12),%d2`), which disassembles `addq`, `tst` and the rest of
/// what the pin's own `llvm-objdump` shows as `<unknown>`.
pub fn from_gnu(text: &str) -> Result<String, String> {
    // a branch target reads `80000074 <k+0x8>`
    let text = text.split(" <").next().unwrap_or(text).trim();
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let name = gnu_mnemonic(name);
    let operands: Vec<String> = split_operands(rest.trim()).into_iter().map(gnu_operand).collect();
    Ok(if operands.is_empty() { name } else { format!("{name} {}", operands.join(", ")) })
}
