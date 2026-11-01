//! The MC68040's clocks: MC68040UM Section 10 (printed pages 10-9 to 10-28),
//! read from their transcription in `m68040/`.
//!
//! # How it works
//! The manual gives each instruction an `<ea> calculate` time and an `execute`
//! time written `lead L + base`, per addressing mode. The model's clocks are
//! the busiest stage of the instruction in a stall-free stream:
//! `max(<ea> calculate, base)`. The lead time is the stall the execute stage
//! absorbs, so it is hidden here; `calculate + lead + base` is the same
//! instruction after a dependency.
//!
//! # Limits
//! 1. The 68040 is pipelined: one number per instruction is a throughput
//!    estimate, not a latency. Stalls, write-back, cache and ATC misses, and
//!    the FPU's own pipeline are not in it.
//! 2. Memory-indirect and full-format modes are not priced (the pin emits
//!    none).
//! 3. Floating-point instructions are not priced.
//! 4. A taken branch is priced as the manual's "Branch Taken" row; the branch
//!    cache is not modelled.

use std::sync::LazyLock;

use super::table::key;
use super::{Mode, Operand, Size, mode, size_of};

struct Grid {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse(csv: &str) -> Grid {
    let mut lines = csv.lines().skip(1);
    let split = |line: &str| {
        let mut fields = vec![String::new()];
        let mut quoted = false;
        for c in line.chars() {
            match c {
                '"' => quoted = !quoted,
                ',' if !quoted => fields.push(String::new()),
                c => fields.last_mut().unwrap().push(c),
            }
        }
        fields
    };
    let header = split(lines.next().expect("a header row"));
    Grid { header, rows: lines.filter(|line| !line.is_empty()).map(split).collect() }
}

/// The instruction names a header column group lists: `ADD, AND, EOR` is three,
/// `MULS.W/L` two, footnote marks and `(<ea>, Dn)` dropped.
fn group_names(header: &str) -> Vec<String> {
    let mut text = header.to_owned();
    for suffix in [" <ea> Calculate", " Execute"] {
        if let Some(rest) = text.strip_suffix(suffix) {
            text = rest.to_owned();
        }
    }
    while let (Some(open), Some(close)) = (text.find('['), text.find(']')) {
        text.replace_range(open..=close, "");
    }
    for drop in ["(<ea>, Dn)", "(<ea>, Rn)", "*"] {
        text = text.replace(drop, "");
    }
    text.split(", ")
        .map(str::trim)
        .flat_map(|name| match name.split_once('/') {
            Some((head, tail)) if head.contains('.') => {
                let stem = head.split_once('.').unwrap().0;
                vec![head.to_owned(), format!("{stem}.{tail}")]
            }
            _ => vec![name.to_owned()],
        })
        .map(|name| key(&name))
        .collect()
}

/// One instruction group of a grid: its names and its two columns.
struct Group {
    names: Vec<String>,
    grid: usize,
    calculate: usize,
    execute: usize,
}

struct Tables {
    mode_grids: Vec<Grid>,
    groups: Vec<Group>,
    moves: Grid,
    misc: Grid,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let files = [
        include_str!("m68040/t10.6-1.csv"),
        include_str!("m68040/t10.6-2.csv"),
        include_str!("m68040/t10.6-3.csv"),
        include_str!("m68040/t10.6-4.csv"),
        include_str!("m68040/t10.6-5.csv"),
        include_str!("m68040/t10.6-6.csv"),
        include_str!("m68040/t10.6-7.csv"),
        include_str!("m68040/t10.6-8.csv"),
        include_str!("m68040/t10.6-9.csv"),
        include_str!("m68040/t10.6-10.csv"),
        include_str!("m68040/t10.6-11.csv"),
        include_str!("m68040/t10.6-12.csv"),
        include_str!("m68040/t10.6-13.csv"),
        include_str!("m68040/t10.6-14.csv"),
        include_str!("m68040/t10.6-15.csv"),
        include_str!("m68040/t10.6-16.csv"),
    ];
    let mode_grids: Vec<Grid> = files.iter().map(|csv| parse(csv)).collect();
    let mut groups = Vec::new();
    for (grid, one) in mode_grids.iter().enumerate() {
        for column in (1..one.header.len()).step_by(2) {
            groups.push(Group {
                names: group_names(&one.header[column]),
                grid,
                calculate: column,
                execute: column + 1,
            });
        }
    }
    Tables {
        mode_grids,
        groups,
        moves: parse(include_str!("m68040/t10.4.csv")),
        misc: parse(include_str!("m68040/t10.5.csv")),
    }
});

