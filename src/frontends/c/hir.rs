//! Port of `qbopt/cfront/hir.py`: the stream as a unit, its symbols, its data,
//! and each procedure's trees.
//!
//! The trees are the code generator's own: a node is the call that built it,
//! and its operands are the handles of the nodes it was built from. Nothing is
//! lowered here; `raise_hir` does that.

use crate::support::hash::HashSet;
use std::fmt;

use crate::support::hash::IndexMap;

use super::stream::Record;
use crate::support::pyrepr::{self, Repr, Tuple};

// fe_attr (bld/cg/h/cg.h)
pub const FE_PROC: i64 = 0x1;
pub const FE_CONSTANT: i64 = 0x10;
pub const FE_VOLATILE: i64 = 0x800;
pub const FE_INTERNAL: i64 = 0x1000;
pub const FE_GLOBAL: i64 = 0x4;
pub const FE_IMPORT: i64 = 0x8;
pub const PRIVATE: i64 = 0x40; // a segment of its own, outside DGROUP
// call_class and call_class_target (cgauxcc.h, x86auxcc.h)
pub const REVERSE_PARMS: i64 = 0x1;
pub const CALLER_POPS: i64 = 0x80;
pub const FAR_CALL: i64 = 0x4;
// cg_target_switches (x86swi.h)
pub const BIG_DATA: i64 = 0x2;
pub const BIG_CODE: i64 = 0x4;

/// Borland headers expose these as compiler intrinsics, while their callable
/// medium-model fallbacks use the ordinary runtime entry-point names.
pub fn intrinsic_runtime(base: &str) -> Option<&'static str> {
    match base {
        "__inportb__" => Some("inportb"),
        "__inportw__" => Some("inport"),
        "__outportb__" => Some("outportb"),
        "__outportw__" => Some("outport"),
        _ => None,
    }
}

pub const STATEMENTS: [&str; 9] = [
    "CGDone",
    "CGTrash",
    "CGControl",
    "CGReturn",
    "CGSelCase",
    "CGSelRange",
    "CGSelOther",
    "CGSelect",
    "CGBigLabel",
];
pub const IGNORED: [&str; 9] = [
    "START",
    "STOP",
    "FINI",
    "ABORT",
    "BENewLabel",
    "BEFiniLabel",
    "CGLastParm",
    "DBSrcFile",
    "BEFiniBack",
];

/// A construct the C path refuses rather than guesses at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unsupported(pub String);

impl fmt::Display for Unsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unsupported {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub at: i64,      // the two-byte hole in the code
    pub kind: String, // offset | segment | reloff
    pub symbol: i64,
    pub offset: i64,
}

impl Repr for Fixup {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Fixup",
            &[
                ("at", self.at.repr()),
                ("kind", self.kind.repr()),
                ("symbol", self.symbol.repr()),
                ("offset", self.offset.repr()),
            ],
        )
    }
}

/// Inline assembly, as the pragma the front end made of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Code {
    pub data: Vec<u8>,
    pub fixups: Vec<Fixup>,
}

impl Repr for Code {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Code",
            &[
                ("data", pyrepr::bytes(&self.data)),
                ("fixups", pyrepr::tuple(&self.fixups)),
            ],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub id: i64,
    pub name: String,
    pub base: String,
    pub pattern: String,
    pub attr: i64,
    pub call_class: i64,
    pub call_target: i64,
    pub register_parms: bool, // any argument passed in a register
    pub code: Option<Code>,
    pub segment: i64,
}

impl Symbol {
    pub fn proc(&self) -> bool {
        self.attr & FE_PROC != 0
    }

    pub fn imported(&self) -> bool {
        self.attr & FE_IMPORT != 0
    }

    pub fn constant(&self) -> bool {
        self.attr & FE_CONSTANT != 0
    }

    pub fn volatile(&self) -> bool {
        self.attr & FE_VOLATILE != 0
    }

    pub fn internal(&self) -> bool {
        self.attr & FE_INTERNAL != 0
    }

    pub fn exported(&self) -> bool {
        self.attr & FE_GLOBAL != 0 && !self.imported()
    }

    pub fn far(&self) -> bool {
        self.call_target & FAR_CALL != 0
    }

    pub fn object_name(&self) -> String {
        if self.pattern == "^" {
            return self.base.to_uppercase();
        }
        let base = intrinsic_runtime(&self.base).unwrap_or(&self.base);
        if self.pattern.is_empty() {
            base.to_owned()
        } else {
            self.pattern.replace('*', base)
        }
    }
}

