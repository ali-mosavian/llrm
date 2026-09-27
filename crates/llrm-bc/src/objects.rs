//! DGROUP carved into one global per variable. A segment is cut at every
//! address something names -- an operand's landmark, a CodeView variable, a
//! relocation's target -- and a cut that an access or a relocated field
//! crosses is dropped, so that span stays one object. A COMMON segment's
//! layout is shared with other modules, so it stays whole.

use std::collections::{BTreeMap, BTreeSet};

use llrm_bcmachine::model::ir::nodes::Node;
use llrm_bcmachine::objectfile::module::{self, Space};
use llrm_bcmachine::objectfile::{cvinfo, omf};
use llrm_mir::{CastOp, Constant, ConstantExpr, ConstantId, ConstantKind, GlobalId, GlobalVariable, Linkage, Module, Type, TypeId};

use crate::machine::Facts;
use crate::{FAR, SEGMENT};

/// One carved object: the bytes `[start, end)` of a segment.
#[derive(Clone, Debug)]
pub struct Object {
    pub start: i64,
    pub end: i64,
    pub global: GlobalId,
    /// A pointer to it.
    pub reference: ConstantId,
}

#[derive(Clone, Debug, Default)]
pub struct Objects {
    /// Each DGROUP segment's objects, by start.
    segments: BTreeMap<i64, BTreeMap<i64, Object>>,
    /// Each EXTDEF's global, by its index.
    externals: BTreeMap<i64, ConstantId>,
    /// The base of each segment outside DGROUP that data points into, as
    /// a far pointer: the code segment the recompiled module still has.
    bases: BTreeMap<i64, ConstantId>,
    /// The segments outside DGROUP that hold data: each is one object.
    far: BTreeSet<i64>,
}

/// A relocated field in a data segment.
struct Relocation {
    at: i64,
    width: i64,
    loc: i64,
    target: String,
    index: i64,
    disp: i64,
}

impl Objects {
    /// The object holding byte `disp` of DGROUP segment `segment`, or the
    /// last one where `disp` is its end.
    pub fn at(&self, segment: i64, disp: i64) -> Option<&Object> {
        let objects = self.segments.get(&segment)?;
        let (_, found) = objects.range(..=disp).next_back()?;
        (disp < found.end || objects.range(disp..).next().is_none() && disp == found.end).then_some(found)
    }

    pub fn external(&self, index: i64) -> Option<ConstantId> {
        self.externals.get(&index).copied()
    }

    pub fn base(&self, segment: i64) -> Option<ConstantId> {
        self.bases.get(&segment).copied()
    }

    /// Any DGROUP object: what DS names.
    pub fn any(&self) -> Option<&Object> {
        self.segments.iter().filter(|(segment, _)| !self.far.contains(segment)).flat_map(|(_, objects)| objects.values()).next()
    }

