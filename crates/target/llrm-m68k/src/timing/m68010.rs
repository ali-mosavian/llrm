//! The MC68010's clocks: MC68000UM Section 9, Tables 9-1 to 9-19 (printed
//! pages 9-2 to 9-13), read from their transcription in `m68010/`.
//!
//! # How it works
//! Which table, row and column an instruction is in follows the manual's
//! layout; the numbers come from the CSVs, never from this file. A cell marked
//! `+` gets the effective address time of Table 9-1 added.
//!
//! # Limits
//! 1. Loop mode (Tables 9-3, 9-5, 9-7, 9-11, 9-13, 9-17's right half) is for a
//!    `DBcc` closing a one-instruction loop; an instruction is priced alone, so
//!    it is not used.
//! 2. MULS/MULU print two unlabeled rows (42 with `+`, 40 without). MULS takes
//!    the first, MULU the second; both are maxima.
//! 3. Table 9-16 prints `MOVEM.L (An),<list>` as 24+8n; its neighbour `(An)+`
//!    and the 68000's table say 12+8n. DEVIATE: 12+8n.
//! 4. Exception processing (Table 9-19) is not priced.

use std::sync::LazyLock;

use super::table::{Cell, Table};
use super::{Mode, Operand, Size, mode, places, size_of};

static TABLES: LazyLock<Vec<(u8, Table)>> = LazyLock::new(|| {
    let load = |number: u8, text: &str, two: bool| (number, Table::parse(text, two));
    vec![
        load(1, include_str!("m68010/t9-1.csv"), true),
        load(2, include_str!("m68010/t9-2.csv"), false),
        load(4, include_str!("m68010/t9-4.csv"), false),
        load(6, include_str!("m68010/t9-6.csv"), true),
        load(8, include_str!("m68010/t9-8.csv"), true),
        load(9, include_str!("m68010/t9-9.csv"), true),
        load(10, include_str!("m68010/t9-10.csv"), true),
        load(12, include_str!("m68010/t9-12.csv"), true),
        load(14, include_str!("m68010/t9-14.csv"), true),
        load(15, include_str!("m68010/t9-15.csv"), true),
        load(16, include_str!("m68010/t9-16.csv"), true),
        load(17, include_str!("m68010/t9-17.csv"), true),
        load(18, include_str!("m68010/t9-18.csv"), true),
    ]
});

fn table(number: u8) -> &'static Table {
    &TABLES.iter().find(|(one, _)| *one == number).expect("a loaded table").1
}

fn label(mode: Mode) -> &'static str {
    match mode {
        Mode::Dn => "Dn",
        Mode::An => "An",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp => "(d16,An)",
        Mode::Index => "(d8,An,Xn)",
        Mode::AbsW => "(xxx).W",
        Mode::AbsL => "(xxx).L",
        Mode::PcDisp => "(d16,PC)",
        Mode::PcIndex => "(d8,PC,Xn)",
        Mode::Imm => "#<data>",
    }
}

/// A size label of a row that holds for `size` (`Byte, Word`, `Long`, or none).
fn sized(size: Size) -> impl Fn(&str) -> bool {
    move |text: &str| match size {
        Size::L => text.is_empty() || text == "-" || text.contains("long"),
        _ => text.is_empty() || text == "-" || text.contains("byte") || text.contains("word"),
    }
}

fn any(_: &str) -> bool {
    true
}

fn cell(
    number: u8,
    row: &str,
    size: Size,
    column: &str,
) -> Option<Cell> {
    table(number).get(row, sized(size), column)
}

/// Table 9-1: the effective address time, with the operand fetched or not.
fn ea(
    mode: Mode,
    size: Size,
    fetch: bool,
) -> Option<u32> {
    let width = if size == Size::L { "Long" } else { "Byte, Word" };
    let how = if fetch { "Fetch" } else { "No Fetch" };
    let row = if mode == Mode::Imm { "#<data>" } else { label(mode) };
    Some(table(1).get(row, any, &format!("{width} / {how}"))?.base)
}

/// A cell with its effective address time added where the table says so.
fn with_ea(
    cell: Cell,
    operand: Option<Mode>,
    size: Size,
    fetch: bool,
) -> Option<u32> {
    match (cell.plus_ea, operand) {
        (true, Some(mode)) => Some(cell.base + ea(mode, size, fetch)?),
        (true, None) => None,
        (false, _) => Some(cell.base),
    }
}

/// The row of the immediate table an arithmetic instruction is in.
fn immediate_name(base: &str) -> Option<&'static str> {
    Some(match base {
        "add" | "addi" => "ADDI",
        "sub" | "subi" => "SUBI",
        "and" | "andi" => "ANDI",
        "or" | "ori" => "ORI",
        "eor" | "eori" => "EORI",
        "cmp" | "cmpi" => "CMPI",
        _ => return None,
    })
}

