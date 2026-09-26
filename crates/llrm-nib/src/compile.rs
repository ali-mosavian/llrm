//! Port of `qbopt/frontend/modern/compile.py`: common HIR from the modern
//! frontend to a fresh 16-bit OMF object.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;

use llrm_core::backend::cpu::{self as targets, ProfileOrName};
use llrm_core::backend::{addressvalues, assemble, frame as frames, jumps, lower, lower_int64, masm, omfwrite};
use llrm_core::flow;
use llrm_core::abi::qb::{HirAbi, physicalize};
use llrm_core::hir::lower::{DGROUP, Lowered};
use llrm_core::hir::{self, callmemory, model};
use llrm_core::model::mir::{Kind, MirBody};
use llrm_core::model::passes::Options;
use llrm_core::objectfile::module::{Addr, Space};
use llrm_core::analysis::interprocedural;
use llrm_core::optimize::interprocedural as whole;
use llrm_core::optimize::rotate;
use llrm_core::support::hash::IndexMap;
use llrm_core::support::pyrepr;

/// Lower HIR and attach whole-module call memory effects.
pub fn semantic_lowered(program: &model::Program) -> Result<Vec<Lowered>, String> {
    if program.modules.len() != 1 {
        return Err("native Nib compilation currently accepts one module".to_owned());
    }
    let module = &program.modules[0];
    let functions = &module.functions;
    let lowered = hir::lower::lower(program).map_err(|error| error.to_string())?;
    callmemory::annotated(module, functions, &lowered, None, None)
}

/// Run the common MIR fixed point for one Nib function.
pub fn optimized(
    program: &model::Program,
    function: &model::Function,
    lowered: &Lowered,
    target: &targets::Profile,
    calls: Option<&IndexMap<i64, String>>,
    options: &Options,
) -> Result<Lowered, String> {
    watched(program, function, lowered, target, calls, options, None)
}

/// `optimized`, showing `watch` the body after each pass.
pub fn watched(
    program: &model::Program,
    function: &model::Function,
    lowered: &Lowered,
    target: &targets::Profile,
    calls: Option<&IndexMap<i64, String>>,
    options: &Options,
    watch: Option<&mut dyn FnMut(&str, &MirBody)>,
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
        watch,
    )?;
    Ok(Lowered { body: MirBody::clone(&body), ..lowered.clone() })
}

// DOS C symbols carry one leading underscore. Runtime builtins already
// use their compact, mangled ABI names (for example `N$PS`), and an exported
// function is named by its ABI's symbol.
fn object_name(function: &model::Function) -> String {
    if function.linkage == model::FunctionLinkage::External || function.name.starts_with("__") {
        function.name.clone()
    } else {
        format!("_{}", function.name)
    }
}

fn _checked(error: flow::Checked) -> String {
    match error {
        flow::Checked::Refused(raised) => raised.message,
        flow::Checked::Malformed(malformed) => malformed.0,
    }
}

