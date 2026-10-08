//! `-g`: a module's debug information as [`llrm_object::debug::Info`], whichever format a writer
//! encodes it in. [`described`] reads MIR's metadata (`llrm_mir::debuginfo`); once the code is
//! laid out, [`laid_out`] adds each procedure's place, variables and lines.

use std::collections::BTreeSet;
use llrm_mir::{debuginfo as di, MetadataId};
use llrm_object::debug::{self as model, Info, Kind, Location, Variable};

use crate::backend::objbuild::Segment;
use crate::model::lir::DebugPlace;
use crate::backend::{globals, masm};
use crate::model::ir::Space;
use crate::support::hash::IndexMap;

/// A procedure as its debugger names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub r#type: model::TypeId,
    /// The module's own code, whose variables are the module's.
    pub module: bool,
}

/// A variable in data; `scope` the symbol of the procedure declaring it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Global {
    pub name: String,
    pub r#type: model::TypeId,
    pub symbol: String,
    pub displacement: i64,
    pub scope: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Debug {
    pub format: model::Format,
    pub language: model::Language,
    pub dialect: model::Dialect,
    pub producer: model::Producer,
    pub frame_register: String,
    /// What the target calls a call's return address in call frame information; empty where it numbers none.
    pub return_register: String,
    /// The frame and stack registers of 32-bit code, and the bytes a call pushes (near, far): what its frame
    /// rows are read from.
    pub frame: Option<(iced_x86::Register, iced_x86::Register, [i64; 2])>,
    pub registers: Vec<model::Register>,
    pub types: Vec<model::Type>,
    /// Each MIR type node's type.
    pub nodes: IndexMap<MetadataId, model::TypeId>,
    /// By each procedure's symbol.
    pub procedures: IndexMap<String, Procedure>,
    pub globals: Vec<Global>,
}

fn narrow<T: TryFrom<i64>>(value: i64, what: &str) -> Result<T, String> {
    T::try_from(value).map_err(|_| format!("{what} {value} does not fit its field"))
}

fn scalar(scalar: di::Scalar) -> model::Scalar {
    use model::Scalar as S;
    match scalar {
        di::Scalar::Void => S::Void,
        di::Scalar::Char => S::Char,
        di::Scalar::Int8 => S::Int { bytes: 1, signed: true },
        di::Scalar::UInt8 => S::Int { bytes: 1, signed: false },
        di::Scalar::Int16 => S::Int { bytes: 2, signed: true },
        di::Scalar::UInt16 => S::Int { bytes: 2, signed: false },
        di::Scalar::Int32 => S::Int { bytes: 4, signed: true },
        di::Scalar::UInt32 => S::Int { bytes: 4, signed: false },
        di::Scalar::Int64 => S::Int { bytes: 8, signed: true },
        di::Scalar::UInt64 => S::Int { bytes: 8, signed: false },
        di::Scalar::Float32 => S::Float { bytes: 4 },
        di::Scalar::Float64 => S::Float { bytes: 8 },
        di::Scalar::Float80 => S::Float { bytes: 10 },
        di::Scalar::Currency => S::Currency,
        di::Scalar::String => S::BasicString { far: false },
        di::Scalar::FarString => S::BasicString { far: true },
    }
}

