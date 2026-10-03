//! The BASIC runtime's module object, as BC writes one: B$ENRA and B$EXSA
//! frames, the statement table, the 30h MODULE_CODE header before the code,
//! BASIC's segments and classes, and the final spelling of the x87 pseudos
//! isel leaves.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;
use llrm_mir::program::SegmentLayout;
use llrm_mir::{GlobalId, GlobalKind, Module};

use super::Options;
use crate::abi::qb::HirAbi;
use crate::backend::assemble::{self, Abi, Target};
use crate::backend::constpool::Pool;
use crate::backend::target::Segments;
use crate::backend::{addressvalues, codeview, globals, isel, masm, omfwrite};
use crate::hir::model;
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir;
use crate::objectfile::module::{Addr, Space};
use crate::objectfile::omf;
use crate::support::hash::{IndexMap, IndexSet};
use crate::support::pyrepr::Repr;

/// `struct.pack_into("<H", buffer, at, value)`.
fn pack_into(buffer: &mut [u8], at: usize, value: i64) {
    buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

pub fn _insn(at: i64, what: Semantics) -> Arc<lir::Insn> {
    Arc::new(lir::Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

pub fn _semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

pub fn _reg(register: Register) -> Loc {
    Loc::Reg(ir::Reg { register, width: 2 })
}

#[allow(non_snake_case)]
pub fn _RUNTIME_FRAME_HEADER(runtime: model::RuntimeProfile) -> Result<i64, String> {
    match runtime {
        model::RuntimeProfile::Qb45 => Ok(10),
        model::RuntimeProfile::Pds71 => Ok(18),
        model::RuntimeProfile::Vbdos => Ok(20),
        model::RuntimeProfile::Freestanding => Err(format!("KeyError: {}", runtime.repr())),
    }
}

/// Enter and leave a BASIC runtime frame.
///
/// B$FCMD and the managed-string runtime consult BASIC's current frame, so a
/// merely zeroed C-style frame is not sufficient. B$ENRA itself pushes BP,
/// installs the BASIC frame chain, saves SI/DI, and allocates the local bytes;
/// B$EXSA reverses that work. The frontend therefore emits these procedures
/// without MASM's native shell and rebases only locals below the runtime
/// header.
///
/// BX is the maximum number of runtime-produced STRING temporaries an HIR
/// instruction consumes and produces together. This count must come from
/// resolved typed expressions, never from allocator spill slots.
/// The main body's frame as the data object `MAIN_FRAME`, `size` bytes: each
/// BP-relative local moved there. BC's module-level code has no frame of
/// its own, and the runtime takes BP there for its own: framed by B$ENRA,
/// QB 4.5 read the frame's missing return address as the error's, and
/// reported a fatal error in "line 49152 of module $ p".
pub fn _static_frame(body: &lir::LirBody, size: i64) -> lir::LirBody {
    let moved = |addr: &Addr| -> Addr {
        let segment = if addr.segment == Register::SS { Register::None } else { addr.segment };
        Addr { space: Space::Segment, index: MAIN_FRAME_ID, disp: size + addr.disp, segment, ..*addr }
    };
    let variables = body
        .variables
        .iter()
        .map(|one| lir::DebugVariable { addr: if one.addr.space == Space::Frame { moved(&one.addr) } else { one.addr }, ..one.clone() })
        .collect();
    // Through BP no longer: the data object's own address.
    let through = |register: Register| if matches!(register, Register::BP | Register::EBP) { Register::None } else { register };
    let operand = |r#where: &Loc| -> Loc {
        match r#where {
            Loc::Mem(mem) if mem.in_frame() => Loc::Mem(ir::Mem { addr: mem.addr.map(|addr| moved(&addr)), through: through(mem.through), ..mem.clone() }),
            Loc::Address(address) if address.in_frame() => Loc::Address(ir::Address { addr: address.addr.map(|addr| moved(&addr)), through: through(address.through), ..address.clone() }),
            other => other.clone(),
        }
    };
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let insns = block
                .insns
                .iter()
                .map(|one| match &one.what {
                    Some(what) => {
                        let mut replaced = (**one).clone();
                        replaced.what = Some(Semantics { dests: what.dests.iter().map(operand).collect(), sources: what.sources.iter().map(operand).collect(), ..what.clone() });
                        Arc::new(replaced)
                    }
                    None => Arc::clone(one),
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    lir::LirBody { variables, ..body.with_blocks(blocks) }
}

/// The main body's static frame: its label, and its data object's id,
/// which no global or pooled constant takes.
pub const MAIN_FRAME: &str = "$QB$FRAME";
pub const MAIN_FRAME_ID: i64 = i64::MAX;

pub fn _runtime_frame(
    body: &lir::LirBody,
    size: i64,
    runtime: model::RuntimeProfile,
    temporary_strings: i64,
) -> Result<(lir::LirBody, IndexMap<i64, masm::Callee>), String> {
    let size = size + (size & 1);
    let header = _RUNTIME_FRAME_HEADER(runtime)?;
    if size > 0x7FFE {
        return Err(format!("{}: {size} byte BASIC frame exceeds a 16-bit BP displacement", body.name).into());
    }
    if !(0..=0xFFFF).contains(&temporary_strings) {
        return Err(format!("{}: too many temporary STRING slots", body.name).into());
    }
    let serial = body.blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
    let imm = |value: i64| Loc::Imm(ir::Imm { value, width: 2, address: None });
    let enter = [
        _insn(serial, _semantics(Operation::Move, "mov", vec![_reg(Register::CX)], vec![imm(size)])),
        _insn(serial + 1, _semantics(Operation::Move, "mov", vec![_reg(Register::BX)], vec![imm(temporary_strings)])),
        _insn(serial + 2, _semantics(Operation::Call, "call", vec![], vec![])),
    ];
    let leave_at = serial + 3;
    let leave = _insn(leave_at, _semantics(Operation::Call, "call", vec![], vec![]));
    let framed: Vec<lir::LirBlock> = body
        .blocks
        .iter()
        .map(|block| {
            let insns = if block.at == body.entry {
                enter.iter().cloned().chain(block.insns.iter().cloned()).collect()
            } else {
                block.insns.clone()
            };
            block.with_insns(insns)
        })
        .collect();
    let framed: Vec<lir::LirBlock> = framed
        .into_iter()
        .map(|block| {
            let mut insns = Vec::new();
            for one in &block.insns {
                if one.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
                    insns.push(Arc::clone(&leave));
                }
                insns.push(Arc::clone(one));
            }
            lir::LirBlock { insns, ..block }
        })
        .collect();

    // B$ENRA preserves the ordinary far-Pascal parameter offsets and
    // inserts its own header below BP, between BP and source locals.
    let moved = |addr: &Addr| {
        let disp = if addr.disp > 0 { addr.disp } else { addr.disp - header };
        Addr { disp, ..*addr }
    };
    let variables = body.variables.iter().map(|one| lir::DebugVariable { addr: moved(&one.addr), ..one.clone() }).collect();
    let operand = |r#where: &Loc| -> Loc {
        match r#where {
            Loc::Mem(mem) if mem.in_frame() => {
                Loc::Mem(ir::Mem { addr: Some(moved(&mem.addr.unwrap())), ..mem.clone() })
            }
            Loc::Address(address) if address.in_frame() => {
                Loc::Address(ir::Address { addr: Some(moved(&address.addr.unwrap())), ..address.clone() })
            }
            other => other.clone(),
        }
    };
    let framed: Vec<lir::LirBlock> = framed
        .into_iter()
        .map(|block| {
            let insns = block
                .insns
                .iter()
                .map(|one| match &one.what {
                    Some(what) => {
                        let mut replaced = (**one).clone();
                        replaced.what = Some(Semantics {
                            dests: what.dests.iter().map(operand).collect(),
                            sources: what.sources.iter().map(operand).collect(),
                            ..what.clone()
                        });
                        Arc::new(replaced)
                    }
                    None => Arc::clone(one),
                })
                .collect();
            lir::LirBlock { insns, ..block }
        })
        .collect();
    Ok((
        lir::LirBody { variables, ..body.with_blocks(framed) },
        IndexMap::from_iter([(serial + 2, masm::Callee::new("B$ENRA", true)), (leave_at, masm::Callee::new("B$EXSA", true))]),
    ))
}

