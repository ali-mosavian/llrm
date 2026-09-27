//! The rich route for BC objects, as `llc` takes a module to an object:
//! the object raised onto MIR by llrm-bc, optimized by the MIR pipeline,
//! each function selected by isel and run through the machine phases, and
//! a fresh BASIC object written around the code.
//!
//! A recompile, not a rewrite: every function is emitted again, and one
//! the raise refuses fails the whole program, with nothing written.
//!
//! A program is its modules as LINK takes them: DGROUP is the linked one,
//! and the machine is the program's, both handed in rather than read from
//! a global.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_core::abi::qb::HirAbi;
use llrm_core::backend::assemble::{self, Abi, Target};
use llrm_core::backend::constpool::Pool;
use llrm_core::backend::cpu::{self, Profile, ProfileOrName};
use llrm_core::backend::{addressvalues, globals, masm};
use llrm_core::hir::model::RuntimeProfile;
use llrm_core::model::ir::Space;
use llrm_core::support::hash::IndexMap;
use llrm_mir::{GlobalId, GlobalKind, Module};
use llrm_omf::module::{self as found_module, Family};
use llrm_omf::omf::{self, Record};
use llrm_qb::compile;

/// The main body's symbol, as the QB route names it.
const MAIN: &str = "$QB$MAIN";
/// The code segment's first byte: the module header.
const HEADER: &str = "$QB$HEADER";
/// The module header's size, and where the runtime enters the module.
const HEADER_BYTES: usize = 0x30;

/// Each segment the module header names, by the word that names it and
/// the label `compile::written_basic` fixes it up to.
const NAMED: [(usize, &str, &str); 5] =
    [(12, "BC_DS", "$QB$DS"), (14, "BC_DATA", "$QB$DATA"), (16, "BC_FT", "$QB$FT"), (24, "COMMON", "$QB$COMMON"), (32, "BC_CN", "$QB$CN")];
/// The header word naming the statement table, which is written afresh.
const STATEMENTS: usize = 10;
/// The header word holding the main body's frame size, which the main
/// body's own frame now carries.
const MAIN_FRAME: usize = 0x22;
/// The group BASIC's near data is in.
const DGROUP: &str = "DGROUP";
/// Where BC keeps its constants.
const CONSTANTS: &str = "BC_CN";

/// A program: its BC objects, by file name, in link order, and the CPU it
/// runs on.
pub struct Program<'p> {
    pub modules: Vec<(String, &'p [u8])>,
    pub cpu: &'p Profile,
}

/// Each module of `program` compiled again, in its order.
pub fn program(program: &Program) -> Result<Vec<Vec<u8>>, String> {
    let mut parsed = Vec::new();
    for (name, data) in &program.modules {
        let records = omf::parse(data).map_err(|error| format!("{name}: {}", error.0))?;
        let found = found_module::of(&records).ok_or_else(|| format!("{name}: the module has no code segment"))?;
        parsed.push((name, records, found));
    }
    let dgroup = linked_dgroup(parsed.iter().map(|(_, records, found)| (records.as_slice(), found)));
    parsed.iter().map(|(name, records, found)| recompiled(records, found, &dgroup, program.cpu, name).map_err(|why| format!("{name}: {why}"))).collect()
}

/// `data`, a program of one BC object, compiled again for `cpu`.
pub fn compiled(data: &[u8], cpu: &str, name: &str) -> Result<Vec<u8>, String> {
    let cpu = cpu::profile(ProfileOrName::Name(cpu))?;
    let mut written = program(&Program { modules: vec![(name.to_owned(), data)], cpu })?;
    Ok(written.remove(0))
}

/// The segments LINK puts in DGROUP: each module's GRPDEF names some.
fn linked_dgroup<'r>(modules: impl Iterator<Item = (&'r [Rc<Record>], &'r found_module::Module)>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (records, found) in modules {
        let segments = omf::segments(records);
        names.extend(found.dgroup.members.iter().filter_map(|&index| segments.get(index as usize).cloned().flatten().map(|(name, _)| name)));
    }
    names
}