    pub fn build(facts: &Facts, module: &mut Module) -> Result<Objects, String> {
        let found = facts.found;
        let records = &found.records;
        let segments = omf::segments(records);
        let externals = omf::externals(records);
        let mut objects = Objects::default();
        for (index, name) in externals.iter().enumerate().skip(1) {
            // BC calls a procedure of its own through an EXTDEF of its name.
            if let Some(defined) = module.named(name) {
                objects.externals.insert(index as i64, module.reference(defined));
                continue;
            }
            let byte = module.context.types.int(8);
            let variable = GlobalVariable { ty: byte, constant: false, initializer: None, align: None };
            let global = add_unique(module, name, |module, one| module.add_variable(one, variable.clone(), Linkage::External));
            objects.externals.insert(index as i64, module.reference(global));
        }
        // DGROUP, and every far segment its data points into, transitively.
        let code = omf::code_segment(records).map(|(index, _, _)| index);
        let mut data: BTreeSet<i64> = found.dgroup.members.clone();
        loop {
            let reached: BTreeSet<i64> = relocations(records, &data)
                .values()
                .flatten()
                .filter(|one| one.target == "segment" && Some(one.index) != code && !data.contains(&one.index))
                .map(|one| one.index)
                .collect();
            if reached.is_empty() {
                break;
            }
            data.extend(reached);
        }
        objects.far = data.difference(&found.dgroup.members).copied().collect();
        let relocations = relocations(records, &data);
        // The code segment is emitted again, under its own name.
        if let Some(code) = code.filter(|&code| relocations.values().flatten().any(|one| one.target == "segment" && one.index == code)) {
            let Some(Some((name, _))) = segments.get(code as usize) else { return Err("no code segment".to_owned()) };
            let byte = module.context.types.int(8);
            let variable = GlobalVariable { ty: byte, constant: true, initializer: None, align: None };
            let global = add_unique(module, name, |module, named| module.add_variable(named, variable.clone(), Linkage::External));
            module.globals[global.0 as usize].address_space = FAR;
            objects.bases.insert(code, module.reference(global));
        }
        let info = cvinfo::parse(records);
        let landmarks = module::landmarks(found);
        let mut spans: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
        for body in &facts.bodies {
            for node in body.nodes.values() {
                accesses(node, &mut spans);
            }
        }
        let mut carved: Vec<(i64, i64, i64, Option<String>)> = Vec::new();
        for &segment in &data {
            let Some(Some((segname, size))) = segments.get(segment as usize) else { continue };
            let size = *size;
            if found.dgroup.shared.contains(&segment) || objects.far.contains(&segment) {
                carved.push((segment, 0, size, Some(segname.clone())));
                continue;
            }
            let mut cuts: BTreeSet<i64> = BTreeSet::from([0, size]);
            let mut names: BTreeMap<i64, String> = BTreeMap::new();
            cuts.extend(landmarks.get(&(Space::Segment, segment)).into_iter().flatten().copied());
            for variable in info.variables.iter().filter(|one| one.segment == segment) {
                cuts.insert(variable.offset);
                names.entry(variable.offset).or_insert_with(|| variable.name.clone());
            }
            for one in relocations.values().flatten() {
                if one.target == "segment" && one.index == segment {
                    cuts.insert(one.disp);
                }
            }
            let mut crossing: Vec<(i64, i64)> = spans.get(&segment).cloned().unwrap_or_default();
            crossing.extend(relocations.get(&segment).into_iter().flatten().map(|one| (one.at, one.at + one.width)));
            for variable in info.variables.iter().filter(|one| one.segment == segment) {
                if let Some(width) = variable.type_name().as_deref().and_then(width_of) {
                    crossing.push((variable.offset, variable.offset + width));
                }
            }
            for (low, high) in crossing {
                let inside: Vec<i64> = cuts.range(low + 1..high).copied().collect();
                for one in inside {
                    cuts.remove(&one);
                }
            }
            let cuts: Vec<i64> = cuts.into_iter().filter(|&one| (0..=size).contains(&one)).collect();
            for pair in cuts.windows(2) {
                let name = names.get(&pair[0]).cloned().unwrap_or_else(|| format!("{segname}.{:04x}", pair[0]));
                carved.push((segment, pair[0], pair[1], Some(name)));
            }
        }
        for (segment, start, end, name) in &carved {
            let byte = module.context.types.int(8);
            let ty = module.context.types.intern(Type::Array { element: byte, count: (end - start) as u64 });
            let shared = found.dgroup.shared.contains(segment);
            let variable = GlobalVariable { ty, constant: false, initializer: None, align: None };
            let linkage = if shared { Linkage::Common } else { Linkage::Internal };
            let named = name.clone().unwrap_or_default();
            let global = add_unique(module, &named, |module, one| module.add_variable(one, variable.clone(), linkage));
            if objects.far.contains(segment) {
                module.globals[global.0 as usize].address_space = FAR;
            }
            let reference = module.reference(global);
            objects.segments.entry(*segment).or_default().insert(*start, Object { start: *start, end: *end, global, reference });
        }
        for &segment in &data {
            let Some(Some((_, size))) = segments.get(segment as usize) else { continue };
            let image = omf::segment_image(records, segment, *size);
            let mine = relocations.get(&segment).map(Vec::as_slice).unwrap_or_default();
            for object in objects.segments.get(&segment).cloned().unwrap_or_default().values() {
                let inside: Vec<&Relocation> = mine.iter().filter(|one| object.start <= one.at && one.at < object.end).collect();
                let (ty, initializer) = objects.initializer(module, object, &image, &inside)?;
                if found.dgroup.shared.contains(&segment) && !matches!(module.context.get(initializer).kind, ConstantKind::Zero) {
                    return Err("a COMMON block with initial data".to_owned());
                }
                let llrm_mir::GlobalKind::Variable(variable) = &mut module.globals[object.global.0 as usize].kind else { unreachable!("a variable") };
                variable.ty = ty;
                variable.initializer = Some(initializer);
            }
        }
        Ok(objects)
    }

