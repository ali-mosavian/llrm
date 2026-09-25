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

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::{hir, libfunc, raise_hir, stream};
use crate::analysis::{alias, interprocedural};
use crate::optimize::interprocedural as module;
use crate::optimize::rotate;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::{
    cpu, executed, frame, jumps, lower, lower_int64, masm, omfwrite,
};
use crate::flow;
use crate::model::lir;
use crate::model::passes::Options;
use crate::model::mir::{self, Arg, Const, MemRef, MirBody};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::{self, Repr};

#[derive(Debug)]
pub enum CompileError {
    Unsupported(hir::Unsupported),
    NotPorted(&'static str),
    Io(std::io::Error),
    Emission(omfwrite::Error),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(error) => write!(formatter, "{error}"),
            Self::NotPorted(function) => write!(formatter, "not yet ported: {function}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Emission(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<hir::Unsupported> for CompileError {
    fn from(error: hir::Unsupported) -> Self {
        Self::Unsupported(error)
    }
}

impl From<String> for CompileError {
    fn from(error: String) -> Self {
        Self::Unsupported(hir::Unsupported(error))
    }
}

impl From<omfwrite::Error> for CompileError {
    fn from(error: omfwrite::Error) -> Self {
        Self::Emission(error)
    }
}

impl From<masm::Unprintable> for CompileError {
    fn from(error: masm::Unprintable) -> Self {
        Self::Emission(omfwrite::Error::Unprintable(error))
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
    target: &str,
    options: &Options,
) -> Result<masm::Module, CompileError> {
    // `targets.profile` wants the name the table holds.
    let target = cpu::names().into_iter().find(|name| *name == target).unwrap_or("");

    let unit = hir::unit(&stream::parse(text))?;
    write(dump, "stream", || text.to_owned())?;
    write(dump, "hir", || hir::text(&unit))?;
    let mut shared = raise_hir::Shared::default();
    let mut raised_procedures: Vec<raise_hir::Raised> = Vec::new();
    for proc in &unit.procs {
        raised_procedures.push(raise_hir::raised(&unit, proc, &mut shared)?);
    }
    let aliases: IndexMap<String, alias::Procedure> = raised_procedures
        .iter()
        .map(|one| {
            let procedure =
                alias::Procedure {
                    body: one.body.clone(),
                    calls: one.calls.clone(),
                    arguments: one.arguments.clone(),
                    named: Default::default(),
                    outside: Default::default(),
                };
            (one.name.clone(), procedure)
        })
        .collect();
    let callees: BTreeSet<String> = raised_procedures.iter().flat_map(|one| one.calls.values().cloned()).collect();
    let known = libfunc::summaries(callees.iter().map(String::as_str));
    let modref = alias::summaries(&aliases, Some(&known)).map_err(hir::Unsupported)?;
    let initial = _constant_initializers(&unit);
    let mut bodies: IndexMap<String, Rc<MirBody>> = IndexMap::default();
    for one in &raised_procedures {
        let mut body = alias::calls_annotated(&aliases[&one.name], &modref).map_err(hir::Unsupported)?;
        body.initial = _body_initializers(&body, &initial);
        bodies.insert(one.name.clone(), Rc::new(body));
    }
    let address_taken = _address_taken_procedures(&unit);
    let call_arguments: IndexMap<String, IndexMap<i64, BTreeSet<i64>>> = raised_procedures
        .iter()
        .map(|one| (one.name.clone(), interprocedural::argument_sites(&bodies[&one.name], &one.contracts)))
        .collect();

    let profile = cpu::profile(cpu::ProfileOrName::Name(target)).map_err(hir::Unsupported)?;
    let mut private = BTreeSet::new();
    if _optimise {
        private = raised_procedures
            .iter()
            .filter(|one| !one.symbol.exported() && !address_taken.contains(&one.name))
            .map(|one| one.name.clone())
            .collect();
        let sources = raised_procedures
            .iter()
            .map(|one| (one.name.clone(), (&one.calls, &one.constants)))
            .collect::<IndexMap<_, _>>();
        let constants = interprocedural::constant_parameters(&sources, &private);
        for raised in &raised_procedures {
            let Some(constants) = constants.get(&raised.name) else {
                continue;
            };
            let specialized = interprocedural::specialize_parameters(&bodies[&raised.name], &raised.parameters, constants);
            bodies.insert(raised.name.clone(), specialized);
        }
    }

    let run_optimiser = |raised: &raise_hir::Raised, body: &Rc<MirBody>, prefix: &str| -> Result<Rc<MirBody>, CompileError> {
        let mut failed = None;
        let mut observe = |stage: &str, after: &MirBody| {
            if failed.is_none() {
                if let Err(error) =
                    write(dump, &format!("passes/{}.{prefix}{stage}", raised.name), || _mir_text(&raised.name, after))
                {
                    failed = Some(error);
                }
            }
        };
        let body = flow::optimized(
            body,
            &BTreeSet::new(),
            &raised.calls,
            cpu::ProfileOrName::Profile(profile),
            options.clone(),
            None,
            None,
            None,
            if dump.is_some() { Some(&mut observe) } else { None },
        )
        .map_err(hir::Unsupported)?;
        let body = rotate::entered(&body).map_err(|error| hir::Unsupported(error.to_string()))?;
        if dump.is_some() {
            observe("rotate", &body);
        }
        if let Some(error) = failed {
            return Err(error.into());
        }
        Ok(body)
    };

    let mut noreturn: BTreeSet<String> = BTreeSet::new();
    if _optimise {
        for one in &raised_procedures {
            let optimised = run_optimiser(one, &bodies[&one.name], "")?;
            bodies.insert(one.name.clone(), optimised);
        }

        let mut roots = raised_procedures
            .iter()
            .filter(|one| one.symbol.exported())
            .map(|one| one.name.clone())
            .collect::<BTreeSet<_>>();
        roots.extend(address_taken.iter().cloned());
        let call_far = profile.cost("call_far").map_err(hir::Unsupported)?;
        let procedures = raised_procedures
            .iter()
            .map(|one| module::Procedure {
                name: &one.name,
                calls: &one.calls,
                parameters: &one.parameters,
                constants: &one.constants,
                arguments: &call_arguments[&one.name],
            })
            .collect::<Vec<_>>();
        let found = module::optimized(
            &procedures,
            &mut bodies,
            &private,
            &roots,
            call_far,
            &mut |index, body, stage| run_optimiser(&raised_procedures[index], body, stage),
            &mut |index, stage, body| {
                let name = &raised_procedures[index].name;
                Ok(write(dump, &format!("passes/{name}.{stage}"), || _mir_text(name, body))?)
            },
        )?;
        drop(procedures);
        noreturn = found.noreturn;
        raised_procedures.retain(|one| found.reachable.contains(&one.name));
    }

    let mut mirs = Vec::new();
    let mut lirs: Vec<String> = Vec::new();
    let mut costs: Vec<String> = Vec::new();
    let mut procedures: Vec<masm::Procedure> = Vec::new();
    for raised in &raised_procedures {
        let body = &bodies[&raised.name];
        if dump.is_some() {
            mirs.push(_mir_text(&raised.name, body));
            if _optimise {
                mirs.push(_mir_text(&format!("{} (opt)", raised.name), body));
            }
        }
        let legalized =
            lower_int64::expanded(body, Some(&raised.calls), Some(&raised.contracts), Some(&raised.hints))
                .map_err(|error| hir::Unsupported(error.0))?;
        // Python compares `body is not raised.body`, which is never the same object.
        write(dump, &format!("passes/{}.int64-lower", raised.name), || _mir_text(&raised.name, &legalized.body))?;
        let low = lower::lowered(
            &raised.name,
            &legalized.body,
            Some(&legalized.calls),
            BTreeSet::new(),
            Some(&legalized.contracts),
            cpu::ProfileOrName::Name(target),
            lower::Lowered {
                hints: Some(&legalized.hints),
                terminal: interprocedural::terminal_sites(&legalized.calls, &noreturn),
                ..Default::default()
            },
        )
        .map_err(|error| hir::Unsupported(error.0));
        // Until the machine phases are ported, `mir` is written before the
        // first procedure stops so the raise can still be diffed.
        let low = match low {
            Ok(low) => low,
            Err(error) => {
                write(dump, "mir", || mirs.join("\n"))?;
                return Err(error.into());
            }
        };
        let low = flow::verified(low, "lower", true).map_err(|error| hir::Unsupported(error.0))?;
        write(dump, &format!("passes/{}.lir-lower", raised.name), || _lir_text(&raised.name, &low))?;
        if dump.is_some() {
            lirs.push(_lir_text(&raised.name, &low));
        }
        let frame = frame::of(&low, Some(&legalized.calls), "", None).map_err(|error| hir::Unsupported(error.0))?;
        let frame = Rc::new(RefCell::new(frame));
        let mut phases = flow::machine(&low.pins, Some(Rc::clone(&frame)), Some(&legalized.calls), false, target)
            .map_err(hir::Unsupported)?;
        let mut in_ssa = true;
        let mut low = low;
        for (number, phase) in phases.iter_mut().enumerate() {
            if phase.class_name() == "Prologue" {
                continue;
            }
            if phase.class_name() == "PhiElimination" {
                in_ssa = false;
            }
            low = flow::checked(low, phase.as_mut(), in_ssa).map_err(|error| {
                hir::Unsupported(match error {
                    flow::Checked::Refused(raised) => raised.message,
                    flow::Checked::Malformed(malformed) => malformed.0,
                })
            })?;
            write(
                dump,
                &format!("phases/{}.{number:02}-{}", raised.name, phase.class_name()),
                || _lir_text(&raised.name, &low),
            )?;
        }
        let reserve = {
            let frame = frame.borrow();
            -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor)
        };
        let mut callees: IndexMap<i64, masm::Callee> = raised
            .callees
            .iter()
            .map(|(at, one)| {
                let code = raised.inline.get(at).cloned().unwrap_or_default();
                (*at, masm::Callee { name: one.object_name(), far: one.far(), code })
            })
            .collect();
        for (at, code) in &legalized.inline {
            let code = code.iter().map(|one| masm::InlinePart::Bytes(one.clone())).collect();
            callees.insert(*at, masm::Callee { name: legalized.calls[at].clone(), far: false, code });
        }
        let procedure = masm::Procedure {
            name: raised.name.clone(),
            public: raised.symbol.exported(),
            far: raised.symbol.far(),
            body: low,
            reserve,
            callees,
            interrupt: None,
        };
        let overhead = masm::return_overhead_bytes(&procedure)? as i64;
        let masm::Procedure { name, public, far, body, reserve, callees, interrupt } = procedure;
        let low = jumps::duplicated_returns(body, overhead);
        if dump.is_some() {
            lirs.push(_lir_text(&format!("{} (allocated)", raised.name), &low));
            costs.push(executed::summary(&low));
            costs.push(format!("{} loop trip counts {:?}", low.name, low.loop_trip_counts));
        }
        procedures.push(masm::Procedure { name, public, far, body: low, reserve, callees, interrupt });
    }
    write(dump, "mir", || mirs.join("\n"))?;
    write(dump, "lir", || lirs.join("\n"))?;
    write(dump, "cost", || costs.join("\n") + "\n")?;
    let mut externs = _externs(&unit);
    externs.extend(shared.runtime.values().map(|one| (one.object_name(), "far".to_owned())));
    let mut data = if _optimise {
        let kept = raised_procedures
            .iter()
            .map(|one| (one.name.clone(), bodies[&one.name].clone()))
            .collect::<IndexMap<_, _>>();
        _data(&unit, Some(&_reachable_data(&unit, &kept)))?
    } else {
        _data(&unit, None)?
    };
    data.extend(_literals(&shared));
    let built = masm::Module {
        code: format!("{}_TEXT", _module.to_uppercase()),
        names: raise_hir::names(&unit, Some(&shared)),
        externs,
        publics: unit.symbols.values().filter(|one| one.exported()).map(|one| one.object_name()).collect(),
        data,
        procedures,
        private: unit
            .segments
            .values()
            .filter(|one| one.attr & hir::PRIVATE != 0)
            .map(|one| one.name.clone())
            .collect(),
        requests: BTreeSet::new(),
    };
    if dump.is_some() {
        let text = masm::text(&built)?;
        write(dump, "asm", || text)?;
    }
    Ok(built)
}

/// Internal procedure symbols used as values rather than direct callees.
fn _address_taken_procedures(unit: &hir::Unit) -> BTreeSet<String> {
    let mut direct = BTreeSet::new();
    for call in unit.calls.values() {
        if !call.target.starts_with('n') {
            continue;
        }
        let target = hir::handle(&call.target);
        if let Some(node) = unit.nodes.get(&target) {
            if node.call == "CGFEName"
                && !node.args.is_empty()
                && node.args[0].starts_with('y')
                && hir::handle(&node.args[0]) == call.symbol
            {
                direct.insert(target);
            }
        }
    }

    let mut referenced = BTreeSet::new();
    let mut sequences: Vec<Vec<String>> = unit.nodes.values().map(|node| node.args.clone()).collect();
    sequences.extend(unit.procs.iter().flat_map(|proc| proc.body.iter().map(|statement| statement.args.clone())));
    sequences.extend(unit.calls.values().flat_map(|call| call.parms.iter().map(|(node, _type)| vec![node.clone()])));
    for args in &sequences {
        referenced.extend(
            args.iter()
                .filter(|arg| {
                    arg.starts_with('n') && arg.len() > 1 && arg[1..].chars().all(|char| char.is_ascii_digit())
                })
                .map(|arg| hir::handle(arg)),
        );
    }

    let mut taken = BTreeSet::new();
    for (at, node) in &unit.nodes {
        if node.call == "CGFEName" && !node.args.is_empty() && node.args[0].starts_with('y') {
            if let Some(symbol) = unit.symbols.get(&hir::handle(&node.args[0])) {
                if symbol.proc() && (!direct.contains(at) || referenced.contains(at)) {
                    taken.insert(symbol.object_name());
                }
            }
        }
    }
    for symbol in unit.backs.values() {
        if let Some(symbol) = unit.symbols.get(symbol).filter(|symbol| symbol.proc()) {
            taken.insert(symbol.object_name());
        }
    }
    for symbol in unit.symbols.values() {
        for fixup in symbol.code.iter().flat_map(|code| &code.fixups) {
            if let Some(target) = unit.symbols.get(&fixup.symbol).filter(|target| target.proc()) {
                taken.insert(target.object_name());
            }
        }
    }
    for segment in unit.segments.values() {
        for (call, args) in &segment.items {
            if call == "DGFEPtr" {
                if let Some(symbol) = unit.symbols.get(&hir::handle(&args.0[0])).filter(|symbol| symbol.proc()) {
                    taken.insert(symbol.object_name());
                }
            }
        }
    }
    taken
}

pub fn _lir_text(name: &str, body: &lir::LirBody) -> String {
    let mut out = vec![format!("== {name}")];
    for block in &body.blocks {
        out.push(format!("block {} -> {}", block.at, pyrepr::tuple(&block.succ)));
        out.extend(block.phis.iter().map(|phi| format!("  phi {}", phi.repr())));
        out.extend(block.insns.iter().map(|one| {
            format!(
                "  {:4} {} req={} del={}",
                one.at,
                one.what.repr(),
                pyrepr::tuple(&one.requires),
                pyrepr::tuple(&one.delivers)
            )
        }));
    }
    out.join("\n") + "\n"
}

fn _externs(unit: &hir::Unit) -> Vec<(String, String)> {
    unit.symbols
        .values()
        .filter(|one| one.imported() && one.code.is_none() && !raise_hir::EMITTED.contains(&one.name.as_str()))
        .map(|one| {
            let kind = if one.proc() {
                if one.far() { "far" } else { "near" }
            } else if unit.grouped(one) {
                "byte"
            } else {
                "far-byte"
            };
            (one.object_name(), kind.to_owned())
        })
        .collect()
}

/// Each data segment's items.
fn _data(unit: &hir::Unit, kept: Option<&BTreeSet<i64>>) -> Result<Vec<(String, Vec<masm::Datum>)>, hir::Unsupported> {
    let mut out = vec![];
    for segment in unit.segments.values() {
        if segment.items.is_empty() || segment.attr & 0x1 != 0 {
            continue; // EXEC: code has no data items
        }
        let mut items = vec![];
        let spans = _data_labels(unit);
        let dropped: BTreeSet<usize> = spans
            .iter()
            .filter(|(symbol, (segment_id, _, _))| {
                kept.is_some_and(|kept| *segment_id == segment.id && !kept.contains(symbol))
            })
            .map(|(_, (_, start, _))| *start)
            .collect();
        let mut skip = false;
        let label_name = |back: &str| {
            let symbol = unit.backs[&hir::handle(back)];
            if symbol != 0 { unit.symbols[&symbol].object_name() } else { format!("L_b{}", hir::handle(back)) }
        };
        for (at, (call, args)) in segment.items.iter().enumerate() {
            if dropped.contains(&at) {
                skip = true;
            } else if call == "DGLabel" {
                skip = false;
            }
            if skip {
                continue;
            }
            let args: Vec<&str> = args.0.iter().map(String::as_str).collect();
            let number = |text: &str| text.parse::<i64>().map_err(|_| hir::Unsupported(format!("not a number: {text}")));
            items.push(match (call.as_str(), &args[..]) {
                ("DGLabel", [back]) => masm::Datum::Label(masm::Label { name: label_name(back) }),
                ("DGUBytes", [size]) => masm::Datum::Fill(masm::Fill {
                    size: number(size)?,
                    byte: if segment.name == "_BSS" { None } else { Some(0) },
                }),
                ("DGIBytes", [size, byte]) => {
                    masm::Datum::Fill(masm::Fill { size: number(size)?, byte: Some(number(byte)? as u8) })
                }
                ("DGBytes", [_size, data]) => masm::Datum::Bytes(hex_bytes(data)),
                ("DGInteger", [value, type_]) => {
                    // The shim prints a negative item as its 32-bit two's complement.
                    let width = raise_hir::widths(type_).unwrap_or(2) as usize;
                    let value = number(value)?;
                    masm::Datum::Bytes(value.to_le_bytes()[..width].to_vec())
                }
                ("DGFEPtr", [symbol, type_, offset]) => {
                    let far = raise_hir::far_pointers(type_) || matches!(*type_, "TY_LONG_CODE_PTR" | "TY_CODE_PTR");
                    masm::Datum::Pointer(masm::Pointer {
                        name: unit.symbols[&hir::handle(symbol)].object_name(),
                        offset: number(offset)?,
                        far,
                    })
                }
                ("DGBackPtr", [back, _segment, offset, type_]) => masm::Datum::Pointer(masm::Pointer {
                    name: label_name(back),
                    offset: number(offset)?,
                    far: raise_hir::far_pointers(type_),
                }),
                ("DGAlign", [align]) => masm::Datum::Align(masm::Align { to: number(align)? }),
                _ => return Err(hir::Unsupported(format!("data item {call} {}", args.join(" ")))),
            });
        }
        out.push((segment.name.clone(), items));
    }
    Ok(out)
}

/// The float constants the raise placed, in DGROUP's constant segment.
fn _literals(shared: &raise_hir::Shared) -> Vec<(String, Vec<masm::Datum>)> {
    let mut lines = vec![];
    for (packed, number) in &shared.literals {
        lines.push(masm::Datum::Label(masm::Label { name: format!("L_f{number}") }));
        lines.push(masm::Datum::Bytes(packed.clone()));
    }
    if lines.is_empty() { vec![] } else { vec![("CONST".to_owned(), lines)] }
}

/// Each named data object's `(segment, first item, after item)` span.
/// Named data proven observable from emitted code, linkage, or data.
///
/// Only a labelled non-procedure symbol that is neither imported nor public
/// is deleted, and only after every root has been closed over
/// data-initializer pointers.
fn _reachable_data(unit: &hir::Unit, bodies: &IndexMap<String, Rc<MirBody>>) -> BTreeSet<i64> {
    let labels = _data_labels(unit);
    let candidates = labels
        .keys()
        .copied()
        .filter(|symbol| {
            let one = &unit.symbols[symbol];
            !one.proc() && !one.imported() && !one.exported()
        })
        .collect::<BTreeSet<_>>();
    let mut kept = labels.keys().copied().filter(|symbol| !candidates.contains(symbol)).collect::<BTreeSet<_>>();
    for body in bodies.values() {
        kept.extend(_referenced_data(body, &candidates));
    }
    // Inline assembly's relocation table is the exact reference evidence.
    for symbol in unit.symbols.values() {
        for fixup in symbol.code.iter().flat_map(|code| &code.fixups) {
            if candidates.contains(&fixup.symbol) {
                kept.insert(fixup.symbol);
            }
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for symbol in kept.clone() {
            let Some(&(segment, start, after)) = labels.get(&symbol) else {
                continue;
            };
            for (call, args) in &unit.segments[&segment].items[start..after] {
                let target = match call.as_str() {
                    "DGFEPtr" => Some(hir::handle(&args.0[0])),
                    "DGBackPtr" => unit.backs.get(&hir::handle(&args.0[0])).copied(),
                    _ => None,
                };
                if let Some(target) = target {
                    if candidates.contains(&target) && !kept.contains(&target) {
                        kept.insert(target);
                        changed = true;
                    }
                }
            }
        }
    }
    kept
}

fn _data_labels(unit: &hir::Unit) -> IndexMap<i64, (i64, usize, usize)> {
    let mut out = IndexMap::default();
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
    let reference = |one: &MemRef, symbol: &mut dyn FnMut(&mir::Symbol)| {
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

/// Rendering a stage costs more than most passes, so only a dump does it.
fn write(dump: Option<&Path>, stage: &str, text: impl FnOnce() -> String) -> std::io::Result<()> {
    if let Some(dump) = dump {
        let path = dump.join(stage);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text())?;
    }
    Ok(())
}

struct Args {
    source: PathBuf,
    output: Option<PathBuf>,
    dump: Option<PathBuf>,
    opt: bool,
    cpu: String,
    options: Options,
    include: Vec<String>,
}

/// The code-generator stream wccq records for one C file.
pub fn recorded(source: &Path, includes: &[String]) -> Result<String, hir::Unsupported> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wccq = Path::new(option_env!("LLRM_WCCQ").ok_or_else(|| hir::Unsupported("llrm was built without the toolchain feature".into()))?);
    // Borland's medium model: far code, near data, cdecl, byte-packed structs,
    // 16-bit enums, x87 inline, no stack probes, no default library. -fp3 is for
    // inline assembly: qcport's own uses 387 instructions.
    let borland = format!("-fi={}", root.join("src/frontends/c/borland.h").display());
    let flags = ["-mm", "-3", "-fpi87", "-fp3", "-zp1", "-ei", "-ecc", "-s", "-zl", "-zq", borland.as_str()];
    let failed = |detail: String| hir::Unsupported(format!("wccq failed on {}:\n{detail}", source.display()));
    let scratch = tempfile::tempdir().map_err(|error| failed(error.to_string()))?;
    let out = scratch.path().join("unit.cgs");
    let absolute = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let searched = includes.iter().map(|one| format!("-I{}", absolute(Path::new(one)).display()));
    // In the scratch directory, where wccq also leaves its .err file.
    let done = std::process::Command::new(&wccq)
        .args(flags)
        .args(searched)
        .arg(format!("-fo={}/unit.obj", scratch.path().display()))
        .arg(absolute(source))
        .env("QBOPT_CG_STREAM", &out)
        .current_dir(scratch.path())
        .output()
        .map_err(|error| failed(error.to_string()))?;
    if !done.status.success() || !out.exists() {
        let text = String::from_utf8_lossy(&done.stdout).into_owned() + &String::from_utf8_lossy(&done.stderr);
        return Err(failed(text));
    }
    fs::read_to_string(&out).map_err(|error| failed(error.to_string()))
}

const USAGE: &str =
    "usage: llrm-c [-h] [-o OUTPUT] [-I INCLUDE] [--dump DUMP] [--opt] [--cpu CPU] [-O {s,2}] source";

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let (mut source, mut output, mut dump, mut opt, mut cpu) =
        (None, None, None, false, "386".to_owned());
    let mut include = Vec::new();
    let mut options = crate::model::passes::O2();
    let mut rest = argv.iter();
    while let Some(one) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or(format!("argument {name}: expected one argument"))
        };
        match one.as_str() {
            "-o" | "--output" => output = Some(PathBuf::from(value("-o/--output")?)),
            "-I" | "--include" => include.push(value("-I/--include")?),
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--opt" => opt = true,
            "--cpu" => cpu = value("--cpu")?,
            level if level.starts_with("-O") => {
                let text = if level.len() > 2 { level[2..].to_owned() } else { value("-O")? };
                options = flow::level_option(&text).map_err(|message| format!("argument -O: {message}"))?;
            }
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
        options,
        include,
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
    let result = (|| -> Result<(), CompileError> {
        let text = if args.source.extension().and_then(|one| one.to_str()) == Some("cgs") {
            fs::read_to_string(&args.source)?
        } else {
            recorded(&args.source, &args.include)?
        };
        let output = args
            .output
            .clone()
            .unwrap_or_else(|| args.source.with_extension("asm"));
        let module = args
            .source
            .file_stem()
            .and_then(|one| one.to_str())
            .unwrap_or_default();
        let built = assembled(&text, module, args.opt, args.dump.as_deref(), &args.cpu, &args.options)?;
        let name = args.source.file_name().and_then(|one| one.to_str()).unwrap_or_default();
        if output.extension().and_then(|one| one.to_str()).map(str::to_lowercase).as_deref() == Some("obj") {
            fs::write(&output, omfwrite::written(&built, name)?)?;
        } else {
            fs::write(&output, masm::text(&built)?)?;
        }
        Ok(())
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
    use crate::frontends::c::hir;
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

    /// A pass that changed nothing returned a copy, so the proof caches keyed
    /// on the body missed: matmul solved 268 constant fixed points to
    /// Python's 186.  Python solves 33 and 15 on this fixture.
    #[test]
    fn test_unchanged_bodies_reuse_their_proofs_as_python_does() {
        use crate::analysis::consts::SOLVED;
        use crate::optimize::transform::HALVED;

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/loopaddr.cgs");
        let text = std::fs::read_to_string(path).unwrap();
        SOLVED.with(|solved| solved.set(0));
        HALVED.with(|halved| halved.set(0));
        assert!(assembled(&text, "loopaddr", true, None, "386", &crate::model::passes::O2()).is_ok());
        assert_eq!((SOLVED.with(|solved| solved.get()), HALVED.with(|halved| halved.get())), (32, 15));
    }

    /// The innermost loop's lines, from its label to its backward branch.
    fn innermost(fixture: &str) -> Vec<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/c/{fixture}.cgs"));
        let text = std::fs::read_to_string(path).unwrap();
        let built = assembled(&text, fixture, true, None, "486", &crate::model::passes::O2()).unwrap();
        let asm = crate::backend::masm::text(&built).unwrap();
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        let (top, back) = lines
            .iter()
            .enumerate()
            .filter(|(_, one)| one.starts_with('j') && !one.starts_with("jmp"))
            .find_map(|(at, one)| {
                let label = format!("{}:", one.split_whitespace().nth(1)?);
                Some((lines[..at].iter().position(|line| *line == label)?, at))
            })
            .expect("a loop");
        lines[top..back].iter().map(|one| (*one).to_owned()).collect()
    }

    /// The innermost loop's counting: its steps by a constant and its compares.
    fn loop_counting(fixture: &str) -> Vec<String> {
        let constant = |one: &str| one.rsplit(", ").next().is_some_and(|last| last.parse::<i64>().is_ok());
        innermost(fixture)
            .into_iter()
            .filter(|one| {
                one.starts_with("inc ")
                    || one.starts_with("dec ")
                    || one.starts_with("cmp ")
                    || (one.starts_with("add ") || one.starts_with("sub ")) && constant(one)
            })
            .collect()
    }

    /// `dot` indexes `a[i]` and `b[i]`: before strength waited for the other passes to
    /// settle, it kept two pointers and a counter, three steps per iteration.
    #[test]
    fn test_addresses_differing_by_base_share_one_stepped_offset() {
        assert_eq!(loop_counting("dot"), ["add bx, 2"]);
    }

    /// `bytes` indexes by `i` itself, with a bound only known at run time: the
    /// counter never counted to zero, so each iteration compared it with `n` in memory.
    #[test]
    fn test_a_counter_read_only_as_offsets_counts_to_zero() {
        assert_eq!(loop_counting("bytes"), ["inc bx"]);
    }

    /// `from1` reads `a[i]` and `b[i - 1]`: `(i - 1) * 2` was a second root beside
    /// `i * 2`, so each array stepped its own pointer beside a counter.
    #[test]
    fn test_subscripts_of_one_stride_share_one_offset() {
        assert_eq!(loop_counting("from1"), ["add bx, 2"]);
        let loop_ = innermost("from1");
        let two_registers = |one: &String| {
            let inside = one.split_once("ptr [").map_or("", |(_, inside)| inside).as_bytes();
            inside.len() > 4 && inside[2] == b'+' && inside[3].is_ascii_alphabetic()
        };
        assert!(loop_.iter().filter(|one| one.contains("ptr [")).all(two_registers), "{loop_:#?}");
    }

    /// Rotation consumed the syntax that proved crc's counts, so the instrument
    /// guessed nine in ten and read 1505 executed instructions instead of 1356.
    #[test]
    fn test_rotation_keeps_provable_trip_counts() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/crc.cgs");
        let text = std::fs::read_to_string(path).unwrap();
        let dump = tempfile::tempdir().unwrap();
        assembled(&text, "crc", true, Some(dump.path()), "486", &crate::model::passes::O2()).unwrap();
        let cost = std::fs::read_to_string(dump.path().join("cost")).unwrap();
        assert!(!cost.contains("loop trip counts []"), "{cost}");
    }

    /// A callee taking arguments in registers: the raise pushed them anyway,
    /// and ls linked against `strlen_`, Watcom's register-convention strlen.
    #[test]
    fn test_register_convention_is_refused() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/regs.cgs");
        match assembled(&std::fs::read_to_string(path).unwrap(), "regs", false, None, "386", &crate::model::passes::O2()) {
            Err(CompileError::Unsupported(refused)) => {
                assert!(refused.to_string().contains("_twice has a register calling convention"), "{refused}");
            }
            _ => panic!("a register convention was compiled as a stack one"),
        }
    }

    /// toolchain/owshim/build.sh hardcoded macOS ARM64's defines and clang, so no wccq
    /// could be built on any other host and llrm-c refused every C file.
    #[test]
    fn test_wccq_built_here_records_the_committed_stream() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = root.join("tests/fixtures/c/halve.c");
        let without_path = |text: &str| text.lines().filter(|line| !line.contains("DBSrcFile")).collect::<Vec<_>>().join("\n");
        let recorded = super::recorded(&source, &[]).expect("wccq records halve.c");
        let committed = std::fs::read_to_string(root.join("tests/fixtures/c/halve.cgs")).unwrap();
        assert_eq!(without_path(&recorded), without_path(&committed));
    }
}