fn bus_label(mode: Mode) -> &'static str {
    match mode {
        Mode::AbsW => "(xxx) W",
        Mode::PcDisp => "(d8 PC)",
        Mode::PcIndex => "(d16,PC,Xn)",
        other => label(other),
    }
}

pub(super) fn clocks(
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    bytes: u32,
    taken: bool,
) -> Option<u32> {
    let size = size_of(suffix);
    let long = size == Size::L;
    let misc = |row: &str, column: &str| table(18).get(row, any, column);
    let register = |row: &str| misc(row, "Register").map(|cell| cell.base);
    match (base, operands) {
        ("move", [source, Operand::Status]) => {
            let cell = misc("MOVE to SR", if mode(source)? == Mode::Dn { "Register" } else { "Memory" })?;
            with_ea(cell, mode(source), Size::W, true)
        }
        ("move", [Operand::Status, destination]) => match mode(destination)? {
            Mode::Dn => register("MOVE from SR"),
            memory => with_ea(misc("MOVE from SR", "Memory")?, Some(memory), Size::W, false),
        },
        ("move", [_, Operand::Usp]) | ("move", [Operand::Usp, _]) => register("MOVE to USP"),
        ("movec", [Operand::Control, _]) => misc("MOVEC", "Source→Register").map(|cell| cell.base),
        ("movec", [_, Operand::Control]) => misc("MOVEC", "Register→Destination").map(|cell| cell.base),
        ("move" | "movea", [source, destination]) => {
            let number = if long { 4 } else { 2 };
            Some(table(number).get(label(mode(source)?), any, &format!("Destination / {}", label(mode(destination)?)))?.base)
        }
        ("moveq", [_, _]) => cell(8, "MOVEQ", Size::L, "op #, Dn").map(|cell| cell.base),
        ("add" | "sub" | "and" | "or" | "eor" | "cmp" | "adda" | "suba" | "cmpa", [source, destination]) => {
            let (source, destination) = (mode(source)?, mode(destination)?);
            let name = base.to_ascii_uppercase();
            match (base, destination) {
                (_, Mode::An) => with_ea(cell(6, &name, size, "op<ea>, An")?, Some(source), size, true),
                ("eor", Mode::Dn) => cell(6, &name, size, "op<ea>, Dn").map(|cell| cell.base),
                ("eor", memory) => with_ea(cell(6, &name, size, "op Dn, <M>")?, Some(memory), size, true),
                (_, Mode::Dn) if source == Mode::Imm && base != "cmp" => {
                    cell(8, immediate_name(base)?, size, "op #, Dn").map(|cell| cell.base)
                }
                (_, Mode::Dn) => with_ea(cell(6, &name, size, "op<ea>, Dn")?, Some(source), size, true),
                (_, memory) if source == Mode::Dn => with_ea(cell(6, &name, size, "op Dn, <M>")?, Some(memory), size, true),
                (_, memory) if source == Mode::Imm => {
                    with_ea(cell(8, immediate_name(base)?, size, "op #, M")?, Some(memory), size, true)
                }
                _ => None,
            }
        }
        ("andi" | "ori" | "eori", [_, Operand::Status]) => register("ANDI to SR"),
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => match mode(destination)? {
            Mode::Dn => cell(8, immediate_name(base)?, size, "op #, Dn").map(|cell| cell.base),
            memory => with_ea(cell(8, immediate_name(base)?, size, "op #, M")?, Some(memory), size, true),
        },
        ("addq" | "subq", [_, destination]) => {
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn => cell(8, &name, size, "op #, Dn").map(|cell| cell.base),
                Mode::An => cell(8, &name, size, "op #, An").map(|cell| cell.base),
                memory => with_ea(cell(8, &name, size, "op #, M")?, Some(memory), size, true),
            }
        }
        ("clr", [destination]) => {
            Some(table(10).get("CLR", sized(size), bus_label(mode(destination)?))?.base)
        }
        ("neg" | "negx" | "not" | "nbcd", [destination]) => {
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn => cell(9, &name, size, "Register").map(|cell| cell.base),
                memory => with_ea(cell(9, &name, size, "Memory")?, Some(memory), size, true),
            }
        }
        ("tst", [destination]) => match mode(destination)? {
            Mode::Dn | Mode::An => cell(9, "TST", size, "Register").map(|cell| cell.base),
            memory => with_ea(cell(9, "TST", size, "Memory")?, Some(memory), size, true),
        },
        ("tas", [destination]) => match mode(destination)? {
            Mode::Dn => cell(9, "TAS", Size::B, "Register").map(|cell| cell.base),
            memory => with_ea(cell(9, "TAS", Size::B, "Memory")?, Some(memory), Size::B, false),
        },
        (set, [destination]) if super::set_on_condition(set) => match mode(destination)? {
            Mode::Dn => cell(9, "Scc", Size::B, "Register").map(|cell| cell.base),
            memory => with_ea(cell(9, "Scc", Size::B, "Memory")?, Some(memory), Size::B, false),
        },
        ("ext", _) => misc("EXT", "Register").map(|cell| cell.base),
        ("swap", _) => register("SWAP"),
        ("nop", _) => register("NOP"),
        ("exg", _) => register("EXG"),
        (shift, [_, _]) if super::is_shift(shift) => {
            let cell = table(12).get(&shift.to_ascii_uppercase(), sized(size), "Register")?;
            Some(cell.base + cell.per * places(operands).unwrap_or(0))
        }
        (shift, [memory]) if super::is_shift(shift) => {
            with_ea(table(12).get(&shift.to_ascii_uppercase(), sized(Size::W), "Memory")?, mode(memory), Size::W, true)
        }
        ("muls" | "mulu", [source, _]) => {
            let nth = usize::from(base == "mulu");
            let text = table(6).text_nth("MULS", nth, any, "op<ea>, Dn")?;
            with_ea(super::table::parse_cell(text)?, mode(source), Size::W, true).map(|clocks| {
                // the second row prints no `+`: MULU takes the same effective address time
                if nth == 1 { clocks + ea(mode(source).unwrap_or(Mode::Dn), Size::W, true).unwrap_or(0) } else { clocks }
            })
        }
        ("divs" | "divu", [source, _]) => {
            with_ea(cell(6, &base.to_ascii_uppercase(), Size::W, "op<ea>, Dn")?, mode(source), Size::W, true)
        }
        ("chk", [source, _]) => with_ea(misc("CHK", "Register")?, mode(source), Size::W, true),
        ("lea" | "pea", [source]) | ("lea", [source, _]) | ("pea", [source, ..]) => {
            let name = base.to_ascii_uppercase();
            Some(table(16).get(&name, any, bus_label(mode(source)?))?.base)
        }
        ("jmp" | "jsr", [target]) => {
            let name = base.to_ascii_uppercase();
            let column = match target {
                // a call to an address is absolute short in four bytes, long in six
                Operand::Target => if bytes == 4 { "(xxx) W" } else { "(xxx).L" },
                other => bus_label(mode(other)?),
            };
            Some(table(16).get(&name, any, column)?.base)
        }
        ("bra", [_]) => cell(15, "BRA", Size::B, "Branch Taken").map(|cell| cell.base),
        ("bsr", [_]) => cell(15, "BSR", Size::B, "Branch Taken").map(|cell| cell.base),
        (condition, [_]) if super::condition_branch(condition) => {
            let word = |text: &str| text.contains(if bytes <= 2 { "byte" } else { "word" });
            let column = if taken { "Branch Taken" } else { "Branch Not Taken" };
            table(15).get("Bcc", word, column).map(|cell| cell.base)
        }
        (dbcc, [_, _]) if dbcc.starts_with("db") => {
            let (row, column) = match (dbcc == "dbt", taken) {
                (true, _) => ("cc true", "Branch Not Taken"),
                (false, true) => ("cc false", "Branch Taken"),
                (false, false) => ("cc false", "Branch Not Taken"),
            };
            table(15).get("DBcc", |text| text.contains(row.replace(' ', "").as_str()), column).map(|cell| cell.base)
        }
        ("rts", []) => register("RTS"),
        ("rtr", []) => register("RTR"),
        ("rtd", [_]) => register("RTD"),
        ("rte", []) => table(18).get("RTE", |text| text.contains("short"), "Register").map(|cell| cell.base),
        ("link", [_, _]) => register("LINK"),
        ("unlk", [_]) => register("UNLK"),
        ("trapv", []) => register("TRAPV"),
        ("stop", [_]) => register("STOP"),
        ("reset", []) => register("RESET"),
        ("movem", [Operand::List(registers), memory]) => movem("MOVEM R→M", memory, long, *registers),
        // DEVIATE (limit 3): the page prints 24+8n for `(An)` long; its neighbour `(An)+` is 12+8n
        ("movem", [memory, Operand::List(registers)]) if long && mode(memory)? == Mode::Ind => {
            movem("MOVEM M→R", &Operand::Mode(Mode::Post, 0), long, *registers)
        }
        ("movem", [memory, Operand::List(registers)]) => movem("MOVEM M→R", memory, long, *registers),
        ("btst" | "bchg" | "bclr" | "bset", [source, destination]) => {
            let kind = if mode(source)? == Mode::Dn { "Dynamic" } else { "Static" };
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn => cell(14, &name, Size::L, &format!("{kind} / Register")).map(|cell| cell.base),
                memory => with_ea(cell(14, &name, Size::B, &format!("{kind} / Memory"))?, Some(memory), Size::B, true),
            }
        }
        _ => None,
    }
}

/// Table 9-16's MOVEM rows: `base + per * n`.
fn movem(
    row: &str,
    memory: &Operand,
    long: bool,
    registers: u32,
) -> Option<u32> {
    let cell = table(16).get(row, sized(if long { Size::L } else { Size::W }), bus_label(mode(memory)?))?;
    Some(cell.base + cell.per * registers)
}
