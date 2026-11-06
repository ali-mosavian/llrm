//! The clocks of the MC68000 and MC68010: MC68000UM Section 8 (Tables 8-1 to
//! 8-14, printed pages 8-2 to 8-11) and Section 9 (Tables 9-1 to 9-19, printed
//! pages 9-2 to 9-13), read from their transcriptions in `m68000/` and
//! `m68010/`.
//!
//! # How it works
//! Which table, row and column an instruction is in follows the manual's
//! layout, the same for both processors; the numbers come from the CSVs, never
//! from this file. A cell marked `+` gets the effective address time of the
//! first table added.
//!
//! # Limits
//! 1. Loop mode (MC68010 Tables 9-3, 9-5, 9-7, 9-11, 9-13, 9-17's right half)
//!    is for a `DBcc` closing a one-instruction loop; an instruction is priced
//!    alone, so it is not used.
//! 2. MC68010 MULS/MULU print two unlabeled rows (42 with `+`, 40 without).
//!    MULS takes the first, MULU the second; both are maxima.
//! 3. MC68010 Table 9-16 prints `MOVEM.L (An),<list>` as 24+8n; its neighbour
//!    `(An)+` and the MC68000's table say 12+8n. DEVIATE: 12+8n.
//! 4. Exception processing is priced for `trap` only.
//! 5. `andi`/`ori`/`eori` to CCR and `move` to or from CCR are priced as their
//!    SR rows; the manuals print the same clocks.

use std::sync::LazyLock;

use super::table::{Cell, Table};
use super::{Mode, Operand, Size, mode, places, size_of};

/// The part of the manual a table is: the same on both processors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Ea,
    MoveWord,
    MoveLong,
    Standard,
    Immediate,
    Single,
    Clear,
    Shift,
    Bit,
    Conditional,
    Jump,
    Multi,
    Misc,
    Movep,
    Exception,
}

pub(super) struct Model {
    tables: Vec<(Role, Table)>,
    /// The MC68010's: the two unlabeled MULS/MULU rows and the `MOVEM.L (An)`
    /// misprint, and no register-source penalty (the MC68000's footnote
    /// `**` of Table 8-4).
    mc68010: bool,
}

fn load(
    mc68010: bool,
    files: &[(Role, &str, bool)],
) -> Model {
    Model { tables: files.iter().map(|(role, text, two)| (*role, Table::parse(text, *two))).collect(), mc68010 }
}

pub(super) static M68000: LazyLock<Model> = LazyLock::new(|| {
    load(
        false,
        &[
            (Role::Ea, include_str!("m68000/t8-1.csv"), true),
            (Role::MoveWord, include_str!("m68000/t8-2.csv"), false),
            (Role::MoveLong, include_str!("m68000/t8-3.csv"), false),
            (Role::Standard, include_str!("m68000/t8-4.csv"), true),
            (Role::Immediate, include_str!("m68000/t8-5.csv"), true),
            (Role::Single, include_str!("m68000/t8-6.csv"), true),
            (Role::Shift, include_str!("m68000/t8-7.csv"), true),
            (Role::Bit, include_str!("m68000/t8-8.csv"), true),
            (Role::Conditional, include_str!("m68000/t8-9.csv"), true),
            (Role::Jump, include_str!("m68000/t8-10.csv"), true),
            (Role::Multi, include_str!("m68000/t8-11.csv"), true),
            (Role::Misc, include_str!("m68000/t8-12.csv"), true),
            (Role::Movep, include_str!("m68000/t8-13.csv"), true),
            (Role::Exception, include_str!("m68000/t8-14.csv"), false),
        ],
    )
});

pub(super) static M68010: LazyLock<Model> = LazyLock::new(|| {
    load(
        true,
        &[
            (Role::Ea, include_str!("m68010/t9-1.csv"), true),
            (Role::MoveWord, include_str!("m68010/t9-2.csv"), false),
            (Role::MoveLong, include_str!("m68010/t9-4.csv"), false),
            (Role::Standard, include_str!("m68010/t9-6.csv"), true),
            (Role::Immediate, include_str!("m68010/t9-8.csv"), true),
            (Role::Single, include_str!("m68010/t9-9.csv"), true),
            (Role::Clear, include_str!("m68010/t9-10.csv"), true),
            (Role::Shift, include_str!("m68010/t9-12.csv"), true),
            (Role::Bit, include_str!("m68010/t9-14.csv"), true),
            (Role::Conditional, include_str!("m68010/t9-15.csv"), true),
            (Role::Jump, include_str!("m68010/t9-16.csv"), true),
            (Role::Multi, include_str!("m68010/t9-17.csv"), true),
            (Role::Misc, include_str!("m68010/t9-18.csv"), true),
            (Role::Exception, include_str!("m68010/t9-19.csv"), false),
        ],
    )
});

