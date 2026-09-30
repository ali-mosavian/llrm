//! `-g`: a module's CodeView debug information. [`described`] reads MIR's
//! metadata (`llrm_mir::debuginfo`); once the code is laid out,
//! [`segments`] adds each procedure's place and frame variables and
//! encodes $$SYMBOLS and $$TYPES through `llrm_omf::cvwrite`.

use llrm_mir::{debuginfo as di, MetadataId};
use llrm_omf::cvwrite::{self, Flavor, Reach, Scalar, Type, TypeId};

use crate::backend::omfwrite::{Fixup, Segment, OFFSET, POINTER};
use crate::backend::{globals, masm};
use crate::model::ir::Space;
use crate::support::hash::IndexMap;

/// A procedure as its debugger names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub r#type: TypeId,
    /// The module's own code, whose variables are the module's.
    pub module: bool,
}

/// A variable in data; `scope` the symbol of the procedure declaring it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Global {
    pub data: cvwrite::Data,
    pub scope: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Debug {
    pub flavor: Flavor,
    pub types: Vec<Type>,
    /// Each MIR type node's type.
    pub nodes: IndexMap<MetadataId, TypeId>,
    /// By each procedure's symbol.
    pub procedures: IndexMap<String, Procedure>,
    pub globals: Vec<Global>,
}

fn narrow<T: TryFrom<i64>>(value: i64, what: &str) -> Result<T, String> {
    T::try_from(value).map_err(|_| format!("{what} {value} does not fit its field"))
}

fn scalar(scalar: di::Scalar) -> Scalar {
    match scalar {
        di::Scalar::Char => Scalar::Char,
        di::Scalar::Int8 => Scalar::Int8,
        di::Scalar::UInt8 => Scalar::UInt8,
        di::Scalar::Int16 => Scalar::Int16,
        di::Scalar::UInt16 => Scalar::UInt16,
        di::Scalar::Int32 => Scalar::Int32,
        di::Scalar::UInt32 => Scalar::UInt32,
        di::Scalar::Float32 => Scalar::Float32,
        di::Scalar::Float64 => Scalar::Float64,
        di::Scalar::Float80 => Scalar::Float80,
        di::Scalar::Currency => Scalar::Currency,
        di::Scalar::String => Scalar::String { far: false },
        di::Scalar::FarString => Scalar::String { far: true },
    }
}

fn reach(reach: di::Reach) -> Reach {
    match reach {
        di::Reach::Near => Reach::Near,
        di::Reach::Far => Reach::Far,
        di::Reach::Huge => Reach::Huge,
    }
}

