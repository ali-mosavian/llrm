//! The MC68060's clocks: M68060UM Section 10 (printed pages 10-14 to 10-24),
//! read from their transcription in `m68060/`.
//!
//! # How it works
//! A cell is `clocks(reads/writes)`; the model's number is `clocks`, plus the
//! effective address time of Table 10-5 where the manual says to add it. One
//! instruction is priced as the operand execution pipe alone runs it.
//!
//! # Limits
//! 1. The 68060 is superscalar and pipelined: a number is the instruction's own
//!    time, not a stream's. Pairing in the second pipe, stalls, cache and ATC
//!    misses and the FPU are not in it.
//! 2. Branches, `jmp` and `jsr` are priced as predicted correctly (the branch
//!    cache hit); a misprediction costs more. These are `partial`.
//! 3. Bit field times are the "< 5 bytes" rows; the "= 5 bytes" rows are
//!    longer. `partial`.
//! 4. `move16` is priced as the manual does, for cache misses (11 on a read
//!    hit).
//! 5. Memory-indirect and full-format modes, the "+1 if `(Ay)+` or `-(Ay)` is
//!    the destination `An`" footnote of Table 10-9, floating point, the cache
//!    and MMU instructions and exception entry are not priced.

use std::sync::LazyLock;

use super::table::{key, split_csv};
use super::{Mode, Operand, Size, mode, size_of};

struct Grid {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// A label without typesetting: `key`, the footnote marks and the column group
/// prefixes.
fn norm(label: &str) -> String {
    let label = key(label);
    let label: String = label.chars().filter(|c| !matches!(c, '\u{b9}' | '\u{b2}' | '\u{b3}')).collect();
    label.strip_prefix("destination/").unwrap_or(&label).to_owned()
}

fn grid(csv: &str) -> Grid {
    let mut lines = csv.lines().skip(1);
    let header = split_csv(lines.next().expect("a header row"));
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let mut fields = split_csv(line);
        // a ditto mark repeats the row above
        for at in 0..2.min(fields.len()) {
            if fields[at] == "\"" {
                fields[at] = rows.last().map(|above| above[at].clone()).unwrap_or_default();
            }
        }
        rows.push(fields);
    }
    Grid { header, rows }
}

/// The clocks of a cell: `1+n(n/0)`, `<=22(1/0)`. A dash is no entry.
fn clocks_of(
    cell: &str,
    registers: u32,
) -> Option<u32> {
    let cell: String = cell.chars().filter(|c| !matches!(c, '\u{b9}' | '\u{b2}' | '\u{b3}' | '*' | ' ')).collect();
    let cell = cell.strip_prefix("<=").unwrap_or(&cell);
    cell.split('(').next()?.split('+').map(|term| if term == "n" { Some(registers) } else { term.parse().ok() }).sum()
}

impl Grid {
    fn column(
        &self,
        label: &str,
    ) -> Option<usize> {
        let label = norm(label);
        self.header.iter().position(|name| norm(name) == label)
    }

    /// The cell of the row `name` (one of its comma-separated instructions; a
    /// name with `(` is a prefix) at `column`, preferring the row whose size
    /// contains `size`.
    fn cell(
        &self,
        name: &str,
        size: &str,
        column: &str,
        registers: u32,
    ) -> Option<u32> {
        let (name, column) = (norm(name), self.column(column)?);
        let rows: Vec<&Vec<String>> = self
            .rows
            .iter()
            .filter(|row| {
                let first = &row[0];
                let names: Vec<&str> = if first.contains('(') { vec![first] } else { first.split(", ").collect() };
                names.iter().any(|one| norm(one) == name || (name.contains('(') && norm(one).starts_with(&name)))
            })
            .collect();
        let sized = rows.iter().find(|row| row.get(1).is_some_and(|one| norm(one).contains(size)));
        sized
            .and_then(|row| clocks_of(&row[column], registers))
            .or_else(|| rows.iter().find_map(|row| clocks_of(&row[column], registers)))
    }
}

