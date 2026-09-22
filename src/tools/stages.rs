//! Port of `tools/stages.py`: every pass's effect on one object, as text.
//!
//! One file per stage, `s<N>-<form>-<stage>.txt`. The Python tool prints and
//! redirects stdout; here each stage is written to a `String`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::analysis::{floatfacts, frameescape, loops};
use crate::frontend::fpstack;
use crate::model::ir::{self, Loc};
use crate::model::lir::LirBody;
use crate::model::mir::{self, Arg, Cell, Kind, MemRef, MirBody, Op};
use crate::objectfile::cvinfo::{DebugInfo, Local, Procedure};
use crate::objectfile::module::{Addr, Module, Space};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

/// Python `f"{value:#x}"` for any sign.
fn hex(value: i64) -> String {
    if value < 0 { format!("-{:#x}", value.unsigned_abs()) } else { format!("{value:#x}") }
}

/// Python `f"{value:#06x}"`.
fn hex6(value: i64) -> String {
    if value < 0 { format!("-{:#05x}", value.unsigned_abs()) } else { format!("{value:#06x}") }
}

/// Python `f"{value:+#x}"`.
fn signed_hex(value: i64) -> String {
    if value < 0 { format!("-{:#x}", value.unsigned_abs()) } else { format!("+{value:#x}") }
}

/// Python `_shape`: blocks, successors and op counts on one line, with the loop count.
fn _shape(body: &MirBody) -> String {
    let found = loops::loops(&body.blocks, Some(body.entry));
    let drawn: Vec<String> = body
        .blocks
        .iter()
        .map(|one| {
            let succ: Vec<String> = one.succ.iter().map(|&s| hex(s)).collect();
            let succ = if succ.is_empty() { "-".to_owned() } else { succ.join(",") };
            format!("{}->{}[{}]", hex(one.at), succ, one.ops.len())
        })
        .collect();
    format!("{} loop(s) | {}", found.len(), drawn.join(" "))
}

/// Python `_ops`: per block, each op's address and name.
pub type Shapes = IndexMap<i64, Vec<(i64, String)>>;

fn _ops(body: &MirBody) -> Shapes {
    body.blocks
        .iter()
        .map(|one| {
            (
                one.at,
                one.ops
                    .iter()
                    .map(|op| (op.at, if op.name.is_empty() { "?".to_owned() } else { op.name.clone() }))
                    .collect(),
            )
        })
        .collect()
}

/// Python `_report`: one stage, the shape of each body now against what it was.
fn _report(out: &mut String, tag: &str, bodies: &[(String, Rc<MirBody>)], was: Option<&IndexMap<String, Shapes>>, verbose: bool) -> IndexMap<String, Shapes> {
    let _ = writeln!(out, "\n=== {tag}");
    let mut now: IndexMap<String, Shapes> = IndexMap::default();
    for (name, body) in bodies {
        let _ = writeln!(out, "  {name}: {}", _shape(body));
        now.insert(name.clone(), _ops(body));
        let Some(was) = was.filter(|_| verbose) else {
            continue;
        };
        let empty = Shapes::default();
        let before = was.get(name).unwrap_or(&empty);
        let current = &now[name];
        let before_at: BTreeSet<i64> = before.values().flatten().map(|(at, _)| *at).collect();
        let now_at: BTreeSet<i64> = current.values().flatten().map(|(at, _)| *at).collect();
        let gone: Vec<i64> = before_at.difference(&now_at).copied().collect();
        for (at, ops) in current {
            let prior = before.get(at);
            let came: Vec<String> = ops
                .iter()
                .filter(|(op, _)| !prior.is_some_and(|prior| prior.iter().any(|(b, _)| b == op)))
                .map(|(op, n)| format!("{} {n}", hex(*op)))
                .collect();
            if !came.is_empty() {
                let _ = writeln!(out, "      into {}: {}", hex(*at), came.join(", "));
            }
        }
        if !gone.is_empty() {
            let _ = writeln!(out, "      dropped: {}", gone.iter().map(|&one| hex(one)).collect::<Vec<_>>().join(", "));
        }
    }
    now
}

/// Python `_SIZE`: how wide each of BASIC's own types is.
fn _size(type_name: &str) -> i64 {
    match type_name {
        "INTEGER" => 2,
        "LONG" | "SINGLE" | "STRING" => 4,
        "DOUBLE" => 8,
        _ => 0,
    }
}

