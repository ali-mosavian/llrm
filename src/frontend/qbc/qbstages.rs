//! Port of `tools/qbstages.py`: dump every implemented QB source-frontend
//! stage to adjacent text files. `llrm-qb --dump DIR` writes this tree.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use super::compile::{self as qb_compile, Stage, StageValue};
use super::driver::parsed;
use super::stage_text;
use crate::backend::masm;
use crate::hir::{codec, dump, model};
use crate::model::ir::{Loc, Operation};
use crate::model::lir;
use crate::model::passes::Options;
use crate::objectfile::module::Space;

/// Decode the physical source exactly as qbfront does before lexing.
pub(super) fn _source_text(path: &Path) -> Result<String, String> {
    let raw = std::fs::read(path).map_err(|error| error.to_string())?;
    let raw = raw.split(|byte| *byte == 0x1a).next().unwrap_or_default();
    Ok(match std::str::from_utf8(raw) {
        Ok(text) => text.to_owned(),
        Err(_) => raw.iter().map(|byte| if byte.is_ascii() { char::from(*byte) } else { CP437[usize::from(*byte - 0x80)] }).collect(),
    })
}

/// Python's `cp437` codec above 0x7F.
const CP437: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}',
    '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}',
    '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}',
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}',
    '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}', '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}',
    '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}',
    '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}',
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}',
    '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}', '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}',
    '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}',
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}',
];

pub(super) fn _lir(body: &lir::LirBody, callees: Option<&IndexMap<i64, masm::Callee>>) -> String {
    let empty = IndexMap::new();
    let callees = callees.unwrap_or(&empty);
    let mut lines = vec![format!("; entry L0_{}", body.entry), format!("{} proc", body.name)];
    for block in &body.blocks {
        let successors = block.succ.iter().map(|one| format!("L0_{one}")).collect::<Vec<_>>().join(", ");
        let successors = if successors.is_empty() { "return".to_owned() } else { successors };
        lines.push(format!("L0_{}: ; successors: {successors}", block.at));
        for phi in &block.phis {
            let incoming = phi.incoming.iter().map(|(at, value)| format!("L0_{at}:v{value}")).collect::<Vec<_>>().join(", ");
            lines.push(format!("    ; phi v{} <- {incoming}", phi.result));
        }
        for one in &block.insns {
            let callee = callees.get(&one.at);
            if let Some(callee) = callee {
                if !callee.code.is_empty() && one.what.as_ref().is_some_and(|what| what.op == Operation::Call) {
                    lines.push(format!("    ; inline {} at {}", callee.name, one.at));
                    lines.extend(stage_text::inline_text(&callee.code).into_iter().map(|line| format!("    {line}")));
                    continue;
                }
            }
            let Some(what) = &one.what else {
                lines.push(format!("    db ? ; {}: source bytes carried unchanged", one.at));
                continue;
            };
            let rendered = stage_text::instruction_text(what);
            lines.extend(rendered.into_iter().filter(|line| !line.is_empty()).map(|line| format!("    {line} ; {}", one.at)));
        }
    }
    lines.push(format!("{} endp", body.name));
    lines.join("\n") + "\n"
}

