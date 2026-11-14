//! Clock counts of the 68000 family for one instruction, from Motorola's
//! published tables. A measuring instrument: nothing in the compiler calls it.
//!
//! # How it works
//! [`cost`] reads an instruction in the pin's assembler syntax (what `asm`
//! prints and `llvm-objdump` shows) and returns its clock periods.
//! - 68000, 68010: MC68000UM sections 8 and 9, read from their tables
//!   (`family.rs`).
//! - 68030, 68040, 68060: their manuals' tables (`m68030.rs`, `m68040.rs`,
//!   `m68060.rs`).
//! - 68020: MC68020UM section 8, the cache case column (instruction cache hit,
//!   no wait states), in `m68020.rs`. The manual says its tables bound the real
//!   time and do not predict it.
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

mod counts;
mod family;
mod m68020;
mod m68030;
mod m68040;
mod m68060;
mod table;
pub use table::keys_made;

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
    /// The 68020's full-format index with a 16-bit base displacement.
    IndexW,
    /// ... with a 32-bit base displacement.
    IndexL,
    /// ... with the base displacement suppressed.
    IndexB,
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

/// A base displacement as the disassembler prints it: decimal, or `0x` hex.
fn displacement(text: &str) -> Option<i64> {
    let (sign, digits) = text.strip_prefix('-').map_or((1, text), |rest| (-1, rest));
    let value = match digits.strip_prefix("0x") {
        Some(hex) => i64::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<i64>().ok()?,
    };
    Some(sign * value)
}

/// `%d1.l*4`, `%a2.w`, `%d0`: an index register with its size and scale.
fn is_index(text: &str) -> bool {
    let register = text.split('.').next().unwrap_or(text);
    is_data(register) || is_address(register)
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
                // `%fp` and `%sp` end the lists gcc and the pin print
                // (`%d2-%d7/%a2-%fp`): `%a6` and `%a7`.
                let number = |register: &str| match register {
                    "%fp" => Some(6),
                    "%sp" => Some(7),
                    _ => register.get(2..)?.parse::<u32>().ok(),
                };
                count += number(to)?.checked_sub(number(from)?)? + 1;
            }
            None if is_data(part) || is_address(part) => count += 1,
            None => return None,
        }
    }
    Some(count)
}

/// An error that is a parser fault, not a missing table row: the cycles tool stops on it.
pub const UNPARSEABLE: &str = "unparseable displacement";

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
            // a displacement that does not fit the brief format's 8 bits is
            // the full format's base displacement (68020 and up)
            // an empty base is the full format's suppressed one: `@(ffffffff800020a0,%d0:l)`
            [disp, base, _] if is_address(base) || *base == "%pc" || base.is_empty() => match (displacement(disp), *base == "%pc") {
                (Some(d), _) if (-128..=127).contains(&d) && !base.is_empty() => {
                    Ok(Operand::Mode(if *base == "%pc" { Mode::PcIndex } else { Mode::Index }, 0))
                }
                (Some(d), _) if (-32768..=32767).contains(&d) => Ok(Operand::Mode(Mode::IndexW, 0)),
                (Some(_), _) => Ok(Operand::Mode(Mode::IndexL, 0)),
                // a label in the brief form's 8 bits: the table `jmp` and its `move.w` (M68K-WORD-TABLE)
                (None, true) if disp.starts_with('.') => Ok(Operand::Mode(Mode::PcIndex, 0)),
                (None, _) => Err(format!("{UNPARSEABLE} `{disp}` in `{text}`")),
            },
            [base, index] if (is_address(base) || *base == "%pc") && is_index(index) => {
                Ok(Operand::Mode(Mode::IndexB, 0))
            }
            _ => Err(unsupported()),
        };
    }
    if text.strip_suffix(":l").is_some_and(|address| address.starts_with("0x")) {
        return Ok(Operand::Mode(Mode::AbsL, 0));
    }
    if text.strip_suffix(":w").is_some_and(|address| {
        address.starts_with("0x") || address.trim_start_matches('-').chars().all(|c| c.is_ascii_digit())
    }) {
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

/// What a byte of code is worth at -O1, -O2 and -O3, in thousandths of a
/// clock: a tie-break. The levels are judged by clocks (user ruling
/// 2026-11-14), so no clock is given up for bytes; -Os orders bytes first.
const TIE_MILLICLOCKS_PER_BYTE: i64 = 1;

pub fn byte_milliclocks(_cpu: SelectCpu) -> i64 {
    TIE_MILLICLOCKS_PER_BYTE
}

/// Whether `extra_bytes` of code are worth `saved_clocks` on `cpu`: the trade
/// passes only at N extra bytes per clock saved <= the row's allowance.
pub fn trade_passes(
    cpu: SelectCpu,
    extra_bytes: i64,
    saved_clocks: i64,
) -> bool {
    saved_clocks * 1000 >= extra_bytes * byte_milliclocks(cpu)
}

/// What a choice between two priced candidates minimizes: the level's price.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Price {
    /// Bytes, then clocks (`-Os`).
    Size,
    /// A candidate is taken only when no larger and no slower (`-O1`).
    Strict,
    /// Clocks plus bytes at the CPU's byte price (`-O2`, `-O3`).
    #[default]
    Rate,
}