#[allow(non_snake_case)]
fn _SEGMENT_SHAPE(name: &str) -> Option<(u8, &'static str)> {
    Some(match name {
        "BR_DATA" => (0x68, "BLANK"),
        "BR_SKYS" => (0x68, "BLANK"),
        "COMMON" => (0x78, "BLANK"),
        "BC_DATA" => (0x48, "BC_DATA"),
        "NMALLOC" => (0x58, "BC_VARS"),
        "ENMALLOC" => (0x58, "BC_VARS"),
        "BC_FT" => (0x48, "BC_SEGS"),
        "BC_CN" => (0x68, "BC_SEGS"),
        "BC_DS" => (0x68, "BC_SEGS"),
        "BC_SAB" => (0x48, "BC_SEGS"),
        "BC_SA" => (0x48, "BC_SEGS"),
        "FDATA" => (0x60, "FAR_DATA"),
        "FSL_CONST" => (0x60, "FAR_DATA"),
        _ => return None,
    })
}

/// Apply the BASIC segment classes/combine modes to a fresh OMF envelope.
fn _basic_segment_classes(data: &[u8], code: &str) -> Result<Vec<u8>, String> {
    let value = |error: omf::ValueError| error.0;
    let records = omf::parse(data).map_err(value)?;
    let old_names = omf::names(&records);
    let mut names = old_names.clone();
    for name in ["BC_CODE", "BLANK", "BC_DATA", "BC_VARS", "BC_SEGS"] {
        if !names.iter().any(|one| one == name) {
            names.push(name.to_owned());
        }
    }
    let mut name_index: IndexMap<String, i64> = IndexMap::default();
    for (index, name) in names.iter().enumerate() {
        if index != 0 {
            name_index.insert(name.clone(), index as i64);
        }
    }
    let latin1 = |text: &str| -> Vec<u8> { text.chars().map(|one| one as u32 as u8).collect() };
    let mut rewritten: Vec<omf::Record> = Vec::new();
    let mut lnames_done = false;
    for record in &records {
        if record.r#type & 0xFE == omf::LNAMES {
            if lnames_done {
                return Err("fresh OMF unexpectedly contains multiple LNAMES records".into());
            }
            let body: Vec<u8> = names[1..]
                .iter()
                .flat_map(|name| {
                    let encoded = latin1(name);
                    std::iter::once(encoded.len() as u8).chain(encoded)
                })
                .collect();
            rewritten.push(omf::Record::new(record.r#type, body));
            lnames_done = true;
            continue;
        }
        if record.r#type & 0xFE != omf::SEGDEF {
            rewritten.push((**record).clone());
            continue;
        }
        let body = &record.body;
        let at = 1 + if body[0] >> 5 == 0 { 3 } else { 0 } + 2;
        let (segment_name_index, after_name) = omf::_index(body, at);
        let (_class_index, after_class) = omf::_index(body, after_name);
        let (overlay_index, after_overlay) = omf::_index(body, after_class);
        let segment_name = &old_names[segment_name_index as usize];
        let (acbp, class_name) = if segment_name == code {
            (0x68, "BC_CODE".to_owned())
        } else {
            match _SEGMENT_SHAPE(segment_name) {
                Some((acbp, class_name)) => (acbp, class_name.to_owned()),
                None => (body[0], old_names[_class_index as usize].clone()),
            }
        };
        let made: Vec<u8> = [
            vec![acbp],
            body[1..at].to_vec(),
            omf::as_index(segment_name_index).map_err(value)?,
            omf::as_index(name_index[&class_name]).map_err(value)?,
            omf::as_index(overlay_index).map_err(value)?,
            body[after_overlay..].to_vec(),
        ]
        .concat();
        rewritten.push(omf::Record::new(record.r#type, made));
    }
    Ok(rewritten.iter().flat_map(omf::Record::emit).collect())
}

/// Remove the native shell when B$ENRA/B$EXSA own the whole frame.
///
/// The shared MASM model supplies a C-shaped BP shell whenever a body
/// addresses BP or calls anything. B$ENRA itself saves BP, SI and DI, and
/// B$EXSA restores them, so this source-ABI exception stays in the frontend.
pub fn _basic_listing(procedure: &masm::Procedure, number: usize) -> Result<Vec<masm::Item>, String> {
    let listing = masm::listing(procedure, number).map_err(|error| error.0)?;
    let runtime_frame = procedure.callees.values().any(|callee| callee.name == "B$ENRA");
    let module_body = procedure.name == "$QB$MAIN";
    if !runtime_frame && !module_body {
        return Ok(listing);
    }
    // The procedure's first line stands before its prologue.
    let (mut stripped, listing): (Vec<masm::Item>, &[masm::Item]) = match listing.split_first() {
        Some((line @ masm::Item::Mark(_), rest)) => (vec![line.clone()], rest),
        _ => (Vec::new(), &listing),
    };
    let (enter, leave) = masm::_frame_parts(procedure);
    let same = |items: &[masm::Item], semantics: &[Semantics]| {
        items.len() == semantics.len()
            && items.iter().zip(semantics).all(|(item, one)| matches!(item, masm::Item::Semantics(what) if what == one))
    };
    if listing.len() < enter.len() || !same(&listing[..enter.len()], &enter) {
        return Err(format!("{}: native frame prefix changed shape", procedure.name).into());
    }
    let listing = &listing[enter.len()..];
    let mut at = 0;
    while at < listing.len() {
        let after = at + leave.len();
        if runtime_frame
            && !leave.is_empty()
            && after <= listing.len()
            && same(&listing[at..after], &leave)
            && after < listing.len()
            && matches!(&listing[after], masm::Item::Semantics(what) if what.op == Operation::Return)
        {
            at = after;
            continue;
        }
        stripped.push(listing[at].clone());
        at += 1;
    }
    Ok(stripped)
}

/// A BASIC module's text as its object holds it: without the native shell
/// where the runtime owns the frame.
pub fn text(module: &masm::Module) -> Result<String, String> {
    masm::text_by(module, |procedure, number| _basic_listing(procedure, number).map_err(|error| masm::Unprintable(error.to_string()))).map_err(|error| error.0)
}

/// A BASIC module's object: `module`'s code after the 30h MODULE_CODE
/// `header`, its data in BASIC's segments, and the statement table last.
pub fn written_basic(module: &masm::Module, header: Vec<u8>, name: &str) -> Result<Vec<u8>, String> {
    // Build the same semantic segments as backend.omfwrite.written, then add
    // the BASIC-owned MODULE_CODE envelope before asking its canonical record
    // serializer to write OMF.
    let mut segments = vec![omfwrite::Segment::new(&module.code, "CODE", false)];
    // A BASIC object does not own C's `_DATA` segment.  Even a zero-length
    // declaration is observable: when this is the first link object it makes
    // LINK establish the DATA class before BC_DATA, unlike BC/PDS/VBDOS, and
    // the BASIC runtime then initializes its local heap against the wrong
    // DGROUP boundary.
    let mut named: IndexSet<String> = IndexSet::default();
    for (name, _items) in &module.data {
        if named.insert(name.clone()) {
            let private = module.private.contains(name);
            segments.push(omfwrite::Segment::new(name, if private { "FAR_DATA" } else { "DATA" }, !private));
        }
    }
    let mut symbols: IndexMap<String, (usize, usize)> = IndexMap::default();
    for (name, items) in &module.data {
        let index = segments.iter().position(|one| &one.name == name).expect("every data segment was made");
        omfwrite::_data(&mut segments[index], index, items, &mut symbols);
    }
    let every: Vec<usize> = (0..module.procedures.len()).collect();
    omfwrite::_code_by(&mut segments[0], 0, module, &every, &mut symbols, _basic_listing).map_err(|error| error.to_string())?;

    let code = &mut segments[0];
    code.image = [header, std::mem::take(&mut code.image)].concat();
    code.lines = code.lines.iter().map(|&(line, at)| (line, at + 48)).collect();
    code.bodies = code.bodies.iter().map(|&(mark, at)| (mark, at + 48)).collect();
    code.spans = std::iter::once([0, 48]).chain(code.spans.iter().map(|[start, end]| [start + 48, end + 48])).collect();
    if module.procedures.last().is_none_or(|last| last.name != "$QB$STAT") {
        return Err("the BASIC statement table must be the final code procedure".into());
    }
    let statement_data = masm::label(module.procedures.len() - 1, 1);
    // The table is data carried by an opaque inline item.  The generic
    // assembly model conservatively emits a private BP prologue before such
    // an item, so OF_STA must name its first block label after that
    // prologue, not the procedure symbol.
    let shifted: Vec<omfwrite::Fixup> =
        code.fixups.iter().map(|one| omfwrite::Fixup { at: one.at + 48, ..one.clone() }).collect();
    code.fixups = [
        omfwrite::Fixup::new(10, omfwrite::OFFSET, statement_data),
    ]
    .into_iter()
    .chain(NAMED.iter().map(|&(word, _, label)| omfwrite::Fixup::new(word, omfwrite::OFFSET, label)))
    .chain(shifted)
    .collect();
    let mut symbols: IndexMap<String, (usize, usize)> = symbols
        .into_iter()
        .map(|(name, (segment, offset))| (name, (segment, if segment == 0 { offset + 48 } else { offset })))
        .collect();
    symbols.insert(HEADER.into(), (0, 0));
    // BC_DS stores DATA keys as literal final code offsets, not relocations.
    // Resolve the frontend's symbolic row labels only after the 30h module
    // header has shifted every code symbol, then remove their temporary
    // fixups. Leaving both the 0030h field and an OFFSET fixup made LINK add
    // them and B$RSTB searched for 0060h forever.
    let read_index = segments.iter().position(|one| one.name == "BC_DS").expect("BC_DS is always emitted");
    let read_segment = &mut segments[read_index];
    for fixup in read_segment.fixups.clone() {
        let (segment, offset) = symbols[&fixup.name];
        if fixup.loc != omfwrite::OFFSET || segment != 0 {
            return Err("BC_DS DATA key must resolve to a near code offset".into());
        }
        pack_into(&mut read_segment.image, fixup.at, offset as i64);
    }
    read_segment.fixups.clear();
    if let Some(debug) = &module.debug {
        let described = codeview::segments(debug, module, name, &segments[0], &symbols)?;
        segments.extend(described);
    }
    let externs: IndexMap<String, String> = module.externs.iter().cloned().collect();
    let records = omfwrite::_records(module, name, &mut segments, &symbols, &externs)
        .map_err(|error| error.to_string())?;
    let emitted: Vec<u8> = records.iter().flat_map(|record| record.emit()).collect();
    _basic_segment_classes(&emitted, &module.code)
}

/// `_CODE`: 80387 encodings. FEXP2 splits x into nearest integer n and
/// fraction f, then computes (2**f) * (2**n).
#[allow(non_snake_case)]
fn _CODE(name: &str) -> Option<Vec<u8>> {
    let hex: &str = match name {
        "fsin" => "d9fe",
        "fcos" => "d9ff",
        "fatan" => "d9e8d9f3",     // fld1; fpatan
        "flog2" => "d9e8d9c9d9f1", // fld1; fxch; fyl2x
        "fexp2" => "d9c0d9fcd9c9d8e1d9f0d9e8dec1d9fdddd9",
        "fround" => "d9fc", // frndint
        _ => return None,
    };
    Some((0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect())
}

#[derive(Clone, Debug)]
pub struct Finalized {
    pub body: lir::LirBody,
    pub callees: IndexMap<i64, masm::Callee>,
}

/// Replace allocated QB intrinsic pseudos with inline-byte placeholders.
pub fn finalized(body: &lir::LirBody, parameter_bytes: i64) -> Result<Finalized, String> {
    let body = masm::cleaned_returns(body, parameter_bytes)?;
    let mut sites: IndexMap<i64, masm::Callee> = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut instructions = Vec::new();
        for instruction in &block.insns {
            let what = instruction.what.as_ref();
            let code = what.and_then(|what| _CODE(what.name.as_deref().unwrap_or("")));
            let Some(code) = code else {
                instructions.push(Arc::clone(instruction));
                continue;
            };
            let what = what.expect("a named operation");
            let st0 = vec![Loc::St(ir::St { index: 0 })];
            let name = what.name.clone().unwrap_or_default();
            if what.op != Operation::FloatUnary || what.dests != st0 || what.sources != st0 {
                return Err(format!("{name} must be allocated as st(0) -> st(0)"));
            }
            sites.insert(
                instruction.at,
                masm::Callee { name: format!("$inline_{name}"), far: false, code: vec![masm::InlinePart::Bytes(code)] },
            );
            let mut replaced = (**instruction).clone();
            replaced.what = Some(Semantics { name: Some(name), ..Semantics::new(Operation::Call) });
            instructions.push(Arc::new(replaced));
        }
        blocks.push(block.with_insns(instructions));
    }
    Ok(Finalized { body: body.with_blocks(blocks), callees: sites })
}

/// The main body's symbol.
pub const MAIN: &str = "$QB$MAIN";
/// The code segment's first byte: the module header.
pub const HEADER: &str = "$QB$HEADER";
/// Each segment the module header names, by the word that names it and the
/// label starting the segment that word is fixed up to.
pub const NAMED: [(usize, &str, &str); 5] =
    [(12, "BC_DS", "$QB$DS"), (14, "BC_DATA", "$QB$DATA"), (16, "BC_FT", "$QB$FT"), (24, "COMMON", "$QB$COMMON"), (32, "BC_CN", "$QB$CN")];

/// A BASIC module object as its frontend lays it out around the code.
pub struct Object {
    /// The code segment's name.
    pub code: String,
    /// MODULE_CODE, but for the words `written_basic` writes afresh.
    pub header: Vec<u8>,
    /// The body the runtime enters right after the header, by its MIR name.
    pub main: String,
    /// Each global's symbol, by its MIR name, where it is not that name.
    pub symbols: BTreeMap<String, String>,
    /// Each HIR data object's symbol, by its id, for a module entering as HIR.
    pub data: BTreeMap<i64, String>,
    /// Each data segment, in order.
    pub segments: Vec<Segment>,
    /// The segment the compiler's constants go in.
    pub constants: String,
    /// The segments outside DGROUP.
    pub private: BTreeSet<String>,
    /// The near symbols LINK must resolve though no code calls them: the
    /// graphics drivers a SCREEN mode needs.
    pub requests: BTreeSet<String>,
    /// How each function is framed, by its MIR name, where not by B$ENRA
    /// with no temporary STRING slot.
    pub frames: BTreeMap<String, Frame>,
    /// Each source line's BASIC line number, for the statement table.
    pub line_numbers: BTreeMap<i64, i64>,
}

/// How a function is framed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frame {
    /// By B$ENRA and B$EXSA, with this many temporary STRING slots.
    Runtime { strings: i64 },
    /// By the function itself, its locals zeroed by its own code.
    Own,
}

/// A data segment: what it holds, in order, and its size where stated.
pub struct Segment {
    pub name: String,
    pub items: Vec<Item>,
    pub size: Option<i64>,
}

/// What a segment holds: a datum as it is, or a global's data by its MIR
/// name, at its offset where stated; one without an offset the pipeline
/// may delete.
#[derive(Clone, Debug)]
pub enum Item {
    Datum(masm::Datum),
    Global { name: String, at: Option<i64> },
    /// A HIR data object, by its id, for a module entering as HIR: its
    /// global once emitted.
    Object(i64),
}

/// `program`, one HIR module, compiled into the BASIC module object `object`
/// lays out: emitted, each data object `object` names resolved to its
/// global, optimized, assembled.
pub fn compiled(program: &model::Program, object: &Object, options: &Options) -> Result<masm::Module, String> {
    let (mut mir, data) = super::emitted(program, options)?;
    let [(module, data)] = [(&mir.modules[0], &data[0])];
    let global = |id: &i64| data.get(id).and_then(|&global| module.global(global).name.clone());
    let mut resolved = Object { segments: Vec::new(), symbols: object.symbols.clone(), data: BTreeMap::new(), private: object.private.clone(), requests: object.requests.clone(), frames: object.frames.clone(), line_numbers: object.line_numbers.clone(), code: object.code.clone(), header: object.header.clone(), main: object.main.clone(), constants: object.constants.clone() };
    resolved.symbols.extend(object.data.iter().filter_map(|(id, symbol)| Some((global(id)?, symbol.clone()))));
    for segment in &object.segments {
        let items = segment.items.iter().filter_map(|item| match item {
            Item::Object(id) => global(id).map(|name| Item::Global { name, at: None }),
            other => Some(other.clone()),
        });
        resolved.segments.push(Segment { name: segment.name.clone(), items: items.collect(), size: segment.size });
    }
    super::optimized(&mut mir, options)?;
    assembled(&mir.modules[0], &resolved, program.runtime, options)
}

/// A lifter's `module` compiled into the BASIC module object `object` lays
/// out, the object file `name`: linked against `runtime`, a module of
/// declarations alone, in a program whose segments `segments` lays out,
/// what the layout places kept, optimized, assembled.
#[allow(clippy::too_many_arguments)]
pub fn lifted(module: Module, runtime: Module, object: &Object, family: model::RuntimeProfile, segments: &SegmentLayout, options: &Options, name: &str) -> Result<Vec<u8>, String> {
    let target = std::rc::Rc::new(crate::abi::qb::LoweredTarget::of(options.cpu()?, object_abi(object, family)));
    let mut program = super::linked(vec![module], runtime, target)?;
    program.segments = segments.clone();
    let placed = object.segments.iter().flat_map(|one| &one.items).filter_map(|item| match item {
        Item::Global { name, .. } => Some(name.clone()),
        Item::Datum(_) | Item::Object(_) => None,
    });
    program.exports.kept = placed.collect();
    super::optimized(&mut program, options)?;
    self::object(&program.modules[0], object, family, options, name)
}

/// `module` compiled for the machine into the BASIC module object `object`
/// lays out, the object file `name`; its runtime `runtime`'s.
pub fn object(module: &Module, object: &Object, runtime: model::RuntimeProfile, options: &Options, name: &str) -> Result<Vec<u8>, String> {
    written_basic(&assembled(module, object, runtime, options)?, object.header.clone(), name)
}

/// The calls of the BASIC module object `object` lays out: its runtime's,
/// and its own by their symbols.
fn object_abi(object: &Object, runtime: model::RuntimeProfile) -> HirAbi {
    HirAbi { runtime, objects: object.symbols.clone(), preserved: BTreeSet::new() }
}

/// `module` selected and assembled as a BASIC module: the main body first,
/// each body framed as the runtime frames it, the statement table last, and
/// the data where `object` lays it out.
pub fn assembled(module: &Module, object: &Object, runtime: model::RuntimeProfile, options: &Options) -> Result<masm::Module, String> {
    let abi = object_abi(object, runtime);
    let mut names = globals::names(module, &|name| abi.linked(name))?;
    // A symbol the frontend states stands as it is, BASIC's type suffix and all.
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        if let Some(symbol) = global.name.as_ref().and_then(|name| object.symbols.get(name)) {
            names.extend(globals::segment_name(module, id, symbol));
            names.insert((globals::space(module, id), i64::from(id.0)), symbol.clone());
        }
    }
    names.extend(crate::hir::symbols::symbol_names());
    let main = module.named(&object.main).ok_or("no main body")?;
    names.insert((Space::Segment, i64::from(main.0)), MAIN.to_owned());
    names.insert((Space::Segment, MAIN_FRAME_ID), MAIN_FRAME.to_owned());
    let pool = Rc::new(RefCell::new(Pool::new(module.globals.len() as i64)));
    let segments = Segments::of(&options.machine);
    let cpu = options.cpu()?;
    let mut procedures = Vec::new();
    let mut referenced: BTreeMap<String, bool> = BTreeMap::new();
    let mut rows = Vec::new();
    let mut handled = false;
    let mut main_frame = 0;
    // The runtime enters the module right after its header.
    let order = std::iter::once(main).chain((0..module.globals.len() as u32).map(GlobalId).filter(|&id| id != main));
    for id in order {
        let GlobalKind::Function(function) = &module.global(id).kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let frame = object.frames.get(module.global(id).name.as_deref().unwrap_or_default()).copied().unwrap_or(Frame::Runtime { strings: 0 });
        // B$ENRA zero-fills a runtime frame's locals.
        let target = Target { cpu, segments: &segments, runtime: runtime.value(), basic: true, zeroed: matches!(frame, Frame::Runtime { .. }) };
        let (procedure, landing, statics) = procedure(module, id, id == main, frame, &names, &abi, &pool, &target, runtime)?;
        main_frame += statics;
        for callee in procedure.callees.values() {
            referenced.insert(callee.name.clone(), callee.far);
        }
        rows.extend(procedure.body.blocks.first().map(|entry| super::entry_row(procedures.len(), entry.at)));
        // RESUME NEXT (B$RESN) continues at the first row past the error:
        // with a landing pad, that must be the pad. Without one, each line
        // is a row, the runtime's for an error's line.
        if landing.is_none() && !object.line_numbers.is_empty() {
            let starts = masm::line_starts(&procedure, procedures.len()).map_err(|error| error.0)?;
            for (order, (label, line)) in starts.into_iter().enumerate() {
                let number = object.line_numbers.get(&i64::from(line)).copied().unwrap_or(0);
                rows.push((procedures.len() as i64, order as i64, label, number));
            }
        }
        rows.extend(landing.map(|at| super::landing_row(procedures.len(), at)));
        handled |= landing.is_some();
        procedures.push(procedure);
    }
    // A row states a line, and BC keeps rows only for a module that handles
    // errors: without one, the runtime reports a fault "No line number".
    if !handled {
        rows.clear();
    }
    procedures.push(super::statement_table(&rows));
    let mut data = Vec::new();
    for segment in &object.segments {
        data.push((segment.name.clone(), laid_out(module, segment, &names)?));
    }
    let mut pooled = Vec::new();
    if main_frame > 0 {
        pooled.extend([masm::Datum::Object(masm::Label { name: MAIN_FRAME.to_owned() }), masm::Datum::Bytes(vec![0; main_frame as usize])]);
    }
    for (bytes, id) in pool.borrow().entries() {
        let label = format!("$QB$D{id}");
        names.insert((Space::Segment, id), label.clone());
        pooled.extend([masm::Datum::Object(masm::Label { name: label }), masm::Datum::Bytes(bytes.to_vec())]);
    }
    let laid: BTreeSet<&str> = object.segments.iter().flat_map(|one| &one.items).filter_map(|item| match item {
        Item::Global { name, .. } => Some(name.as_str()),
        Item::Datum(_) | Item::Object(_) => None,
    }).collect();
    pooled.extend(super::added_data(module, &|id| module.global(id).name.as_deref().is_some_and(|name| laid.contains(name)), &names)?);
    if !pooled.is_empty() {
        match data.iter_mut().find(|(name, _)| *name == object.constants) {
            Some((_, datums)) => datums.extend(pooled),
            None => data.push((object.constants.clone(), pooled)),
        }
    }
    let defined: BTreeSet<&str> = procedures.iter().map(|one| one.name.as_str()).collect();
    let mut externs: Vec<(String, String)> = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, &far)| (name.clone(), if far { "far" } else { "near" }.to_owned()))
        .collect();
    for (at, global) in module.globals.iter().enumerate() {
        if matches!(&global.kind, GlobalKind::Variable(variable) if variable.initializer.is_none()) {
            externs.extend(names.get(&(Space::External, at as i64)).filter(|name| name.as_str() != HEADER).map(|name| (name.clone(), "byte".to_owned())));
        }
    }
    externs.extend(object.requests.iter().map(|name| (name.clone(), "near".to_owned())));
    externs.sort();
    externs.dedup();
    let flavor = llrm_omf::cvwrite::Flavor { qb45: runtime == model::RuntimeProfile::Qb45 };
    let debug = codeview::described(module, &names, flavor)?;
    Ok(masm::Module {
        code: object.code.clone(),
        names,
        externs,
        publics: procedures.iter().filter(|one| one.public).map(|one| one.name.clone()).collect(),
        data,
        procedures,
        private: object.private.clone(),
        requests: object.requests.clone(),
        debug,
        stack: 0,
    })
}