    /// An object's type and initializer: its bytes, a relocated field as the
    /// pointer it holds.
    fn initializer(&self, module: &mut Module, object: &Object, image: &[u8], relocations: &[&Relocation]) -> Result<(TypeId, ConstantId), String> {
        let context = &mut module.context;
        let byte = context.types.int(8);
        let bytes = |context: &mut llrm_mir::Context, from: i64, to: i64| {
            let slice: Vec<u8> = (from..to).map(|at| image.get(at as usize).copied().unwrap_or(0)).collect();
            let ty = context.types.intern(Type::Array { element: byte, count: slice.len() as u64 });
            let kind = if slice.iter().all(|&one| one == 0) { ConstantKind::Zero } else { ConstantKind::Bytes(slice) };
            context.constant(Constant { ty, kind })
        };
        let mut members = Vec::new();
        let mut at = object.start;
        let mut sorted: Vec<&&Relocation> = relocations.iter().collect();
        sorted.sort_by_key(|one| one.at);
        for relocation in sorted {
            if relocation.at + relocation.width > object.end {
                return Err(format!("a relocated field crossing the end of object {:#x}", object.start));
            }
            if relocation.at > at {
                members.push(bytes(context, at, relocation.at));
            }
            let addend = i64::from(u16::from_le_bytes([image.get(relocation.at as usize).copied().unwrap_or(0), image.get(relocation.at as usize + 1).copied().unwrap_or(0)]));
            let target = match relocation.target.as_str() {
                "segment" => match (self.at(relocation.index, relocation.disp), self.base(relocation.index)) {
                    (Some(target), _) => offset_constant(context, target.reference, relocation.disp - target.start + addend),
                    (None, Some(base)) => offset_constant(context, base, relocation.disp + addend),
                    _ => return Err(format!("a relocation past the end of segment {}", relocation.index)),
                },
                "external" => {
                    let target = self.external(relocation.index).ok_or("a relocation to an unknown external")?;
                    offset_constant(context, target, relocation.disp + addend)
                }
                other => return Err(format!("a data relocation to a {other}")),
            };
            let far = context.types.ptr(FAR);
            members.push(match relocation.loc {
                // A near offset into a far segment is its far pointer's low word.
                omf::LOC_OFF16 if context.get(target).ty == far => {
                    let (long, word) = (context.types.int(32), context.types.int(16));
                    let whole = context.constant(Constant { ty: long, kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt, value: target }) });
                    context.constant(Constant { ty: word, kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::Trunc, value: whole }) })
                }
                omf::LOC_OFF16 => target,
                omf::LOC_PTR32 => cast(context, CastOp::AddrSpaceCast, target, FAR),
                omf::LOC_BASE => {
                    let far = cast(context, CastOp::AddrSpaceCast, target, FAR);
                    cast(context, CastOp::AddrSpaceCast, far, SEGMENT)
                }
                other => return Err(format!("a data relocation of kind {other}")),
            });
            at = relocation.at + relocation.width;
        }
        if members.is_empty() || object.end > at {
            members.push(bytes(context, at, object.end));
        }
        if members.len() == 1 {
            return Ok((context.get(members[0]).ty, members[0]));
        }
        let fields = members.iter().map(|&one| context.get(one).ty).collect();
        let ty = context.types.intern(Type::Struct { fields, packed: true });
        Ok((ty, context.constant(Constant { ty, kind: ConstantKind::Aggregate(members) })))
    }
}

/// `pointer` advanced by `offset` bytes, as a constant.
fn offset_constant(context: &mut llrm_mir::Context, pointer: ConstantId, offset: i64) -> ConstantId {
    if offset == 0 {
        return pointer;
    }
    let (byte, word) = (context.types.int(8), context.types.int(16));
    let index = context.int(word, i128::from(offset));
    let ty = context.get(pointer).ty;
    context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::GetElementPtr { source: byte, inbounds: false, operands: vec![pointer, index] }) })
}

fn cast(context: &mut llrm_mir::Context, op: CastOp, value: ConstantId, space: u32) -> ConstantId {
    let ty = context.types.ptr(space);
    if context.get(value).ty == ty {
        return value;
    }
    context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::Cast { op, value }) })
}

/// The bytes a CodeView primitive occupies.
fn width_of(name: &str) -> Option<i64> {
    Some(match name {
        "INTEGER" => 2,
        "LONG" | "SINGLE" | "STRING" => 4,
        "DOUBLE" | "CURRENCY" => 8,
        _ => name.strip_prefix("STRING * ")?.parse().ok()?,
    })
}

/// Every static access `node` makes to a segment, as `[disp, disp + width)`.
fn accesses(node: &Node, spans: &mut BTreeMap<i64, Vec<(i64, i64)>>) {
    let effects = node.effects();
    for cell in effects.loads.iter().chain(&effects.stores) {
        let Some(addr) = cell.addr else { continue };
        if addr.space == Space::Segment && addr.base == iced_x86::Register::None && cell.width > 0 {
            spans.entry(addr.index).or_default().push((addr.disp, addr.disp + i64::from(cell.width)));
        }
    }
}

/// The relocated fields of each DGROUP segment.
fn relocations(records: &[std::rc::Rc<omf::Record>], dgroup: &BTreeSet<i64>) -> BTreeMap<i64, Vec<Relocation>> {
    let mut out: BTreeMap<i64, Vec<Relocation>> = BTreeMap::new();
    for fixup in omf::fixups(records) {
        let Some(segment) = fixup.seg.filter(|one| dgroup.contains(one)) else { continue };
        let width = match fixup.loc {
            omf::LOC_PTR32 => 4,
            omf::LOC_LOBYTE | omf::LOC_HIBYTE => 1,
            _ => 2,
        };
        out.entry(segment).or_default().push(Relocation { at: fixup.offset, width, loc: fixup.loc, target: fixup.target.clone(), index: fixup.index, disp: fixup.disp });
    }
    out
}

/// `name`, or `name.N` for the least N free, as LLVM uniques names.
pub fn add_unique(module: &mut Module, name: &str, mut add: impl FnMut(&mut Module, &str) -> Result<GlobalId, String>) -> GlobalId {
    std::iter::once(name.to_owned()).chain((1..).map(|n| format!("{name}.{n}"))).find_map(|one| add(module, &one).ok()).expect("some name is free")
}
