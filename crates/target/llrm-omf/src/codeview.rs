//! [`llrm_object::debug::Info`] as CodeView 4: the $$SYMBOLS and $$TYPES segments, and the line
//! numbers each section's LINNUM records carry. A fact CodeView 4 as `cvwrite` writes it cannot
//! say is refused with what it was.

use llrm_object::debug::{self as model, Info, Location};
use llrm_object::{Definition, Kind as Fixup, Object, Reloc, Role, Section, Target};

use crate::cvwrite::{self, Flavor, Reach, Scalar, Type};
use crate::write::Error;

pub const SYMBOLS: &str = "$$SYMBOLS";
pub const TYPES: &str = "$$TYPES";

fn refused<T>(what: impl std::fmt::Display) -> Result<T, Error> {
    Err(Error::Unencodable(format!("CodeView: {what}")))
}

fn narrow<T: TryFrom<i64>>(value: i64, what: &str) -> Result<T, Error> {
    T::try_from(value).or_else(|_| refused(format!("{what} {value} does not fit its field")))
}

fn scalar(one: model::Scalar) -> Result<Scalar, Error> {
    use model::Scalar as M;
    Ok(match one {
        M::Void => Scalar::Void,
        M::Char => Scalar::Char,
        M::Int { bytes: 1, signed: true } => Scalar::Int8,
        M::Int { bytes: 1, signed: false } => Scalar::UInt8,
        M::Int { bytes: 2, signed: true } => Scalar::Int16,
        M::Int { bytes: 2, signed: false } => Scalar::UInt16,
        M::Int { bytes: 4, signed: true } => Scalar::Int32,
        M::Int { bytes: 4, signed: false } => Scalar::UInt32,
        M::Float { bytes: 4 } => Scalar::Float32,
        M::Float { bytes: 8 } => Scalar::Float64,
        M::Float { bytes: 10 } => Scalar::Float80,
        M::Currency => Scalar::Currency,
        M::BasicString { far } => Scalar::String { far },
        other => return refused(format!("no type for {other:?}")),
    })
}