struct Tables {
    ea: Grid,
    move_word: Grid,
    move_long: Grid,
    move16: Grid,
    standard: Grid,
    immediate: Grid,
    single: Grid,
    clear: Grid,
    shift: Grid,
    bit_dynamic: Grid,
    bit_static: Grid,
    bit_field: Grid,
    branch: Grid,
    jump: Grid,
    leave: Grid,
    lea: Grid,
    multiprecision: Grid,
    status: Grid,
    moves: Grid,
    misc: Grid,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables {
    ea: grid(include_str!("m68060/t10-5.csv")),
    move_word: grid(include_str!("m68060/t10-6.csv")),
    move_long: grid(include_str!("m68060/t10-7.csv")),
    move16: grid(include_str!("m68060/t10-8.csv")),
    standard: grid(include_str!("m68060/t10-9.csv")),
    immediate: grid(include_str!("m68060/t10-10.csv")),
    single: grid(include_str!("m68060/t10-11.csv")),
    clear: grid(include_str!("m68060/t10-12.csv")),
    shift: grid(include_str!("m68060/t10-13.csv")),
    bit_dynamic: grid(include_str!("m68060/t10-14.csv")),
    bit_static: grid(include_str!("m68060/t10-15.csv")),
    bit_field: grid(include_str!("m68060/t10-16.csv")),
    branch: grid(include_str!("m68060/t10-17.csv")),
    jump: grid(include_str!("m68060/t10-18.csv")),
    leave: grid(include_str!("m68060/t10-19.csv")),
    lea: grid(include_str!("m68060/t10-20.csv")),
    multiprecision: grid(include_str!("m68060/t10-21.csv")),
    status: grid(include_str!("m68060/t10-22.csv")),
    moves: grid(include_str!("m68060/t10-23.csv")),
    misc: grid(include_str!("m68060/t10-24.csv")),
});

/// The label the manual's tables print for `mode`; `grouped` is the `(xxx).WL`
/// of a column.
fn label(
    mode: Mode,
    grouped: bool,
) -> &'static str {
    match mode {
        Mode::Dn => "Dn",
        Mode::An => "An",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp => "(d16,An)",
        Mode::Index => "(d8,An,Xi*SF)",
        Mode::AbsW if grouped => "(xxx).WL",
        Mode::AbsL if grouped => "(xxx).WL",
        Mode::AbsW => "(xxx).W",
        Mode::AbsL => "(xxx).L",
        Mode::PcDisp => "(d16,PC)",
        Mode::PcIndex => "(d8,PC,Xi*SF)",
        Mode::Imm => "#<data>",
    }
}

/// Table 10-5: the effective address calculation of `mode`.
fn ea_time(mode: Mode) -> Option<u32> {
    let ea = &TABLES.ea;
    let wanted = norm(label(mode, false));
    ea.rows.iter().find(|row| norm(&row[0]) == wanted).and_then(|row| clocks_of(&row[2], 0))
}

/// The memory operand's mode, else the last register one.
fn effective(operands: &[Operand]) -> Option<Mode> {
    let modes: Vec<Mode> = operands.iter().filter_map(mode).collect();
    modes
        .iter()
        .copied()
        .find(|mode| !matches!(mode, Mode::Dn | Mode::An | Mode::Imm))
        .or_else(|| modes.last().copied())
}

fn size_word(size: Size) -> &'static str {
    match size {
        Size::B => "byte",
        Size::W => "word",
        Size::L => "long",
    }
}

/// A table with a `Register` and a `Memory` column: the memory one adds the
/// effective address time.
fn register_or_memory(
    grid: &Grid,
    name: &str,
    size: Size,
    mode: Mode,
) -> Option<u32> {
    if matches!(mode, Mode::Dn | Mode::An | Mode::Imm) {
        grid.cell(name, size_word(size), "Register", 0)
    } else {
        Some(grid.cell(name, size_word(size), "Memory", 0)? + ea_time(mode)?)
    }
}

/// A table with a column per destination mode; the effective address time is in
/// the cell.
fn by_destination(
    grid: &Grid,
    name: &str,
    size: Size,
    mode: Mode,
) -> Option<u32> {
    grid.cell(name, size_word(size), label(mode, true), 0)
}

