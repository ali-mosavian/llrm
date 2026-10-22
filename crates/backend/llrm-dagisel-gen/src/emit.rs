//! Writes the pruned table as Rust.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use llrm_dagisel::matcher::{Emit, Matcher, Slice, Vt};

use crate::features::Cpu;
use crate::flatten::Flat;
use crate::inc::{ComplexPattern, Ids};
use crate::predicate::Expr;

pub struct Source<'a> {
    pub cpu_name: &'a str,
    pub cpu: &'a Cpu,
    pub pin: &'a str,
    pub flat: &'a Flat,
    pub ids: &'a Ids,
    pub open: &'a BTreeMap<u16, Expr>,
    pub node_predicates: &'a [Vec<String>],
    pub complex_patterns: &'a [ComplexPattern],
    pub xforms: &'a [String],
}

fn vt(v: Vt) -> String {
    format!("Vt({})", v.0)
}

fn slice(s: Slice) -> String {
    format!("Slice {{ start: {}, len: {} }}", s.start, s.len)
}

fn emit_struct(e: &Emit) -> String {
    format!(
        "Emit {{ opcode: {}, flags: {}, vts: [{}, {}], num_vts: {}, operands: {} }}",
        e.opcode,
        e.flags,
        vt(e.vts[0]),
        vt(e.vts[1]),
        e.num_vts,
        slice(e.operands)
    )
}

fn item(m: &Matcher) -> String {
    use Matcher as M;
    match m {
        M::Scope => "Scope".into(),
        M::Alt(n) => format!("Alt({n})"),
        M::RecordNode => "RecordNode".into(),
        M::RecordChild(c) => format!("RecordChild({c})"),
        M::RecordMemRef => "RecordMemRef".into(),
        M::CaptureGlueInput => "CaptureGlueInput".into(),
        M::MoveChild(c) => format!("MoveChild({c})"),
        M::MoveSibling(c) => format!("MoveSibling({c})"),
        M::MoveParent => "MoveParent".into(),
        M::CheckSame(r) => format!("CheckSame({r})"),
        M::CheckChildSame { child, record } => format!("CheckChildSame {{ child: {child}, record: {record} }}"),
        M::CheckPatternPredicate(p) => format!("CheckPatternPredicate({p})"),
        M::CheckPredicate(p) => format!("CheckPredicate({p})"),
        M::CheckComplexPat { pattern, record } => format!("CheckComplexPat {{ pattern: {pattern}, record: {record} }}"),
        M::CheckOpcode(o) => format!("CheckOpcode({o})"),
        M::SwitchOpcode => "SwitchOpcode".into(),
        M::OpcodeCase { size, opcode } => format!("OpcodeCase {{ size: {size}, opcode: {opcode} }}"),
        M::CheckType(v) => format!("CheckType({})", vt(*v)),
        M::SwitchType => "SwitchType".into(),
        M::TypeCase { size, vt: v } => format!("TypeCase {{ size: {size}, vt: {} }}", vt(*v)),
        M::CheckChildType { child, vt: v } => format!("CheckChildType {{ child: {child}, vt: {} }}", vt(*v)),
        M::CheckInteger(i) => format!("CheckInteger({i})"),
        M::CheckChildInteger { child, value } => format!("CheckChildInteger {{ child: {child}, value: {value} }}"),
        M::CheckCondCode(c) => format!("CheckCondCode({c})"),
        M::CheckChild2CondCode(c) => format!("CheckChild2CondCode({c})"),
        M::CheckValueType(v) => format!("CheckValueType({})", vt(*v)),
        M::CheckAndImm(i) => format!("CheckAndImm({i})"),
        M::CheckOrImm(i) => format!("CheckOrImm({i})"),
        M::CheckImmAllOnesV => "CheckImmAllOnesV".into(),
        M::CheckImmAllZerosV => "CheckImmAllZerosV".into(),
        M::CheckUndef => "CheckUndef".into(),
        M::CheckFoldableChainNode => "CheckFoldableChainNode".into(),
        M::EmitInteger { vt: v, value } => format!("EmitInteger {{ vt: {}, value: {value} }}", vt(*v)),
        M::EmitRegister { vt: v, register } => format!("EmitRegister {{ vt: {}, register: {register} }}", vt(*v)),
        M::EmitConvertToTarget(r) => format!("EmitConvertToTarget({r})"),
        M::EmitMergeInputChains(s) => format!("EmitMergeInputChains({})", slice(*s)),
        M::EmitCopyToReg { record, register } => format!("EmitCopyToReg {{ record: {record}, register: {register} }}"),
        M::EmitNodeXForm { xform, record } => format!("EmitNodeXForm {{ xform: {xform}, record: {record} }}"),
        M::EmitNode(e) => format!("EmitNode({})", emit_struct(e)),
        M::MorphNodeTo(e) => format!("MorphNodeTo({})", emit_struct(e)),
        M::CompleteMatch(s) => format!("CompleteMatch({})", slice(*s)),
    }
}

