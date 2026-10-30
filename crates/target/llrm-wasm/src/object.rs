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
const EXTERNAL_GLOBAL: u8 = 3;
const SYMBOL_KIND_GLOBAL: u8 = 2;
const WASM_SYMBOL_BINDING_LOCAL: u8 = 2;
const R_WASM_MEMORY_ADDR_I32: u8 = 5;
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
                "global_op32" | "global_op64" => RelocKind::GlobalIndexLeb,
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
    /// The wasm globals the body names.
    pub globals: Vec<(String, ValType)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocKind {
    /// `R_WASM_FUNCTION_INDEX_LEB`.
    FunctionIndex,
    /// `R_WASM_MEMORY_ADDR_LEB`.
    MemoryAddrLeb,
    /// `R_WASM_MEMORY_ADDR_SLEB`.
    MemoryAddrSleb,
    /// `R_WASM_GLOBAL_INDEX_LEB`.
    GlobalIndexLeb,
}

impl RelocKind {
    fn code(self) -> u8 {
        match self {
            RelocKind::FunctionIndex => 0,
            RelocKind::MemoryAddrLeb => 3,
            RelocKind::MemoryAddrSleb => 4,
            RelocKind::GlobalIndexLeb => 7,
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
        globals: f.globals.clone(),
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
    /// The assembler name: `.L` first for a private one.
    pub name: String,
    pub kind: DataKind,
    /// The log2 of the alignment.
    pub align: u32,
    pub bytes: Vec<u8>,
    /// Internal or private: `WASM_SYMBOL_BINDING_LOCAL`.
    pub local: bool,
    /// A mergeable C string: `WASM_SEGMENT_FLAG_STRINGS`.
    pub strings: bool,
    /// The pointers in `bytes`: `R_WASM_MEMORY_ADDR_I32` at each.
    pub relocs: Vec<DataReloc>,
}

/// A pointer in a data object: where it is, what it points to and how far in.
pub struct DataReloc {
    pub offset: usize,
    pub symbol: String,
    pub addend: i64,
}

/// A module's variables: the defined ones, and the names of the declared ones.
#[derive(Default)]
pub struct Data {
    pub objects: Vec<DataObject>,
    pub undefined: Vec<String>,
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
    /// A variable declared here and defined elsewhere.
    UndefinedData {
        name: &'a str,
    },
    /// A wasm global the module imports.
    Global {
        name: &'a str,
        ty: ValType,
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

/// Adds the symbol `name` if the assembler would have just created it: a variable of the module, a wasm global
/// or a called function (`f` has the signature).
fn introduce<'a>(
    symbols: &mut Vec<Symbol<'a>>,
    data: &'a Data,
    f: Option<&'a Encoded>,
    name: &'a str,
) {
    let known = symbols.iter().any(|s| match s {
        Symbol::Function { name: one, .. } | Symbol::UndefinedData { name: one } | Symbol::Global { name: one, .. } => {
            *one == name
        }
        Symbol::Data { object } => data.objects[*object].name == name,
    });
    if known {
        return;
    }
    if let Some(object) = data.objects.iter().position(|d| d.name == name) {
        symbols.push(Symbol::Data { object });
    } else if data.undefined.iter().any(|d| d == name) {
        symbols.push(Symbol::UndefinedData { name });
    } else if let Some((_, ty)) = f.and_then(|f| f.globals.iter().find(|(one, _)| one == name)) {
        symbols.push(Symbol::Global { name, ty: *ty });
    } else if let Some((_, params, results)) = f.and_then(|f| f.callees.iter().find(|(one, ..)| one == name)) {
        symbols.push(Symbol::Function { name, defined: None, params, results });
    } else {
        panic!("symbol {name} is no variable, global or called function");
    }
}

/// The object file for `functions` and `data`, in order.
pub fn write(
    functions: &[Encoded],
    data: &Data,
) -> Vec<u8> {
    let objects = &data.objects;
    let mut symbols: Vec<Symbol> = Vec::new();
    for (at, f) in functions.iter().enumerate() {
        let defined = Symbol::Function { name: &f.name, defined: Some(at), params: &f.params, results: &f.results };
        let known = symbols.iter().position(|s| matches!(s, Symbol::Function { name, .. } if *name == f.name));
        match known {
            Some(known) => symbols[known] = defined,
            None => symbols.push(defined),
        }
        for reloc in &f.relocs {
            introduce(&mut symbols, data, Some(f), &reloc.symbol);
        }
    }
    for one in objects {
        introduce(&mut symbols, data, None, &one.name);
        for reloc in &one.relocs {
            introduce(&mut symbols, data, None, &reloc.symbol);
        }
    }
    let is_import = |symbol: &Symbol| matches!(symbol, Symbol::Function { defined: None, .. } | Symbol::Global { .. });
    let imports: Vec<usize> = (0..symbols.len()).filter(|&s| is_import(&symbols[s])).collect();
    // Function and global indices: the imports first, then the definitions.
    let mut index_of = vec![0u32; symbols.len()];
    let (mut functions_imported, mut globals_imported) = (0u32, 0u32);
    for &s in &imports {
        match symbols[s] {
            Symbol::Global { .. } => {
                index_of[s] = globals_imported;
                globals_imported += 1;
            }
            _ => {
                index_of[s] = functions_imported;
                functions_imported += 1;
            }
        }
    }
    for (s, symbol) in symbols.iter().enumerate() {
        if let Symbol::Function { defined: Some(at), .. } = symbol {
            index_of[s] = functions_imported + *at as u32;
        }
    }
    let position = |name: &str| {
        symbols
            .iter()
            .position(|s| match s {
                Symbol::Function { name: one, .. }
                | Symbol::UndefinedData { name: one }
                | Symbol::Global { name: one, .. } => *one == name,
                Symbol::Data { object } => objects[*object].name == name,
            })
            .expect("a symbol")
    };
    // Segment addresses: each aligned to its object's alignment.
    let mut address = Vec::with_capacity(objects.len());
    let mut end = 0u32;
    for one in objects {
        end = end.next_multiple_of(1 << one.align);
        address.push(end);
        end += one.bytes.len() as u32;
    }
    // The address of a symbol: where its segment is, nothing for an undefined one.
    let address_of = |s: usize| match symbols[s] {
        Symbol::Data { object } => i64::from(address[object]),
        _ => 0,
    };

    let mut types: Vec<(&[ValType], &[ValType])> = Vec::new();
    let signatures: Vec<usize> = functions.iter().map(|f| type_index(&mut types, (&f.params, &f.results))).collect();
    let import_types: Vec<usize> = imports
        .iter()
        .map(|&s| match &symbols[s] {
            Symbol::Function { params, results, .. } => type_index(&mut types, (params, results)),
            _ => 0,
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
        name(&mut payload, "env");
        match symbols[s] {
            Symbol::Function { name: import, .. } => {
                name(&mut payload, import);
                payload.push(EXTERNAL_FUNCTION);
                uleb(&mut payload, ty as u64);
            }
            Symbol::Global { name: import, ty } => {
                name(&mut payload, import);
                // Mutable: the stack pointer.
                payload.extend([EXTERNAL_GLOBAL, val_type(ty), 1]);
            }
            _ => unreachable!("an import is a function or a global"),
        }
    }
    section(&mut out, SECTION_IMPORT, &payload);

    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    for &signature in &signatures {
        uleb(&mut payload, signature as u64);
    }
    section(&mut out, SECTION_FUNCTION, &payload);

    if !objects.is_empty() {
        payload.clear();
        uleb(&mut payload, objects.len() as u64);
        section(&mut out, SECTION_DATA_COUNT, &payload);
    }

    // The code section, with each relocated value patched in and its offset from the section's start.
    payload.clear();
    uleb(&mut payload, functions.len() as u64);
    let mut code_relocs: Vec<(usize, usize, RelocKind, i64)> = Vec::new();
    for f in functions {
        uleb(&mut payload, f.body.len() as u64);
        let start = payload.len();
        payload.extend_from_slice(&f.body);
        for reloc in &f.relocs {
            let s = position(&reloc.symbol);
            let at = start + reloc.offset;
            let mut value = Vec::new();
            match reloc.kind {
                RelocKind::FunctionIndex | RelocKind::GlobalIndexLeb => uleb_padded(&mut value, index_of[s]),
                RelocKind::MemoryAddrLeb => uleb_padded(&mut value, (address_of(s) + reloc.addend) as u32),
                RelocKind::MemoryAddrSleb => sleb_padded(&mut value, (address_of(s) + reloc.addend) as i32),
            }
            payload[at..at + 5].copy_from_slice(&value);
            code_relocs.push((at, s, reloc.kind, reloc.addend));
        }
    }
    section(&mut out, SECTION_CODE, &payload);

    // The data section; each pointer is patched in and recorded by its offset from the section's start.
    let mut data_relocs: Vec<(usize, usize, i64)> = Vec::new();
    if !objects.is_empty() {
        payload.clear();
        uleb(&mut payload, objects.len() as u64);
        for (one, at) in objects.iter().zip(&address) {
            // Active segment of memory 0 at `i32.const address`.
            payload.extend([0, OPCODE_I32_CONST]);
            sleb(&mut payload, i64::from(*at));
            payload.push(OPCODE_END);
            uleb(&mut payload, one.bytes.len() as u64);
            let start = payload.len();
            payload.extend_from_slice(&one.bytes);
            for reloc in &one.relocs {
                let s = position(&reloc.symbol);
                let value = (address_of(s) + reloc.addend) as u32;
                payload[start + reloc.offset..start + reloc.offset + 4].copy_from_slice(&value.to_le_bytes());
                data_relocs.push((start + reloc.offset, s, reloc.addend));
            }
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
            Symbol::Global { .. } => {
                table.extend([SYMBOL_KIND_GLOBAL, WASM_SYMBOL_UNDEFINED]);
                uleb(&mut table, u64::from(index_of[s]));
            }
            Symbol::Data { object } => {
                // Flags, name, segment, offset in it, size.
                let one = &objects[*object];
                table.extend([SYMBOL_KIND_DATA, if one.local { WASM_SYMBOL_BINDING_LOCAL } else { 0 }]);
                name(&mut table, &one.name);
                uleb(&mut table, *object as u64);
                uleb(&mut table, 0);
                uleb(&mut table, one.bytes.len() as u64);
            }
            Symbol::UndefinedData { name: one } => {
                table.extend([SYMBOL_KIND_DATA, WASM_SYMBOL_UNDEFINED]);
                name(&mut table, one);
            }
        }
    }
    payload.push(WASM_SYMBOL_TABLE);
    uleb_padded(&mut payload, table.len() as u32);
    payload.extend_from_slice(&table);
    if !objects.is_empty() {
        let mut info = Vec::new();
        uleb(&mut info, objects.len() as u64);
        for one in objects {
            name(&mut info, &one.segment_name());
            uleb(&mut info, u64::from(one.align));
            uleb(&mut info, u64::from(one.strings));
        }
        payload.push(WASM_SEGMENT_INFO);
        uleb_padded(&mut payload, info.len() as u32);
        payload.extend_from_slice(&info);
    }
    custom(&mut out, "linking", &payload);

    let code_section = if objects.is_empty() { SECTION_INDEX_CODE } else { SECTION_INDEX_CODE + 1 };
    if !code_relocs.is_empty() {
        payload.clear();
        uleb(&mut payload, code_section);
        uleb(&mut payload, code_relocs.len() as u64);
        for &(offset, s, kind, addend) in &code_relocs {
            payload.push(kind.code());
            uleb(&mut payload, offset as u64);
            uleb(&mut payload, s as u64);
            if matches!(kind, RelocKind::MemoryAddrLeb | RelocKind::MemoryAddrSleb) {
                sleb(&mut payload, addend);
            }
        }
        custom(&mut out, "reloc.CODE", &payload);
    }
    if !data_relocs.is_empty() {
        payload.clear();
        uleb(&mut payload, code_section + 1);
        uleb(&mut payload, data_relocs.len() as u64);
        for &(offset, s, addend) in &data_relocs {
            payload.push(R_WASM_MEMORY_ADDR_I32);
            uleb(&mut payload, offset as u64);
            uleb(&mut payload, s as u64);
            sleb(&mut payload, addend);
        }
        custom(&mut out, "reloc.DATA", &payload);
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
