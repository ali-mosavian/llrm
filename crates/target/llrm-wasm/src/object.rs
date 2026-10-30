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
const SECTION_DATA: u8 = 11;
const SECTION_DATA_COUNT: u8 = 12;
const OPCODE_I32_CONST: u8 = 0x41;
const OPCODE_END: u8 = 0x0b;
const SYMBOL_KIND_DATA: u8 = 1;
const WASM_SEGMENT_INFO: u8 = 5;
const SECTION_CUSTOM: u8 = 0;
const LINKING_VERSION: u8 = 2;
const WASM_SYMBOL_TABLE: u8 = 8;
const SYMBOL_KIND_FUNCTION: u8 = 0;
const EXTERNAL_FUNCTION: u8 = 0;
const EXTERNAL_MEMORY: u8 = 2;
const PAGE: u32 = 65536;
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
        if let Operand::Sym(at) = *operand {
            // The value is the symbol's final index or address, patched in by `write`.
            let (symbol, addend) = &symbols[at as usize];
            let kind = match *kind {
                "function32_op" => RelocKind::FunctionIndex,
                "offset32_op" => RelocKind::MemoryAddrLeb,
                "i32imm_op" => RelocKind::MemoryAddrSleb,
                other => panic!("operand class {other} of {} takes no symbol yet", asm.text),
            };
            relocs.push(Reloc { offset: out.len(), symbol: symbol.clone(), kind, addend: *addend });
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocKind {
    /// `R_WASM_FUNCTION_INDEX_LEB`.
    FunctionIndex,
    /// `R_WASM_MEMORY_ADDR_LEB`.
    MemoryAddrLeb,
    /// `R_WASM_MEMORY_ADDR_SLEB`.
    MemoryAddrSleb,
}

impl RelocKind {
    fn code(self) -> u8 {
        match self {
            RelocKind::FunctionIndex => 0,
            RelocKind::MemoryAddrLeb => 3,
            RelocKind::MemoryAddrSleb => 4,
        }
    }
}

/// A relocation in a body: the offset of its five-byte LEB from the start of
/// the body, the symbol it names and the addend of an address.
pub struct Reloc {
    pub offset: usize,
    pub symbol: String,
    pub kind: RelocKind,
    pub addend: i64,
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

/// Where a data object goes, as `TargetLoweringObjectFile` picks its section:
/// `.rodata.NAME` for a constant, `.bss.NAME` for a zero variable, else
/// `.data.NAME`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataKind {
    Data,
    Bss,
    Rodata,
}

/// A defined variable: one data segment.
pub struct DataObject {
    pub name: String,
    pub kind: DataKind,
    /// The log2 of the alignment.
    pub align: u32,
    pub bytes: Vec<u8>,
}

impl DataObject {
    fn segment_name(&self) -> String {
        let prefix = match self.kind {
            DataKind::Data => ".data.",
            DataKind::Bss => ".bss.",
            DataKind::Rodata => ".rodata.",
        };
        format!("{prefix}{}", self.name)
    }
}

/// A symbol, in the order the assembler created it: a function's own when its
/// header is printed, a callee's or a variable's at its first use, then the
/// variables nothing used.
enum Symbol<'a> {
    Function {
        name: &'a str,
        /// The position in `functions` of its definition, `None` for an import.
        defined: Option<usize>,
        params: &'a [ValType],
        results: &'a [ValType],
    },
    Data {
        /// The position in `data`.
        object: usize,
    },
}

fn type_index<'a>(
    types: &mut Vec<(&'a [ValType], &'a [ValType])>,
    signature: (&'a [ValType], &'a [ValType]),
) -> usize {
    types.iter().position(|t| *t == signature).unwrap_or_else(|| {
        types.push(signature);
        types.len() - 1
    })
}