/// Return emitted source-global names with their exact zero-fill extent.
///
/// Diagnostic-only: the frontend owns BASIC's spelling of source globals,
/// while the shared MASM writer owns the bytes.
fn _source_globals(program: &model::Program, module: &masm::Module) -> IndexMap<String, (i64, String)> {
    let mut globals_ = IndexMap::new();
    for source_module in &program.modules {
        let types: IndexMap<i64, &model::Type> = source_module.types.iter().map(|one| (one.id, one)).collect();
        for function in &source_module.functions {
            for place in &function.places {
                let Some(extent) = place.extent else { continue };
                if place.storage != model::Storage::Module || place.name.starts_with('$') {
                    continue;
                }
                let Some(name) = module.names.get(&(Space::Segment, place.symbol)) else { continue };
                let type_name = types[&place.r#type].name.clone();
                globals_.insert(name.clone(), (extent, type_name));
            }
        }
    }
    globals_
}

/// Spell initialized zero bytes compactly, preserving the emitted bytes.
fn _zero_fill(size: i64) -> String {
    match size {
        1 => "db 0".into(),
        2 => "dw 0".into(),
        4 => "dd 0".into(),
        8 => "dq 0".into(),
        _ => format!("db {size} dup (0)"),
    }
}

/// Render the emitted data model as readable, byte-equivalent MASM.
fn _pretty_preamble(program: &model::Program, module: &masm::Module) -> String {
    let globals_ = _source_globals(program, module);
    let mut out: Vec<String> = vec![".model medium".into(), ".386".into(), String::new()];
    out.extend(module.publics.iter().map(|name| format!("public {name}")));
    out.extend(
        ["", "; --------------------------------------------------------------------------", "; Data", ""]
            .map(str::to_owned),
    );
    for (segment, items) in &module.data {
        let private = module.private.contains(segment);
        if out.last().is_some_and(|last| !last.is_empty()) {
            out.push(String::new());
        }
        out.push(masm::SEGMENTS.get(segment.as_str()).map_or_else(
            || format!("{segment} segment word public '{}'", if private { "FAR_DATA" } else { "DATA" }),
            |one| (*one).to_owned(),
        ));
        out.extend(module.externs.iter().filter(|(_, kind)| kind == "byte").map(|(name, _)| format!("extern {name}:byte")));
        let mut source_heading = false;
        let mut index = 0;
        while index < items.len() {
            let item = &items[index];
            let following = items.get(index + 1);
            if let (masm::Datum::Label(label), Some(masm::Datum::Bytes(following))) = (item, following) {
                if let Some((extent, type_name)) = globals_.get(&label.name) {
                    if following.len() as i64 == *extent && following.iter().all(|byte| *byte == 0) {
                        if !source_heading {
                            out.extend(
                                ["", "    ; QB source globals: BC-compatible effective names", ""].map(str::to_owned),
                            );
                            source_heading = true;
                        }
                        out.push(format!("{:<20} {:<16} ; {type_name}", label.name, _zero_fill(*extent)));
                        index += 2;
                        continue;
                    }
                }
            }
            match item {
                masm::Datum::Bytes(bytes) if bytes.len() >= 4 && bytes.iter().all(|byte| *byte == 0) => {
                    out.push(format!("    {}", _zero_fill(bytes.len() as i64)));
                }
                masm::Datum::Bytes(_) => out.extend(masm::datum(item).into_iter().map(|line| format!("    {line}"))),
                _ => out.extend(masm::datum(item)),
            }
            index += 1;
        }
        if !masm::SEGMENTS.contains_key(segment.as_str()) {
            out.push(format!("{segment} ends"));
            if !private {
                out.push(format!("DGROUP group {segment}"));
            }
        }
    }
    out.extend(module.externs.iter().filter(|(_, kind)| kind != "byte").map(|(name, kind)| {
        format!("extern {name}:{}", if kind == "far-byte" { "byte" } else { kind })
    }));
    out.extend([
        String::new(),
        "; --------------------------------------------------------------------------".to_owned(),
        "; Code".to_owned(),
        format!(".code {}", module.code),
    ]);
    out.join("\n") + "\n"
}

fn word_character(one: char) -> bool {
    one.is_alphanumeric() || one == '_'
}

/// `re.finditer(r"\b(L\d+_\d+)\b", line)`.
fn _labels_in(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let starts = chars[at] == 'L' && (at == 0 || !word_character(chars[at - 1]));
        if starts {
            let mut end = at + 1;
            let first = end;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            if end > first && end < chars.len() && chars[end] == '_' {
                end += 1;
                let second = end;
                while end < chars.len() && chars[end].is_ascii_digit() {
                    end += 1;
                }
                if end > second && (end == chars.len() || !word_character(chars[end])) {
                    found.push(chars[at..end].iter().collect());
                    at = end;
                    continue;
                }
            }
        }
        at += 1;
    }
    found
}

/// `re.fullmatch(r"    ([A-Za-z][A-Za-z0-9]*)\s+(.+)", line)`.
fn _mnemonic(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("    ")?;
    if !rest.starts_with(|one: char| one.is_ascii_alphabetic()) {
        return None;
    }
    let name_end = rest.find(|one: char| !one.is_ascii_alphanumeric()).unwrap_or(rest.len());
    let (name, after) = rest.split_at(name_end);
    let operands = after.trim_start();
    // `\s+` needs one space and `.+` one character; backtracking hands the
    // last space to `.+` when nothing else follows.
    if operands.len() == after.len() {
        return None;
    }
    if operands.is_empty() {
        let spaces = after.len();
        if spaces < 2 {
            return None;
        }
        return Some((name, &after[spaces - 1..]));
    }
    Some((name, operands))
}

