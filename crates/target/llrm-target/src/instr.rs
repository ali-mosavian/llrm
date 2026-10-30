//! The reader of a target's instruction description (`x86.instr`, ...): one
//! schema for every target, read by the generators and the target crates.

/// What a family's description file says that the reader must know: the
/// registers its fixed and implicit columns may name, the widths a form may
/// have, the flags and the conditions `{cc}` expands to, and whether a row ends
/// in a column for an assembler's name (x86's `iced`). Each family states its
/// own in `dialect.toml` beside its instruction file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dialect {
    /// The instruction file's name, as an error spells it (`x86.instr`).
    pub file: String,
    pub registers: Vec<String>,
    pub widths: Vec<u32>,
    /// Flag names and their bits.
    pub flags: Vec<(String, u32)>,
    /// Each `{cc}` condition in order, and the flags it tests.
    pub conditions: Vec<(String, u32)>,
    /// Rows have a last column naming the assembler's code.
    pub assembler_column: bool,
    /// The flags an arithmetic instruction writes: what a rule's `any` reads.
    pub arithmetic: Vec<String>,
}

impl Dialect {
    /// The dialect `text` (`dialect.toml`) states.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error| format!("dialect.toml: {error}"))?;
        let strings = |key: &str| -> Result<Vec<String>, String> {
            table
                .get(key)
                .and_then(toml::Value::as_array)
                .ok_or_else(|| format!("dialect.toml: no list `{key}`"))?
                .iter()
                .map(|one| {
                    one.as_str().map(str::to_owned).ok_or_else(|| format!("dialect.toml: `{key}` holds a non-string"))
                })
                .collect()
        };
        let flags: Vec<(String, u32)> = table
            .get("flags")
            .and_then(toml::Value::as_table)
            .ok_or("dialect.toml: no table `flags`")?
            .iter()
            .map(|(name, bit)| {
                let bit = bit.as_integer().and_then(|bit| u32::try_from(bit).ok());
                bit.map(|bit| (name.clone(), bit)).ok_or_else(|| format!("dialect.toml: flag `{name}` is no bit"))
            })
            .collect::<Result<_, _>>()?;
        let conditions = table
            .get("conditions")
            .and_then(toml::Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|one| {
                        let name = one
                            .get("name")
                            .and_then(toml::Value::as_str)
                            .ok_or("dialect.toml: a condition has no name")?;
                        let mut mask = 0;
                        for flag in one.get("flags").and_then(toml::Value::as_array).into_iter().flatten() {
                            let flag = flag.as_str().unwrap_or("");
                            mask |=
                                flags.iter().find(|(one, _)| one == flag).map(|(_, bit)| *bit).ok_or_else(|| {
                                    format!("dialect.toml: condition `{name}` tests no flag `{flag}`")
                                })?;
                        }
                        Ok((name.to_owned(), mask))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?
            .unwrap_or_default();
        let widths = table
            .get("widths")
            .and_then(toml::Value::as_array)
            .ok_or("dialect.toml: no list `widths`")?
            .iter()
            .map(|one| {
                one.as_integer().and_then(|bits| u32::try_from(bits).ok()).ok_or("dialect.toml: a width is no number")
            })
            .collect::<Result<_, _>>()?;
        let arithmetic = strings("arithmetic").unwrap_or_default();
        if let Some(missing) = arithmetic.iter().find(|name| !flags.iter().any(|(one, _)| one == *name)) {
            return Err(format!("dialect.toml: `arithmetic` names no flag `{missing}`"));
        }
        Ok(Self {
            file: table.get("file").and_then(toml::Value::as_str).ok_or("dialect.toml: no `file`")?.to_owned(),
            registers: strings("registers")?,
            widths,
            flags,
            conditions,
            assembler_column: table.get("assembler_column").and_then(toml::Value::as_bool).unwrap_or(false),
            arithmetic,
        })
    }

    pub fn flag(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.flags.iter().find(|(one, _)| one == name).map(|(_, bit)| *bit)
    }

    /// The bits of the flags an arithmetic instruction writes.
    pub fn arithmetic_mask(&self) -> u32 {
        self.arithmetic.iter().filter_map(|name| self.flag(name)).fold(0, |all, bit| all | bit)
    }
}

/// One LIR operand of a form: the kinds it may be, and the dest whose
/// register a tied source is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operand {
    pub kinds: String,
    pub tied: Option<usize>,
}

