//! DGROUP carved into one global per variable. A segment is cut at every
//! address something names -- an operand's landmark, a CodeView variable, a
//! relocation's target -- and a cut that an access, a long's pair of words
//! or a relocated field crosses is dropped, so that span stays one object.
//! A COMMON segment's layout is shared with other modules, so it stays whole.
//!
//! What it is carved by is `Carving`, one value: the segment layout's facts
//! and the addresses the code names, gathered from one object.

use std::collections::{BTreeMap, BTreeSet};

use llrm_bcmachine::model::ir::Loc;
use llrm_bcmachine::model::ir::nodes::Node;
use llrm_bcmachine::objectfile::module::{self, Space};
use llrm_bcmachine::objectfile::{cvinfo, omf};
use llrm_mir::{CastOp, Constant, ConstantExpr, ConstantId, ConstantKind, GlobalId, GlobalVariable, Linkage, Module, Type, TypeId};

use llrm_mir::program::SegmentLayout;

use crate::machine::Facts;

/// What DGROUP is carved by: the program's segment layout -- DGROUP's
/// segments, the COMMON ones other modules share, the code segment, whether
/// SS is DS -- and what the code names in it: the cuts (landmarks and
/// CodeView variables, those with a name) and the spans no cut may cross
/// (static accesses, a long's two words, a CodeView variable's width, a
/// DIM request's descriptor). The object's relocations cut too; they are
/// its own.
#[derive(Clone, Debug, Default)]
pub struct Carving {
    pub dgroup: BTreeSet<i64>,
    pub shared: BTreeSet<i64>,
    pub code: Option<i64>,
    /// Whether code reads CS or an address in the code segment.
    pub code_named: bool,
    pub stack_in_data: bool,
    pub cuts: BTreeMap<i64, BTreeMap<i64, Option<String>>>,
    pub spans: BTreeMap<i64, Vec<(i64, i64)>>,
    /// The displacements a register indexes from: an array's origin.
    pub indexed: BTreeMap<i64, BTreeSet<i64>>,
}

