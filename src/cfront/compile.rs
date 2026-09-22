//! Port of `qbopt/cfront/compile.py`: C through Open Watcom's front end and
//! the backend, to an object or jwasm source.
//!
//! ```text
//! llrm-c pal.cgs -o pal.obj [--dump DIR] [--opt]
//! ```
//!
//! Stages not yet ported stop with [`CompileError::NotPorted`], naming the
//! Python function the port has reached.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use std::collections::BTreeSet;

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::{hir, libfunc, raise_hir, stream};
use crate::analysis::alias;
use crate::backend::lower_int64;
use crate::model::mir::{self, Arg, Const, MemRef, MirBody};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::{self, Repr};

#[derive(Debug)]
pub enum CompileError {
    Unsupported(hir::Unsupported),
    NotPorted(&'static str),
    Io(std::io::Error),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(error) => write!(formatter, "{error}"),
            Self::NotPorted(function) => write!(formatter, "not yet ported: {function}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<hir::Unsupported> for CompileError {
    fn from(error: hir::Unsupported) -> Self {
        Self::Unsupported(error)
    }
}

impl From<std::io::Error> for CompileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn assembled(
    text: &str,
    _module: &str,
    _optimise: bool,
    dump: Option<&Path>,
    _cpu: &str,
) -> Result<(), CompileError> {
    let unit = hir::unit(&stream::parse(text))?;
    write(dump, "stream", text)?;
    write(dump, "hir", &hir::text(&unit))?;
    let mut shared = raise_hir::Shared::default();
    let mut raised_procedures = Vec::new();
    for proc in &unit.procs {
        raised_procedures.push(raise_hir::raised(&unit, proc, &mut shared)?);
    }
    let aliases: IndexMap<String, alias::Procedure> = raised_procedures
        .iter()
        .map(|one| {
            let procedure =
                alias::Procedure { body: one.body.clone(), calls: one.calls.clone(), arguments: one.arguments.clone() };
            (one.name.clone(), procedure)
        })
        .collect();
    let callees: BTreeSet<String> = raised_procedures.iter().flat_map(|one| one.calls.values().cloned()).collect();
    let known = libfunc::summaries(callees.iter().map(String::as_str));
    let modref = alias::summaries(&aliases, Some(&known)).map_err(hir::Unsupported)?;
    let initial = _constant_initializers(&unit);
    let mut bodies: IndexMap<String, MirBody> = IndexMap::new();
    for one in &raised_procedures {
        let mut body = alias::calls_annotated(&aliases[&one.name], &modref).map_err(hir::Unsupported)?;
        body.initial = _body_initializers(&body, &initial);
        bodies.insert(one.name.clone(), body);
    }
    let mut mirs = Vec::new();
    for raised in &raised_procedures {
        let body = &bodies[&raised.name];
        mirs.push(_mir_text(&raised.name, body));
        let legalized =
            lower_int64::expanded(body, Some(&raised.calls), Some(&raised.contracts), Some(&raised.hints))
                .map_err(|error| hir::Unsupported(error.0))?;
        // Python compares `body is not raised.body`, which is never the same object.
        write(dump, &format!("passes/{}.int64-lower", raised.name), &_mir_text(&raised.name, &legalized.body))?;
    }
    // Python writes `mir` once every procedure is lowered; until lowering is
    // ported it is written here so the raise can be diffed.
    write(dump, "mir", &mirs.join("\n"))?;
    Err(CompileError::NotPorted("qbopt.backend.lower.lowered"))
}

/// Each named data object's `(segment, first item, after item)` span.
fn _data_labels(unit: &hir::Unit) -> IndexMap<i64, (i64, usize, usize)> {
    let mut out = IndexMap::new();
    for segment in unit.segments.values() {
        let labels: Vec<(usize, Option<i64>)> = segment
            .items
            .iter()
            .enumerate()
            .filter(|(_, (call, args))| call == "DGLabel" && !args.0.is_empty())
            .map(|(at, (_, args))| (at, unit.backs.get(&hir::handle(&args.0[0])).copied()))
            .collect();
        for (number, (start, symbol)) in labels.iter().enumerate() {
            if let Some(symbol) = symbol.filter(|symbol| unit.symbols.contains_key(symbol)) {
                let after = labels.get(number + 1).map_or(segment.items.len(), |next| next.0);
                out.insert(symbol, (segment.id, *start, after));
            }
        }
    }
    out
}

/// Exact loader bytes for private immutable C objects.
fn _constant_initializers(unit: &hir::Unit) -> Vec<(MemRef, Const)> {
    let labels = _data_labels(unit);
    let assembly_references: BTreeSet<i64> = unit
        .symbols
        .values()
        .filter_map(|symbol| symbol.code.as_ref())
        .flat_map(|code| code.fixups.iter().map(|fixup| fixup.symbol))
        .collect();
    let mut out = Vec::new();
    for (symbol_id, (segment_id, start, after)) in &labels {
        let symbol = &unit.symbols[symbol_id];
        if !symbol.constant()
            || !symbol.internal()
            || symbol.volatile()
            || symbol.imported()
            || symbol.proc()
            || assembly_references.contains(symbol_id)
        {
            continue;
        }
        let mut data: Vec<u8> = Vec::new();
        let mut complete = true;
        for (call, args) in &unit.segments[segment_id].items[start + 1..*after] {
            let args: Vec<&str> = args.0.iter().map(String::as_str).collect();
            match (call.as_str(), &args[..]) {
                ("DGBytes", [size, raw]) => {
                    let added = hex_bytes(raw);
                    if added.len() as i64 != hir::int(size) {
                        complete = false;
                        break;
                    }
                    data.extend(added);
                }
                ("DGIBytes", [size, byte]) => {
                    let count = usize::try_from(hir::int(size)).unwrap_or(0);
                    data.extend(std::iter::repeat_n((hir::int(byte) & 0xFF) as u8, count));
                }
                ("DGUBytes", [size]) => {
                    data.extend(std::iter::repeat_n(0, usize::try_from(hir::int(size)).unwrap_or(0)));
                }
                ("DGInteger", [value, type_]) => {
                    let width = raise_hir::widths(type_).unwrap_or(2);
                    let mask = (BigInt::from(1) << (8 * width)) - 1;
                    let number: BigInt = big(value) & mask;
                    let mut bytes = number.to_bytes_le().1;
                    bytes.resize(width as usize, 0);
                    data.extend(bytes);
                }
                _ => {
                    complete = false;
                    break;
                }
            }
        }
        if !complete {
            continue;
        }
        out.extend(data.iter().enumerate().map(|(offset, byte)| {
            let addr = Addr { index: *symbol_id, ..Addr::new(Space::Segment, offset as i64) };
            (MemRef::new(Some(addr), 1), Const::new(*byte, 1))
        }));
    }
    out
}

/// `bytes.fromhex(raw)`.
fn hex_bytes(raw: &str) -> Vec<u8> {
    let digits: Vec<char> = raw.chars().filter(|one| !one.is_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let text: String = pair.iter().collect();
            u8::from_str_radix(&text, 16)
                .unwrap_or_else(|_| panic!("ValueError: non-hexadecimal number found in fromhex() arg"))
        })
        .collect()
}

/// `int(text)`, unbounded.
fn big(text: &str) -> BigInt {
    text.trim().parse().unwrap_or_else(|_| panic!("ValueError: invalid literal for int() with base 10: {text:?}"))
}

/// Private data symbols that the emitted MIR directly names.
fn _referenced_data(body: &MirBody, candidates: &BTreeSet<i64>) -> BTreeSet<i64> {
    let mut found = BTreeSet::new();
    let mut symbol = |one: &mir::Symbol| {
        if one.space == Space::Segment && candidates.contains(&one.index) {
            found.insert(one.index);
        }
        if one.space == Space::Group && candidates.contains(&(one.index - raise_hir::SELECTOR)) {
            found.insert(one.index - raise_hir::SELECTOR);
        }
    };
    let mut reference = |one: &MemRef, symbol: &mut dyn FnMut(&mir::Symbol)| {
        if let Some(addr) = one.addr {
            symbol(&mir::Symbol::new(addr.space, addr.index, addr.disp, one.width));
        }
        for named in [&one.symbolic, &one.allocation].into_iter().flatten() {
            symbol(named);
        }
    };
    for block in &body.blocks {
        for op in &block.ops {
            for operand in op.args.iter().chain(&op.results) {
                match operand {
                    Arg::Symbol(one) => symbol(one),
                    Arg::Cell(cell) => reference(&cell.r#ref, &mut symbol),
                    _ => {}
                }
            }
            for one in op.loads.iter().chain(&op.stores) {
                reference(one, &mut symbol);
            }
            for (one, _value) in &op.memory_values {
                reference(one, &mut symbol);
            }
            if let Some(array) = &op.array {
                symbol(&array.descriptor);
            }
        }
    }
    found
}

/// Loader facts for immutable objects directly named by one body.
fn _body_initializers(body: &MirBody, initial: &[(MemRef, Const)]) -> Vec<(MemRef, Const)> {
    let segment_index = |one: &MemRef| one.addr.filter(|addr| addr.space == Space::Segment).map(|addr| addr.index);
    let candidates: BTreeSet<i64> = initial.iter().filter_map(|(one, _)| segment_index(one)).collect();
    let referenced = _referenced_data(body, &candidates);
    initial
        .iter()
        .filter(|(one, _)| segment_index(one).is_some_and(|index| referenced.contains(&index)))
        .cloned()
        .collect()
}

/// Python `_mir_text`.
pub fn _mir_text(name: &str, body: &MirBody) -> String {
    let mut out = vec![format!("== {name}")];
    for block in &body.blocks {
        out.push(format!("block {} -> {}", block.at, pyrepr::tuple(&block.succ)));
        out.extend(block.phis.iter().map(|phi| format!("  phi {}", phi.repr())));
        for op in &block.ops {
            let extra = if op.test.is_some() || op.target.is_some() {
                let test = op.test.map_or_else(|| "None".to_owned(), |one| one.to_string());
                let target = op.target.map_or_else(|| "None".to_owned(), |one| one.to_string());
                format!(" test={test} target={target}")
            } else {
                String::new()
            };
            out.push(format!(
                "  {:>4} {} {} -> {}{extra}",
                op.at,
                op.kind,
                pyrepr::tuple(&op.args),
                pyrepr::tuple(&op.results)
            ));
        }
    }
    out.join("\n") + "\n"
}

fn write(dump: Option<&Path>, stage: &str, text: &str) -> std::io::Result<()> {
    if let Some(dump) = dump {
        let path = dump.join(stage);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
    }
    Ok(())
}

struct Args {
    source: PathBuf,
    output: Option<PathBuf>,
    dump: Option<PathBuf>,
    opt: bool,
    cpu: String,
}

const USAGE: &str =
    "usage: llrm-c [-h] [-o OUTPUT] [-I INCLUDE] [--dump DUMP] [--opt] [--cpu CPU] source";

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let (mut source, mut output, mut dump, mut opt, mut cpu) =
        (None, None, None, false, "386".to_owned());
    let mut rest = argv.iter();
    while let Some(one) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or(format!("argument {name}: expected one argument"))
        };
        match one.as_str() {
            "-o" | "--output" => output = Some(PathBuf::from(value("-o/--output")?)),
            "-I" | "--include" => {
                value("-I/--include")?;
            }
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--opt" => opt = true,
            "--cpu" => cpu = value("--cpu")?,
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unrecognized arguments: {flag}"));
            }
            path if source.is_none() => source = Some(PathBuf::from(path)),
            extra => return Err(format!("unrecognized arguments: {extra}")),
        }
    }
    let source = source.ok_or("the following arguments are required: source")?;
    Ok(Args {
        source,
        output,
        dump,
        opt,
        cpu,
    })
}