impl Repr for Symbol {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Symbol",
            &[
                ("id", self.id.repr()),
                ("name", self.name.repr()),
                ("base", self.base.repr()),
                ("pattern", self.pattern.repr()),
                ("attr", self.attr.repr()),
                ("call_class", self.call_class.repr()),
                ("call_target", self.call_target.repr()),
                ("register_parms", self.register_parms.repr()),
                ("code", self.code.repr()),
                ("segment", self.segment.repr()),
            ],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub call: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    pub target: String,
    pub type_: String,
    pub symbol: i64,
    pub parms: Vec<(String, String)>, // (node, type), last argument first
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Statement {
    pub call: String,
    pub args: Vec<String>,
    pub line: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proc {
    pub symbol: i64,
    pub type_: String,
    pub parms: Vec<(i64, String)>,
    pub autos: Vec<(String, String)>, // ("y5" | "t3", type)
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub id: i64,
    pub name: String,
    pub attr: i64,
    pub items: Vec<(String, Tuple<String>)>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Unit {
    pub target: i64,
    pub symbols: IndexMap<i64, Symbol>,
    pub backs: IndexMap<i64, i64>, // back handle -> symbol, 0 for a literal
    pub types: IndexMap<String, i64>,
    pub aliases: IndexMap<String, String>,
    pub segments: IndexMap<i64, Segment>,
    pub nodes: IndexMap<i64, Node>,
    pub calls: IndexMap<i64, Call>,
    pub procs: Vec<Proc>,
}

impl Unit {
    /// Whether the symbol is in DGROUP, reached through DS.
    pub fn grouped(&self, symbol: &Symbol) -> bool {
        if symbol.imported() && symbol.segment < 0 {
            // Open Watcom reports -1 for an explicitly far imported object;
            // its selector is the external itself, never DGROUP.
            return false;
        }
        match self.segments.get(&symbol.segment) {
            None => true,
            Some(segment) => segment.attr & PRIVATE == 0,
        }
    }

    pub fn canonical_type(&self, type_: &str) -> String {
        let mut type_ = type_.to_owned();
        let mut seen = HashSet::default();
        while let Some(next) = self.aliases.get(&type_) {
            if !seen.insert(type_.clone()) {
                break;
            }
            type_ = next.clone();
        }
        type_
    }
}

/// `int(token[1:])`.
pub fn handle(token: &str) -> i64 {
    int(&token[1..])
}

/// `int(text)`.
pub(crate) fn int(text: &str) -> i64 {
    text.trim()
        .parse()
        .unwrap_or_else(|_| panic!("ValueError: invalid literal for int() with base 10: {text:?}"))
}

/// `int(text, 16)`.
pub(crate) fn hex(text: &str) -> i64 {
    let digits = text.trim();
    let digits = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
        .unwrap_or(digits);
    i64::from_str_radix(digits, 16)
        .unwrap_or_else(|_| panic!("ValueError: invalid literal for int() with base 16: {text:?}"))
}

fn field<'a>(one: &'a Record, key: &str) -> &'a str {
    one.fields
        .get(key)
        .unwrap_or_else(|| panic!("KeyError: {key:?}"))
}

fn arg(one: &Record, index: usize) -> &str {
    one.args
        .get(index)
        .unwrap_or_else(|| panic!("IndexError: tuple index out of range"))
}

/// The current `proc`; Python raises `AttributeError` on `None`.
fn open(proc: Option<usize>) -> usize {
    proc.expect("AttributeError: 'NoneType' object has no attribute")
}

pub fn unit(records: &[Record]) -> Result<Unit, Unsupported> {
    let mut made = Unit::default();
    let mut proc: Option<usize> = None;
    let mut segment: Option<i64> = None;
    let mut line = 0;
    for one in records {
        let args = &one.args;
        match one.call.as_str() {
            "UNSUPPORTED" => {
                return Err(Unsupported(format!(
                    "stream line {}: the shim refused {}",
                    one.line,
                    args.join(" ")
                )));
            }
            "INIT" => made.target = hex(field(one, "target")),
            "SEG" => {
                let id = int(arg(one, 0));
                made.segments.insert(
                    id,
                    Segment {
                        id,
                        name: field(one, "name").to_owned(),
                        attr: hex(field(one, "attr")),
                        items: Vec::new(),
                    },
                );
            }
            "SETSEG" => segment = Some(int(arg(one, 0))),
            "TYPE" => {
                made.types
                    .insert(arg(one, 0).to_owned(), int(field(one, "size")));
            }
            "ALIAS" => {
                made.aliases
                    .insert(arg(one, 0).to_owned(), arg(one, 1).to_owned());
            }
            "SYM" => {
                let id = handle(arg(one, 0));
                made.symbols.insert(
                    id,
                    Symbol {
                        id,
                        name: field(one, "name").to_owned(),
                        base: field(one, "base").to_owned(),
                        pattern: field(one, "pattern").to_owned(),
                        attr: hex(field(one, "attr")),
                        call_class: 0,
                        call_target: 0,
                        register_parms: false,
                        code: None,
                        segment: int(one.fields.get("seg").map_or("0", String::as_str)),
                    },
                );
            }
            "CALLCONV" => {
                let symbol = made
                    .symbols
                    .get_mut(&handle(arg(one, 0)))
                    .expect("KeyError: symbol");
                symbol.call_class = hex(field(one, "class"));
                symbol.call_target = hex(field(one, "target"));
                symbol.register_parms =
                    one.fields.get("parms").map_or("[]", String::as_str) != "[]";
            }
            "CODE" => {
                let fixups = field(one, "fix")
                    .split(',')
                    .filter(|one| *one != "-")
                    .map(|one| {
                        let parts: Vec<&str> = one.split(':').collect();
                        let [at, kind, target, offset] = parts[..] else {
                            panic!("ValueError: not enough values to unpack");
                        };
                        Fixup {
                            at: int(at),
                            kind: kind.to_owned(),
                            symbol: handle(target),
                            offset: int(offset),
                        }
                    })
                    .collect();
                let data = from_hex(field(one, "bytes"));
                made.symbols
                    .get_mut(&handle(arg(one, 0)))
                    .expect("KeyError: symbol")
                    .code = Some(Code { data, fixups });
            }
            "BENewBack" => {
                let result = one
                    .result
                    .as_deref()
                    .expect("TypeError: 'NoneType' object is not subscriptable");
                made.backs.insert(handle(result), handle(arg(one, 0)));
            }
            "CGProcDecl" => {
                made.procs.push(Proc {
                    symbol: handle(arg(one, 0)),
                    type_: arg(one, 1).to_owned(),
                    parms: Vec::new(),
                    autos: Vec::new(),
                    body: Vec::new(),
                });
                proc = Some(made.procs.len() - 1);
            }
            "CGParmDecl" => {
                let at = open(proc);
                let parm = (handle(arg(one, 0)), arg(one, 1).to_owned());
                made.procs[at].parms.push(parm);
            }
            "CGAutoDecl" => {
                let at = open(proc);
                let auto = (arg(one, 0).to_owned(), arg(one, 1).to_owned());
                made.procs[at].autos.push(auto);
            }
            "CGTemp" => {
                let at = open(proc);
                let result = one.result.clone().expect("CGTemp returns a handle");
                made.procs[at].autos.push((result, arg(one, 0).to_owned()));
            }
            "CGInitCall" => {
                let result = one.result.as_deref().expect("CGInitCall returns a handle");
                made.calls.insert(
                    handle(result),
                    Call {
                        target: arg(one, 0).to_owned(),
                        type_: arg(one, 1).to_owned(),
                        symbol: handle(arg(one, 2)),
                        parms: Vec::new(),
                    },
                );
            }
            "CGAddParm" => {
                let parm = (arg(one, 1).to_owned(), arg(one, 2).to_owned());
                made.calls
                    .get_mut(&handle(arg(one, 0)))
                    .expect("KeyError: call")
                    .parms
                    .push(parm);
            }
            "DBSrcCue" => line = int(arg(one, 1)),
            "CGSelInit" => {
                let at = open(proc);
                let result = one.result.clone().expect("CGSelInit returns a handle");
                made.procs[at].body.push(Statement {
                    call: one.call.clone(),
                    args: vec![result],
                    line,
                });
            }
            call if STATEMENTS.contains(&call) => {
                let at = open(proc);
                made.procs[at].body.push(Statement {
                    call: call.to_owned(),
                    args: args.clone(),
                    line,
                });
            }
            call if call.starts_with("DG") => {
                let Some(segment) = segment else {
                    return Err(Unsupported(format!(
                        "stream line {}: data before any segment",
                        one.line
                    )));
                };
                made.segments
                    .get_mut(&segment)
                    .expect("KeyError: segment")
                    .items
                    .push((call.to_owned(), Tuple(args.clone())));
            }
            call if IGNORED.contains(&call) => {}
            _ if one
                .result
                .as_deref()
                .is_some_and(|result| result.starts_with('n')) =>
            {
                let result = one.result.as_deref().expect("checked");
                made.nodes.insert(
                    handle(result),
                    Node {
                        call: one.call.clone(),
                        args: args.clone(),
                    },
                );
            }
            _ => {
                return Err(Unsupported(format!(
                    "stream line {}: {}",
                    one.line, one.call
                )));
            }
        }
    }
    Ok(made)
}

/// `bytes.fromhex`.
fn from_hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    digits
        .chunks(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("ascii");
            u8::from_str_radix(pair, 16)
                .unwrap_or_else(|_| panic!("ValueError: non-hexadecimal number found in fromhex()"))
        })
        .collect()
}

/// The unit, one fact per line, for a stage dump.
pub fn text(made: &Unit) -> String {
    let mut out = vec![format!("target 0x{:x}", made.target)];
    for one in made.segments.values() {
        out.push(format!(
            "segment {} {} attr=0x{:x} items={}",
            one.id,
            one.name,
            one.attr,
            one.items.repr()
        ));
    }
    for one in made.symbols.values() {
        out.push(format!("symbol {}", one.repr()));
    }
    for proc in &made.procs {
        out.push(format!(
            "proc {} {} parms={} autos={}",
            made.symbols[&proc.symbol].object_name(),
            proc.type_,
            proc.parms.repr(),
            proc.autos.repr()
        ));
        for one in &proc.body {
            out.push(format!(
                "  {}: {} {}",
                one.line,
                one.call,
                one.args.join(" ")
            ));
        }
    }
    out.join("\n") + "\n"
}
