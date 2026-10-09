//! `.debug$T`: the model's types as `LF_*` records, numbered from 0x1000. A
//! scalar is a primitive index and has no record.

use llrm_object::debug::{Info, Reach, Scalar, Type, TypeId};
use llrm_object::{Object, Unsupported};
use llrm_support::leaf::{numeric, pad};

use super::{name, put16, put32, record, refused};

const FIRST: u32 = 0x1000;
const LF_MODIFIER: u16 = 0x1001;
const LF_POINTER: u16 = 0x1002;
const LF_PROCEDURE: u16 = 0x1008;
const LF_ARGLIST: u16 = 0x1201;
const LF_FIELDLIST: u16 = 0x1203;
const LF_BITFIELD: u16 = 0x1205;
const LF_ENUMERATE: u16 = 0x1502;
const LF_ARRAY: u16 = 0x1503;
const LF_STRUCTURE: u16 = 0x1505;
const LF_UNION: u16 = 0x1506;
const LF_ENUM: u16 = 0x1507;
const LF_MEMBER: u16 = 0x150D;
const FORWARD_REFERENCE: u16 = 0x80;
const T_VOID: u32 = 0x03;
const T_UQUAD: u32 = 0x23;

/// What the types came to: their records, and each model type's index.
pub struct Types {
    pub records: Vec<u8>,
    /// By the model's `TypeId`.
    pub index: Vec<u32>,
}

fn primitive(scalar: Scalar) -> Result<u32, Unsupported> {
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
        Scalar::Int { bytes: 4, signed: true } => 0x74,
        Scalar::Int { bytes: 4, signed: false } => 0x75,
        Scalar::Int { bytes: 8, signed: true } => 0x13,
        Scalar::Int { bytes: 8, signed: false } => 0x23,
        Scalar::Float { bytes: 4 } => 0x40,
        Scalar::Float { bytes: 8 } => 0x41,
        Scalar::Float { bytes: 10 } => 0x42,
        other => return refused(format!("no primitive type for {other:?}")),
    })
}

/// CV_call_e of a convention's C name; none is the C default.
fn convention(name: Option<&str>) -> Result<u8, Unsupported> {
    Ok(match name {
        None | Some("cdecl") => 0x00,
        Some("pascal") => 0x02,
        Some("fastcall") => 0x04,
        Some("stdcall") => 0x07,
        Some("thiscall") => 0x0B,
        Some(other) => return refused(format!("no calling convention code for {other}")),
    })
}

struct Builder<'a> {
    object: &'a Object,
    info: &'a Info,
    records: Vec<u8>,
    count: u32,
    /// Where each type is: None until written.
    index: Vec<Option<u32>>,
    started: Vec<bool>,
    /// A struct's forward reference, where one was needed before it was whole.
    forward: Vec<Option<u32>>,
}