fn sleb_padded(
    out: &mut Vec<u8>,
    value: i32,
) {
    for i in 0..4 {
        out.push((value >> (7 * i)) as u8 & 0x7f | 0x80);
    }
    out.push((value >> 28) as u8 & 0x7f);
}

/// The object file for `functions` and `data`, in order.
pub fn write(
    functions: &[Encoded],
    data: &[DataObject],
) -> Vec<u8> {
    let mut symbols: Vec<Symbol> = Vec::new();
    let named = |symbols: &[Symbol], name: &str| {
        symbols.iter().position(|s| match s {
            Symbol::Function { name: one, .. } => *one == name,
            Symbol::Data { object } => data[*object].name == name,
        })
    };
    for (at, f) in functions.iter().enumerate() {
        let defined = Symbol::Function { name: &f.name, defined: Some(at), params: &f.params, results: &f.results };
        match named(&symbols, &f.name) {
            Some(known) => symbols[known] = defined,
            None => symbols.push(defined),
        }
        for reloc in &f.relocs {
            if named(&symbols, &reloc.symbol).is_some() {
                continue;
            }
            if let Some(object) = data.iter().position(|d| d.name == reloc.symbol) {
                symbols.push(Symbol::Data { object });
                continue;
            }
            let (_, params, results) =
                f.callees.iter().find(|(name, ..)| *name == reloc.symbol).expect("a used symbol is defined or called");
            symbols.push(Symbol::Function { name: &reloc.symbol, defined: None, params, results });
        }
    }
    for (object, one) in data.iter().enumerate() {
        if named(&symbols, &one.name).is_none() {
            symbols.push(Symbol::Data { object });
        }
    }
    let imports: Vec<usize> =
        (0..symbols.len()).filter(|&s| matches!(symbols[s], Symbol::Function { defined: None, .. })).collect();
    // Function indices: the imports first, then the definitions.
    let mut index_of = vec![0u32; symbols.len()];
    for (n, &s) in imports.iter().enumerate() {
        index_of[s] = n as u32;
    }
    for (s, symbol) in symbols.iter().enumerate() {
        if let Symbol::Function { defined: Some(at), .. } = symbol {
            index_of[s] = (imports.len() + at) as u32;
        }
    }
    // Segment addresses: each aligned to its object's alignment.
    let mut address = Vec::with_capacity(data.len());
    let mut end = 0u32;
    for one in data {
        end = end.next_multiple_of(1 << one.align);
        address.push(end);
        end += one.bytes.len() as u32;
    }

    let mut types: Vec<(&[ValType], &[ValType])> = Vec::new();
    let signatures: Vec<usize> = functions.iter().map(|f| type_index(&mut types, (&f.params, &f.results))).collect();
    let import_types: Vec<usize> = imports
        .iter()
        .map(|&s| match &symbols[s] {
            Symbol::Function { params, results, .. } => type_index(&mut types, (params, results)),
            Symbol::Data { .. } => unreachable!("an import is a function"),
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
    uleb(&mut payload, 1 + imports.len() as u64);
    name(&mut payload, "env");
    name(&mut payload, "__linear_memory");
    // Limits: no maximum, the pages the data fills as the minimum.
    payload.extend([EXTERNAL_MEMORY, 0]);
    uleb(&mut payload, u64::from(end.div_ceil(PAGE)));
    for (&s, &ty) in imports.iter().zip(&import_types) {
        let Symbol::Function { name: import, .. } = symbols[s] else { unreachable!("an import is a function") };
        name(&mut payload, "env");
        name(&mut payload, import);
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

    if !data.is_empty() {
        payload.clear();
        uleb(&mut payload, data.len() as u64);
        section(&mut out, SECTION_DATA_COUNT, &payload);
    }

    // The code section, with each relocated value patched in and its offset from the section's start.
    let symbol_of = |name: &str| named(&symbols, name).expect("a symbol");
    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    let mut code_relocs: Vec<(usize, usize, RelocKind, i64)> = Vec::new();
    for f in functions {
        uleb(&mut payload, f.body.len() as u64);
        let start = payload.len();
        payload.extend_from_slice(&f.body);
        for reloc in &f.relocs {
            let s = symbol_of(&reloc.symbol);
            let at = start + reloc.offset;
            let mut value = Vec::new();
            match (&symbols[s], reloc.kind) {
                (Symbol::Function { .. }, RelocKind::FunctionIndex) => uleb_padded(&mut value, index_of[s]),
                (Symbol::Data { object }, RelocKind::MemoryAddrLeb) => {
                    uleb_padded(&mut value, (i64::from(address[*object]) + reloc.addend) as u32)
                }
                (Symbol::Data { object }, RelocKind::MemoryAddrSleb) => {
                    sleb_padded(&mut value, (i64::from(address[*object]) + reloc.addend) as i32)
                }
                _ => panic!("relocation {:?} against symbol {}", reloc.kind, reloc.symbol),
            }
            payload[at..at + 5].copy_from_slice(&value);
            code_relocs.push((at, s, reloc.kind, reloc.addend));
        }
    }
    section(&mut out, SECTION_CODE, &payload);

    if !data.is_empty() {
        payload.clear();
        uleb(&mut payload, data.len() as u64);
        for (one, at) in data.iter().zip(&address) {
            // Active segment of memory 0 at `i32.const address`.
            payload.extend([0, OPCODE_I32_CONST]);
            sleb(&mut payload, i64::from(*at));
            payload.push(OPCODE_END);
            uleb(&mut payload, one.bytes.len() as u64);
            payload.extend_from_slice(&one.bytes);
        }
        section(&mut out, SECTION_DATA, &payload);
    }

    payload.clear();
    payload.push(LINKING_VERSION);
    let mut table = Vec::new();
    uleb(&mut table, symbols.len() as u64);
    for (s, symbol) in symbols.iter().enumerate() {
        match symbol {
            Symbol::Function { name: function, defined, .. } => {
                table.push(SYMBOL_KIND_FUNCTION);
                if defined.is_some() {
                    // Flags (global, default visibility, defined), index, name.
                    table.push(0);
                    uleb(&mut table, u64::from(index_of[s]));
                    name(&mut table, function);
                } else {
                    table.push(WASM_SYMBOL_UNDEFINED);
                    uleb(&mut table, u64::from(index_of[s]));
                }
            }
            Symbol::Data { object } => {
                // Flags, name, segment, offset in it, size.
                table.extend([SYMBOL_KIND_DATA, 0]);
                name(&mut table, &data[*object].name);
                uleb(&mut table, *object as u64);
                uleb(&mut table, 0);
                uleb(&mut table, data[*object].bytes.len() as u64);
            }
        }
    }
    payload.push(WASM_SYMBOL_TABLE);
    uleb_padded(&mut payload, table.len() as u32);
    payload.extend_from_slice(&table);
    if !data.is_empty() {
        let mut info = Vec::new();
        uleb(&mut info, data.len() as u64);
        for one in data {
            name(&mut info, &one.segment_name());
            uleb(&mut info, u64::from(one.align));
            uleb(&mut info, 0);
        }
        payload.push(WASM_SEGMENT_INFO);
        uleb_padded(&mut payload, info.len() as u32);
        payload.extend_from_slice(&info);
    }
    custom(&mut out, "linking", &payload);

    if !code_relocs.is_empty() {
        payload.clear();
        uleb(&mut payload, if data.is_empty() { SECTION_INDEX_CODE } else { SECTION_INDEX_CODE + 1 });
        uleb(&mut payload, code_relocs.len() as u64);
        for &(offset, s, kind, addend) in &code_relocs {
            payload.push(kind.code());
            uleb(&mut payload, offset as u64);
            uleb(&mut payload, s as u64);
            if kind != RelocKind::FunctionIndex {
                sleb(&mut payload, addend);
            }
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