/// Python `_part`: what is read out of a variable, where it is not the whole of it.
fn _part(into: i64, size: i64, width: i64) -> String {
    if size == 0 || width >= size {
        return String::new();
    }
    if width * 2 == size {
        return if into == 0 {
            ".lo".to_owned()
        } else if into == width {
            ".hi".to_owned()
        } else {
            format!("+{into}")
        };
    }
    if into != 0 { format!("+{into}:{width}") } else { format!(":{width}") }
}

/// Python `Cells`: what each cell one body touches is called, and the legend.
pub struct Cells {
    named: IndexMap<(String, u32), String>,
    order: Vec<(String, Option<Addr>, u32)>,
    symbols: Vec<((i64, i64), String, i64)>,
    slots: IndexMap<i64, (String, String)>,
}

impl Cells {
    pub fn new(debug: Option<&DebugInfo>) -> Self {
        let mut symbols: Vec<((i64, i64), String, i64)> = debug
            .map(|debug| {
                debug
                    .variables
                    .iter()
                    .map(|one| {
                        let stride = if one.stride != 0 { one.stride } else { _size(one.type_name().as_deref().unwrap_or("")) };
                        (one.data.unwrap_or((one.segment, one.offset)), one.name.clone(), stride)
                    })
                    .collect()
            })
            .unwrap_or_default();
        symbols.sort();
        let mut slots = IndexMap::default();
        for proc in debug.map(|debug| debug.procedures.as_slice()).unwrap_or_default() {
            for one in &proc.locals {
                slots.insert(one.bp_offset, (one.name.clone(), one.type_name().unwrap_or_default()));
            }
        }
        Self { named: IndexMap::default(), order: Vec::new(), symbols, slots }
    }

    pub fn of(&mut self, reference: &MemRef) -> String {
        let Some(addr) = reference.addr else {
            return "[?]".to_owned();
        };
        let index = reference.base.map(|base| format!("[{base}]")).unwrap_or_default();
        match addr.space {
            Space::Frame => {
                let disp = addr.disp;
                if let Some(said) = self.slots.get(&disp) {
                    return format!("{}{index}", said.0);
                }
                let slot = if disp < 0 { format!("L{:x}", -disp) } else { format!("P{disp:x}") };
                return format!("{slot}{index}");
            }
            Space::Stack => return format!("push{:+}{index}", addr.disp),
            _ => {}
        }
        if let Some((name, into, size)) = self._inside(addr.index, addr.disp) {
            return format!("{name}{index}{}", _part(into, size, i64::from(reference.width)));
        }
        let key = (addr.repr(), reference.width);
        if !self.named.contains_key(&key) {
            let number = self.order.len();
            let letter = char::from(b'A' + (number % 26) as u8);
            let name: String = std::iter::repeat_n(letter, 1 + number / 26).collect();
            self.named.insert(key.clone(), name.clone());
            self.order.push((name, Some(addr), reference.width));
        }
        format!("{}{index}:{}", self.named[&key], reference.width)
    }

    /// `bisect.bisect_right(self.symbols, ((segment, disp), "\xff", 0)) - 1`.
    fn _inside(&self, segment: i64, disp: i64) -> Option<(String, i64, i64)> {
        let key = (segment, disp);
        let at = self
            .symbols
            .partition_point(|(where_, name, _)| (*where_, name.as_str()) <= (key, "\u{ff}"));
        let at = at.checked_sub(1)?;
        let ((where_, start), name, size) = &self.symbols[at];
        if *where_ != segment || disp < *start || disp - start > 0x100 {
            return None;
        }
        Some((name.clone(), disp - start, *size))
    }

    pub fn legend(&self) -> Vec<String> {
        self.order
            .iter()
            .map(|(name, addr, width)| {
                let addr = addr.map_or_else(|| "None".to_owned(), |addr| addr.repr());
                format!("      {name:4} {addr} :{width}")
            })
            .collect()
    }
}