fn expr(e: &Expr, cpu: &Cpu) -> Result<String, String> {
    Ok(match e {
        Expr::Atom(a) => {
            let method = cpu.dynamic_method(a).ok_or_else(|| format!("`{a}` is open but has no PatternContext method"))?;
            format!("ctx.{method}()")
        }
        Expr::Not(e) => format!("!{}", expr(e, cpu)?),
        Expr::And(parts) => {
            let parts: Result<Vec<_>, _> = parts.iter().map(|p| expr(p, cpu).map(|s| format!("({s})"))).collect();
            parts?.join(" && ")
        }
        Expr::Or(parts) => {
            let parts: Result<Vec<_>, _> = parts.iter().map(|p| expr(p, cpu).map(|s| format!("({s})"))).collect();
            parts?.join(" || ")
        }
    })
}

/// The machine opcodes the table emits, by number, with the pin's names.
fn machine_opcodes(flat: &Flat, ids: &Ids) -> BTreeMap<u16, String> {
    let wanted: BTreeSet<u16> = flat
        .items
        .iter()
        .filter_map(|m| match m {
            Matcher::EmitNode(e) | Matcher::MorphNodeTo(e) => Some(e.opcode),
            _ => None,
        })
        .collect();
    let mut by_value: BTreeMap<u16, Vec<&str>> = BTreeMap::new();
    for (name, &value) in &ids.0 {
        let short = name.strip_prefix("X86::").or_else(|| name.strip_prefix("TargetOpcode::"));
        if let (Some(short), true) = (short, wanted.contains(&(value as u16))) {
            by_value.entry(value as u16).or_default().push(short);
        }
    }
    by_value
        .into_iter()
        .map(|(v, mut names)| {
            names.sort_unstable();
            (v, names.join("|"))
        })
        .collect()
}

