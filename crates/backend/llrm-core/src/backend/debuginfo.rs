//! `-g`: a module's debug information as [`llrm_object::debug::Info`],
//! whichever format a writer encodes it in. [`described`] reads MIR's metadata
//! (`llrm_mir::debuginfo`); once the code is laid out, [`laid_out`] adds each
//! procedure's place, variables and lines.

use std::collections::BTreeSet;

use llrm_mir::{MetadataId, debuginfo as di};
use llrm_object::debug::{self as model, Info, Kind, Location, Variable};

use crate::backend::objbuild::Segment;
use crate::backend::{globals, masm};
use crate::model::ir::Space;
use crate::model::lir::DebugPlace;
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
    /// The format finds a frame cell from the canonical frame address
    /// (`FrameBase::Cfa`); set where the format is.
    pub cfa: bool,
    /// The format says where a value is over a range of code: a cell reached
    /// through the stack pointer is told range by range.
    pub ranges: bool,
    /// The target's register file, as described: which registers are views of
    /// which, and which hold values.
    pub file: Vec<llrm_target::registers::Register>,
    /// What the target calls a call's return address in call frame information;
    /// empty where it numbers none.
    pub return_register: String,
    /// The frame and stack registers of 32-bit code, and the bytes a call
    /// pushes (near, far): what its frame rows are read from.
    pub frame: Option<(iced_x86::Register, iced_x86::Register, [i64; 2])>,
    pub registers: Vec<model::Register>,
    pub types: Vec<model::Type>,
    /// Each MIR type node's type.
    pub nodes: IndexMap<MetadataId, model::TypeId>,
    /// By each procedure's symbol.
    pub procedures: IndexMap<String, Procedure>,
    pub globals: Vec<Global>,
}

