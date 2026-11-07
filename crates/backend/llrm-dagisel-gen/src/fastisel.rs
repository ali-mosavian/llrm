//! Reads the pin's `X86GenFastISel.inc` (`llvm-tblgen -gen-fast-isel`) into rows.
//!
//! # How it works
//! Each `fastEmit_<ISD>_<OP>_MVT_<vt>_<shape>` function becomes one row: the
//! result type it accepts and its arms, each an optional subtarget condition,
//! an opcode and a register class, in the pin's order. A top-level dispatcher
//! (`fastEmit_ISD_ADD_rr`) is counted and left out: it is derived from the rows.
//! A function of any other form is counted as unread, never dropped.
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

type Key = (String, String, String, Shape);

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
        let key = (namespace.to_owned(), op.to_owned(), vt.to_owned(), shape);
        self.rows.get(&key).cloned().unwrap_or_default()
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
    if rest.contains("_MVT_") {
        return None;
    }
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
    Some(((namespace.to_owned(), op.to_owned(), vt.to_owned(), shape), predicate))
}

/// `text` without parentheses that enclose all of it.
fn unwrap(text: &str) -> &str {
    let mut text = text.trim();
    while text.starts_with('(') && text.ends_with(')') {
        let mut depth = 0;
        let closes_last = text.char_indices().all(|(at, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            depth > 0 || at == text.len() - 1
        });
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

/// The arms and result type of one function body, or `None` for a form not read.
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
        if head.contains("override") || head.contains("(MVT VT,") {
            table.dispatchers += 1;
            continue;
        }
        let read = split_name(&name).and_then(|(key, predicate)| {
            let (ret, arms) = body(&lines[start..end])?;
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
