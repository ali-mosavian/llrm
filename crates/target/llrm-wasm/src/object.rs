//! The wasm relocatable object: the sections `llc -filetype=obj` writes for
//! straight-line functions, with no relocations yet.
//!
//! # How it works
//! Type, import (`env.__linear_memory`), function and code sections, then the
//! custom `linking` (version 2, a symbol table of the defined functions) and
//! `target_features` sections. Section sizes are 5-byte padded LEBs so a
//! linker can rewrite them; function sizes are not.
//!
//! DEVIATE: no data segments, relocations (`reloc.CODE`), imports of
//! functions, tables or globals, and no `name` or debug sections; the feature
//! list is LLVM 20's generic CPU, not the pin's. Reason: the slice calls and
//! addresses nothing. Exit: calls and globals (W9) and the pin wasm tree.
//!
//! # See also
//! `WasmObjectWriter.cpp` (`writeObject`, `writeTypeSection`,
//! `writeImportSection`, `writeFunctionSection`, `writeCodeSection`,
//! `writeLinkingMetaDataSection`), `WebAssemblyMCCodeEmitter.cpp`
//! (`encodeInstruction`), pin d1106deb71cc.

use crate::asm_strings::find;
use crate::mir::{Function, Operand, ValType};

/// The features `llc -mtriple=wasm32` of LLVM 20.1.8 writes for the generic
/// CPU.
const TARGET_FEATURES: [&str; 8] = [
    "bulk-memory",
    "bulk-memory-opt",
    "call-indirect-overlong",
    "multivalue",
    "mutable-globals",
    "nontrapping-fptoint",
    "reference-types",
    "sign-ext",
];

const SECTION_TYPE: u8 = 1;
const SECTION_IMPORT: u8 = 2;
const SECTION_FUNCTION: u8 = 3;
const SECTION_CODE: u8 = 10;
const SECTION_CUSTOM: u8 = 0;
const LINKING_VERSION: u8 = 2;
const WASM_SYMBOL_TABLE: u8 = 8;
const SYMBOL_KIND_FUNCTION: u8 = 0;
const EXTERNAL_MEMORY: u8 = 2;

