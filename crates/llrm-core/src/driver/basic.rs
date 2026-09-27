//! The BASIC runtime's module object, as BC writes one: B$ENRA and B$EXSA
//! frames, the statement table, the 30h MODULE_CODE header before the code,
//! BASIC's segments and classes, and the final spelling of the x87 pseudos
//! isel leaves.

use std::sync::Arc;

use iced_x86::Register;

use crate::backend::{masm, omfwrite};
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

/// Zero a native frame as BASIC's runtime entry routines do.
///
/// QB variables begin at zero, and runtime-managed string/array descriptors
/// require that invariant before their first assignment.  The shared backend
/// deliberately owns only reservation; this source ABI initialization stays
/// in the frontend and runs before any source instruction.
pub fn _initialize_frame(
    body: &lir::LirBody,
    size: i64,
) -> Result<(lir::LirBody, IndexMap<i64, masm::Callee>), String> {
    let size = size + (size & 1);
    if size == 0 {
        return Ok((body.clone(), IndexMap::default()));
    }
    if size > 0x7FFE {
        return Err(format!("{}: {size} byte native frame exceeds a 16-bit BP displacement", body.name).into());
    }
    let at = body.blocks.iter().flat_map(|block| &block.insns).map(|one| one.at).max().unwrap_or(0) + 1;
    let initialize = _insn(at, _semantics(Operation::Call, "frame-zero", vec![], vec![]));
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            if block.at == body.entry {
                let insns = std::iter::once(Arc::clone(&initialize)).chain(block.insns.iter().cloned()).collect();
                block.with_insns(insns)
            } else {
                block.clone()
            }
        })
        .collect();
    let code: Vec<u8> = [
        vec![0x06, 0x57, 0x16, 0x07, 0x31, 0xc0, 0x8d, 0xbe],
        ((-size) as i16).to_le_bytes().to_vec(),
        vec![0xB9],
        ((size / 2) as u16).to_le_bytes().to_vec(),
        vec![0xfc, 0xf3, 0xab, 0x5f, 0x07],
    ]
    .concat();
    Ok((
        body.with_blocks(blocks),
        IndexMap::from_iter([(
            at,
            masm::Callee { name: "$frame_zero".into(), far: false, code: vec![masm::InlinePart::Bytes(code)] },
        )]),
    ))
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

    let operand = |r#where: &Loc| -> Loc {
        // B$ENRA preserves the ordinary far-Pascal parameter offsets and
        // inserts its own header below BP, between BP and source locals.
        let moved = |addr: &Addr| {
            let disp = if addr.disp > 0 { addr.disp } else { addr.disp - header };
            Addr { disp, ..*addr }
        };
        match r#where {
            Loc::Mem(mem) if mem.addr.is_some_and(|addr| addr.space == Space::Frame) => {
                Loc::Mem(ir::Mem { addr: Some(moved(&mem.addr.unwrap())), ..mem.clone() })
            }
            Loc::Address(address) if address.addr.is_some_and(|addr| addr.space == Space::Frame) => {
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
        body.with_blocks(framed),
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
    let (enter, leave) = masm::_frame_parts(procedure);
    let same = |items: &[masm::Item], semantics: &[Semantics]| {
        items.len() == semantics.len()
            && items.iter().zip(semantics).all(|(item, one)| matches!(item, masm::Item::Semantics(what) if what == one))
    };
    if listing.len() < enter.len() || !same(&listing[..enter.len()], &enter) {
        return Err(format!("{}: native frame prefix changed shape", procedure.name).into());
    }
    let listing = &listing[enter.len()..];
    let mut stripped: Vec<masm::Item> = Vec::new();
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

/// Encode BASIC listings with their frontend-owned runtime frame shell.
fn _basic_code(
    segment: &mut omfwrite::Segment,
    module: &masm::Module,
    symbols: &mut IndexMap<String, (usize, usize)>,
) -> Result<(), String> {
    let unencodable = |error: omfwrite::Unencodable| error.0;
    let mut items: Vec<omfwrite::Encoded> = Vec::new();
    for (number, procedure) in module.procedures.iter().enumerate() {
        items.push(omfwrite::Encoded::Label(masm::Label { name: procedure.name.clone() }));
        for item in _basic_listing(procedure, number)? {
            match omfwrite::_items(&item, &module.names, number) {
                Ok(encoded) => items.extend(encoded),
                Err(error) => return Err(format!("{}: {error}", procedure.name)),
            }
        }
    }
    let labels = omfwrite::_relaxed(&mut items).map_err(unencodable)?;
    let mut at = 0;
    for item in &items {
        match item {
            omfwrite::Encoded::Label(masm::Label { name }) => {
                symbols.insert(name.clone(), (0, at));
            }
            omfwrite::Encoded::Piece(omfwrite::Piece { code, fixups }) => segment.put(code, fixups),
            omfwrite::Encoded::Jump(omfwrite::Jump { name, label, long }) => {
                segment.put(&omfwrite::_jump(name, labels[label], at, *long).map_err(unencodable)?.code, &[]);
            }
            omfwrite::Encoded::Near(omfwrite::Near { name }) if labels.contains_key(name) => {
                let distance = labels[name] - (at as i64 + 3);
                let Ok(distance) = i16::try_from(distance) else {
                    return Err("'h' format requires -32768 <= number <= 32767".into());
                };
                segment.put(&[&[0xE8][..], &distance.to_le_bytes()].concat(), &[]);
            }
            omfwrite::Encoded::Near(omfwrite::Near { name }) => {
                segment.put(&[0; 3], &[omfwrite::Fixup { relative: true, ..omfwrite::Fixup::new(1, omfwrite::OFFSET, name.clone()) }]);
                segment.image[at] = 0xE8;
            }
        }
        at = segment.image.len();
    }
    Ok(())
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
    _basic_code(&mut segments[0], module, &mut symbols)?;

    let code = &mut segments[0];
    code.image = [header, std::mem::take(&mut code.image)].concat();
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
        omfwrite::Fixup::new(12, omfwrite::OFFSET, "$QB$DS"),
        omfwrite::Fixup::new(14, omfwrite::OFFSET, "$QB$DATA"),
        omfwrite::Fixup::new(16, omfwrite::OFFSET, "$QB$FT"),
        omfwrite::Fixup::new(24, omfwrite::OFFSET, "$QB$COMMON"),
        omfwrite::Fixup::new(32, omfwrite::OFFSET, "$QB$CN"),
    ]
    .into_iter()
    .chain(shifted)
    .collect();
    let mut symbols: IndexMap<String, (usize, usize)> = symbols
        .into_iter()
        .map(|(name, (segment, offset))| (name, (segment, if segment == 0 { offset + 48 } else { offset })))
        .collect();
    symbols.insert("$QB$HEADER".into(), (0, 0));
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

/// Return one audited expansion for diagnostics and stage dumps.
pub fn expansion(name: &str) -> Option<Vec<u8>> {
    _CODE(name)
}