fn typed(one: &model::Type) -> Result<Type, Error> {
    use model::Type as M;
    Ok(match one {
        M::Scalar(one) => Type::Scalar(scalar(*one)?),
        M::FixedString(length) => Type::FixedString(narrow(i64::from(*length), "a STRING's length")?),
        M::Array { element, bytes: None } => Type::Array(*element),
        M::Array { element, bytes: Some(bytes) } => Type::Sized { element: *element, bytes: *bytes },
        M::Struct { name, bytes, fields } => {
            let fields = fields
                .iter()
                .map(|field| Ok(cvwrite::Field { name: field.name.clone(), r#type: field.r#type, offset: narrow(i64::from(field.offset), "a field's offset")?, bits: field.bits }))
                .collect::<Result<Vec<_>, Error>>()?;
            Type::Struct { name: name.clone(), bytes: *bytes, fields }
        }
        M::Pointer { target, reach, .. } => Type::Pointer {
            target: *target,
            reach: match reach {
                model::Reach::Near => Reach::Near,
                model::Reach::Far => Reach::Far,
                model::Reach::Huge => Reach::Huge,
            },
        },
        M::Reference(target) => Type::Reference(*target),
        M::Procedure { result, parameters, .. } => Type::Procedure { result: *result, parameters: parameters.clone() },
        M::Enum { name, .. } => return refused(format!("enum {name} is not written yet")),
        M::Typedef { name, .. } => return refused(format!("typedef {name} is not written yet")),
        M::Qualified { .. } => return refused("const and volatile are not written yet"),
    })
}

fn data(object: &Object, variable: &model::Variable) -> Result<Option<cvwrite::Data>, Error> {
    let Location::Static { symbol, disp } = &variable.location else { return Ok(None) };
    Ok(Some(cvwrite::Data { name: variable.name.clone(), r#type: variable.r#type, symbol: object.symbols[*symbol].name.clone(), displacement: narrow(*disp, "a variable's offset")? }))
}

fn module(object: &Object, info: &Info) -> Result<cvwrite::Module, Error> {
    // QB 4.5's module record is nameless.
    let name = (info.producer != model::Producer::Qb45).then(|| object.name.clone());
    let types = info.types.iter().map(typed).collect::<Result<Vec<_>, Error>>()?;
    let mut written = cvwrite::Module { name, types, ..cvwrite::Module::default() };
    for global in &info.globals {
        written.data.extend(data(object, global)?);
    }
    if let Some(first) = info.code.first() {
        // None: no code, so no procedure to start at.
        let start = object.symbols.iter().find(|one| one.definition == Definition::Defined { section: first.section, offset: first.offset });
        written.start = start.map(|one| one.name.clone()).unwrap_or_default();
        written.length = narrow(first.length as i64, "the module's code length")?;
    }
    for function in &info.functions {
        let mut statics = Vec::new();
        let mut locals = Vec::new();
        for variable in &function.variables {
            match &variable.location {
                Location::Frame { disp } => locals.push(cvwrite::Local { name: variable.name.clone(), r#type: variable.r#type, bp: narrow(*disp, "a frame offset")? }),
                Location::Static { .. } => statics.extend(data(object, variable)?),
                Location::Register(register) => return refused(format!("{} is in register {register}, which is not written yet", variable.name)),
                // A parameter that arrives in a register and is there until the body starts, or one the optimiser
                // removed: CodeView 4 as written has no register symbol (S_REGISTER) and no "optimized out", so it is
                // left out, as it was before the model said it.
                Location::List(entries) if entries.iter().all(|(_, location)| matches!(location, Location::Register(_))) => {}
                Location::List(_) => return refused(format!("{} has a location list, which is not written yet", variable.name)),
            }
        }
        if !function.blocks.is_empty() {
            return refused(format!("{} has block scopes, which are not written yet", function.name));
        }
        if function.module {
            written.data.extend(statics);
            continue;
        }
        let [range, ..] = function.ranges[..] else { return refused(format!("{} has no code", function.name)) };
        let (start, end) = function.body.unwrap_or((0, range.length));
        written.procedures.push(cvwrite::Procedure {
            name: function.name.clone(),
            symbol: object.symbols[function.symbol].name.clone(),
            r#type: function.r#type,
            length: narrow(range.length as i64, "a procedure's length")?,
            debug_start: narrow(start as i64, "a body's start")?,
            debug_end: narrow(end as i64, "a body's end")?,
            far: function.far,
            locals,
            statics,
        });
    }
    Ok(written)
}

/// `info`'s two debug sections for `object`: $$SYMBOLS, then $$TYPES.
pub fn sections(object: &Object, info: &Info) -> Result<[Section; 2], Error> {
    match info.format {
        model::Format::Default | model::Format::CodeView => {}
        model::Format::Dwarf { .. } => return refused("OMF cannot carry DWARF: use -gcodeview, or -fobject-format=elf"),
        model::Format::TurboDebugger => return refused("this writer does not write Turbo Debugger's information yet"),
    }
    let flavor = Flavor { qb45: info.producer == model::Producer::Qb45 };
    let encoded = cvwrite::written(&module(object, info)?, flavor).map_err(Error::Unencodable)?;
    let mut relocs = Vec::new();
    let mut image = encoded.symbols;
    for one in &encoded.relocations {
        let symbol = object.symbols.iter().position(|symbol| symbol.name == one.symbol).ok_or_else(|| Error::Unencodable(format!("CodeView: {} is no symbol of the object", one.symbol)))?;
        // The image holds zeros where LINK fills in, the displacement is the addend.
        image[one.at..one.at + 2].fill(0);
        relocs.push(Reloc { at: one.at, kind: if one.far { Fixup::FarPointer } else { Fixup::Abs { width: 2 } }, target: Target::Symbol(symbol), addend: i64::from(one.displacement as i16) });
    }
    let section = |name: &str, image: Vec<u8>, relocs: Vec<Reloc>| {
        let spans = if image.is_empty() { Vec::new() } else { vec![[0, image.len()]] };
        Section { name: name.to_owned(), role: Role::Debug, near: true, align: 1, image, spans, relocs }
    };
    Ok([section(SYMBOLS, image, relocs), section(TYPES, encoded.types, Vec::new())])
}

/// Each section's (line, offset) pairs. CodeView 4 names one file per module.
pub fn lines(object: &Object, info: &Info) -> Result<Vec<Vec<(u32, usize)>>, Error> {
    let mut out = vec![Vec::new(); object.sections.len()];
    for one in &info.lines {
        if one.file != 0 {
            return refused(format!("line {} is in file {}: one file per module", one.line, one.file));
        }
        out[one.section].push((one.line, one.offset));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use llrm_object::debug::{Function, Line, Range, Scalar as S, Type as T, Variable};
    use llrm_object::{Arch, Binding, Definition, Symbol};

    use super::*;
    use crate::{cvinfo, omf, write};

    fn int() -> T {
        T::Scalar(S::Int { bytes: 2, signed: true })
    }

    /// `_f(x) { y }` in eight bytes of code, `x` and `y` in the frame, on lines 3 and 4.
    fn object(variables: Vec<Variable>, types: Vec<T>) -> Object {
        let text = Section { name: "_TEXT".into(), role: Role::Text, near: true, align: 1, image: vec![0x90; 8], spans: vec![[0, 8]], relocs: Vec::new() };
        let function = Function {
            name: "f".into(),
            symbol: 0,
            r#type: 1,
            ranges: vec![Range { section: 0, offset: 0, length: 8 }],
            body: Some((1, 7)),
            far: false,
            module: false,
            variables,
            blocks: Vec::new(),
        };
        let info = Info {
            code: vec![Range { section: 0, offset: 0, length: 8 }],
            types,
            functions: vec![function],
            files: vec![model::File { name: "f.c".into(), checksum: None }],
            lines: vec![Line { section: 0, offset: 0, file: 0, line: 3, column: 0 }, Line { section: 0, offset: 4, file: 0, line: 4, column: 0 }],
            ..Info::default()
        };
        Object {
            name: "f.c".into(),
            arch: Arch::I8086,
            sections: vec![text],
            symbols: vec![Symbol { name: "_f".into(), binding: Binding::Public, definition: Definition::Defined { section: 0, offset: 0 }, group: None }],
            omf_groups: Vec::new(),
            debug: Some(info),
        }
    }

    fn local(name: &str, kind: model::Kind, disp: i64) -> Variable {
        Variable { name: name.into(), r#type: 0, kind, location: Location::Frame { disp } }
    }

    fn procedure() -> T {
        T::Procedure { result: Some(0), parameters: vec![0], convention: None }
    }

    /// The model's parameter, local, return type and lines reach CodeView as the records a debugger
    /// reads; before the model, only the backend's own structures could write them.
    #[test]
    fn a_function_of_the_model_is_a_procedure_with_its_frame_variables_and_lines() {
        let made = object(vec![local("x", model::Kind::Parameter, 4), local("y", model::Kind::Local, -2)], vec![int(), procedure()]);
        let records = omf::parse(&write::write(&made).unwrap()).unwrap();
        let shape = cvinfo::parse(&records).shape();
        assert_eq!(shape, ["LOCAL f.y: INTEGER", "PARAM f.x: INTEGER", "PROC f flags 0 (INTEGER) -> INTEGER"], "{shape:#?}");
        let lines: Vec<(u16, u16)> = records.iter().filter(|one| one.r#type == omf::LINNUM).flat_map(|one| omf::lines(one).1).collect();
        assert_eq!(lines, [(3, 0), (4, 4)]);
    }

    /// A register location is the allocator's answer CodeView 4 can name (S_REGISTER) but this
    /// writer does not yet: it is refused by name, never written as a frame cell.
    #[test]
    fn a_register_location_is_refused_not_written_as_a_frame_cell() {
        let in_register = Variable { name: "x".into(), r#type: 0, kind: model::Kind::Parameter, location: Location::Register("ax".into()) };
        let why = write::write(&object(vec![in_register], vec![int(), procedure()])).unwrap_err().to_string();
        assert!(why.contains("x is in register ax"), "{why}");
    }

    /// A 64-bit integer is refused with its type, not truncated to a long.
    #[test]
    fn a_64_bit_integer_is_refused_not_narrowed() {
        let wide = T::Scalar(S::Int { bytes: 8, signed: true });
        let why = write::write(&object(Vec::new(), vec![wide, procedure()])).unwrap_err().to_string();
        assert!(why.contains("no type for"), "{why}");
    }

    /// A parameter that arrives in a register (there until the body starts) and one the optimiser removed have
    /// no frame cell CodeView 4 could name: they are left out, and the frame variables beside them are not.
    #[test]
    fn a_register_parameter_and_a_removed_one_are_left_out_of_codeview_not_refused() {
        let entry = Range { section: 0, offset: 0, length: 5 };
        let in_register = Variable { name: "r".into(), r#type: 0, kind: model::Kind::Parameter, location: Location::List(vec![(entry, Location::Register("ax".into()))]) };
        let removed = Variable { name: "g".into(), r#type: 0, kind: model::Kind::Parameter, location: Location::List(Vec::new()) };
        let made = object(vec![local("x", model::Kind::Parameter, 4), in_register, removed], vec![int(), procedure()]);
        let shape = cvinfo::parse(&omf::parse(&write::write(&made).unwrap()).unwrap()).shape();
        assert_eq!(shape, ["PARAM f.x: INTEGER", "PROC f flags 0 (INTEGER) -> INTEGER"], "{shape:#?}");
    }
}
