//! Port of `qbopt/frontend/modern/compile.py`: common HIR from the modern
//! frontend to a fresh 16-bit OMF object.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;

use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::{addressvalues, frame as frames, jumps, lower, lower_int64, masm, omfwrite};
use crate::flow;
use crate::frontend::qbc::abi::physicalize;
use crate::hir::lower::Lowered;
use crate::hir::{self, callmemory, model};
use crate::model::mir::{Kind, MirBody};
use crate::model::passes::Options;
use crate::objectfile::module::Space;
use crate::optimize::rotate;
use crate::support::hash::IndexMap;
use crate::support::pyrepr;

/// Lower HIR and attach whole-module call memory effects.
pub fn semantic_lowered(program: &model::Program) -> Result<Vec<Lowered>, String> {
    if program.modules.len() != 1 {
        return Err("native modern compilation currently accepts one module".to_owned());
    }
    let module = &program.modules[0];
    let functions = &module.functions;
    let lowered = hir::lower::lower(program).map_err(|error| error.to_string())?;
    callmemory::annotated(module, functions, &lowered, None)
}

/// Run the common MIR fixed point for one modern-language function.
pub fn optimized(
    program: &model::Program,
    function: &model::Function,
    lowered: &Lowered,
    target: &targets::Profile,
    calls: Option<&IndexMap<i64, String>>,
    options: &Options,
) -> Result<Lowered, String> {
    let module = program
        .modules
        .iter()
        .find(|one| one.functions.contains(function))
        .ok_or_else(|| "StopIteration".to_owned())?;
    let dgroup: BTreeSet<i64> = module
        .data
        .iter()
        .filter(|one| {
            one.linkage == model::DataLinkage::Internal
                && !matches!(one.address, model::AddressKind::Far | model::AddressKind::Huge)
        })
        .map(|one| one.id)
        .collect();
    let owned;
    let calls = match calls {
        Some(calls) => calls,
        None => {
            owned = lowered
                .body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .filter(|operation| operation.kind == Kind::Call)
                .map(|operation| (operation.at, operation.name.clone()))
                .collect::<IndexMap<i64, String>>();
            &owned
        }
    };
    let body = flow::optimized(
        &Rc::new(lowered.body.clone()),
        &dgroup,
        calls,
        ProfileOrName::Profile(target),
        options.clone(),
        None,
        None,
        None,
        None,
    )?;
    Ok(Lowered { body: MirBody::clone(&body), ..lowered.clone() })
}

// DOS C symbols carry one leading underscore. Runtime builtins already
// use their compact, mangled ABI names (for example `_pt`).
fn object_name(name: &str) -> String {
    if name.starts_with("__") { name.to_owned() } else { format!("_{name}") }
}

fn _checked(error: flow::Checked) -> String {
    match error {
        flow::Checked::Refused(message) => message,
        flow::Checked::Malformed(malformed) => malformed.0,
    }
}

