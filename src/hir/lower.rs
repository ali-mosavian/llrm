//! Port of `qbopt/hir/lower.py`: lower verified common HIR into the existing
//! MIR, without a p-code tier.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use num_traits::ToPrimitive;

use crate::abi::machine;
use crate::hir::escape;
use crate::hir::model;
use crate::hir::verify::{InvalidHIR, verify};
use crate::model::floating;
use crate::model::ir::Operation;
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance, Slice};
use crate::model::mir::{self, Arg, Cell, Const, Held, MemRef, OpCode};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr;

/// The callee name of an inline block's call: it names no routine.
pub const ASM: &str = "$asm";

#[allow(non_snake_case)]
fn _KINDS(value: &str) -> mir::Kind {
    *mir::Kind::ALL.iter().find(|one| one.as_str() == value).expect("a HIR op names a MIR kind")
}

#[allow(non_snake_case)]
fn _FORMATS(evaluation: model::FloatEvaluation) -> floating::Format {
    match evaluation {
        model::FloatEvaluation::Binary32 => floating::Format::Binary32,
        model::FloatEvaluation::Binary64 => floating::Format::Binary64,
        model::FloatEvaluation::Extended80 => floating::Format::Extended80,
        model::FloatEvaluation::None => panic!("KeyError: <FloatEvaluation.NONE: 'none'>"),
    }
}

const _BINARY_FLOAT: [model::Op; 4] = [model::Op::Fadd, model::Op::Fsub, model::Op::Fmul, model::Op::Fdiv];
const _UNARY_FLOAT: [model::Op; 8] = [
    model::Op::Fneg,
    model::Op::Fabs,
    model::Op::Fsqrt,
    model::Op::Fsin,
    model::Op::Fcos,
    model::Op::Fatan,
    model::Op::Flog2,
    model::Op::Fexp2,
];

#[allow(non_snake_case)]
fn _X87_INTRINSICS(op: model::Op) -> Option<&'static str> {
    match op {
        model::Op::Fsin => Some("fsin"),
        model::Op::Fcos => Some("fcos"),
        model::Op::Fatan => Some("fatan"),
        model::Op::Flog2 => Some("flog2"),
        model::Op::Fexp2 => Some("fexp2"),
        _ => None,
    }
}

fn _stored_format(type_: &model::Type) -> Result<floating::Format, InvalidHIR> {
    if type_.kind == model::TypeKind::Float {
        return Ok(if type_.width == 4 { floating::Format::Binary32 } else { floating::Format::Binary64 });
    }
    if matches!(type_.kind, model::TypeKind::Integer | model::TypeKind::Boolean) {
        match type_.width {
            2 => return Ok(floating::Format::Signed16),
            4 => return Ok(floating::Format::Signed32),
            _ => {}
        }
    }
    Err(InvalidHIR(format!("{}: no floating storage format", type_.name)))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lowered {
    pub name: String,
    pub body: mir::MirBody,
    pub values: IndexMap<i64, mir::Value>,
    pub externals: Option<IndexMap<i64, String>>,
    // Stable handoff from a source HIR instruction to the unique MIR address
    // of the operation it became. HIR ids are per-function source identities;
    // MIR operation ids also include terminators and can numerically collide.
    pub source_instructions: Option<IndexMap<i64, i64>>,
}

fn _space(place: &model::Place) -> Space {
    if matches!(place.storage, model::Storage::Local | model::Storage::Parameter) {
        return Space::Frame;
    }
    if place.storage == model::Storage::External {
        return Space::External;
    }
    Space::Segment
}

/// A frame piece and whether any place holding it has its address handed out.
type _Pieces = std::collections::HashMap<i64, Vec<(i64, i64, Identity, bool)>>;

fn _pieces(function: &model::Function) -> _Pieces {
    let pieces = model::frame_pieces(&function.places);
    let exposed = escape::exposed_frame(function);
    let reached: BTreeSet<&Identity> =
        exposed.iter().flat_map(|place| pieces.get(place).into_iter().flatten()).map(|piece| &piece.2).collect();
    pieces
        .iter()
        .map(|(place, spans)| {
            let spans = spans.iter().map(|(low, high, identity)| (*low, *high, identity.clone(), reached.contains(identity)));
            (*place, spans.collect())
        })
        .collect()
}

/// What lowering knows of a module's data symbols: which have their address
/// held elsewhere, and how many bytes each holds.
pub(crate) struct _Symbols {
    escaped: BTreeSet<i64>,
    sizes: IndexMap<i64, i64>,
}

impl _Symbols {
    fn new(module: &model::Module) -> Self {
        let sizes = module
            .data
            .iter()
            .filter(|one| one.linkage != model::DataLinkage::External)
            .map(|one| (one.id, one.bytes.len() as i64))
            .collect();
        Self { escaped: escape::escaped(module), sizes }
    }
}

/// `width` bytes from `place`'s start, in the objects holding them.
fn _provenance(
    place: &model::Place,
    width: i64,
    symbols: &_Symbols,
    pieces: &_Pieces,
) -> Result<Provenance, InvalidHIR> {
    if _space(place) == Space::Frame {
        let (start, end) = (place.offset, place.offset + width);
        let mut slices = BTreeSet::new();
        for (low, high, identity, exposed) in &pieces[&place.id] {
            if *low < end && start < *high {
                let object_ = MemoryObject {
                    identity: Some(identity.clone()),
                    extent: Some(high - low),
                    addressed: *exposed,
                    captured: *exposed,
                    ..MemoryObject::new(MemoryKind::Frame)
                };
                let slice = Slice::new(object_, start.max(*low) - low, end.min(*high) - low, 1, 1)
                    .map_err(|error| InvalidHIR(error.to_string()))?;
                slices.insert(slice);
            }
        }
        return Ok(Provenance { slices, restrict: BTreeSet::new() });
    }
    _one(_global(place, symbols), place.offset, place.offset + width)
}

/// The symbol `place` names part of: one object in every function, whose
/// offsets are the symbol's.
fn _global(place: &model::Place, symbols: &_Symbols) -> MemoryObject {
    let identity = Identity::Int(place.symbol);
    let private = matches!(place.storage, model::Storage::Static | model::Storage::Module | model::Storage::External)
        && !symbols.escaped.contains(&place.symbol);
    MemoryObject {
        identity: Some(identity),
        extent: symbols.sizes.get(&place.symbol).copied(),
        addressed: !private,
        captured: !private,
        ..MemoryObject::new(MemoryKind::Global)
    }
}

/// The objects code outside `module` reaches by name only, with that name:
/// no pointer holds them, yet any callee this module cannot see may read or
/// write them.
pub fn named_externals(module: &model::Module) -> Vec<(String, MemoryObject)> {
    let symbols = _Symbols::new(module);
    let name = |symbol: i64| module.data.iter().find(|one| one.id == symbol).map(|one| one.name.clone());
    module
        .functions
        .iter()
        .flat_map(|function| &function.places)
        .filter(|place| place.storage == model::Storage::External)
        .filter_map(|place| Some((name(place.symbol)?, _global(place, &symbols))))
        .filter(|(_, object_)| !object_.addressed)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// `memory.Provenance.one(object_, low, high)`.
/// The first `width` bytes of `object_`, a frame cell at `offset`.
fn _frame_ref(object_: MemoryObject, offset: i64, width: i64) -> Result<MemRef, InvalidHIR> {
    Ok(MemRef {
        space: Some(Space::Frame),
        provenance: Some(_one(object_, 0, width)?),
        ..MemRef::new(Some(Addr::new(Space::Frame, offset)), width as u32)
    })
}

/// The x87 integer format a conversion to `type_` stores, and its width:
/// the narrowest signed one that holds all of `type_`'s values.
fn _integer_format(type_: &model::Type) -> (floating::Format, i64) {
    match (type_.width, type_.signed == Some(true) || type_.kind == model::TypeKind::Boolean) {
        (1, _) | (2, true) => (floating::Format::Signed16, 2),
        (2, false) | (4, true) => (floating::Format::Signed32, 4),
        _ => (floating::Format::Signed64, 8),
    }
}

fn _one(object_: MemoryObject, low: i64, high: i64) -> Result<Provenance, InvalidHIR> {
    Provenance::one_with_slice(object_, low, high, 1, 1, BTreeSet::new()).map_err(|error| InvalidHIR(error.to_string()))
}

fn _addr(space: Space, disp: i64, index: i64) -> Addr {
    Addr { index, ..Addr::new(space, disp) }
}

fn _ref(
    place: &model::Place,
    type_: &model::Type,
    symbols: &_Symbols,
    pieces: &_Pieces,
) -> Result<MemRef, InvalidHIR> {
    let space = _space(place);
    let index = if space == Space::Frame { 0 } else { place.symbol };
    let provenance = _provenance(place, type_.width, symbols, pieces)?;
    Ok(MemRef {
        space: Some(space),
        provenance: Some(provenance),
        volatile: place.volatile,
        ..MemRef::new(Some(_addr(space, place.offset, index)), type_.width as u32)
    })
}

/// The selector a far address of static data takes: DGROUP's.
pub const DGROUP: (Space, i64) = (Space::Group, 0);

/// The names of the symbols lowering itself introduces, which every
/// object built from lowered MIR must define.
pub fn symbol_names() -> IndexMap<(Space, i64), String> {
    IndexMap::from_iter([(DGROUP, "DGROUP".to_owned())])
}

pub fn lower(program: &model::Program) -> Result<Vec<Lowered>, InvalidHIR> {
    verify(program)?;
    let mut out = Vec::new();
    for module in &program.modules {
        let types: IndexMap<i64, &model::Type> = module.types.iter().map(|one| (one.id, one)).collect();
        let externals: IndexMap<i64, String> = module
            .data
            .iter()
            .filter(|one| one.linkage == model::DataLinkage::External)
            .map(|one| (one.id, one.name.clone()))
            .collect();
        let symbols = _Symbols::new(module);
        for function in &module.functions {
            out.push(_function(
                &module.name,
                &_materialized_booleans(&_taken_branches(function), &types),
                &types,
                &externals,
                program.array_order,
                &symbols,
            )?);
        }
    }
    Ok(out)
}

const _COMPARISONS: [model::Op; 16] = [
    model::Op::Eq,
    model::Op::Ne,
    model::Op::Lt,
    model::Op::Le,
    model::Op::Gt,
    model::Op::Ge,
    model::Op::Below,
    model::Op::BelowEq,
    model::Op::Above,
    model::Op::AboveEq,
    model::Op::StringEq,
    model::Op::StringNe,
    model::Op::StringLt,
    model::Op::StringLe,
    model::Op::StringGt,
    model::Op::StringGe,
];
const _STRING_COMPARISONS: [model::Op; 6] = [
    model::Op::StringEq,
    model::Op::StringNe,
    model::Op::StringLt,
    model::Op::StringLe,
    model::Op::StringGt,
    model::Op::StringGe,
];

/// `referenced`, which `_materialized_booleans` and `_function` each nest.
fn referenced(operand: &model::Operand) -> Vec<i64> {
    match operand {
        model::Operand::ValueRef(one) => vec![one.value],
        model::Operand::ArrayElement(model::ArrayElement { indices, .. })
        | model::Operand::ProjectedPlace(model::ProjectedPlace { indices, .. }) => {
            indices.iter().flat_map(referenced).collect()
        }
        model::Operand::IndirectPlace(one) => vec![one.base],
        model::Operand::DescriptorPlace(one) => vec![one.base],
        _ => Vec::new(),
    }
}

/// Turn non-branch comparison values into ordinary -1/0 frame values.
///
/// MIR represents comparisons as flags. A comparison consumed directly by
/// one branch can remain flags; every other QB boolean is a language value
/// and must be materialized without inventing a machine-level SETcc tier.
/// Splitting HIR here preserves evaluation order and lets ordinary SSA and
/// CFG cleanup remove the temporary later.
fn _materialized_booleans(function: &model::Function, types: &IndexMap<i64, &model::Type>) -> model::Function {
    let mut values = function.values.clone();
    let mut places = function.places.clone();
    let mut blocks = function.blocks.clone();
    let mut next_value = values.iter().map(|one| one.id).max().unwrap_or(0) + 1;
    let mut next_place = places.iter().map(|one| one.id).max().unwrap_or(0) + 1;
    let mut next_block = blocks.iter().map(|one| one.id).max().unwrap_or(0) + 1;
    let mut next_instruction =
        blocks.iter().flat_map(|block| &block.instructions).map(|one| one.id).max().unwrap_or(0) + 1;

    loop {
        let mut uses: IndexMap<i64, i64> = IndexMap::default();
        for block in &blocks {
            for operand in block
                .instructions
                .iter()
                .flat_map(|instruction| &instruction.operands)
                .chain(&block.terminator.operands)
            {
                for value in referenced(operand) {
                    *uses.entry(value).or_insert(0) += 1;
                }
            }
        }
        let mut found = None;
        'blocks: for (block_index, block) in blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                if !_COMPARISONS.contains(&instruction.op) || instruction.results.len() != 1 {
                    continue;
                }
                let result = instruction.results[0];
                let direct = uses.get(&result) == Some(&1)
                    && block.terminator.kind == model::TerminatorKind::Branch
                    && block.terminator.operands == [model::Operand::value_ref(result)];
                if !direct {
                    found = Some((block_index, instruction_index));
                    break 'blocks;
                }
            }
        }
        let Some((block_index, instruction_index)) = found else {
            return model::Function { values, places, blocks, ..function.clone() };
        };

        let block = blocks[block_index].clone();
        let mut comparison = block.instructions[instruction_index].clone();
        let result = comparison.results[0];
        let result_type = values.iter().find(|one| one.id == result).expect("a verified result").r#type;
        let width = types[&result_type].width;
        let frame_low = places
            .iter()
            .filter(|one| matches!(one.storage, model::Storage::Local | model::Storage::Parameter))
            .map(|one| one.offset)
            .min()
            .unwrap_or(0);
        let place = model::Place {
            extent: Some(width),
            ..model::Place::new(
                next_place,
                &format!("$bool{next_place}"),
                result_type,
                model::Storage::Local,
                frame_low - width,
            )
        };
        next_place += 1;
        places.push(place.clone());
        let condition = next_value;
        next_value += 1;
        values.push(model::Value { id: condition, r#type: result_type });
        comparison.results = vec![condition];
        let (true_block, false_block, join_block) = (next_block, next_block + 1, next_block + 2);
        next_block += 3;
        let mut prefix_instructions = block.instructions[..instruction_index].to_vec();
        prefix_instructions.push(comparison);
        let prefix = model::Block::new(
            block.id,
            prefix_instructions,
            model::Terminator::new(
                model::TerminatorKind::Branch,
                vec![model::Operand::value_ref(condition)],
                vec![true_block, false_block],
            ),
        );
        let true_store = model::Instruction::new(
            next_instruction,
            model::Op::Store,
            Vec::new(),
            vec![model::Operand::place_ref(place.id), model::Operand::constant(result_type, -1)],
        );
        next_instruction += 1;
        let false_store = model::Instruction::new(
            next_instruction,
            model::Op::Store,
            Vec::new(),
            vec![model::Operand::place_ref(place.id), model::Operand::constant(result_type, 0)],
        );
        next_instruction += 1;
        let load = model::Instruction::new(
            next_instruction,
            model::Op::Load,
            vec![result],
            vec![model::Operand::place_ref(place.id)],
        );
        next_instruction += 1;
        let mut join_instructions = vec![load];
        join_instructions.extend(block.instructions[instruction_index + 1..].iter().cloned());
        let made = [
            prefix,
            model::Block::new(
                true_block,
                vec![true_store],
                model::Terminator::new(model::TerminatorKind::Jump, Vec::new(), vec![join_block]),
            ),
            model::Block::new(
                false_block,
                vec![false_store],
                model::Terminator::new(model::TerminatorKind::Jump, Vec::new(), vec![join_block]),
            ),
            model::Block::new(join_block, join_instructions, block.terminator.clone()),
        ];
        blocks.splice(block_index..block_index + 1, made);
    }
}

/// `mir.Value(id, at, variable=id, version=1)`.
fn _value(id: i64, at: i64) -> mir::Value {
    mir::Value { id: id as u32, at, flags: false, variable: id as u32, version: 1 }
}

/// The locals `_function`'s nested functions close over.
struct _Scope<'a> {
    module: &'a str,
    function: &'a model::Function,
    types: &'a IndexMap<i64, &'a model::Type>,
    array_order: model::ArrayOrder,
    symbols: &'a _Symbols,
    values: IndexMap<i64, mir::Value>,
    value_types: IndexMap<i64, &'a model::Type>,
    integer_ranges: mir::OrderedMap<mir::Value, mir::IntegerRange>,
    places: IndexMap<i64, &'a model::Place>,
    pieces: _Pieces,
    parameter_numbers: IndexMap<i64, i64>,
    next_frame_offset: i64,
    at: i64,
    next_value: i64,
}

