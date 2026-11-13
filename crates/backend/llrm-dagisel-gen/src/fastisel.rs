//! Reads the pin's `X86GenFastISel.inc` (`llvm-tblgen -gen-fast-isel`) into
//! rows.
//!
//! # How it works
//! Each `fastEmit_<ISD>_<OP>_MVT_<vt>_<shape>` function becomes one row: the
//! result type it accepts and its arms, each an optional subtarget condition,
//! an opcode and a register class, in the pin's order. A top-level dispatcher
//! (`fastEmit_ISD_ADD_rr`) is counted and left out: it is derived from the
//! rows. A function of any other form is counted as unread, never dropped.
//!
//! # Used by
//! Nothing yet (step 1 of the FastISel generator).

use std::collections::BTreeMap;

/// The operand shape of a `fastEmit_*` function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    R,
    Rr,
    Ri,
    /// `_ri_Predicate_<name>`: the immediate must pass the named test.
    RiPredicated,
    I,
}

impl Shape {
    /// The number the generated table stores.
    pub fn code(self) -> u8 {
        match self {
            Shape::R => 0,
            Shape::Rr => 1,
            Shape::Ri => 2,
            Shape::RiPredicated => 3,
            Shape::I => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    /// The subtarget test as the pin spells it; none for an unconditional arm.
    pub cond: Option<String>,
    pub opcode: String,
    pub class: String,
}

#[derive(Clone, Debug, Default)]
pub struct Rows {
    /// The `RetVT` the function accepts (`None`: the body does not test it).
    pub ret: Option<String>,
    pub imm_predicate: Option<String>,
    pub arms: Vec<Arm>,
}

/// Namespace, operation, source type, result type (empty when the body tests
/// `RetVT`), shape.
type Key = (String, String, String, String, Shape);

#[derive(Default)]
pub struct Table {
    rows: BTreeMap<Key, Rows>,
    dispatchers: usize,
    unread: usize,
}

impl Table {
    /// The row for `<namespace>::<op>` of `vt` (`"ISD"`, `"ADD"`, `"i32"`).
    pub fn rows(
        &self,
        namespace: &str,
        op: &str,
        vt: &str,
        shape: Shape,
    ) -> Rows {
        let key = (namespace.to_owned(), op.to_owned(), vt.to_owned(), String::new(), shape);
        self.rows.get(&key).cloned().unwrap_or_default()
    }

    /// Like [`Table::rows`] for a function named with its result type too
    /// (`fastEmit_ISD_SIGN_EXTEND_MVT_i8_MVT_i32_r`).
    pub fn rows_to(
        &self,
        namespace: &str,
        op: &str,
        vt: &str,
        ret: &str,
        shape: Shape,
    ) -> Rows {
        let key = (namespace.to_owned(), op.to_owned(), vt.to_owned(), ret.to_owned(), shape);
        self.rows.get(&key).cloned().unwrap_or_default()
    }

    /// The arm each row takes on `cpu`: the first whose condition the CPU
    /// states true, as in the pin's `fastEmit_*` bodies. A row whose arms are
    /// all false is absent; one whose arm needs a run-time answer is an error.
    pub fn resolve(
        &self,
        cpu: &crate::features::Cpu,
    ) -> Result<Chosen, String> {
        use crate::predicate::{self, Decided};
        let mut picked = BTreeMap::new();
        let mut undecided = 0;
        'rows: for (key, rows) in &self.rows {
            for arm in &rows.arms {
                let take = match &arm.cond {
                    None => true,
                    // An atom the CPU file does not state, or one decided at
                    // run time, leaves the whole row to the DAG: a later arm
                    // may not be taken past an arm that might have been.
                    Some(text) => match predicate::parse(text).and_then(|e| e.decide(&|atom| cpu.verdict(atom))) {
                        Ok(Decided::Yes) => true,
                        Ok(Decided::No) => false,
                        Ok(Decided::Open(_)) | Err(_) => {
                            undecided += 1;
                            continue 'rows;
                        }
                    },
                };
                if take {
                    picked.insert(key.clone(), arm.clone());
                    continue 'rows;
                }
            }
        }
        Ok(Chosen { picked, undecided })
    }

    pub fn parsed(&self) -> usize {
        self.rows.len()
    }

    pub fn dispatchers(&self) -> usize {
        self.dispatchers
    }

    pub fn unread(&self) -> usize {
        self.unread
    }
}

/// `<ns>_<OP>_MVT_<vt>_<shape>[_Predicate_<p>]` of a function name, if it has
/// that form.
fn split_name(name: &str) -> Option<(Key, Option<String>)> {
    let rest = name.strip_prefix("fastEmit_")?;
    let (namespace, rest) = rest.split_once('_')?;
    if namespace != "ISD" && namespace != "X86ISD" {
        return None;
    }
    let (op, rest) = rest.split_once("_MVT_")?;
    let (rest, ret) = match rest.split_once("_MVT_") {
        Some((vt, tail)) => {
            if tail.contains("_MVT_") {
                return None;
            }
            // `i8_MVT_i32_r`: the source type, then the result type and shape.
            let (ret, shape) = tail.rsplit_once('_')?;
            (format!("{vt}_{shape}"), ret.to_owned())
        }
        None => (rest.to_owned(), String::new()),
    };
    let rest = rest.as_str();
    let (rest, predicate) = match rest.split_once("_Predicate_") {
        Some((head, p)) => (head, Some(p.to_owned())),
        None => (rest, None),
    };
    let (vt, shape) = rest.rsplit_once('_')?;
    let shape = match (shape, predicate.is_some()) {
        ("r", false) => Shape::R,
        ("rr", false) => Shape::Rr,
        ("ri", false) => Shape::Ri,
        ("ri", true) => Shape::RiPredicated,
        ("i", false) => Shape::I,
        _ => return None,
    };
    Some(((namespace.to_owned(), op.to_owned(), vt.to_owned(), ret, shape), predicate))
}

/// `text` without parentheses that enclose all of it.
fn unwrap(text: &str) -> &str {
    let mut text = text.trim();
    while text.starts_with('(') && text.ends_with(')') {
        let mut depth = 0;
        let closes_last = text
            .char_indices()
            .all(
                |(at, c)| {
                    match c {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    depth > 0 || at == text.len() - 1
                },
            );
        if !closes_last {
            break;
        }
        text = text[1..text.len() - 1].trim();
    }
    text
}

/// One `return fastEmitInst_*(X86::OPC, &X86::CLASS, ...)` line.
fn inst(line: &str) -> Option<(String, String)> {
    let args = line.trim().strip_prefix("return fastEmitInst_")?;
    let args = args.split_once('(')?.1;
    let mut parts = args.split(',');
    let opcode = parts.next()?.trim().strip_prefix("X86::")?;
    let class = parts.next()?.trim().strip_prefix("&X86::")?.strip_suffix("RegClass")?;
    Some((opcode.to_owned(), class.to_owned()))
}

/// The arms and result type of one function body, or `None` for a form not
/// read.
fn body(lines: &[&str]) -> Option<(Option<String>, Vec<Arm>)> {
    let mut ret = None;
    let mut arms = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at].trim();
        at += 1;
        if let Some(vt) = line.strip_prefix("if (RetVT.SimpleTy != MVT::") {
            ret = Some(vt.strip_suffix(')')?.to_owned());
            // the `return Register();` that follows
            at += 1;
        } else if let Some(cond) = line.strip_prefix("if (").and_then(|l| l.strip_suffix(") {")) {
            let cond = unwrap(cond);
            let (opcode, class) = inst(lines.get(at)?)?;
            at += 1;
            if lines.get(at)?.trim() != "}" {
                return None;
            }
            at += 1;
            arms.push(Arm { cond: Some(cond.to_owned()), opcode, class });
        } else if line == "return Register();" || line.is_empty() {
        } else if let Some((opcode, class)) = inst(line) {
            arms.push(Arm { cond: None, opcode, class });
        } else {
            return None;
        }
    }
    Some((ret, arms))
}