impl Operand {
    pub fn allows(
        &self,
        kind: char,
    ) -> bool {
        self.kinds.contains(kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Dest,
    Source,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Form {
    pub name: String,
    pub operation: String,
    pub dests: Vec<Operand>,
    pub sources: Vec<Operand>,
    /// Bits.
    pub widths: Vec<u32>,
    pub cost: String,
    /// Operands pinned to a register root when they are a register.
    pub fixed: Vec<(Side, usize, String)>,
    /// Register roots it reads and writes beyond its operands (`push` uses
    /// `sp`). `rep` in `writes` marks a repeated string operation, whose
    /// count register is written and whose other writes happen only if it runs;
    /// `count` a shift, whose flags a constant count of none leaves alone;
    /// `idiom` in `reads` an operation that reads nothing when both its
    /// operands are one register (`xor ax, ax`); `self` one that is no
    /// instruction then (`mov ax, ax`); `narrow` one whose address
    /// registers are the destination's width; `same` one whose registers must
    /// be one size; `ax*` is `ax` at the width of the row. `-root` drops an
    /// operand register the machine does not touch, `@8` only where the
    /// narrowest source is a byte: LIR carries DX through a byte `div`, which
    /// leaves it alone.
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    /// The flags it reads, and those it writes or leaves undefined, as bits of
    /// `RflagsBits`.
    pub flags_read: u32,
    pub flags_written: u32,
    /// iced's Code name, `{w}` still to substitute.
    pub iced: Option<String>,
    pub line: usize,
}

impl Form {
    /// iced's Code name at `bits`, or at the form's first width.
    pub fn code_name(
        &self,
        bits: Option<u32>,
    ) -> Option<String> {
        let bits = bits.or(self.widths.first().copied()).map(|bits| bits.to_string()).unwrap_or_default();
        self.iced.as_ref().map(|template| template.replace("{w}", &bits))
    }

    pub fn operand(
        &self,
        side: Side,
        index: usize,
    ) -> Option<&Operand> {
        match side {
            Side::Dest => self.dests.get(index),
            Side::Source => self.sources.get(index),
        }
    }
}

/// What an operand may be: a register, cell, immediate, address or float
/// register, then a shift, condition or extension field.
const KINDS: &str = "rmiashcex";

fn operands(
    d: &Dialect,
    text: &str,
    line: usize,
) -> Result<Vec<Operand>, String> {
    let file = &d.file;
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|one| {
            if let Some(tied) = one.strip_prefix('^') {
                let dest = tied.parse().map_err(|_| format!("{file}:{line}: `{one}` ties to no dest number"))?;
                return Ok(Operand { kinds: "r".into(), tied: Some(dest) });
            }
            if one.is_empty() || !one.chars().all(|kind| KINDS.contains(kind)) {
                return Err(format!("{file}:{line}: operand `{one}` is not made of the kinds r m i a s h c e x"));
            }
            Ok(Operand { kinds: one.into(), tied: None })
        })
        .collect()
}

fn fixed(
    d: &Dialect,
    text: &str,
    line: usize,
) -> Result<Vec<(Side, usize, String)>, String> {
    let file = &d.file;
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|one| {
            let bad = || format!("{file}:{line}: fixed `{one}` is not dN=REG or sN=REG");
            let (at, register) = one.split_once('=').ok_or_else(bad)?;
            let side = match at.as_bytes().first() {
                Some(b'd') => Side::Dest,
                Some(b's') => Side::Source,
                _ => return Err(bad()),
            };
            let index = at[1..].parse().map_err(|_| bad())?;
            if !d.registers.iter().any(|one| one == register) {
                return Err(format!("{file}:{line}: `{register}` is no register root"));
            }
            Ok((side, index, register.to_owned()))
        })
        .collect()
}

fn implicit(
    d: &Dialect,
    text: &str,
    line: usize,
) -> Result<Vec<String>, String> {
    let file = &d.file;
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|entry| {
            let root = entry.trim_start_matches('-').split('@').next().unwrap_or("").trim_end_matches('*');
            if ["rep", "idiom", "count", "self", "narrow", "same", "cond"].contains(&entry) {
                return Ok(entry.to_owned());
            }
            let width = entry.split_once('@').map(|(_, width)| width);
            if !d.registers.iter().any(|one| one == root) || width.is_some_and(|width| width.parse::<u32>().is_err()) {
                return Err(format!("{file}:{line}: `{entry}` is not [-]root[@width]"));
            }
            Ok(entry.to_owned())
        })
        .collect()
}