pub fn uleb(
    out: &mut Vec<u8>,
    mut value: u64,
) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub fn sleb(
    out: &mut Vec<u8>,
    mut value: i64,
) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0) {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// `encodeULEB128(value, OS, PadTo)` at five bytes.
fn uleb_padded(
    out: &mut Vec<u8>,
    value: u32,
) {
    for i in 0..4 {
        out.push((value >> (7 * i)) as u8 & 0x7f | 0x80);
    }
    out.push((value >> 28) as u8);
}

fn val_type(ty: ValType) -> u8 {
    match ty {
        ValType::I32 => 0x7f,
        ValType::I64 => 0x7e,
        ValType::F32 => 0x7d,
        ValType::F64 => 0x7c,
    }
}

fn name(
    out: &mut Vec<u8>,
    text: &str,
) {
    uleb(out, text.len() as u64);
    out.extend_from_slice(text.as_bytes());
}

fn section(
    out: &mut Vec<u8>,
    id: u8,
    payload: &[u8],
) {
    out.push(id);
    uleb_padded(out, payload.len() as u32);
    out.extend_from_slice(payload);
}

fn custom(
    out: &mut Vec<u8>,
    section_name: &str,
    payload: &[u8],
) {
    let mut body = Vec::new();
    name(&mut body, section_name);
    body.extend_from_slice(payload);
    section(out, SECTION_CUSTOM, &body);
}

/// `encodeInstruction`: the opcode bytes, then the immediates by operand class.
pub fn encode_instruction(
    out: &mut Vec<u8>,
    inst: &crate::mir::Inst,
) {
    let asm = find(inst.opcode).unwrap_or_else(|| panic!("opcode {} has no encoding", inst.opcode));
    let binary = asm.binary;
    if binary < 1 << 8 {
        out.push(binary as u8);
    } else if binary < 1 << 16 {
        out.push((binary >> 8) as u8);
        uleb(out, u64::from(binary as u8));
    } else if binary < 1 << 24 {
        out.push((binary >> 16) as u8);
        uleb(out, u64::from(binary as u16));
    } else {
        panic!("opcode {} is a pseudo or has no encoding", asm.text);
    }
    let immediates = inst.operands.iter().filter(|o| o.is_immediate());
    for (kind, operand) in asm.kinds.iter().zip(immediates) {
        let Operand::Imm(value) = *operand else {
            panic!("operand {operand:?} of {} needs a relocation, not written yet", asm.text)
        };
        match *kind {
            "i32imm_op" => sleb(out, i64::from(value as i32)),
            "i64imm_op" => sleb(out, value),
            "offset32_op" => uleb(out, u64::from(value as u32)),
            "f32imm_op" | "f64imm_op" | "function32_op" | "function64_op" | "global_op" | "signature" | "bb_op" => {
                panic!("operand class {kind} of {} is not encoded yet", asm.text)
            }
            _ => uleb(out, value as u64),
        }
    }
}

/// A function after the passes, encoded: the body is the locals and the code.
pub struct Encoded {
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
    pub body: Vec<u8>,
}

pub fn encode_function(f: &Function) -> Encoded {
    let mut body = Vec::new();
    let mut groups: Vec<(u64, ValType)> = Vec::new();
    for &ty in &f.locals {
        match groups.last_mut() {
            Some((count, last)) if *last == ty => *count += 1,
            _ => groups.push((1, ty)),
        }
    }
    uleb(&mut body, groups.len() as u64);
    for (count, ty) in groups {
        uleb(&mut body, count);
        body.push(val_type(ty));
    }
    for inst in &f.blocks[0] {
        encode_instruction(&mut body, inst);
    }
    Encoded { name: f.name.clone(), params: f.params.clone(), results: f.results.clone(), body }
}

/// The object file for `functions`, in order.
pub fn write(functions: &[Encoded]) -> Vec<u8> {
    let mut types: Vec<(&[ValType], &[ValType])> = Vec::new();
    let signatures: Vec<usize> = functions
        .iter()
        .map(|f| {
            let signature = (&f.params[..], &f.results[..]);
            types
                .iter()
                .position(|t| *t == signature)
                .unwrap_or_else(
                    || {
                        types.push(signature);
                        types.len() - 1
                    },
                )
        })
        .collect();

    let mut out = b"\0asm\x01\0\0\0".to_vec();

    let mut payload = Vec::new();
    uleb(&mut payload, types.len() as u64);
    for (params, results) in &types {
        payload.push(0x60);
        for list in [params, results] {
            uleb(&mut payload, list.len() as u64);
            payload.extend(list.iter().map(|t| val_type(*t)));
        }
    }
    section(&mut out, SECTION_TYPE, &payload);

    payload.clear();
    uleb(&mut payload, 1);
    name(&mut payload, "env");
    name(&mut payload, "__linear_memory");
    payload.extend([EXTERNAL_MEMORY, 0, 0]);
    section(&mut out, SECTION_IMPORT, &payload);

    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    for &signature in &signatures {
        uleb(&mut payload, signature as u64);
    }
    section(&mut out, SECTION_FUNCTION, &payload);

    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    for f in functions {
        uleb(&mut payload, f.body.len() as u64);
        payload.extend_from_slice(&f.body);
    }
    section(&mut out, SECTION_CODE, &payload);

    payload.clear();
    payload.push(LINKING_VERSION);
    let mut symbols = Vec::new();
    uleb(&mut symbols, functions.len() as u64);
    for (index, f) in functions.iter().enumerate() {
        // Kind, flags (global, default visibility, defined), index, name.
        symbols.extend([SYMBOL_KIND_FUNCTION, 0]);
        uleb(&mut symbols, index as u64);
        name(&mut symbols, &f.name);
    }
    payload.push(WASM_SYMBOL_TABLE);
    uleb_padded(&mut payload, symbols.len() as u32);
    payload.extend_from_slice(&symbols);
    custom(&mut out, "linking", &payload);

    payload.clear();
    uleb(&mut payload, TARGET_FEATURES.len() as u64);
    for feature in TARGET_FEATURES {
        payload.push(b'+');
        name(&mut payload, feature);
    }
    custom(&mut out, "target_features", &payload);
    out
}