/// Table 10-9: `op <ea>,An`, `op <ea>,Dn` or `op Dn,<M>`, plus the effective
/// address time of the memory operand.
fn standard(
    name: &str,
    size: Size,
    source: Mode,
    destination: Mode,
) -> Option<u32> {
    let (column, ea) = match destination {
        Mode::An => ("op <ea>,An", source),
        Mode::Dn => ("op <ea>,Dn", source),
        _ => ("op Dn,<M>", destination),
    };
    Some(TABLES.standard.cell(name, size_word(size), column, 0)? + ea_time(ea)?)
}

fn branch_column(taken: bool) -> &'static str {
    if taken { "Predicted Correctly as Taken" } else { "Predicted Correctly as Not Taken" }
}

/// `jmp` and `jsr` print their `(d16,PC)` and absolute forms apart from the
/// rest.
fn jump(
    name: &str,
    target: Mode,
) -> Option<u32> {
    let row = match target {
        Mode::PcDisp => format!("{name} (d16,PC)"),
        Mode::AbsW | Mode::AbsL => format!("{name} xxx.WL"),
        _ => format!("Remaining {name}"),
    };
    Some(TABLES.jump.cell(&row, "", branch_column(true), 0)? + ea_time(target)?)
}

/// The superscripted footnotes of Table 10-9 and 10-11 that name a column of
/// the clocks.
fn miscellaneous(
    name: &str,
    size: Size,
    mode: Option<Mode>,
) -> Option<u32> {
    let misc = &TABLES.misc;
    match mode {
        Some(mode) if !matches!(mode, Mode::Dn | Mode::An | Mode::Imm) => {
            Some(misc.cell(name, size_word(size), "Memory", 0)? + ea_time(mode)?)
        }
        _ => misc.cell(name, size_word(size), "Register", 0),
    }
}

/// `true` for an instruction whose number is the best case (see the module
/// limits).
pub(super) fn partial(
    base: &str,
    suffix: Option<&str>,
) -> bool {
    matches!(base, "divs" | "divu") && suffix != Some("l")
        || base.starts_with("bf")
        || matches!(base, "bra" | "bsr" | "jmp" | "jsr")
        || super::condition_branch(base)
        || base.starts_with("db")
}