/// A defined function selected, through the machine phases, and framed as
/// BASIC frames it: a naked one not at all, the main body only where it
/// reserves anything, and ending the program where it returns; every other
/// as `frame` says. Its landing pad's block, where it has one, and the
/// bytes of the main body's frame, which is static.
#[allow(clippy::too_many_arguments)]
fn procedure(
    module: &Module,
    id: GlobalId,
    main: bool,
    frame: Frame,
    names: &IndexMap<(Space, i64), String>,
    abi: &HirAbi,
    pool: &Rc<RefCell<Pool>>,
    target: &Target<'_>,
    runtime: model::RuntimeProfile,
) -> Result<(masm::Procedure, Option<i64>, i64), String> {
    let global = module.global(id);
    let machined = assemble::machined(module, global.name.as_deref().unwrap_or_default(), abi, pool, target)?;
    let finalized = finalized(&machined.body, machined.popped)?;
    let mut callees = finalized.callees;
    let mut reserve = 0;
    let mut statics = 0;
    let (body, framed) = if !super::framed(module, id) {
        (finalized.body, IndexMap::default())
    } else if main {
        statics = machined.reserve + (machined.reserve & 1);
        (_static_frame(&finalized.body, statics), IndexMap::default())
    } else {
        match frame {
            Frame::Runtime { strings } => _runtime_frame(&finalized.body, machined.reserve, runtime, strings)?,
            Frame::Own => {
                reserve = machined.reserve;
                (finalized.body, IndexMap::default())
            }
        }
    };
    callees.extend(framed);
    let mut body = addressvalues::converted(&body);
    if main {
        let exits;
        (body, exits) = ends_program(&body);
        callees.extend(exits);
    }
    for (at, callee) in &machined.calls {
        if let Some(code) = machined.inline.get(at) {
            callees.insert(*at, masm::Callee { name: callee.clone(), far: false, code: vec![masm::InlinePart::Bytes(code.clone())] });
            continue;
        }
        let linked = match module.named(callee) {
            Some(one) => names[&(globals::space(module, one), i64::from(one.0))].clone(),
            None => abi.linked(callee),
        };
        callees.insert(*at, masm::Callee::new(linked, machined.far.contains(at)));
    }
    let procedure = masm::Procedure {
        name: names[&(Space::Segment, i64::from(id.0))].clone(),
        public: !main && global.linkage == llrm_mir::Linkage::External,
        far: isel::far(global).map_err(|error| error.0)?,
        body,
        // B$ENRA reserves the frame; masm's own shell only one of its own.
        reserve,
        callees,
        interrupt: None,
    };
    Ok((procedure, machined.landing, statics))
}

