//! The MC68020's clocks: M68020UM Section 8, Tables 8.2.1 to 8.2.16 (printed
//! pages 8-13 to 8-40), the cache case column (instruction cache hit, no wait
//! states), read from their transcription in `m68020/`.
//!
//! # How it works
//! An instruction's row gives its clocks; the row's mark says which
//! effective address table adds its time: `*` fetch (8.2.1), `**` fetch
//! immediate (8.2.2), `†` calculate (8.2.3), `‡` calculate immediate (8.2.4),
//! `%` jump (8.2.5). A move is its own matrix (8.2.6).
//!
//! # Limits
//! 1. The 68020 overlaps instructions: the cache case is a bound, not a
//!    prediction. Bus waits, alignment and instruction cache misses are not in
//!    it.
//! 2. Bit fields are the "< 5 bytes" rows; `cas` and `cas2` the successful
//!    compare. Both are `partial`.
//! 3. Memory-indirect and full-format modes are not priced.
//! 4. Table 8.2.4 prints no `-(An)` row, so `movem` to `-(An)` takes the
//!    `(An)+` row.
//! 5. Exception processing, `trapcc`, `stop`, `reset`, `rte`, `callm` and `rtm`
//!    are not priced.

use std::sync::LazyLock;

use super::table::{Cell, key, parse_cell, split_csv};
use super::{Mode, Operand, Size, mode, places, size_of};

struct Row {
    instruction: String,
    operands: String,
    mark: String,
    cache: String,
}

/// The rows of the instruction tables 8.2.7 to 8.2.16.
static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    let files = [
        include_str!("m68020/t8.2.7.csv"),
        include_str!("m68020/t8.2.8.csv"),
        include_str!("m68020/t8.2.9.csv"),
        include_str!("m68020/t8.2.10.csv"),
        include_str!("m68020/t8.2.11.csv"),
        include_str!("m68020/t8.2.12.csv"),
        include_str!("m68020/t8.2.13.csv"),
        include_str!("m68020/t8.2.14.csv"),
        include_str!("m68020/t8.2.15.csv"),
        include_str!("m68020/t8.2.16.csv"),
    ];
    files
        .iter()
        .flat_map(|text| text.lines().skip(1))
        .map(split_csv)
        .map(|fields| Row {
            instruction: fields[0].clone(),
            operands: fields[1].clone(),
            mark: fields[2].clone(),
            cache: fields[4].clone(),
        })
        .collect()
});

/// An effective address table: `(mode label, cache cell)`.
fn grid(text: &str) -> Vec<(String, String)> {
    text.lines().skip(1).map(split_csv).map(|fields| (key(&fields[0]), fields[2].clone())).collect()
}

struct Grids {
    fetch: Vec<(String, String)>,
    fetch_immediate: Vec<(String, String)>,
    calculate: Vec<(String, String)>,
    calculate_immediate: Vec<(String, String)>,
    jump: Vec<(String, String)>,
    moves: Vec<Vec<String>>,
}

static GRIDS: LazyLock<Grids> = LazyLock::new(|| Grids {
    fetch: grid(include_str!("m68020/t8.2.1.csv")),
    fetch_immediate: grid(include_str!("m68020/t8.2.2.csv")),
    calculate: grid(include_str!("m68020/t8.2.3.csv")),
    calculate_immediate: grid(include_str!("m68020/t8.2.4.csv")),
    jump: grid(include_str!("m68020/t8.2.5.csv")),
    moves: include_str!("m68020/t8.2.6-cache.csv").lines().map(split_csv).collect(),
});

fn lookup(
    grid: &[(String, String)],
    label: &str,
) -> Option<u32> {
    let label = key(label);
    grid.iter().find(|(name, _)| *name == label).and_then(|(_, cell)| parse_cell(cell)).map(|cell| cell.base)
}

/// The label of `mode` in the effective address tables.
fn label(
    mode: Mode,
    size: Size,
) -> &'static str {
    match mode {
        Mode::Dn => "Dn",
        Mode::An => "An",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp | Mode::PcDisp => "(d16,An) or (d16,PC)",
        Mode::AbsW => "(xxx).W",
        Mode::AbsL => "(xxx).L",
        Mode::Index | Mode::PcIndex => "(d8,An,Xn) or (d8,PC,Xn)",
        Mode::Imm => match size {
            Size::B => "#<data>.B",
            Size::W => "#<data>.W",
            Size::L => "#<data>.L",
        },
    }
}

fn fetch(
    mode: Mode,
    size: Size,
) -> Option<u32> {
    lookup(&GRIDS.fetch, label(mode, size))
}