/// Python `_short`: one MIR operand.
fn _short(one: &Arg, cells: &mut Cells) -> String {
    match one {
        Arg::Held(held) => held.value.to_string(),
        Arg::Const(value) => value.n.to_string(),
        Arg::Symbol(symbol) => {
            format!("&{}:{}+{}", symbol.space.value(), symbol.index, hex(symbol.offset + symbol.addend))
        }
        Arg::FrameAddress(frame) => format!("frame({})", signed_hex(frame.offset)),
        Arg::Cell(cell) => cells.of(&cell.r#ref),
        Arg::Opaque(opaque) if !opaque.name.is_empty() => opaque.name.clone(),
        _ => "?".to_owned(),
    }
}

/// Python `_signature`: what a procedure takes and keeps, where the object says so.
fn _signature(proc: Option<&Procedure>) -> Vec<String> {
    let Some(proc) = proc else {
        return Vec::new();
    };
    let params: Vec<&Local> = proc.locals.iter().filter(|one| one.bp_offset > 0).collect();
    let keeps: Vec<&Local> = proc.locals.iter().filter(|one| one.bp_offset < 0).collect();
    let params = params.iter().map(|one| _declared(one)).collect::<Vec<_>>().join(", ");
    let mut out = vec![format!("      {} ({})", proc.name, if params.is_empty() { "-" } else { &params })];
    if !keeps.is_empty() {
        out.push(format!("      locals  {}", keeps.iter().map(|one| _declared(one)).collect::<Vec<_>>().join(", ")));
    }
    out
}

fn _declared(one: &Local) -> String {
    format!("{} {} at bp{:+}", one.name, one.type_name().unwrap_or_else(|| "?".to_owned()), one.bp_offset)
}

/// Python `_depth`: how deeply nested each block is.
fn _depth(body: &MirBody) -> IndexMap<i64, usize> {
    let mut out: IndexMap<i64, usize> = IndexMap::default();
    for found in loops::loops(&body.blocks, Some(body.entry)) {
        for at in &found.body {
            *out.entry(*at).or_insert(0) += 1;
        }
    }
    out
}

fn symbol_of(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Add => "+",
        Kind::Sub => "-",
        Kind::Mul => "*",
        Kind::Div => "/",
        Kind::Rem => "%",
        Kind::And => "&",
        Kind::Or => "|",
        Kind::Xor => "^",
        Kind::Shl => "<<",
        Kind::Shr => ">>",
        Kind::Sar => ">>>",
        _ => return None,
    })
}

fn prefix_of(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Neg => "-",
        Kind::Not => "~",
        Kind::Address => "&",
        _ => return None,
    })
}