/// `segment`'s data: a segment the module header names starts with its
/// label, and each global lies where the frontend states. Refuses data
/// naming the code past the header, which the recompile moves.
fn laid_out(module: &Module, segment: &Segment, names: &IndexMap<(Space, i64), String>) -> Result<Vec<masm::Datum>, String> {
    let name = &segment.name;
    let mut out = Vec::new();
    if let Some(&(_, _, label)) = NAMED.iter().find(|(_, one, _)| one == name) {
        out.push(masm::Datum::Label(masm::Label { name: label.to_owned() }));
    }
    let mut offset = 0;
    for item in &segment.items {
        let datums = match item {
            Item::Datum(datum) => vec![datum.clone()],
            Item::Global { name: global, at } => {
                if at.is_some_and(|at| at != offset) {
                    return Err(format!("{name} has a gap at {offset:#x}"));
                }
                match (module.named(global), at) {
                    (Some(id), _) => {
                        // Where it says, as `data::Layout` lays a global down.
                        let align = match &module.global(id).kind {
                            GlobalKind::Variable(variable) => variable.align.unwrap_or(1) as i64,
                            GlobalKind::Function(_) => 1,
                        };
                        let padding = (-offset).rem_euclid(align);
                        if padding != 0 && at.is_some() {
                            return Err(format!("{name} places @{global} at {offset:#x}, off its alignment {align}"));
                        }
                        let datums = globals::datums(module, id, names)?;
                        if padding == 0 { datums } else { [vec![masm::Datum::Bytes(vec![0; padding as usize])], datums].concat() }
                    }
                    // One the frontend placed nowhere in particular goes with its global.
                    (None, None) => continue,
                    (None, Some(_)) => return Err(format!("{name} holds @{global}, which the module lacks")),
                }
            }
            Item::Object(id) => return Err(format!("{name} holds data object {id}, which no HIR entry resolved")),
        };
        for datum in &datums {
            if let masm::Datum::Pointer(masm::Pointer { name: target, offset: at, .. }) = datum {
                if target == HEADER && *at != 0 {
                    return Err(format!("{name} names code at {at:#x}, which the recompile moves"));
                }
            }
        }
        offset += datums.iter().map(size_of).sum::<i64>();
        out.extend(datums);
    }
    if segment.size.is_some_and(|size| size != offset) {
        return Err(format!("{name} holds {offset:#x} of its {:#x} bytes", segment.size.unwrap_or_default()));
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

/// Spell BASIC module fallthrough as the runtime's implicit B$CENP.
pub fn ends_program(body: &lir::LirBody) -> (lir::LirBody, IndexMap<i64, masm::Callee>) {
    let mut sites: IndexMap<i64, masm::Callee> = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut instructions = Vec::new();
        let mut exits = false;
        for instruction in &block.insns {
            let mut instruction = Arc::clone(instruction);
            if instruction.what.as_ref().is_some_and(|what| what.op == Operation::Return) {
                exits = true;
                sites.insert(instruction.at, masm::Callee::new("B$CENP", true));
                let mut replaced = (*instruction).clone();
                replaced.what = Some(_semantics(Operation::Call, "call", vec![], vec![]));
                instruction = Arc::new(replaced);
            }
            instructions.push(instruction);
        }
        // Only the rewritten return becomes non-returning. Branch and jump
        // blocks retain their CFG edges; MASM listing uses the untaken edge to
        // insert an explicit jump when it is not the next laid-out block.
        let succ = if exits { vec![] } else { block.succ.clone() };
        blocks.push(lir::LirBlock { succ, ..block.with_insns(instructions) });
    }
    (lir::LirBody { noreturn: true, ..body.with_blocks(blocks) }, sites)
}

#[cfg(test)]
#[path = "basic_tests.rs"]
mod basic_tests;