impl Carving {
    /// One object's, in the program whose segments `segments` lays out: a
    /// segment is DGROUP's, or COMMON, as the program links it, by name.
    pub fn of(facts: &Facts, segments: &SegmentLayout) -> Carving {
        let found = facts.found;
        let code = omf::code_segment(&found.records).map(|(index, _, _)| index);
        let named = omf::segments(&found.records);
        let indexes = |names: &dyn Fn(&str) -> bool| -> BTreeSet<i64> {
            named.iter().enumerate().filter(|(_, one)| one.as_ref().is_some_and(|(name, _)| names(name))).map(|(at, _)| at as i64).collect()
        };
        let mut carving = Carving {
            dgroup: indexes(&|name| segments.data_group.members.iter().any(|one| one == name)),
            shared: indexes(&|name| segments.data_group.common.contains(name)),
            code,
            code_named: code.is_some_and(|code| names_code(facts, code)),
            stack_in_data: segments.stack_in_data,
            ..Carving::default()
        };
        for ((space, segment), offsets) in module::landmarks(found) {
            if space == Space::Segment {
                carving.cuts.entry(segment).or_default().extend(offsets.into_iter().map(|one| (one, None)));
            }
        }
        for variable in cvinfo::parse(&found.records).variables {
            if let Some(width) = variable.type_name().as_deref().and_then(width_of) {
                carving.spans.entry(variable.segment).or_default().push((variable.offset, variable.offset + width));
            }
            let named = carving.cuts.entry(variable.segment).or_default().entry(variable.offset).or_default();
            named.get_or_insert(variable.name);
        }
        for body in &facts.bodies {
            for node in body.nodes.values() {
                accesses(node, &mut carving.spans, &mut carving.indexed);
            }
            // A long's two words are one access.
            for (segment, disp) in body.pairs.values().filter_map(|pair| pair.span()) {
                carving.spans.entry(segment).or_default().push((disp, disp + 4));
            }
        }
        // A DIM request's descriptor is one object.
        for one in crate::arrays::requests(facts) {
            carving.spans.entry(one.segment).or_default().push((one.start, one.end()));
        }
        carving
    }
}

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
    /// The address spaces of the target the objects are recompiled for.
    spaces: llrm_mir::spaces::Spaces,
    /// Each DGROUP segment's objects, by start.
    segments: BTreeMap<i64, BTreeMap<i64, Object>>,
    /// Each EXTDEF's global, by its index.
    externals: BTreeMap<i64, ConstantId>,
    /// The base of each segment outside DGROUP that data points into, as
    /// a far pointer: the code segment the recompiled module still has.
    bases: BTreeMap<i64, (GlobalId, ConstantId)>,
    /// The segments outside DGROUP that hold data: each is one object.
    far: BTreeSet<i64>,
    /// The code segment's index.
    code: Option<i64>,
    dgroup: BTreeSet<i64>,
    stack_in_data: bool,
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

    /// The DGROUP segment and object `global` is.
    pub fn placed(&self, global: GlobalId) -> Option<(i64, &Object)> {
        self.segments
            .iter()
            .filter(|(segment, _)| !self.far.contains(segment))
            .flat_map(|(&segment, objects)| objects.values().map(move |one| (segment, one)))
            .find(|(_, one)| one.global == global)
    }

    /// Whether `segment` is one of DGROUP's.
    pub fn in_dgroup(&self, segment: i64) -> bool {
        self.dgroup.contains(&segment)
    }

    /// Whether segment register `register` names DGROUP.
    pub fn names_data(&self, register: iced_x86::Register) -> bool {
        register == iced_x86::Register::DS || register == iced_x86::Register::SS && self.stack_in_data
    }

    pub fn external(&self, index: i64) -> Option<ConstantId> {
        self.externals.get(&index).copied()
    }

    /// A near code offset held as a value, which is its original offset.
    /// What holds one is a DATA row's key and the RESTORE that searches for
    /// it (B$RSTB), or a handler's address; a handler is entered by the
    /// runtime's protocol, which refuses its module, so what is left is a
    /// key, and the original offset keeps both sides equal.
    pub fn key(&self, segment: i64, disp: i64) -> Option<i64> {
        (Some(segment) == self.code).then_some(disp)
    }

    pub fn base(&self, segment: i64) -> Option<ConstantId> {
        self.bases.get(&segment).map(|&(_, reference)| reference)
    }

    /// Where each object sits: its segment, first byte and global, in
    /// segment and address order; and the global each segment the objects
    /// point into but the raise does not carve is.
    pub fn placement(&self) -> crate::Placement {
        let objects = self.segments.iter().flat_map(|(&segment, objects)| objects.values().map(move |one| (segment, one.start, one.global))).collect();
        let bases = self.bases.iter().map(|(&segment, &(global, _))| (segment, global)).collect();
        crate::Placement { objects, bases }
    }

    /// The code segment's base, as a far pointer.
    pub fn code(&self) -> Option<ConstantId> {
        self.code.and_then(|one| self.base(one))
    }

    /// Byte `disp` of a segment outside DGROUP, as a far pointer.
    pub fn far_address(&self, context: &mut llrm_mir::Context, segment: i64, disp: i64) -> Option<ConstantId> {
        if self.far.contains(&segment) {
            let object = self.at(segment, disp)?;
            return Some(offset_constant(context, object.reference, disp - object.start));
        }
        self.base(segment).map(|base| offset_constant(context, base, disp))
    }

    /// Any DGROUP object: what DS names.
    pub fn any(&self) -> Option<&Object> {
        self.segments.iter().filter(|(segment, _)| !self.far.contains(segment)).flat_map(|(_, objects)| objects.values()).next()
    }

    pub fn build(carving: &Carving, found: &module::Module, module: &mut Module, spaces: llrm_mir::spaces::Spaces) -> Result<Objects, String> {
        let records = &found.records;
        let segments = omf::segments(records);
        let externals = omf::externals(records);
        let mut objects = Objects { dgroup: carving.dgroup.clone(), stack_in_data: carving.stack_in_data, spaces, ..Objects::default() };
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
        let code = carving.code;
        let mut data: BTreeSet<i64> = carving.dgroup.clone();
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
        objects.far = data.difference(&carving.dgroup).copied().collect();
        let relocations = relocations(records, &data);
        objects.code = code;
        // The code segment is emitted again, under its own name.
        if let Some(code) = code.filter(|&code| relocations.values().flatten().any(|one| one.target == "segment" && one.index == code) || carving.code_named) {
            let Some(Some((name, _))) = segments.get(code as usize) else { return Err("no code segment".to_owned()) };
            let byte = module.context.types.int(8);
            let variable = GlobalVariable { ty: byte, constant: true, initializer: None, align: None };
            let global = add_unique(module, name, |module, named| module.add_variable(named, variable.clone(), Linkage::External));
            module.globals[global.0 as usize].address_space = spaces.far;
            objects.bases.insert(code, (global, module.reference(global)));
        }
        let mut carved: Vec<(i64, i64, i64, Option<String>)> = Vec::new();
        for &segment in &data {
            let Some(Some((segname, size))) = segments.get(segment as usize) else { continue };
            let size = *size;
            if carving.shared.contains(&segment) || objects.far.contains(&segment) {
                carved.push((segment, 0, size, Some(segname.clone())));
                continue;
            }
            let named = carving.cuts.get(&segment);
            let mut cuts: BTreeSet<i64> = BTreeSet::from([0, size]);
            cuts.extend(named.into_iter().flat_map(BTreeMap::keys).copied());
            let names: BTreeMap<i64, String> = named.into_iter().flatten().filter_map(|(&at, name)| Some((at, name.clone()?))).collect();
            for one in relocations.values().flatten() {
                if one.target == "segment" && one.index == segment {
                    cuts.insert(one.disp);
                }
            }
            let mut crossing: Vec<(i64, i64)> = carving.spans.get(&segment).cloned().unwrap_or_default();
            let origins = carving.indexed.get(&segment).into_iter().flatten().filter(|&&one| (0..size).contains(&one));
            crossing.extend(origins.map(|&one| indexed_reach(one, &names, size)));
            crossing.extend(relocations.get(&segment).into_iter().flatten().map(|one| (one.at, one.at + one.width)));
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
            let shared = carving.shared.contains(segment);
            let variable = GlobalVariable { ty, constant: false, initializer: None, align: None };
            let linkage = if shared { Linkage::Common } else { Linkage::Internal };
            let named = name.clone().unwrap_or_default();
            let global = add_unique(module, &named, |module, one| module.add_variable(one, variable.clone(), linkage));
            if objects.far.contains(segment) {
                module.globals[global.0 as usize].address_space = spaces.far;
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
                if carving.shared.contains(&segment) && !matches!(module.context.get(initializer).kind, ConstantKind::Zero) {
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
            if let Some(key) = (relocation.loc == omf::LOC_OFF16 && relocation.target == "segment").then(|| self.key(relocation.index, relocation.disp + addend)).flatten() {
                let word = context.types.int(16);
                members.push(context.constant(Constant { ty: word, kind: ConstantKind::Int(key as u16 as u128) }));
                at = relocation.at + relocation.width;
                continue;
            }
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
            let far = context.types.ptr(self.spaces.far);
            members.push(match relocation.loc {
                // A near offset into a far segment is its far pointer as `i16`.
                omf::LOC_OFF16 if context.get(target).ty == far => {
                    let word = context.types.int(16);
                    context.constant(Constant { ty: word, kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt, value: target }) })
                }
                omf::LOC_OFF16 => target,
                omf::LOC_PTR32 => cast(context, CastOp::AddrSpaceCast, target, self.spaces.far),
                omf::LOC_BASE => {
                    let far = cast(context, CastOp::AddrSpaceCast, target, self.spaces.far);
                    cast(context, CastOp::AddrSpaceCast, far, crate::segment(&self.spaces))
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

/// Whether any code reads CS or an address in the code segment.
fn names_code(facts: &Facts, code: i64) -> bool {
    facts.bodies.iter().flat_map(|body| body.nodes.values()).any(|node| {
        let semantics = node.semantics();
        semantics.sources.iter().chain(&semantics.dests).any(|one| match one {
            Loc::Reg(reg) => reg.register == iced_x86::Register::CS,
            Loc::Imm(imm) => imm.address.is_some_and(|address| address.space == Space::Segment && address.index == code),
            _ => false,
        })
    })
}

/// Every static access `node` makes to a segment, as `[disp, disp + width)`,
/// and the displacement of every indexed one.
fn accesses(node: &Node, spans: &mut BTreeMap<i64, Vec<(i64, i64)>>, indexed: &mut BTreeMap<i64, BTreeSet<i64>>) {
    let effects = node.effects();
    for cell in effects.loads.iter().chain(&effects.stores) {
        let Some(addr) = cell.addr else { continue };
        if addr.space != Space::Segment || cell.width <= 0 {
            continue;
        }
        if addr.base == iced_x86::Register::None {
            spans.entry(addr.index).or_default().push((addr.disp, addr.disp + i64::from(cell.width)));
        } else {
            indexed.entry(addr.index).or_default().insert(addr.disp);
        }
    }
}

/// What an index from `origin` may reach, a subscript in range and its
/// lower bound not negative: from the origin through the array the next
/// name starts, or itself names, to the name after it; with no names, the
/// segment's end. A landmark cannot end it: the array's own elements are
/// landmarks, and BASE 1's origin is before its first.
fn indexed_reach(origin: i64, names: &BTreeMap<i64, String>, size: i64) -> (i64, i64) {
    let array = names.range(origin..).next().map(|(&at, _)| at);
    let end = array.and_then(|at| names.range(at + 1..).next().map(|(&after, _)| after));
    (origin, end.unwrap_or(size))
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
