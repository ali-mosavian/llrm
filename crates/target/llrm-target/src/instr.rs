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
    /// The letters an opcode file's placeholders take at each width:
    /// `letters.g = { 32 = "W", 64 = "X" }` (arm64's general register widths).
    /// A placeholder's first character names the axis of the widths column it
    /// reads (`{zn}` reads axis `z`, with letters of its own).
    pub letters: Vec<(String, Vec<(u32, String)>)>,
    /// The opcode file beside the instruction file, where the family has one.
    pub opcodes: Option<String>,
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
        let letters = table
            .get("letters")
            .and_then(toml::Value::as_table)
            .map(|axes| {
                axes.iter()
                    .map(|(axis, widths)| {
                        let axis = axis.clone();
                        let widths = widths
                            .as_table()
                            .ok_or_else(|| format!("dialect.toml: `letters.{axis}` is no table"))?
                            .iter()
                            .map(|(bits, letter)| {
                                let bits = bits
                                    .parse::<u32>()
                                    .map_err(
                                        |_| format!("dialect.toml: `letters.{axis}` width `{bits}` is no number"),
                                    )?;
                                let letter = letter
                                    .as_str()
                                    .ok_or_else(|| format!("dialect.toml: `letters.{axis}.{bits}` is no string"))?;
                                Ok((bits, letter.to_owned()))
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        Ok((axis, widths))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            letters,
            opcodes: table.get("opcodes").and_then(toml::Value::as_str).map(str::to_owned),
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

/// The Rust source of the `Row` that says what `form` reads, writes, is tied
/// and pinned to, and does to the flags: what a target's `rows()` returns.
pub fn row_source(form: &Form) -> String {
    let list = |names: &[String]| names.iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ");
    let kinds = form
        .dests
        .iter()
        .chain(&form.sources)
        .map(|one| format!("{:?}", if one.tied.is_some() { "r" } else { one.kinds.as_str() }))
        .collect::<Vec<_>>()
        .join(", ");
    let ties = form
        .sources
        .iter()
        .enumerate()
        .filter_map(|(at, one)| one.tied.map(|dest| format!("({at}, {dest})")))
        .collect::<Vec<_>>()
        .join(", ");
    let pins = form
        .fixed
        .iter()
        .map(|(side, at, root)| format!("({}, {at}, {root:?})", *side == Side::Dest))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Row {{ reads: &[{}], writes: &[{}], kinds: &[{kinds}], width: {}, ties: &[{ties}], pins: &[{pins}], flags_read: {:#x}, flags_written: {:#x} }}",
        list(&form.reads),
        list(&form.writes),
        form.widths.first().copied().unwrap_or(0),
        form.flags_read,
        form.flags_written
    )
}

/// `pub fn rows(name, dests, sources)`: the rows of every form, keyed by the
/// form's name and its operand counts, after `header`.
pub fn rows_source<'a>(
    forms: impl IntoIterator<Item = &'a Form>,
    header: &str,
) -> String {
    let mut by_key: Vec<((String, usize, usize), Vec<String>)> = Vec::new();
    for form in forms {
        let value = row_source(form);
        let key = (form.name.clone(), form.dests.len(), form.sources.len());
        match by_key.iter_mut().find(|(one, _)| *one == key) {
            Some((_, values)) => {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            None => by_key.push((key, vec![value])),
        }
    }
    let mut code = String::from(header);
    code.push_str("pub fn rows(name: &str, dests: usize, sources: usize) -> &'static [Row] {\n    match (name, dests, sources) {\n");
    for ((name, dests, sources), values) in &by_key {
        code.push_str(&format!("        ({name:?}, {dests}, {sources}) => &[{}],\n", values.join(", ")));
    }
    code.push_str("        _ => &[],\n    }\n}\n");
    code
}

/// `pub fn llvm_names(name)`: the LLVM opcodes each form name stands for (one,
/// or the alternatives `[A|B]` lists), for a scheduling model keyed by the
/// pin's opcode.
pub fn llvm_names_source(opcodes: &[Opcode]) -> String {
    let mut by_name: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for one in opcodes {
        let names = by_name.entry(one.name.as_str()).or_default();
        for llvm in &one.llvm {
            if !names.contains(&llvm.as_str()) {
                names.push(llvm);
            }
        }
    }
    let mut code = String::from("pub fn llvm_names(name: &str) -> &'static [&'static str] {\n    match name {\n");
    for (name, llvm) in &by_name {
        let list: Vec<String> = llvm.iter().map(|one| format!("{one:?}")).collect();
        code.push_str(&format!("        {name:?} => &[{}],\n", list.join(", ")));
    }
    code.push_str("        _ => &[],\n    }\n}\n");
    let mut every: Vec<&str> = by_name.values().flatten().copied().collect();
    every.sort_unstable();
    every.dedup();
    code.push_str(
        "\n/// Every LLVM opcode a form name stands for.\n#[allow(dead_code)]\npub const LLVM_OPCODES: &[&str] = &[\n",
    );
    for name in every {
        code.push_str(&format!("    {name:?},\n"));
    }
    code.push_str("];\n");
    code
}

/// A form of an opcode file: the name the selector uses, the LLVM opcodes it
/// stands for (one, or the alternatives `[A|B]` lists), and the form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opcode {
    pub name: String,
    pub llvm: Vec<String>,
    pub form: Form,
}

/// The forms of an opcode file (`opcodes.instr`): the columns of `parse` with
/// the assembler column holding LLVM's opcode, and the name and that column
/// templates. `{g}` and `{f}` are the dialect's letters at each width of the
/// widths column (`g32,g64,f16`: axes by prefix, every combination);
/// `{a|b|c}` in both is one alternative per row, in step; `[A|B]` in the
/// assembler column lists the opcodes one row may be (`LDRSH[W|X]ui`).
pub fn parse_opcodes(
    text: &str,
    dialect: &Dialect,
) -> Result<Vec<Opcode>, String> {
    let file = dialect.opcodes.as_deref().unwrap_or(&dialect.file);
    let mut with_column = dialect.clone();
    with_column.assembler_column = true;
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = raw.split('#').next().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let columns: Vec<&str> = content.split_whitespace().collect();
        if columns.len() != 10 {
            return Err(format!("{file}:{line}: {} columns, not 10", columns.len()));
        }
        let (name, llvm) = (columns[0], columns[9]);
        let mut axes: Vec<(char, Vec<u32>)> = Vec::new();
        if columns[3] != "-" {
            for item in columns[3].split(',') {
                let axis = item.chars().next().unwrap_or(' ');
                let bits: u32 = item[axis.len_utf8()..]
                    .parse()
                    .map_err(|_| format!("{file}:{line}: width `{item}` is not an axis letter and a number"))?;
                match axes.iter_mut().find(|(one, _)| *one == axis) {
                    Some((_, list)) => list.push(bits),
                    None => axes.push((axis, vec![bits])),
                }
            }
        }
        let steps = alternatives(name).max(alternatives(llvm));
        let steps = if steps == 0 { 1 } else { steps };
        let mut combos: Vec<Vec<(char, u32)>> = vec![Vec::new()];
        for (axis, list) in &axes {
            combos = combos
                .iter()
                .flat_map(|so_far| {
                    list.iter().map(move |bits| {
                        let mut next = so_far.clone();
                        next.push((*axis, *bits));
                        next
                    })
                })
                .collect();
        }
        for combo in &combos {
            for step in 0..steps {
                let spell = |template: &str| -> Result<String, String> {
                    let mut text = String::new();
                    let mut rest = template;
                    while let Some(open) = rest.find('{') {
                        text.push_str(&rest[..open]);
                        let close = rest[open..]
                            .find('}')
                            .map(|at| open + at)
                            .ok_or_else(|| format!("{file}:{line}: `{template}` has an unclosed `{{`"))?;
                        let inside = &rest[open + 1..close];
                        if inside.contains('|') {
                            let list: Vec<&str> = inside.split('|').collect();
                            text.push_str(list.get(step).ok_or_else(|| {
                                format!("{file}:{line}: `{template}`: alternative {step} of `{{{inside}}}` is missing")
                            })?);
                        } else if let Some(axis) = inside.chars().next().filter(|axis| axis.is_ascii_alphabetic()) {
                            let bits =
                                combo.iter().find(|(one, _)| *one == axis).map(|(_, bits)| *bits).ok_or_else(|| {
                                    format!("{file}:{line}: `{{{inside}}}` has no `{axis}` width in the row")
                                })?;
                            let letter = dialect
                                .letters
                                .iter()
                                .find(|(one, _)| one == inside)
                                .and_then(|(_, list)| list.iter().find(|(width, _)| *width == bits))
                                .map(|(_, letter)| letter.as_str())
                                .ok_or_else(|| format!("{file}:{line}: no letter for `{inside}` at {bits} bits"))?;
                            text.push_str(letter);
                        } else {
                            return Err(format!("{file}:{line}: `{{{inside}}}` is no placeholder"));
                        }
                        rest = &rest[close + 1..];
                    }
                    text.push_str(rest);
                    Ok(text)
                };
                let (name, llvm) = (spell(name)?, spell(llvm)?);
                let (first, alternatives) = match (llvm.find('['), llvm.find(']')) {
                    (Some(open), Some(close)) if open < close => {
                        let (head, tail) = (&llvm[..open], &llvm[close + 1..]);
                        let all: Vec<String> =
                            llvm[open + 1..close].split('|').map(|one| format!("{head}{one}{tail}")).collect();
                        (all[0].clone(), all)
                    }
                    (None, None) => (llvm.clone(), vec![llvm.clone()]),
                    _ => return Err(format!("{file}:{line}: `{llvm}` has an unmatched `[` or `]`")),
                };
                let width = combo.first().map_or("-".to_owned(), |(_, bits)| bits.to_string());
                let synthesized = format!(
                    "{name} {} {} {width} {} {} {} {} {} {first}",
                    columns[1], columns[2], columns[4], columns[5], columns[6], columns[7], columns[8]
                );
                let mut forms = parse(&synthesized, &with_column).map_err(|error| format!("{file}:{line}: {error}"))?;
                for form in forms.drain(..) {
                    out.push(Opcode {
                        name: form.name.clone(),
                        llvm: alternatives.clone(),
                        form: Form { line, ..form },
                    });
                }
            }
        }
    }
    Ok(out)
}

fn alternatives(template: &str) -> usize {
    template
        .split('{')
        .skip(1)
        .filter_map(|part| part.split('}').next())
        .filter(|inside| inside.contains('|'))
        .map(|inside| inside.split('|').count())
        .max()
        .unwrap_or(0)
}

thread_local! {
    static PARSED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has read a forms text, for the test that a
/// text is read once however many readers ask.
pub fn parsed() -> usize {
    PARSED.with(std::cell::Cell::get)
}

/// [`parse`], once per distinct text: the DAG tables and the register classes
/// each read the same forms (the classes only the pinned rows, through
/// [`pinned`], a second scan of every row for 0.6 M Ir). A target's text is
/// read with its own dialect.
pub fn shared(
    text: &str,
    dialect: &Dialect,
) -> Result<std::sync::Arc<Vec<Form>>, String> {
    static FORMS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Vec<Form>>>>> =
        std::sync::LazyLock::new(Default::default);
    if let Some(found) = FORMS.lock().expect("the forms lock").get(text) {
        return Ok(std::sync::Arc::clone(found));
    }
    let forms = std::sync::Arc::new(parse(text, dialect)?);
    FORMS.lock().expect("the forms lock").insert(text.to_owned(), std::sync::Arc::clone(&forms));
    Ok(forms)
}

fn parse_rows(
    text: &str,
    d: &Dialect,
    keep: impl Fn(&str) -> bool,
) -> Result<Vec<Form>, String> {
    PARSED.with(|count| count.set(count.get() + 1));
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
    /// A dialect with letters, as arm64's opcode file reads.
    fn letters() -> Dialect {
        Dialect::parse(
            "file = \"arm64.instr\"\nregisters = [\"x0\"]\nwidths = [16, 32, 64]\n\
             [flags]\nZ = 4\n[letters]\ng = { 32 = \"W\", 64 = \"X\" }\nf = { 16 = \"H\", 32 = \"S\", 64 = \"D\" }\n",
        )
        .expect("a dialect")
    }

    fn names(text: &str) -> Vec<String> {
        parse_opcodes(text, &letters()).expect("parses").into_iter().map(|one| one.name).collect()
    }

    /// One row names every combination of its axes' widths, an opcode file's
    /// template standing for the lines nobody should copy by hand.
    #[test]
    fn an_opcode_row_names_every_combination_of_its_widths() {
        assert_eq!(names("ADD{g}ri binary r/r,i,i g32,g64 alu - - - -/- ADD{g}ri"), ["ADDWri", "ADDXri"]);
        let both = names("FCVTZS{g}{f}r unary r/x g32,g64,f16,f32,f64 fp_cvt - - - -/- FCVTZS{g}{f}r");
        assert_eq!(both.len(), 6);
        assert!(both.contains(&"FCVTZSXHr".to_owned()) && both.contains(&"FCVTZSWDr".to_owned()));
    }

    /// An alternative of the name goes with the same alternative of the opcode;
    /// `[A|B]` lists the opcodes one name may be.
    #[test]
    fn alternatives_go_in_step_and_a_bracket_lists_opcodes() {
        let rows = parse_opcodes(
            "LDRX.{ui|pre} move r/m - load - - - -/- LDRX{ui|pre}\nLDRSH.ui move r/m - load - - - -/- LDRSH[W|X]ui",
            &letters(),
        )
        .expect("parses");
        assert_eq!((rows[0].name.as_str(), rows[0].llvm.clone()), ("LDRX.ui", vec!["LDRXui".to_owned()]));
        assert_eq!((rows[1].name.as_str(), rows[1].llvm.clone()), ("LDRX.pre", vec!["LDRXpre".to_owned()]));
        assert_eq!(rows[2].llvm, ["LDRSHWui", "LDRSHXui"]);
    }

    /// A placeholder with no width in the row, or no letter for it, is an
    /// error naming the line.
    #[test]
    fn a_placeholder_without_a_letter_names_its_line() {
        let error = parse_opcodes("ADD{g}ri binary r/r,i,i - alu - - - -/- ADD{g}ri", &letters()).unwrap_err();
        assert!(error.contains(":1:") && error.contains("{g}"), "{error}");
        let error = parse_opcodes("ADD{g}ri binary r/r,i,i g16 alu - - - -/- ADD{g}ri", &letters()).unwrap_err();
        assert!(error.contains("no letter for `g` at 16 bits"), "{error}");
    }

    use super::*;

    /// A dialect with an assembler column, as x86's.
    fn dialect() -> Dialect {
        Dialect::parse(
            "file = \"x86.instr\"\nregisters = [\"ax\", \"cx\"]\nwidths = [8, 16, 32]\nassembler_column = true\n\
             [flags]\nCF = 16\nZF = 4\n",
        )
        .expect("a dialect")
    }

    /// The DAG tables and the register classes each read the x86 forms: two
    /// reads of one text per compile. However often a text is asked, it is
    /// read once.
    #[test]
    fn test_a_forms_text_is_read_once_however_often_it_is_asked() {
        let text = "forms_once_probe binary r/r,r 32 alu_rr - - - -/- Add_{w}";
        let before = parsed();
        for _ in 0..20 {
            assert_eq!(shared(text, &dialect()).expect("parses").len(), 1);
        }
        assert_eq!(parsed() - before, 1);
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
