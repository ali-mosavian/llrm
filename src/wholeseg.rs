//! An object whose code segment this pass wrote, rather than edited.
//!
//! Port of `qbopt/wholeseg.py`. BC's object is a frontend, not an output
//! template: `layout::rebuild` places every body afresh and
//! `backend::omfwrite` serializes a complete new object, so record
//! boundaries, branch displacements and fixup offsets are all produced
//! rather than preserved.
//!
//! Refuses far more than it accepts, and says why each time.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;


use crate::abi::{nativecalls, runtime};
use crate::analysis::noreturn;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::{frame as frames, lower, nativeframe, omfwrite, pointers};
use crate::flow;
use crate::frontends::bc::blocks::{self as split, Block, CodeMap, code_map};
use crate::frontends::bc::{extent, fppatches};
use crate::model::ir;
use crate::model::lir::LirBody;
use crate::model::mir::{self, AllocationHints, MirBody};
use crate::model::passes::{Exception, O2, Options};
use crate::objectfile::module::{self, Addr, Module, SourceMap, Space};
use crate::objectfile::{addends, omf};
use crate::optimize::rotate;
use crate::support::hash::IndexMap;

/// What `watch` is shown: a MIR body, an LIR body, or the route's words.
pub enum Watched<'a> {
    Mir(&'a MirBody),
    Lir(&'a LirBody),
    Route(&'a str),
}

/// `watch(stage, name, low)`.
pub type Watch<'w> = &'w mut dyn FnMut(&str, Option<&str>, Watched<'_>);

/// Which emitter produced an object, or that none did.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Emission {
    Lir,
    Mir,
    Refused,
}

impl Emission {
    pub const fn value(self) -> &'static str {
        match self {
            Emission::Lir => "lir",
            Emission::Mir => "mir",
            Emission::Refused => "refused",
        }
    }
}

/// An object, what made it, and what it said about doing so.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Emitted {
    pub data: Vec<u8>,
    pub outcome: Emission,
    pub reason: String,
    /// Retained for readers of historical MIR-emission results.
    pub fallback_reason: Option<String>,
}

fn value_error(message: impl Into<String>) -> Exception {
    Exception::new("ValueError", message)
}

/// `native_fpu=False` asked for the emulator, which no longer exists.
pub fn _native_only(native_fpu: bool) -> Result<(), Exception> {
    if !native_fpu {
        return Err(value_error(
            "native floating point is the only path; native_fpu=False is no longer supported",
        ));
    }
    Ok(())
}

/// The object rewritten, and which emitter did it.
///
/// `watch(stage, name, low)` is called after lowering and after each machine
/// phase of *this* run, and once with the route the bytes came from.
#[allow(clippy::too_many_arguments)]
pub fn emitted(
    data: &[u8],
    optimise: bool,
    native_fpu: bool,
    only: Option<&str>,
    watch: Option<Watch<'_>>,
    cpu: ProfileOrName<'static>,
    basic_semantics: bool,
    bounds_checks: bool,
    external_contracts: Option<&IndexMap<String, runtime::Contract>>,
    options: &Options,
) -> Result<Emitted, Exception> {
    _native_only(native_fpu)?;
    let (out, why) = _rebuilt(
        data,
        optimise,
        native_fpu,
        only,
        watch,
        cpu,
        basic_semantics,
        bounds_checks,
        external_contracts,
        options,
    )?;
    if why != REBUILT {
        return Ok(Emitted { data: out, outcome: Emission::Refused, reason: why, fallback_reason: None });
    }
    Ok(Emitted { data: out, outcome: Emission::Lir, reason: why, fallback_reason: None })
}

/// `emitted`, as every caller already reads it.
pub fn rebuilt(
    data: &[u8],
    optimise: bool,
    native_fpu: bool,
    only: Option<&str>,
    basic_semantics: bool,
    bounds_checks: bool,
) -> Result<(Vec<u8>, String), Exception> {
    let got = emitted(
        data,
        optimise,
        native_fpu,
        only,
        None,
        ProfileOrName::Name("386"),
        basic_semantics,
        bounds_checks,
        None,
        &O2(),
    )?;
    Ok((got.data, got.reason))
}

fn same_records(one: &[Rc<omf::Record>], other: &[Rc<omf::Record>]) -> bool {
    one.len() == other.len() && one.iter().zip(other).all(|(one, other)| Rc::ptr_eq(one, other))
}