/// Python `_says`: one operation, in three-address form.
fn _says(op: &Op, cells: &mut Cells, calls: &IndexMap<i64, String>, verbose: bool) -> String {
    let args: Vec<String> = op.args.iter().map(|one| _short(one, cells)).collect();
    let into = op.results.iter().map(|one| _short(one, cells)).collect::<Vec<_>>().join(", ");

    let mut notes: Vec<String> = Vec::new();
    let mut seen: Vec<&MemRef> = Vec::new();
    for reference in op.loads.iter().chain(&op.stores) {
        if !seen.contains(&reference) {
            seen.push(reference);
        }
    }
    for reference in seen {
        if let Some(allocation) = reference.allocation {
            let cell = _short(&Arg::Cell(Cell { r#ref: reference.clone() }), cells);
            let of = _short(&Arg::Symbol(allocation), cells);
            notes.push(format!("in bounds {cell} of {of}"));
        }
    }
    if !op.memory_values.is_empty() {
        let values: Vec<String> = op
            .memory_values
            .iter()
            .map(|(reference, value)| {
                format!(
                    "{}={}",
                    _short(&Arg::Cell(Cell { r#ref: reference.clone() }), cells),
                    _short(&Arg::Const(value.clone()), cells)
                )
            })
            .collect();
        notes.push(format!("on return {}", values.join(", ")));
    }
    if let Some(request) = &op.array {
        let bounds = request.bounds.iter().map(|(low, high)| format!("{low}..{high}")).collect::<Vec<_>>().join(", ");
        let action = if request.replaces { "replace array" } else { "allocate array" };
        notes.push(format!(
            "request {action} {} ({bounds}), element {}",
            _short(&Arg::Symbol(request.descriptor), cells),
            request.element_width
        ));
    }
    if verbose {
        let flags: Vec<String> = op.uses.iter().filter(|one| one.flags).map(ToString::to_string).collect();
        if !flags.is_empty() {
            notes.push(format!("with {}", flags.join(", ")));
        }
        if !op.merges.is_empty() {
            notes.push(format!("keeps {}", op.merges.keys().map(ToString::to_string).collect::<Vec<_>>().join(", ")));
        }
    }
    let said = if notes.is_empty() { String::new() } else { format!("    ; {}", notes.join("; ")) };

    match op.kind {
        Kind::Jump => return op.target.map_or_else(|| "goto ?".to_owned(), |target| format!("goto {}", hex(target))),
        Kind::Branch => {
            let asked = op.test.map_or_else(|| "?".to_owned(), |test| test.name().to_lowercase());
            let where_ = op.target.map(|target| format!(" goto {}", hex(target))).unwrap_or_default();
            let reads = op.uses.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
            let reads = if reads.is_empty() { "?".to_owned() } else { reads };
            return format!("if {reads} {asked}{where_}{said}");
        }
        Kind::Call => {
            let made = op.defines.iter().filter(|one| !one.flags).map(ToString::to_string).collect::<Vec<_>>().join(", ");
            let who = calls.get(&op.at).cloned().unwrap_or_default();
            let head = if made.is_empty() { String::new() } else { format!("{made} := ") };
            return format!("{head}call {who}").trim_end().to_owned() + &said;
        }
        Kind::Arg => return format!("arg {}{said}", args.first().map_or("?", String::as_str)),
        _ => {}
    }
    if matches!(op.kind, Kind::Copy | Kind::Load | Kind::Store) && !into.is_empty() && args.len() == 1 {
        return format!("{into} := {}{said}", args[0]);
    }
    let head = if into.is_empty() { String::new() } else { format!("{into} := ") };
    if let Some(symbol) = symbol_of(op.kind).filter(|_| args.len() == 2) {
        return format!("{head}{} {symbol} {}{said}", args[0], args[1]);
    }
    if let Some(prefix) = prefix_of(op.kind).filter(|_| args.len() == 1) {
        return format!("{head}{prefix}{}{said}", args[0]);
    }
    let kind = op.kind.name().to_lowercase();
    if into.is_empty() {
        return format!("{kind} {}", args.join(", ")).trim_end().to_owned() + &said;
    }
    format!("{into} := {kind} {}", args.join(", ")).trim_end().to_owned() + &said
}

/// Python `_mir`: what each pass decided, as `c := a op b` and nothing else.
fn _mir(out: &mut String, bodies: &[(String, Rc<MirBody>)], found: Option<&Module>, verbose: bool, debug: Option<&DebugInfo>) {
    let _ = writeln!(out, "  --- mir");
    let empty = IndexMap::default();
    let calls = found.map_or(&empty, |found| &found.calls);
    let mut procs: IndexMap<String, &Procedure> = IndexMap::default();
    for one in debug.map(|debug| debug.procedures.as_slice()).unwrap_or_default() {
        procs.insert(one.name.trim_end_matches(['&', '%', '!', '#', '$']).to_owned(), one);
    }
    for (name, body) in bodies {
        let _ = writeln!(out, "  {name}");
        let escapes = frameescape::analysed(body);
        if !escapes.origins.is_empty() || !escapes.opaque_addresses.is_empty() {
            let exposed: BTreeSet<i64> = escapes.exposed.iter().copied().collect();
            let exposed = exposed.iter().map(|&one| signed_hex(one)).collect::<Vec<_>>().join(", ");
            let exposed = if exposed.is_empty() { "none observed".to_owned() } else { exposed };
            let opaque: BTreeSet<i64> = escapes.opaque_addresses.iter().copied().collect();
            let opaque = opaque.iter().map(|&at| hex(at)).collect::<Vec<_>>().join(", ");
            let opaque = if opaque.is_empty() { "none".to_owned() } else { opaque };
            let _ = writeln!(out, "  frame addresses exposed: {exposed}; opaque address sites: {opaque}");
        }
        let last = name.split_whitespace().last().unwrap_or("");
        for line in _signature(procs.get(last).copied()) {
            let _ = writeln!(out, "{line}");
        }
        let mut cells = Cells::new(debug);
        let depth = _depth(body);
        let floats = fpstack::readings(body);
        let exact = match found {
            Some(found) => floatfacts::known(body, &found.dgroup.members, calls, None),
            None => IndexMap::default(),
        };
        for block in &body.blocks {
            let pad = "  ".repeat(depth.get(&block.at).copied().unwrap_or(0));
            let succ = block.succ.iter().map(|&one| hex(one)).collect::<Vec<_>>().join(", ");
            let succ = if succ.is_empty() { "-".to_owned() } else { succ };
            let _ = writeln!(out, "\n    {}  {pad}-> {succ}", hex6(block.at));
            for phi in &block.phis {
                let mut incoming: Vec<(i64, mir::Value)> = phi.incoming.iter().map(|(&at, &value)| (at, value)).collect();
                incoming.sort();
                let came = incoming.iter().map(|(at, value)| format!("{}:{value}", hex(*at))).collect::<Vec<_>>().join(", ");
                let _ = writeln!(out, "    {:6}  {pad}{} := phi {came}", "", phi.result);
            }
            for op in &block.ops {
                let flow = if op.stack.is_some() && op.floating_origin.is_none() { floats.get(&op.at) } else { None };
                let mut values = String::new();
                if let Some(flow) = flow.filter(|flow| !flow.uses.is_empty() || flow.defines.is_some()) {
                    let uses = flow.uses.values().map(ToString::to_string).collect::<Vec<_>>().join(", ");
                    let uses = if uses.is_empty() { "-".to_owned() } else { uses };
                    let defines = flow.defines.as_ref().map_or_else(|| "-".to_owned(), ToString::to_string);
                    values = format!("  ; fp values: {uses} -> {defines}");
                }
                if op.barrier() && op.memory_complete {
                    let reads = op.loads.iter().map(|one| cells.of(one)).collect::<Vec<_>>().join(",");
                    let writes = op.stores.iter().map(|one| cells.of(one)).collect::<Vec<_>>().join(",");
                    let reads = if reads.is_empty() { "-".to_owned() } else { reads };
                    let writes = if writes.is_empty() { "-".to_owned() } else { writes };
                    let _ = write!(values, "  ; memory: complete reads={reads} writes={writes}");
                }
                if let Some(rule) = &op.floating {
                    let inputs = rule.inputs.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
                    let _ = write!(
                        values,
                        "  ; {inputs} -> {}; precision={} rounding={} exceptions={}",
                        rule.result, rule.precision, rule.rounding, rule.exceptions
                    );
                    let numeric: Vec<&floatfacts::Finite> = op
                        .results
                        .iter()
                        .filter_map(|arg| match arg {
                            Arg::Held(held) => exact.get(&held.value),
                            _ => None,
                        })
                        .collect();
                    if !numeric.is_empty() {
                        let facts = numeric
                            .iter()
                            .map(|fact| if fact.negative_zero { "-0".to_owned() } else { fact.value.to_string() })
                            .collect::<Vec<_>>()
                            .join(",");
                        let _ = write!(values, " exact={facts}");
                    }
                }
                let _ = writeln!(out, "    {}  {pad}{}{values}", hex6(op.at), _says(op, &mut cells, calls, verbose));
            }
        }
        if let Some(found) = found {
            for proof in floatfacts::loop_exits(body, &found.dgroup.members, calls) {
                let values = proof
                    .stores
                    .iter()
                    .map(|(reference, fact)| format!("{}={}", cells.of(reference), hex_big(&fact.n)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(out, "\n    exact loop exit {} after {} iterations: {values}", hex(proof.header), proof.count);
            }
        }
        if !cells.order.is_empty() {
            let _ = writeln!(out, "\n    where:");
            for line in cells.legend() {
                let _ = writeln!(out, "{line}");
            }
        }
    }
}

fn hex_big(value: &num_bigint::BigInt) -> String {
    if value.sign() == num_bigint::Sign::Minus { format!("-{:#x}", -value) } else { format!("{value:#x}") }
}

/// Python `dump`'s body: one stage, as MIR.
pub fn mir_stage(
    tag: &str,
    bodies: &[(String, Rc<MirBody>)],
    was: Option<&IndexMap<String, Shapes>>,
    debug: Option<&DebugInfo>,
    found: Option<&Module>,
    quiet: bool,
    verbose: bool,
) -> (String, IndexMap<String, Shapes>) {
    let mut out = String::new();
    let now = _report(&mut out, tag, bodies, was, !quiet);
    _mir(&mut out, bodies, found, verbose, debug);
    (out, now)
}

/// Python `_machine`'s per-stage file: `=== stage`, then each body.
pub fn lir_stage(stage: &str, bodies: &[(String, LirBody)]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== {stage}");
    for (name, one) in bodies {
        _lir_body(&mut out, name, one);
    }
    out
}

/// Python `_lir_body`: one lowered body, its instructions and operands.
fn _lir_body(out: &mut String, name: &str, body: &LirBody) {
    let count: usize = body.blocks.iter().map(|block| block.insns.len()).sum();
    let _ = writeln!(out, "  {name}: {count} instructions");
    for block in &body.blocks {
        let _ = writeln!(out, "    block {}", hex6(block.at));
        for phi in &block.phis {
            let arms = phi.incoming.iter().map(|(at, value)| format!("{}:v{value}", hex6(*at))).collect::<Vec<_>>().join(" ");
            let _ = writeln!(out, "      v{} := phi {arms}", phi.result);
        }
        for one in &block.insns {
            let Some(what) = &one.what else {
                let covers = one.covers.map_or_else(|| "None".to_owned(), |(a, b)| format!("({a}, {b})"));
                let _ = writeln!(out, "      {}  (carried, {covers})", hex6(one.at));
                continue;
            };
            let dests = what.dests.iter().map(_operand).collect::<Vec<_>>().join(", ");
            let sources = what.sources.iter().map(_operand).collect::<Vec<_>>().join(", ");
            let said = if dests.is_empty() { String::new() } else { format!("{dests} := ") };
            let mut notes: Vec<String> = Vec::new();
            if one.covers.is_some_and(|(a, b)| a == b) {
                notes.push("inserted".to_owned());
            }
            if let Some(group) = one.group {
                notes.push(format!("parallel-copy {group}"));
            }
            if one.spill_reload {
                notes.push("spill reload".to_owned());
            }
            if one.frame_adjust {
                notes.push("frame adjust".to_owned());
            }
            if !one.requires.is_empty() {
                let parts = one.requires.iter().map(|(held, register)| format!("v{}@{}", held.value, _name_of(*register))).collect::<Vec<_>>();
                notes.push(format!("requires {}", parts.join(",")));
            }
            if !one.delivers.is_empty() {
                let parts = one.delivers.iter().map(|(held, register)| format!("v{}@{}", held.value, _name_of(*register))).collect::<Vec<_>>();
                notes.push(format!("delivers {}", parts.join(",")));
            }
            if !one.clobbers.is_empty() {
                let parts = one.clobbers.iter().map(|register| _name_of(*register)).collect::<Vec<_>>();
                notes.push(format!("clobbers {}", parts.join(",")));
            }
            let annotation = if notes.is_empty() { String::new() } else { format!("  ; {}", notes.join("; ")) };
            let name = what.name.clone().unwrap_or_else(|| "None".to_owned());
            let line = format!("      {}  {said}{name} {sources}{annotation}", hex6(one.at));
            let _ = writeln!(out, "{}", line.trim_end());
        }
    }
}

/// Python `_name_of`: iced's attribute name for a register number.
pub fn _name_of(register: iced_x86::Register) -> String {
    format!("{register:?}").to_uppercase()
}

/// Python `_operand`: one machine operand, short enough to diff.
fn _operand(one: &Loc) -> String {
    match one {
        Loc::Reg(reg) => _name_of(reg.register),
        Loc::Held(held) => format!("v{}", held.value),
        Loc::Imm(imm) => {
            if imm.value >= 0 { format!("{:#x}", imm.value) } else { imm.value.to_string() }
        }
        Loc::Mem(mem) => {
            let addr = mem.addr.map_or_else(|| "None".to_owned(), |addr| addr.repr());
            match mem.base {
                None => format!("[{addr}]"),
                Some(base) => {
                    let placed = if mem.through == iced_x86::Register::None { "unplaced".to_owned() } else { _name_of(mem.through) };
                    format!("[{addr} v{}@{placed}]", base.value)
                }
            }
        }
        other => ir_repr(other),
    }
}

fn ir_repr(one: &Loc) -> String {
    one.repr()
}

/// Python `_bodies`: every MIR body in this object, or nothing if it does not map.
fn _bodies(
    data: &[u8],
    basic_semantics: bool,
    bounds_checks: bool,
    external: Option<&IndexMap<String, crate::abi::runtime::Contract>>,
) -> Result<(Option<Module>, Vec<(String, Rc<MirBody>)>), String> {
    let records = crate::objectfile::omf::parse(data).map_err(|error| error.to_string())?;
    let Some(found) = crate::objectfile::module::of(&records) else {
        return Ok((None, Vec::new()));
    };
    let Ok(mapped) = crate::frontend::blocks::code_map(&found) else {
        return Ok((Some(found), Vec::new()));
    };
    let mut contracts = crate::abi::runtime::for_module(&found, external).map_err(|error| error.to_string())?;
    let blocks = crate::frontend::blocks::partition(&found, &mapped);
    let raised = mir::bodies(&found, &blocks, Some(&mut contracts), basic_semantics, bounds_checks)?;
    Ok((Some(found), raised.values))
}

/// Python `main`'s options this port reproduces.
struct Arguments {
    object: std::path::PathBuf,
    cpu: String,
    only: Option<String>,
    basic_semantics: bool,
    bounds_checks: bool,
    quiet: bool,
    body: Option<String>,
    verbose: bool,
    dump: std::path::PathBuf,
}

fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let mut object = None;
    let mut dump = None;
    let mut given = Arguments {
        object: std::path::PathBuf::new(),
        cpu: "386".to_owned(),
        only: None,
        basic_semantics: false,
        bounds_checks: false,
        quiet: false,
        body: None,
        verbose: false,
        dump: std::path::PathBuf::new(),
    };
    let mut at = 0;
    while at < argv.len() {
        let one = argv[at].clone();
        let mut value = || {
            at += 1;
            argv.get(at).cloned().ok_or_else(|| format!("argument {one}: expected one argument"))
        };
        match one.as_str() {
            "--dump" => dump = Some(std::path::PathBuf::from(value()?)),
            "--cpu" => given.cpu = value()?,
            "--only" => given.only = Some(value()?),
            "--body" => given.body = Some(value()?),
            "--basic-semantics" => given.basic_semantics = true,
            "--bounds-checks" => given.bounds_checks = true,
            "--quiet" => given.quiet = true,
            "--verbose" => given.verbose = true,
            _ if one.starts_with('-') => return Err(format!("unrecognized arguments: {one}")),
            _ if object.is_none() => object = Some(std::path::PathBuf::from(&one)),
            _ => return Err(format!("unrecognized arguments: {one}")),
        }
        at += 1;
    }
    given.object = object.ok_or("the following arguments are required: object")?;
    given.dump = dump.ok_or("--dump DIR is required")?;
    Ok(given)
}

/// Python `main` with `--dump`: one file per stage, up to the BC-only emission.
pub fn main(argv: &[String]) -> Result<i32, String> {
    let args = parse_args(argv)?;
    let data = std::fs::read(&args.object).map_err(|error| format!("{}: {error}", args.object.display()))?;
    std::fs::create_dir_all(&args.dump).map_err(|error| error.to_string())?;
    let selected =
        |name: &str| args.body.as_ref().is_none_or(|body| name.to_lowercase().contains(&body.to_lowercase()));
    let write = |number: usize, form: &str, name: &str, text: &str| -> Result<(), String> {
        let path = args.dump.join(format!("s{number:02}-{form}-{name}.txt"));
        std::fs::write(&path, text).map_err(|error| error.to_string())?;
        println!("  {}", path.display());
        Ok(())
    };
    let dump = |number: usize,
                name: &str,
                tag: &str,
                bodies: &[(String, Rc<MirBody>)],
                was: Option<&IndexMap<String, Shapes>>,
                debug: &DebugInfo,
                found: &Module|
     -> Result<IndexMap<String, Shapes>, String> {
        let bodies: Vec<(String, Rc<MirBody>)> =
            bodies.iter().filter(|(body_name, _)| selected(body_name)).cloned().collect();
        let (text, now) = mir_stage(tag, &bodies, was, Some(debug), Some(found), args.quiet, args.verbose);
        write(number, "mir", name, &text)?;
        Ok(now)
    };

    let (found, raised) = _bodies(&data, args.basic_semantics, args.bounds_checks, None)?;
    let records = crate::objectfile::omf::parse(&data).map_err(|error| error.to_string())?;
    let debug = crate::objectfile::cvinfo::parse(&records);
    let Some(found) = found.filter(|_| !raised.is_empty()) else {
        println!("  nothing to raise");
        return Ok(1);
    };
    dump(0, "omf", &format!("BC ({} bytes)", data.len()), &raised, None, &debug, &found)?;
    // The stages after the raise are what `wholeseg.emitted` shows its watch,
    // and wholeseg is ported separately; this is where the two meet.
    Err("not yet ported: qbopt.wholeseg.emitted".to_owned())
}