/// Lower one Nib module to allocated machine form.
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
        return Err("native Nib compilation currently accepts one module".to_owned());
    }
    let module = &program.modules[0];
    let target = targets::profile(cpu)?;
    let semantic = semantic_lowered(program)?;
    let mut procedures: Vec<masm::Procedure> = Vec::new();
    let mut referenced: IndexMap<String, String> = IndexMap::default();
    let linked_names: BTreeMap<&str, String> =
        module.functions.iter().map(|function| (function.name.as_str(), object_name(function))).collect();

    assert_eq!(module.functions.len(), semantic.len(), "zip(strict=True)");
    let mut physicals = Vec::new();
    let mut arguments = Vec::new();
    for (function, lowered) in module.functions.iter().zip(&semantic) {
        let lowered = optimized(program, function, lowered, target, None, options)?;
        let mut physical = physicalize(program, function, &lowered).map_err(|error| error.to_string())?;
        arguments.push(interprocedural::argument_sites(&physical.lowered.body, &physical.contracts));
        physical.lowered =
            optimized(program, function, &physical.lowered, target, Some(&physical.calls), options)?;
        physicals.push(physical);
    }

    // The whole-module step binds formals to the pushes that feed them, so
    // it runs on physical MIR: semantic MIR has neither.
    let public = |function: &model::Function| function.name == entry || function.linkage == model::FunctionLinkage::External;
    let named = |wanted: bool| -> BTreeSet<String> {
        module.functions.iter().filter(|function| public(function) == wanted).map(|function| function.name.clone()).collect()
    };
    let (roots, private) = (named(true), named(false));
    // Call constants come from MIR alone: Nib has no source-time table of them.
    let no_constants = IndexMap::default();
    let summaries: Vec<whole::Procedure> = module
        .functions
        .iter()
        .zip(&physicals)
        .zip(&arguments)
        .map(|((function, physical), arguments)| whole::Procedure {
            name: &function.name,
            calls: &physical.calls,
            parameters: &physical.parameters,
            constants: &no_constants,
            arguments,
        })
        .collect();
    let mut bodies: IndexMap<String, Rc<MirBody>> = module
        .functions
        .iter()
        .zip(&physicals)
        .map(|(function, physical)| (function.name.clone(), Rc::new(physical.lowered.body.clone())))
        .collect();
    let found = whole::optimized::<String>(
        &summaries,
        &mut bodies,
        &private,
        &roots,
        target.cost("call_far")?,
        &mut |index, body, _| {
            let physical = &physicals[index];
            let lowered = Lowered { body: MirBody::clone(body), ..physical.lowered.clone() };
            let body = optimized(program, &module.functions[index], &lowered, target, Some(&physical.calls), options)?;
            Ok(Rc::new(body.body))
        },
        &mut |_, _, _| Ok(()),
    )?;
    drop(summaries);

    for (function, mut physical) in module.functions.iter().zip(physicals) {
        if !found.reachable.contains(&function.name) {
            continue;
        }
        physical.lowered.body = MirBody::clone(&bodies[&function.name]);
        // Rotation is deliberately after the scalar fixed point: counted-loop
        // analyses need the canonical pre-tested form, while final machine
        // lowering wants a proven nonempty loop entered at its body so the
        // latch step can provide the branch flags.
        let rotated = rotate::entered(&Rc::new(physical.lowered.body.clone())).map_err(|error| error.to_string())?;
        physical.lowered.body = MirBody::clone(&rotated);
        let mut legalized = lower_int64::expanded(
            &physical.lowered.body,
            Some(&physical.calls),
            Some(&physical.contracts),
            Some(&physical.hints_for(&physical.lowered.body)),
        )
        .map_err(|error| error.0)?;
        legalized.inline.extend(physical.inline.iter().map(|(at, inline)| (*at, vec![inline.code.clone()])));
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
                terminal: interprocedural::terminal_sites(&legalized.calls, &found.noreturn),
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
        let body = masm::cleaned_returns(&body, function.abi.as_ref().map_or(0, |abi| abi.parameter_bytes))?;

        let reserve = {
            let frame = owned_frame.borrow();
            -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor)
        };
        // The call table keeps sites the optimizer removed; only a surviving call names its callee.
        let sites: BTreeSet<i64> = legalized
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|operation| operation.kind == Kind::Call)
            .map(|operation| operation.at)
            .collect();
        let mut callees: IndexMap<i64, masm::Callee> = IndexMap::default();
        for (at, name) in legalized.calls.iter().filter(|(at, _)| sites.contains(at)) {
            if let Some(inline) = legalized.inline.get(at) {
                let code = inline.iter().map(|one| masm::InlinePart::Bytes(one.clone())).collect();
                callees.insert(*at, masm::Callee { name: name.clone(), far: false, code });
                continue;
            }
            let far = physical.far_calls.contains(at);
            let linked_name = linked_names.get(name.as_str()).cloned().unwrap_or_else(|| name.clone());
            callees.insert(*at, masm::Callee::new(linked_name.clone(), far));
            referenced.insert(linked_name, if far { "far" } else { "near" }.to_owned());
        }

        let public = public(function);
        let linked_name = linked_names[function.name.as_str()].clone();
        let interrupt = function
            .abi
            .as_ref()
            .is_some_and(|abi| abi.distance == model::CallDistance::Interrupt)
            .then_some(Addr { index: DGROUP.1, ..Addr::new(DGROUP.0, 0) });
        let procedure = masm::Procedure {
            name: linked_name.clone(),
            public,
            far: true,
            body,
            reserve,
            callees: callees.clone(),
            interrupt,
        };
        let overhead = masm::return_overhead_bytes(&procedure).map_err(|error| error.to_string())? as i64;
        let body = jumps::duplicated_returns(procedure.body, overhead);
        procedures.push(masm::Procedure { name: linked_name, public, far: true, body, reserve, callees, interrupt });
    }

    // A module without the entry is a library: only its exports are public.
    let library = module.functions.iter().any(|one| one.linkage == model::FunctionLinkage::External);
    if !linked_names.contains_key(entry) && !library {
        return Err(format!("entry function {} does not exist", pyrepr::string(entry)));
    }

    let defined: BTreeSet<&str> = procedures.iter().map(|procedure| procedure.name.as_str()).collect();
    let mut externs: Vec<(String, String)> = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, distance)| (name.clone(), distance.clone()))
        .collect();
    externs.sort();
    let mut names = llrm_core::hir::lower::symbol_names();
    names.extend(module.data.iter().map(|item| ((Space::Segment, item.id), format!("{}$D{}", module.name, item.id))));
    let callables: BTreeMap<i64, &str> = module.callables.iter().map(|one| (one.id, one.name.as_str())).collect();
    let mut data = Vec::new();
    for item in &module.data {
        data.push(masm::Datum::Label(masm::Label { name: names[&(Space::Segment, item.id)].clone() }));
        data.extend(_initialized(item, &names, &|id| {
            let name = callables.get(&id).copied().unwrap_or_default();
            linked_names.get(name).cloned().unwrap_or_else(|| name.to_owned())
        })?);
    }
    let data = vec![("_DATA".to_owned(), data)];
    Ok(masm::Module {
        code: format!("{}_TEXT", module.name.to_uppercase()),
        names,
        externs,
        publics: procedures
            .iter()
            .filter(|one| one.public)
            .map(|one| one.name.clone())
            .collect(),
        data,
        procedures,
        private: BTreeSet::new(),
        requests: BTreeSet::new(),
    })
}