impl Price {
    /// The price of a build: `size` wins, then `strict`.
    pub fn of(
        size: bool,
        strict: bool,
    ) -> Price {
        if size {
            Price::Size
        } else if strict && false {
            Price::Strict
        } else {
            Price::Rate
        }
    }

    /// Whether `candidate` beats `incumbent`, each `(bytes, clocks)`, on `cpu`.
    pub fn beats(
        self,
        cpu: SelectCpu,
        candidate: (usize, u32),
        incumbent: (usize, u32),
    ) -> bool {
        match self {
            Price::Size => candidate < incumbent,
            Price::Strict => candidate.0 <= incumbent.0 && candidate.1 <= incumbent.1 && candidate != incumbent,
            Price::Rate => {
                let key = |(bytes, clocks): (usize, u32)| order(cpu, false, clocks, bytes as u32);
                key(candidate) < key(incumbent)
            }
        }
    }
}

/// The order a priced choice minimizes on `cpu`: bytes then clocks for size,
/// else clocks plus bytes at the CPU's byte price, ties to fewer clocks.
pub fn order(
    cpu: SelectCpu,
    size: bool,
    clocks: u32,
    bytes: u32,
) -> (i64, i64) {
    let (clocks, bytes) = (i64::from(clocks), i64::from(bytes));
    if size { (bytes, clocks) } else { (clocks * 1000 + bytes * byte_milliclocks(cpu), clocks) }
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
            SelectCpu::M68010 => Cpu::M68010,
            SelectCpu::M68020 => Cpu::M68020,
            SelectCpu::M68030 => Cpu::M68030,
            SelectCpu::M68040 => Cpu::M68040,
            SelectCpu::M68060 => Cpu::M68060,
        },
        text,
        bytes,
        taken,
    )
}

pub use counts::DataRegisters;

/// [`cost`] for any CPU of the family.
pub fn cost_on(
    cpu: Cpu,
    text: &str,
    bytes: u32,
    taken: bool,
) -> Result<Cost, String> {
    cost_on_counted(cpu, text, bytes, taken, None)
}

/// [`cost_on`] with the places a shift by a register moves, when the trace
/// knows them ([`DataRegisters::places_of`]): the 68000 and 68010 add two
/// clocks a place and the cost is no longer a bound.
pub fn cost_on_counted(
    cpu: Cpu,
    text: &str,
    bytes: u32,
    taken: bool,
    count: Option<u32>,
) -> Result<Cost, String> {
    let text = text.trim();
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let (base, suffix) = match name.split_once('.') {
        Some((base, suffix)) => (base, Some(suffix)),
        None => (name, None),
    };
    let mut operands: Vec<Operand> = split_operands(rest).into_iter().map(operand).collect::<Result<_, _>>()?;
    if matches!(base, "lea" | "pea") {
        // the address of a symbol is absolute short in four bytes, long in six
        if let Some(one @ Operand::Target) = operands.first_mut() {
            *one = Operand::Mode(if bytes == 4 { Mode::AbsW } else { Mode::AbsL }, 0);
        }
    }
    if base == "movem" {
        // a list of one register reads as a register operand
        for one in &mut operands {
            if matches!(one, Operand::Mode(Mode::Dn | Mode::An, _)) {
                *one = Operand::List(1);
            }
        }
    }
    let clocks = match cpu {
        Cpu::M68000 => family::clocks(&family::M68000, base, suffix, &operands, bytes, taken),
        Cpu::M68010 => family::clocks(&family::M68010, base, suffix, &operands, bytes, taken),
        Cpu::M68020 => m68020::clocks(base, suffix, &operands, rest, bytes, taken),
        Cpu::M68030 => m68030::clocks(base, suffix, &operands, bytes, taken),
        Cpu::M68040 => m68040::clocks(base, suffix, &operands, bytes, taken),
        Cpu::M68060 => m68060::clocks(base, suffix, &operands, rest, taken),
    }
    .ok_or_else(|| format!("`{text}` is not in the {cpu:?} tables"))?;
    let by_register = is_shift(base)
        && matches!(cpu, Cpu::M68000 | Cpu::M68010)
        && places(&operands).is_none()
        && operands.len() == 2;
    let counted = by_register && count.is_some();
    let clocks = clocks + 2 * count.filter(|_| by_register).unwrap_or(0);
    let partial = if cpu == Cpu::M68060 {
        m68060::partial(base, suffix)
    } else if matches!(cpu, Cpu::M68020 | Cpu::M68030 | Cpu::M68040) {
        // bit fields and `cas` are a bound on the 68020 and 68030 (the "< 5
        // bytes" and successful-compare rows) and omit the long-word-boundary
        // term on the 68040
        matches!(base, "mulu" | "muls" | "divu" | "divs") || m68020::partial(base)
    } else {
        matches!(base, "mulu" | "muls" | "divu" | "divs")
            || (by_register && !counted)
            || (set_on_condition(base) && cpu == Cpu::M68000)
    };
    Ok(Cost { clocks, partial })
}