/// 8.2.2 and 8.2.4: `#<data>.W|L,<destination>`; a byte immediate is the word
/// row.
fn immediate_label(
    mode: Mode,
    size: Size,
) -> Option<String> {
    let destination = match mode {
        Mode::Dn => "Dn",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp => "(bd,An)",
        Mode::AbsW => "xxx.W",
        Mode::AbsL => "xxx.L",
        Mode::Index => "(d8,An,Xn) or (d8,PC,Xn)",
        _ => return None,
    };
    Some(format!("#<data>.{},{destination}", if size == Size::L { 'L' } else { 'W' }))
}

fn fetch_immediate(
    mode: Mode,
    size: Size,
) -> Option<u32> {
    lookup(&GRIDS.fetch_immediate, &immediate_label(mode, size)?)
}

fn calculate(mode: Mode) -> Option<u32> {
    let label = match mode {
        Mode::AbsW => "<data>.W",
        Mode::AbsL => "<data>.L",
        other => label(other, Size::W),
    };
    lookup(&GRIDS.calculate, label)
}

fn calculate_immediate(mode: Mode) -> Option<u32> {
    // the table prints no `-(An)` row; it is `(An)+`'s (limit 5)
    let mode = if mode == Mode::Pre { Mode::Post } else { mode };
    lookup(&GRIDS.calculate_immediate, &immediate_label(mode, Size::W)?)
}

fn jump(mode: Mode) -> Option<u32> {
    let label = match mode {
        Mode::Disp | Mode::PcDisp => "(d16,An)",
        other => label(other, Size::W),
    };
    lookup(&GRIDS.jump, label)
}

/// The effective address time a row's mark asks for.
fn marked(
    mark: &str,
    operand: Option<Mode>,
    size: Size,
) -> Option<u32> {
    match (mark, operand) {
        ("", _) => Some(0),
        (_, None) => None,
        ("*", Some(mode)) => fetch(mode, size),
        ("**", Some(mode)) => fetch_immediate(mode, size),
        ("\u{2020}", Some(mode)) => calculate(mode),
        ("\u{2021}", Some(mode)) => calculate_immediate(mode),
        ("%", Some(mode)) => jump(mode),
        _ => None,
    }
}

fn row(
    instruction: &str,
    operands: &str,
) -> Option<&'static Row> {
    let (instruction, operands) = (key(instruction), key(operands));
    ROWS.iter().find(|row| key(&row.instruction) == instruction && key(&row.operands) == operands)
}

/// A row's cache clocks, `+ n * registers`, with the effective address time of
/// its mark added.
fn price(
    instruction: &str,
    operands: &str,
    ea: Option<Mode>,
    size: Size,
    registers: u32,
) -> Option<u32> {
    let row = row(instruction, operands)?;
    let text: String = row.cache.chars().filter(|c| !c.is_whitespace()).collect();
    let Cell { base, per, .. } = parse_cell(&text)?;
    Some(base + per * registers + marked(&row.mark, ea, size)?)
}

/// A row named whole (`DBcc (cc = True)`), no operands column.
fn named(instruction: &str) -> Option<u32> {
    price(instruction, "", None, Size::W, 0)
}

/// 8.2.6: a move, by source row and destination column.
fn move_clocks(
    source: Mode,
    size: Size,
    destination: Mode,
) -> Option<u32> {
    let source_label = match source {
        Mode::Dn | Mode::An => "Rn",
        Mode::Imm if size == Size::L => "#<data>.L",
        Mode::Imm => "#<data>.B,W",
        other => label(other, size),
    };
    let destination_label = match destination {
        Mode::Imm => return None,
        other => label(other, size),
    };
    let moves = &GRIDS.moves;
    // the matrix labels PC displacement and index with the address register
    // forms
    let strip = |text: &str| key(text.split(" or ").next().unwrap_or(text));
    let column = moves[0].iter().position(|name| strip(name) == strip(destination_label))?;
    let found = moves[1..].iter().find(|fields| strip(&fields[0]) == strip(source_label))?;
    parse_cell(&found[column]).map(|cell| cell.base)
}

