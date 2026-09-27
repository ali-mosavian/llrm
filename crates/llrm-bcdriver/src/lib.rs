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

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_core::abi::machine::{self, Machine};
use llrm_core::driver::{self, basic};
use llrm_core::hir::model::RuntimeProfile;
use llrm_mir::GlobalId;
use llrm_mir::program::SegmentLayout;
use llrm_omf::module::{self as found_module, Family};
use llrm_omf::omf::{self, Record};

/// The module header's size, and where the runtime enters the module.
const HEADER_BYTES: usize = 0x30;

/// The header word naming the statement table, which is written afresh.
const STATEMENTS: usize = 10;
/// The header word holding the main body's frame size, which the main
/// body's own frame now carries.
const MAIN_FRAME: usize = 0x22;
/// Where BC keeps its constants.
const CONSTANTS: &str = "BC_CN";

/// A program: its BC objects, by file name, in link order, and the machine
/// it runs on.
pub struct Program<'p> {
    pub modules: Vec<(String, &'p [u8])>,
    pub machine: &'p Machine,
}

/// Each module of `program` compiled again, in its order.
pub fn program(program: &Program) -> Result<Vec<Vec<u8>>, String> {
    let mut parsed = Vec::new();
    for (name, data) in &program.modules {
        let records = omf::parse(data).map_err(|error| format!("{name}: {}", error.0))?;
        let found = found_module::of(&records).ok_or_else(|| format!("{name}: the module has no code segment"))?;
        parsed.push((name, records, found));
    }
    let segments = llrm_bc::segments(parsed.iter().map(|(_, _, found)| found));
    parsed.iter().map(|(name, records, found)| recompiled(records, found, &segments, program.machine, name).map_err(|why| format!("{name}: {why}"))).collect()
}

/// `data`, a program of one BC object, compiled again for `cpu`.
pub fn compiled(data: &[u8], cpu: &str, name: &str) -> Result<Vec<u8>, String> {
    let machine = on(cpu);
    let mut written = program(&Program { modules: vec![(name.to_owned(), data)], machine: &machine })?;
    Ok(written.remove(0))
}

/// The built-in machine with its code priced for `cpu`.
fn on(cpu: &str) -> Machine {
    Machine { cpu: cpu.to_owned(), ..machine::BUILT_IN.clone() }
}

/// One module of a program whose segments `layout` lays out, compiled again.
fn recompiled(records: &[Rc<Record>], found: &found_module::Module, layout: &SegmentLayout, machine: &Machine, name: &str) -> Result<Vec<u8>, String> {
    let segments = omf::segments(records);
    let records = records.to_vec();
    let llrm_bc::Raised { module, runtime, placement, .. } = llrm_bc::raise_in(found, machine, layout).map_err(|refusal| refusal.to_string())?;
    let (code_segment, code, _) = omf::code_segment(&records).ok_or("the module has no code segment")?;
    let named = |id: GlobalId| module.global(id).name.clone().ok_or("an unnamed global");
    let mut symbols = BTreeMap::new();
    for (&segment, &global) in &placement.bases {
        if segment != code_segment {
            return Err(format!("data points into segment {segment}, which is neither data nor code"));
        }
        symbols.insert(named(global)?, basic::HEADER.to_owned());
    }
    let data = data_segments(&placement, &segments, code_segment, &named)?;
    let private = data.iter().map(|one| &one.name).filter(|name| name.as_str() == "FDATA" || name.as_str() == "FSL_CONST").cloned().collect();
    let object = basic::Object {
        code,
        header: header(found, &records, code_segment, &segments)?,
        main: "main".to_owned(),
        symbols,
        data: BTreeMap::new(),
        segments: data,
        constants: CONSTANTS.to_owned(),
        private,
        requests: BTreeSet::new(),
        frames: BTreeMap::new(),
    };
    let family = match found_module::family(&records) {
        Family::Quickbasic => RuntimeProfile::Qb45,
        Family::Pds => RuntimeProfile::Pds71,
        Family::Vbdos => RuntimeProfile::Vbdos,
        other => return Err(format!("a {other:?} object")),
    };
    basic::lifted(module, runtime, &object, family, layout, &driver::Options::of(machine.clone()), name)
}

/// Each data segment the object had, in its order, holding its objects in
/// their order, each where the object put it.
fn data_segments(
    placement: &llrm_bc::Placement,
    segments: &[Option<(String, i64)>],
    code_segment: i64,
    named: &dyn Fn(GlobalId) -> Result<String, &'static str>,
) -> Result<Vec<basic::Segment>, String> {
    let mut placed: BTreeMap<i64, Vec<(i64, GlobalId)>> = BTreeMap::new();
    for &(segment, start, global) in &placement.objects {
        placed.entry(segment).or_default().push((start, global));
    }
    let mut out = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let index = index as i64;
        let Some((name, size)) = segment else { continue };
        if index == code_segment || name.starts_with("$$") {
            continue;
        }
        let mut items = Vec::new();
        for &(start, global) in placed.get(&index).map(Vec::as_slice).unwrap_or_default() {
            items.push(basic::Item::Global { name: named(global)?, at: Some(start) });
        }
        out.push(basic::Segment { name: name.clone(), items, size: Some(*size) });
    }
    Ok(out)
}

/// The module header, MODULE_CODE: BC's own bytes, but for the words
/// `written_basic` writes afresh. Refuses a header relocated other than as
/// BASIC relocates it.
fn header(found: &found_module::Module, records: &[Rc<omf::Record>], code_segment: i64, segments: &[Option<(String, i64)>]) -> Result<Vec<u8>, String> {
    let mut bytes = found.code.get(..HEADER_BYTES).ok_or("no module header")?.to_vec();
    for fixup in omf::fixups(records).into_iter().filter(|one| one.seg == Some(code_segment) && (one.offset as usize) < HEADER_BYTES) {
        let at = fixup.offset as usize;
        let named = segments.get(fixup.index as usize).and_then(Option::as_ref).map(|(name, _)| name.as_str());
        let expected = basic::NAMED.iter().any(|&(word, segment, _)| word == at && fixup.target == "segment" && named == Some(segment));
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
        let machine = on(&cpu);
        let written = program(&Program { modules: read.iter().map(|(name, data)| (name.clone(), data.as_slice())).collect(), machine: &machine })?;
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