/// Align instructions and hide only display-only, unreferenced block labels.
pub(super) fn _display_assembly(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let referenced: BTreeSet<String> =
        lines.iter().filter(|line| !line.ends_with(':')).flat_map(|line| _labels_in(line)).collect();
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let heading = line.ends_with(" proc far") || line.ends_with(" proc near");
        let label = line.ends_with(':');
        let name = if label { &line[..line.len() - 1] } else { "" };
        let entry_label = out.last().is_some_and(|last| last.ends_with(" proc far") || last.ends_with(" proc near"));
        if label && name.starts_with('L') && !entry_label && !referenced.contains(name) {
            continue;
        }
        // Keep a procedure's entry label adjacent to its envelope: it makes
        // the runtime frame sequence easy to scan.  Any surviving internal
        // label starts a visually distinct basic block.
        if (heading || (label && !entry_label)) && out.last().is_some_and(|last| !last.is_empty()) {
            out.push(String::new());
        }
        if heading {
            out.push("; --------------------------------------------------------------------------".to_owned());
            out.push(format!("; Procedure: {}", line.split(" proc ").next().unwrap_or(line)));
        }
        let mut line = line.to_owned();
        if line.starts_with("    ") {
            if let Some((name, operands)) = _mnemonic(&line) {
                line = format!("    {name:<8}{operands}");
            }
        }
        out.push(line);
    }
    out.join("\n") + "\n"
}

/// Render the exact return cleanup carried by the emitted assembly model.
///
/// The shared MASM printer spells every far return as bare `retf` even when
/// its semantics carry the immediate the OMF writer encodes; keep this
/// showcase truthful without changing the shared backend.
fn _emitted_asm(program: &model::Program, module: &masm::Module, pretty: bool) -> Result<String, String> {
    let mut cleanup: IndexMap<String, i64> = IndexMap::new();
    for (number, procedure) in module.procedures.iter().enumerate() {
        for item in masm::listing(procedure, number).map_err(|error| error.0)? {
            if let masm::Item::Semantics(item) = item {
                if item.op == Operation::Return {
                    if let Some(Loc::Imm(imm)) = item.sources.first() {
                        cleanup.insert(procedure.name.clone(), imm.value);
                    }
                }
            }
        }
    }

    // masm.text() uses the shared native frame shell. BASIC OMF emission uses
    // _basic_listing(), where B$ENRA/B$EXSA own that shell. Replace each
    // procedure with the listing which object_bytes() actually encodes.
    let mut rendered =
        if pretty { _pretty_preamble(program, module) } else { masm::text(module).map_err(|error| error.0)? };
    if pretty {
        // The shared printer contributes the procedure envelopes below.  Its
        // data preamble has already been replaced by the readable equivalent.
        for (number, procedure) in module.procedures.iter().enumerate() {
            rendered += &(masm::_procedure(procedure, &module.names, number).map_err(|error| error.0)?.join("\n") + "\n");
        }
        rendered += "end\n";
    }
    for (number, procedure) in module.procedures.iter().enumerate() {
        let heading = format!("{} proc {}", procedure.name, if procedure.far { "far" } else { "near" });
        let ending = format!("{} endp", procedure.name);
        let mut lines = vec![heading.clone()];
        for item in qb_compile::_basic_listing(procedure, number).map_err(|error| error.to_string())? {
            match &item {
                masm::Item::Label(masm::Label { name }) => lines.push(format!("{name}:")),
                masm::Item::Callee(masm::Callee { code, .. }) if !code.is_empty() => {
                    lines.extend(masm::_code(code).into_iter().map(|line| format!("    {line}")));
                }
                masm::Item::Callee(masm::Callee { name, far, .. }) => {
                    lines.push(format!("    call {}{name}", if *far { "far ptr " } else { "" }));
                }
                masm::Item::Semantics(what) => lines.extend(
                    masm::_instruction(what, &module.names, number)
                        .map_err(|error| error.0)?
                        .into_iter()
                        .map(|line| format!("    {line}")),
                ),
            }
        }
        lines.push(ending.clone());
        let mut rendered_lines: Vec<String> = rendered.lines().map(str::to_owned).collect();
        let start = rendered_lines.iter().position(|line| *line == heading);
        let stop = start.and_then(|start| {
            rendered_lines[start + 1..].iter().position(|line| *line == ending).map(|found| found + start + 1)
        });
        let (Some(start), Some(stop)) = (start, stop) else {
            return Err(format!("missing exact procedure envelope for {}", procedure.name));
        };
        rendered_lines.splice(start..=stop, lines);
        rendered = rendered_lines.join("\n") + "\n";
    }

    let mut current: Option<String> = None;
    let mut lines = Vec::new();
    for line in rendered.lines() {
        if line.ends_with(" proc far") || line.ends_with(" proc near") {
            current = Some(line.split(" proc ").next().unwrap_or(line).to_owned());
        } else if line.ends_with(" endp") {
            current = None;
        }
        let mut line = line.to_owned();
        if let Some(value) = current.as_ref().and_then(|name| cleanup.get(name)) {
            if line.trim() == "retf" {
                line = format!("    retf {value}");
            }
        }
        lines.push(line);
    }
    let result = lines.join("\n") + "\n";
    Ok(if pretty { _display_assembly(&result) } else { result })
}