/// A cell is `lead L + base`; `a/b` lists two cases (`pick` chooses; footnotes
/// say which: immediate or register count, word or long); `D'` and `A'` are the
/// registers of a `movem` list (`registers` together); footnote marks are
/// dropped. A dash is no entry.
fn stage(
    text: &str,
    pick: usize,
    registers: u32,
) -> Option<(u32, u32)> {
    let mut text = text.trim().to_owned();
    while let (Some(open), Some(close)) = (text.find('['), text.find(']')) {
        text.replace_range(open..=close, "");
    }
    let text = text.replace('*', "");
    let text = text.trim();
    if text.is_empty() || text.starts_with('\u{2014}') || text == "-" {
        return None;
    }
    let text = text.split('/').nth(pick).or_else(|| text.split('/').next()).unwrap_or(text).trim();
    let (mut lead, mut base) = (0, 0);
    for (index, term) in text.split('+').map(str::trim).enumerate() {
        if let Some(count) = term.strip_suffix('L').filter(|_| index == 0) {
            lead = count.trim().parse().ok()?;
        } else if term == "D'" {
            base += registers;
        } else if term == "A'" || term == "A" || term == "N" {
            // the address registers of the list are counted with the data
            // registers
        } else {
            base += term.parse::<u32>().ok()?;
        }
    }
    Some((lead, base))
}

/// The busiest stage: see the module docs.
fn busiest(
    calculate: &str,
    execute: &str,
    pick: usize,
    registers: u32,
) -> Option<u32> {
    let (_, calculate) = stage(calculate, pick, registers)?;
    let (_, base) = stage(execute, pick, registers)?;
    Some(calculate.max(base))
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Dn => "Dn",
        Mode::An => "An",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp => "(d16,An)",
        Mode::PcDisp => "(d16,PC)",
        Mode::AbsW | Mode::AbsL => "(xxx).W, (xxx).L",
        Mode::Imm => "#<xxx>",
        Mode::Index => "(d8,An,Xn)",
        Mode::PcIndex => "(d8,PC,Xn)",
    }
}

/// Section 10.6: `name` at the addressing mode of its `<ea>` operand.
fn by_mode(
    name: &str,
    operand: Mode,
) -> Option<u32> {
    by_mode_with(name, operand, 0, 0)
}

fn by_mode_with(
    name: &str,
    operand: Mode,
    pick: usize,
    registers: u32,
) -> Option<u32> {
    let tables = &*TABLES;
    let name = key(name);
    let group = tables.groups.iter().find(|group| group.names.contains(&name))?;
    let grid = &tables.mode_grids[group.grid];
    let row = key(mode_label(operand));
    let found = grid.rows.iter().find(|fields| key(&fields[0]) == row)?;
    busiest(&found[group.calculate], &found[group.execute], pick, registers)
}

/// Section 10.5: the row of `instruction` (carried down) whose condition column
/// starts with `condition`.
fn miscellaneous(
    instruction: &str,
    condition: &str,
) -> Option<u32> {
    let (instruction, condition) = (key(instruction), key(condition));
    let mut carried = String::new();
    for fields in &TABLES.misc.rows {
        if !fields[0].is_empty() {
            carried = key(fields[0].split('[').next().unwrap_or(""));
        }
        if carried.starts_with(&instruction) && key(&fields[1]).starts_with(&condition) {
            return busiest(&fields[2], &fields[3], 0, 0);
        }
    }
    None
}

/// Section 10.4: a move, by source row and destination column.
fn move_clocks(
    source: Mode,
    destination: Mode,
) -> Option<u32> {
    let grid = &TABLES.moves;
    // a source address register reads as the data register row (no `An` row is
    // printed)
    let row = key(mode_label(if source == Mode::An { Mode::Dn } else { source }));
    let found = grid.rows.iter().find(|fields| key(&fields[0]) == row)?;
    let column = key(&format!("DEST {} <ea> Calculate", mode_label(destination)));
    let calculate = grid.header.iter().position(|name| key(name) == column)?;
    busiest(&found[calculate], &found[calculate + 1], 0, 0)
}

/// The operand whose addressing mode picks the row: the memory one of two, else
/// the first.
fn effective(operands: &[Operand]) -> Option<Mode> {
    let modes: Vec<Mode> = operands.iter().filter_map(mode).collect();
    modes
        .iter()
        .copied()
        .find(|mode| !matches!(mode, Mode::Dn | Mode::An | Mode::Imm))
        .or_else(|| modes.last().copied())
}

