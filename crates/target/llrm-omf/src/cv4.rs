//! [`Info`] of a C program as CodeView 4 the way Microsoft's C7-era tools write it: $$TYPES numbered from
//! 0x1000 and $$SYMBOLS of 16-bit (`S_GPROC16`) or 32-bit (`S_GPROC32`) records, in the pre-link layout
//! LINK /CO hands to CVPACK. Record layouts are Open Watcom's `bld/watcom/h/cv4f.h`; every shape marked
//! "ML" below was also read from an object ML 6.11 wrote under /Zi (docs/machine/codeview.md).
//!
//! BASIC's compilers write a different, older dialect: `cvwrite` is that one.

use llrm_object::debug::{self as model, Info, Location, Reach, Scalar, Type, TypeId};
use llrm_object::{Binding, Kind as Fixup, Object, Reloc, Role, Section, Target};
use llrm_support::leaf::{numeric, pad};

use crate::codeview::{SYMBOLS, TYPES};
use crate::write::Error;

const SIGNATURE: [u8; 4] = [1, 0, 0, 0];
const FIRST_TYPE: usize = 0x1000;

const LF_MODIFIER: u16 = 0x0001;
const LF_POINTER: u16 = 0x0002;
const LF_ARRAY: u16 = 0x0003;
const LF_STRUCTURE: u16 = 0x0005;
const LF_UNION: u16 = 0x0006;
const LF_ENUM: u16 = 0x0007;
const LF_PROCEDURE: u16 = 0x0008;
const LF_ARGLIST: u16 = 0x0201;
const LF_FIELDLIST: u16 = 0x0204;
const LF_BITFIELD: u16 = 0x0206;
const LF_ENUMERATE: u16 = 0x0403;
const LF_MEMBER: u16 = 0x0406;

const S_COMPILE: u16 = 0x0001;
const S_REGISTER: u16 = 0x0002;
const S_UDT: u16 = 0x0004;
const S_END: u16 = 0x0006;
const S_OBJNAME: u16 = 0x0009;

const T_VOID: u16 = 0x0003;
/// A public member: what ML writes for every member.
const PUBLIC: u16 = 3;

fn refused<T>(what: impl std::fmt::Display) -> Result<T, Error> {
    Err(Error::Unencodable(format!("CodeView 4: {what}")))
}

fn narrow<T: TryFrom<i64>>(value: i64, what: &str) -> Result<T, Error> {
    T::try_from(value).or_else(|_| refused(format!("{what} {value} does not fit its field")))
}

/// A length-prefixed name in latin-1.
fn pascal(out: &mut Vec<u8>, text: &str) -> Result<(), Error> {
    let bytes = text.chars().map(|one| u8::try_from(u32::from(one)).or_else(|_| refused(format!("{text:?} is not latin-1")))).collect::<Result<Vec<u8>, Error>>()?;
    out.push(narrow(bytes.len() as i64, &format!("the length of {text:?}"))?);
    out.extend(bytes);
    Ok(())
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes());
}

/// A member of a list or a record's end, padded so the next starts on four bytes.
fn align(list: &mut Vec<u8>) {
    while list.len() % 4 != 0 {
        list.push(pad(4 - list.len() % 4));
    }
}

/// The calling convention's CV_call_e for a near procedure; `far` is the next one.
fn convention(name: Option<&str>) -> Result<u8, Error> {
    Ok(match name {
        None | Some("cdecl") => 0x00,
        Some("pascal") => 0x02,
        Some("fastcall") => 0x04,
        Some("stdcall") => 0x07,
        Some(other) => return refused(format!("no calling convention code for {other}")),
    })
}

struct Types<'a> {
    info: &'a Info,
    /// 32-bit code: a four-byte integer is an int, a near pointer is four bytes.
    wide: bool,
    /// Each record's leaf and data, before the length and the padding; empty until made.
    records: Vec<Vec<u8>>,
    /// By model type; None until made, and a struct's is set before its members are.
    index: Vec<Option<u16>>,
    /// A procedure type by the model's and whether it is far: far is the function's, not the type's.
    procedures: Vec<((TypeId, bool), u16)>,
}