/// The labels the tables print for `mode`: the pages disagree on the PC modes'
/// displacement width and on the space of `(xxx) W`.
fn labels(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::AbsW => &["(xxx).W", "(xxx) W"],
        Mode::PcDisp => &["(d16,PC)", "(d8,PC)", "(d,PC)", "(d16 PC)", "(d8 PC)"],
        Mode::PcIndex => &["(d8,PC,Xn)", "(d16,PC,Xn)", "(d,PC,Xn)"],
        Mode::Dn => &["Dn"],
        Mode::An => &["An"],
        Mode::Ind => &["(An)"],
        Mode::Post => &["(An)+"],
        Mode::Pre => &["-(An)"],
        Mode::Disp => &["(d16,An)"],
        Mode::Index => &["(d8,An,Xn)"],
        Mode::AbsL => &["(xxx).L"],
        Mode::Imm => &["#<data>"],
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

impl Model {
    fn table(
        &self,
        role: Role,
    ) -> &Table {
        &self.tables.iter().find(|(one, _)| *one == role).expect("a loaded table").1
    }

    fn has(
        &self,
        role: Role,
    ) -> bool {
        self.tables.iter().any(|(one, _)| *one == role)
    }

    fn cell(
        &self,
        role: Role,
        row: &str,
        size: Size,
        column: &str,
    ) -> Option<Cell> {
        self.table(role).get(row, sized(size), column)
    }

    fn base(
        &self,
        role: Role,
        row: &str,
        size: Size,
        column: &str,
    ) -> Option<u32> {
        self.cell(role, row, size, column).map(|cell| cell.base)
    }

    /// The cell of `row` at the column that prints `mode`.
    fn at(
        &self,
        role: Role,
        row: &str,
        size_ok: impl Fn(&str) -> bool,
        column: Mode,
    ) -> Option<Cell> {
        labels(column).iter().find_map(|name| self.table(role).get(row, &size_ok, name))
    }

    fn misc(
        &self,
        row: &str,
        column: &str,
    ) -> Option<Cell> {
        self.table(Role::Misc).get(row, any, column)
    }

    fn register(
        &self,
        row: &str,
    ) -> Option<u32> {
        self.misc(row, "Register").map(|cell| cell.base)
    }

    /// The effective address time, with the operand fetched or not (the MC68010
    /// prints both).
    fn ea(
        &self,
        mode: Mode,
        size: Size,
        fetch: bool,
    ) -> Option<u32> {
        let width = if size == Size::L { "Long" } else { "Byte, Word" };
        let how = if fetch { "Fetch" } else { "No Fetch" };
        let table = self.table(Role::Ea);
        labels(mode)
            .iter()
            .find_map(
                |name| table
                    .get(name, any, &format!("{width} / {how}"))
                    .or_else(|| table.get(name, any, width))
                    .map(|cell| cell.base),
            )
    }

    /// A cell with its effective address time added where the table says so.
    fn with_ea(
        &self,
        cell: Cell,
        operand: Option<Mode>,
        size: Size,
        fetch: bool,
    ) -> Option<u32> {
        match (cell.plus_ea, operand) {
            (true, Some(mode)) => Some(cell.base + self.ea(mode, size, fetch)?),
            (true, None) => None,
            (false, _) => Some(cell.base),
        }
    }

    /// Row `name` of a table printed by register or memory column, the
    /// effective address time added.
    fn register_or_memory(
        &self,
        role: Role,
        name: &str,
        size: Size,
        operand: Mode,
        fetch: bool,
    ) -> Option<u32> {
        match operand {
            Mode::Dn => self.base(role, name, size, "Register"),
            memory => self.with_ea(self.cell(role, name, size, "Memory")?, Some(memory), size, fetch),
        }
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

pub(super) fn clocks(
    model: &Model,
    base: &str,
    suffix: Option<&str>,
    operands: &[Operand],
    bytes: u32,
    taken: bool,
) -> Option<u32> {
    let size = size_of(suffix);
    let long = size == Size::L;
    let m = model;
    let misc = |row: &str, column: &str| m.misc(row, column);
    let register = |row: &str| m.register(row);
    match (base, operands) {
        ("move", [source, Operand::Status]) => {
            let cell = misc("MOVE to SR", if mode(source)? == Mode::Dn { "Register" } else { "Memory" })?;
            m.with_ea(cell, mode(source), Size::W, true)
        }
        ("move", [Operand::Status, destination]) => match mode(destination)? {
            Mode::Dn => register("MOVE from SR"),
            memory => m.with_ea(misc("MOVE from SR", "Memory")?, Some(memory), Size::W, false),
        },
        ("move", [_, Operand::Usp]) | ("move", [Operand::Usp, _]) => register("MOVE to USP"),
        ("movec", [Operand::Control, _]) => misc("MOVEC", "Source→Register").map(|cell| cell.base),
        ("movec", [_, Operand::Control]) => misc("MOVEC", "Register→Destination").map(|cell| cell.base),
        ("move" | "movea", [source, destination]) => {
            let (role, source, destination) =
                (if long { Role::MoveLong } else { Role::MoveWord }, mode(source)?, mode(destination)?);
            labels(source)
                .iter()
                .find_map(
                    |row| labels(destination).iter().find_map(|column| {
                        m.table(role).get(row, any, &format!("Destination / {column}")).map(|cell| cell.base)
                    }),
                )
        }
        ("moveq", [_, _]) => m.base(Role::Immediate, "MOVEQ", Size::L, "op #, Dn"),
        ("add" | "sub" | "and" | "or" | "eor" | "cmp" | "adda" | "suba" | "cmpa", [source, destination]) => {
            let (source, destination) = (mode(source)?, mode(destination)?);
            let name = match base {
                "adda" => "ADD".to_owned(),
                "suba" => "SUB".to_owned(),
                "cmpa" => "CMP".to_owned(),
                _ => base.to_ascii_uppercase(),
            };
            let standard = |column: &str| m.cell(Role::Standard, &name, size, column);
            // Table 8-4 `**`: a long ADD, AND, OR or SUB with a register or
            // immediate source is two clocks more
            let starred = u32::from(
                !m.mc68010
                    && long
                    && matches!(name.as_str(), "ADD" | "AND" | "OR" | "SUB")
                    && matches!(source, Mode::Dn | Mode::An | Mode::Imm)
                    && matches!(destination, Mode::Dn | Mode::An),
            ) * 2;
            match (base, destination) {
                (_, Mode::An) => {
                    m.with_ea(standard("op<ea>, An")?, Some(source), size, true).map(|clocks| clocks + starred)
                }
                ("eor", Mode::Dn) => standard("op<ea>, Dn").map(|cell| cell.base),
                ("eor", memory) => m.with_ea(standard("op Dn, <M>")?, Some(memory), size, true),
                (_, Mode::Dn) if source == Mode::Imm && base != "cmp" => {
                    m.base(Role::Immediate, immediate_name(base)?, size, "op #, Dn")
                }
                (_, Mode::Dn) => {
                    m.with_ea(standard("op<ea>, Dn")?, Some(source), size, true).map(|clocks| clocks + starred)
                }
                (_, memory) if source == Mode::Dn => m.with_ea(standard("op Dn, <M>")?, Some(memory), size, true),
                (_, memory) if source == Mode::Imm => m.with_ea(
                    m.cell(Role::Immediate, immediate_name(base)?, size, "op #, M")?,
                    Some(memory),
                    size,
                    true,
                ),
                _ => None,
            }
        }
        ("andi" | "ori" | "eori", [_, Operand::Status]) => register("ANDI to SR"),
        ("addi" | "subi" | "andi" | "ori" | "eori" | "cmpi", [_, destination]) => match mode(destination)? {
            Mode::Dn => m.base(Role::Immediate, immediate_name(base)?, size, "op #, Dn"),
            memory => {
                m.with_ea(m.cell(Role::Immediate, immediate_name(base)?, size, "op #, M")?, Some(memory), size, true)
            }
        },
        ("addq" | "subq", [_, destination]) => {
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn => m.base(Role::Immediate, &name, size, "op #, Dn"),
                Mode::An => m.base(Role::Immediate, &name, size, "op #, An"),
                memory => m.with_ea(m.cell(Role::Immediate, &name, size, "op #, M")?, Some(memory), size, true),
            }
        }
        // the MC68010 prints a clear table of its own, which reads no operand
        ("clr", [destination]) if m.has(Role::Clear) => {
            m.at(Role::Clear, "CLR", sized(size), mode(destination)?).map(|cell| cell.base)
        }
        ("clr" | "neg" | "negx" | "not" | "nbcd", [destination]) => {
            m.register_or_memory(Role::Single, &base.to_ascii_uppercase(), size, mode(destination)?, true)
        }
        ("tst", [destination]) => match mode(destination)? {
            Mode::An => m.base(Role::Single, "TST", size, "Register"),
            operand => m.register_or_memory(Role::Single, "TST", size, operand, true),
        },
        ("tas", [destination]) => m.register_or_memory(Role::Single, "TAS", Size::B, mode(destination)?, false),
        (set, [destination]) if super::set_on_condition(set) => {
            m.register_or_memory(Role::Single, "Scc", Size::B, mode(destination)?, false)
        }
        ("ext", _) => misc("EXT", "Register").map(|cell| cell.base),
        ("swap", _) => register("SWAP"),
        ("nop", _) => register("NOP"),
        ("exg", _) => register("EXG"),
        (shift, [_, _]) if super::is_shift(shift) => {
            let cell = m.table(Role::Shift).get(&shift.to_ascii_uppercase(), sized(size), "Register")?;
            Some(cell.base + cell.per * places(operands).unwrap_or(0))
        }
        (shift, [memory]) if super::is_shift(shift) => m.with_ea(
            m.table(Role::Shift).get(&shift.to_ascii_uppercase(), sized(Size::W), "Memory")?,
            mode(memory),
            Size::W,
            true,
        ),
        ("muls" | "mulu", [source, _]) if m.mc68010 => {
            let nth = usize::from(base == "mulu");
            let text = m.table(Role::Standard).text_nth("MULS", nth, any, "op<ea>, Dn")?;
            m.with_ea(super::table::parse_cell(text)?, mode(source), Size::W, true).map(|clocks| {
                // the second row prints no `+`: MULU takes the same effective
                // address time
                if nth == 1 {
                    clocks + m.ea(mode(source).unwrap_or(Mode::Dn), Size::W, true).unwrap_or(0)
                } else {
                    clocks
                }
            })
        }
        ("muls" | "mulu" | "divs" | "divu", [source, _]) => m.with_ea(
            m.cell(Role::Standard, &base.to_ascii_uppercase(), Size::W, "op<ea>, Dn")?,
            mode(source),
            Size::W,
            true,
        ),
        ("chk", [source, _]) => {
            let cell = misc("CHK", "Register").or_else(|| misc("CHK (No Trap)", "Register"))?;
            m.with_ea(cell, mode(source), Size::W, true)
        }
        ("lea" | "pea", [source]) | ("lea", [source, _]) | ("pea", [source, ..]) => {
            m.at(Role::Jump, &base.to_ascii_uppercase(), any, mode(source)?).map(|cell| cell.base)
        }
        ("jmp" | "jsr", [target]) => {
            let name = base.to_ascii_uppercase();
            let target = match target {
                // a call to an address is absolute short in four bytes, long in
                // six
                Operand::Target => {
                    if bytes == 4 {
                        Mode::AbsW
                    } else {
                        Mode::AbsL
                    }
                }
                other => mode(other)?,
            };
            m.at(Role::Jump, &name, any, target).map(|cell| cell.base)
        }
        ("bra", [_]) => m.base(Role::Conditional, "BRA", Size::B, "Branch Taken"),
        ("bsr", [_]) => m.base(Role::Conditional, "BSR", Size::B, "Branch Taken"),
        (condition, [_]) if super::condition_branch(condition) => {
            let word = |text: &str| text.contains(if bytes <= 2 { "byte" } else { "word" });
            let column = if taken { "Branch Taken" } else { "Branch Not Taken" };
            m.table(Role::Conditional).get("Bcc", word, column).map(|cell| cell.base)
        }
        (dbcc, [_, _]) if dbcc.starts_with("db") => {
            let (row, column) = match (dbcc == "dbt", taken) {
                (true, _) => ("cctrue", "Branch Not Taken"),
                (false, true) => ("ccfalse", "Branch Taken"),
                (false, false) => ("ccfalse", "Branch Not Taken"),
            };
            // the MC68000 prints two `cc false` rows, by whether the count
            // expired
            let row = |text: &str| {
                text.contains(row)
                    && (!text.contains("expired") || text.contains(if taken { "notexpired" } else { "counterexpired" }))
            };
            m.table(Role::Conditional).get("DBcc", row, column).map(|cell| cell.base)
        }
        ("rts", []) => register("RTS"),
        ("rtr", []) => register("RTR"),
        ("rtd", [_]) => register("RTD"),
        ("rte", []) => m
            .table(Role::Misc)
            .get("RTE", |text| text.contains("short"), "Register")
            .or_else(|| misc("RTE", "Register"))
            .map(|cell| cell.base),
        ("link", [_, _]) => register("LINK"),
        ("unlk", [_]) => register("UNLK"),
        ("trapv", []) => register("TRAPV"),
        ("stop", [_]) => register("STOP"),
        ("reset", []) => register("RESET"),
        ("trap", [_]) => {
            let found = m.table(Role::Exception).get("TRAP Instruction", any, "Periods")?;
            Some(found.base)
        }
        ("abcd" | "sbcd" | "addx" | "subx" | "cmpm", [first, _]) => {
            let name = base.to_ascii_uppercase();
            let registers = mode(first)? == Mode::Dn;
            let columns: [&str; 2] =
                if registers { ["op Dn, Dn", "Nonlooped / op Dn, Dn"] } else { ["op M, M", "Nonlooped / op M, M"] };
            let size = if matches!(base, "abcd" | "sbcd") { Size::B } else { size };
            columns.iter().find_map(|column| m.base(Role::Multi, &name, size, column))
        }
        ("movep", [first, _]) if m.has(Role::Movep) => {
            let column = if mode(first)? == Mode::Dn { "Register → Memory" } else { "Memory → Register" };
            m.base(Role::Movep, "MOVEP", size, column)
        }
        // the MC68010 prints it in its miscellaneous table
        ("movep", [first, _]) => {
            let column = if mode(first)? == Mode::Dn { "Register→Destination" } else { "Source→Register" };
            m.table(Role::Misc).get("MOVEP", sized(size), column).map(|cell| cell.base)
        }
        ("movem", [Operand::List(registers), memory]) => m.movem("MOVEM R→M", memory, long, *registers),
        // DEVIATE (limit 3): the page prints 24+8n for `(An)` long; its
        // neighbour `(An)+` is 12+8n
        ("movem", [memory, Operand::List(registers)]) if m.mc68010 && long && mode(memory)? == Mode::Ind => {
            m.movem("MOVEM M→R", &Operand::Mode(Mode::Post, 0), long, *registers)
        }
        ("movem", [memory, Operand::List(registers)]) => m.movem("MOVEM M→R", memory, long, *registers),
        ("btst" | "bchg" | "bclr" | "bset", [source, destination]) => {
            let kind = if mode(source)? == Mode::Dn { "Dynamic" } else { "Static" };
            let name = base.to_ascii_uppercase();
            match mode(destination)? {
                Mode::Dn => m.base(Role::Bit, &name, Size::L, &format!("{kind} / Register")),
                memory => m.with_ea(
                    m.cell(Role::Bit, &name, Size::B, &format!("{kind} / Memory"))?,
                    Some(memory),
                    Size::B,
                    true,
                ),
            }
        }
        _ => None,
    }
}

impl Model {
    /// The JMP/JSR/LEA/PEA/MOVEM table's MOVEM rows: `base + per * n`.
    fn movem(
        &self,
        row: &str,
        memory: &Operand,
        long: bool,
        registers: u32,
    ) -> Option<u32> {
        let cell = self.at(Role::Jump, row, sized(if long { Size::L } else { Size::W }), mode(memory)?)?;
        Some(cell.base + cell.per * registers)
    }
}