const BASES: [&str; 63] = [
    "movea", "move", "moveq", "movem", "adda", "addi", "addq", "addx", "add", "suba", "subi", "subq", "subx", "sub",
    "andi", "and", "ori", "or", "eori", "eor", "cmpa", "cmpi", "cmpm", "cmp", "clr", "negx", "neg", "not", "tst",
    "ext", "extb", "swap", "lsl", "lsr", "asl", "asr", "rol", "ror", "roxl", "roxr", "link", "unlk", "lea", "pea",
    "jsr", "jmp", "rts", "bra", "bsr", "mulu", "muls", "divu", "divs", "exg", "nop", "dbf", "dbra", "dbt", "dbne",
    "dbeq", "scc", "seq", "sne",
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
            // objdump shows a PC-relative target as its absolute address; the
            // clocks depend on the mode, taken here as a 16-bit displacement
            let displacement =
                if base == "%pc" && displacement.len() == 8 && displacement.chars().all(|c| c.is_ascii_hexdigit()) {
                    "0".to_owned()
                } else {
                    displacement.to_owned()
                };
            match parts.next() {
                None => format!("({displacement},{base})"),
                Some(index) => {
                    // binutils prints an index displacement as bare hex, sign-extended to 64 bits
                    let decimal = u64::from_str_radix(&displacement, 16).map_or(displacement, |bits| (bits as i64).to_string());
                    format!("({decimal},{base},{})", index.split(':').next().unwrap_or(index))
                }
            }
        }
    }
}

/// The pin's syntax for one line of binutils' `objdump -d` (GNU syntax:
/// `movel %a0@(12),%d2`), which disassembles `addq`, `tst` and the rest of
/// what the pin's own `llvm-objdump` shows as `<unknown>`.
pub fn from_gnu(text: &str) -> Result<String, String> {
    // a branch target reads `80000074 <k+0x8>`
    // an operand may be followed by its symbol mid-line: `%pc@(80000062)
    // <a>,%a0`
    let mut plain = String::new();
    let mut rest = text;
    while let Some((before, after)) = rest.split_once(" <") {
        plain.push_str(before);
        rest = after.split_once('>').map_or("", |(_, tail)| tail);
    }
    plain.push_str(rest);
    let text = plain.trim();
    let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    // `divull %d5,%d2,%d3` is the 32 / 32 `divu.l %d5, %d3` with the remainder
    // register named; the clocks are the long divide's either way
    let long_divide = ["divul", "divull", "divsl", "divsll"].contains(&name);
    let name = if long_divide { format!("div{}.l", &name[3..4]) } else { gnu_mnemonic(name) };
    // objdump prints an absolute address bare, as it does a branch target: only
    // a branch or `dbcc` takes it as a target
    let target = name.starts_with('b') || name.starts_with("db");
    let operands: Vec<String> = split_operands(rest.trim())
        .into_iter()
        .map(gnu_operand)
        .map(|one| {
            if !target && !one.is_empty() && one.chars().all(|c| c.is_ascii_hexdigit()) {
                // a short address is the 16-bit absolute form (`pea 0:w`)
                format!("0x{one}:{}", if one.len() >= 4 { 'l' } else { 'w' })
            } else {
                one
            }
        })
        .collect();
    let operands = match operands.as_slice() {
        [source, _, quotient] if long_divide => vec![source.clone(), quotient.clone()],
        // `mulu.l %d1,%d4,%d0` is the 32 x 32 -> 64 product, `%d4` its high
        // half: the manual's table has one `MUL.L` row for both forms
        [source, _, low] if matches!(name.as_str(), "mulu.l" | "muls.l") => vec![source.clone(), low.clone()],
        _ => operands,
    };
    Ok(if operands.is_empty() { name } else { format!("{name} {}", operands.join(", ")) })
}