/// `main`: exit status 0 on success, 1 on a refusal, 2 on bad arguments.
pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{USAGE}\nllrm-c: error: {message}");
            return 2;
        }
    };
    let result = (|| {
        if args.source.extension().and_then(|one| one.to_str()) != Some("cgs") {
            return Err(CompileError::NotPorted("qbopt.cfront.compile.recorded"));
        }
        let text = fs::read_to_string(&args.source)?;
        let _output = args
            .output
            .clone()
            .unwrap_or_else(|| args.source.with_extension("asm"));
        let module = args
            .source
            .file_stem()
            .and_then(|one| one.to_str())
            .unwrap_or_default();
        assembled(&text, module, args.opt, args.dump.as_deref(), &args.cpu)
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("llrm-c: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{CompileError, _body_initializers, _constant_initializers, assembled};
    use crate::cfront::hir;
    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
    use crate::objectfile::module::{Addr, Space};
    use crate::support::pyrepr::Tuple;

    fn segment(offset: i64, index: i64) -> Addr {
        Addr { index, ..Addr::new(Space::Segment, offset) }
    }

    fn item(call: &str, args: &[&str]) -> (String, Tuple<String>) {
        (call.to_owned(), Tuple(args.iter().map(|one| (*one).to_owned()).collect()))
    }

    fn symbol(id: i64, name: &str, attr: i64, segment: i64) -> hir::Symbol {
        hir::Symbol {
            id,
            name: name.to_owned(),
            base: name.to_owned(),
            pattern: "_*".to_owned(),
            attr,
            call_class: 0,
            call_target: 0,
            register_parms: false,
            code: None,
            segment,
        }
    }

    /// CRC's constant byte table reached MIR with no initializer facts.
    ///
    /// Numeric bytes of a private immutable object are loader-established facts;
    /// mutable, volatile, relocatable, or inline-assembly-visible objects are not.
    #[test]
    fn test_private_constant_data_seeds_complete_loader_bytes_only() {
        let mut unit = hir::Unit::default();
        unit.symbols.insert(1, symbol(1, "table", hir::FE_CONSTANT | hir::FE_INTERNAL, 1));
        unit.backs.insert(1, 1);
        unit.segments.insert(
            1,
            hir::Segment {
                id: 1,
                name: "CONST2".to_owned(),
                attr: 0,
                items: vec![
                    item("DGLabel", &["b1"]),
                    item("DGBytes", &["2", "3132"]),
                    item("DGIBytes", &["2", "255"]),
                    item("DGInteger", &["4660", "TY_UINT_2"]),
                ],
            },
        );

        let expected: Vec<(MemRef, Const)> = b"12\xff\xff\x34\x12"
            .iter()
            .enumerate()
            .map(|(offset, byte)| (MemRef::new(Some(segment(offset as i64, 1)), 1), Const::new(*byte, 1)))
            .collect();
        assert_eq!(_constant_initializers(&unit), expected);

        unit.symbols[&1].attr &= !hir::FE_CONSTANT;
        assert!(_constant_initializers(&unit).is_empty());
        unit.symbols[&1].attr |= hir::FE_CONSTANT | hir::FE_VOLATILE;
        assert!(_constant_initializers(&unit).is_empty());
        unit.symbols[&1].attr &= !hir::FE_VOLATILE;
        unit.segments[&1].items.push(item("DGFEPtr", &["y1", "TY_NEAR_POINTER", "0"]));
        assert!(_constant_initializers(&unit).is_empty());

        unit.segments[&1].items.pop();
        let mut inline = symbol(2, "inline", hir::FE_PROC, 0);
        inline.code = Some(hir::Code {
            data: b"\x90\x90".to_vec(),
            fixups: vec![hir::Fixup { at: 0, kind: "offset".to_owned(), symbol: 1, offset: 0 }],
        });
        unit.symbols.insert(2, inline);
        assert!(_constant_initializers(&unit).is_empty());
    }

    /// A large qcport lookup table must not enlarge SCCP in every procedure.
    ///
    /// Module initializer facts belong only to bodies that directly name their
    /// object; unrelated functions previously received every byte in the module.
    #[test]
    fn test_constant_loader_facts_are_local_to_referencing_bodies() {
        let first = MemRef::new(Some(segment(0, 7)), 1);
        let second = MemRef::new(Some(segment(0, 8)), 1);
        let initial = vec![(first.clone(), Const::new(1, 1)), (second, Const::new(2, 1))];
        let loaded = Value::new(1, 1);
        let load = Op {
            kind: Kind::Load,
            loads: vec![first],
            results: vec![Arg::Held(Held { value: loaded, width: 1 })],
            ..Op::new(1, OpCode::Operation(Operation::Nothing), "", vec![loaded], vec![])
        };
        let body = MirBody::new(1, vec![MirBlock::new(1, vec![], vec![load], vec![])]);

        assert_eq!(_body_initializers(&body, &initial), [initial[0].clone()]);
    }

    /// A callee taking arguments in registers: the raise pushed them anyway,
    /// and ls linked against `strlen_`, Watcom's register-convention strlen.
    #[test]
    fn test_register_convention_is_refused() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/c/regs.cgs");
        match assembled(&std::fs::read_to_string(path).unwrap(), "regs", false, None, "386") {
            Err(CompileError::Unsupported(refused)) => {
                assert!(refused.to_string().contains("_twice has a register calling convention"), "{refused}");
            }
            _ => panic!("a register convention was compiled as a stack one"),
        }
    }
}