/// `one`, its targets and members already read into `nodes`; `near` the width of an offset.
fn typed(one: &di::Type, nodes: &IndexMap<MetadataId, model::TypeId>, near: u8) -> Result<model::Type, String> {
    let node = |id: MetadataId| nodes.get(&id).copied().ok_or_else(|| format!("debug type !{} is not listed", id.0));
    let target = || one.target.map(node).ok_or_else(|| format!("a {} of nothing", one.kind.value()))?;
    Ok(match one.kind {
        di::Kind::Scalar => {
            let described = scalar(di::Scalar::from_value(&one.name).ok_or_else(|| format!("no debug scalar {}", one.name))?);
            match &one.spelling {
                Some(spelling) => model::Type::Basic { name: spelling.clone(), scalar: described },
                None => model::Type::Scalar(described),
            }
        }
        di::Kind::FixedString => model::Type::FixedString(narrow(one.size, "a STRING's length")?),
        di::Kind::Array => model::Type::Array { element: target()?, bytes: None },
        di::Kind::Sized => model::Type::Array { element: target()?, bytes: Some(narrow(one.size, "an array's size")?) },
        di::Kind::Pointer => {
            let (reach, bytes) = match one.reach {
                di::Reach::Near => (model::Reach::Near, near),
                di::Reach::Far => (model::Reach::Far, near + 2),
                di::Reach::Huge => (model::Reach::Huge, near + 2),
            };
            model::Type::Pointer { target: target()?, bytes, reach }
        }
        di::Kind::Reference => model::Type::Reference(target()?),
        di::Kind::Struct | di::Kind::Union => {
            let fields = one
                .members
                .iter()
                .map(|member| {
                    let bits = member.bits.map(|(start, width)| Ok::<_, String>((narrow(start, "a bit field's start")?, narrow(width, "a bit field's width")?))).transpose()?;
                    Ok(model::Field { name: member.name.clone(), r#type: node(member.r#type)?, offset: narrow(member.offset, "a field's offset")?, bits })
                })
                .collect::<Result<Vec<_>, String>>()?;
            model::Type::Struct { name: one.name.clone(), bytes: narrow(one.size, "a structure's size")?, fields, union: one.kind == di::Kind::Union }
        }
        di::Kind::Procedure => model::Type::Procedure {
            result: one.target.map(node).transpose()?,
            parameters: one.members.iter().map(|member| node(member.r#type)).collect::<Result<Vec<_>, String>>()?,
            convention: None,
        },
    })
}

/// `module`'s debug information for `arch`, each global and function by the symbol `names` gives
/// it; None without any.
pub fn described(module: &llrm_mir::Module, names: &IndexMap<(Space, i64), String>, producer: model::Producer, arch: &dyn llrm_target::Target) -> Result<Option<Debug>, String> {
    let near = arch.object().bitness;
    let functions = di::functions(module);
    let globals = di::globals(module);
    if functions.is_empty() && globals.is_empty() {
        return Ok(None);
    }
    let symbol = |name: &str| module.named(name).and_then(|id| names.get(&(globals::space(module, id), i64::from(id.0)))).cloned();
    // Every type has its place before any is read: a member may name one made after it.
    let listed = di::types(module);
    let nodes: IndexMap<MetadataId, model::TypeId> = listed.iter().enumerate().map(|(at, &id)| (id, at)).collect();
    let mut types = Vec::new();
    for &id in &listed {
        let one = di::read_type(module, id).ok_or_else(|| format!("debug type !{} does not read", id.0))?;
        types.push(typed(&one, &nodes, (near / 8) as u8)?);
    }
    let node = |id: MetadataId| nodes.get(&id).copied().ok_or_else(|| format!("debug type !{} unlisted", id.0));
    let mut procedures = IndexMap::default();
    for function in &functions {
        let Some(at) = symbol(&function.function) else { continue };
        procedures.insert(at, Procedure { name: function.name.clone(), r#type: node(function.r#type)?, module: function.module });
    }
    let mut out = Vec::new();
    for global in globals {
        let Some(at) = symbol(&global.global) else { continue };
        out.push(Global { name: global.name, r#type: node(global.r#type)?, symbol: at, displacement: global.offset, scope: global.scope.as_deref().and_then(&symbol) });
    }
    let file = llrm_target::registers::parse(&arch.registers_text())?;
    let frame_register = llrm_target::registers::of_class(&file, "frame").first().map(|one| (*one).to_owned()).ok_or("the target's register file has no frame register")?;
    let return_register = llrm_target::registers::of_class(&file, "pc").first().map(|one| (*one).to_owned()).unwrap_or_default();
    let frame = (near == 32).then(|| {
        let registers = arch.frame_registers();
        (registers.pointer, registers.stack, [arch.return_address_bytes(false), arch.return_address_bytes(true)])
    });
    let registers = file.into_iter().map(|one| model::Register { name: one.name, bits: one.bits, dwarf: one.dwarf, codeview: one.codeview }).collect();
    let language = match di::language(module) {
        Some(di::Language::C) => model::Language::C,
        Some(di::Language::Basic) => model::Language::Basic,
        Some(di::Language::Nib) => model::Language::Nib,
        None => model::Language::Unknown,
    };
    let dialect = match di::dialect(module) {
        di::Dialect::Bc => model::Dialect::Bc,
        di::Dialect::Cv4 => model::Dialect::Cv4,
    };
    Ok(Some(Debug { format: model::Format::Default, language, dialect, producer, frame_register, return_register, frame, registers, types, nodes, procedures, globals: out }))
}

/// Where one value is, as the model says it.
fn located(at: super::valuetrack::Where) -> Location {
    match at {
        super::valuetrack::Where::Place(masm::Place::Register(register)) => Location::Register(format!("{register:?}").to_lowercase()),
        super::valuetrack::Where::Place(masm::Place::Cell { disp, .. }) => Location::Frame { disp },
        super::valuetrack::Where::Constant(value) => Location::Constant(value),
    }
}

/// A variable's parts, each the bytes of it (none for all of it) with where it is over ranges of the code, as the variable's one
/// location over the code: the whole where there is one, else the pieces there are, in order, a piece nowhere left a gap.
fn combined(parts: Vec<(Option<(u32, u32)>, Vec<(usize, usize, super::valuetrack::Where)>)>) -> Vec<(usize, usize, Location)> {
    let mut edges: BTreeSet<usize> = BTreeSet::new();
    for (_, ranges) in &parts {
        for &(from, to, _) in ranges {
            edges.extend([from, to]);
        }
    }
    let edges: Vec<usize> = edges.into_iter().collect();
    let mut out: Vec<(usize, usize, Location)> = Vec::new();
    for pair in edges.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let at = |ranges: &Vec<(usize, usize, super::valuetrack::Where)>| ranges.iter().find(|&&(a, b, _)| a <= from && to <= b).map(|&(.., place)| place);
        let location = if let Some(whole) = parts.iter().find(|(piece, _)| piece.is_none()).and_then(|(_, ranges)| at(ranges)) {
            located(whole)
        } else {
            let mut pieces: Vec<(u32, u32, super::valuetrack::Where)> = parts.iter().filter_map(|(piece, ranges)| Some((piece.map(|(offset, _)| offset)?, piece.map(|(_, bytes)| bytes)?, at(ranges)?))).collect();
            if pieces.is_empty() {
                continue;
            }
            pieces.sort_by_key(|&(offset, ..)| offset);
            let mut made = Vec::new();
            let mut next = 0;
            for (offset, bytes, place) in pieces {
                if offset > next {
                    made.push((offset - next, None));
                }
                made.push((bytes, Some(located(place))));
                next = offset + bytes;
            }
            Location::Pieces(made)
        };
        match out.last_mut() {
            Some(last) if last.1 == from && last.2 == location => last.1 = to,
            _ => out.push((from, to, location)),
        }
    }
    out
}

/// `module`'s debug information for the object `source`, its code laid out in `segments` (a procedure's
/// is the one `symbols` puts it in) at `symbols`, each symbol at its index in `ids`; only what a symbol still
/// defines is named.
pub fn laid_out(debug: &Debug, module: &masm::Module, source: &str, segments: &[Segment], symbols: &IndexMap<String, (usize, usize)>, ids: &IndexMap<String, usize>) -> Result<Info, String> {
    let defined = |name: &str| symbols.contains_key(name);
    let type_of = |node: MetadataId| debug.nodes.get(&node).copied().ok_or_else(|| format!("debug type !{} unread", node.0));
    let variable = |name: &str, r#type: model::TypeId, kind: Kind, symbol: &str, disp: i64| Variable { name: name.to_owned(), r#type, kind, location: Location::Static { symbol: ids[symbol], disp } };
    let mut starts: Vec<(usize, usize)> = module.procedures.iter().filter_map(|one| symbols.get(&one.name).copied()).collect();
    starts.sort_unstable();
    let mut info = Info { format: debug.format, language: debug.language, dialect: debug.dialect, producer: debug.producer, frame_register: debug.frame_register.clone(), return_register: debug.return_register.clone(), registers: debug.registers.clone(), files: vec![model::File { name: source.to_owned(), checksum: None }], types: debug.types.clone(), ..Info::default() };
    // The register the code would address a cell by is the one pushed below the return address: a frame address away.
    if let Some((.., entry)) = debug.frame.filter(|_| module.procedures.iter().any(|one| one.body.cfa_variables)) {
        info.frame_base = model::FrameBase::Cfa { bias: 2 * entry[0] };
    }
    info.globals = debug.globals.iter().filter(|one| one.scope.is_none() && defined(&one.symbol)).map(|one| variable(&one.name, one.r#type, Kind::Local, &one.symbol, one.displacement)).collect();
    for procedure in &module.procedures {
        let (Some(described), Some(&(section, start))) = (debug.procedures.get(&procedure.name), symbols.get(&procedure.name)) else { continue };
        let code = &segments[section];
        let end = starts.iter().find(|&&(at, one)| at == section && one > start).map_or(code.image.len(), |&(_, one)| one);
        // Its own statics, then each variable of its body.
        let mut variables: Vec<Variable> = debug
            .globals
            .iter()
            .filter(|one| one.scope.as_ref() == Some(&procedure.name) && defined(&one.symbol))
            .map(|one| variable(&one.name, one.r#type, Kind::Local, &one.symbol, one.displacement))
            .collect();
        // A body starts before its end, and ends after its start.
        let bound = |mark: masm::Mark, within: std::ops::Range<usize>| code.bodies.iter().find(|&&(one, at)| one == mark && within.contains(&at)).map(|&(_, at)| at - start);
        let body = (bound(masm::Mark::BodyStart, start..end).unwrap_or(0), bound(masm::Mark::BodyEnd, start + 1..end + 1).unwrap_or(end - start));
        // How to find the caller from each place in the code, where the code can be followed.
        let frame = debug.frame.and_then(|(frame, stack, entry)| {
            let pops: Vec<(usize, i64)> = code.bodies.iter().filter_map(|&(mark, at)| match mark {
                masm::Mark::Pops(bytes) if (start + 1..=end).contains(&at) => Some((at - start, bytes)),
                _ => None,
            }).collect();
            super::cfi::rows(&code.image[start..end], frame, stack, entry[usize::from(procedure.far)], &pops).ok()
        });
        for one in &procedure.body.variables {
            let r#type = type_of(one.r#type)?;
            let kind = if one.parameter { Kind::Parameter } else { Kind::Local };
            let addr = match &one.place {
                DebugPlace::At(addr) => addr,
                // A parameter that arrives in a register is there until the body starts, and no longer said:
                // the register is the allocator's from then on. The range takes in the body's first instruction.
                DebugPlace::Register(register) => {
                    let entry = model::Range { section, offset: start, length: body.0 + 1 };
                    let location = Location::List(vec![(entry, Location::Register(format!("{register:?}").to_lowercase()))]);
                    variables.push(Variable { name: one.name.clone(), r#type, kind, location });
                    continue;
                }
                // Found over the code from what the notes say.
                DebugPlace::Tracked(_) => {
                    let (model::FrameBase::Cfa { bias }, Some(rows)) = (info.frame_base, frame.as_ref()) else { continue };
                    let DebugPlace::Tracked(node) = one.place else { unreachable!("matched above") };
                    let marks: Vec<(usize, masm::Mark)> = code.bodies.iter().filter(|&&(_, at)| (start..=end).contains(&at)).map(|&(mark, at)| (at - start, mark)).collect();
                    // A note a pass lost with the instruction it stood before leaves the variable at whatever the notes before it said,
                    // which may be a value it no longer has: a variable with one is said to be nowhere.
                    let emitted: BTreeSet<u32> = marks.iter().filter_map(|(_, mark)| if let masm::Mark::Note(note) = mark { Some(*note) } else { None }).collect();
                    let lost = procedure.body.notes.iter().enumerate().any(|(note, said)| said.variable == node && !emitted.contains(&(note as u32)));
                    let found = if lost { Default::default() } else { super::valuetrack::tracked(&code.image[start..end], rows, bias, &procedure.body.notes, &marks) };
                    let parts: Vec<_> = found.into_iter().filter(|((variable, _), _)| *variable == node).map(|((_, piece), ranges)| (piece, ranges)).collect();
                    let entries = combined(parts).into_iter().map(|(from, to, location)| (model::Range { section, offset: start + from, length: to - from }, location));
                    variables.push(Variable { name: one.name.clone(), r#type, kind, location: Location::List(entries.collect()) });
                    continue;
                }
                // The optimiser removed it: no location anywhere is "optimized out".
                DebugPlace::Gone => {
                    variables.push(Variable { name: one.name.clone(), r#type, kind, location: Location::List(Vec::new()) });
                    continue;
                }
            };
            match addr.space {
                Space::Frame => {
                    let home = Location::Frame { disp: addr.disp };
                    // The argument is in its register until the function stores it into the home.
                    let stored = one.arrives.zip(debug.frame).and_then(|(register, (frame, stack, entry))| {
                        let cell = match info.frame_base {
                            model::FrameBase::Register => super::arrival::Cell::Frame { register: frame, disp: addr.disp },
                            model::FrameBase::Cfa { bias } => super::arrival::Cell::Stack { register: stack, entry: entry[usize::from(procedure.far)], from_cfa: addr.disp - bias },
                        };
                        super::arrival::stored(&code.image[start..end], cell, register).map(|at| (register, at))
                    });
                    let location = match stored {
                        Some((register, at)) if at < end - start => {
                            let range = |offset, length| model::Range { section, offset, length };
                            Location::List(vec![(range(start, at), Location::Register(format!("{register:?}").to_lowercase())), (range(start + at, end - start - at), home)])
                        }
                        _ => home,
                    };
                    variables.push(Variable { name: one.name.clone(), r#type, kind, location })
                }
                space => {
                    let Some(symbol) = module.names.get(&(space, addr.index)).filter(|one| defined(one)) else { continue };
                    variables.push(variable(&one.name, r#type, kind, symbol, addr.disp));
                }
            }
        }
        info.functions.push(model::Function {
            name: described.name.clone(),
            symbol: ids[&procedure.name],
            r#type: described.r#type,
            ranges: vec![model::Range { section, offset: start, length: end - start }],
            body: Some(body),
            far: procedure.far,
            module: described.module,
            variables,
            blocks: Vec::new(),
            frame: frame.unwrap_or_default(),
        });
    }
    // The code of each segment that has a procedure: from the first to its last byte.
    for section in starts.iter().map(|&(section, _)| section).collect::<std::collections::BTreeSet<_>>() {
        let first = starts.iter().find(|&&(at, _)| at == section).map_or(0, |&(_, one)| one);
        info.code.push(model::Range { section, offset: first, length: segments[section].image.len() - first });
    }
    for (section, segment) in segments.iter().enumerate() {
        info.lines.extend(segment.lines.iter().map(|&(line, offset)| model::Line { section, offset, file: 0, line, column: 0 }));
    }
    Ok(info)
}

