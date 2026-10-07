//! [`llrm_object::debug::Info`] as Borland's debug information in an OMF object: the `COMENT`
//! records of classes 0xE1-0xE8 and 0xEA that Turbo C++ writes under `-v`, which `TLINK /v` turns
//! into the table Turbo Debugger reads. docs/machine/turbo-debugger.md has the measurements behind
//! every byte. 16-bit only. A fact these records cannot say is refused, with what it was.

use std::collections::BTreeMap;
use std::rc::Rc;

use llrm_object::debug::{self as model, Function, Info, Kind, Location, Scalar, Type};
use llrm_object::{Binding, Definition, Object};

use crate::omf::{self, Record};
use crate::write::Error;

const PUBLIC_TYPE: u8 = 0xE1;
const STRUCT_MEMBERS: u8 = 0xE2;
const TYPE_DEFINITION: u8 = 0xE3;
const BEGIN_SCOPE: u8 = 0xE5;
const LOCALS: u8 = 0xE6;
const END_SCOPE: u8 = 0xE7;
const SOURCE_FILE: u8 = 0xE8;
const COMPILER: u8 = 0xEA;

const TID_NEAR_POINTER: u8 = 0x15;
const TID_FAR_POINTER: u8 = 0x16;
const TID_ARRAY: u8 = 0x1A;
const TID_STRUCT: u8 = 0x1E;
const TID_UNION: u8 = 0x1F;
const TID_FUNCTION: u8 = 0x23;

/// A local's flag: in a frame cell, a parameter; a static; a type's name: a typedef, a tag; a function.
const LOCAL: u8 = 0x02;
const PARAMETER: u8 = 0x0A;
const STATIC: u8 = 0x00;
const TYPEDEF: u8 = 0x06;
const TAG: u8 = 0x07;
const FUNCTION: u8 = 0x18;
/// A local or a parameter in a register, and the register's number after the flag.
const REGISTER_LOCAL: u8 = 0x04;
const REGISTER_PARAMETER: u8 = 0x0C;

/// Where a type's own indices begin: the ones below are the scalars'.
const FIRST_INDEX: u16 = 0x18;

fn refused<T>(what: impl std::fmt::Display) -> Result<T, Error> {
    Err(Error::Unencodable(format!("Turbo Debugger: {what}")))
}

fn comment(class: u8, data: Vec<u8>) -> Rc<Record> {
    Rc::new(Record::new(omf::COMENT, [&[0x00, class][..], &data].concat()))
}

fn pascal(name: &str) -> Result<Vec<u8>, Error> {
    let bytes = name.chars().map(|one| u8::try_from(u32::from(one)).or_else(|_| refused(format!("{name:?} is not latin-1")))).collect::<Result<Vec<u8>, Error>>()?;
    let length = u8::try_from(bytes.len()).or_else(|_| refused(format!("{name:?} is longer than 255 bytes")))?;
    Ok([vec![length], bytes].concat())
}

/// A type index: one byte below 0x80, else the high bits behind 0x80 and the low byte.
fn index(number: u16) -> Result<Vec<u8>, Error> {
    match number {
        0..=0x7F => Ok(vec![number as u8]),
        0x80..=0x7FFF => Ok(vec![0x80 | (number >> 8) as u8, (number & 0xFF) as u8]),
        _ => refused(format!("type index {number} does not fit")),
    }
}

/// Borland's number of a 16-bit or 8-bit x86 register (its encoding: AX 0, CX 1, DX 2, BX 3, SP 4, BP 5,
/// SI 6, DI 7; AL 0 .. BH 7); the records are x86's alone, so the names are too.
fn register_number(variable: &str, register: &str) -> Result<u8, Error> {
    const WORDS: [&str; 8] = ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di"];
    const BYTES: [&str; 8] = ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"];
    match WORDS.iter().chain(&BYTES).position(|one| *one == register) {
        Some(at) => Ok((at % 8) as u8),
        None => refused(format!("{variable} is in register {register}, which has no Borland number")),
    }
}

