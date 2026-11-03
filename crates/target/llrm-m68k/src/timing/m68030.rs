//! The MC68030's clocks, instruction-cache case: MC68030UM Section 11.6
//! (printed pages 11-26 to 11-51), read from their transcription in `m68030/`.
//!
//! # How it works
//! An instruction's clocks are its row's I-cache cell plus the effective
//! address table its mark names (`*` fetch, `**` calculate or fetch immediate,
//! `#`, `%`, by the footnote of that table). A row is looked up by its printed
//! label and mark: a mark the classification here does not expect is "not in
//! the tables", never a guess. The cache case assumes two-clock memory.
//!
//! # Limits
//! 1. Memory-indirect and full-format modes are not priced (the pin emits
//!    none).
//! 2. A bit field in memory is priced as under five bytes; the five-byte row is
//!    4 to 8 clocks more, so it is partial.
//! 3. A shift by a register is priced at the "count <= size" row (`%`) and is
//!    partial; the `+` row (count above the size) is 2 to 4 clocks more.
//! 4. Exception processing and the coprocessor rows are not priced.

use std::sync::LazyLock;

use super::table::{Cell, key, parse_cell};
use super::{Mode, Operand, Size, mode, places, size_of};

struct Row {
    mark: String,
    label: String,
    cell: Cell,
}

fn load(csv: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in csv.lines().skip(2) {
        let mut fields = vec![String::new()];
        let mut quoted = false;
        for c in line.chars() {
            match c {
                '"' => quoted = !quoted,
                ',' if !quoted => fields.push(String::new()),
                c => fields.last_mut().unwrap().push(c),
            }
        }
        if fields.len() < 7 {
            continue;
        }
        let Some(cell) = parse_cell(&fields[6]) else { continue };
        rows.push(Row {
            mark: fields[1].split_whitespace().collect(),
            label: key(&format!("{} {}", fields[2], fields[3])),
            cell,
        });
    }
    rows
}

static TABLES: LazyLock<Vec<Vec<Row>>> = LazyLock::new(|| {
    vec![
        load(include_str!("m68030/t11.6.1.csv")),
        load(include_str!("m68030/t11.6.2.csv")),
        load(include_str!("m68030/t11.6.3.csv")),
        load(include_str!("m68030/t11.6.4.csv")),
        load(include_str!("m68030/t11.6.5.csv")),
        load(include_str!("m68030/t11.6.6.csv")),
        load(include_str!("m68030/t11.6.7.csv")),
        load(include_str!("m68030/t11.6.8.csv")),
        load(include_str!("m68030/t11.6.9.csv")),
        load(include_str!("m68030/t11.6.10.csv")),
        load(include_str!("m68030/t11.6.11.csv")),
        load(include_str!("m68030/t11.6.12.csv")),
        load(include_str!("m68030/t11.6.13.csv")),
        load(include_str!("m68030/t11.6.14.csv")),
        load(include_str!("m68030/t11.6.15.csv")),
        load(include_str!("m68030/t11.6.16.csv")),
        load(include_str!("m68030/t11.6.17.csv")),
        load(include_str!("m68030/t11.6.18.csv")),
    ]
});

/// The cell of subsection 11.6.`table` whose printed label is `label` and whose
/// mark is `mark`.
fn get(
    table: usize,
    label: &str,
    mark: &str,
) -> Option<Cell> {
    let label = key(label);
    TABLES[table - 1].iter().find(|row| row.label == label && row.mark == mark).map(|row| row.cell)
}

/// A row of the effective address tables (11.6.1 to 11.6.5), by the mode's
/// printed name.
fn address(
    table: usize,
    label: &str,
) -> Option<u32> {
    let label = key(label);
    TABLES[table - 1]
        .iter()
        .find(|row| row.label.starts_with(&label) && row.label[label.len()..].starts_with(',') || row.label == label)
        .map(|row| row.cell.base)
}

fn mode_label(mode: Mode) -> Option<&'static str> {
    Some(match mode {
        Mode::Dn => "Dn",
        Mode::An => "An",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp | Mode::PcDisp => "(d16,An) or (d16,PC)",
        Mode::AbsW => "(xxx).W",
        Mode::AbsL => "(xxx).L",
        Mode::Index | Mode::PcIndex => "(d8,An,Xn) or (d8,PC,Xn)",
        Mode::Imm => return None,
    })
}