impl Builder<'_> {
    fn add(
        &mut self,
        kind: u16,
        data: &[u8],
    ) -> Result<u32, Unsupported> {
        record(&mut self.records, kind, data, pad)?;
        self.count += 1;
        Ok(FIRST + self.count - 1)
    }

    fn pointer(
        &mut self,
        target: u32,
        bytes: u8,
        mode: u32,
    ) -> Result<u32, Unsupported> {
        let kind = match bytes {
            4 => 0x0A,
            8 => 0x0C,
            other => return refused(format!("a {other}-byte pointer has no C13 kind")),
        };
        let mut data = Vec::new();
        put32(&mut data, target);
        put32(&mut data, kind | mode << 5 | u32::from(bytes) << 13);
        Ok(self.add(LF_POINTER, &data))?
    }

    /// `id`'s index, writing it and what it names first.
    fn of(
        &mut self,
        id: TypeId,
    ) -> Result<u32, Unsupported> {
        if let Some(done) = self.index[id] {
            return Ok(done);
        }
        let one = self
            .info
            .types
            .get(id)
            .ok_or_else(|| Unsupported(format!("CodeView: type {id} is not in the model")))?
            .clone();
        let made = match one {
            Type::Scalar(scalar) | Type::Basic { scalar, .. } => primitive(scalar)?,
            Type::Typedef { target, .. } => self.of(target)?,
            Type::Pointer { target, bytes, reach } => {
                if reach != Reach::Near {
                    return refused("a far or huge pointer is 16-bit code's");
                }
                let target = self.of(target)?;
                self.pointer(target, bytes, 0)?
            }
            Type::Reference(target) => {
                let target = self.of(target)?;
                self.pointer(target, self.object.arch.bits() as u8 / 8, 1)?
            }
            Type::Qualified { target, constant, volatile } => {
                let target = self.of(target)?;
                let mut data = Vec::new();
                put32(&mut data, target);
                put16(&mut data, u16::from(constant) | u16::from(volatile) << 1);
                self.add(LF_MODIFIER, &data)?
            }
            Type::Array { element, bytes } => {
                let Some(bytes) = bytes else { return refused("BASIC's array, whose bounds are its descriptor's") };
                let element = self.of(element)?;
                let mut data = Vec::new();
                put32(&mut data, element);
                put32(&mut data, T_UQUAD);
                numeric(&mut data, i64::from(bytes));
                name(&mut data, "");
                self.add(LF_ARRAY, &data)?
            }
            Type::FixedString(_) => return refused("BASIC's STRING * n"),
            Type::Enum { name: label, underlying, enumerators } => {
                let underlying = self.of(underlying)?;
                let mut list = Vec::new();
                for one in &enumerators {
                    put16(&mut list, LF_ENUMERATE);
                    put16(&mut list, 3);
                    numeric(&mut list, one.value);
                    name(&mut list, &one.name);
                    // Members of a list are padded where they end.
                    while list.len() % 4 != 0 {
                        list.push(pad(4 - list.len() % 4));
                    }
                }
                let list = self.add(LF_FIELDLIST, &list)?;
                let mut data = Vec::new();
                put16(&mut data, enumerators.len() as u16);
                put16(&mut data, 0);
                put32(&mut data, underlying);
                put32(&mut data, list);
                name(&mut data, &label);
                self.add(LF_ENUM, &data)?
            }
            Type::Struct { name: label, bytes, fields, union } => {
                return self.structure(id, &label, bytes, &fields, union);
            }
            Type::Procedure { result, parameters, convention: called } => {
                let result = result.map(|one| self.of(one)).transpose()?.unwrap_or(T_VOID);
                let parameters = parameters.iter().map(|&one| self.of(one)).collect::<Result<Vec<_>, _>>()?;
                let mut list = Vec::new();
                put32(&mut list, parameters.len() as u32);
                parameters.iter().for_each(|&one| put32(&mut list, one));
                let list = self.add(LF_ARGLIST, &list)?;
                let mut data = Vec::new();
                put32(&mut data, result);
                data.push(convention(called.as_deref())?);
                data.push(0);
                put16(&mut data, parameters.len() as u16);
                put32(&mut data, list);
                self.add(LF_PROCEDURE, &data)?
            }
        };
        self.index[id] = Some(made);
        Ok(made)
    }

    /// A struct that is reached again while its fields are being written (a
    /// list's `next`) is its forward reference; the whole struct has the
    /// same name, which is how a reader joins them.
    fn structure(
        &mut self,
        id: TypeId,
        label: &str,
        bytes: u32,
        fields: &[llrm_object::debug::Field],
        union: bool,
    ) -> Result<u32, Unsupported> {
        if self.started[id] {
            if let Some(forward) = self.forward[id] {
                return Ok(forward);
            }
            let mut data = Vec::new();
            put16(&mut data, 0);
            put16(&mut data, FORWARD_REFERENCE);
            put32(&mut data, 0);
            // A struct has its derived class and vshape here; a union has
            // neither.
            if !union {
                put32(&mut data, 0);
                put32(&mut data, 0);
            }
            numeric(&mut data, 0);
            name(&mut data, label);
            let forward = self.add(if union { LF_UNION } else { LF_STRUCTURE }, &data)?;
            self.forward[id] = Some(forward);
            return Ok(forward);
        }
        self.started[id] = true;
        let mut list = Vec::new();
        for field in fields {
            let mut kind = self.of(field.r#type)?;
            if let Some((start, width)) = field.bits {
                let mut data = Vec::new();
                put32(&mut data, kind);
                data.extend([width, start]);
                kind = self.add(LF_BITFIELD, &data)?;
            }
            put16(&mut list, LF_MEMBER);
            put16(&mut list, 3);
            put32(&mut list, kind);
            numeric(&mut list, i64::from(field.offset));
            name(&mut list, &field.name);
            while list.len() % 4 != 0 {
                list.push(pad(4 - list.len() % 4));
            }
        }
        let list = self.add(LF_FIELDLIST, &list)?;
        let mut data = Vec::new();
        put16(&mut data, fields.len() as u16);
        put16(&mut data, 0);
        put32(&mut data, list);
        if !union {
            put32(&mut data, 0);
            put32(&mut data, 0);
        }
        numeric(&mut data, i64::from(bytes));
        name(&mut data, label);
        let whole = self.add(if union { LF_UNION } else { LF_STRUCTURE }, &data)?;
        self.index[id] = Some(whole);
        Ok(whole)
    }
}

pub fn encode(
    object: &Object,
    info: &Info,
) -> Result<Types, Unsupported> {
    let n = info.types.len();
    let mut builder = Builder {
        object,
        info,
        records: Vec::new(),
        count: 0,
        index: vec![None; n],
        started: vec![false; n],
        forward: vec![None; n],
    };
    for id in 0..n {
        builder.of(id)?;
    }
    Ok(Types {
        records: builder.records,
        index: builder.index.into_iter().map(|one| one.expect("every type was written")).collect(),
    })
}
