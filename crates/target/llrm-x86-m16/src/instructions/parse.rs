//! `x86.instr`'s reader. Its only dependency is `llrm-lir`, which a build script
//! has too, so it can include this with `#[path]`.

/// Condition codes, in iced's spelling, that `{cc}` expands to.
pub const CONDITIONS: [&str; 16] =
    ["o", "no", "b", "ae", "e", "ne", "be", "a", "s", "ns", "p", "np", "l", "ge", "le", "g"];

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
    /// Register roots it reads and writes beyond its operands (`push` uses `sp`). `rep` in `writes` marks a repeated
    /// string operation, whose count register is written and whose other writes happen only if it runs; `count` a
    /// shift, whose flags a constant count of none leaves alone; `idiom` in `reads` an operation that reads nothing
    /// when both its operands are one register (`xor ax, ax`); `self` one that is no instruction then (`mov ax, ax`);
    /// `narrow` one whose address registers are the destination's width; `same` one whose registers must be one size;
    /// `ax*` is `ax` at the width of the row. `-root` drops an operand register the machine does not touch, `@8`
    /// only where the narrowest source is a byte: LIR carries DX through a byte `div`, which leaves it alone.
    pub reads: Vec<String>,
    pub writes: Vec<String>,
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

const REGISTERS: [&str; 14] = ["ax", "bx", "cx", "dx", "si", "di", "bp", "sp", "es", "ds", "fs", "gs", "ss", "ah"];

fn operands(
    text: &str,
    line: usize,
) -> Result<Vec<Operand>, String> {
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|one| {
            if let Some(tied) = one.strip_prefix('^') {
                let dest = tied.parse().map_err(|_| format!("x86.instr:{line}: `{one}` ties to no dest number"))?;
                return Ok(Operand { kinds: "r".into(), tied: Some(dest) });
            }
            if one.is_empty() || !one.chars().all(|kind| "rmias".contains(kind)) {
                return Err(format!("x86.instr:{line}: operand `{one}` is not made of the kinds r m i a s"));
            }
            Ok(Operand { kinds: one.into(), tied: None })
        })
        .collect()
}

fn fixed(
    text: &str,
    line: usize,
) -> Result<Vec<(Side, usize, String)>, String> {
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|one| {
            let bad = || format!("x86.instr:{line}: fixed `{one}` is not dN=REG or sN=REG");
            let (at, register) = one.split_once('=').ok_or_else(bad)?;
            let side = match at.as_bytes().first() {
                Some(b'd') => Side::Dest,
                Some(b's') => Side::Source,
                _ => return Err(bad()),
            };
            let index = at[1..].parse().map_err(|_| bad())?;
            if !REGISTERS.contains(&register) {
                return Err(format!("x86.instr:{line}: `{register}` is no register root"));
            }
            Ok((side, index, register.to_owned()))
        })
        .collect()
}

fn implicit(
    text: &str,
    line: usize,
) -> Result<Vec<String>, String> {
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|entry| {
            let root = entry.trim_start_matches('-').split('@').next().unwrap_or("").trim_end_matches('*');
            if ["rep", "idiom", "count", "self", "narrow", "same"].contains(&entry) {
                return Ok(entry.to_owned());
            }
            let width = entry.split_once('@').map(|(_, width)| width);
            if !REGISTERS.contains(&root) || width.is_some_and(|width| width.parse::<u32>().is_err()) {
                return Err(format!("x86.instr:{line}: `{entry}` is not [-]root[@width]"));
            }
            Ok(entry.to_owned())
        })
        .collect()
}

/// Every form `text` describes, `{cc}` expanded.
pub fn parse(text: &str) -> Result<Vec<Form>, String> {
    parse_rows(text, |_| true)
}

/// The forms that pin an operand to a register, for a reader that wants only those: the description is read at every
/// compile, and a row costs about 5,000 instructions to read.
pub fn pinned(text: &str) -> Result<Vec<Form>, String> {
    parse_rows(text, |pinned| pinned != "-")
}

fn parse_rows(
    text: &str,
    keep: impl Fn(&str) -> bool,
) -> Result<Vec<Form>, String> {
    let mut forms = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = raw.split('#').next().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let columns: Vec<&str> = content.split_whitespace().collect();
        let [name, operation, shape, widths, cost, pinned, reads, writes, iced] = columns[..] else {
            return Err(format!("x86.instr:{line}: {} columns, not 9", columns.len()));
        };
        if !keep(pinned) {
            continue;
        }
        if llrm_lir::Operation::named(operation).is_none() {
            return Err(format!("x86.instr:{line}: no operation `{operation}`"));
        }
        let (dests, sources) =
            shape.split_once('/').ok_or_else(|| format!("x86.instr:{line}: shape `{shape}` has no `/`"))?;
        let (dests, sources) = (operands(dests, line)?, operands(sources, line)?);
        if let Some(bad) = sources.iter().filter_map(|one| one.tied).find(|&dest| dest >= dests.len()) {
            return Err(format!("x86.instr:{line}: a source tied to dest {bad}, which it has not"));
        }
        let widths = if widths == "-" {
            Vec::new()
        } else {
            widths
                .split(',')
                .map(|bits| {
                    bits.parse()
                        .ok()
                        .filter(|bits| [8, 16, 32].contains(bits))
                        .ok_or_else(|| format!("x86.instr:{line}: width `{bits}`"))
                })
                .collect::<Result<_, _>>()?
        };
        let pinned = fixed(pinned, line)?;
        let iced = (iced != "-").then(|| iced.to_owned());
        let form = Form {
            name: name.into(),
            operation: operation.into(),
            dests,
            sources,
            widths,
            cost: cost.into(),
            fixed: pinned,
            reads: implicit(reads, line)?,
            writes: implicit(writes, line)?,
            iced,
            line,
        };
        if name.contains("{cc}") {
            for condition in CONDITIONS {
                forms.push(Form {
                    name: name.replace("{cc}", condition),
                    iced: form.iced.as_ref().map(|one| one.replace("{cc}", condition)),
                    ..form.clone()
                });
            }
        } else {
            forms.push(form);
        }
    }
    Ok(forms)
}