/// `one`, its targets and members already read into `nodes`.
fn typed(one: &di::Type, nodes: &IndexMap<MetadataId, TypeId>) -> Result<Type, String> {
    let node = |id: MetadataId| nodes.get(&id).copied().ok_or_else(|| format!("debug type !{} listed after its use", id.0));
    let target = || one.target.map(node).ok_or_else(|| format!("a {} of nothing", one.kind.value()))?;
    Ok(match one.kind {
        di::Kind::Scalar => Type::Scalar(scalar(di::Scalar::from_value(&one.name).ok_or_else(|| format!("no debug scalar {}", one.name))?)),
        di::Kind::FixedString => Type::FixedString(narrow(one.size, "a STRING's length")?),
        di::Kind::Array => Type::Array(target()?),
        di::Kind::Sized => Type::Sized { element: target()?, bytes: narrow(one.size, "an array's size")? },
        di::Kind::Pointer => Type::Pointer { target: target()?, reach: reach(one.reach) },
        di::Kind::Reference => Type::Reference(target()?),
        di::Kind::Struct => {
            let fields = one
                .members
                .iter()
                .map(|member| Ok(cvwrite::Field { name: member.name.clone(), r#type: node(member.r#type)?, offset: narrow(member.offset, "a field's offset")? }))
                .collect::<Result<Vec<_>, String>>()?;
            Type::Struct { name: one.name.clone(), bytes: narrow(one.size, "a structure's size")?, fields }
        }
        di::Kind::Procedure => Type::Procedure {
            result: one.target.map(node).transpose()?,
            parameters: one.members.iter().map(|member| node(member.r#type)).collect::<Result<Vec<_>, String>>()?,
        },
    })
}

/// `module`'s debug information, each global and function by the symbol
/// `names` gives it; None without any.
pub fn described(module: &llrm_mir::Module, names: &IndexMap<(Space, i64), String>, flavor: Flavor) -> Result<Option<Debug>, String> {
    let functions = di::functions(module);
    let globals = di::globals(module);
    if functions.is_empty() && globals.is_empty() {
        return Ok(None);
    }
    let symbol = |name: &str| module.named(name).and_then(|id| names.get(&(globals::space(module, id), i64::from(id.0)))).cloned();
    let (mut types, mut nodes) = (Vec::new(), IndexMap::default());
    for id in di::types(module) {
        let one = di::read_type(module, id).ok_or_else(|| format!("debug type !{} does not read", id.0))?;
        types.push(typed(&one, &nodes)?);
        nodes.insert(id, types.len() - 1);
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
        let data = cvwrite::Data { name: global.name, r#type: node(global.r#type)?, symbol: at, displacement: narrow(global.offset, "a variable's offset")? };
        out.push(Global { data, scope: global.scope.as_deref().and_then(&symbol) });
    }
    Ok(Some(Debug { flavor, types, nodes, procedures, globals: out }))
}

/// `module`'s $$SYMBOLS and $$TYPES, the object `source`'s, its code laid
/// out in `code` at `symbols`; only what a symbol still defines is named.
pub fn segments(debug: &Debug, module: &masm::Module, source: &str, code: &Segment, symbols: &IndexMap<String, (usize, usize)>) -> Result<[Segment; 2], String> {
    let defined = |name: &str| symbols.contains_key(name);
    let type_of = |node: MetadataId| debug.nodes.get(&node).copied().ok_or_else(|| format!("debug type !{} unread", node.0));
    let offset = |at: usize, what: &str| narrow::<u16>(at as i64, what);
    let mut starts: Vec<usize> = module.procedures.iter().filter_map(|one| symbols.get(&one.name).map(|&(_, at)| at)).collect();
    starts.sort_unstable();
    // QB 4.5's module record is nameless.
    let name = (!debug.flavor.qb45).then(|| source.to_owned());
    let mut written = cvwrite::Module { name, types: debug.types.clone(), ..cvwrite::Module::default() };
    written.data.extend(debug.globals.iter().filter(|one| one.scope.is_none() && defined(&one.data.symbol)).map(|one| one.data.clone()));
    for procedure in &module.procedures {
        let (Some(described), Some(&(_, start))) = (debug.procedures.get(&procedure.name), symbols.get(&procedure.name)) else { continue };
        let end = starts.iter().copied().find(|&one| one > start).unwrap_or(code.image.len());
        let mut locals = Vec::new();
        let mut statics: Vec<cvwrite::Data> =
            debug.globals.iter().filter(|one| one.scope.as_ref() == Some(&procedure.name) && defined(&one.data.symbol)).map(|one| one.data.clone()).collect();
        for variable in &procedure.body.variables {
            let r#type = type_of(variable.r#type)?;
            match variable.addr.space {
                Space::Frame => locals.push(cvwrite::Local { name: variable.name.clone(), r#type, bp: narrow(variable.addr.disp, "a frame offset")? }),
                space => {
                    let Some(symbol) = module.names.get(&(space, variable.addr.index)).filter(|one| defined(one)) else { continue };
                    let displacement = narrow(variable.addr.disp, "a variable's offset")?;
                    statics.push(cvwrite::Data { name: variable.name.clone(), r#type, symbol: symbol.clone(), displacement });
                }
            }
        }
        if described.module {
            written.data.extend(statics);
            continue;
        }
        // A body starts before its end, and ends after its start.
        let bound = |mark: masm::Mark, within: std::ops::Range<usize>| code.bodies.iter().find(|&&(one, at)| one == mark && within.contains(&at)).map(|&(_, at)| at - start);
        written.procedures.push(cvwrite::Procedure {
            name: described.name.clone(),
            symbol: procedure.name.clone(),
            r#type: described.r#type,
            length: offset(end - start, "a procedure's length")?,
            debug_start: offset(bound(masm::Mark::BodyStart, start..end).unwrap_or(0), "a body's start")?,
            debug_end: offset(bound(masm::Mark::BodyEnd, start + 1..end + 1).unwrap_or(end - start), "a body's end")?,
            far: procedure.far,
            locals,
            statics,
        });
    }
    let first = starts.first().copied().unwrap_or(0);
    written.start = module.procedures.iter().find(|one| symbols.get(&one.name).is_some_and(|&(_, at)| at == first)).map(|one| one.name.clone()).unwrap_or_default();
    written.length = offset(code.image.len() - first, "the module's code length")?;
    let encoded = cvwrite::written(&written, debug.flavor)?;
    let mut bytes = encoded.symbols;
    for one in &encoded.relocations {
        // The displacement is the field's own value, as in code.
        bytes[one.at..one.at + 2].copy_from_slice(&one.displacement.to_le_bytes());
    }
    let fixups: Vec<Fixup> = encoded.relocations.iter().map(|one| Fixup::new(one.at, if one.far { POINTER } else { OFFSET }, one.symbol.clone())).collect();
    let mut symbols_segment = Segment::new("$$SYMBOLS", "DEBSYM", false);
    symbols_segment.put(&bytes, &fixups);
    let mut types_segment = Segment::new("$$TYPES", "DEBTYP", false);
    types_segment.put(&encoded.types, &[]);
    Ok([symbols_segment, types_segment])
}