impl Types<'_> {
    fn reserve(&mut self) -> Result<u16, Error> {
        let at = FIRST_TYPE + self.records.len();
        self.records.push(Vec::new());
        narrow(at as i64, "a type index")
    }

    fn fill(&mut self, at: u16, leaf: u16, data: &[u8]) {
        let mut record = leaf.to_le_bytes().to_vec();
        record.extend(data);
        self.records[usize::from(at) - FIRST_TYPE] = record;
    }

    fn add(&mut self, leaf: u16, data: &[u8]) -> Result<u16, Error> {
        let at = self.reserve()?;
        self.fill(at, leaf, data);
        Ok(at)
    }

    /// The primitive index of a scalar.
    fn primitive(&self, scalar: Scalar) -> Result<u16, Error> {
        Ok(match scalar {
            Scalar::Void => T_VOID,
            Scalar::Bool { bytes: 1 } => 0x30,
            Scalar::Bool { bytes: 2 } => 0x31,
            Scalar::Bool { bytes: 4 } => 0x32,
            Scalar::Char => 0x70,
            Scalar::Int { bytes: 1, signed: true } => 0x10,
            Scalar::Int { bytes: 1, signed: false } => 0x20,
            Scalar::Int { bytes: 2, signed: true } => 0x11,
            Scalar::Int { bytes: 2, signed: false } => 0x21,
            // A 32-bit program's four-byte integer is an int; a 16-bit one's is a long.
            Scalar::Int { bytes: 4, signed: true } => if self.wide { 0x74 } else { 0x12 },
            Scalar::Int { bytes: 4, signed: false } => if self.wide { 0x75 } else { 0x22 },
            Scalar::Int { bytes: 8, signed: true } => 0x13,
            Scalar::Int { bytes: 8, signed: false } => 0x23,
            Scalar::Float { bytes: 4 } => 0x40,
            Scalar::Float { bytes: 8 } => 0x41,
            Scalar::Float { bytes: 10 } => 0x42,
            other => return refused(format!("no primitive for {other:?}")),
        })
    }

    /// `id`'s index, made with what it names first. A type reached again through a struct that names it (a
    /// cycle) is already made when its own turn comes: each composite looks again before it adds its record, or
    /// there would be two of it.
    fn of(&mut self, id: TypeId) -> Result<u16, Error> {
        if let Some(done) = self.index[id] {
            return Ok(done);
        }
        let one = self.info.types.get(id).ok_or_else(|| Error::Unencodable(format!("CodeView 4: type {id} is not in the model")))?.clone();
        let made = match one {
            Type::Scalar(scalar) => self.primitive(scalar)?,
            Type::Typedef { target, .. } => self.of(target)?,
            Type::Pointer { target, bytes, reach } => {
                let target = self.of(target)?;
                if let Some(done) = self.index[id] {
                    return Ok(done);
                }
                self.pointer(target, bytes, reach, 0)?
            }
            Type::Reference(target) => {
                let target = self.of(target)?;
                if let Some(done) = self.index[id] {
                    return Ok(done);
                }
                self.pointer(target, if self.wide { 4 } else { 2 }, Reach::Near, 1)?
            }
            Type::Qualified { target, constant, volatile } => {
                let target = self.of(target)?;
                if let Some(done) = self.index[id] {
                    return Ok(done);
                }
                let mut data = Vec::new();
                put16(&mut data, u16::from(constant) | u16::from(volatile) << 1);
                put16(&mut data, target);
                self.add(LF_MODIFIER, &data)?
            }
            Type::Array { element, bytes } => {
                let Some(bytes) = bytes else { return refused("BASIC's array, whose bounds are its descriptor's") };
                let element = self.of(element)?;
                if let Some(done) = self.index[id] {
                    return Ok(done);
                }
                let mut data = Vec::new();
                put16(&mut data, element);
                // The index type, T_USHORT or T_ULONG: the width of an offset.
                put16(&mut data, if self.wide { 0x22 } else { 0x21 });
                numeric(&mut data, i64::from(bytes));
                pascal(&mut data, "")?;
                self.sealed(LF_ARRAY, data)?
            }
            Type::FixedString(_) => return refused("BASIC's STRING * n"),
            Type::Enum { name, underlying, enumerators } => {
                let underlying = self.of(underlying)?;
                let mut list = Vec::new();
                for one in &enumerators {
                    put16(&mut list, LF_ENUMERATE);
                    put16(&mut list, PUBLIC);
                    numeric(&mut list, one.value);
                    pascal(&mut list, &one.name)?;
                    align(&mut list);
                }
                let list = self.add(LF_FIELDLIST, &list)?;
                let mut data = Vec::new();
                put16(&mut data, narrow(enumerators.len() as i64, "an enum's enumerator count")?);
                put16(&mut data, underlying);
                put16(&mut data, list);
                put16(&mut data, 0);
                pascal(&mut data, &name)?;
                self.sealed(LF_ENUM, data)?
            }
            Type::Struct { name, bytes, fields, union } => return self.structure(id, &name, bytes, &fields, union),
            Type::Procedure { .. } => self.procedure(id, false)?,
        };
        self.index[id] = Some(made);
        Ok(made)
    }

    /// A record whose end is padded to the four bytes the next starts on.
    fn sealed(&mut self, leaf: u16, mut data: Vec<u8>) -> Result<u16, Error> {
        // The length field and the leaf are four bytes before the data.
        while (4 + data.len()) % 4 != 0 {
            data.push(pad(4 - (4 + data.len()) % 4));
        }
        self.add(leaf, &data)
    }

    /// A pointer of `bytes` to `target`: a primitive pointer where the target is a primitive (what C7 writes),
    /// else LF_POINTER, whose four bytes after the type ML writes as zeros.
    fn pointer(&mut self, target: u16, bytes: u8, reach: Reach, mode: u16) -> Result<u16, Error> {
        // CV_ptrtype_e and the primitive pointer's mode, by width.
        let (kind, mode_of_primitive): (u16, u16) = match (reach, bytes) {
            (Reach::Near, 2) => (0, 1),
            (Reach::Far, 4) => (1, 2),
            (Reach::Huge, 4) => (2, 3),
            (Reach::Near, 4) => (10, 4),
            (Reach::Far, 6) => (11, 5),
            (reach, bytes) => return refused(format!("a {bytes}-byte {reach:?} pointer has no kind")),
        };
        if mode == 0 && target < 0x100 && target & 0x700 == 0 && target != 0 {
            return Ok(target | mode_of_primitive << 8);
        }
        let mut data = Vec::new();
        put16(&mut data, kind | mode << 5);
        put16(&mut data, target);
        put32(&mut data, 0);
        self.sealed(LF_POINTER, data)
    }

    /// The model's procedure `id`, called far or near.
    fn procedure(&mut self, id: TypeId, far: bool) -> Result<u16, Error> {
        if let Some(&(_, done)) = self.procedures.iter().find(|(key, _)| *key == (id, far)) {
            return Ok(done);
        }
        let Type::Procedure { result, parameters, convention: called } = self.info.types[id].clone() else {
            return refused(format!("type {id} is no procedure"));
        };
        let result = match result {
            Some(one) => self.of(one)?,
            None => T_VOID,
        };
        let parameters = parameters.iter().map(|&one| self.of(one)).collect::<Result<Vec<_>, _>>()?;
        // A cycle through a struct made it while its parameters were being.
        if let Some(&(_, done)) = self.procedures.iter().find(|(key, _)| *key == (id, far)) {
            return Ok(done);
        }
        let mut list = Vec::new();
        put16(&mut list, narrow(parameters.len() as i64, "a parameter count")?);
        parameters.iter().for_each(|&one| put16(&mut list, one));
        let list = self.sealed(LF_ARGLIST, list)?;
        let mut data = Vec::new();
        put16(&mut data, result);
        data.push(convention(called.as_deref())? + u8::from(far));
        data.push(0);
        put16(&mut data, narrow(parameters.len() as i64, "a parameter count")?);
        put16(&mut data, list);
        let made = self.sealed(LF_PROCEDURE, data)?;
        self.procedures.push(((id, far), made));
        Ok(made)
    }

    /// A struct or union. Its index is taken before its members are made, so one that points back to it (a list's
    /// `next`) names it, as ML names a record before the field list that follows it.
    fn structure(&mut self, id: TypeId, name: &str, bytes: u32, fields: &[model::Field], union: bool) -> Result<u16, Error> {
        let at = self.reserve()?;
        self.index[id] = Some(at);
        let mut list = Vec::new();
        for field in fields {
            let mut kind = self.of(field.r#type)?;
            if let Some((start, width)) = field.bits {
                let mut data = vec![width, start];
                put16(&mut data, kind);
                kind = self.add(LF_BITFIELD, &data)?;
            }
            put16(&mut list, LF_MEMBER);
            put16(&mut list, kind);
            put16(&mut list, PUBLIC);
            numeric(&mut list, i64::from(field.offset));
            pascal(&mut list, &field.name)?;
            align(&mut list);
        }
        let list = self.add(LF_FIELDLIST, &list)?;
        let mut data = Vec::new();
        put16(&mut data, narrow(fields.len() as i64, "a struct's member count")?);
        put16(&mut data, list);
        put16(&mut data, 0);
        if !union {
            // The derived list and the vshape: none.
            put16(&mut data, 0);
            put16(&mut data, 0);
        }
        numeric(&mut data, i64::from(bytes));
        pascal(&mut data, name)?;
        while (4 + data.len()) % 4 != 0 {
            data.push(pad(4 - (4 + data.len()) % 4));
        }
        self.fill(at, if union { LF_UNION } else { LF_STRUCTURE }, &data);
        Ok(at)
    }

    /// The table's bytes: each record's length, leaf and data.
    fn bytes(&self) -> Result<Vec<u8>, Error> {
        let mut out = SIGNATURE.to_vec();
        for (at, record) in self.records.iter().enumerate() {
            let length = u16::try_from(record.len()).or_else(|_| refused(format!("type {:#x} is {} bytes, which its 16-bit length cannot say", FIRST_TYPE + at, record.len())))?;
            put16(&mut out, length);
            out.extend(record);
        }
        Ok(out)
    }
}