/// The frontend options `dumped` takes, as `tools/qbstages.py` spells them.
#[derive(Clone, Debug, Default)]
pub struct Frontend {
    pub dialect: String,
    pub runtime: String,
    pub array_order: String,
    pub huge_arrays: bool,
    pub checked_arrays: bool,
    pub unchecked_bounds: bool,
    pub mbf: bool,
    pub alternate_math: bool,
    pub includes: Vec<PathBuf>,
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn dumped(source: &Path, output: &Path, frontend: &Frontend, options: &Options) -> Result<PathBuf, String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let program = parsed(
        source,
        &frontend.dialect,
        &frontend.runtime,
        None,
        &frontend.includes,
        &frontend.array_order,
        frontend.huge_arrays,
        frontend.checked_arrays,
        frontend.unchecked_bounds,
        frontend.mbf,
        frontend.alternate_math,
    )
    .map_err(|error| error.0)?;
    let functions: Vec<&model::Function> = program.modules.iter().flat_map(|module| &module.functions).collect();
    write(&output.join("00-input.bas"), &_source_text(source)?)?;
    let numbers: IndexMap<*const model::Function, usize> =
        functions.iter().enumerate().map(|(number, function)| (*function as *const _, number + 1)).collect();
    let mut next_machine_stage: IndexMap<*const model::Function, usize> =
        functions.iter().map(|function| (*function as *const _, 8)).collect();

    let mut observe = |event: &Stage| -> Result<(), String> {
        if event.name == "hir" {
            let StageValue::Program(program) = event.value else { unreachable!("hir carries the program") };
            return write(&output.join("01-hir.json"), &codec::encode(program, None).map_err(|error| error.0)?);
        }
        if event.name == "emitted-assembly" {
            // This is the assembly model object_bytes() is about to encode,
            // not a fresh assembled(program) diagnostic reconstruction.
            let StageValue::Module(module) = event.value else { unreachable!("emitted-assembly carries the module") };
            write(&output.join("99-emitted-asm.asm"), &_emitted_asm(&program, module, true)?)?;
            write(&output.join("99-emitted-asm.raw.asm"), &_emitted_asm(&program, module, false)?)?;
            return Ok(());
        }
        let Some(function) = event.function else {
            return Err(format!("stage {} has no source function", event.name));
        };
        let key = function as *const model::Function;
        let number = numbers[&key];
        let stem = format!("{number:02}-{}", function.name);
        let mir = |value: StageValue| match value {
            StageValue::Lowered(lowered) => dump::mir_text(lowered),
            _ => unreachable!("a MIR stage carries a lowered body"),
        };
        fn lir_of<'v>(value: StageValue<'v>) -> &'v lir::LirBody {
            match value {
                StageValue::Lir(body) => body,
                _ => unreachable!("a LIR stage carries a LIR body"),
            }
        }
        let (path, text) = match event.name.as_str() {
            "source-mir" => (output.join(format!("{stem}-02-mir.txt")), mir(event.value)),
            "optimized-mir" => (output.join(format!("{stem}-03-optimized-mir.txt")), mir(event.value)),
            "physical-mir" => (output.join(format!("{stem}-04-physical-mir.txt")), mir(event.value)),
            "optimized-physical-mir" => (output.join(format!("{stem}-05-optimized-physical-mir.txt")), mir(event.value)),
            "rotated-mir" => (output.join(format!("{stem}-06-rotated-mir.txt")), mir(event.value)),
            "initial-lir" => (output.join(format!("{stem}-07-lir.txt")), _lir(lir_of(event.value), None)),
            name if name.starts_with("machine:") => {
                let stage = next_machine_stage[&key];
                next_machine_stage[&key] = stage + 1;
                (
                    output.join(format!("{stem}-{stage:02}-{}.txt", &name["machine:".len()..])),
                    _lir(lir_of(event.value), None),
                )
            }
            "final-lir" => {
                let stage = next_machine_stage[&key];
                (output.join(format!("{stem}-{stage:02}-inline-x87.txt")), _lir(lir_of(event.value), event.callees.as_ref()))
            }
            _ => return Err(format!("unknown QB compiler stage {}", event.name)),
        };
        write(&path, &text)
    };

    qb_compile::object_bytes(&program, source, Some(&mut observe), options).map_err(|error| error.to_string())?;
    Ok(output.to_path_buf())
}