pub(super) fn clocks(
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    rest: &str,
    bytes: u32,
    taken: bool,
) -> Option<u32> {
    let size = size_of(suffix);
    let long = size == Size::L;
    let name = base.to_ascii_uppercase();
    let ccr = rest.contains("%ccr");
    match (base, operands) {
        ("move", [source, Operand::Status]) => {
            price("MOVE", if ccr { "EA,CCR" } else { "EA,SR" }, mode(source), Size::W, 0)
        }
        ("move", [Operand::Status, destination]) => match mode(destination)? {
            Mode::Dn => price("MOVE", "PSW,Rn", None, size, 0),
            memory => price("MOVE", "PSW,Mem", Some(memory), size, 0),
        },
        ("move", [_, Operand::Usp]) | ("move", [Operand::Usp, _]) => price("MOVE", "USP", None, size, 0),
        ("movec", [Operand::Control, _]) => price("MOVEC", "Cr,Rn", None, size, 0),
        ("movec", [_, Operand::Control]) => price("MOVEC", "Rn,Cr", None, size, 0),
        ("move" | "movea", [source, destination]) => move_clocks(mode(source)?, size, mode(destination)?),
        ("moveq", [_, _]) => price("MOVEQ", "#<data>,Dn", None, size, 0),
        ("add" | "sub" | "and" | "or" | "eor" | "cmp" | "adda" | "suba" | "cmpa", [source, destination]) => {
            let (source, destination) = (mode(source)?, mode(destination)?);
            let (instruction, immediate) = match base {
                "adda" => ("ADDA".to_owned(), false),
                "suba" => ("SUBA".to_owned(), false),
                "cmpa" => ("CMPA".to_owned(), false),
                _ => (name.clone(), source == Mode::Imm),
            };
            if immediate {
                let target = if destination == Mode::Dn { "#<data>,Dn" } else { "#<data>,Mem" };
                let target = if base == "cmp" { "#<data>,EA" } else { target };
                return price(&format!("{name}I"), target, Some(destination), size, 0);
            }
            match (base, destination) {
                ("eor", Mode::Dn) => price("EOR", "Dn,Dn", Some(Mode::Dn), size, 0),
                ("eor", memory) => price("EOR", "Dn,Mem", Some(memory), size, 0),
                (_, Mode::An) => match base {
                    "add" | "adda" => price("ADDA", "EA,An", Some(source), size, 0),
                    "sub" | "suba" => price("SUBA", "EA,An", Some(source), size, 0),
                    "cmp" | "cmpa" => price("CMPA", "EA,An", Some(source), size, 0),
                    _ => None,
                },
                (_, Mode::Dn) => price(&instruction, "EA,Dn", Some(source), size, 0),
                (_, memory) => price(&instruction, "Dn,EA", Some(memory), size, 0),
            }
        }
        ("andi" | "ori" | "eori", [_, Operand::Status]) => {
            named(&format!("{name} to {}", if ccr { "CCR" } else { "SR" }))
        }
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => {
            let destination = mode(destination)?;
            let target = match (base, destination) {
                ("cmpi", _) => "#<data>,EA",
                (_, Mode::Dn) => "#<data>,Dn",
                _ => "#<data>,Mem",
            };
            price(&name, target, Some(destination), size, 0)
        }
        ("addq" | "subq", [_, destination]) => match mode(destination)? {
            Mode::Dn | Mode::An => price(&name, "#<data>,Rn", None, size, 0),
            memory => price(&name, "#<data>,Mem", Some(memory), size, 0),
        },
        ("clr" | "neg" | "negx" | "not", [destination]) => match mode(destination)? {
            Mode::Dn => price(&name, "Dn", None, size, 0),
            memory => price(&name, "Mem", Some(memory), size, 0),
        },
        ("ext" | "nbcd", _) => price(&name, "Dn", None, size, 0),
        ("tas", [destination]) => match mode(destination)? {
            Mode::Dn => price("TAS", "Dn", None, size, 0),
            memory => price("TAS", "Mem", Some(memory), size, 0),
        },
        (set, [destination]) if super::set_on_condition(set) => match mode(destination)? {
            Mode::Dn => price("Scc", "Dn", None, size, 0),
            memory => price("Scc", "Mem", Some(memory), size, 0),
        },
        ("tst", [destination]) => price("TST", "EA", mode(destination), size, 0),
        ("swap", _) => price("SWAP", "Rx,Ry", None, size, 0),
        ("exg", _) => price("EXG", "Ry,Rx", None, size, 0),
        ("nop", _) => named("NOP"),
        (shift, [_, _]) if super::is_shift(shift) => {
            let operand = match shift {
                "lsl" | "lsr" if places(operands).is_some() => "Dn (Static)",
                "lsl" | "lsr" => "Dn (Dynamic)",
                _ => "Dn",
            };
            price(&name, operand, None, size, 0)
        }
        (shift, [memory]) if super::is_shift(shift) => {
            let row = if shift.starts_with("rox") { "ROXd".to_owned() } else { name.clone() };
            price(&row, "Mem by 1", mode(memory), Size::W, 0)
        }
        ("mulu" | "muls", [source, _]) => {
            price(if long { "MUL.L" } else { "MUL.W" }, "EA,Dn", mode(source), Size::W, 0)
        }
        ("divu" | "divs", [source, _]) => {
            price(&format!("{name}.{}", if long { 'L' } else { 'W' }), "EA,Dn", mode(source), Size::W, 0)
        }
        ("btst" | "bchg" | "bclr" | "bset", [source, destination]) => {
            let first = if mode(source)? == Mode::Imm { "#<data>" } else { "Dn" };
            match mode(destination)? {
                Mode::Dn => price(&name, &format!("{first},Dn"), None, size, 0),
                memory => price(&name, &format!("{first},Mem"), Some(memory), Size::B, 0),
            }
        }
        (field, _) if field.starts_with("bf") => {
            let memory = operands.iter().filter_map(mode).find(|mode| !matches!(mode, Mode::Dn | Mode::Imm));
            match memory {
                None => price(&name, "Dn", None, size, 0),
                Some(memory) => price(&name, "Mem (< 5 Bytes)", Some(memory), Size::W, 0),
            }
        }
        ("abcd" | "sbcd" | "addx" | "subx" | "cmpm" | "pack" | "unpk", [first, ..]) => {
            let operands = match (base, mode(first)? == Mode::Dn) {
                ("cmpm", _) => "(An)+,(An)+",
                ("pack" | "unpk", true) => "Dn,Dn,#<data>",
                ("pack" | "unpk", false) => "-(An),-(An),#<data>",
                (_, true) => "Dn,Dn",
                (_, false) => "-(An),-(An)",
            };
            price(&name, operands, None, size, 0)
        }
        ("chk", [source, _]) => price("CHK", "", mode(source), Size::W, 0),
        ("chk2" | "cmp2", [source, ..]) => price(&name, "EA,Rn", mode(source), size, 0),
        ("lea", [source, _]) => price("LEA", "", mode(source), size, 0),
        ("pea", [source]) => price("PEA", "", mode(source), size, 0),
        ("jmp" | "jsr", [target]) => {
            let target = match target {
                Operand::Target => Mode::AbsL,
                other => mode(other)?,
            };
            price(&name, "", Some(target), size, 0)
        }
        ("bra", [_]) => named("Bcc (Taken)"),
        ("bsr", [_]) => named("BSR"),
        (condition, [_]) if super::condition_branch(condition) => {
            if taken {
                named("Bcc (Taken)")
            } else {
                named(match bytes {
                    0..=2 => "Bcc.B (Not Taken)",
                    3..=4 => "Bcc.W (Not Taken)",
                    _ => "Bcc.L (Not Taken)",
                })
            }
        }
        (dbcc, [_, _]) if dbcc.starts_with("db") => named(match (dbcc == "dbt", taken) {
            (true, _) => "DBcc (cc = True)",
            (false, true) => "DBcc (cc = False, Count Not Expired)",
            (false, false) => "DBcc (cc = False, Count Expired)",
        }),
        ("rts", []) => named("RTS"),
        ("rtr", []) => named("RTR"),
        ("rtd", [_]) => named("RTD"),
        ("link", [_, _]) => named(if long { "LINK.L" } else { "LINK.W" }),
        ("unlk", [_]) => named("UNLK"),
        ("cas", [.., memory]) => price("CAS (Successful Compare)", "", mode(memory), size, 0),
        ("cas2", _) => named("CAS2 (Successful Compare)"),
        ("movep", [first, _]) => {
            let width = if long { 'L' } else { 'W' };
            let operands = if mode(first)? == Mode::Dn { "Dn,(d16,An)" } else { "(d16,An),Dn" };
            price(&format!("MOVEP.{width}"), operands, None, size, 0)
        }
        ("moves", [first, second]) => {
            if matches!(mode(first)?, Mode::Dn | Mode::An) {
                price("MOVES", "Rn,EA", mode(second), size, 0)
            } else {
                price("MOVES", "EA,Rn", mode(first), size, 0)
            }
        }
        ("movem", [Operand::List(registers), memory]) => price("MOVEM", "RL,EA", mode(memory), Size::W, *registers),
        ("movem", [memory, Operand::List(registers)]) => price("MOVEM", "EA,RL", mode(memory), Size::W, *registers),
        _ => None,
    }
}

/// `true` for an instruction whose number is a bound, not exact (limit 2).
pub(super) fn partial(base: &str) -> bool {
    base.starts_with("bf") || matches!(base, "cas" | "cas2")
}