struct Symbols<'a> {
    object: &'a Object,
    info: &'a Info,
    wide: bool,
    /// Past the signature.
    bytes: Vec<u8>,
    relocs: Vec<Reloc>,
}

impl Symbols<'_> {
    fn record(&mut self, code: u16, data: &[u8]) -> Result<usize, Error> {
        let length = narrow(2 + data.len() as i64, "a symbol record's length")?;
        put16(&mut self.bytes, length);
        put16(&mut self.bytes, code);
        let at = self.bytes.len();
        self.bytes.extend(data);
        Ok(at)
    }

    /// An address field at `at` (offset, then segment: two and two bytes, or four and two) that LINK fills in with
    /// the place `target` plus `addend` has.
    fn address(&mut self, at: usize, target: Target, addend: i64) {
        if self.wide {
            self.relocs.push(Reloc { at, kind: Fixup::Abs { width: 4 }, target, addend });
            self.relocs.push(Reloc { at: at + 4, kind: Fixup::SegmentBase, target, addend: 0 });
        } else {
            self.relocs.push(Reloc { at, kind: Fixup::FarPointer, target, addend: i64::from(addend as i16) });
        }
    }

    /// The offset and the segment of an address, as zeros the fixups fill.
    fn zero_address(&self, data: &mut Vec<u8>) {
        data.extend(if self.wide { &[0u8; 6][..] } else { &[0u8; 4][..] });
    }

    fn scope_prefix(&self, data: &mut Vec<u8>) {
        // pParent, pEnd, pNext: LINK and CVPACK fill them in, as ML leaves them.
        data.extend([0u8; 12]);
    }
}