pub(super) fn clocks(
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    rest: &str,
    taken: bool,
) -> Option<u32> {
    let tables = &*TABLES;
    let size = size_of(suffix);
    let name = base.to_ascii_uppercase();
    let long = size == Size::L;
    let ccr = rest.contains("%ccr");
    match (base, operands) {
        ("andi" | "ori" | "eori", [_, Operand::Status]) if ccr => {
            tables.misc.cell(&format!("{name} to CCR"), "byte", "Register", 0)
        }
        ("andi" | "ori" | "eori", [_, Operand::Status]) => {
            tables.status.cell(&format!("{name} to SR"), "", "Execution Time", 0)
        }
        ("move", [source, Operand::Status]) if ccr => miscellaneous("MOVE to CCR", Size::W, mode(source)),
        ("move", [source, Operand::Status]) => {
            Some(tables.status.cell("MOVE to SR", "", "Execution Time", 0)? + ea_time(mode(source)?)?)
        }
        ("move", [Operand::Status, destination]) if ccr => miscellaneous("MOVE from CCR", Size::W, mode(destination)),
        ("move", [Operand::Status, destination]) => {
            Some(tables.status.cell("MOVE from SR", "", "Execution Time", 0)? + ea_time(mode(destination)?)?)
        }
        ("move", [_, Operand::Usp]) => tables.misc.cell("MOVE to USP", "", "Register", 0),
        ("move", [Operand::Usp, _]) => tables.misc.cell("MOVE from USP", "", "Register", 0),
        ("movec", [Operand::Control, _] | [_, Operand::Control]) => {
            let register = rest
                .split(',')
                .find_map(|one| one.trim().strip_prefix('%').filter(|r| r.chars().all(|c| c.is_ascii_lowercase())));
            let group = if matches!(register, Some("sfc" | "dfc" | "usp" | "vbr" | "pcr")) {
                "MOVEC (SFC"
            } else {
                "MOVEC (CACR"
            };
            let column = if matches!(operands, [Operand::Control, _]) { "Source -> Reg" } else { "Reg -> Dest" };
            tables.misc.cell(group, "", column, 0)
        }
        ("move", [source, destination]) => {
            let grid = if long { &tables.move_long } else { &tables.move_word };
            let row = norm(label(mode(source)?, false));
            let column = grid.column(label(mode(destination)?, true))?;
            let found = grid.rows.iter().find(|fields| norm(&fields[0]) == row)?;
            clocks_of(&found[column], 0)
        }
        ("movea", [source, _]) => {
            let grid = if long { &tables.move_long } else { &tables.move_word };
            let row = norm(label(mode(source)?, false));
            let column = grid.column("An")?;
            clocks_of(&grid.rows.iter().find(|fields| norm(&fields[0]) == row)?[column], 0)
        }
        ("moveq", _) => by_destination(&tables.immediate, "MOVEQ", Size::L, Mode::Dn),
        ("move16", [source, destination]) => {
            let letter = |mode, from: char| match mode {
                Mode::Ind => Some(format!("(A{from})")),
                Mode::Post => Some(format!("(A{from})+")),
                Mode::AbsL => Some("(xxx).L".to_owned()),
                _ => None,
            };
            let row = norm(&letter(mode(source)?, 'y')?);
            let column = tables.move16.column(&letter(mode(destination)?, 'x')?)?;
            clocks_of(&tables.move16.rows.iter().find(|fields| norm(&fields[0]) == row)?[column], 0)
        }
        ("addq" | "subq", [_, destination]) => by_destination(&tables.immediate, &name, size, mode(destination)?),
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => {
            by_destination(&tables.immediate, &name, size, mode(destination)?)
        }
        ("add" | "sub" | "and" | "or" | "eor" | "cmp", [source, destination])
            if mode(source)? == Mode::Imm && mode(destination)? != Mode::An =>
        {
            by_destination(&tables.immediate, &format!("{name}I"), size, mode(destination)?)
        }
        ("add" | "sub" | "and" | "or" | "eor" | "cmp", [source, destination]) => {
            standard(&name, size, mode(source)?, mode(destination)?)
        }
        ("adda" | "suba" | "cmpa", [source, destination]) => {
            standard(&name[..name.len() - 1], size, mode(source)?, mode(destination)?)
        }
        ("mulu" | "muls" | "divu" | "divs", [source, destination]) => {
            let source = mode(source)?;
            let _ = destination;
            let simple = matches!(
                source,
                Mode::Dn | Mode::Ind | Mode::Post | Mode::Pre | Mode::Disp | Mode::PcDisp
            );
            let extra = u32::from(long && !simple);
            Some(standard(&name, size, source, Mode::Dn)? + extra)
        }
        ("clr", [destination]) => by_destination(&tables.clear, "", size, mode(destination)?).or_else(|| {
            let column = tables.clear.column(label(mode(destination)?, true))?;
            let wanted = if long { "long" } else { "byte" };
            tables
                .clear
                .rows
                .iter()
                .find(|row| norm(&row[0]).contains(wanted))
                .and_then(|row| clocks_of(&row[column], 0))
        }),
        ("tst" | "neg" | "negx" | "not" | "nbcd" | "tas", [operand]) => {
            register_or_memory(&tables.single, &name, size, mode(operand)?)
        }
        ("cas", [.., memory]) => {
            let mode = mode(memory)?;
            let simple = matches!(mode, Mode::Ind | Mode::Post | Mode::Pre | Mode::Disp);
            Some(tables.single.cell("CAS", size_word(size), "Memory", 0)? + ea_time(mode)? + u32::from(!simple))
        }
        (set, [operand]) if super::set_on_condition(set) => {
            register_or_memory(&tables.single, "Scc", Size::B, mode(operand)?)
        }
        (shift, [.., last]) if super::is_shift(shift) => {
            let group = if shift.starts_with("as") { "ASL" } else { &name };
            register_or_memory(&tables.shift, group, size, mode(last)?)
        }
        ("btst" | "bchg" | "bclr" | "bset", [source, destination]) => {
            if mode(source)? == Mode::Dn {
                let destination = mode(destination)?;
                let size = if destination == Mode::Dn { Size::L } else { Size::B };
                register_or_memory(&tables.bit_dynamic, &name, size, destination)
            } else {
                let destination = mode(destination)?;
                let size = if destination == Mode::Dn { Size::L } else { Size::B };
                by_destination(&tables.bit_static, &name, size, destination)
            }
        }
        (field, _) if field.starts_with("bf") => {
            by_destination(&tables.bit_field, &format!("{name} (< 5 Bytes)"), Size::L, effective(operands)?)
        }
        ("lea" | "pea", [source, ..]) => {
            let column = label(mode(source)?, false);
            tables.lea.cell(&name, "", column, 0)
        }
        ("movem", [Operand::List(registers), memory]) => {
            tables.lea.cell("MOVEM Reg->Mem", "", label(mode(memory)?, false), *registers)
        }
        ("movem", [memory, Operand::List(registers)]) => {
            tables.lea.cell("MOVEM Mem->Reg", "", label(mode(memory)?, false), *registers)
        }
        ("abcd" | "sbcd" | "addx" | "subx", [first, _]) => {
            let column = if mode(first)? == Mode::Dn { "op Dy,Dx" } else { "op <ea>y,<ea>x" };
            tables.multiprecision.cell(&name, size_word(size), column, 0)
        }
        ("cmpm", _) => tables.multiprecision.cell("CMPM", size_word(size), "op <ea>y,<ea>x", 0),
        ("pack" | "unpk", [first, ..]) => miscellaneous(&name, Size::B, mode(first)),
        ("chk", [source, _]) => miscellaneous("CHK", size, mode(source)),
        ("moves", [first, second]) => {
            let (row, memory) = if matches!(mode(first)?, Mode::Dn | Mode::An) {
                ("Rn -> Dest <DFC>", mode(second)?)
            } else {
                ("Source<SFC> -> Rn", mode(first)?)
            };
            tables.moves.cell(row, size_word(size), label(memory, true), 0)
        }
        ("ext", _) => tables.misc.cell("EXT", if long { "long" } else { "word" }, "Register", 0),
        ("extb", _) => tables.misc.cell("EXTB", "", "Register", 0),
        ("exg" | "swap" | "nop" | "unlk" | "stop" | "reset" | "trapv", _) => tables.misc.cell(&name, "", "Register", 0),
        ("link", _) => tables.misc.cell("LINK", if long { "long" } else { "word" }, "Register", 0),
        (trap, _) if trap.starts_with("trap") && trap != "trap" => {
            tables.misc.cell(if trap == "trapf" { "TRAPF" } else { "TRAPcc" }, "", "Register", 0)
        }
        ("rts" | "rtr" | "rtd" | "rte", _) => {
            let found = tables.leave.rows.iter().find(|row| norm(&row[0]) == norm(&name))?;
            clocks_of(&found[1], 0)
        }
        ("bra", _) => tables.branch.cell("BRA", "", branch_column(true), 0),
        ("bsr", _) => tables.branch.cell("BSR", "", branch_column(true), 0),
        (condition, [_]) if super::condition_branch(condition) => {
            tables.branch.cell("Bcc", "", branch_column(taken), 0)
        }
        (dbcc, [_, _]) if dbcc.starts_with("db") => {
            let row = if matches!(dbcc, "dbf" | "dbra") { "DBRA" } else { "DBcc" };
            tables.branch.cell(row, "", branch_column(taken), 0)
        }
        ("jmp" | "jsr", [target]) => jump(&name, if *target == Operand::Target { Mode::AbsL } else { mode(target)? }),
        _ => None,
    }
}
