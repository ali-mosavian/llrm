//! `x86.instr`'s reader. No dependencies, so a build script can include it
//! with `#[path]`.

/// Condition codes, in iced's spelling, that `{cc}` expands to.
pub const CONDITIONS: [&str; 16] = ["o", "no", "b", "ae", "e", "ne", "be", "a", "s", "ns", "p", "np", "l", "ge", "le", "g"];

/// One LIR operand of a form: the kinds it may be, and the dest whose
/// register a tied source is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operand {
    pub kinds: String,
    pub tied: Option<usize>,
}

impl Operand {
    pub fn allows(&self, kind: char) -> bool {
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
    /// iced's Code name, `{w}` still to substitute.
    pub iced: Option<String>,
    pub line: usize,
}

impl Form {
    /// iced's Code name at `bits`, or at the form's first width.
    pub fn code_name(&self, bits: Option<u32>) -> Option<String> {
        let bits = bits.or(self.widths.first().copied()).map(|bits| bits.to_string()).unwrap_or_default();
        self.iced.as_ref().map(|template| template.replace("{w}", &bits))
    }

    pub fn operand(&self, side: Side, index: usize) -> Option<&Operand> {
        match side {
            Side::Dest => self.dests.get(index),
            Side::Source => self.sources.get(index),
        }
    }
}

const OPERATIONS: [&str; 28] = [
    "move", "xchg", "addr", "binary", "mul", "div", "cmp", "unary", "funnel", "extend", "push", "pop", "leave", "fill", "jump", "branch", "escape", "call",
    "ret", "nothing", "restore", "data", "fload", "fstore", "farith", "farithp", "funary", "barrier",
];

const REGISTERS: [&str; 9] = ["ax", "bx", "cx", "dx", "si", "di", "bp", "sp", "es"];

fn operands(text: &str, line: usize) -> Result<Vec<Operand>, String> {
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

fn fixed(text: &str, line: usize) -> Result<Vec<(Side, usize, String)>, String> {
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

/// Every form `text` describes, `{cc}` expanded.
pub fn parse(text: &str) -> Result<Vec<Form>, String> {
    let mut forms = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = raw.split('#').next().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let columns: Vec<&str> = content.split_whitespace().collect();
        let [name, operation, shape, widths, cost, pinned, iced] = columns[..] else {
            return Err(format!("x86.instr:{line}: {} columns, not 7", columns.len()));
        };
        if !OPERATIONS.contains(&operation) {
            return Err(format!("x86.instr:{line}: no operation `{operation}`"));
        }
        let (dests, sources) = shape.split_once('/').ok_or_else(|| format!("x86.instr:{line}: shape `{shape}` has no `/`"))?;
        let (dests, sources) = (operands(dests, line)?, operands(sources, line)?);
        if let Some(bad) = sources.iter().filter_map(|one| one.tied).find(|&dest| dest >= dests.len()) {
            return Err(format!("x86.instr:{line}: a source tied to dest {bad}, which it has not"));
        }
        let widths = if widths == "-" {
            Vec::new()
        } else {
            widths.split(',').map(|bits| bits.parse().ok().filter(|bits| [8, 16, 32].contains(bits)).ok_or_else(|| format!("x86.instr:{line}: width `{bits}`"))).collect::<Result<_, _>>()?
        };
        let pinned = fixed(pinned, line)?;
        let iced = (iced != "-").then(|| iced.to_owned());
        let form = Form { name: name.into(), operation: operation.into(), dests, sources, widths, cost: cost.into(), fixed: pinned, iced, line };
        if name.contains("{cc}") {
            for condition in CONDITIONS {
                forms.push(Form { name: name.replace("{cc}", condition), iced: form.iced.as_ref().map(|one| one.replace("{cc}", condition)), ..form.clone() });
            }
        } else {
            forms.push(form);
        }
    }
    Ok(forms)
}