pub fn render(src: &Source) -> Result<String, String> {
    let mut out = String::new();
    let w = &mut out;
    let flat = src.flat;
    writeln!(w, "//! The x86-32 matcher table for the {} (pin {}).", src.cpu_name, &src.pin[..12]).unwrap();
    writeln!(w, "//!\n//! Generated by `tools/dagisel/regen.sh`; do not edit.\n").unwrap();
    writeln!(w, "#![allow(clippy::all, unused_imports)]\n").unwrap();
    writeln!(w, "use crate::dag::{{Dag, Hooks, PatternContext, SDValue}};").unwrap();
    writeln!(w, "use crate::matcher::{{Emit, Matcher::*, Slice, Table, Vt}};").unwrap();
    writeln!(w, "use crate::predicates;\n").unwrap();

    writeln!(w, "pub static ITEMS: [crate::matcher::Matcher; {}] = [", flat.items.len()).unwrap();
    for m in &flat.items {
        writeln!(w, "    {},", item(m)).unwrap();
    }
    writeln!(w, "];\n").unwrap();
    writeln!(w, "pub static OPERANDS: [u16; {}] = {:?};\n", flat.operands.len(), flat.operands).unwrap();
    writeln!(w, "pub const TABLE: Table = Table {{ items: &ITEMS, operands: &OPERANDS }};\n").unwrap();

    writeln!(w, "/// The pattern predicates still checked when a function is selected.").unwrap();
    writeln!(w, "pub fn pattern_predicate<C: PatternContext>(ctx: &C, id: u16) -> bool {{\n    match id {{").unwrap();
    for (id, e) in src.open {
        writeln!(w, "        {id} => {},", expr(e, src.cpu)?).unwrap();
    }
    writeln!(w, "        _ => unreachable!(\"pattern predicate {{id}} is not in this table\"),\n    }}\n}}\n").unwrap();

    let used: BTreeSet<u16> = flat
        .items
        .iter()
        .filter_map(|m| if let Matcher::CheckPredicate(p) = m { Some(*p) } else { None })
        .collect();
    writeln!(w, "/// The node predicates the table checks.").unwrap();
    writeln!(w, "pub fn node_predicate<D: Dag, H: Hooks<D>>(dag: &D, hooks: &H, value: SDValue, id: u16) -> bool {{\n    let args = (dag, hooks, value);\n    match id {{").unwrap();
    let mut unmapped = Vec::new();
    for id in &used {
        let names = &src.node_predicates[usize::from(*id)];
        match names.iter().find_map(|n| src.cpu.node_predicate_fn(n)) {
            Some(function) => writeln!(w, "        {id} => predicates::{function}(args),").unwrap(),
            None => unmapped.push(format!("{id}: {names:?}")),
        }
    }
    if !unmapped.is_empty() {
        return Err(format!("node predicates with no [node_predicate] entry:\n  {}", unmapped.join("\n  ")));
    }
    writeln!(w, "        _ => unreachable!(\"node predicate {{id}} is not in this table\"),\n    }}\n}}\n").unwrap();

    writeln!(w, "/// The complex patterns the table checks: `(number, selector function, results)`.").unwrap();
    writeln!(w, "pub const COMPLEX_PATTERNS: &[(u8, &str, u8)] = &[").unwrap();
    let patterns: BTreeSet<u8> =
        flat.items.iter().filter_map(|m| if let Matcher::CheckComplexPat { pattern, .. } = m { Some(*pattern) } else { None }).collect();
    for p in &patterns {
        let c = &src.complex_patterns[usize::from(*p)];
        writeln!(w, "    ({p}, {:?}, {}),", c.function, c.results).unwrap();
    }
    writeln!(w, "];\n").unwrap();

    writeln!(w, "/// The node transforms the table runs: `(number, the pin's C++ call)`.").unwrap();
    writeln!(w, "pub const XFORMS: &[(u8, &str)] = &[").unwrap();
    let xforms: BTreeSet<u8> =
        flat.items.iter().filter_map(|m| if let Matcher::EmitNodeXForm { xform, .. } = m { Some(*xform) } else { None }).collect();
    for x in &xforms {
        writeln!(w, "    ({x}, {:?}),", src.xforms[usize::from(*x)]).unwrap();
    }
    writeln!(w, "];\n").unwrap();

    writeln!(w, "/// The machine opcodes the table emits, with the pin's names (`X86::` and `TargetOpcode::`).").unwrap();
    writeln!(w, "pub const MACHINE_OPCODES: &[(u16, &str)] = &[").unwrap();
    for (value, name) in machine_opcodes(flat, src.ids) {
        writeln!(w, "    ({value}, {name:?}),").unwrap();
    }
    writeln!(w, "];").unwrap();
    Ok(out)
}

/// `generated/memflags.rs`: `MCInstrDesc::mayLoad` and `mayStore` of each emitted opcode.
/// `flags` has a `opcode mayLoad mayStore` line per instruction that has either.
pub fn render_memory_flags(flags: &str, emitted: &BTreeSet<u16>) -> Result<String, String> {
    let mut rows = BTreeMap::new();
    for line in flags.lines() {
        let mut parts = line.split(' ').map(|p| p.parse::<u32>());
        let (Some(Ok(opcode)), Some(Ok(load)), Some(Ok(store))) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("bad memory flags line `{line}`"));
        };
        if emitted.contains(&(opcode as u16)) {
            rows.insert(opcode as u16, (load | store << 1) as u8);
        }
    }
    let mut out = String::from(
        "//! `mayLoad` and `mayStore` of the machine opcodes the tables emit, from the pin's `MCInstrDesc`.\n//!\n//! Generated by `tools/dagisel/regen.sh`; do not edit.\n\n",
    );
    writeln!(out, "/// `(opcode, mayLoad | mayStore << 1)`, sorted by opcode; an opcode with neither is absent.").unwrap();
    writeln!(out, "pub static MEMORY_FLAGS: [(u16, u8); {}] = [", rows.len()).unwrap();
    for (opcode, bits) in &rows {
        writeln!(out, "    ({opcode}, {bits}),").unwrap();
    }
    writeln!(out, "];\n").unwrap();
    writeln!(out, "/// `(mayLoad, mayStore)` of a machine opcode.").unwrap();
    writeln!(out, "pub fn memory_flags(opcode: u16) -> (bool, bool) {{").unwrap();
    writeln!(out, "    match MEMORY_FLAGS.binary_search_by_key(&opcode, |&(o, _)| o) {{").unwrap();
    writeln!(out, "        Ok(at) => (MEMORY_FLAGS[at].1 & 1 != 0, MEMORY_FLAGS[at].1 & 2 != 0),").unwrap();
    writeln!(out, "        Err(_) => (false, false),").unwrap();
    writeln!(out, "    }}\n}}").unwrap();
    Ok(out)
}