pub fn parse(text: &str) -> Result<Table, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut table = Table::default();
    let mut at = 0;
    while at < lines.len() {
        let Some(head) = lines[at].strip_prefix("Register fastEmit_") else {
            at += 1;
            continue;
        };
        let name = format!("fastEmit_{}", head.split('(').next().unwrap_or(""));
        let start = at + 1;
        let mut end = start;
        while end < lines.len() && lines[end] != "}" {
            end += 1;
        }
        if end == lines.len() {
            return Err(format!("{name}: no closing brace"));
        }
        at = end + 1;
        // A body that switches on `RetVT` only picks among functions of their
        // own.
        let switches = lines[start..end].iter().any(|l| l.contains("switch (RetVT"));
        if head.contains("override") || head.contains("(MVT VT,") || switches {
            table.dispatchers += 1;
            continue;
        }
        let read = split_name(&name).and_then(|(key, predicate)| {
            let (tested, arms) = body(&lines[start..end])?;
            let ret = if key.3.is_empty() { tested } else { Some(key.3.clone()) };
            Some((key, Rows { ret, imm_predicate: predicate, arms }))
        });
        match read {
            Some((key, rows)) => {
                table.rows.insert(key, rows);
            }
            None => table.unread += 1,
        }
    }
    Ok(table)
}

/// The arm of each row a CPU takes.
pub struct Chosen {
    picked: BTreeMap<Key, Arm>,
    undecided: usize,
}

impl Chosen {
    /// The rows that have an arm on this CPU.
    pub fn len(&self) -> usize {
        self.picked.len()
    }

    pub fn is_empty(&self) -> bool {
        self.picked.is_empty()
    }

    /// Rows left out because a condition could not be decided for the CPU.
    pub fn undecided(&self) -> usize {
        self.undecided
    }

    /// The rows as Rust source: `(namespace, op, vt, ret, shape, opcode,
    /// class)` sorted by key, for binary search.
    pub fn render(
        &self,
        cpu: &str,
    ) -> String {
        use std::fmt::Write as _;
        let mut out = format!(
            "//! Generated by `tools/dagisel/regen.sh` from the pin's `X86GenFastISel.inc` for `{cpu}`.\n//! Do not edit.\n\n/// `(namespace, op, vt, ret, shape, opcode, register class)` by key.\npub static ROWS: &[(&str, &str, &str, &str, u8, &str, &str)] = &[\n"
        );
        let mut rows: Vec<String> = self
            .picked
            .iter()
            .map(|((ns, op, vt, ret, shape), arm)| {
                format!(
                    "    (\"{ns}\", \"{op}\", \"{vt}\", \"{ret}\", {}, \"{}\", \"{}\"),",
                    shape.code(),
                    arm.opcode,
                    arm.class
                )
            })
            .collect();
        rows.sort();
        for row in rows {
            writeln!(out, "{row}").unwrap();
        }
        out.push_str("];\n");
        out
    }

    pub fn pick(
        &self,
        namespace: &str,
        op: &str,
        vt: &str,
        shape: Shape,
    ) -> Option<&Arm> {
        self.picked.get(&(namespace.to_owned(), op.to_owned(), vt.to_owned(), String::new(), shape))
    }
}
