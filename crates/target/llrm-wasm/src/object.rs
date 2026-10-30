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
const EXTERNAL_FUNCTION: u8 = 0;
const EXTERNAL_MEMORY: u8 = 2;
const WASM_SYMBOL_UNDEFINED: u8 = 0x10;
const R_WASM_FUNCTION_INDEX_LEB: u8 = 0;
/// The position of the code section: after the type, import and function
/// sections.
const SECTION_INDEX_CODE: u64 = 3;

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
    symbols: &[(String, i64)],
    relocs: &mut Vec<Reloc>,
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
        if let (Operand::Sym(at), "function32_op") = (*operand, *kind) {
            // The index is the symbol's final function index, patched in by
            // `write`.
            relocs.push(Reloc { offset: out.len(), symbol: symbols[at as usize].0.clone() });
            uleb_padded(out, 0);
            continue;
        }
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
    pub relocs: Vec<Reloc>,
    /// The signature of each function the body calls.
    pub callees: Vec<(String, Vec<ValType>, Vec<ValType>)>,
}

/// An `R_WASM_FUNCTION_INDEX_LEB` in a body: the offset of its five-byte LEB
/// from the start of the body, and the symbol it names.
pub struct Reloc {
    pub offset: usize,
    pub symbol: String,
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
    let mut relocs = Vec::new();
    for inst in &f.blocks[0] {
        encode_instruction(&mut body, inst, &f.symbols, &mut relocs);
    }
    Encoded {
        name: f.name.clone(),
        params: f.params.clone(),
        results: f.results.clone(),
        body,
        relocs,
        callees: f.callees.clone(),
    }
}

/// A function symbol, in the order the assembler created it: a function's own
/// symbol when its header is printed, a callee's at its first use.
struct FunctionSymbol<'a> {
    name: &'a str,
    /// The position in `functions` of its definition, `None` for an import.
    defined: Option<usize>,
    params: &'a [ValType],
    results: &'a [ValType],
}

fn type_index<'a>(
    types: &mut Vec<(&'a [ValType], &'a [ValType])>,
    signature: (&'a [ValType], &'a [ValType]),
) -> usize {
    types
        .iter()
        .position(|t| *t == signature)
        .unwrap_or_else(
            || {
                types.push(signature);
                types.len() - 1
            },
        )
}

/// The object file for `functions`, in order.
pub fn write(functions: &[Encoded]) -> Vec<u8> {
    let mut symbols: Vec<FunctionSymbol> = Vec::new();
    for (at, f) in functions.iter().enumerate() {
        let defined = FunctionSymbol { name: &f.name, defined: Some(at), params: &f.params, results: &f.results };
        match symbols.iter_mut().find(|s| s.name == f.name) {
            Some(known) => *known = defined,
            None => symbols.push(defined),
        }
        for reloc in &f.relocs {
            if symbols.iter().any(|s| s.name == reloc.symbol) {
                continue;
            }
            let (_, params, results) =
                f.callees.iter().find(|(name, ..)| *name == reloc.symbol).expect("a called function has a signature");
            symbols.push(FunctionSymbol { name: &reloc.symbol, defined: None, params, results });
        }
    }
    let imports: Vec<usize> = (0..symbols.len()).filter(|&s| symbols[s].defined.is_none()).collect();
    // Function indices: the imports first, then the definitions.
    let mut index_of = vec![0u32; symbols.len()];
    for (n, &s) in imports.iter().enumerate() {
        index_of[s] = n as u32;
    }
    for (s, symbol) in symbols.iter().enumerate() {
        if let Some(at) = symbol.defined {
            index_of[s] = (imports.len() + at) as u32;
        }
    }

    let mut types: Vec<(&[ValType], &[ValType])> = Vec::new();
    let signatures: Vec<usize> = functions.iter().map(|f| type_index(&mut types, (&f.params, &f.results))).collect();
    let import_types: Vec<usize> =
        imports.iter().map(|&s| type_index(&mut types, (symbols[s].params, symbols[s].results))).collect();

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
    uleb(&mut payload, 1 + imports.len() as u64);
    name(&mut payload, "env");
    name(&mut payload, "__linear_memory");
    payload.extend([EXTERNAL_MEMORY, 0, 0]);
    for (&s, &ty) in imports.iter().zip(&import_types) {
        name(&mut payload, "env");
        name(&mut payload, symbols[s].name);
        payload.push(EXTERNAL_FUNCTION);
        uleb(&mut payload, ty as u64);
    }
    section(&mut out, SECTION_IMPORT, &payload);

    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    for &signature in &signatures {
        uleb(&mut payload, signature as u64);
    }
    section(&mut out, SECTION_FUNCTION, &payload);

    // The code section, with each call's function index patched in and its
    // offset from the section's start.
    let symbol_of = |name: &str| symbols.iter().position(|s| s.name == name).expect("a symbol");
    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    let mut code_relocs: Vec<(usize, usize)> = Vec::new();
    for f in functions {
        uleb(&mut payload, f.body.len() as u64);
        let start = payload.len();
        payload.extend_from_slice(&f.body);
        for reloc in &f.relocs {
            let s = symbol_of(&reloc.symbol);
            let mut index = Vec::new();
            uleb_padded(&mut index, index_of[s]);
            payload[start + reloc.offset..start + reloc.offset + 5].copy_from_slice(&index);
            code_relocs.push((start + reloc.offset, s));
        }
    }
    section(&mut out, SECTION_CODE, &payload);

    payload.clear();
    payload.push(LINKING_VERSION);
    let mut table = Vec::new();
    uleb(&mut table, symbols.len() as u64);
    for (s, symbol) in symbols.iter().enumerate() {
        table.push(SYMBOL_KIND_FUNCTION);
        if symbol.defined.is_some() {
            // Flags (global, default visibility, defined), index, name.
            table.push(0);
            uleb(&mut table, u64::from(index_of[s]));
            name(&mut table, symbol.name);
        } else {
            table.push(WASM_SYMBOL_UNDEFINED);
            uleb(&mut table, u64::from(index_of[s]));
        }
    }
    payload.push(WASM_SYMBOL_TABLE);
    uleb_padded(&mut payload, table.len() as u32);
    payload.extend_from_slice(&table);
    custom(&mut out, "linking", &payload);

    if !code_relocs.is_empty() {
        payload.clear();
        uleb(&mut payload, SECTION_INDEX_CODE);
        uleb(&mut payload, code_relocs.len() as u64);
        for &(offset, s) in &code_relocs {
            payload.push(R_WASM_FUNCTION_INDEX_LEB);
            uleb(&mut payload, offset as u64);
            uleb(&mut payload, s as u64);
        }
        custom(&mut out, "reloc.CODE", &payload);
    }

    payload.clear();
    uleb(&mut payload, TARGET_FEATURES.len() as u64);
    for feature in TARGET_FEATURES {
        payload.push(b'+');
        name(&mut payload, feature);
    }
    custom(&mut out, "target_features", &payload);
    out
}