fn scalar(one: Scalar) -> Result<u16, Error> {
    Ok(match one {
        Scalar::Void => 0x01,
        Scalar::Char | Scalar::Int { bytes: 1, signed: true } => 0x02,
        Scalar::Int { bytes: 1, signed: false } => 0x08,
        Scalar::Int { bytes: 2, signed: true } => 0x04,
        Scalar::Int { bytes: 2, signed: false } => 0x0A,
        Scalar::Int { bytes: 4, signed: true } => 0x06,
        Scalar::Int { bytes: 4, signed: false } => 0x0C,
        Scalar::Float { bytes: 4 } => 0x0E,
        Scalar::Float { bytes: 8 } => 0x0F,
        Scalar::Float { bytes: 10 } => 0x10,
        other => return refused(format!("no type for {other:?}")),
    })
}

/// The type table: each composite type's record, its index given before its parts' so a cycle closes.
struct Types<'a> {
    info: &'a Info,
    of: Vec<Option<u16>>,
    records: Vec<Rc<Record>>,
    next: u16,
}

impl Types<'_> {
    /// `id`'s index, its records made if it has none yet.
    fn code(&mut self, id: usize) -> Result<u16, Error> {
        if let Some(Some(done)) = self.of.get(id) {
            return Ok(*done);
        }
        let info = self.info;
        match &info.types[id] {
            Type::Scalar(one) => scalar(*one),
            // Const and volatile are not in the records; a typedef is a name, set by `names`.
            Type::Qualified { target, .. } | Type::Typedef { target, .. } => self.code(*target),
            one => {
                let at = self.next;
                self.next += 1;
                self.of[id] = Some(at);
                let (name, size, tid, tail) = self.composite(one)?;
                let mut data = [index(at)?, pascal(name)?, size.to_le_bytes().to_vec(), vec![tid]].concat();
                data.extend(tail);
                self.records.push(comment(TYPE_DEFINITION, data));
                Ok(at)
            }
        }
    }

    /// A composite type's name, size, kind and what follows its kind; its members' records first.
    fn composite<'t>(&mut self, one: &'t Type) -> Result<(&'t str, u16, u8, Vec<u8>), Error> {
        let size = |bytes: u32| u16::try_from(bytes).or_else(|_| refused(format!("a type of {bytes} bytes does not fit its size")));
        Ok(match one {
            Type::Pointer { target, bytes, reach } => {
                let to = self.code(*target)?;
                let function = matches!(self.info.types[*target], Type::Procedure { .. });
                let (tid, kind) = match reach {
                    model::Reach::Near => (TID_NEAR_POINTER, if function { 0x02 } else { 0x04 }),
                    model::Reach::Far => (TID_FAR_POINTER, 0x00),
                    model::Reach::Huge => (TID_FAR_POINTER, 0x01),
                };
                ("", u16::from(*bytes), tid, [index(to)?, vec![kind]].concat())
            }
            Type::Array { element, bytes: Some(bytes) } => {
                let to = self.code(*element)?;
                ("", size(*bytes)?, TID_ARRAY, index(to)?)
            }
            Type::Array { bytes: None, .. } => return refused("a BASIC array is its descriptor's"),
            Type::Struct { name, bytes, fields, union } => {
                let mut members = Vec::new();
                let mut at = 0u32;
                for field in fields {
                    if field.bits.is_some() {
                        return refused(format!("bit field {} is not written yet", field.name));
                    }
                    if field.offset != at {
                        members.extend([0x40]);
                        members.extend(field.offset.to_le_bytes());
                    }
                    let to = self.code(field.r#type)?;
                    members.extend([0x00]);
                    members.extend(pascal(&field.name)?);
                    members.extend(index(to)?);
                    let width = self.info.size_of(field.r#type).ok_or_else(|| Error::Unencodable(format!("Turbo Debugger: field {} has no size", field.name)))?;
                    at = if *union { 0 } else { field.offset + u32::try_from(width).unwrap_or(0) };
                }
                members.push(0xC0);
                members.extend(bytes.to_le_bytes());
                self.records.push(comment(STRUCT_MEMBERS, members));
                (name.as_str(), size(*bytes)?, if *union { TID_UNION } else { TID_STRUCT }, Vec::new())
            }
            Type::Procedure { result, .. } => {
                let ret = match result {
                    Some(result) => self.code(*result)?,
                    None => 0x01,
                };
                // The call's own flags are the function's: `call_flags` sets them where a name is.
                ("", 0, TID_FUNCTION, [index(ret)?, vec![0x04, 0x00]].concat())
            }
            Type::Enum { name, .. } => return refused(format!("enum {name} is not written yet")),
            Type::Reference(_) => return refused("a BASIC reference has no type here"),
            Type::FixedString(_) => return refused("a BASIC STRING * n has no type here"),
            Type::Scalar(_) | Type::Qualified { .. } | Type::Typedef { .. } => unreachable!("handled by `code`"),
        })
    }
}

/// What the writer places among the object's own records.
pub struct Debug {
    /// After THEADR: types, scopes and what they hold, compiler parameters.
    pub before: Vec<Rc<Record>>,
    /// After each public symbol's PUBDEF, by its index in `Object::symbols`.
    pub publics: BTreeMap<usize, Vec<Rc<Record>>>,
    /// After the PUBDEFs: statics and type names of the module.
    pub module: Vec<Rc<Record>>,
    /// Before the LINNUMs.
    pub source: Rc<Record>,
}

fn frame_offset(name: &str, disp: i64) -> Result<[u8; 2], Error> {
    let bp = i16::try_from(disp).or_else(|_| refused(format!("{name} is at frame offset {disp}, which does not fit")))?;
    Ok(bp.to_le_bytes())
}

struct Builder<'a> {
    object: &'a Object,
    info: &'a Info,
    types: Types<'a>,
}