/// One module of a program whose DGROUP is `dgroup`, compiled again.
fn recompiled(records: &[Rc<Record>], found: &found_module::Module, dgroup: &BTreeSet<String>, profile: &Profile, name: &str) -> Result<Vec<u8>, String> {
    // The raise reads DGROUP from the module's own GRPDEF: a segment another
    // module groups would be carved as far data here.
    let segments = omf::segments(records);
    for (index, one) in segments.iter().enumerate() {
        if let Some((segment, _)) = one {
            if dgroup.contains(segment) && !found.dgroup.members.contains(&(index as i64)) {
                return Err(format!("{segment} is in the program's DGROUP but not in this module's"));
            }
        }
    }
    let records = records.to_vec();
    let (mut module, placement) = llrm_bc::raise_placed(found).map_err(|refusal| refusal.to_string())?;
    let applied = llrm_transforms::pipeline::Applied {
        target: Some(Rc::new(llrm_cycles::target::Dos::priced(&profile._costs, profile.prefix_cost, profile.register_capacity, profile.call_register_capacity))),
        dump: std::env::var_os("LLRM_MIR_STAGES").map(Into::into),
        ..Default::default()
    };
    llrm_transforms::pipeline::applied(&mut module, &applied)?;
    let errors = llrm_mir::verify::verify(&module);
    if let Some(first) = errors.first() {
        return Err(format!("the pipeline left invalid MIR: {first}"));
    }
    let runtime = match found_module::family(&records) {
        Family::Quickbasic => RuntimeProfile::Qb45,
        Family::Pds => RuntimeProfile::Pds71,
        Family::Vbdos => RuntimeProfile::Vbdos,
        other => return Err(format!("a {other:?} object")),
    };
    let (code_segment, code_name, _) = omf::code_segment(&records).ok_or("the module has no code segment")?;
    let abi = HirAbi { runtime, objects: Default::default() };
    let mut names = globals::names(&module, &|name| abi.linked(name))?;
    names.extend(llrm_core::hir::lower::symbol_names());
    let main = module.named("main").ok_or("no main body")?;
    names.insert((Space::Segment, i64::from(main.0)), MAIN.to_owned());
    for (&segment, &global) in &placement.bases {
        if segment != code_segment {
            return Err(format!("data points into segment {segment}, which is neither data nor code"));
        }
        names.insert((globals::space(&module, global), i64::from(global.0)), HEADER.to_owned());
    }
    let pool = Rc::new(RefCell::new(Pool::new(module.globals.len() as i64)));
    let target = Target { cpu: profile, runtime: runtime.value(), basic: true };
    let mut procedures = Vec::new();
    let mut referenced: BTreeMap<String, bool> = BTreeMap::new();
    // The runtime enters the module right after its header.
    let order = std::iter::once(main).chain((0..module.globals.len() as u32).map(GlobalId).filter(|&id| id != main));
    for id in order {
        let global = module.global(id);
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.clone().unwrap_or_default();
        let procedure = procedure(&module, &name, id, &names, &abi, &pool, &target, runtime)?;
        for callee in procedure.callees.values() {
            referenced.insert(callee.name.clone(), callee.far);
        }
        procedures.push(procedure);
    }
    procedures.push(compile::_statement_procedure(&[]));
    let mut data = data_segments(&module, &placement, &segments, code_segment, &names)?;
    // The constants isel keeps in memory go where BC keeps its own, as the
    // QB route places them.
    let mut pooled = Vec::new();
    for (bytes, id) in pool.borrow().entries() {
        let label = format!("$QB$D{id}");
        names.insert((Space::Segment, id), label.clone());
        pooled.extend([masm::Datum::Object(masm::Label { name: label }), masm::Datum::Bytes(bytes.to_vec())]);
    }
    if !pooled.is_empty() {
        match data.iter_mut().find(|(name, _)| name == CONSTANTS) {
            Some((_, datums)) => datums.extend(pooled),
            None => data.push((CONSTANTS.to_owned(), pooled)),
        }
    }
    let defined: std::collections::BTreeSet<&str> = procedures.iter().map(|one| one.name.as_str()).collect();
    let externs = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, &far)| (name.clone(), if far { "far" } else { "near" }.to_owned()))
        .chain(external_data(&module, &names))
        .collect();
    let private = data.iter().map(|(name, _)| name).filter(|name| name.as_str() == "FDATA" || name.as_str() == "FSL_CONST").cloned().collect();
    let assembled = masm::Module {
        code: code_name,
        names,
        externs,
        publics: procedures.iter().filter(|one| one.public).map(|one| one.name.clone()).collect(),
        data,
        procedures,
        private,
        requests: Default::default(),
    };
    let header = header(found, &records, code_segment, &segments)?;
    compile::written_basic(&assembled, header, name).map_err(|error| error.to_string())
}