/// The object with its code segment rewritten, and what happened.
///
/// Returns the input unchanged where anything refuses, so a caller can use
/// this as a transform without deciding first whether it will work.
#[allow(clippy::too_many_arguments)]
pub fn _rebuilt(
    data: &[u8],
    optimise: bool,
    native_fpu: bool,
    only: Option<&str>,
    mut watch: Option<Watch<'_>>,
    cpu: ProfileOrName<'static>,
    basic_semantics: bool,
    bounds_checks: bool,
    external_contracts: Option<&IndexMap<String, runtime::Contract>>,
    options: &Options,
) -> Result<(Vec<u8>, String), Exception> {
    let refused = |why: &str| Ok((data.to_vec(), why.to_owned()));
    let target = targets::profile(cpu).map_err(value_error)?;
    let mut records = omf::parse(data).map_err(|error| value_error(error.0))?;
    let Some(mut found) = module::of(&records) else {
        return refused("the module has no code segment");
    };
    let mut mapped: CodeMap = match code_map(&found) {
        Ok(mapped) => mapped,
        Err(why) => return refused(&why),
    };
    if !fppatches::sites(&found, &mapped.starts).is_empty() {
        // A real coprocessor is the only floating-point target.
        if basic_semantics {
            return refused("BASIC floating behaviour cannot be preserved through native conversion");
        }
        records = fppatches::native_records(&found, &mapped.starts);
        let Some(converted) = module::of(&records) else {
            return refused("native floating-point conversion lost the code segment");
        };
        found = converted;
        mapped = match code_map(&found) {
            Ok(mapped) => mapped,
            Err(why) => return refused(&why),
        };
    }

    let normalized = match addends::canonical(&records, found.seg, found.code.len() as i64) {
        Ok(normalized) => normalized,
        Err(why) => return refused(&why),
    };
    if !same_records(&normalized, &records) {
        records = normalized;
        let Some(again) = module::of(&records) else {
            return refused("addend normalization lost the code segment");
        };
        found = again;
        mapped = match code_map(&found) {
            Ok(mapped) => mapped,
            Err(why) => return refused(&why),
        };
    }
    let blocks = split::partition(&found, &mapped);
    // One map for the whole module, and the same object reaches the raise
    // and the lowering: a contract chosen twice can be chosen differently.
    let mut contracts = runtime::for_module(&found, external_contracts).map_err(|error| value_error(error.0))?;
    let mut native_frames: IndexMap<i64, nativeframe::Plan> = IndexMap::default();
    if !split::has_header(&found) {
        let partition = match extent::partition(&found) {
            Ok(partition) if partition.complete() => partition,
            _ => return refused("native procedure ownership is incomplete"),
        };
        contracts = nativecalls::interfaces(&found, &partition, &blocks, &contracts);
        let cleanup: IndexMap<i64, i64> =
            contracts.iter().filter_map(|(&at, rule)| rule.cleanup.map(|cleanup| (at, cleanup))).collect();
        let cleanup = nativecalls::stack_recovery(&found, &partition, &blocks, &cleanup);
        let cleanup: IndexMap<usize, i64> = cleanup.iter().map(|(&at, &size)| (at as usize, size)).collect();
        for procedure in &partition.bodies {
            let owned: Vec<Block> = blocks
                .iter()
                .filter(|block| procedure.ranges.iter().any(|&(start, end)| start <= block.at && block.at < end))
                .cloned()
                .collect();
            let layout = nativeframe::plan(&owned, procedure.seed);
            let layout = layout.and_then(|layout| nativeframe::checked(&owned, &layout, &cleanup));
            let Some(layout) = layout else {
                return refused(&format!("native frame or call cleanup unproved at {:#x}", procedure.seed));
            };
            native_frames.insert(procedure.seed as i64, layout);
        }
    }
    let raised = mir::bodies(&found, &blocks, Some(&mut contracts), basic_semantics, bounds_checks)
        .map_err(|message| Exception::new("Exception", message))?;
    let mut bodies = raised.values;
    let source = raised.source;
    let hints = raised.hints;
    if bodies.is_empty() {
        return refused("no bodies were raised");
    }
    if !bounds_checks {
        let retained = bodies.iter().flat_map(|(_, body)| &body.blocks).flat_map(|block| &block.ops).find(|op| {
            op.kind == mir::Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY")
        });
        if let Some(retained) = retained {
            return refused(&format!("unchecked array lowering unsupported at {:#x} (B$HARY)", retained.at));
        }
    }

    // Optimised as values before being written as bytes. `optimise=false`
    // emits the body exactly as raised.
    if optimise {
        let (shared_blocks, shared_found) = (Rc::new(blocks.clone()), Rc::new(found.clone()));
        let mut done_bodies = Vec::new();
        for (name, body) in bodies {
            // Keep the old diagnostic spelling as an identity selection for
            // callers with saved stage commands.
            if only == Some("widen") {
                if let Some(watch) = watch.as_mut() {
                    watch("mir-widen", Some(&name), Watched::Mir(&body));
                }
                done_bodies.push((name, body));
                continue;
            }
            let mut stage_watch;
            let inner: Option<&mut dyn FnMut(&str, &MirBody)> = match watch.as_mut() {
                Some(watch) => {
                    stage_watch = |stage: &str, state: &MirBody| {
                        watch(&format!("mir-{stage}"), Some(&name), Watched::Mir(state));
                    };
                    Some(&mut stage_watch)
                }
                None => None,
            };
            let mut done = flow::optimized(
                &body,
                &found.dgroup.members,
                &found.calls,
                target,
                Options { unswitch: true, ..options.clone() },
                Some(Rc::clone(&shared_blocks)),
                Some(Rc::clone(&shared_found)),
                only.map(str::to_owned),
                inner,
            )
            .map_err(|message| Exception::new("Exception", message))?;
            if only.is_none() {
                done = rotate::entered(&done).map_err(|error| value_error(error.to_string()))?;
                if let Some(watch) = watch.as_mut() {
                    watch("mir-rotate", Some(&name), Watched::Mir(&done));
                    // Compatibility label for existing stage consumers.
                    watch("mir-widen", Some(&name), Watched::Mir(&done));
                }
            }
            done_bodies.push((name, done));
        }
        bodies = done_bodies;
    }

    // Every byte the decoder walked into, so layout can tell a gap it may
    // carry from one that is real code it simply did not raise.
    let reached: BTreeSet<i64> = blocks
        .iter()
        .flat_map(|block| &block.insns)
        .flat_map(|insn| insn.at as i64..insn.end() as i64)
        .collect();
    let fields: BTreeSet<i64> =
        omf::fixups(&records).into_iter().filter(|one| one.seg == Some(found.seg)).map(|one| one.offset).collect();
    let short = _through_lir(
        &found,
        records,
        &blocks,
        bodies,
        &source,
        &hints,
        &mapped,
        &fields,
        &reached,
        native_fpu,
        &contracts,
        watch.as_deref_mut(),
        target,
        basic_semantics,
        &native_frames,
    )?;
    match short {
        Ok(out) => {
            if let Some(watch) = watch.as_mut() {
                watch("route", None, Watched::Route("the LIR emitter wrote these bytes"));
            }
            Ok((out, REBUILT.to_owned()))
        }
        Err(short) => {
            if let Some(watch) = watch.as_mut() {
                let said = format!("the LIR emitter refused ({short}); the input is unchanged");
                watch("route", None, Watched::Route(&said));
            }
            Ok((data.to_vec(), short))
        }
    }
}

