//! Writes the pruned table as Rust.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use llrm_dagisel::matcher::{Emit, Matcher, Slice, Vt};

use crate::features::Cpu;
use crate::flatten::Flat;
use crate::inc::{ComplexPattern, Ids};
use crate::predicate::Expr;

/// How the emitted Rust names its surroundings: the x86 tables sit inside
/// llrm-dagisel; a target's sit in its own crate.
pub struct Style {
    /// The path of llrm-dagisel (`crate` or `llrm_dagisel`).
    pub dagisel: &'static str,
    /// The module of the node-predicate functions; its last segment is
    /// `predicates`.
    pub predicates: &'static str,
    /// The instruction namespace of the pin's names (`X86::`).
    pub ns: &'static str,
    /// What the table is of, for the doc line.
    pub title: &'static str,
    /// The script that regenerates it.
    pub regen: &'static str,
    /// The instruction descriptor lookup of `memory_flags`.
    pub desc: &'static str,
    /// `instrinfo.rs` defines `desc` itself (llrm-dagisel's reads its own
    /// generated table).
    pub local_desc: bool,
    /// The dag items the table's header imports besides the matcher types.
    pub dag_imports: &'static str,
    /// The node predicates are listed by name (`NODE_PREDICATES`) instead of
    /// called: the target has no predicate hooks yet, and a pattern
    /// predicate left open is an error.
    pub listed_predicates: bool,
    /// What the `node_predicate` function asks of its hooks (`X86Predicates<D>`
    /// for the x86 tables).
    pub predicate_bound: &'static str,
}

impl Style {
    pub const X86: Style = Style {
        dagisel: "crate",
        predicates: "crate::predicates",
        ns: "X86::",
        title: "x86-32",
        regen: "tools/dagisel/regen.sh",
        desc: "crate::instr::desc",
        local_desc: false,
        dag_imports: "Dag, Hooks, PatternContext, SDValue, X86Predicates",
        listed_predicates: false,
        predicate_bound: "X86Predicates<D>",
    };
}

pub struct Source<'a> {
    pub style: &'a Style,
    pub cpu_name: &'a str,
    pub cpu: &'a Cpu,
    pub pin: &'a str,
    pub flat: &'a Flat,
    pub ids: &'a Ids,
    pub open: &'a BTreeMap<u16, Expr>,
    pub node_predicates: &'a [Vec<String>],
    pub complex_patterns: &'a [ComplexPattern],
    pub xforms: &'a [String],
    /// The names of the instructions (not registers or classes, whose numbers
    /// collide with theirs).
    pub instructions: &'a BTreeSet<String>,
    /// Qualified instruction names the hand code selects (`extra_symbols`): listed in `MACHINE_OPCODES` too.
    pub extra: &'a [String],
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
        M::CaptureDeactivationSymbol => "CaptureDeactivationSymbol".into(),
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
        M::CheckTypeRes { res, vt: v } => format!("CheckTypeRes {{ res: {res}, vt: {} }}", vt(*v)),
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