pub(super) fn clocks(
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    bytes: u32,
    taken: bool,
) -> Option<u32> {
    let size = size_of(suffix);
    let name = base.to_ascii_uppercase();
    let long = size == Size::L;
    match (base, operands) {
        ("andi" | "ori" | "eori", [_, Operand::Status]) => miscellaneous(&format!("{name} #<xxx>,SR"), ""),
        ("move", [source, Operand::Status]) => by_mode("MOVE to SR", mode(source)?),
        ("move", [Operand::Status, destination]) => by_mode("MOVE from SR", mode(destination)?),
        ("move", [_, Operand::Usp]) => miscellaneous("MOVE USP", "An,USP"),
        ("move", [Operand::Usp, _]) => miscellaneous("MOVE USP", "USP,An"),
        ("movec", [Operand::Control, _]) => miscellaneous("MOVEC", "Rc,Rn"),
        ("movec", [_, Operand::Control]) => miscellaneous("MOVEC", "Rn,Rc"),
        ("move", [source, destination]) => move_clocks(mode(source)?, mode(destination)?),
        ("movea", [source, _]) => by_mode("MOVEA.L", mode(source)?),
        ("moveq", _) => miscellaneous("MOVEQ", ""),
        ("add" | "sub" | "and" | "or" | "eor" | "cmp", [source, destination])
            if mode(source)? == Mode::Imm && base != "cmp" =>
        {
            by_mode(&format!("{}I", name), mode(destination)?)
        }
        (
            "add" | "sub" | "and" | "or" | "eor" | "cmp" | "tst" | "adda" | "suba" | "clr" | "neg" | "negx" | "not"
            | "nbcd" | "tas" | "pea" | "lea" | "addi" | "subi" | "andi" | "ori" | "eori" | "cmpi" | "addq" | "subq"
            | "chk" | "scc" | "jmp" | "jsr",
            _,
        ) => {
            let operand = match operands {
                [Operand::Target] => Mode::AbsL,
                _ => effective(operands)?,
            };
            let name = if super::set_on_condition(base) { "Scc".to_owned() } else { name };
            by_mode(&name, operand)
        }
        ("cmpa", _) => by_mode("CMPA.L", effective(operands)?),
        (set, [destination]) if super::set_on_condition(set) => by_mode("Scc", mode(destination)?),
        (shift, [_, _]) | (shift, [_]) if super::is_shift(shift) => {
            let group = match shift {
                "asl" => "ASL".to_owned(),
                "asr" | "lsl" | "lsr" => name.clone(),
                "rol" | "ror" => name.clone(),
                _ => name.clone(),
            };
            let by_register = matches!(operands, [Operand::Mode(Mode::Dn, _), _]);
            by_mode_with(&group, effective(operands)?, usize::from(by_register), 0)
        }
        ("mulu" | "muls" | "divu" | "divs", _) => by_mode_with(
            &format!("{name}.{}", if long { "L" } else { "W" }),
            effective(operands)?,
            usize::from(long),
            0,
        ),
        ("btst" | "bchg" | "bclr" | "bset", [source, destination]) => {
            by_mode_with(&name, mode(destination)?, usize::from(mode(source)? == Mode::Dn), 0)
        }
        (field, _) if field.starts_with("bf") => by_mode(&name, effective(operands)?),
        ("movem", [Operand::List(registers), memory]) => {
            by_mode_with("MOVEM <list>,<ea>", mode(memory)?, 0, *registers)
        }
        ("movem", [memory, Operand::List(registers)]) => {
            by_mode_with("MOVEM.L <ea>,<list>", mode(memory)?, 0, *registers)
        }
        ("abcd" | "sbcd" | "addx" | "subx", [first, _]) => {
            miscellaneous(&name, if mode(first)? == Mode::Dn { "Dy,Dx" } else { "-(Ay),-(Ax)" })
        }
        ("cmpm", _) => miscellaneous("CMPM", ""),
        ("ext", _) => miscellaneous("EXT", if long { "Long" } else { "Word" }),
        ("extb", _) => miscellaneous("EXTB", ""),
        ("exg", [first, second]) => miscellaneous(
            "EXG",
            match (mode(first)?, mode(second)?) {
                (Mode::Dn, Mode::Dn) => "Dy,Dx",
                (Mode::An, Mode::An) => "Ay,Ax",
                _ => "Dy,Ax",
            },
        ),
        ("swap" | "nop" | "link" | "unlk" | "rts" | "rtr" | "rtd" | "bsr" | "trap" | "reset", _) => {
            miscellaneous(&name, "")
        }
        ("bra", _) => miscellaneous("BRA", if taken { "Branch Taken" } else { "Branch Not Taken" }),
        (condition, [_]) if super::condition_branch(condition) => {
            let _ = bytes;
            miscellaneous("Bcc", if taken { "Branch Taken" } else { "Branch Not Taken" })
        }
        (dbcc, [_, _]) if dbcc.starts_with("db") => miscellaneous(
            "DBcc",
            match (dbcc == "dbt", taken) {
                (true, _) => "True",
                (false, true) => "False, Count > ",
                (false, false) => "False, Count = ",
            },
        ),
        ("trapv", []) => miscellaneous("TRAPV", "Not Taken"),
        ("trapcc" | "trapt" | "trapf" | "trapeq" | "trapne", _) => miscellaneous("TRAPcc", "Not Taken"),
        ("movep", [first, _]) => {
            let width = if long { 'L' } else { 'W' };
            if mode(first) == Some(Mode::Dn) {
                miscellaneous("MOVEP", &format!("MOVEP.{width} Dn,d16(An)"))
            } else {
                miscellaneous("MOVEP", &format!("MOVEP.{width} d16(An),Dn"))
            }
        }
        _ => None,
    }
}