/// A defined function selected, through the machine phases, and framed as
/// BASIC frames it: a procedure by B$ENRA and B$EXSA, the main body by
/// them too where it needs any frame at all.
#[allow(clippy::too_many_arguments)]
fn procedure(
    module: &Module,
    name: &str,
    id: GlobalId,
    names: &IndexMap<(Space, i64), String>,
    abi: &HirAbi,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
    runtime: RuntimeProfile,
) -> Result<masm::Procedure, String> {
    let contracts = |callee: &str, pops: bool, pushed: i64| abi.contract(callee, pops, pushed);
    let machined = assemble::machined(module, name, &contracts, pool, target)?;
    let finalized = llrm_qb::inline_x87::finalized(&machined.body, machined.popped)?;
    let mut callees = finalized.callees;
    let is_main = names[&(Space::Segment, i64::from(id.0))] == MAIN;
    let (body, framed) = if is_main && machined.reserve == 0 {
        compile::_initialize_frame(&finalized.body, 0).map_err(|error| error.to_string())?
    } else {
        compile::_runtime_frame(&finalized.body, machined.reserve, runtime, 0).map_err(|error| error.to_string())?
    };
    callees.extend(framed);
    for (at, callee) in &machined.calls {
        let linked = match module.named(callee) {
            Some(one) => names[&(globals::space(module, one), i64::from(one.0))].clone(),
            None => abi.linked(callee),
        };
        callees.insert(*at, masm::Callee::new(linked, machined.far.contains(at)));
    }
    let global = module.global(id);
    Ok(masm::Procedure {
        name: names[&(Space::Segment, i64::from(id.0))].clone(),
        public: !is_main && global.linkage == llrm_mir::Linkage::External,
        far: true,
        body: addressvalues::converted(&body),
        // B$ENRA reserves the frame; masm's own shell reserves nothing.
        reserve: 0,
        callees,
        interrupt: None,
    })
}

/// Each data segment the object had, in its order, holding its objects'
/// data in their order; a segment the module header names starts with the
/// label `written_basic` fixes that word up to.
fn data_segments(
    module: &Module,
    placement: &llrm_bc::Placement,
    segments: &[Option<(String, i64)>],
    code_segment: i64,
    names: &IndexMap<(Space, i64), String>,
) -> Result<Vec<(String, Vec<masm::Datum>)>, String> {
    let mut placed: BTreeMap<i64, Vec<(i64, GlobalId)>> = BTreeMap::new();
    for &(segment, start, global) in &placement.objects {
        placed.entry(segment).or_default().push((start, global));
    }
    let near: BTreeSet<String> = (0..module.globals.len() as u32)
        .map(GlobalId)
        .filter(|&id| module.global(id).address_space == 0 && matches!(module.global(id).kind, GlobalKind::Variable(_)))
        .filter_map(|id| names.get(&(globals::space(module, id), i64::from(id.0))).cloned())
        .collect();
    let mut out = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let index = index as i64;
        let Some((name, size)) = segment else { continue };
        if index == code_segment || name.starts_with("$$") {
            continue;
        }
        let mut items = Vec::new();
        if let Some(&(_, _, label)) = NAMED.iter().find(|(_, segment, _)| segment == name) {
            items.push(masm::Datum::Label(masm::Label { name: label.to_owned() }));
        }
        let mut at = 0;
        for &(start, global) in placed.get(&index).map(Vec::as_slice).unwrap_or_default() {
            if start != at {
                return Err(format!("{name} has a gap at {at:#x}"));
            }
            let datums: Vec<masm::Datum> = globals::datums(module, global, names)?
                .into_iter()
                .flat_map(|datum| match datum {
                    // A far pointer to DGROUP data is DGROUP-relative in both
                    // halves, as BC writes an array descriptor's: its own
                    // segment's selector with the group-relative offset
                    // BASIC's code adds to shifts every access (arrprm).
                    masm::Datum::Pointer(masm::Pointer { name, offset, far: true }) if near.contains(&name) => {
                        vec![masm::Datum::Pointer(masm::Pointer { name, offset, far: false }), masm::Datum::SegmentWord(DGROUP.to_owned())]
                    }
                    other => vec![other],
                })
                .collect();
            let bytes: i64 = datums.iter().map(size_of).sum();
            for datum in &datums {
                if let masm::Datum::Pointer(masm::Pointer { name: target, offset, .. }) = datum {
                    if target == HEADER && *offset != 0 {
                        return Err(format!("{name} names code at {offset:#x}, which the recompile moves"));
                    }
                }
            }
            items.extend(datums);
            at += bytes;
        }
        if at != *size {
            return Err(format!("{name} holds {at:#x} of its {size:#x} bytes"));
        }
        out.push((name.clone(), items));
    }
    Ok(out)
}

/// The bytes a datum occupies.
fn size_of(datum: &masm::Datum) -> i64 {
    match datum {
        masm::Datum::Bytes(bytes) => bytes.len() as i64,
        masm::Datum::Pointer(pointer) => if pointer.far { 4 } else { 2 },
        masm::Datum::SegmentWord(_) => 2,
        masm::Datum::Fill(fill) => fill.size,
        _ => 0,
    }
}