/// `READ/WRITTEN`, each `-` or flag names separated by commas; `cc` as the read
/// side is the condition's.
fn flag_masks(
    d: &Dialect,
    text: &str,
    line: usize,
) -> Result<(u32, u32, bool), String> {
    let file = &d.file;
    let bad = || format!("{file}:{line}: flags `{text}` are not READ/WRITTEN, each `-` or flag names");
    let (read, written) = text.split_once('/').ok_or_else(bad)?;
    let mask = |list: &str| -> Result<u32, String> {
        if list == "-" {
            return Ok(0);
        }
        list.split(',').try_fold(0, |all, name| d.flag(name).map(|bit| all | bit).ok_or_else(bad))
    };
    if read == "cc" {
        return Ok((0, mask(written)?, true));
    }
    Ok((mask(read)?, mask(written)?, false))
}

/// Every form `text` describes, `{cc}` expanded.
pub fn parse(
    text: &str,
    dialect: &Dialect,
) -> Result<Vec<Form>, String> {
    parse_rows(text, dialect, |_| true)
}

/// The forms that pin an operand to a register, for a reader that wants only
/// those: the description is read at every compile, and a row costs about 5,000
/// instructions to read.
pub fn pinned(
    text: &str,
    dialect: &Dialect,
) -> Result<Vec<Form>, String> {
    parse_rows(text, dialect, |pinned| pinned != "-")
}