/// Lower one modern module to allocated machine form.
///
/// The named entry is public for the runtime to call. Source procedures use
/// one uniform far-call ABI for now; the runtime may live in another code
/// segment, and parameter layout must not depend on which caller reached a
/// procedure.
pub fn assembled(
    program: &model::Program,
    entry: &str,
    cpu: ProfileOrName<'static>,
    options: &Options,
) -> Result<masm::Module, String> {
    if program.modules.len() != 1 {
        return Err("native modern compilation currently accepts one module".to_owned());
    }
    let module = &program.modules[0];
    let target = targets::profile(cpu)?;
    let semantic = semantic_lowered(program)?;
    let mut procedures: Vec<masm::Procedure> = Vec::new();
    let mut referenced: IndexMap<String, String> = IndexMap::default();
    let source_names: BTreeSet<&str> = module.functions.iter().map(|function| function.name.as_str()).collect();

    assert_eq!(module.functions.len(), semantic.len(), "zip(strict=True)");
    for (function, lowered) in module.functions.iter().zip(&semantic) {
        let lowered = optimized(program, function, lowered, target, None, options)?;
        let mut physical = physicalize(program, function, &lowered).map_err(|error| error.to_string())?;
        physical.lowered =
            optimized(program, function, &physical.lowered, target, Some(&physical.calls), options)?;
        // Rotation is deliberately after the scalar fixed point: counted-loop
        // analyses need the canonical pre-tested form, while final machine
        // lowering wants a proven nonempty loop entered at its body so the
        // latch step can provide the branch flags.
        let rotated = rotate::entered(&Rc::new(physical.lowered.body.clone())).map_err(|error| error.to_string())?;
        physical.lowered.body = MirBody::clone(&rotated);
        let legalized = lower_int64::expanded(
            &physical.lowered.body,
            Some(&physical.calls),
            Some(&physical.contracts),
            Some(&physical.hints),
        )
        .map_err(|error| error.0)?;
        let low = lower::lowered(
            &physical.lowered.name,
            &legalized.body,
            Some(&legalized.calls),
            BTreeSet::new(),
            Some(&legalized.contracts),
            ProfileOrName::Profile(target),
            lower::Lowered {
                hints: Some(&legalized.hints),
                pointer_model: Some(physical.pointer_model.clone()),
                ..Default::default()
            },
        )
        .map_err(|error| error.0)?;
        let mut body = flow::verified(low, "lower", true).map_err(|error| error.0)?;
        let owned_frame = frames::of(&body, Some(&legalized.calls), "", None).map_err(|error| error.0)?;
        let owned_frame = Rc::new(RefCell::new(owned_frame));
        let mut in_ssa = true;
        let pinned = body.pins.clone();
        let mut phases = flow::machine(
            &pinned,
            Some(Rc::clone(&owned_frame)),
            Some(&legalized.calls),
            false,
            ProfileOrName::Profile(target),
        )?;
        for phase in phases.iter_mut() {
            if phase.class_name() == "Prologue" {
                continue;
            }
            if phase.class_name() == "PhiElimination" {
                in_ssa = false;
            }
            body = flow::checked(body, phase.as_mut(), in_ssa).map_err(_checked)?;
        }

        let body = addressvalues::converted(&body);

        let reserve = {
            let frame = owned_frame.borrow();
            -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor)
        };
        let mut callees: IndexMap<i64, masm::Callee> = IndexMap::default();
        for (at, name) in &legalized.calls {
            if let Some(inline) = legalized.inline.get(at) {
                let code = inline.iter().map(|one| masm::InlinePart::Bytes(one.clone())).collect();
                callees.insert(*at, masm::Callee { name: name.clone(), far: false, code });
                continue;
            }
            let far = physical.far_calls.contains(at);
            let linked_name = if source_names.contains(name.as_str()) { object_name(name) } else { name.clone() };
            callees.insert(*at, masm::Callee::new(linked_name.clone(), far));
            referenced.insert(linked_name, if far { "far" } else { "near" }.to_owned());
        }

        let is_entry = function.name == entry;
        let linked_name = object_name(&function.name);
        let procedure = masm::Procedure {
            name: linked_name.clone(),
            public: is_entry,
            far: true,
            body,
            reserve,
            callees: callees.clone(),
        };
        let overhead = masm::return_overhead_bytes(&procedure).map_err(|error| error.to_string())? as i64;
        let body = jumps::duplicated_returns(procedure.body, overhead);
        procedures.push(masm::Procedure { name: linked_name, public: is_entry, far: true, body, reserve, callees });
    }

    let linked_entry = object_name(entry);
    if !procedures.iter().any(|procedure| procedure.name == linked_entry) {
        return Err(format!("entry function {} does not exist", pyrepr::string(entry)));
    }

    let defined: BTreeSet<&str> = procedures.iter().map(|procedure| procedure.name.as_str()).collect();
    let mut externs: Vec<(String, String)> = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, distance)| (name.clone(), distance.clone()))
        .collect();
    externs.sort();
    let names: IndexMap<(Space, i64), String> =
        module.data.iter().map(|item| ((Space::Segment, item.id), format!("{}$D{}", module.name, item.id))).collect();
    let data = vec![(
        "_DATA".to_owned(),
        module
            .data
            .iter()
            .flat_map(|item| {
                [
                    masm::Datum::Label(masm::Label { name: names[&(Space::Segment, item.id)].clone() }),
                    masm::Datum::Bytes(item.bytes.iter().map(|one| *one as u8).collect()),
                ]
            })
            .collect(),
    )];
    Ok(masm::Module {
        code: format!("{}_TEXT", module.name.to_uppercase()),
        names,
        externs,
        publics: vec![linked_entry],
        data,
        procedures,
        private: BTreeSet::new(),
    })
}

/// Compile a modern program directly to an OMF object.
pub fn written(program: &model::Program, entry: &str, source: &Path, options: &Options) -> Result<Vec<u8>, String> {
    let name = source.file_name().map(|one| one.to_string_lossy().into_owned()).unwrap_or_default();
    omfwrite::written(&assembled(program, entry, ProfileOrName::Name("386"), options)?, &name)
        .map_err(|error| error.to_string())
}