/// 11.6.1: fetching an operand.
fn fea(
    mode: Mode,
    size: Size,
) -> Option<u32> {
    let label = match mode {
        Mode::Imm if size == Size::L => "#(data).L",
        Mode::Imm => "#(data).W",
        other => mode_label(other)?,
    };
    address(1, label)
}

/// 11.6.3: calculating an address.
fn cea(mode: Mode) -> Option<u32> {
    address(3, mode_label(mode)?)
}

/// 11.6.5: a jump address (`(An)` and the like; `(d16,PC)` is `(d16,An)`'s
/// row).
fn jea(mode: Mode) -> Option<u32> {
    let label = match mode {
        Mode::Disp | Mode::PcDisp => "(d16,An)",
        Mode::Index | Mode::PcIndex => "(d8,An,Xn) or (d8,PC,Xn)",
        other => mode_label(other)?,
    };
    address(5, label)
}

/// 11.6.2 and 11.6.4: an immediate and the address of its destination, by
/// the immediate's width.
fn immediate_with(
    table: usize,
    destination: Mode,
    size: Size,
) -> Option<u32> {
    let width = if size == Size::L { "L" } else { "W" };
    let place = match destination {
        Mode::AbsW => "$XXX.W",
        Mode::AbsL => "$XXX.L",
        Mode::Disp | Mode::PcDisp => "(d16,An)",
        Mode::Index | Mode::PcIndex => "(d8,An,Xn) or (d8,PC,Xn)",
        other => mode_label(other)?,
    };
    address(table, &format!("#<data>.{width},{place}"))
}

fn fiea(
    destination: Mode,
    size: Size,
) -> Option<u32> {
    immediate_with(2, destination, size)
}

fn ciea(
    destination: Mode,
    size: Size,
) -> Option<u32> {
    immediate_with(4, destination, size)
}

fn plain(cell: Option<Cell>) -> Option<u32> {
    cell.map(|cell| cell.base)
}