/// The exceptions `_through_lir` names as a body it cannot place.
const CAUGHT: [&str; 7] = ["Unlowered", "Unraisable", "Refused", "Spilled", "Unplaced", "Simultaneous", "Tangled"];

/// Every body lowered, placed and written, or why one could not be.
///
/// `Ok(Err(why))` is a refusal in its own words; `Err` an exception
/// nothing here catches.
#[allow(clippy::too_many_arguments)]
pub fn _through_lir<'w>(
    found: &Module,
    records: Vec<Rc<omf::Record>>,
    blocks: &[Block],
    bodies: Vec<(String, Rc<MirBody>)>,
    source: &SourceMap,
    hints: &IndexMap<i64, AllocationHints>,
    mapped: &CodeMap,
    fields: &BTreeSet<i64>,
    reached: &BTreeSet<i64>,
    native_fpu: bool,
    contracts: &IndexMap<i64, runtime::Contract>,
    mut watch: Option<&mut (dyn FnMut(&str, Option<&str>, Watched<'_>) + 'w)>,
    cpu: &'static targets::Profile,
    basic_semantics: bool,
    native_frames: &IndexMap<i64, nativeframe::Plan>,
) -> Result<Result<Vec<u8>, String>, Exception> {
    let _ = blocks;
    let mut records = records;
    let mut pointer_model = None;
    let pointed = bodies.iter().flat_map(|(_, body)| &body.blocks).flat_map(|block| &block.ops).any(|op| {
        op.kind == mir::Kind::PtrOffset || op.loads.iter().chain(&op.stores).any(|r#ref| r#ref.pointer)
    });
    if pointed {
        let (with, index) = omf::with_external(&records, "b$HugeShift").map_err(|error| value_error(error.0))?;
        records = with;
        let cell = ir::Mem { disp_width: 2, ..ir::Mem::new(Some(Addr { index, ..Addr::new(Space::External, 0) }), 1) };
        pointer_model = Some(pointers::Model::new(pointers::HugeShift::Cell(cell)).map_err(value_error)?);
    }

    let symbols: IndexMap<String, i64> = omf::pubdef_names(&records, found.seg)
        .map_err(|error| value_error(error.0))?
        .into_iter()
        .map(|(at, name)| (name, at))
        .collect();
    let terminal_calls = noreturn::terminal_sites(contracts);
    let local_calls: IndexMap<i64, i64> = found
        .calls
        .iter()
        .filter_map(|(at, name)| symbols.get(name).map(|target| (*at, *target)))
        .collect();
    let entries: IndexMap<i64, MirBody> =
        bodies.iter().map(|(_, body)| (body.entry, MirBody::clone(body))).collect();
    let no_return = noreturn::inferred(&entries, &local_calls, &terminal_calls);
    let mut terminal_sites = terminal_calls.clone();
    terminal_sites.extend(local_calls.iter().filter(|(_, target)| no_return.contains(target)).map(|(at, _)| *at));
    let mut trimmed = Vec::new();
    for (name, body) in bodies {
        let after = noreturn::after_terminal_calls(&body, &terminal_sites);
        if !Rc::ptr_eq(&after, &body) {
            if let Some(watch) = watch.as_mut() {
                watch("mir-noreturn", Some(&name), Watched::Mir(&after));
            }
        }
        trimmed.push((name, after));
    }
    let bodies = trimmed;
    let family = module::family(&found.records);
    // Python's `source.absorbed` dict: its ids, and its records as `sites`.
    let absorbed: BTreeSet<u32> = source.absorbed.keys().copied().collect();
    let refusal = |name: &str, raised: Exception| -> Result<Result<Vec<u8>, String>, Exception> {
        if CAUGHT.contains(&raised.kind) {
            // A contract this cannot honour, or an operand no encoding
            // covers: this body falls back rather than taking the module down.
            return Ok(Err(format!("{name}: {}: {}", raised.kind, raised.message)));
        }
        Err(raised)
    };
    let mut done = Vec::new();
    for (name, body) in &bodies {
        let low = lower::lowered(
            name,
            body,
            Some(&found.calls),
            absorbed.clone(),
            Some(contracts),
            cpu,
            lower::Lowered {
                coverage: source.coverage.clone(),
                nodes: source.nodes.clone(),
                occurrences: Some(&source.occurrences),
                hints: Some(&hints[&body.entry]),
                pointer_model: pointer_model.clone(),
                noreturn: no_return.contains(&body.entry),
                terminal: terminal_sites.clone(),
                sites: source.absorbed.clone(),
            },
        );
        let low = match low {
            Ok(low) => low,
            Err(short) => return refusal(name, Exception::defined_in("qbopt.backend.lower", "Unlowered", short.0)),
        };
        let mut low = flow::verified(low, "lower", true).map_err(|error| Exception::defined_in("qbopt.backend.verify", "Malformed", error.0))?;
        let native = native_frames.get(&body.entry);
        if let Some(native) = native {
            low = nativeframe::bound(&low, native);
        }
        if let Some(watch) = watch.as_mut() {
            watch("lowered", Some(name), Watched::Lir(&low));
        }
        let frame = match frames::of(&low, Some(&found.calls), family.value(), native.cloned()) {
            Ok(frame) => frame,
            Err(short) => return refusal(name, Exception::defined_in("qbopt.backend.frame", "Refused", short.0)),
        };
        let mut in_ssa = true;
        let mut phases =
            flow::machine(&low.pins, Some(Rc::new(RefCell::new(frame))), Some(&found.calls), basic_semantics, cpu)
                .map_err(value_error)?;
        for phase in &mut phases {
            if phase.class_name() == "PhiElimination" {
                in_ssa = false;
            }
            low = match flow::checked(low, phase.as_mut(), in_ssa) {
                Ok(low) => low,
                Err(flow::Checked::Refused(raised)) => return refusal(name, raised),
                Err(flow::Checked::Malformed(malformed)) => return Err(Exception::defined_in("qbopt.backend.verify", "Malformed", malformed.0)),
            };
            if let Some(watch) = watch.as_mut() {
                watch(phase.name(), Some(name), Watched::Lir(&low));
            }
        }
        done.push(low);
    }
    let tables: Vec<(i64, i64)> = mapped.tables.iter().map(|&(lo, hi)| (lo as i64, hi as i64)).collect();
    omfwrite::written_bc(
        found,
        &done,
        &records,
        &IndexMap::default(),
        &tables,
        fields,
        Some(reached),
        native_fpu,
        false,
        Some(source),
    )
    .map_err(|error| match error {
        omfwrite::Error::Survived(one) => Exception::defined_in("qbopt.backend.omfwrite", "Survived", one.0),
        omfwrite::Error::Unencodable(one) => Exception::defined_in("qbopt.backend.omfwrite", "Unencodable", one.0),
        omfwrite::Error::Unprintable(one) => one.into(),
        omfwrite::Error::Value(one) => value_error(one.0),
    })
}

pub const REBUILT: &str = "rebuilt";

#[cfg(test)]
#[path = "wholeseg_tests.rs"]
mod tests;