/// The same through the rich MIR: the HIR emitted as MIR, then selected
/// and assembled whole. It runs no MIR passes yet.
pub fn assembled_from_mir(program: &model::Program, entry: &str, cpu: ProfileOrName<'static>) -> Result<masm::Module, String> {
    if program.modules.len() != 1 {
        return Err("native Nib compilation currently accepts one module".to_owned());
    }
    let module = &program.modules[0];
    let emitted = hir::mir::emit(program).swap_remove(0);
    if let Some((name, why)) = emitted.refused.first() {
        return Err(format!("@{name}: {why}"));
    }
    let mut mir = emitted.module;
    llrm_mir::transforms::optimized(&mut mir)?;
    // The entry is public for the runtime to call; a library has none.
    match mir.named(entry) {
        Some(id) => mir.globals[id.0 as usize].linkage = llrm_mir::Linkage::External,
        None if module.functions.iter().any(|one| one.linkage == model::FunctionLinkage::External) => {}
        None => return Err(format!("entry function {} does not exist", pyrepr::string(entry))),
    }
    let objects = module.functions.iter().map(|function| (function.name.clone(), object_name(function))).collect();
    let abi = HirAbi { runtime: program.runtime, objects };
    assemble::assembled(&mir, &abi, &format!("{}_TEXT", module.name.to_uppercase()), cpu)
}

/// `item`'s bytes, each relocated field a pointer to what it names: data,
/// or a callable's code, which `code` names.
fn _initialized(
    item: &model::DataObject,
    names: &IndexMap<(Space, i64), String>,
    code: &dyn Fn(i64) -> String,
) -> Result<Vec<masm::Datum>, String> {
    let bytes = |out: &mut Vec<masm::Datum>, from: i64, to: i64| {
        if from < to {
            out.push(masm::Datum::Bytes(item.bytes[from as usize..to as usize].iter().map(|one| *one as u8).collect()));
        }
    };
    let mut relocations: Vec<&model::DataRelocation> = item.relocations.iter().collect();
    relocations.sort_by_key(|one| one.at);
    let mut out = Vec::new();
    let mut cursor = 0;
    for relocation in relocations {
        let far = matches!(relocation.address, model::AddressKind::Far | model::AddressKind::Huge);
        if far && !relocation.code {
            // One POINTER fixup would pair the offset in DGROUP with the object's own segment.
            return Err(format!("{}: a far pointer to data is not supported", item.name));
        }
        let name = if relocation.code { code(relocation.target) } else { names[&(Space::Segment, relocation.target)].clone() };
        bytes(&mut out, cursor, relocation.at);
        out.push(masm::Datum::Pointer(masm::Pointer { name, offset: relocation.addend, far }));
        cursor = relocation.at + if far { 4 } else { 2 };
    }
    bytes(&mut out, cursor, item.bytes.len() as i64);
    Ok(out)
}

/// Makes each export no symbol in `used` names internal, so that it and
/// what only it calls are dropped: a linker's own elimination keeps
/// whatever any segment references, even one it drops.
pub fn keep_exports(program: &mut model::Program, used: &BTreeSet<String>) {
    for function in program.modules.iter_mut().flat_map(|module| module.functions.iter_mut()) {
        if function.linkage == model::FunctionLinkage::External && !used.contains(&function.name) {
            function.linkage = model::FunctionLinkage::Internal;
        }
    }
}

/// Compile a Nib program directly to an OMF object.
/// The processor objects are compiled for.
pub const CPU: &str = "486";

pub fn written(program: &model::Program, entry: &str, source: &Path, options: &Options) -> Result<Vec<u8>, String> {
    written_as(program, entry, source, options, omfwrite::CodeLayout::OneSegment)
}

/// The same, its code laid out as `layout` says: a segment per procedure
/// lets a linker that drops unreferenced segments keep only what is
/// called; every Nib call is far, so that is safe.
pub fn written_as(program: &model::Program, entry: &str, source: &Path, options: &Options, layout: omfwrite::CodeLayout) -> Result<Vec<u8>, String> {
    _object(&assembled(program, entry, ProfileOrName::Name(CPU), options)?, source, layout)
}

pub fn written_from_mir(program: &model::Program, entry: &str, source: &Path, layout: omfwrite::CodeLayout) -> Result<Vec<u8>, String> {
    _object(&assembled_from_mir(program, entry, ProfileOrName::Name(CPU))?, source, layout)
}

fn _object(module: &masm::Module, source: &Path, layout: omfwrite::CodeLayout) -> Result<Vec<u8>, String> {
    let name = source.file_name().map(|one| one.to_string_lossy().into_owned()).unwrap_or_default();
    omfwrite::written_as(module, &name, layout).map_err(|error| error.to_string())
}