fn expr(
    e: &Expr,
    cpu: &Cpu,
) -> Result<String, String> {
    Ok(match e {
        Expr::Atom(a) => {
            let method =
                cpu.dynamic_method(a).ok_or_else(|| format!("`{a}` is open but has no PatternContext method"))?;
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
fn machine_opcodes(
    flat: &Flat,
    ids: &Ids,
    instructions: &BTreeSet<String>,
    ns: &str,
    extra: &[String],
) -> BTreeMap<u16, String> {
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
        let short = name.strip_prefix(ns).or_else(|| name.strip_prefix("TargetOpcode::"));
        if let (Some(short), true) = (short.filter(|s| instructions.contains(*s)), (wanted.contains(&(value as u16)) || extra.contains(name))) {
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
    let (style, dagisel) = (src.style, src.style.dagisel);
    writeln!(w, "//! The {} matcher table for the {} (pin {}).", style.title, src.cpu_name, &src.pin[..12]).unwrap();
    writeln!(w, "//!\n//! Generated by `{}`; do not edit.\n", style.regen).unwrap();
    writeln!(w, "#![allow(clippy::all, unused_imports)]\n").unwrap();
    writeln!(w, "use {dagisel}::dag::{{{}}};", style.dag_imports).unwrap();
    writeln!(w, "use {dagisel}::matcher::{{Emit, Matcher::*, Slice, Table, Vt}};").unwrap();
    let alias = if style.predicates.ends_with("::predicates") { "" } else { " as predicates" };
    writeln!(w, "use {}{alias};\n", style.predicates).unwrap();

    writeln!(w, "pub static ITEMS: [{dagisel}::matcher::Matcher; {}] = [", flat.items.len()).unwrap();
    for m in &flat.items {
        writeln!(w, "    {},", item(m)).unwrap();
    }
    writeln!(w, "];\n").unwrap();
    writeln!(w, "pub static OPERANDS: [u16; {}] = {:?};\n", flat.operands.len(), flat.operands).unwrap();
    writeln!(w, "pub const TABLE: Table = Table {{ items: &ITEMS, operands: &OPERANDS }};\n").unwrap();

    let used: BTreeSet<u16> =
        flat.items.iter().filter_map(|m| if let Matcher::CheckPredicate(p) = m { Some(*p) } else { None }).collect();
    if style.listed_predicates {
        if let Some((id, _)) = src.open.iter().next() {
            return Err(format!("pattern predicate {id} is open: the AArch64 table has no PatternContext yet"));
        }
        writeln!(w, "/// The node predicates the table checks: `(number, the PatFrag names that define it)`.").unwrap();
        writeln!(w, "pub const NODE_PREDICATES: &[(u16, &[&str])] = &[").unwrap();
        for id in &used {
            writeln!(w, "    ({id}, &{:?}),", src.node_predicates[usize::from(*id)]).unwrap();
        }
        writeln!(w, "];\n").unwrap();
    } else {
        writeln!(w, "/// The pattern predicates still checked when a function is selected.").unwrap();
        // With no open predicate nothing reads the context: the name says so (`-D
        // warnings` refuses `ctx` unused).
        let ctx = if src.open.is_empty() { "_ctx" } else { "ctx" };
        writeln!(w, "pub fn pattern_predicate<C: PatternContext>({ctx}: &C, id: u16) -> bool {{\n    match id {{")
            .unwrap();
        for (id, e) in src.open {
            writeln!(w, "        {id} => {},", expr(e, src.cpu)?).unwrap();
        }
        writeln!(w, "        _ => unreachable!(\"pattern predicate {{id}} is not in this table\"),\n    }}\n}}\n")
            .unwrap();

        writeln!(w, "/// The node predicates the table checks.").unwrap();
        writeln!(w, "pub fn node_predicate<D: Dag, H: {}>(dag: &D, hooks: &H, value: SDValue, id: u16) -> bool {{\n    let args = (dag, hooks, value);\n    match id {{", style.predicate_bound).unwrap();
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
        writeln!(w, "        _ => unreachable!(\"node predicate {{id}} is not in this table\"),\n    }}\n}}\n")
            .unwrap();
    }

    writeln!(w, "/// The complex patterns the table checks: `(number, selector function, results)`.").unwrap();
    writeln!(w, "pub const COMPLEX_PATTERNS: &[(u8, &str, u8)] = &[").unwrap();
    let patterns: BTreeSet<u8> = flat
        .items
        .iter()
        .filter_map(|m| if let Matcher::CheckComplexPat { pattern, .. } = m { Some(*pattern) } else { None })
        .collect();
    for p in &patterns {
        let c = &src.complex_patterns[usize::from(*p)];
        writeln!(w, "    ({p}, {:?}, {}),", c.function, c.results).unwrap();
    }
    writeln!(w, "];\n").unwrap();

    writeln!(w, "/// The node transforms the table runs: `(number, the pin's C++ call)`.").unwrap();
    writeln!(w, "pub const XFORMS: &[(u8, &str)] = &[").unwrap();
    let xforms: BTreeSet<u8> = flat
        .items
        .iter()
        .filter_map(|m| if let Matcher::EmitNodeXForm { xform, .. } = m { Some(*xform) } else { None })
        .collect();
    for x in &xforms {
        writeln!(w, "    ({x}, {:?}),", src.xforms[usize::from(*x)]).unwrap();
    }
    writeln!(w, "];\n").unwrap();

    writeln!(w, "/// The machine opcodes the table emits, with the pin's names (`{}` and `TargetOpcode::`).", style.ns)
        .unwrap();
    writeln!(w, "pub const MACHINE_OPCODES: &[(u16, &str)] = &[").unwrap();
    for (value, name) in machine_opcodes(flat, src.ids, src.instructions, style.ns, src.extra) {
        writeln!(w, "    ({value}, {name:?}),").unwrap();
    }
    writeln!(w, "];").unwrap();
    Ok(out)
}

/// The `MCID` flags in the pin's order; a flag's bit is its position.
const MCID_FLAGS: &[&str] = &[
    "Variadic",
    "Pseudo",
    "Meta",
    "Return",
    "EHScopeReturn",
    "Call",
    "Barrier",
    "Terminator",
    "Branch",
    "IndirectBranch",
    "Compare",
    "MoveImm",
    "MoveReg",
    "Bitcast",
    "Select",
    "DelaySlot",
    "FoldableAsLoad",
    "MayLoad",
    "MayStore",
    "MayRaiseFPException",
    "Predicable",
    "NotDuplicable",
    "UnmodeledSideEffects",
    "Commutable",
    "ConvertibleTo3Addr",
    "UsesCustomInserter",
    "HasPostISelHook",
    "Rematerializable",
    "CheapAsAMove",
    "ExtraSrcRegAllocReq",
    "ExtraDefRegAllocReq",
    "RegSequence",
    "ExtractSubreg",
    "InsertSubreg",
    "Convergent",
    "Add",
    "Trap",
    "SoftFail",
    "Authenticated",
    "PreISelOpcode",
];

fn const_name(flag: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = flag.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
        let prev_lower = i > 0 && chars[i - 1].is_lowercase();
        if i > 0 && c.is_uppercase() && (prev_lower || next_lower) {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// `generated/instrinfo.rs`: the `MCInstrDesc` of each emitted opcode, from the
/// lines `tools/dagisel/instrinfo.py` writes.
pub fn render_instr_info(
    lines: &str,
    ids: &Ids,
    emitted: &BTreeSet<u16>,
    style: &Style,
) -> Result<String, String> {
    let number = |name: &str| -> Result<i64, String> { if let Ok(n) = name.parse() { Ok(n) } else { ids.get(name) } };
    let regs = |list: &str| -> Result<Vec<i64>, String> {
        if list == "-" {
            return Ok(Vec::new());
        }
        list.split(',').map(&number).collect()
    };
    let mut out = String::from("//! The pin's `MCInstrDesc` of the machine opcodes the tables emit.\n//!\n");
    writeln!(out, "//! Generated by `{}`; do not edit.\n", style.regen).unwrap();
    writeln!(out, "use {}::instr::{{InstrDesc, OperandDesc}};\n", style.dagisel).unwrap();
    writeln!(out, "/// `MCID::*` flags: bit `n` is the pin's `n`th flag.\npub mod flag {{").unwrap();
    for (bit, name) in MCID_FLAGS.iter().enumerate() {
        writeln!(out, "    pub const {}: u64 = 1 << {bit};", const_name(name)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    let mut rows = BTreeMap::new();
    for line in lines.lines() {
        let mut parts = line.split(' ');
        let mut next = || parts.next().ok_or_else(|| format!("short instrinfo line `{line}`"));
        let name = next()?;
        let Some(&opcode) =
            ids.0.get(&format!("{}{name}", style.ns)).or_else(|| ids.0.get(&format!("TargetOpcode::{name}")))
        else {
            continue;
        };
        if !emitted.contains(&(opcode as u16)) {
            continue;
        }
        let opcode = opcode as u32;
        let (num_ops, num_defs, size) = (next()?, next()?, next()?);
        let flags = next()?;
        let (uses, defs) = (regs(next()?)?, regs(next()?)?);
        let mut mask = 0u64;
        if flags != "-" {
            for flag in flags.split(',') {
                let bit =
                    MCID_FLAGS.iter().position(|f| *f == flag).ok_or_else(|| format!("unknown MCID flag {flag}"))?;
                mask |= 1 << bit;
            }
        }
        let mut operands = String::new();
        for op in parts.filter(|p| !p.is_empty()) {
            let mut f = op.splitn(3, '|');
            let (class, kind, tied) =
                (f.next().unwrap(), f.next().ok_or("bad operand")?, f.next().ok_or("bad operand")?);
            let (tied, letters) = tied.split_once('/').unwrap_or((tied, ""));
            let tied_to = match tied {
                "-" | "E" => -1,
                n => n.parse::<i64>().map_err(|_| format!("bad tie {tied}"))?,
            };
            write!(
                operands,
                "OperandDesc {{ reg_class: {}, kind: {}, tied_to: {tied_to}, early_clobber: {}, pointer_class: {}, branch_target: {} }}, ",
                number(class)?,
                number(kind)?,
                tied == "E",
                letters.contains('L'),
                letters.contains('B'),
            )
            .unwrap();
        }
        let list = |v: &[i64]| v.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ");
        rows.insert(
            opcode as u16,
            format!(
                "InstrDesc {{ num_operands: {num_ops}, num_defs: {num_defs}, size: {size}, flags: {mask}, implicit_uses: &[{}], implicit_defs: &[{}], operands: &[{}] }}",
                list(&uses),
                list(&defs),
                operands.trim_end_matches(", ")
            ),
        );
    }
    writeln!(out, "/// `(opcode, descriptor)`, sorted by opcode.").unwrap();
    writeln!(out, "pub static INSTRS: [(u16, InstrDesc); {}] = [", rows.len()).unwrap();
    for (opcode, row) in &rows {
        writeln!(out, "    ({opcode}, {row}),").unwrap();
    }
    writeln!(out, "];\n").unwrap();
    if style.local_desc {
        writeln!(out, "/// The descriptor of a machine opcode the matcher tables emit.").unwrap();
        writeln!(out, "pub fn desc(opcode: u16) -> Option<&'static InstrDesc> {{").unwrap();
        writeln!(out, "    INSTRS.binary_search_by_key(&opcode, |&(o, _)| o).ok().map(|at| &INSTRS[at].1)").unwrap();
        writeln!(out, "}}\n").unwrap();
    }
    writeln!(out, "/// `(mayLoad, mayStore)` of a machine opcode.").unwrap();
    writeln!(out, "pub fn memory_flags(opcode: u16) -> (bool, bool) {{").unwrap();
    writeln!(out, "    {}(opcode).map_or((false, false), |d| (d.may_load(), d.may_store()))", style.desc).unwrap();
    writeln!(out, "}}").unwrap();
    Ok(out)
}