fn narrow<T: TryFrom<i64>>(
    value: i64,
    what: &str,
) -> Result<T, String> {
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

/// `one`, its targets and members already read into `nodes`; `near` the width
/// of an offset.
fn typed(
    one: &di::Type,
    nodes: &IndexMap<MetadataId, model::TypeId>,
    near: u8,
) -> Result<model::Type, String> {
    let node = |id: MetadataId| nodes.get(&id).copied().ok_or_else(|| format!("debug type !{} is not listed", id.0));
    let target = || one.target.map(node).ok_or_else(|| format!("a {} of nothing", one.kind.value()))?;
    Ok(match one.kind {
        di::Kind::Scalar => {
            let described =
                scalar(di::Scalar::from_value(&one.name).ok_or_else(|| format!("no debug scalar {}", one.name))?);
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
                    let bits = member
                        .bits
                        .map(|(start, width)| {
                            Ok::<_, String>((
                                narrow(start, "a bit field's start")?,
                                narrow(width, "a bit field's width")?,
                            ))
                        })
                        .transpose()?;
                    Ok(model::Field {
                        name: member.name.clone(),
                        r#type: node(member.r#type)?,
                        offset: narrow(member.offset, "a field's offset")?,
                        bits,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            model::Type::Struct {
                name: one.name.clone(),
                bytes: narrow(one.size, "a structure's size")?,
                fields,
                union: one.kind == di::Kind::Union,
            }
        }
        di::Kind::Procedure => model::Type::Procedure {
            result: one.target.map(node).transpose()?,
            parameters: one.members.iter().map(|member| node(member.r#type)).collect::<Result<Vec<_>, String>>()?,
            convention: None,
        },
    })
}

/// `module`'s debug information for `arch`, each global and function by the
/// symbol `names` gives it; None without any.
pub fn described(
    module: &llrm_mir::Module,
    names: &IndexMap<(Space, i64), String>,
    producer: model::Producer,
    arch: &dyn llrm_target::Target,
) -> Result<Option<Debug>, String> {
    let near = arch.object().bitness;
    let functions = di::functions(module);
    let globals = di::globals(module);
    if functions.is_empty() && globals.is_empty() {
        return Ok(None);
    }
    let symbol = |name: &str| {
        module.named(name).and_then(|id| names.get(&(globals::space(module, id), i64::from(id.0)))).cloned()
    };
    // Every type has its place before any is read: a member may name one made
    // after it.
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
        procedures.insert(
            at,
            Procedure { name: function.name.clone(), r#type: node(function.r#type)?, module: function.module },
        );
    }
    let mut out = Vec::new();
    for global in globals {
        let Some(at) = symbol(&global.global) else { continue };
        out.push(Global {
            name: global.name,
            r#type: node(global.r#type)?,
            symbol: at,
            displacement: global.offset,
            scope: global.scope.as_deref().and_then(&symbol),
        });
    }
    let file = llrm_target::registers::parse(&arch.registers_text())?;
    let frame_register = llrm_target::registers::of_class(&file, "frame")
        .first()
        .map(|one| (*one).to_owned())
        .ok_or("the target's register file has no frame register")?;
    let return_register =
        llrm_target::registers::of_class(&file, "pc").first().map(|one| (*one).to_owned()).unwrap_or_default();
    let frame = Some({
        let registers = arch.frame_registers();
        (registers.pointer, registers.stack, [arch.return_address_bytes(false), arch.return_address_bytes(true)])
    });
    let file_copy = file.clone();
    let registers = file
        .into_iter()
        .map(|one| model::Register { name: one.name, bits: one.bits, dwarf: one.dwarf, codeview: one.codeview })
        .collect();
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
    Ok(Some(Debug {
        format: model::Format::Default,
        language,
        dialect,
        producer,
        frame_register,
        cfa: false,
        ranges: false,
        file: file_copy,
        return_register,
        frame,
        registers,
        types,
        nodes,
        procedures,
        globals: out,
    }))
}

/// Where one value is, as the model says it.
fn located(at: super::valuetrack::Where) -> Location {
    match at {
        super::valuetrack::Where::Place(masm::Place::Register(register)) => {
            Location::Register(format!("{register:?}").to_lowercase())
        }
        super::valuetrack::Where::Place(masm::Place::Cell { disp, .. }) => Location::Frame { disp },
        super::valuetrack::Where::Constant(value) => Location::Constant(value),
    }
}

/// The one place that holds a value in every range of `ranges`, which must run
/// on from the first without a gap to the code `until`: the place, and the
/// offset the first range starts at.
fn stable(
    ranges: &[(usize, usize, Vec<super::valuetrack::Where>)],
    until: usize,
) -> Option<(usize, super::valuetrack::Where)> {
    let (first, last) = (ranges.first()?, ranges.last()?);
    if last.1 < until || ranges.windows(2).any(|pair| pair[0].1 != pair[1].0) {
        return None;
    }
    let place = first.2.iter().copied().find(|place| {
        matches!(place, super::valuetrack::Where::Place(_)) && ranges.iter().all(|one| one.2.contains(place))
    })?;
    Some((first.0, place))
}

/// A variable's parts, each the bytes of it (none for all of it) with where it
/// is over ranges of the code, as the variable's one location over the code:
/// the whole where there is one, else the pieces there are, in order, a piece
/// nowhere left a gap.
fn combined(
    parts: Vec<(Option<(u32, u32)>, Vec<(usize, usize, super::valuetrack::Where)>)>
) -> Vec<(usize, usize, Location)> {
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
        let at = |ranges: &Vec<(usize, usize, super::valuetrack::Where)>| {
            ranges.iter().find(|&&(a, b, _)| a <= from && to <= b).map(|&(.., place)| place)
        };
        let location =
            if let Some(whole) = parts.iter().find(|(piece, _)| piece.is_none()).and_then(|(_, ranges)| at(ranges)) {
                located(whole)
            } else {
                let mut pieces: Vec<(u32, u32, super::valuetrack::Where)> = parts
                    .iter()
                    .filter_map(|(piece, ranges)| {
                        Some((piece.map(|(offset, _)| offset)?, piece.map(|(_, bytes)| bytes)?, at(ranges)?))
                    })
                    .collect();
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

/// `module`'s debug information for the object `source`, its code laid out in
/// `segments` (a procedure's is the one `symbols` puts it in) at `symbols`,
/// each symbol at its index in `ids`; only what a symbol still defines is
/// named.
pub fn laid_out(
    debug: &Debug,
    module: &masm::Module,
    source: &str,
    segments: &[Segment],
    symbols: &IndexMap<String, (usize, usize)>,
    ids: &IndexMap<String, usize>,
) -> Result<Info, String> {
    let defined = |name: &str| symbols.contains_key(name);
    let type_of =
        |node: MetadataId| debug.nodes.get(&node).copied().ok_or_else(|| format!("debug type !{} unread", node.0));
    let variable = |name: &str, r#type: model::TypeId, kind: Kind, symbol: &str, disp: i64| Variable {
        name: name.to_owned(),
        r#type,
        kind,
        location: Location::Static { symbol: ids[symbol], disp },
    };
    let mut starts: Vec<(usize, usize)> =
        module.procedures.iter().filter_map(|one| symbols.get(&one.name).copied()).collect();
    starts.sort_unstable();
    let mut info = Info {
        format: debug.format,
        language: debug.language,
        dialect: debug.dialect,
        producer: debug.producer,
        frame_register: debug.frame_register.clone(),
        return_register: debug.return_register.clone(),
        registers: debug.registers.clone(),
        files: vec![model::File { name: source.to_owned(), checksum: None }],
        types: debug.types.clone(),
        ..Info::default()
    };
    // The register the code would address a cell by is the one pushed below the
    // return address: a frame address away.
    if let Some((.., entry)) = debug.frame.filter(|_| debug.cfa) {
        info.frame_base = model::FrameBase::Cfa { bias: 2 * entry[0] };
    }
    info.globals = debug
        .globals
        .iter()
        .filter(|one| one.scope.is_none() && defined(&one.symbol))
        .map(|one| variable(&one.name, one.r#type, Kind::Local, &one.symbol, one.displacement))
        .collect();
    for procedure in &module.procedures {
        let (Some(described), Some(&(section, start))) =
            (debug.procedures.get(&procedure.name), symbols.get(&procedure.name))
        else {
            continue;
        };
        let code = &segments[section];
        let end =
            starts.iter().find(|&&(at, one)| at == section && one > start).map_or(code.image.len(), |&(_, one)| one);
        // Its own statics, then each variable of its body.
        let mut variables: Vec<Variable> = debug
            .globals
            .iter()
            .filter(|one| one.scope.as_ref() == Some(&procedure.name) && defined(&one.symbol))
            .map(|one| variable(&one.name, one.r#type, Kind::Local, &one.symbol, one.displacement))
            .collect();
        // A body starts before its end, and ends after its start.
        let bound = |mark: masm::Mark, within: std::ops::Range<usize>| {
            code.bodies.iter().find(|&&(one, at)| one == mark && within.contains(&at)).map(|&(_, at)| at - start)
        };
        let body = (
            bound(masm::Mark::BodyStart, start..end).unwrap_or(0),
            bound(masm::Mark::BodyEnd, start + 1..end + 1).unwrap_or(end - start),
        );
        // How to find the caller from each place in the code, where the code
        // can be followed.
        let frame = debug
            .frame
            .and_then(
                |(frame, stack, entry)| {
                    let pops: Vec<(usize, i64)> = code
                        .bodies
                        .iter()
                        .filter_map(|&(mark, at)| match mark {
                            masm::Mark::Pops(bytes) if (start + 1..=end).contains(&at) => Some((at - start, bytes)),
                            _ => None,
                        })
                        .collect();
                    super::cfi::rows(
                        &code.image[start..end],
                        procedure.body.bits,
                        frame,
                        stack,
                        entry[usize::from(procedure.far)],
                        &pops,
                    )
                    .ok()
                },
            );
        for one in &procedure.body.variables {
            let r#type = type_of(one.r#type)?;
            let kind = if one.parameter { Kind::Parameter } else { Kind::Local };
            let addr = match &one.place {
                DebugPlace::At(addr) => addr,
                // A parameter that arrives in a register is there until the
                // body starts, and no longer said: the register is the
                // allocator's from then on. The range takes in the body's first
                // instruction.
                DebugPlace::Register(register) => {
                    let entry = model::Range { section, offset: start, length: body.0 + 1 };
                    let location =
                        Location::List(vec![(entry, Location::Register(format!("{register:?}").to_lowercase()))]);
                    variables.push(Variable { name: one.name.clone(), r#type, kind, location });
                    continue;
                }
                // Found over the code from what the notes say.
                DebugPlace::Tracked(_) => {
                    // Where a frame register would sit below the canonical
                    // frame address: the return address and the
                    // register saved.
                    let bias = match (info.frame_base, debug.frame) {
                        (model::FrameBase::Cfa { bias }, _) => bias,
                        (_, Some((.., entry))) => entry[usize::from(procedure.far)] + entry[0],
                        _ => continue,
                    };
                    let Some(rows) = frame.as_ref() else { continue };
                    let DebugPlace::Tracked(node) = one.place else { unreachable!("matched above") };
                    let marks: Vec<(usize, masm::Mark)> = code
                        .bodies
                        .iter()
                        .filter(|&&(_, at)| (start..=end).contains(&at))
                        .map(|&(mark, at)| (at - start, mark))
                        .collect();
                    // A note a pass lost with the instruction it stood before
                    // leaves the variable at whatever the notes
                    // before it said, which may be a value it no longer has: a
                    // variable with one is said to be
                    // nowhere.
                    let emitted: BTreeSet<u32> = marks
                        .iter()
                        .filter_map(|(_, mark)| if let masm::Mark::Note(note) = mark { Some(*note) } else { None })
                        .collect();
                    let lost = procedure
                        .body
                        .notes
                        .iter()
                        .enumerate()
                        .any(|(note, said)| said.variable == node && !emitted.contains(&(note as u32)));
                    let found = if lost {
                        Default::default()
                    } else {
                        super::valuetrack::tracked(
                            &code.image[start..end],
                            procedure.body.bits,
                            &super::valuetrack::Regs::new(
                                &debug.file,
                                debug.frame.map_or(iced_x86::Register::None, |(pointer, ..)| pointer),
                                debug.frame.map_or(iced_x86::Register::None, |(_, stack, _)| stack),
                            ),
                            rows,
                            bias,
                            &procedure.body.notes,
                            &marks,
                        )
                    };
                    let parts: Vec<_> = found
                        .into_iter()
                        .filter(|((variable, _), _)| *variable == node)
                        .map(|((_, piece), ranges)| (piece, ranges))
                        .collect();
                    // A format that names one place for a scope (CodeView 4,
                    // Turbo Debugger) says a variable that is in
                    // one register, or one cell, from where it first has a
                    // value to the last statement (past it
                    // the epilogue puts the registers back, and no line is
                    // there to read): the place every range
                    // of it has. One that has none, or has a gap, it leaves
                    // out.
                    let single = match &parts[..] {
                        [(None, ranges)] if matches!(info.frame_base, model::FrameBase::Register) => {
                            Some(stable(ranges, body.1))
                        }
                        _ => None,
                    };
                    let entries: Vec<(model::Range, Location)> = match single {
                        Some(Some((from, place))) => vec![(
                            model::Range { section, offset: start + from, length: end - start - from },
                            located(place),
                        )],
                        Some(None) => Vec::new(),
                        None => combined(
                            parts
                                .into_iter()
                                .map(|(piece, ranges)| {
                                    (
                                        piece,
                                        ranges.into_iter().map(|(from, to, places)| (from, to, places[0])).collect(),
                                    )
                                })
                                .collect(),
                        )
                        .into_iter()
                        .map(|(from, to, location)| {
                            (model::Range { section, offset: start + from, length: to - from }, location)
                        })
                        .collect(),
                    };
                    if single.is_some() {
                        // A cell is a frame variable like any other.
                        if let [(_, Location::Frame { disp })] = &entries[..] {
                            variables.push(Variable {
                                name: one.name.clone(),
                                r#type,
                                kind,
                                location: Location::Frame { disp: *disp },
                            });
                            continue;
                        }
                    }
                    variables.push(Variable {
                        name: one.name.clone(),
                        r#type,
                        kind,
                        location: Location::List(entries),
                    });
                    continue;
                }
                // The optimiser removed it: no location anywhere is "optimized
                // out".
                DebugPlace::Gone => {
                    variables.push(Variable {
                        name: one.name.clone(),
                        r#type,
                        kind,
                        location: Location::List(Vec::new()),
                    });
                    continue;
                }
            };
            match addr.space {
                Space::Frame => {
                    let home = Location::Frame { disp: addr.disp };
                    // The argument is in its register until the function stores
                    // it into the home.
                    let stored = one
                        .arrives
                        .zip(debug.frame.filter(|_| procedure.body.bits == 32))
                        .and_then(
                            |(register, (frame, stack, entry))| {
                                let cell = match info.frame_base {
                                    model::FrameBase::Register => {
                                        super::arrival::Cell::Frame { register: frame, disp: addr.disp }
                                    }
                                    model::FrameBase::Cfa { bias } => super::arrival::Cell::Stack {
                                        register: stack,
                                        entry: entry[usize::from(procedure.far)],
                                        from_cfa: addr.disp - bias,
                                    },
                                };
                                super::arrival::stored(&code.image[start..end], cell, register).map(|at| (register, at))
                            },
                        );
                    let location = match stored {
                        Some((register, at)) if at < end - start => {
                            let range = |offset, length| model::Range { section, offset, length };
                            Location::List(vec![
                                (range(start, at), Location::Register(format!("{register:?}").to_lowercase())),
                                (range(start + at, end - start - at), home),
                            ])
                        }
                        _ => home,
                    };
                    variables.push(Variable { name: one.name.clone(), r#type, kind, location })
                }
                space => {
                    let Some(symbol) = module.names.get(&(space, addr.index)).filter(|one| defined(one)) else {
                        continue;
                    };
                    variables.push(variable(&one.name, r#type, kind, symbol, addr.disp));
                }
            }
        }
        // A format that finds a cell by the frame register says nothing of one
        // in a function that keeps none (the code is not changed for
        // `-g` to keep it), nor of a place that is in a cell of a list: it is
        // left out.
        if matches!(info.frame_base, model::FrameBase::Register)
            && procedure.body.bits == 32
            && frame.as_ref().is_some_and(|rows| !rows.iter().any(|row| row.cfa_register == debug.frame_register))
        {
            let framed = |location: &Location| match location {
                Location::Frame { .. } => true,
                Location::List(entries) => entries.iter().any(|(_, place)| matches!(place, Location::Frame { .. })),
                _ => false,
            };
            // Through the stack pointer, a format with ranges says it over each
            // stretch the pointer is as far from the frame address:
            // the cell is `disp - bias` from the address, and the address is
            // the register plus the row's offset.
            let bias = debug.frame.map(|(.., entry)| entry[usize::from(procedure.far)] + entry[0]);
            let rows = frame.as_deref().filter(|_| debug.ranges);
            variables = variables
                .into_iter()
                .filter_map(|one| match (&one.location, rows, bias) {
                    (Location::Frame { disp }, Some(rows), Some(bias)) => {
                        let entries: Vec<(model::Range, Location)> = rows
                            .iter()
                            .enumerate()
                            .map(|(at, row)| {
                                let to = rows.get(at + 1).map_or(end - start, |next| next.offset);
                                (
                                    model::Range { section, offset: start + row.offset, length: to - row.offset },
                                    Location::Relative {
                                        register: row.cfa_register.clone(),
                                        disp: disp - bias + row.cfa_offset,
                                    },
                                )
                            })
                            .filter(|(range, _)| range.length > 0)
                            .collect();
                        Some(Variable { location: Location::List(entries), ..one })
                    }
                    (location, ..) if framed(location) => None,
                    _ => Some(one),
                })
                .collect();
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
    // The code of each segment that has a procedure: from the first to its last
    // byte.
    for section in starts.iter().map(|&(section, _)| section).collect::<std::collections::BTreeSet<_>>() {
        let first = starts.iter().find(|&&(at, _)| at == section).map_or(0, |&(_, one)| one);
        info.code.push(model::Range { section, offset: first, length: segments[section].image.len() - first });
    }
    for (section, segment) in segments.iter().enumerate() {
        info.lines.extend(segment.lines.iter().map(|&(line, offset)| model::Line {
            section,
            offset,
            file: 0,
            line,
            column: 0,
        }));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::super::masm::Place;
    use super::super::valuetrack::Where;
    use super::stable;

    fn reg(register: iced_x86::Register) -> Where {
        Where::Place(Place::Register(register))
    }

    /// A format with one place for a scope takes the place every range has, not
    /// the first of the first range: `k` was computed in `ax` and copied to
    /// `bx` for a call that clobbers `ax`, so `bx` is its place.
    #[test]
    fn the_place_a_scope_is_given_is_one_every_range_holds() {
        use iced_x86::Register::{AX, BX};
        let ranges = [(2, 6, vec![reg(AX), reg(BX)]), (6, 12, vec![reg(BX)])];
        assert_eq!(stable(&ranges, 11), Some((2, reg(BX))));
    }

    /// A gap, a range that stops short of the last statement, and a place no
    /// range shares, each leave the variable out.
    #[test]
    fn a_variable_that_has_a_gap_or_stops_short_or_moves_has_no_place_for_a_scope() {
        use iced_x86::Register::{AX, BX};
        assert_eq!(stable(&[(2, 4, vec![reg(AX)]), (5, 12, vec![reg(AX)])], 11), None, "a gap");
        assert_eq!(stable(&[(2, 8, vec![reg(AX)])], 11), None, "short of the last statement");
        assert_eq!(stable(&[(2, 4, vec![reg(AX)]), (4, 12, vec![reg(BX)])], 11), None, "moved");
    }
}