/// Each variable the module names but does not define.
fn external_data(module: &Module, names: &IndexMap<(Space, i64), String>) -> Vec<(String, String)> {
    module
        .globals
        .iter()
        .enumerate()
        .filter(|(_, global)| matches!(&global.kind, GlobalKind::Variable(variable) if variable.initializer.is_none()))
        .filter_map(|(at, _)| names.get(&(Space::External, at as i64)).filter(|name| name.as_str() != HEADER).map(|name| (name.clone(), "byte".to_owned())))
        .collect()
}

/// The module header, MODULE_CODE: BC's own bytes, but for the words
/// `written_basic` writes afresh. Refuses a header relocated other than as
/// BASIC relocates it.
fn header(found: &found_module::Module, records: &[Rc<omf::Record>], code_segment: i64, segments: &[Option<(String, i64)>]) -> Result<Vec<u8>, String> {
    let mut bytes = found.code.get(..HEADER_BYTES).ok_or("no module header")?.to_vec();
    for fixup in omf::fixups(records).into_iter().filter(|one| one.seg == Some(code_segment) && (one.offset as usize) < HEADER_BYTES) {
        let at = fixup.offset as usize;
        let named = segments.get(fixup.index as usize).and_then(Option::as_ref).map(|(name, _)| name.as_str());
        let expected = NAMED.iter().any(|&(word, segment, _)| word == at && fixup.target == "segment" && named == Some(segment));
        let statements = at == STATEMENTS && fixup.target == "segment" && fixup.index == code_segment;
        if !expected && !statements {
            return Err(format!("the module header's word at {at:#x} is relocated as BASIC does not"));
        }
        // The label a word is fixed up to starts its segment: the offset
        // into it (OF_DS is BC_DS + 2) goes in the word, as LINK adds to it.
        if expected {
            let word = u16::from_le_bytes([bytes[at], bytes[at + 1]]).wrapping_add(fixup.disp as u16);
            bytes[at..at + 2].copy_from_slice(&word.to_le_bytes());
        }
    }
    bytes[STATEMENTS..STATEMENTS + 2].fill(0);
    bytes[MAIN_FRAME..MAIN_FRAME + 2].fill(0);
    Ok(bytes)
}

/// `llrm-omf --rich`: `OBJ... [LIB...] -o OUT [--manifest M] [--cpu CPU]`.
/// The objects are one program; OUT is the object, or a directory for
/// several. A library is the runtime, which is not recompiled.
pub fn main(argv: &[String]) -> i32 {
    let mut inputs = Vec::new();
    let (mut output, mut manifest, mut cpu) = (None, None, "386".to_owned());
    let mut arguments = argv.iter();
    while let Some(one) = arguments.next() {
        match one.as_str() {
            "-o" | "--output" => output = arguments.next().cloned(),
            "--manifest" => manifest = arguments.next().cloned(),
            "--cpu" => cpu = arguments.next().cloned().unwrap_or(cpu),
            "--rich" | "--dry-run" => {}
            other if other.starts_with('-') => {}
            other if other.to_ascii_lowercase().ends_with(".lib") => {}
            other => inputs.push(std::path::PathBuf::from(other)),
        }
    }
    let Some(output) = output.filter(|_| !inputs.is_empty()) else {
        eprintln!("llrm-omf --rich: OBJ... [LIB...] -o OUT [--manifest M] [--cpu CPU]");
        return 2;
    };
    let read: Result<Vec<(String, Vec<u8>)>, String> = inputs
        .iter()
        .map(|path| {
            let name = path.file_name().map_or_else(String::new, |one| one.to_string_lossy().into_owned());
            std::fs::read(path).map(|data| (name, data)).map_err(|error| format!("{}: {error}", path.display()))
        })
        .collect();
    let written = read.and_then(|read| {
        let cpu = cpu::profile(ProfileOrName::Name(&cpu))?;
        let written = program(&Program { modules: read.iter().map(|(name, data)| (name.clone(), data.as_slice())).collect(), cpu })?;
        Ok(read.into_iter().map(|(name, _)| name).zip(written).collect::<Vec<_>>())
    });
    let written = match written {
        Ok(written) => written,
        Err(why) => {
            eprintln!("llrm-omf: {why}");
            return 1;
        }
    };
    let single = written.len() == 1;
    for (name, bytes) in written {
        let path = if single { std::path::PathBuf::from(&output) } else { std::path::Path::new(&output).join(name) };
        if let Err(error) = std::fs::write(&path, bytes) {
            eprintln!("llrm-omf: {}: {error}", path.display());
            return 1;
        }
    }
    if let Some(manifest) = manifest {
        let _ = std::fs::write(manifest, "{\"route\": \"rich\"}\n");
    }
    0
}