impl Builder<'_> {
    /// One local's entry in an E6 record.
    fn entry(&mut self, variable: &model::Variable) -> Result<Vec<u8>, Error> {
        let to = self.types.code(variable.r#type)?;
        let mut data = [pascal(&variable.name)?, index(to)?].concat();
        match &variable.location {
            Location::Frame { disp } => {
                data.push(if variable.kind == Kind::Parameter { PARAMETER } else { LOCAL });
                data.extend(frame_offset(&variable.name, *disp)?);
            }
            Location::Static { symbol, disp } => return self.placed(&variable.name, to, false, *symbol, *disp),
            // A register a parameter arrives in and is there until the body starts: an entry of the body's scope,
            // a register parameter's flag 0xC and the register's number.
            Location::List(entries) if matches!(&entries[..], [(_, Location::Register(_))]) => {
                let Location::Register(register) = &entries[0].1 else { unreachable!("matched") };
                data.push(if variable.kind == Kind::Parameter { REGISTER_PARAMETER } else { REGISTER_LOCAL });
                data.push(register_number(&variable.name, register)?);
            }
            // The optimiser removed it: Turbo Debugger's records have no "optimized out", so it is left out.
            Location::List(entries) if entries.is_empty() => return Ok(Vec::new()),
            Location::Register(register) => return refused(format!("{} is in register {register}, which is not written yet", variable.name)),
            Location::List(_) => return refused(format!("{} has a location list, which is not written yet", variable.name)),
        }
        Ok(data)
    }

    fn locals(&mut self, variables: &[&model::Variable]) -> Result<Option<Rc<Record>>, Error> {
        let mut data = Vec::new();
        for variable in variables {
            data.extend(self.entry(variable)?);
        }
        Ok((!data.is_empty()).then(|| comment(LOCALS, data)))
    }

    fn scope(&self, section: usize, offset: usize) -> Result<Rc<Record>, Error> {
        let at = u16::try_from(offset).or_else(|_| refused(format!("code at offset {offset} does not fit")))?;
        let segment = u8::try_from(section + 1).or_else(|_| refused("a segment number past 255"))?;
        Ok(comment(BEGIN_SCOPE, [vec![segment], at.to_le_bytes().to_vec()].concat()))
    }

    fn end(&self, offset: usize) -> Result<Rc<Record>, Error> {
        Ok(comment(END_SCOPE, u16::try_from(offset).or_else(|_| refused(format!("code at offset {offset} does not fit")))?.to_le_bytes().to_vec()))
    }

    fn block(&mut self, out: &mut Vec<Rc<Record>>, block: &model::Block) -> Result<(), Error> {
        let [range] = block.ranges[..] else { return refused("a block of several ranges is not written yet") };
        out.push(self.scope(range.section, range.offset)?);
        let variables: Vec<&model::Variable> = block.variables.iter().collect();
        out.extend(self.locals(&variables)?);
        for inner in &block.blocks {
            self.block(out, inner)?;
        }
        out.push(self.end(range.offset + range.length)?);
        Ok(())
    }

    /// A function's scopes: its own, with the parameters as passed in, and its body's, with the
    /// locals.
    fn function(&mut self, out: &mut Vec<Rc<Record>>, function: &Function) -> Result<(), Error> {
        let [range] = function.ranges[..] else { return refused(format!("{} has {} ranges: one is written", function.name, function.ranges.len())) };
        out.push(self.scope(range.section, range.offset)?);
        let parameters: Vec<&model::Variable> = function.variables.iter().filter(|one| one.kind == Kind::Parameter).collect();
        // The ones a frame cell holds are the function's own; one a register holds is the body's.
        let reversed: Vec<&model::Variable> = parameters.iter().rev().copied().filter(|one| !matches!(one.location, Location::List(_))).collect();
        out.extend(self.locals(&reversed)?);
        let (start, _) = function.body.unwrap_or((0, range.length));
        out.push(self.scope(range.section, range.offset + start)?);
        for block in &function.blocks {
            self.block(out, block)?;
        }
        let locals: Vec<&model::Variable> = function.variables.iter().filter(|one| one.kind != Kind::Parameter).chain(parameters.iter().copied()).collect();
        out.extend(self.locals(&locals)?);
        out.push(self.end(range.offset + range.length)?);
        out.push(self.end(range.offset + range.length)?);
        Ok(())
    }

    /// The function's type and its record, a call of it far or near, C's or Pascal's.
    fn function_type(&mut self, function: &Function) -> Result<(u16, Rc<Record>), Error> {
        let Type::Procedure { convention, .. } = &self.info.types[function.r#type] else { return refused(format!("{}'s type is no procedure", function.name)) };
        let call = match convention.as_deref() {
            None | Some("cdecl") => 0x00,
            Some("pascal") => 0x01,
            Some(other) => return refused(format!("{}'s calling convention {other} is not written", function.name)),
        };
        let at = self.types.next;
        self.types.next += 1;
        let Type::Procedure { result, .. } = &self.info.types[function.r#type] else { unreachable!("checked") };
        let ret = match result {
            Some(result) => self.types.code(*result)?,
            None => 0x01,
        };
        let far = if function.far { 0x04 } else { 0x00 };
        let data = [index(at)?, pascal("")?, 0u16.to_le_bytes().to_vec(), vec![TID_FUNCTION], index(ret)?, vec![far | call, 0x00]].concat();
        Ok((at, comment(TYPE_DEFINITION, data)))
    }

    /// A module-level static: a function (flag 0x18, kind 0) or data (flag 0, kind 1) at its place.
    fn placed(&self, name: &str, to: u16, function: bool, symbol: usize, disp: i64) -> Result<Vec<u8>, Error> {
        let Definition::Defined { section, offset } = self.object.symbols[symbol].definition else { return refused(format!("{name} is in a symbol this object does not define")) };
        let segment = u8::try_from(section + 1).or_else(|_| refused("a segment number past 255"))?;
        let at = u16::try_from(offset as i64 + disp).or_else(|_| refused(format!("{name} is past a 16-bit offset")))?;
        Ok([pascal(name)?, index(to)?, vec![if function { FUNCTION } else { STATIC }, u8::from(!function), segment], at.to_le_bytes().to_vec()].concat())
    }
}

pub fn records(object: &Object, info: &Info) -> Result<Debug, Error> {
    if object.arch.bits() != 16 {
        return refused("its records are 16-bit: Borland's 32-bit information is another format");
    }
    if info.language != model::Language::C {
        return refused(format!("only C's information is written, not {:?}'s", info.language));
    }
    let mut builder = Builder { object, info, types: Types { info, of: vec![None; info.types.len()], records: Vec::new(), next: FIRST_INDEX } };
    // Each function's records, in the order of its code.
    let mut functions: Vec<&Function> = info.functions.iter().collect();
    functions.sort_by_key(|one| one.ranges.first().map(|range| (range.section, range.offset)));
    let mut scopes = Vec::new();
    for function in &functions {
        builder.function(&mut scopes, function)?;
    }
    let far = info.functions.iter().any(|one| one.far);
    let mut publics: BTreeMap<usize, Vec<Rc<Record>>> = BTreeMap::new();
    let mut module = Vec::new();
    let mut statics = Vec::new();
    let section_of = |symbol: usize| match object.symbols[symbol].definition {
        Definition::Defined { section, offset } => Some((section, offset)),
        Definition::Undefined => None,
    };
    for function in &info.functions {
        let (at, record) = builder.function_type(function)?;
        if object.symbols[function.symbol].binding == Binding::Public {
            let list = publics.entry(function.symbol).or_default();
            list.push(record);
            list.push(comment(PUBLIC_TYPE, [index(at)?, vec![FUNCTION]].concat()));
        } else if section_of(function.symbol).is_some() {
            module.push(record);
            statics.extend(builder.placed(&function.name, at, true, function.symbol, 0)?);
        }
    }
    for global in &info.globals {
        let Location::Static { symbol, disp } = &global.location else { return refused(format!("{} is no static", global.name)) };
        let to = builder.types.code(global.r#type)?;
        if object.symbols[*symbol].binding == Binding::Public && *disp == 0 {
            publics.entry(*symbol).or_default().push(comment(PUBLIC_TYPE, [index(to)?, vec![0x00]].concat()));
        } else {
            statics.extend(builder.placed(&global.name, to, false, *symbol, *disp)?);
        }
    }
    if !statics.is_empty() {
        module.push(comment(LOCALS, statics));
    }
    // Names of types: typedefs and tags, as a local scope of the module lists them.
    let mut names = Vec::new();
    for id in 0..info.types.len() {
        let (name, kind, target) = match &info.types[id] {
            Type::Struct { name, .. } if !name.is_empty() => (name.as_str(), TAG, id),
            Type::Typedef { name, target } => (name.as_str(), TYPEDEF, *target),
            _ => continue,
        };
        // A name of a type this unit cannot write is not written: nothing uses it.
        let Ok(to) = builder.types.code(target) else { continue };
        names.extend([pascal(name)?, index(to)?, vec![kind]].concat());
    }
    if !names.is_empty() {
        module.push(comment(LOCALS, names));
    }
    let mut before = std::mem::take(&mut builder.types.records);
    before.extend(scopes);
    // C, and the memory model: 8 is tiny, 9 small, 10 medium: far code or not.
    before.push(comment(COMPILER, vec![0x01, if far { 0x0A } else { 0x09 }]));
    let name = info.files.first().map_or("", |one| one.name.as_str());
    let source = comment(SOURCE_FILE, [vec![0x00], pascal(name)?, 0u32.to_le_bytes().to_vec()].concat());
    Ok(Debug { before, publics, module, source })
}

#[cfg(test)]
mod tests {
    use llrm_object::debug::{Block, Field, File, Format, Function, Language, Range, Variable};
    use llrm_object::{Arch, Section, Symbol};

    use super::*;
    use crate::write;

    fn int() -> Type {
        Type::Scalar(Scalar::Int { bytes: 2, signed: true })
    }

    fn variable(name: &str, r#type: usize, kind: Kind, location: Location) -> Variable {
        Variable { name: name.into(), r#type, kind, location }
    }

    /// `int f(int a) { int l; }` at 0..0x20, body at 6..0x1c, `_f` public; a global `_g` in data.
    fn object(types: Vec<Type>, variables: Vec<Variable>, info: impl FnOnce(&mut Info)) -> Object {
        let text = Section { name: "T_TEXT".into(), role: llrm_object::Role::Text, near: true, align: 1, image: vec![0x90; 0x20], spans: vec![[0, 0x20]], relocs: Vec::new() };
        let data = Section { name: "_DATA".into(), role: llrm_object::Role::Data, near: true, align: 2, image: vec![0; 8], spans: vec![[0, 8]], relocs: Vec::new() };
        let symbol = |name: &str, section, offset, binding| Symbol { name: name.into(), binding, definition: Definition::Defined { section, offset }, group: None };
        let procedure = types.len();
        let mut types = types;
        types.push(Type::Procedure { result: Some(0), parameters: vec![0], convention: None });
        let function = Function {
            name: "f".into(),
            symbol: 0,
            r#type: procedure,
            ranges: vec![Range { section: 0, offset: 0, length: 0x20 }],
            body: Some((6, 0x1c)),
            far: false,
            module: false,
            variables,
            blocks: Vec::new(),
            frame: Vec::new(),
        };
        let mut made = Info {
            format: Format::TurboDebugger,
            language: Language::C,
            code: vec![Range { section: 0, offset: 0, length: 0x20 }],
            files: vec![File { name: "t.c".into(), checksum: None }],
            types,
            functions: vec![function],
            ..Info::default()
        };
        info(&mut made);
        Object { name: "t.c".into(), arch: Arch::I8086, sections: vec![text, data], symbols: vec![symbol("_f", 0, 0, Binding::Public), symbol("_g", 1, 2, Binding::Public)], omf_groups: Vec::new(), debug: Some(made) }
    }

    /// The (class, data) of each COMENT of an object, in order.
    fn comments(object: &Object) -> Vec<(u8, Vec<u8>)> {
        omf::parse(&write::write(object).expect("writes")).expect("parses").iter().filter(|one| one.r#type == omf::COMENT).map(|one| (one.body[1], one.body[2..].to_vec())).collect()
    }

    fn all(object: &Object, class: u8) -> Vec<Vec<u8>> {
        comments(object).into_iter().filter(|(one, _)| *one == class).map(|(_, data)| data).collect()
    }

    /// A function's records as Turbo C++ writes them (measured on its `-v` objects): its scope with the
    /// parameters, last first, its body's with the locals and then the parameters, and both end where the
    /// code does; a local is its type, 0x02 and its BP, a parameter's flag is 0x0A.
    #[test]
    fn a_function_is_two_scopes_with_its_parameters_and_locals() {
        let made = object(vec![int()], vec![variable("a", 0, Kind::Parameter, Location::Frame { disp: 6 }), variable("l", 0, Kind::Local, Location::Frame { disp: -2 })], |_| {});
        let scopes: Vec<(u8, Vec<u8>)> = comments(&made).into_iter().filter(|(class, _)| matches!(*class, BEGIN_SCOPE | LOCALS | END_SCOPE)).collect();
        assert_eq!(
            scopes,
            [
                (BEGIN_SCOPE, vec![1, 0x00, 0x00]),
                (LOCALS, vec![1, b'a', 0x04, 0x0A, 0x06, 0x00]),
                (BEGIN_SCOPE, vec![1, 0x06, 0x00]),
                (LOCALS, vec![1, b'l', 0x04, 0x02, 0xFE, 0xFF, 1, b'a', 0x04, 0x0A, 0x06, 0x00]),
                (END_SCOPE, vec![0x20, 0x00]),
                (END_SCOPE, vec![0x20, 0x00]),
            ]
        );
    }

    /// A public function's type record follows its PUBDEF with the call's flags (0x04 far, 0x00 near) and
    /// the return type, and a record of its own type, 0x18 for a function; a public variable's is its
    /// type and 0.
    #[test]
    fn a_public_symbol_is_followed_by_its_type() {
        let global = variable("g", 0, Kind::Local, Location::Static { symbol: 1, disp: 0 });
        let made = object(vec![int()], Vec::new(), |info| info.globals.push(global));
        let records = omf::parse(&write::write(&made).unwrap()).unwrap();
        let after = |name: &[u8]| {
            let at = records.iter().position(|one| one.r#type == omf::PUBDEF && one.body.windows(name.len()).any(|window| window == name)).expect("a PUBDEF");
            records[at + 1..].iter().take_while(|one| one.r#type == omf::COMENT).map(|one| (one.body[1], one.body[2..].to_vec())).collect::<Vec<_>>()
        };
        assert_eq!(&after(b"_f")[..2], [(TYPE_DEFINITION, vec![0x18, 0x00, 0x00, 0x00, TID_FUNCTION, 0x04, 0x00, 0x00]), (PUBLIC_TYPE, vec![0x18, FUNCTION])]);
        assert_eq!(&after(b"_g")[..1], [(PUBLIC_TYPE, vec![0x04, 0x00])]);
        // A far function's call flag is 0x04.
        let mut far = made.clone();
        far.debug.as_mut().unwrap().functions[0].far = true;
        assert_eq!(&after_of(&far, b"_f")[0].1[6..8], [0x04, 0x00]);
    }

    fn after_of(object: &Object, name: &[u8]) -> Vec<(u8, Vec<u8>)> {
        let records = omf::parse(&write::write(object).unwrap()).unwrap();
        let at = records.iter().position(|one| one.r#type == omf::PUBDEF && one.body.windows(name.len()).any(|window| window == name)).expect("a PUBDEF");
        records[at + 1..].iter().take_while(|one| one.r#type == omf::COMENT).map(|one| (one.body[1], one.body[2..].to_vec())).collect()
    }

    /// A struct's members, each with its bit width (0 for none), name and type; a member not where the
    /// last ended is preceded by 0x40 and its offset, and the record ends 0xC0 and the size. The struct
    /// has index 0x18 before its members' own (a pointer to it, 0x19, names it before its record).
    #[test]
    fn a_struct_has_its_members_and_marks_the_padding() {
        let field = |name: &str, r#type, offset| Field { name: name.into(), r#type, offset, bits: None };
        let types = vec![
            int(),
            Type::Scalar(Scalar::Int { bytes: 1, signed: true }),
            Type::Struct { name: "s".into(), bytes: 6, fields: vec![field("c", 1, 0), field("i", 0, 2), field("p", 3, 4)], union: false },
            Type::Pointer { target: 2, bytes: 2, reach: model::Reach::Near },
        ];
        let made = object(types, vec![variable("v", 2, Kind::Local, Location::Frame { disp: -6 })], |_| {});
        let members = all(&made, STRUCT_MEMBERS);
        // c, then i past a gap of one byte, then p, a pointer to the struct.
        assert_eq!(members, [vec![0x00, 1, b'c', 0x02, 0x40, 0x02, 0, 0, 0, 0x00, 1, b'i', 0x04, 0x00, 1, b'p', 0x19, 0xC0, 0x06, 0, 0, 0]]);
        let types = all(&made, TYPE_DEFINITION);
        assert!(types.contains(&vec![0x18, 1, b's', 0x06, 0x00, TID_STRUCT]), "{types:?}");
        assert!(types.contains(&vec![0x19, 0x00, 0x02, 0x00, TID_NEAR_POINTER, 0x18, 0x04]), "{types:?}");
    }

    /// A type's index is one byte below 0x80 and past it 0x80 | its high bits and its low byte (Turbo C++
    /// numbers 150 structs up to 0x80 0xAD).
    #[test]
    fn an_index_past_0x7f_takes_two_bytes() {
        assert_eq!(index(0x7F).unwrap(), [0x7F]);
        assert_eq!(index(0x80).unwrap(), [0x80, 0x80]);
        assert_eq!(index(0xAD).unwrap(), [0x80, 0xAD]);
        assert_eq!(index(0x1A7).unwrap(), [0x81, 0xA7]);
        assert!(index(0x8000).is_err());
    }

    /// What these records cannot say is refused by name: a register location, a bit field, an enum, a
    /// 32-bit object, a language but C's.
    #[test]
    fn what_turbo_debugger_cannot_say_is_refused_by_name() {
        let refused = |made: Object| write::write(&made).unwrap_err().to_string();
        let register = object(vec![int()], vec![variable("r", 0, Kind::Local, Location::Register("si".into()))], |_| {});
        assert!(refused(register).contains("r is in register si"));
        let bits = Type::Struct { name: "s".into(), bytes: 2, fields: vec![Field { name: "b".into(), r#type: 0, offset: 0, bits: Some((0, 3)) }], union: false };
        assert!(refused(object(vec![int(), bits], vec![variable("v", 1, Kind::Local, Location::Frame { disp: -2 })], |_| {})).contains("bit field b"));
        let enumeration = Type::Enum { name: "e".into(), underlying: 0, enumerators: Vec::new() };
        assert!(refused(object(vec![int(), enumeration], vec![variable("v", 1, Kind::Local, Location::Frame { disp: -2 })], |_| {})).contains("enum e"));
        let mut wide = object(vec![int()], Vec::new(), |_| {});
        wide.arch = Arch::I386;
        assert!(refused(wide).contains("16-bit"));
        assert!(refused(object(vec![int()], Vec::new(), |info| info.language = Language::Basic)).contains("only C's"));
    }

    /// A block is a scope of its own inside the body's, ending where it does.
    #[test]
    fn a_block_is_a_scope_inside_the_body() {
        let made = object(vec![int()], Vec::new(), |info| {
            info.functions[0].blocks.push(Block { ranges: vec![Range { section: 0, offset: 8, length: 8 }], variables: vec![variable("y", 0, Kind::Local, Location::Frame { disp: -4 })], blocks: Vec::new() });
        });
        let scopes: Vec<(u8, Vec<u8>)> = comments(&made).into_iter().filter(|(class, _)| matches!(*class, BEGIN_SCOPE | LOCALS | END_SCOPE)).collect();
        assert_eq!(scopes[2..5], [(BEGIN_SCOPE, vec![1, 0x08, 0x00]), (LOCALS, vec![1, b'y', 0x04, 0x02, 0xFC, 0xFF]), (END_SCOPE, vec![0x10, 0x00])]);
    }

    /// A parameter that arrives in a register is an entry of the body's scope alone (Turbo C++ writes a register
    /// parameter so, flag 0xC and the register's number: SI is 6, BL is 3), and one the optimiser removed has no
    /// entry, which these records have no "optimized out" to say.
    #[test]
    fn a_register_parameter_is_the_bodys_entry_and_a_removed_one_is_left_out() {
        let entry = Range { section: 0, offset: 0, length: 7 };
        let in_register = |name: &str, register: &str| variable(name, 0, Kind::Parameter, Location::List(vec![(entry, Location::Register(register.into()))]));
        let made = object(vec![int()], vec![in_register("p", "si"), in_register("c", "bl"), variable("g", 0, Kind::Parameter, Location::List(Vec::new())), variable("a", 0, Kind::Parameter, Location::Frame { disp: 6 })], |_| {});
        let scopes: Vec<(u8, Vec<u8>)> = comments(&made).into_iter().filter(|(class, _)| matches!(*class, BEGIN_SCOPE | LOCALS | END_SCOPE)).collect();
        // The function's scope holds the parameter a cell holds; the body's, the registers' and that one again.
        assert_eq!(scopes[1], (LOCALS, vec![1, b'a', 0x04, 0x0A, 0x06, 0x00]));
        assert_eq!(scopes[3], (LOCALS, vec![1, b'p', 0x04, 0x0C, 0x06, 1, b'c', 0x04, 0x0C, 0x03, 1, b'a', 0x04, 0x0A, 0x06, 0x00]));
        // A register Turbo Debugger has no number for is refused by name.
        let wide = object(vec![int()], vec![in_register("w", "eax")], |_| {});
        assert!(write::write(&wide).unwrap_err().to_string().contains("w is in register eax"));
    }
}