fn value_width(type_: &model::Type) -> u32 {
    if type_.kind == model::TypeKind::Float { 10 } else { type_.width as u32 }
}

fn _held(value: mir::Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn _const(n: i64, width: u32) -> Arg {
    Arg::Const(Const::new(n, width))
}

impl<'a> _Scope<'a> {
    /// A fresh frame cell of `width` for instruction `id`'s conversion, and its offset.
    fn frame_cell(&mut self, id: i64, width: i64) -> (MemoryObject, i64) {
        self.next_frame_offset -= width;
        let object_ = MemoryObject {
            identity: Some(Identity::Tuple(vec![Identity::Str("float-convert".to_owned()), Identity::Int(id)])),
            extent: Some(width),
            ..MemoryObject::new(MemoryKind::Frame)
        };
        (object_, self.next_frame_offset)
    }

    /// `argument`, an integer of `source`, stored to a new frame cell of
    /// `width` bytes, widened as its sign says: the cell.
    fn integer_cell(
        &mut self,
        argument: &Arg,
        source: &model::Type,
        id: i64,
        width: i64,
        before: &mut Vec<mir::Op>,
    ) -> Result<MemRef, InvalidHIR> {
        let (object_, offset) = self.frame_cell(id, width);
        let word = width.min(4) as u32;
        let mut step = |this: &mut Self, kind: mir::Kind, operation: Operation, name: &str, arg: Arg, result: Arg, loads: Vec<MemRef>, stores: Vec<MemRef>| {
            let uses: Vec<mir::Value> = match &arg {
                Arg::Held(held) => vec![held.value],
                Arg::Cell(cell) => [cell.r#ref.base, cell.r#ref.segment].into_iter().flatten().collect(),
                _ => Vec::new(),
            };
            let made = match &result {
                Arg::Held(held) => vec![held.value],
                _ => Vec::new(),
            };
            this.at += 1;
            before.push(mir::Op {
                loads,
                stores,
                kind,
                args: vec![arg],
                results: vec![result],
                id: Some(this.at as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(this.at, OpCode::Operation(operation), name, made, uses)
            });
        };
        let fresh = |this: &mut Self, width: u32| {
            let value = _value(this.next_value, this.at + 1);
            this.next_value += 1;
            Held { value, width }
        };
        let value = match argument {
            Arg::Const(constant) => Arg::Const(Const { n: constant.n.clone(), width: word }),
            Arg::Held(_) | Arg::Cell(_) => {
                let mut value = argument.clone();
                if let Arg::Cell(cell) = argument {
                    let loaded = fresh(self, source.width as u32);
                    let loads = vec![cell.r#ref.clone()];
                    step(self, mir::Kind::Load, Operation::Move, "mov", value, Arg::Held(loaded), loads, Vec::new());
                    value = Arg::Held(loaded);
                }
                if (source.width as u32) < word {
                    let signed = source.signed != Some(false);
                    let widened = fresh(self, word);
                    let kind = if signed { mir::Kind::SignExtend } else { mir::Kind::ZeroExtend };
                    let name = if signed { "movsx" } else { "movzx" };
                    step(self, kind, Operation::Extend, name, value, Arg::Held(widened), Vec::new(), Vec::new());
                    value = Arg::Held(widened);
                }
                value
            }
            _ => return Err(InvalidHIR(format!("{}.{}: no integer to convert", self.module, self.function.name))),
        };
        // An unsigned dword is loaded as a qword: its high half is zero.
        let halves = [(value, 0)].into_iter().chain((width == 8).then(|| (_const(0, 4), 4)));
        for (half, at) in halves {
            let cell = _frame_ref(object_.clone(), offset + at, i64::from(word))?;
            let cell = MemRef { provenance: Some(_one(object_.clone(), at, at + i64::from(word))?), ..cell };
            step(self, mir::Kind::Store, Operation::Move, "mov", half, Arg::Cell(Cell { r#ref: cell.clone() }), Vec::new(), vec![cell]);
        }
        _frame_ref(object_, offset, width)
    }

    /// `args`' float stored to `reference` as `stored` says.
    fn float_store(&mut self, reference: &MemRef, args: &[Arg], stored: floating::Semantics, name: &str) -> mir::Op {
        let uses = args
            .iter()
            .filter_map(|one| match one {
                Arg::Held(one) => Some(one.value),
                _ => None,
            })
            .collect();
        let store = mir::Op {
            floating: Some(stored),
            stores: vec![reference.clone()],
            kind: mir::Kind::Fstore,
            args: args.to_vec(),
            results: vec![Arg::Cell(Cell { r#ref: reference.clone() })],
            id: Some(self.at as u32),
            reads_complete: true,
            memory_complete: true,
            ..mir::Op::new(self.at, OpCode::Operation(Operation::FloatStore), name, Vec::new(), uses)
        };
        self.at += 1;
        store
    }

    fn fresh(&mut self, type_: &'a model::Type) -> Held {
        let value = _value(self.next_value, self.at + 1);
        self.values.insert(self.next_value, value);
        self.value_types.insert(self.next_value, type_);
        self.next_value += 1;
        Held { value, width: value_width(type_) }
    }

    fn arithmetic(
        &mut self,
        kind: mir::Kind,
        left: Arg,
        right: Arg,
        type_: &'a model::Type,
        before: &mut Vec<mir::Op>,
    ) -> Held {
        let result = self.fresh(type_);
        let uses = [&left, &right]
            .into_iter()
            .filter_map(|one| match one {
                Arg::Held(one) => Some(one.value),
                _ => None,
            })
            .collect();
        self.at += 1;
        let machine = if kind == mir::Kind::PtrOffset { Operation::Binary } else { Operation::Nothing };
        let name = if kind == mir::Kind::PtrOffset { "ptr_offset" } else { "" };
        before.push(mir::Op {
            kind,
            args: vec![left, right],
            results: vec![Arg::Held(result)],
            id: Some(self.at as u32),
            reads_complete: true,
            memory_complete: true,
            ..mir::Op::new(self.at, OpCode::Operation(machine), name, vec![result.value], uses)
        });
        result
    }

    fn operand(&mut self, one: &model::Operand, before: &mut Vec<mir::Op>) -> Result<Arg, InvalidHIR> {
        match one {
            model::Operand::ValueRef(model::ValueRef { value }) => {
                Ok(_held(self.values[value], value_width(self.value_types[value])))
            }
            model::Operand::Constant(model::Constant { r#type, value: model::Number::Int(value) }) => {
                Ok(_const(*value, self.types[r#type].width as u32))
            }
            model::Operand::Constant(_) => Err(InvalidHIR(format!(
                "{}.{}: floating constants require a constant-pool place", self.module, self.function.name
            ))),
            model::Operand::PlaceRef(model::PlaceRef { place }) => {
                let type_ = self.types[&self.places[place].r#type];
                Ok(Arg::Cell(Cell { r#ref: _ref(self.places[place], type_, self.symbols, &self.pieces)? }))
            }
            model::Operand::ArrayElement(model::ArrayElement { place: place_id, indices }) => {
                let place = self.places[place_id];
                let array = self.types[&place.r#type];
                let element = self.types[&array.element.expect("a verified array has an element")];
                let mut offset: Option<Held> = None;
                let mut dimensions: Vec<(&model::Operand, &(i64, i64))> = indices.iter().zip(&array.bounds).collect();
                if self.array_order == model::ArrayOrder::ColumnMajor {
                    dimensions.reverse();
                }
                for (index, (lower, upper)) in dimensions {
                    let got = self.operand(index, before)?;
                    let index_type = match index {
                        model::Operand::Constant(one) => self.types[&one.r#type],
                        model::Operand::ValueRef(one) => self.value_types[&one.value],
                        _ => unreachable!("an index is a value or a constant"),
                    };
                    let adjusted =
                        self.arithmetic(mir::Kind::Sub, got, _const(*lower, index_type.width as u32), index_type, before);
                    if let Some(previous) = offset {
                        let count = upper - lower + 1;
                        let scaled = self.arithmetic(
                            mir::Kind::Mul,
                            Arg::Held(previous),
                            _const(count, index_type.width as u32),
                            index_type,
                            before,
                        );
                        offset = Some(self.arithmetic(
                            mir::Kind::Add,
                            Arg::Held(scaled),
                            Arg::Held(adjusted),
                            index_type,
                            before,
                        ));
                    } else {
                        offset = Some(adjusted);
                    }
                }
                let offset = offset.expect("an array has a dimension");
                let offset_type = self.value_types[&(offset.value.id as i64)];
                let offset = self.arithmetic(
                    mir::Kind::Mul,
                    Arg::Held(offset),
                    _const(element.width, offset.width),
                    offset_type,
                    before,
                );
                let space = _space(place);
                let index = if space == Space::Frame { 0 } else { place.symbol };
                let provenance = _provenance(
                    place,
                    place.extent.filter(|one| *one != 0).unwrap_or(array.width),
                    self.symbols,
                    &self.pieces,
                )?;
                let r#ref = MemRef {
                    base: Some(offset.value),
                    space: Some(space),
                    base_width: offset.width,
                    provenance: Some(provenance),
                    volatile: place.volatile,
                    inbounds: true,
                    ..MemRef::new(Some(_addr(space, place.offset, index)), element.width as u32)
                };
                Ok(Arg::Cell(Cell { r#ref }))
            }
            model::Operand::ProjectedPlace(model::ProjectedPlace {
                place: place_id,
                indices,
                offset: field_offset,
                r#type: type_id,
            }) => {
                let place = self.places[place_id];
                let root = self.types[&place.r#type];
                let field_type = self.types[type_id];
                let space = _space(place);
                let segment = if space == Space::Frame { 0 } else { place.symbol };
                let provenance = _provenance(
                    place,
                    place.extent.filter(|one| *one != 0).unwrap_or(root.width),
                    self.symbols,
                    &self.pieces,
                )?;
                if indices.is_empty() {
                    return Ok(Arg::Cell(Cell {
                        r#ref: MemRef {
                            space: Some(space),
                            provenance: Some(provenance),
                            volatile: place.volatile,
                            ..MemRef::new(
                                Some(_addr(space, place.offset + field_offset, segment)),
                                field_type.width as u32,
                            )
                        },
                    }));
                }
                let element = self.types[&root.element.expect("a verified array has an element")];
                let mut offset: Option<Held> = None;
                let mut dimensions: Vec<(&model::Operand, &(i64, i64))> = indices.iter().zip(&root.bounds).collect();
                if self.array_order == model::ArrayOrder::ColumnMajor {
                    dimensions.reverse();
                }
                for (index, (lower, upper)) in dimensions {
                    let got = self.operand(index, before)?;
                    let index_type = match index {
                        model::Operand::Constant(one) => self.types[&one.r#type],
                        model::Operand::ValueRef(one) => self.value_types[&one.value],
                        _ => unreachable!("an index is a value or a constant"),
                    };
                    let adjusted =
                        self.arithmetic(mir::Kind::Sub, got, _const(*lower, index_type.width as u32), index_type, before);
                    if let Some(previous) = offset {
                        let count = upper - lower + 1;
                        let scaled = self.arithmetic(
                            mir::Kind::Mul,
                            Arg::Held(previous),
                            _const(count, index_type.width as u32),
                            index_type,
                            before,
                        );
                        offset = Some(self.arithmetic(
                            mir::Kind::Add,
                            Arg::Held(scaled),
                            Arg::Held(adjusted),
                            index_type,
                            before,
                        ));
                    } else {
                        offset = Some(adjusted);
                    }
                }
                let offset = offset.expect("an array has a dimension");
                let offset_type = self.value_types[&(offset.value.id as i64)];
                let mut offset = self.arithmetic(
                    mir::Kind::Mul,
                    Arg::Held(offset),
                    _const(element.width, offset.width),
                    offset_type,
                    before,
                );
                if *field_offset != 0 {
                    let offset_type = self.value_types[&(offset.value.id as i64)];
                    offset = self.arithmetic(
                        mir::Kind::Add,
                        Arg::Held(offset),
                        _const(*field_offset, offset.width),
                        offset_type,
                        before,
                    );
                }
                Ok(Arg::Cell(Cell {
                    r#ref: MemRef {
                        base: Some(offset.value),
                        space: Some(space),
                        base_width: offset.width,
                        provenance: Some(provenance),
                        volatile: place.volatile,
                        inbounds: true,
                        ..MemRef::new(Some(_addr(space, place.offset, segment)), field_type.width as u32)
                    },
                }))
            }
            model::Operand::IndirectPlace(model::IndirectPlace { base, offset, r#type: type_id, volatile, inbounds }) => {
                let type_ = self.types[type_id];
                let pointer_type = self.value_types[base];
                let parameter = self.parameter_numbers.get(base).copied();
                let provenance = parameter.map(|parameter| {
                    Provenance::one(MemoryObject {
                        identity: Some(Identity::Int(parameter)),
                        ..MemoryObject::new(MemoryKind::Parameter)
                    })
                });
                if pointer_type.address == model::AddressKind::Near {
                    return Ok(Arg::Cell(Cell {
                        r#ref: MemRef {
                            base: Some(self.values[base]),
                            space: Some(Space::Literal),
                            base_width: pointer_type.width as u32,
                            provenance,
                            inbounds: *inbounds,
                            volatile: *volatile,
                            ..MemRef::new(Some(Addr::new(Space::Literal, *offset)), type_.width as u32)
                        },
                    }));
                }
                let mut pointer = self.values[base];
                if pointer_type.address == model::AddressKind::Far {
                    // A far pointer's selector and offset are independent
                    // address components.  Constant field selection advances
                    // only the 16-bit offset; unlike a huge pointer it must not
                    // normalize carry into the selector.  Preserve that form
                    // in MIR so every consumer, integral or floating, reaches
                    // lowering as one segmented memory reference.
                    let mut halves = Vec::new();
                    for bit in [0, 16] {
                        let half = _value(self.next_value, self.at + 1);
                        self.next_value += 1;
                        self.at += 1;
                        before.push(mir::Op {
                            kind: mir::Kind::Extract,
                            args: vec![_held(pointer, 4), _const(bit, 1)],
                            results: vec![_held(half, 2)],
                            id: Some(self.at as u32),
                            reads_complete: true,
                            memory_complete: true,
                            ..mir::Op::new(
                                self.at,
                                OpCode::Operation(Operation::Move),
                                "extract",
                                vec![half],
                                vec![pointer],
                            )
                        });
                        halves.push(half);
                    }
                    let (mut offset_value, segment_value) = (halves[0], halves[1]);
                    if *offset != 0 {
                        let adjusted = _value(self.next_value, self.at + 1);
                        self.next_value += 1;
                        self.at += 1;
                        before.push(mir::Op {
                            kind: mir::Kind::Add,
                            args: vec![_held(offset_value, 2), _const(*offset, 2)],
                            results: vec![_held(adjusted, 2)],
                            id: Some(self.at as u32),
                            reads_complete: true,
                            memory_complete: true,
                            ..mir::Op::new(
                                self.at,
                                OpCode::Operation(Operation::Nothing),
                                "",
                                vec![adjusted],
                                vec![offset_value],
                            )
                        });
                        offset_value = adjusted;
                    }
                    return Ok(Arg::Cell(Cell {
                        r#ref: MemRef {
                            base: Some(offset_value),
                            segment: Some(segment_value),
                            space: Some(Space::Far),
                            base_width: 2,
                            provenance,
                            inbounds: *inbounds,
                            volatile: *volatile,
                            ..MemRef::new(Some(Addr::new(Space::Far, 0)), type_.width as u32)
                        },
                    }));
                }
                if *offset != 0 {
                    let adjusted = self.arithmetic(
                        mir::Kind::PtrOffset,
                        _held(pointer, pointer_type.width as u32),
                        _const(*offset, 4),
                        pointer_type,
                        before,
                    );
                    pointer = adjusted.value;
                }
                if type_.kind == model::TypeKind::Float {
                    let mut halves = Vec::new();
                    for bit in [0, 16] {
                        let half = _value(self.next_value, self.at + 1);
                        self.next_value += 1;
                        self.at += 1;
                        before.push(mir::Op {
                            kind: mir::Kind::Extract,
                            args: vec![_held(pointer, 4), _const(bit, 1)],
                            results: vec![_held(half, 2)],
                            id: Some(self.at as u32),
                            reads_complete: true,
                            memory_complete: true,
                            ..mir::Op::new(
                                self.at,
                                OpCode::Operation(Operation::Move),
                                "extract",
                                vec![half],
                                vec![pointer],
                            )
                        });
                        halves.push(half);
                    }
                    let (offset_value, segment_value) = (halves[0], halves[1]);
                    return Ok(Arg::Cell(Cell {
                        r#ref: MemRef {
                            base: Some(offset_value),
                            segment: Some(segment_value),
                            space: Some(Space::Far),
                            base_width: 2,
                            provenance,
                            inbounds: *inbounds,
                            ..MemRef::new(Some(Addr::new(Space::Far, 0)), type_.width as u32)
                        },
                    }));
                }
                Ok(Arg::Cell(Cell {
                    r#ref: MemRef {
                        base: Some(pointer),
                        base_width: pointer_type.width as u32,
                        pointer: true,
                        provenance,
                        inbounds: *inbounds,
                        volatile: *volatile,
                        ..MemRef::new(None, type_.width as u32)
                    },
                }))
            }
            model::Operand::DescriptorPlace(model::DescriptorPlace { base, field, r#type: type_id }) => {
                let pointer_type = self.value_types[base];
                let pointee = pointer_type.element.map(|element| self.types[&element]);
                let scoped_view = pointee
                    .is_some_and(|pointee| pointee.kind == model::TypeKind::Opaque && pointee.name.starts_with("$slice["));
                let offset = if scoped_view {
                    if *field == model::DescriptorField::Length { 0 } else { 2 }
                } else if *field == model::DescriptorField::Length {
                    -4
                } else {
                    -2
                };
                self.operand(
                    &model::Operand::IndirectPlace(model::IndirectPlace {
                        base: *base,
                        offset,
                        r#type: *type_id,
                        volatile: false,
                        inbounds: false,
                    }),
                    before,
                )
            }
        }
    }

    fn operation(&mut self, instruction: &model::Instruction) -> Result<Vec<mir::Op>, InvalidHIR> {
        let mut before: Vec<mir::Op> = Vec::new();
        let mut args = instruction
            .operands
            .iter()
            .map(|one| self.operand(one, &mut before))
            .collect::<Result<Vec<_>, _>>()?;
        if instruction.op == model::Op::Address {
            let [Arg::Cell(cell)] = &args[..] else {
                return Err(InvalidHIR(format!("{}.{}: address needs one place", self.module, self.function.name)));
            };
            // ADDRESS observes no payload bytes. A source object's extent may
            // be many kilobytes, but the real-mode LEA operand is a word-sized
            // effective address. Carrying the object's width here eventually
            // asks the encoder for an impossible 5279-byte memory operand.
            args = vec![Arg::Cell(Cell { r#ref: MemRef { width: 2, ..cell.r#ref.clone() } })];
        }
        if instruction.op == model::Op::Address
            && self.value_types[&instruction.results[0]].address == model::AddressKind::Near
            && matches!(&args[0], Arg::Cell(cell) if cell.r#ref.base.is_some())
        {
            // A 16-bit frame address cannot use every allocator register as
            // BP's index (`[bp+bx]` has no encoding). Materialize the fixed
            // base and add the dynamic byte offset as ordinary integer MIR;
            // the latter may then live in any general register.
            let Arg::Cell(cell) = &args[0] else { unreachable!("matched above") };
            let reference = cell.r#ref.clone();
            let reference_base = reference.base.expect("matched above");
            if let Some(addr) = reference.addr.filter(|addr| addr.space == Space::Literal) {
                self.at += 1;
                let result = self.values[&instruction.results[0]];
                let displacement = addr.disp;
                let direct = mir::Op {
                    kind: if displacement == 0 { mir::Kind::Copy } else { mir::Kind::Add },
                    args: if displacement == 0 {
                        vec![_held(reference_base, reference.base_width)]
                    } else {
                        vec![_held(reference_base, reference.base_width), _const(displacement, reference.base_width)]
                    },
                    results: vec![_held(result, 2)],
                    id: Some(instruction.id as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(
                        self.at,
                        OpCode::Operation(if displacement == 0 { Operation::Move } else { Operation::Binary }),
                        if displacement == 0 { "mov" } else { "add" },
                        vec![result],
                        vec![reference_base],
                    )
                };
                before.push(direct);
                return Ok(before);
            }
            let base = _value(self.next_value, self.at + 1);
            self.next_value += 1;
            self.at += 1;
            let base_address = mir::Op {
                kind: mir::Kind::Address,
                args: vec![Arg::Cell(Cell { r#ref: MemRef { base: None, width: 2, ..reference.clone() } })],
                results: vec![_held(base, 2)],
                id: Some(self.at as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(self.at, OpCode::Operation(Operation::Address), "lea", vec![base], Vec::new())
            };
            self.at += 1;
            let result = self.values[&instruction.results[0]];
            let added = mir::Op {
                kind: mir::Kind::Add,
                args: vec![_held(base, 2), _held(reference_base, reference.base_width)],
                results: vec![_held(result, 2)],
                id: Some(instruction.id as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(
                    self.at,
                    OpCode::Operation(Operation::Binary),
                    "add",
                    vec![result],
                    vec![base, reference_base],
                )
            };
            before.push(base_address);
            before.push(added);
            return Ok(before);
        }
        if instruction.op == model::Op::Address
            && matches!(
                self.value_types[&instruction.results[0]].address,
                model::AddressKind::Far | model::AddressKind::Huge
            )
        {
            let [Arg::Cell(cell)] = &args[..] else {
                return Err(InvalidHIR(format!("{}.{}: whole address needs one place", self.module, self.function.name)));
            };
            let reference = cell.r#ref.clone();
            let mut offset = _value(self.next_value, self.at + 1);
            self.next_value += 1;
            self.at += 1;
            // A cell through a far pointer is `segment:base+disp` already.
            let literal =
                reference.addr.is_some_and(|addr| addr.space == Space::Literal) || reference.segment.is_some();
            let mut offset_ops = if let (true, Some(reference_base)) = (literal, reference.base) {
                // An IndirectPlace is already relative to a pointer value.
                // Its literal displacement is not an absolute symbol that can
                // be addressed independently: the offset half is base+disp.
                let displacement = reference.addr.map_or(0, |addr| addr.disp);
                vec![mir::Op {
                    kind: if displacement == 0 { mir::Kind::Copy } else { mir::Kind::Add },
                    args: if displacement == 0 {
                        vec![_held(reference_base, reference.base_width)]
                    } else {
                        vec![_held(reference_base, reference.base_width), _const(displacement, reference.base_width)]
                    },
                    results: vec![_held(offset, 2)],
                    id: Some(self.at as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(
                        self.at,
                        OpCode::Operation(if displacement == 0 { Operation::Move } else { Operation::Binary }),
                        if displacement == 0 { "mov" } else { "add" },
                        vec![offset],
                        vec![reference_base],
                    )
                }]
            } else {
                let address = mir::Op {
                    kind: mir::Kind::Address,
                    args: vec![Arg::Cell(Cell { r#ref: MemRef { base: None, width: 2, ..reference.clone() } })],
                    results: vec![_held(offset, 2)],
                    id: Some(self.at as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(self.at, OpCode::Operation(Operation::Address), "lea", vec![offset], Vec::new())
                };
                vec![address]
            };
            if let (Some(reference_base), false) = (reference.base, literal) {
                // A whole pointer still contains a 16-bit offset. Keep its
                // segment construction below, but form that offset with the
                // same unrestricted integer add used by a near pointer. This
                // prevents allocation from turning a local array element into
                // an unencodable 16-bit address such as [bp+bx].
                let added = _value(self.next_value, self.at + 1);
                self.next_value += 1;
                self.at += 1;
                offset_ops.push(mir::Op {
                    kind: mir::Kind::Add,
                    args: vec![_held(offset, 2), _held(reference_base, reference.base_width)],
                    results: vec![_held(added, 2)],
                    id: Some(self.at as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(
                        self.at,
                        OpCode::Operation(Operation::Binary),
                        "add",
                        vec![added],
                        vec![offset, reference_base],
                    )
                });
                offset = added;
            }
            let segment = _value(self.next_value, self.at + 1);
            self.next_value += 1;
            self.at += 1;
            let selector_source = if let Some(segment) = reference.segment {
                _held(segment, 2)
            } else if reference.space == Some(Space::Frame) {
                Arg::FrameSelector(mir::FrameSelector::default())
            } else {
                Arg::Symbol(mir::Symbol::new(DGROUP.0, DGROUP.1, 0, 2))
            };
            let selector = mir::Op {
                kind: mir::Kind::Copy,
                args: vec![selector_source],
                results: vec![_held(segment, 2)],
                id: Some(self.at as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(self.at, OpCode::Operation(Operation::Move), "mov", vec![segment], Vec::new())
            };
            self.at += 1;
            let result = self.values[&instruction.results[0]];
            let joined = mir::Op {
                kind: mir::Kind::Concat,
                args: vec![_held(segment, 2), _held(offset, 2)],
                results: vec![_held(result, 4)],
                id: Some(instruction.id as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(self.at, OpCode::Operation(Operation::Move), "", vec![result], vec![segment, offset])
            };
            before.extend(offset_ops);
            before.push(selector);
            before.push(joined);
            return Ok(before);
        }
        if instruction.op == model::Op::PtrOffset
            && self.value_types[&instruction.results[0]].address == model::AddressKind::Far
        {
            let valid = args.len() == 2
                && matches!(&args[0], Arg::Held(one) if one.width == 4)
                && matches!(&args[1], Arg::Held(Held { width: 1 | 2 | 4, .. }) | Arg::Const(Const { width: 1 | 2 | 4, .. }));
            if !valid {
                return Err(InvalidHIR(format!(
                    "{}.{}: far pointer offset needs a pointer and integer displacement", self.module, self.function.name
                )));
            }
            let (Arg::Held(pointer), mut displacement) = (args[0].clone(), args[1].clone()) else {
                unreachable!("checked above");
            };
            let displacement_width = match &displacement {
                Arg::Held(one) => one.width,
                Arg::Const(one) => one.width,
                _ => unreachable!("checked above"),
            };
            if displacement_width != 2 {
                displacement = match displacement {
                    Arg::Held(one) => _held(one.value, 2),
                    Arg::Const(one) => Arg::Const(Const { n: one.n, width: 2 }),
                    _ => unreachable!("checked above"),
                };
            }
            let mut halves = Vec::new();
            for bit in [0, 16] {
                let half = _value(self.next_value, self.at + 1);
                self.next_value += 1;
                self.at += 1;
                before.push(mir::Op {
                    kind: mir::Kind::Extract,
                    args: vec![Arg::Held(pointer), _const(bit, 1)],
                    results: vec![_held(half, 2)],
                    id: Some(self.at as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(
                        self.at,
                        OpCode::Operation(Operation::Move),
                        "extract",
                        vec![half],
                        vec![pointer.value],
                    )
                });
                halves.push(half);
            }
            let (offset, segment) = (halves[0], halves[1]);
            let adjusted = _value(self.next_value, self.at + 1);
            self.next_value += 1;
            self.at += 1;
            let uses = [_held(offset, 2), displacement.clone()]
                .into_iter()
                .filter_map(|one| match one {
                    Arg::Held(one) => Some(one.value),
                    _ => None,
                })
                .collect();
            before.push(mir::Op {
                kind: mir::Kind::Add,
                args: vec![_held(offset, 2), displacement],
                results: vec![_held(adjusted, 2)],
                id: Some(self.at as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(self.at, OpCode::Operation(Operation::Binary), "add", vec![adjusted], uses)
            });
            self.at += 1;
            let result = self.values[&instruction.results[0]];
            before.push(mir::Op {
                kind: mir::Kind::Concat,
                args: vec![_held(segment, 2), _held(adjusted, 2)],
                results: vec![_held(result, 4)],
                id: Some(instruction.id as u32),
                reads_complete: true,
                memory_complete: true,
                ..mir::Op::new(self.at, OpCode::Operation(Operation::Move), "", vec![result], vec![segment, adjusted])
            });
            return Ok(before);
        }
        if matches!(instruction.op, model::Op::PointerSegment | model::Op::PointerOffset) {
            let [Arg::Held(pointer)] = &args[..] else {
                return Err(InvalidHIR(format!("{}.{}: pointer projection needs one pointer", self.module, self.function.name)));
            };
            let pointer = *pointer;
            if instruction.op == model::Op::PointerSegment && pointer.width != 4 {
                return Err(InvalidHIR(format!("{}.{}: segment projection needs one far pointer", self.module, self.function.name)));
            }
            if instruction.op == model::Op::PointerOffset && !matches!(pointer.width, 2 | 4) {
                return Err(InvalidHIR(format!(
                    "{}.{}: offset projection needs a near or far pointer", self.module, self.function.name
                )));
            }
            self.at += 1;
            let result = self.values[&instruction.results[0]];
            let projected = if instruction.op == model::Op::PointerSegment {
                mir::Op {
                    kind: mir::Kind::Shr,
                    args: vec![Arg::Held(pointer), _const(16, 1)],
                    results: vec![_held(result, 4)],
                    id: Some(instruction.id as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(
                        self.at,
                        OpCode::Operation(Operation::Nothing),
                        "",
                        vec![result],
                        vec![pointer.value],
                    )
                }
            } else {
                mir::Op {
                    kind: mir::Kind::Copy,
                    args: vec![_held(pointer.value, 2)],
                    results: vec![_held(result, 2)],
                    id: Some(instruction.id as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(
                        self.at,
                        OpCode::Operation(Operation::Move),
                        "mov",
                        vec![result],
                        vec![pointer.value],
                    )
                }
            };
            before.push(projected);
            return Ok(before);
        }
        if instruction.op == model::Op::Convert && instruction.operands.len() == 1 && instruction.results.len() == 1
        {
            let source_type = match &instruction.operands[0] {
                model::Operand::ValueRef(one) => self.value_types[&one.value],
                model::Operand::PlaceRef(one) => self.types[&self.places[&one.place].r#type],
                model::Operand::Constant(one) => self.types[&one.r#type],
                model::Operand::ProjectedPlace(one) => self.types[&one.r#type],
                model::Operand::IndirectPlace(one) => self.types[&one.r#type],
                model::Operand::DescriptorPlace(one) => self.types[&one.r#type],
                model::Operand::ArrayElement(_) => {
                    panic!("AttributeError: 'ArrayElement' object has no attribute 'type'")
                }
            };
            let target_type = self.value_types[&instruction.results[0]];
            if source_type.kind == model::TypeKind::Float
                && target_type.kind == model::TypeKind::Float
                && target_type.width >= source_type.width
            {
                if let Arg::Held(held) = &args[0] {
                    // Both QB formats evaluate in extended80. Widening changes
                    // the source type of later storage and calls, but creates no
                    // new machine value and performs no rounding.
                    self.values.insert(instruction.results[0], held.value);
                    return Ok(before);
                }
            }
        }
        self.at += 1;
        let mut made: Vec<mir::Value> = instruction.results.iter().map(|one| self.values[one]).collect();
        let mut results: Vec<Arg> = made
            .iter()
            .zip(&instruction.results)
            .map(|(one, source)| _held(*one, value_width(self.value_types[source])))
            .collect();
        if instruction.op == model::Op::Load && instruction.operands.len() == 1 && made.len() == 1 {
            if let model::Operand::DescriptorPlace(descriptor) = &instruction.operands[0] {
                let pointer = self.value_types[&descriptor.base];
                let mut element = self.types[&pointer.element.expect("a descriptor pointer has an element")];
                if element.kind == model::TypeKind::Opaque && element.name.starts_with("$slice[") {
                    element = self.types[&element.element.expect("a slice has an element")];
                }
                // Translate the target ABI rule here, at the HIR boundary.  A
                // descriptor-backed slice fits in one pointer-offset domain, so
                // its element count cannot exceed that domain divided by the
                // element width.  MIR receives only the resulting integer fact.
                let offset_width =
                    if pointer.address == model::AddressKind::Near { pointer.width } else { pointer.width / 2 };
                let field_width = self.value_types[&instruction.results[0]].width;
                let maximum = ((1_i128 << (field_width * 8)) - 1).min((1_i128 << (offset_width * 8)) / element.width as i128);
                self.integer_ranges.insert(made[0], mir::IntegerRange::new(0, maximum, field_width as u32));
            }
        }
        // Shared MIR deliberately has no target-instruction catalogue. Keep
        // source intrinsics as unary floating computation, never CALL: FSQRT
        // is the existing unary-float carrier and ``name`` retains the exact
        // operation for the QB-owned physical boundary. No sqrt facts are
        // claimed because these operations have no ``floating`` rule below.
        let mut kind = if _STRING_COMPARISONS.contains(&instruction.op) || instruction.op == model::Op::Asm {
            mir::Kind::Call
        } else if _X87_INTRINSICS(instruction.op).is_some() {
            mir::Kind::Fsqrt
        } else if matches!(instruction.op, model::Op::Udiv | model::Op::Urem | model::Op::Udivmod) {
            mir::Kind::Udivmod
        } else if instruction.op == model::Op::Truncate {
            mir::Kind::Fstore
        } else {
            _KINDS(instruction.op.value())
        };
        if matches!(instruction.op, model::Op::Div | model::Op::Rem | model::Op::Udiv | model::Op::Urem) {
            let source = instruction.results[0];
            let extra = _value(self.next_value, self.at);
            let source_type = self.value_types[&source];
            self.value_types.insert(self.next_value, source_type);
            self.next_value += 1;
            made = if matches!(instruction.op, model::Op::Div | model::Op::Udiv) {
                vec![made[0], extra]
            } else {
                vec![extra, made[0]]
            };
            results = made.iter().map(|one| _held(*one, source_type.width as u32)).collect();
            kind = if matches!(instruction.op, model::Op::Udiv | model::Op::Urem) {
                mir::Kind::Udivmod
            } else {
                mir::Kind::Divmod
            };
        }
        let cells: Vec<MemRef> = args
            .iter()
            .filter_map(|one| match one {
                Arg::Cell(cell) => Some(cell.r#ref.clone()),
                _ => None,
            })
            .collect();
        let mut semantics = None;
        if _BINARY_FLOAT.contains(&instruction.op) || _UNARY_FLOAT.contains(&instruction.op) {
            let formats: Vec<floating::Format> = instruction
                .operands
                .iter()
                .filter_map(|one| match one {
                    model::Operand::ValueRef(one) => Some(_FORMATS(self.value_types[&one.value].evaluation)),
                    _ => None,
                })
                .collect();
            let result = _FORMATS(self.value_types[&instruction.results[0]].evaluation);
            semantics = Some(floating::Semantics::new(
                formats,
                result,
                floating::Precision::Dynamic,
                floating::Rounding::Dynamic,
            ));
            if matches!(instruction.op, model::Op::Fneg | model::Op::Fabs) {
                semantics = semantics.map(|semantics| floating::Semantics {
                    precision: floating::Precision::Exact,
                    rounding: floating::Rounding::None,
                    ..semantics
                });
            } else if _X87_INTRINSICS(instruction.op).is_some() {
                // The strict-float fact model knows FSQRT but not
                // transcendental functions. Omitting a false rule keeps the
                // optimizer conservative without hiding work in a pseudo-call.
                semantics = None;
            }
        }
        if instruction.op == model::Op::Load && self.value_types[&instruction.results[0]].kind == model::TypeKind::Float
        {
            let stored = self.value_types[&instruction.results[0]];
            kind = mir::Kind::Fload;
            semantics = Some(floating::Semantics::new(
                vec![_stored_format(stored)?],
                _FORMATS(self.value_types[&instruction.results[0]].evaluation),
                floating::Precision::Exact,
                floating::Rounding::None,
            ));
        }
        if instruction.op == model::Op::Store {
            if let model::Operand::ValueRef(stored_value) = &instruction.operands[1] {
                if self.value_types[&stored_value.value].kind == model::TypeKind::Float {
                    let stored = self.value_types[&stored_value.value];
                    let source = self.value_types[&stored_value.value];
                    kind = mir::Kind::Fstore;
                    semantics = Some(floating::Semantics::new(
                        vec![_FORMATS(source.evaluation)],
                        _stored_format(stored)?,
                        floating::Precision::Destination,
                        floating::Rounding::Dynamic,
                    ));
                }
            }
        }
        let mut uses: Vec<mir::Value> = Vec::new();
        for one in &args {
            let values = match one {
                Arg::Held(one) => vec![one.value],
                Arg::Cell(cell) => [cell.r#ref.base, cell.r#ref.segment].into_iter().flatten().collect(),
                _ => Vec::new(),
            };
            for value in values {
                if !uses.contains(&value) {
                    uses.push(value);
                }
            }
        }
        let mut operation_kind = if _X87_INTRINSICS(instruction.op).is_some() {
            Operation::FloatUnary
        } else if kind == mir::Kind::Call {
            Operation::Call
        } else {
            Operation::Nothing
        };
        let mut operation_name: String = _X87_INTRINSICS(instruction.op)
            .map(str::to_owned)
            .or_else(|| instruction.asm.as_ref().map(|_| ASM.to_owned()))
            .unwrap_or_else(|| instruction.callee.clone().unwrap_or_default());
        if instruction.op == model::Op::PtrOffset {
            let target_pointer = self.value_types[&instruction.results[0]];
            if target_pointer.width == 4 {
                (operation_kind, operation_name) = (Operation::Binary, "ptr_offset".to_owned());
            } else {
                kind = mir::Kind::Add;
            }
        }
        if matches!(instruction.op, model::Op::Convert | model::Op::Truncate) {
            let source_id = match &instruction.operands[0] {
                model::Operand::ValueRef(one) => self.value_types[&one.value],
                model::Operand::PlaceRef(one) => self.types[&self.places[&one.place].r#type],
                model::Operand::Constant(one) => self.types[&one.r#type],
                model::Operand::ProjectedPlace(one) => self.types[&one.r#type],
                model::Operand::IndirectPlace(one) => self.types[&one.r#type],
                model::Operand::DescriptorPlace(one) => self.types[&one.r#type],
                model::Operand::ArrayElement(_) => {
                    panic!("AttributeError: 'ArrayElement' object has no attribute 'type'")
                }
            };
            let target_type = self.value_types[&instruction.results[0]];
            if matches!(source_id.kind, model::TypeKind::Integer | model::TypeKind::Boolean)
                && matches!(target_type.kind, model::TypeKind::Integer | model::TypeKind::Boolean)
            {
                if let Arg::Const(constant) = &args[0] {
                    args = vec![Arg::Const(Const { n: constant.n.clone(), width: target_type.width as u32 })];
                    (operation_kind, operation_name, kind) = (Operation::Move, "mov".to_owned(), mir::Kind::Copy);
                } else if target_type.width > source_id.width {
                    operation_kind = Operation::Extend;
                    operation_name = if source_id.signed != Some(false) { "movsx" } else { "movzx" }.to_owned();
                    kind = if source_id.signed != Some(false) { mir::Kind::SignExtend } else { mir::Kind::ZeroExtend };
                } else {
                    (operation_kind, operation_name, kind) = (Operation::Move, "mov".to_owned(), mir::Kind::Copy);
                    if let Arg::Held(held) = &args[0] {
                        args = vec![_held(held.value, target_type.width as u32)];
                    }
                }
            } else if matches!(source_id.kind, model::TypeKind::Integer | model::TypeKind::Boolean)
                && target_type.kind == model::TypeKind::Float
            {
                // The x87 loads only a signed integer, from memory: the
                // narrowest format holding every value of the source.
                let (format, width) = _integer_format(source_id);
                let reference = match &args[0] {
                    Arg::Cell(cell) if width == source_id.width => cell.r#ref.clone(),
                    argument => self.integer_cell(argument, source_id, instruction.id, width, &mut before)?,
                };
                let uses = [reference.base, reference.segment].into_iter().flatten().collect();
                let load = mir::Op {
                    floating: Some(floating::Semantics::new(
                        vec![format],
                        _FORMATS(target_type.evaluation),
                        floating::Precision::Exact,
                        floating::Rounding::None,
                    )),
                    loads: vec![reference.clone()],
                    kind: mir::Kind::Fload,
                    args: vec![Arg::Cell(Cell { r#ref: reference })],
                    results,
                    id: Some(instruction.id as u32),
                    reads_complete: true,
                    memory_complete: true,
                    ..mir::Op::new(self.at, OpCode::Operation(Operation::FloatLoad), "fild", made, uses)
                };
                before.push(load);
                return Ok(before);
            } else if source_id.kind == model::TypeKind::Float
                && matches!(target_type.kind, model::TypeKind::Integer | model::TypeKind::Boolean)
            {
                kind = mir::Kind::Fstore;
                let truncates = instruction.op == model::Op::Truncate;
                // Toward zero is fisttp, which FloatAlloc spells for an x87 without one.
                (operation_kind, operation_name) =
                    (Operation::FloatStore, if truncates { "fisttp" } else { "fistp" }.to_owned());
                let rounding = if truncates { floating::Rounding::TowardZero } else { floating::Rounding::Dynamic };
                let held = _integer_format(target_type);
                let stored = |format| {
                    floating::Semantics::new(
                        vec![floating::Format::Extended80],
                        format,
                        floating::Precision::Destination,
                        rounding,
                    )
                };
                if held.1 == target_type.width {
                    semantics = Some(stored(held.0));
                } else {
                    // No x87 format is the target: its range is stored wider,
                    // and its bytes are the low ones.
                    let (object_, offset) = self.frame_cell(instruction.id, held.1);
                    let reference = _frame_ref(object_.clone(), offset, held.1)?;
                    let store = self.float_store(&reference, &args, stored(held.0), &operation_name);
                    let narrow = _frame_ref(object_, offset, target_type.width)?;
                    let load = mir::Op {
                        loads: vec![narrow.clone()],
                        kind: mir::Kind::Load,
                        args: vec![Arg::Cell(Cell { r#ref: narrow })],
                        results,
                        id: Some(instruction.id as u32),
                        reads_complete: true,
                        memory_complete: true,
                        ..mir::Op::new(self.at, OpCode::Operation(Operation::Move), "mov", made, Vec::new())
                    };
                    before.push(store);
                    before.push(load);
                    return Ok(before);
                }
            } else if source_id.kind == model::TypeKind::Float && target_type.kind == model::TypeKind::Float {
                if target_type.width >= source_id.width {
                    kind = mir::Kind::Copy;
                    (operation_kind, operation_name) = (Operation::Move, "mov".to_owned());
                } else {
                    let (object_, offset) = self.frame_cell(instruction.id, target_type.width);
                    let reference = _frame_ref(object_, offset, target_type.width)?;
                    let stored = floating::Semantics::new(
                        vec![floating::Format::Extended80],
                        _stored_format(target_type)?,
                        floating::Precision::Destination,
                        floating::Rounding::Dynamic,
                    );
                    let store = self.float_store(&reference, &args, stored, "fstp");
                    let loaded = floating::Semantics::new(
                        vec![_stored_format(target_type)?],
                        floating::Format::Extended80,
                        floating::Precision::Exact,
                        floating::Rounding::None,
                    );
                    let load = mir::Op {
                        floating: Some(loaded),
                        loads: vec![reference.clone()],
                        kind: mir::Kind::Fload,
                        args: vec![Arg::Cell(Cell { r#ref: reference })],
                        results,
                        id: Some(instruction.id as u32),
                        reads_complete: true,
                        memory_complete: true,
                        ..mir::Op::new(self.at, OpCode::Operation(Operation::FloatLoad), "fld", made, Vec::new())
                    };
                    before.push(store);
                    before.push(load);
                    return Ok(before);
                }
            }
        }
        let mut loads = if matches!(kind, mir::Kind::Load | mir::Kind::Fload) { cells.clone() } else { Vec::new() };
        let mut stores = if matches!(kind, mir::Kind::Store | mir::Kind::Fstore) { cells } else { Vec::new() };
        if instruction.op == model::Op::Call {
            // BASIC's default argument convention is BYREF. A pointer actual
            // therefore publishes its pointee to the callee for both reading
            // and writing; the stack word alone is not the call's complete
            // memory effect. Without these witnesses, promotion deleted the
            // stores of 100000 and 23 before Q45P04's addLong call and passed
            // two uninitialized frame slots instead.
            let mut pointees = Vec::new();
            for argument in &args {
                let Arg::Held(argument) = argument else {
                    continue;
                };
                let Some(type_) = self.value_types.get(&(argument.value.id as i64)) else {
                    continue;
                };
                let Some(element) = type_.element.filter(|_| type_.kind == model::TypeKind::Pointer) else {
                    continue;
                };
                let element = self.types[&element];
                pointees.push(MemRef {
                    base: Some(argument.value),
                    space: Some(Space::Literal),
                    base_width: type_.width as u32,
                    pointer: true,
                    ..MemRef::new(None, element.width as u32)
                });
            }
            loads = pointees.clone();
            stores = pointees;
        }
        if instruction.op == model::Op::Store {
            let (2, Some(Arg::Cell(destination))) = (args.len(), args.first()) else {
                return Err(InvalidHIR(format!("{}.{}: store has no destination cell", self.module, self.function.name)));
            };
            let destination = destination.clone();
            let address_uses: Vec<mir::Value> =
                [destination.r#ref.base, destination.r#ref.segment].into_iter().flatten().collect();
            results = vec![Arg::Cell(destination)];
            args = vec![args[1].clone()];
            uses = Vec::new();
            for value in address_uses.into_iter().chain(args.iter().filter_map(|one| match one {
                Arg::Held(one) => Some(one.value),
                _ => None,
            })) {
                if !uses.contains(&value) {
                    uses.push(value);
                }
            }
        }
        // An inline block touches memory only where it says so.
        let complete = instruction.op != model::Op::Call
            && !_STRING_COMPARISONS.contains(&instruction.op)
            && instruction.asm.as_ref().is_none_or(|asm| !asm.memory);
        let port = matches!(instruction.op, model::Op::PortIn | model::Op::PortOut);
        // A device with no path to memory leaves every cell alone; any other
        // port may start a transfer, so its memory effect stays unknown.
        let silent_port = port && matches!(args.first(), Some(Arg::Const(one)) if one.n.to_i64().is_some_and(|port| machine::current().silent_port(port)));
        let volatile = port || instruction.op == model::Op::Asm || loads.iter().chain(&stores).any(|reference| reference.volatile);
        let final_ = mir::Op {
            floating: semantics,
            loads,
            stores,
            kind,
            args,
            results,
            id: Some(instruction.id as u32),
            args_known: true,
            memory_complete: (complete && !port) || silent_port,
            reads_complete: complete,
            volatile,
            ..mir::Op::new(self.at, OpCode::Operation(operation_kind), operation_name, made, uses)
        };
        before.push(final_);
        Ok(before)
    }
}

fn _function(
    module: &str,
    function: &model::Function,
    types: &IndexMap<i64, &model::Type>,
    externals: &IndexMap<i64, String>,
    array_order: model::ArrayOrder,
    symbols: &_Symbols,
) -> Result<Lowered, InvalidHIR> {
    let values: IndexMap<i64, mir::Value> = function
        .values
        .iter()
        .map(|one| (one.id, mir::Value { id: one.id as u32, at: one.id, flags: false, variable: one.id as u32, version: 1 }))
        .collect();
    let value_types: IndexMap<i64, &model::Type> =
        function.values.iter().map(|one| (one.id, types[&one.r#type])).collect();
    let places: IndexMap<i64, &model::Place> = function.places.iter().map(|one| (one.id, one)).collect();
    let parameter_numbers: IndexMap<i64, i64> =
        function.parameters.iter().enumerate().map(|(number, value)| (*value, number as i64)).collect();
    let next_frame_offset = function
        .places
        .iter()
        .filter(|one| matches!(one.storage, model::Storage::Local | model::Storage::Parameter))
        .map(|one| one.offset)
        .min()
        .unwrap_or(0);
    let next_value = values.keys().copied().max().unwrap_or(0) + 1;
    let mut definitions: IndexMap<i64, &model::Instruction> = IndexMap::default();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        for result in &instruction.results {
            definitions.insert(*result, instruction);
        }
    }

    let mut use_counts: IndexMap<i64, i64> = IndexMap::default();
    for block in &function.blocks {
        let operands = block
            .instructions
            .iter()
            .flat_map(|instruction| &instruction.operands)
            .chain(&block.terminator.operands);
        for operand_ in operands {
            for value in referenced(operand_) {
                *use_counts.entry(value).or_insert(0) += 1;
            }
        }
    }

    let mut scope = _Scope {
        module,
        function,
        types,
        array_order,
        symbols,
        values,
        value_types,
        integer_ranges: mir::OrderedMap::new(),
        places,
        pieces: _pieces(function),
        parameter_numbers,
        next_frame_offset,
        at: 0,
        next_value,
    };

    // HIR block ids are stable source identities. Preserve them through MIR:
    // preprocessing can insert comparison-materialization blocks, so mapping
    // by the post-expansion list position silently retargets independently
    // recorded entries such as ON ERROR handlers and RESUME statement rows.
    let block_at: IndexMap<i64, i64> = function.blocks.iter().map(|one| (one.id, one.id)).collect();
    let mut blocks = Vec::new();
    let mut source_instructions: IndexMap<i64, i64> = IndexMap::default();
    for source in &function.blocks {
        let mut ops: Vec<mir::Op> = Vec::new();
        let mut pending_source: Vec<i64> = Vec::new();
        for instruction in &source.instructions {
            let made = scope.operation(instruction)?;
            if made.is_empty() {
                // A semantic no-op (for example an identity conversion) owns
                // no machine address. Its statement begins at the next real
                // operation, or at this block's terminator if none follows.
                pending_source.push(instruction.id);
                continue;
            }
            // The first emitted operation includes any required operand
            // materialization and is the statement's actual machine entry.
            // MIR addresses are globally unique and survive ordinary
            // optimizer rewrites as Op.at.
            for source_instruction in pending_source.iter().copied().chain([instruction.id]) {
                source_instructions.insert(source_instruction, made[0].at);
            }
            pending_source.clear();
            ops.extend(made);
        }
        let term = &source.terminator;
        let mut before: Vec<mir::Op> = Vec::new();
        let mut args = term
            .operands
            .iter()
            .map(|one| scope.operand(one, &mut before))
            .collect::<Result<Vec<_>, _>>()?;
        if term.kind == model::TerminatorKind::Return {
            let mut materialized = Vec::new();
            for (source_operand, argument) in term.operands.iter().zip(args) {
                let mut argument = argument;
                if let Arg::Const(_) = argument {
                    let type_id = match source_operand {
                        model::Operand::Constant(one) => Some(one.r#type),
                        _ => None,
                    };
                    let Some(type_id) = type_id else {
                        return Err(InvalidHIR(format!("{module}.{}: return constant lost its type", function.name)));
                    };
                    let held = scope.fresh(types[&type_id]);
                    scope.at += 1;
                    before.push(mir::Op {
                        kind: mir::Kind::Copy,
                        args: vec![argument],
                        results: vec![Arg::Held(held)],
                        id: Some(scope.at as u32),
                        reads_complete: true,
                        memory_complete: true,
                        ..mir::Op::new(scope.at, OpCode::Operation(Operation::Move), "mov", vec![held.value], Vec::new())
                    });
                    argument = Arg::Held(held);
                }
                materialized.push(argument);
            }
            args = materialized;
        }
        let before_first_at = before.first().map(|one| one.at);
        ops.extend(before);
        let mut uses: Vec<mir::Value> = args
            .iter()
            .filter_map(|one| match one {
                Arg::Held(one) => Some(one.value),
                _ => None,
            })
            .collect();
        let target = term.targets.first().map(|one| block_at[one]);
        let kind = match term.kind {
            model::TerminatorKind::Jump => mir::Kind::Jump,
            model::TerminatorKind::Branch => mir::Kind::Branch,
            model::TerminatorKind::Switch => mir::Kind::Switch,
            model::TerminatorKind::Return => mir::Kind::Return,
            model::TerminatorKind::Unreachable => mir::Kind::Escape,
        };
        let mut test = None;
        if term.kind == model::TerminatorKind::Branch {
            let Arg::Held(condition) = args[0].clone() else {
                return Err(InvalidHIR(format!("{module}.{}: branch condition must be a value", function.name)));
            };
            let source_value = &term.operands[0];
            let comparison = match source_value {
                model::Operand::ValueRef(one) => definitions.get(&one.value).copied(),
                _ => None,
            };
            let comparisons = |op: model::Op| match op {
                model::Op::Eq => Some(mir::Kind::Eq),
                model::Op::Ne => Some(mir::Kind::Ne),
                model::Op::Lt => Some(mir::Kind::Lt),
                model::Op::Le => Some(mir::Kind::Le),
                model::Op::Gt => Some(mir::Kind::Gt),
                model::Op::Ge => Some(mir::Kind::Ge),
                model::Op::Below => Some(mir::Kind::Below),
                model::Op::BelowEq => Some(mir::Kind::BelowEq),
                model::Op::Above => Some(mir::Kind::Above),
                model::Op::AboveEq => Some(mir::Kind::AboveEq),
                model::Op::StringEq => Some(mir::Kind::Eq),
                model::Op::StringNe => Some(mir::Kind::Ne),
                model::Op::StringLt => Some(mir::Kind::Lt),
                model::Op::StringLe => Some(mir::Kind::Le),
                model::Op::StringGt => Some(mir::Kind::Gt),
                model::Op::StringGe => Some(mir::Kind::Ge),
                _ => None,
            };
            let source_value_id = match source_value {
                model::Operand::ValueRef(one) => Some(one.value),
                _ => None,
            };
            let flags;
            if let Some(comparison) = comparison.filter(|comparison| {
                comparisons(comparison.op).is_some()
                    && source_value_id.and_then(|value| use_counts.get(&value).copied()) == Some(1)
            }) {
                let position = ops.iter().position(|operation_| {
                    operation_.results.iter().any(|result| matches!(result, Arg::Held(result) if result.value == condition.value))
                });
                let Some(position) = position else {
                    return Err(InvalidHIR(format!("{module}.{}: comparison crosses a block", function.name)));
                };
                let compared = ops[position].clone();
                flags = mir::Value {
                    id: scope.next_value as u32,
                    at: compared.at,
                    flags: true,
                    variable: scope.next_value as u32,
                    version: 1,
                };
                scope.next_value += 1;
                let floating_compare = comparison.operands.iter().any(|operand_| match operand_ {
                    model::Operand::ValueRef(one) => scope.value_types[&one.value].kind == model::TypeKind::Float,
                    _ => false,
                });
                if _STRING_COMPARISONS.contains(&comparison.op) {
                    ops[position] = mir::Op { defines: vec![flags], results: Vec::new(), ..compared };
                } else {
                    let compared_uses = compared
                        .args
                        .iter()
                        .filter_map(|argument| match argument {
                            Arg::Held(argument) => Some(argument.value),
                            _ => None,
                        })
                        .collect();
                    ops[position] = mir::Op {
                        op: Some(OpCode::Operation(Operation::Compare)),
                        name: String::new(),
                        defines: vec![flags],
                        uses: compared_uses,
                        kind: if floating_compare { mir::Kind::Fcompare } else { mir::Kind::Sub },
                        results: Vec::new(),
                        ..compared
                    };
                }
                test = comparisons(comparison.op);
            } else {
                flags = mir::Value {
                    id: scope.next_value as u32,
                    at: scope.at + 1,
                    flags: true,
                    variable: scope.next_value as u32,
                    version: 1,
                };
                scope.next_value += 1;
                scope.at += 1;
                ops.push(mir::Op {
                    kind: mir::Kind::Sub,
                    args: vec![Arg::Held(condition), _const(0, condition.width)],
                    id: Some(scope.at as u32),
                    reads_complete: true,
                    ..mir::Op::new(
                        scope.at,
                        OpCode::Operation(Operation::Compare),
                        "",
                        vec![flags],
                        vec![condition.value],
                    )
                });
                test = Some(mir::Kind::Ne);
            }
            args = Vec::new();
            uses = vec![flags];
        }
        scope.at += 1;
        let machine = match term.kind {
            model::TerminatorKind::Jump => Operation::Jump,
            model::TerminatorKind::Branch => Operation::Branch,
            model::TerminatorKind::Switch => Operation::Jump,
            model::TerminatorKind::Return => Operation::Return,
            model::TerminatorKind::Unreachable => Operation::Escape,
        };
        let terminal = mir::Op {
            kind,
            args,
            test,
            target,
            cases: term.cases.iter().map(|(value, label)| (*value, block_at[label])).collect(),
            id: Some(scope.at as u32),
            reads_complete: true,
            ..mir::Op::new(scope.at, OpCode::Operation(machine), "", Vec::new(), uses)
        };
        if !pending_source.is_empty() {
            let marker = before_first_at.unwrap_or(terminal.at);
            for source_instruction in &pending_source {
                source_instructions.insert(*source_instruction, marker);
            }
        }
        ops.push(terminal);
        let mut succ: Vec<i64> = Vec::new();
        for one in term.targets.iter().chain(term.cases.iter().map(|(_, target)| target)) {
            if !succ.contains(&block_at[one]) {
                succ.push(block_at[one]);
            }
        }
        blocks.push(mir::MirBlock { cold: source.cold, ..mir::MirBlock::new(block_at[&source.id], Vec::new(), ops, succ) });
    }
    let pointer_values: BTreeSet<mir::Value> = function
        .values
        .iter()
        .filter(|one| types[&one.r#type].kind == model::TypeKind::Pointer)
        .map(|one| scope.values[&one.id])
        .collect();
    let pointer_seeds: mir::OrderedMap<mir::Value, Provenance> = scope
        .parameter_numbers
        .iter()
        .filter(|(value, _)| scope.value_types[*value].kind == model::TypeKind::Pointer)
        .map(|(value, number)| {
            (
                scope.values[value],
                Provenance::one(MemoryObject {
                    identity: Some(Identity::Int(*number)),
                    ..MemoryObject::new(MemoryKind::Parameter)
                }),
            )
        })
        .collect();
    let body = mir::MirBody {
        sealed: true,
        pointer_values,
        pointer_seeds,
        integer_ranges: scope.integer_ranges,
        ..mir::MirBody::new(block_at[&function.entry], blocks)
    };
    let mut checked = body.clone();
    let mut external: Vec<i64> = Vec::new();
    for one in [body.entry]
        .into_iter()
        .chain(function.external_entries.iter().map(|one| block_at[one]))
        .chain(function.error_handler.map(|one| block_at[&one]))
    {
        if !external.contains(&one) {
            external.push(one);
        }
    }
    if external.len() > 1 {
        // Runtime-dispatched statement and ON ERROR entries are independent
        // roots. Verification gets an empty synthetic super-root; returning it
        // would falsify source control flow, so the lowered body remains exact.
        let root = body.blocks.iter().map(|block| block.at).max().expect("a function has a block") + 1;
        let mut blocks = vec![mir::MirBlock::new(root, Vec::new(), Vec::new(), external)];
        blocks.extend(body.blocks.iter().cloned());
        checked = mir::MirBody { entry: root, ..body.with_blocks(blocks) };
    }
    let problems = mir::verify(&checked);
    if !problems.is_empty() {
        let first: Vec<String> = problems.into_iter().take(3).collect();
        return Err(InvalidHIR(format!(
            "{module}.{}: invalid lowered MIR: {}",
            function.name,
            pyrepr::list(&first)
        )));
    }
    Ok(Lowered {
        name: format!("{module}.{}", function.name),
        body,
        values: scope.values,
        externals: Some(externals.clone()),
        source_instructions: Some(source_instructions),
    })
}

/// `function` with each branch on a constant the jump it takes, and
/// without the blocks that leaves unreachable.
fn _taken_branches(function: &model::Function) -> model::Function {
    let mut function = function.clone();
    for block in &mut function.blocks {
        block.terminator = taken(&block.terminator);
    }
    let roots = [function.entry]
        .into_iter()
        .chain(function.error_handler)
        .chain(function.external_entries.iter().copied());
    let mut reached: BTreeSet<i64> = BTreeSet::new();
    let mut work: Vec<i64> = roots.collect();
    while let Some(id) = work.pop() {
        if !reached.insert(id) {
            continue;
        }
        if let Some(block) = function.blocks.iter().find(|one| one.id == id) {
            work.extend(
                block
                    .terminator
                    .targets
                    .iter()
                    .chain(block.terminator.cases.iter().map(|(_, target)| target)),
            );
        }
    }
    function.blocks.retain(|one| reached.contains(&one.id));
    function
}

/// `terminator`, a branch on a constant being the jump it takes.
fn taken(terminator: &model::Terminator) -> model::Terminator {
    match (&terminator.kind, terminator.operands.as_slice()) {
        (
            model::TerminatorKind::Branch,
            [model::Operand::Constant(model::Constant { value, .. })],
        ) => {
            let taken = !matches!(value, model::Number::Int(0));
            let target = terminator.targets[usize::from(!taken)];
            model::Terminator::new(model::TerminatorKind::Jump, Vec::new(), vec![target])
        }
        _ => terminator.clone(),
    }
}