fn parse_rows(
    text: &str,
    d: &Dialect,
    keep: impl Fn(&str) -> bool,
) -> Result<Vec<Form>, String> {
    let file = &d.file;
    let mut forms = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = raw.split('#').next().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let mut columns: Vec<&str> = content.split_whitespace().collect();
        let wanted = if d.assembler_column { 10 } else { 9 };
        if columns.len() != wanted {
            return Err(format!("{file}:{line}: {} columns, not {wanted}", columns.len()));
        }
        if !d.assembler_column {
            columns.push("-");
        }
        let [name, operation, shape, widths, cost, pinned, reads, writes, flags, iced] = columns[..] else {
            unreachable!("ten columns")
        };
        if !keep(pinned) {
            continue;
        }
        if llrm_lir::Operation::named(operation).is_none() {
            return Err(format!("{file}:{line}: no operation `{operation}`"));
        }
        let (dests, sources) =
            shape.split_once('/').ok_or_else(|| format!("{file}:{line}: shape `{shape}` has no `/`"))?;
        let (dests, sources) = (operands(d, dests, line)?, operands(d, sources, line)?);
        if let Some(bad) = sources.iter().filter_map(|one| one.tied).find(|&dest| dest >= dests.len()) {
            return Err(format!("{file}:{line}: a source tied to dest {bad}, which it has not"));
        }
        let widths = if widths == "-" {
            Vec::new()
        } else {
            widths
                .split(',')
                .map(|bits| {
                    bits.parse()
                        .ok()
                        .filter(|bits| d.widths.contains(bits))
                        .ok_or_else(|| format!("{file}:{line}: width `{bits}`"))
                })
                .collect::<Result<_, _>>()?
        };
        let (flags_read, flags_written, by_condition) = flag_masks(d, flags, line)?;
        let pinned = fixed(d, pinned, line)?;
        let iced = (iced != "-").then(|| iced.to_owned());
        let form = Form {
            name: name.into(),
            operation: operation.into(),
            dests,
            sources,
            widths,
            cost: cost.into(),
            fixed: pinned,
            reads: implicit(d, reads, line)?,
            writes: implicit(d, writes, line)?,
            flags_read,
            flags_written,
            iced,
            line,
        };
        if name.contains("{cc}") {
            for (condition, tested) in &d.conditions {
                forms.push(Form {
                    name: name.replace("{cc}", condition),
                    iced: form.iced.as_ref().map(|one| one.replace("{cc}", condition)),
                    flags_read: if by_condition { *tested } else { form.flags_read },
                    ..form.clone()
                });
            }
        } else {
            forms.push(form);
        }
    }
    Ok(forms)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dialect with an assembler column, as x86's.
    fn dialect() -> Dialect {
        Dialect::parse(
            "file = \"x86.instr\"\nregisters = [\"ax\", \"cx\"]\nwidths = [8, 16, 32]\nassembler_column = true\n\
             [flags]\nCF = 16\nZF = 4\n",
        )
        .expect("a dialect")
    }

    /// A shift, condition or extension operand was refused as "not made of the
    /// kinds r m i a s": arm64's forms carry them.
    #[test]
    fn an_operand_may_be_a_shift_a_condition_or_an_extension() {
        for kind in ["h", "c", "e"] {
            let forms =
                parse(&format!("add3 binary r/r,r,{kind} 32 alu_rr - - - -/- Add_{{w}}"), &dialect()).expect("parses");
            assert!(forms[0].sources[2].allows(kind.chars().next().unwrap()));
        }
        assert!(
            parse("add3 binary r/r,r,z 32 alu_rr - - - -/- Add_{w}", &dialect())
                .unwrap_err()
                .contains("kinds r m i a s h c e x")
        );
    }

    /// A family states its own registers, widths, flags and columns: arm64's
    /// 64-bit rows with no assembler column were refused ("width `64`",
    /// "9 columns, not 10") by the reader's x86 constants.
    #[test]
    fn a_dialect_decides_widths_registers_flags_and_columns() {
        let arm64 = Dialect::parse(
            "file = \"arm64.instr\"\nregisters = [\"x0\", \"sp\"]\nwidths = [32, 64]\n\
             conditions = [{ name = \"eq\", flags = [\"Z\"] }, { name = \"ne\", flags = [\"Z\"] }]\n\
             [flags]\nN = 8\nZ = 4\nC = 2\nV = 1\n",
        )
        .expect("a dialect");
        let forms = parse("b.{cc} branch -/- 64 alu_rr - - - cc/-", &arm64).expect("parses");
        let names: Vec<_> = forms.iter().map(|form| (form.name.as_str(), form.flags_read)).collect();
        assert_eq!(names, [("b.eq", 4), ("b.ne", 4)]);
        assert_eq!(
            parse("x branch -/- 64 alu_rr - - - -/-", &dialect()).unwrap_err(),
            "x86.instr:1: 9 columns, not 10"
        );
        assert_eq!(
            parse("x branch -/- 64 alu_rr x1=sp - - -/-", &arm64).unwrap_err(),
            "arm64.instr:1: fixed `x1=sp` is not dN=REG or sN=REG"
        );
    }
}