/// The MOVE rows of 11.6.6: by destination, `Rn` source (no fetch) or any.
fn move_cell(
    source: Mode,
    destination: Mode,
    size: Size,
) -> Option<u32> {
    let place = match destination {
        Mode::Dn => "Dn",
        Mode::An => "An",
        Mode::Ind => "(An)",
        Mode::Post => "(An)+",
        Mode::Pre => "-(An)",
        Mode::Disp => "(d16,An)",
        Mode::AbsW => "XXX.W",
        Mode::AbsL => "XXX.L",
        Mode::Index => "(d8,An,Xn)",
        _ => return None,
    };
    let register = matches!(source, Mode::Dn | Mode::An);
    // Rn-source rows exist for the first five destinations; the rest are `EA`
    // rows
    let cell = if register {
        get(6, &format!("MOVE Rn,{place}"), "").or_else(|| get(6, &format!("MOVE EA,{place}"), "*"))
    } else {
        get(6, &format!("MOVE EA,{place}"), "*").or_else(|| get(6, &format!("MOVE SOURCE,{place}"), "*"))
    }?;
    let fetch = if register { 0 } else { fea(source, size)? };
    Some(cell.base + fetch)
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
    match (base, operands) {
        ("andi" | "ori" | "eori", [_, Operand::Status]) => plain(get(16, "ANDI to SR", "")),
        ("move", [_, Operand::Usp]) => plain(get(7, "MOVE An,USP", "")),
        ("move", [Operand::Usp, _]) => plain(get(7, "MOVE USP,An", "")),
        ("move", [source, Operand::Status]) => Some(plain(get(7, "MOVE EA,SR", "#"))? + fea(mode(source)?, Size::W)?),
        ("move", [Operand::Status, destination]) => match mode(destination)? {
            Mode::Dn => plain(get(7, "MOVE SR,Dn", "")),
            memory => Some(plain(get(7, "MOVE SR,Mem", "*"))? + cea(memory)?),
        },
        ("move" | "movea", [source, destination]) => move_cell(mode(source)?, mode(destination)?, size),
        ("moveq", [_, _]) => plain(get(9, "MOVEQ #<data>,Dn", "")),
        ("add" | "sub" | "adda" | "suba" | "and" | "or" | "eor" | "cmp" | "cmpa", [source, destination]) => {
            let (source, destination) = (mode(source)?, mode(destination)?);
            let name = base.to_ascii_uppercase();
            let register = matches!(source, Mode::Dn | Mode::An);
            match (base, destination) {
                (_, Mode::An) => {
                    let label = |what: &str| match base {
                        "cmp" | "cmpa" => format!("CMPA {what},An"),
                        _ if long => format!("{}A.L {what},An", &name[..3]),
                        _ => format!("{}A.W {what},An", &name[..3]),
                    };
                    if register {
                        plain(get(8, &label("Rn"), ""))
                    } else {
                        Some(
                            plain(
                                get(8, &label("EA"), "*")
                                    .or_else(|| get(8, &label("EA").replace("ADDA.W", "ADD.W"), "*")),
                            )? + fea(source, size)?,
                        )
                    }
                }
                ("eor", Mode::Dn) => plain(get(8, "EOR Dn,Dn", "")),
                (_, Mode::Dn) if source == Mode::Imm && base != "cmp" => {
                    Some(plain(get(9, &format!("{} #<data>,Dn", immediate_name(base)?), "**"))? + fiea(Mode::Dn, size)?)
                }
                (_, Mode::Dn) if register => {
                    plain(get(8, &format!("{name} Rn,Dn"), "").or_else(|| get(8, &format!("{name} Dn,Dn"), "")))
                }
                (_, Mode::Dn) => Some(plain(get(8, &format!("{name} EA,Dn"), "*"))? + fea(source, size)?),
                (_, memory) if source == Mode::Dn => {
                    Some(plain(get(8, &format!("{name} Dn,EA"), "*"))? + fea(memory, size)?)
                }
                (_, memory) if source == Mode::Imm => {
                    Some(plain(get(9, &format!("{} #<data>,Mem", immediate_name(base)?), "**"))? + fiea(memory, size)?)
                }
                _ => None,
            }
        }
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => {
            let name = immediate_name(base)?;
            match mode(destination)? {
                Mode::Dn => Some(plain(get(9, &format!("{name} #<data>,Dn"), "**"))? + fiea(Mode::Dn, size)?),
                memory => Some(plain(get(9, &format!("{name} #<data>,Mem"), "**"))? + fiea(memory, size)?),
            }
        }
        ("addq" | "subq", [_, destination]) => {
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn | Mode::An => plain(get(9, &format!("{name} #<data>,Rn"), "")),
                memory => Some(plain(get(9, &format!("{name} #<data>,Mem"), "*"))? + fea(memory, size)?),
            }
        }
        ("clr", [destination]) => match mode(destination)? {
            Mode::Dn => plain(get(11, "CLR Dn", "")),
            memory => Some(plain(get(11, "CLR Mem", "**"))? + cea(memory)?),
        },
        ("neg" | "negx" | "not", [destination]) => {
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn => plain(get(11, &format!("{name} Dn"), "")),
                memory => Some(plain(get(11, &format!("{name} Mem"), "*"))? + fea(memory, size)?),
            }
        }
        ("tst", [destination]) => match mode(destination)? {
            Mode::Dn | Mode::An => plain(get(11, "TST Dn", "")),
            memory => Some(plain(get(11, "TST Mem", "*"))? + fea(memory, size)?),
        },
        ("scc" | "seq" | "sne" | "st" | "sf", [destination]) | (_, [destination]) if super::set_on_condition(base) => {
            match mode(destination)? {
                Mode::Dn => plain(get(11, "Scc Dn", "")),
                memory => Some(plain(get(11, "Scc Mem", "**"))? + cea(memory)?),
            }
        }
        ("tas", [destination]) => match mode(destination)? {
            Mode::Dn => plain(get(11, "TAS Dn", "")),
            memory => Some(plain(get(11, "TAS Mem", "**"))? + cea(memory)?),
        },
        ("moves", [first, second]) => match (mode(first)?, mode(second)?) {
            (Mode::Dn | Mode::An, memory) => Some(plain(get(7, "MOVES Rn,EA", "%"))? + jea(memory)?),
            (memory, _) => Some(plain(get(7, "MOVES EA,Rn", "%"))? + jea(memory)?),
        },
        ("ext", _) => plain(get(11, "EXT Dn", "")),
        ("swap", _) => plain(get(7, "SWAP Dn", "")),
        ("exg", _) => plain(get(7, "EXG Ry,Rx", "")),
        ("nop", _) => plain(get(16, "NOP", "")),
        (shift, [_, _]) if super::is_shift(shift) => {
            let by_count = matches!(operands, [Operand::Mode(Mode::Imm, _), _]);
            let (name, mark) = shift_name(shift);
            let label = format!("{name} {},{}", if by_count { "#<data>" } else { "Dx" }, "Dy");
            let cell = if name == "ROXd" {
                get(12, "ROXd Dn", "")
            } else if by_count {
                get(12, &label, "")
            } else {
                get(12, &label, "%").or_else(|| get(12, &label, ""))
            };
            let _ = (mark, places(operands));
            plain(cell)
        }
        (shift, [memory]) if super::is_shift(shift) => {
            let (name, _) = shift_name(shift);
            Some(plain(get(12, &format!("{name} Mem by 1"), "*"))? + fea(mode(memory)?, Size::W)?)
        }
        ("mulu" | "muls" | "divu" | "divs", [source, _]) => {
            let name = base.to_ascii_uppercase();
            let source = mode(source)?;
            if long {
                // `**`: the extension word is fetched with the operand
                Some(plain(get(8, &format!("{name}.L EA,Dn"), "**+"))? + fiea(source, Size::W)?)
            } else {
                Some(plain(get(8, &format!("{name}.W EA,Dn"), "*+"))? + fea(source, Size::W)?)
            }
        }
        ("btst" | "bchg" | "bclr" | "bset", [source, destination]) => {
            let name = base.to_ascii_uppercase();
            let by_data = mode(source)? == Mode::Imm;
            let what = if by_data { "#<data>" } else { "Dn" };
            match mode(destination)? {
                Mode::Dn => plain(get(13, &format!("{name} {what},Dn"), "")),
                memory if by_data => {
                    Some(plain(get(13, &format!("{name} #<data>,Mem"), "#"))? + fiea(memory, Size::B)?)
                }
                memory => Some(plain(get(13, &format!("{name} Dn,Mem"), "*"))? + fea(memory, Size::B)?),
            }
        }
        (field, [first, ..]) if field.starts_with("bf") => {
            let name = field.to_ascii_uppercase();
            let memory = operands.iter().filter_map(mode).find(|mode| !matches!(mode, Mode::Dn | Mode::Imm));
            match memory {
                None => plain(get(14, &format!("{name} Dn"), "")),
                Some(memory) => {
                    let _ = first;
                    Some(plain(get(14, &format!("{name} Mem (<5 Bytes)"), "*"))? + ciea(memory, Size::W)?)
                }
            }
        }
        ("abcd" | "sbcd" | "addx" | "subx", [first, _]) => {
            let name = base.to_ascii_uppercase();
            if mode(first)? == Mode::Dn {
                plain(get(10, &format!("{name} Dn,Dn"), ""))
            } else {
                plain(get(10, &format!("{name} -(An),-(An)"), ""))
            }
        }
        ("cmpm", [_, _]) => plain(get(10, "CMPM (An)+,(An)+", "")),
        ("pack" | "unpk", [first, _, _]) => {
            let name = base.to_ascii_uppercase();
            if mode(first)? == Mode::Dn {
                plain(get(10, &format!("{name} Dn,Dn,#<data>"), ""))
            } else {
                plain(get(10, &format!("{name} -(An),-(An),#<data>"), ""))
            }
        }
        ("chk", [source, _]) => match mode(source)? {
            Mode::Dn => plain(get(16, "CHK Dn,Dn (No Exception)", "")),
            memory => Some(plain(get(16, "CHK EA,Dn (No Exception)", "*"))? + fea(memory, Size::W)?),
        },
        ("cmp2" | "chk2", [source, _]) => {
            let (table, label, mark) =
                if base == "cmp2" { (8, "CMP2 EA,Rn", "**+") } else { (16, "CHK2 Mem,Rn (No Exception)", "#+") };
            Some(plain(get(table, label, mark))? + fiea(mode(source)?, Size::W)?)
        }
        ("cas", [_, _, memory]) => {
            Some(plain(get(16, "CAS (Successful Compare)", "##"))? + ciea(mode(memory)?, Size::W)?)
        }
        ("trapv", []) => plain(get(17, "TRAPV (No Trap)", "")),
        ("trapcc" | "trapt" | "trapf" | "trapeq" | "trapne", _) => plain(get(17, "TRAPcc (No Trap)", "")),
        ("bkpt", [_]) => plain(get(17, "BKPT", "")),
        ("stop", [_]) => plain(get(17, "STOP", "")),
        ("reset", []) => plain(get(17, "RESET Instruction", "")),
        ("movep", [first, _]) => {
            let width = if long { 'L' } else { 'W' };
            if mode(first)? == Mode::Dn {
                plain(get(7, &format!("MOVEP.{width} Dn,(d16,An)"), ""))
            } else {
                plain(get(7, &format!("MOVEP.{width} (d16,An),Dn"), ""))
            }
        }
        ("movec", [Operand::Control, _]) => plain(get(7, "MOVEC Cr,Rn", "")),
        ("movec", [_, Operand::Control]) => plain(get(7, "MOVEC Rn,Cr-A", "")),
        ("lea", [source, _]) => Some(plain(get(16, "LEA", "**"))? + cea(mode(source)?)?),
        ("pea", [source]) => Some(plain(get(16, "PEA", "**"))? + cea(mode(source)?)?),
        ("jmp" | "jsr", [target]) => {
            let name = base.to_ascii_uppercase();
            let address = match target {
                Operand::Target => jea(if bytes == 4 { Mode::AbsW } else { Mode::AbsL })?,
                other => jea(mode(other)?)?,
            };
            Some(plain(get(16, &name, "%"))? + address)
        }
        ("bra", [_]) => plain(get(15, "Bcc (Taken)", "")),
        ("bsr", [_]) => plain(get(16, "BSR", "")),
        (condition, [_]) if super::condition_branch(condition) => {
            if taken {
                plain(get(15, "Bcc (Taken)", ""))
            } else if bytes <= 2 {
                plain(get(15, "Bcc.B (Not Taken)", ""))
            } else {
                plain(get(15, "Bcc.W (Not Taken)", ""))
            }
        }
        (dbcc, [_, _]) if dbcc.starts_with("db") => match (dbcc == "dbt", taken) {
            (true, _) => plain(get(15, "DBcc (cc=True)", "")),
            (false, true) => plain(get(15, "DBcc (cc=False, Count Not Expired)", "")),
            (false, false) => plain(get(15, "DBcc (cc=False, Count Expired)", "")),
        },
        ("rts", []) => plain(get(16, "RTS", "")),
        ("rtr", []) => plain(get(16, "RTR", "")),
        ("rtd", [_]) => plain(get(16, "RTD", "")),
        ("link", [_, _]) => plain(get(16, if long { "LINK.L" } else { "LINK.W" }, "")),
        ("unlk", [_]) => plain(get(16, "UNLK", "")),
        ("movem", [Operand::List(registers), memory]) => {
            let cell = get(7, "MOVEM RL,EA", "%+")?;
            Some(cell.base + cell.per * registers + ciea(mode(memory)?, Size::W)?)
        }
        ("movem", [memory, Operand::List(registers)]) => {
            let cell = get(7, "MOVEM EA,RL", "%+")?;
            Some(cell.base + cell.per * registers + ciea(mode(memory)?, Size::W)?)
        }
        _ => None,
    }
}

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

/// The row name of a shift or rotate in 11.6.12 (`LSd` stands for `LSL` and
/// `LSR`).
fn shift_name(shift: &str) -> (&'static str, &'static str) {
    match shift {
        "lsl" | "lsr" => ("LSd", ""),
        "asl" => ("ASL", ""),
        "asr" => ("ASR", ""),
        "rol" | "ror" => ("ROd", ""),
        _ => ("ROXd", ""),
    }
}