/// A register's CV_HREG number in the target description.
fn register_number(info: &Info, name: &str) -> Result<u16, Error> {
    match info.registers.iter().find(|one| one.name == name).and_then(|one| one.codeview) {
        Some(number) => Ok(number),
        None => refused(format!("register {name} has no CodeView number")),
    }
}

impl Symbols<'_> {
    /// The code of a data symbol and its fixed field: a global is `S_GDATA`, a static `S_LDATA`.
    fn data(&mut self, types: &mut Types, variable: &model::Variable, symbol: usize, disp: i64) -> Result<(), Error> {
        let global = self.object.symbols[symbol].binding == Binding::Public;
        let mut data = Vec::new();
        self.zero_address(&mut data);
        put16(&mut data, types.of(variable.r#type)?);
        pascal(&mut data, &variable.name)?;
        let code = match (self.wide, global) {
            (false, false) => 0x0101,
            (false, true) => 0x0102,
            (true, false) => 0x0201,
            (true, true) => 0x0202,
        };
        let at = self.record(code, &data)?;
        self.address(at, Target::Symbol(symbol), disp);
        Ok(())
    }

    /// `variable` in the scope that `scope` covers. A value CodeView 4 has no record to place over a range of code
    /// (its records name one place for the whole scope) is left out: one that is in a register only part of the
    /// scope, and one the optimiser removed.
    fn variable(&mut self, types: &mut Types, variable: &model::Variable, scope: &[model::Range]) -> Result<(), Error> {
        match &variable.location {
            Location::Static { symbol, disp } => self.data(types, variable, *symbol, *disp),
            Location::Register(register) => self.register(types, variable, register),
            Location::List(entries) => match &entries[..] {
                [(range, Location::Register(register))] if scope == [*range] => self.register(types, variable, register),
                _ => Ok(()),
            },
            Location::Frame { disp } => {
                let mut data = Vec::new();
                let frame = &self.info.frame_register;
                // The frame register is BP's, or another's, which a register-relative record names.
                let based = matches!(frame.as_str(), "ebp" | "bp");
                if based {
                    if self.wide {
                        put32(&mut data, narrow::<i32>(*disp, "a frame offset")? as u32);
                    } else {
                        data.extend(narrow::<i16>(*disp, "a frame offset")?.to_le_bytes());
                    }
                } else {
                    let number = register_number(self.info, frame)?;
                    if self.wide {
                        put32(&mut data, narrow::<i32>(*disp, "a frame offset")? as u32);
                    } else {
                        data.extend(narrow::<i16>(*disp, "a frame offset")?.to_le_bytes());
                    }
                    put16(&mut data, number);
                }
                put16(&mut data, types.of(variable.r#type)?);
                pascal(&mut data, &variable.name)?;
                let code = match (self.wide, based) {
                    (false, true) => 0x0100,
                    (false, false) => 0x010C,
                    (true, true) => 0x0200,
                    (true, false) => 0x020C,
                };
                self.record(code, &data)?;
                Ok(())
            }
        }
    }

    fn register(&mut self, types: &mut Types, variable: &model::Variable, register: &str) -> Result<(), Error> {
        let mut data = Vec::new();
        put16(&mut data, types.of(variable.r#type)?);
        put16(&mut data, register_number(self.info, register)?);
        pascal(&mut data, &variable.name)?;
        self.record(S_REGISTER, &data)?;
        Ok(())
    }

    fn block(&mut self, types: &mut Types, block: &model::Block) -> Result<(), Error> {
        let [range] = block.ranges[..] else { return refused("a lexical block in pieces") };
        let mut data = Vec::new();
        self.scope_prefix(&mut data);
        data.truncate(8);
        if self.wide {
            put32(&mut data, narrow(range.length as i64, "a block's length")?);
            put32(&mut data, 0);
            data.extend([0, 0]);
        } else {
            put16(&mut data, narrow(range.length as i64, "a block's length")?);
            data.extend([0, 0, 0, 0]);
        }
        pascal(&mut data, "")?;
        let at = self.record(if self.wide { 0x0207 } else { 0x0107 }, &data)?;
        self.address(at + 8 + if self.wide { 4 } else { 2 }, Target::Section(range.section), range.offset as i64);
        for variable in &block.variables {
            self.variable(types, variable, &block.ranges)?;
        }
        for inner in &block.blocks {
            self.block(types, inner)?;
        }
        self.record(S_END, &[])?;
        Ok(())
    }

    fn function(&mut self, types: &mut Types, function: &model::Function) -> Result<(), Error> {
        let [range] = function.ranges[..] else { return refused(format!("{} is in pieces", function.name)) };
        let (start, end) = function.body.unwrap_or((0, range.length));
        let mut data = Vec::new();
        self.scope_prefix(&mut data);
        let (length, start, end): (Vec<u8>, Vec<u8>, Vec<u8>) = if self.wide {
            (narrow::<u32>(range.length as i64, "a procedure's length")?.to_le_bytes().to_vec(), narrow::<u32>(start as i64, "a body's start")?.to_le_bytes().to_vec(), narrow::<u32>(end as i64, "a body's end")?.to_le_bytes().to_vec())
        } else {
            (narrow::<u16>(range.length as i64, "a procedure's length")?.to_le_bytes().to_vec(), narrow::<u16>(start as i64, "a body's start")?.to_le_bytes().to_vec(), narrow::<u16>(end as i64, "a body's end")?.to_le_bytes().to_vec())
        };
        data.extend(length);
        data.extend(start);
        data.extend(end);
        let address = data.len();
        self.zero_address(&mut data);
        put16(&mut data, types.procedure(function.r#type, function.far)?);
        // Bit 2: it returns far.
        data.push(if function.far { 0x04 } else { 0x00 });
        pascal(&mut data, &function.name)?;
        let global = self.object.symbols[function.symbol].binding == Binding::Public;
        let code = match (self.wide, global) {
            (false, false) => 0x0104,
            (false, true) => 0x0105,
            (true, false) => 0x0204,
            (true, true) => 0x0205,
        };
        let at = self.record(code, &data)?;
        self.address(at + address, Target::Section(range.section), range.offset as i64);
        for variable in &function.variables {
            self.variable(types, variable, &function.ranges)?;
        }
        for block in &function.blocks {
            self.block(types, block)?;
        }
        self.record(S_END, &[])?;
        Ok(())
    }
}

/// `info`'s two debug sections for `object`, $$SYMBOLS then $$TYPES.
pub fn sections(object: &Object, info: &Info) -> Result<[Section; 2], Error> {
    let wide = object.arch.bits() == 32;
    let mut types = Types { info, wide, records: Vec::new(), index: vec![None; info.types.len()], procedures: Vec::new() };
    let mut symbols = Symbols { object, info, wide, bytes: Vec::new(), relocs: Vec::new() };
    let mut name = SIGNATURE.to_vec();
    pascal(&mut name, &object.name)?;
    symbols.record(S_OBJNAME, &name)?;
    // The machine (80386: llrm's real-mode code is 386 code too), the language (C), the flags (32-bit mode) and
    // the producer's name.
    let mut compile = vec![3, 0];
    put16(&mut compile, if wide { 0x0800 } else { 0 });
    pascal(&mut compile, "llrm")?;
    symbols.record(S_COMPILE, &compile)?;
    for global in &info.globals {
        if let Location::Static { symbol, disp } = &global.location {
            symbols.data(&mut types, global, *symbol, *disp)?;
        }
    }
    for function in &info.functions {
        if function.module {
            for variable in &function.variables {
                symbols.variable(&mut types, variable, &function.ranges)?;
            }
            continue;
        }
        symbols.function(&mut types, function)?;
    }
    // A name for a type: a typedef's, a struct's, a union's, an enum's.
    for (id, one) in info.types.iter().enumerate() {
        let name = match one {
            Type::Typedef { name, .. } | Type::Struct { name, .. } | Type::Enum { name, .. } if !name.is_empty() => name,
            _ => continue,
        };
        let mut data = Vec::new();
        put16(&mut data, types.of(id)?);
        pascal(&mut data, name)?;
        symbols.record(S_UDT, &data)?;
    }
    let section = |name: &str, image: Vec<u8>, relocs: Vec<Reloc>| {
        let spans = vec![[0, image.len()]];
        Section { name: name.to_owned(), role: Role::Debug, near: true, align: 1, image, spans, relocs }
    };
    let image = [&SIGNATURE[..], &symbols.bytes].concat();
    let relocs = symbols.relocs.into_iter().map(|one| Reloc { at: one.at + SIGNATURE.len(), ..one }).collect();
    Ok([section(SYMBOLS, image, relocs), section(TYPES, types.bytes()?, Vec::new())])
}

#[cfg(test)]
#[path = "cv4_tests.rs"]
mod cv4_tests;
