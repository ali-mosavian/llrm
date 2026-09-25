//! Port of `qbopt/hir/verify.py`: structural and semantic checks at the
//! source/frontend boundary.

use crate::support::hash::HashSet;
use std::fmt;

use crate::support::hash::IndexMap;

use crate::hir::model;
use crate::support::pyrepr::{self, Repr};

/// HIR cannot be represented faithfully by the current MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidHIR(pub String);

impl fmt::Display for InvalidHIR {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for InvalidHIR {}

impl From<InvalidHIR> for String {
    fn from(error: InvalidHIR) -> Self {
        error.0
    }
}

macro_rules! invalid {
    ($($arg:tt)*) => {
        return Err(InvalidHIR(format!($($arg)*)))
    };
}

#[allow(non_snake_case)]
fn _RESULTS(op: model::Op) -> Option<Option<usize>> {
    match op {
        model::Op::Store => Some(Some(0)),
        model::Op::PortOut => Some(Some(0)),
        model::Op::Call | model::Op::Asm => Some(None),
        model::Op::Divmod => Some(Some(2)),
        model::Op::Udivmod => Some(Some(2)),
        _ => None,
    }
}

const _PLACES: [model::Op; 3] = [model::Op::Load, model::Op::Store, model::Op::Address];
const _FLOAT: [model::Op; 12] = [
    model::Op::Fadd,
    model::Op::Fsub,
    model::Op::Fmul,
    model::Op::Fdiv,
    model::Op::Fneg,
    model::Op::Fabs,
    model::Op::Fsqrt,
    model::Op::Fsin,
    model::Op::Fcos,
    model::Op::Fatan,
    model::Op::Flog2,
    model::Op::Fexp2,
];
const _INTEGER: [model::Op; 19] = [
    model::Op::Add,
    model::Op::Sub,
    model::Op::Mul,
    model::Op::FixedMul,
    model::Op::FixedDiv,
    model::Op::Div,
    model::Op::Rem,
    model::Op::Divmod,
    model::Op::Udiv,
    model::Op::Urem,
    model::Op::Udivmod,
    model::Op::And,
    model::Op::Or,
    model::Op::Xor,
    model::Op::Shl,
    model::Op::Shr,
    model::Op::Sar,
    model::Op::Neg,
    model::Op::Not,
];
const _COMPARE: [model::Op; 16] = [
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
pub(crate) const _STRING_COMPARE: [model::Op; 6] = [
    model::Op::StringEq,
    model::Op::StringNe,
    model::Op::StringLt,
    model::Op::StringLe,
    model::Op::StringGt,
    model::Op::StringGe,
];
const _UNSIGNED: [model::Op; 7] = [
    model::Op::Udiv,
    model::Op::Urem,
    model::Op::Udivmod,
    model::Op::Below,
    model::Op::BelowEq,
    model::Op::Above,
    model::Op::AboveEq,
];
const _POINTER_PART: [model::Op; 2] = [model::Op::PointerSegment, model::Op::PointerOffset];

fn _operand_type(
    operand: &model::Operand,
    values: &IndexMap<i64, &model::Value>,
    places: &IndexMap<i64, &model::Place>,
) -> Result<i64, InvalidHIR> {
    match operand {
        model::Operand::ValueRef(model::ValueRef { value }) => {
            if !values.contains_key(value) {
                invalid!("unknown value {value}");
            }
            Ok(values[value].r#type)
        }
        model::Operand::Constant(model::Constant { r#type, .. }) => Ok(*r#type),
        model::Operand::PlaceRef(model::PlaceRef { place }) => {
            if !places.contains_key(place) {
                invalid!("unknown place {place}");
            }
            Ok(places[place].r#type)
        }
        model::Operand::ArrayElement(model::ArrayElement { place, .. }) => {
            if !places.contains_key(place) {
                invalid!("unknown place {place}");
            }
            Ok(places[place].r#type)
        }
        model::Operand::ProjectedPlace(model::ProjectedPlace { place, r#type, .. }) => {
            if !places.contains_key(place) {
                invalid!("unknown place {place}");
            }
            Ok(*r#type)
        }
        model::Operand::IndirectPlace(model::IndirectPlace { base, r#type, origin, .. }) => {
            if !values.contains_key(base) {
                invalid!("unknown pointer value {base}");
            }
            if let Some(origin) = origin.filter(|origin| !values.contains_key(origin)) {
                invalid!("unknown origin value {origin}");
            }
            Ok(*r#type)
        }
        model::Operand::DescriptorPlace(model::DescriptorPlace { base, r#type, .. }) => {
            if !values.contains_key(base) {
                invalid!("unknown pointer value {base}");
            }
            Ok(*r#type)
        }
    }
}

pub fn verify(program: &model::Program) -> Result<(), InvalidHIR> {
    if program.schema != model::SCHEMA_VERSION {
        invalid!("unsupported HIR schema {}", program.schema);
    }
    if program.target != model::TargetProfile::I386RealMode {
        invalid!("unsupported target {}", program.target.repr());
    }
    let mut module_ids = HashSet::default();
    for module in &program.modules {
        if module_ids.contains(&module.id) {
            invalid!("duplicate module {}", module.id);
        }
        module_ids.insert(module.id);
        let data: IndexMap<i64, &model::DataObject> = module.data.iter().map(|one| (one.id, one)).collect();
        if data.len() != module.data.len() {
            invalid!("{}: duplicate data object id", module.name);
        }
        for object_ in &module.data {
            if object_.linkage == model::DataLinkage::External
                && (!object_.bytes.is_empty() || !object_.relocations.is_empty())
            {
                invalid!("{}: external {} has an initializer", module.name, object_.name);
            }
            if object_.bytes.iter().any(|byte| !(0..=255).contains(byte)) {
                invalid!("{}: {} has a non-byte initializer", module.name, object_.name);
            }
            for relocation in &object_.relocations {
                if relocation.code && !module.callables.iter().any(|one| one.id == relocation.target) {
                    invalid!("{}: {} relocates to unknown code", module.name, object_.name);
                }
                if !relocation.code && !data.contains_key(&relocation.target) {
                    invalid!("{}: {} relocates to unknown data", module.name, object_.name);
                }
                let width = if matches!(relocation.address, model::AddressKind::Far | model::AddressKind::Huge) {
                    4
                } else {
                    2
                };
                if relocation.at < 0 || relocation.at + width > object_.bytes.len() as i64 {
                    invalid!("{}: {} relocation exceeds initializer", module.name, object_.name);
                }
            }
        }
        let types: IndexMap<i64, &model::Type> = module.types.iter().map(|one| (one.id, one)).collect();
        if types.len() != module.types.len() {
            invalid!("{}: duplicate type id", module.name);
        }
        for type_ in &module.types {
            if type_.width < 0 {
                invalid!("{}: {} has negative width", module.name, type_.name);
            }
            if type_.kind == model::TypeKind::Float && type_.evaluation == model::FloatEvaluation::None {
                invalid!("{}: float {} has no evaluation format", module.name, type_.name);
            }
            if type_.kind == model::TypeKind::Array
                && (type_.element.is_none_or(|element| !types.contains_key(&element))
                    || type_.rank < 1
                    || type_.bounds.len() as i64 != type_.rank
                    || type_.bounds.iter().any(|(lower, upper)| upper < lower))
            {
                invalid!("{}: incomplete array type {}", module.name, type_.name);
            }
        }
        let callables: IndexMap<i64, &model::Callable> = module.callables.iter().map(|one| (one.id, one)).collect();
        if callables.len() != module.callables.len() {
            invalid!("{}: duplicate callable id", module.name);
        }
        for callable_ in &module.callables {
            if callable_.result_type.is_some_and(|result| !types.contains_key(&result)) {
                invalid!("{}: {} has an unknown result type", module.name, callable_.name);
            }
            let count = callable_.parameter_types.len();
            if callable_.parameter_types.iter().any(|one| !types.contains_key(one))
                || ![&callable_.by_value, &callable_.segmented, &callable_.arrays]
                    .iter()
                    .all(|one| one.len() == count)
            {
                invalid!("{}: {} has an incomplete signature", module.name, callable_.name);
            }
        }
        let mut function_ids = HashSet::default();
        for function in &module.functions {
            if function_ids.contains(&function.id) {
                invalid!("{}: duplicate function {}", module.name, function.id);
            }
            function_ids.insert(function.id);
            _function(module, function, &types)?;
        }
    }
    Ok(())
}

fn _function(
    module: &model::Module,
    function: &model::Function,
    types: &IndexMap<i64, &model::Type>,
) -> Result<(), InvalidHIR> {
    let prefix = format!("{}.{}", module.name, function.name);
    if !types.contains_key(&function.result_type) {
        invalid!("{prefix}: unknown result type {}", function.result_type);
    }
    let values: IndexMap<i64, &model::Value> = function.values.iter().map(|one| (one.id, one)).collect();
    let places: IndexMap<i64, &model::Place> = function.places.iter().map(|one| (one.id, one)).collect();
    let data: IndexMap<i64, &model::DataObject> = module.data.iter().map(|one| (one.id, one)).collect();
    let blocks: IndexMap<i64, &model::Block> = function.blocks.iter().map(|one| (one.id, one)).collect();
    let instructions: IndexMap<i64, &model::Instruction> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .map(|one| (one.id, one))
        .collect();
    if values.len() != function.values.len() {
        invalid!("{prefix}: duplicate value id");
    }
    if places.len() != function.places.len() {
        invalid!("{prefix}: duplicate place id");
    }
    if blocks.len() != function.blocks.len() {
        invalid!("{prefix}: duplicate block id");
    }
    if !blocks.contains_key(&function.entry) {
        invalid!("{prefix}: unknown entry block {}", function.entry);
    }
    if let Some(handler) = function.error_handler.filter(|handler| !blocks.contains_key(handler)) {
        invalid!("{prefix}: unknown error-handler block {handler}");
    }
    if function.error_handler_local && function.error_handler.is_none() {
        invalid!("{prefix}: local error-handler flag without a handler");
    }
    if function.external_entries.iter().any(|one| !blocks.contains_key(one)) {
        invalid!("{prefix}: unknown external-entry block");
    }
    if function.external_entries.iter().collect::<HashSet<_>>().len() != function.external_entries.len() {
        invalid!("{prefix}: duplicate external-entry block");
    }
    if function.abi.as_ref().is_some_and(|abi| abi.parameter_bytes < 0) {
        invalid!("{prefix}: negative ABI parameter size");
    }
    let call_sites: IndexMap<i64, &model::CallAbi> = function.calls.iter().map(|one| (one.instruction, one)).collect();
    if call_sites.len() != function.calls.len() {
        invalid!("{prefix}: duplicate call ABI");
    }
    for site in &function.calls {
        let instruction = instructions.get(&site.instruction);
        let Some(instruction) =
            instruction.filter(|one| one.op == model::Op::Call || _STRING_COMPARE.contains(&one.op))
        else {
            invalid!("{prefix}: ABI site {} is not a call", site.instruction);
        };
        let mut order = site.order.clone();
        order.sort();
        if order != (0..instruction.operands.len() as i64).collect::<Vec<_>>() {
            invalid!("{prefix}: call {} has invalid argument order", site.instruction);
        }
        let Some(callable) = site.callee.map(|callee| module.callables.iter().find(|one| one.id == callee)) else {
            continue;
        };
        let Some(callable) = callable else {
            invalid!("{prefix}: call {} names an unknown callable", site.instruction);
        };
        // A value passes in its parameter's representation: a near pointer is not a far one.
        if callable.parameter_types.len() == instruction.operands.len() {
            for (index, (operand, parameter)) in instruction.operands.iter().zip(&callable.parameter_types).enumerate() {
                let (model::Operand::ValueRef(_), true) = (operand, callable.by_value[index]) else {
                    continue;
                };
                let actual = _operand_type(operand, &values, &places)?;
                if types.get(&actual).map(|one| one.width) != types.get(parameter).map(|one| one.width) {
                    invalid!("{prefix}: call {} passes argument {index} in the wrong width", site.instruction);
                }
            }
        }
    }
    for value in values.values() {
        if !types.contains_key(&value.r#type) {
            invalid!("{prefix}: value {} has unknown type {}", value.id, value.r#type);
        }
    }
    for place in places.values() {
        if !types.contains_key(&place.r#type) {
            invalid!("{prefix}: place {} has unknown type {}", place.id, place.r#type);
        }
        let extent = match place.extent {
            Some(extent) if extent >= types[&place.r#type].width => extent,
            _ => invalid!("{prefix}: place {} has incomplete extent", place.name),
        };
        if !matches!(place.storage, model::Storage::Local | model::Storage::Parameter) {
            let Some(object_) = data.get(&place.symbol) else {
                invalid!("{prefix}: place {} has no data object", place.name);
            };
            if place.storage == model::Storage::External && object_.linkage != model::DataLinkage::External {
                invalid!("{prefix}: external place {} names a definition", place.name);
            }
            if place.storage != model::Storage::External && object_.linkage == model::DataLinkage::External {
                invalid!("{prefix}: defined place {} names an external declaration", place.name);
            }
            if object_.linkage != model::DataLinkage::External
                && (place.offset < 0 || place.offset + extent > object_.bytes.len() as i64)
            {
                invalid!("{prefix}: place {} exceeds its data object", place.name);
            }
        }
    }
    let mut defined: HashSet<i64> = HashSet::default();
    for block in &function.blocks {
        for instruction in &block.instructions {
            let expected = _RESULTS(instruction.op).unwrap_or(Some(1));
            if let Some(expected) = expected.filter(|expected| instruction.results.len() != *expected) {
                invalid!(
                    "{prefix}: {} has {} results, expected {expected}",
                    instruction.op,
                    instruction.results.len()
                );
            }
            if instruction.op == model::Op::Call && instruction.callee.as_deref().is_none_or(str::is_empty) {
                invalid!("{prefix}: call {} has no callee", instruction.id);
            }
            if (instruction.op == model::Op::Call || _STRING_COMPARE.contains(&instruction.op))
                && !call_sites.contains_key(&instruction.id)
            {
                invalid!("{prefix}: call {} has no ABI site", instruction.id);
            }
            if _STRING_COMPARE.contains(&instruction.op) && instruction.callee.as_deref() != Some("B$SCMP") {
                invalid!("{prefix}: string comparison is not B$SCMP");
            }
            if _PLACES.contains(&instruction.op) && instruction.operands.is_empty() {
                invalid!("{prefix}: {} {} has no place", instruction.op, instruction.id);
            }
            for result in &instruction.results {
                if !values.contains_key(result) {
                    invalid!("{prefix}: instruction {} defines unknown value {result}", instruction.id);
                }
                if defined.contains(result) {
                    invalid!("{prefix}: value {result} is defined twice");
                }
                defined.insert(*result);
            }
            let mut operand_types = instruction
                .operands
                .iter()
                .map(|one| _operand_type(one, &values, &places))
                .collect::<Result<Vec<_>, _>>()?;
            if operand_types.iter().any(|one| !types.contains_key(one)) {
                invalid!("{prefix}: instruction {} has an unknown operand type", instruction.id);
            }
            for operand in &instruction.operands {
                if let model::Operand::IndirectPlace(model::IndirectPlace { origin: Some(origin), .. }) = operand {
                    if types.get(&values[origin].r#type).is_none_or(|one| one.width != 2) {
                        invalid!("{prefix}: origin value {origin} is not a word offset");
                    }
                }
            }
            for (index, operand) in instruction.operands.iter().enumerate() {
                let type_id = operand_types[index];
                if let model::Operand::ArrayElement(_) = operand {
                    let Some(element) = types[&type_id].element else {
                        invalid!("{prefix}: array element has no element type");
                    };
                    operand_types[index] = element;
                }
            }
            let result_types: Vec<i64> = instruction.results.iter().map(|one| values[one].r#type).collect();
            if instruction.op == model::Op::Copy {
                let pointer_retype = operand_types.len() == 1
                    && result_types.len() == 1
                    && types[&operand_types[0]].kind == model::TypeKind::Pointer
                    && types[&result_types[0]].kind == model::TypeKind::Pointer
                    && types[&operand_types[0]].width == types[&result_types[0]].width;
                if operand_types.len() != 1 || (result_types != operand_types && !pointer_retype) {
                    invalid!("{prefix}: copy changes representation without a conversion");
                }
            }
            if instruction.op == model::Op::Load {
                if operand_types.len() != 1 || result_types != operand_types {
                    invalid!("{prefix}: load result type does not match its place");
                }
                if !matches!(
                    instruction.operands[0],
                    model::Operand::PlaceRef(_)
                        | model::Operand::ArrayElement(_)
                        | model::Operand::ProjectedPlace(_)
                        | model::Operand::IndirectPlace(_)
                        | model::Operand::DescriptorPlace(_)
                ) {
                    invalid!("{prefix}: load operand is not a place");
                }
            }
            if instruction.op == model::Op::Store {
                if operand_types.len() != 2 || operand_types[0] != operand_types[1] {
                    invalid!("{prefix}: store value type does not match its place: {operand_types:?}");
                }
                if !matches!(
                    instruction.operands[0],
                    model::Operand::PlaceRef(_)
                        | model::Operand::ArrayElement(_)
                        | model::Operand::ProjectedPlace(_)
                        | model::Operand::IndirectPlace(_)
                        | model::Operand::DescriptorPlace(_)
                ) {
                    invalid!("{prefix}: store destination is not a place");
                }
            }
            if _FLOAT.contains(&instruction.op) {
                let involved: Vec<i64> = instruction
                    .results
                    .iter()
                    .map(|one| values[one].r#type)
                    .chain(operand_types.iter().copied())
                    .collect();
                if involved.iter().any(|one| types[one].kind != model::TypeKind::Float) {
                    invalid!("{prefix}: {} has a non-floating operand", instruction.op);
                }
            }
            if (instruction.op == model::Op::Asm) != instruction.asm.is_some() {
                invalid!("{prefix}: only an asm instruction carries inline code");
            }
            if let Some(asm) = &instruction.asm {
                if asm.inputs.len() != operand_types.len() || asm.outputs.len() != result_types.len() {
                    invalid!("{prefix}: asm {} names a register for each operand and result", instruction.id);
                }
                // A register is its 16-bit whole; a frontend narrows or widens a part.
                let word = |one: &i64| types[one].width == 2 && types[one].kind == model::TypeKind::Integer;
                let near = |one: &i64| types[one].width == 2 && types[one].kind == model::TypeKind::Pointer;
                if !operand_types.iter().all(|one| word(one) || near(one)) || !result_types.iter().all(word) {
                    invalid!("{prefix}: asm {} moves only 16-bit integers and near pointers", instruction.id);
                }
            }
            if matches!(instruction.op, model::Op::PortIn | model::Op::PortOut) {
                let widths: Vec<i64> = operand_types.iter().map(|one| types[one].width).collect();
                let expected_widths: &[i64] = if instruction.op == model::Op::PortIn { &[2] } else { &[2, 1] };
                if operand_types.iter().chain(&result_types).any(|one| types[one].kind != model::TypeKind::Integer)
                    || widths != expected_widths
                    || result_types.iter().any(|one| types[one].width != 1)
                {
                    invalid!("{prefix}: {} is not a 16-bit port and a byte", instruction.op);
                }
            }
            if instruction.op == model::Op::Truncate
                && (operand_types.len() != 1
                    || result_types.len() != 1
                    || types[&operand_types[0]].kind != model::TypeKind::Float
                    || types[&result_types[0]].kind != model::TypeKind::Integer)
            {
                invalid!("{prefix}: truncate does not take a float to an integer");
            }
            if _INTEGER.contains(&instruction.op) {
                let involved: Vec<i64> = instruction
                    .results
                    .iter()
                    .map(|one| values[one].r#type)
                    .chain(operand_types.iter().copied())
                    .collect();
                if involved
                    .iter()
                    .any(|one| !matches!(types[one].kind, model::TypeKind::Integer | model::TypeKind::Boolean))
                {
                    invalid!("{prefix}: {} has a non-integer operand", instruction.op);
                }
            }
            if matches!(instruction.op, model::Op::FixedMul | model::Op::FixedDiv) {
                if result_types.len() != 1 || operand_types.len() != 3 {
                    invalid!("{prefix}: {} has the wrong arity", instruction.op);
                }
                let value_type = types[&result_types[0]];
                let (left_type, right_type, fraction_type) =
                    (types[&operand_types[0]], types[&operand_types[1]], types[&operand_types[2]]);
                let fraction = &instruction.operands[2];
                let fraction_in_range = match fraction {
                    model::Operand::Constant(model::Constant { value: model::Number::Int(value), .. }) => {
                        (1..32).contains(value)
                    }
                    model::Operand::Constant(model::Constant { value: model::Number::Float(value), .. }) => {
                        1.0 <= *value && *value < 32.0
                    }
                    _ => false,
                };
                if value_type.kind != model::TypeKind::Integer
                    || value_type.width != 4
                    || value_type.signed != Some(true)
                    || left_type != value_type
                    || right_type != value_type
                    || fraction_type.kind != model::TypeKind::Integer
                    || fraction_type.width != 1
                    || !fraction_in_range
                {
                    invalid!("{prefix}: {} is not fixed i32 arithmetic", instruction.op);
                }
            }
            if _COMPARE.contains(&instruction.op) {
                if result_types.len() != 1 || types[&result_types[0]].kind != model::TypeKind::Boolean {
                    invalid!("{prefix}: comparison does not produce a boolean");
                }
                if operand_types.len() != 2 {
                    invalid!("{prefix}: comparison does not have two operands");
                }
                if _STRING_COMPARE.contains(&instruction.op) {
                    // A stored dynamic-string array element is reached through
                    // a whole array pointer, but QB's string arena is near and
                    // its runtimes receive the extracted 16-bit descriptor
                    // offset. A normal scalar supplies a typed near pointer.
                    // Both are the same runtime address form; wider operands
                    // would lose the established real-mode contract.
                    if operand_types.iter().any(|one| types[one].width != 2) {
                        invalid!("{prefix}: string comparison operands are not near addresses");
                    }
                } else if operand_types[0] != operand_types[1] {
                    invalid!("{prefix}: comparison operand types do not agree");
                }
            }
            if _UNSIGNED.contains(&instruction.op) {
                let involved: Vec<i64> = if _COMPARE.contains(&instruction.op) {
                    operand_types.clone()
                } else {
                    result_types.iter().chain(operand_types.iter()).copied().collect()
                };
                let unsigned = involved
                    .iter()
                    .all(|one| types[one].kind == model::TypeKind::Integer && types[one].signed == Some(false));
                if !unsigned {
                    invalid!("{prefix}: {} requires unsigned integer operands", instruction.op);
                }
            }
            if _POINTER_PART.contains(&instruction.op) {
                if operand_types.len() != 1 || result_types.len() != 1 {
                    invalid!("{prefix}: pointer projection has the wrong arity");
                }
                let pointer = types[&operand_types[0]];
                let result = types[&result_types[0]];
                if pointer.kind != model::TypeKind::Pointer
                    || (instruction.op == model::Op::PointerSegment && pointer.width != 4)
                    || (instruction.op == model::Op::PointerOffset && !matches!(pointer.width, 2 | 4))
                    || result.kind != model::TypeKind::Integer
                    || result.width != 2
                {
                    invalid!("{prefix}: pointer projection cannot produce INTEGER");
                }
            }
            if instruction.op == model::Op::Concat {
                if operand_types.len() != 2 || result_types.len() != 1 {
                    invalid!("{prefix}: pointer concat has the wrong arity");
                }
                let (high, low) = (types[&operand_types[0]], types[&operand_types[1]]);
                let result = types[&result_types[0]];
                if high.kind != model::TypeKind::Integer
                    || low.kind != model::TypeKind::Integer
                    || high.width != 2
                    || low.width != 2
                    || result.kind != model::TypeKind::Pointer
                    || result.width != 4
                {
                    invalid!("{prefix}: pointer concat is not INTEGER:INTEGER to 16:16");
                }
            }
            for operand in &instruction.operands {
                if let model::Operand::ArrayElement(operand) = operand {
                    let array = types[&places[&operand.place].r#type];
                    if array.kind != model::TypeKind::Array || operand.indices.len() as i64 != array.rank {
                        invalid!("{prefix}: invalid array element for place {}", operand.place);
                    }
                    for index in &operand.indices {
                        let index_type = types[&_operand_type(index, &values, &places)?];
                        if index_type.kind != model::TypeKind::Integer {
                            invalid!("{prefix}: array index is not an integer");
                        }
                    }
                }
                if let model::Operand::ProjectedPlace(operand) = operand {
                    let root = types[&places[&operand.place].r#type];
                    let container = if root.kind == model::TypeKind::Array {
                        types[&root.element.expect("a verified array has an element")]
                    } else {
                        root
                    };
                    if !types.contains_key(&operand.r#type) || operand.offset < 0 {
                        invalid!("{prefix}: invalid projection for place {}", operand.place);
                    }
                    if operand.offset + types[&operand.r#type].width > container.width {
                        invalid!("{prefix}: projection exceeds place {}", operand.place);
                    }
                    let expected_rank = if root.kind == model::TypeKind::Array { root.rank } else { 0 };
                    if operand.indices.len() as i64 != expected_rank {
                        invalid!("{prefix}: invalid projection rank for place {}", operand.place);
                    }
                    for index in &operand.indices {
                        let index_type = types[&_operand_type(index, &values, &places)?];
                        if index_type.kind != model::TypeKind::Integer {
                            invalid!("{prefix}: projection index is not an integer");
                        }
                    }
                }
                if let model::Operand::IndirectPlace(operand) = operand {
                    if !types.contains_key(&operand.r#type) || operand.offset < 0 {
                        invalid!("{prefix}: invalid indirect place");
                    }
                    let pointer = types[&values[&operand.base].r#type];
                    let Some(element) = pointer
                        .element
                        .filter(|element| pointer.kind == model::TypeKind::Pointer && types.contains_key(element))
                    else {
                        invalid!("{prefix}: indirect place disagrees with pointer type");
                    };
                    if operand.offset + types[&operand.r#type].width > types[&element].width {
                        invalid!("{prefix}: indirect place exceeds its pointee");
                    }
                }
                if let model::Operand::DescriptorPlace(operand) = operand {
                    let pointer = types[&values[&operand.base].r#type];
                    let field = types[&operand.r#type];
                    if pointer.kind != model::TypeKind::Pointer
                        || pointer.element.is_none_or(|element| !types.contains_key(&element))
                    {
                        invalid!("{prefix}: descriptor place needs a sequence pointer");
                    }
                    if field.kind != model::TypeKind::Integer || field.width != 2 || field.signed != Some(false) {
                        invalid!("{prefix}: descriptor field is not u16");
                    }
                }
            }
        }
        let term = &block.terminator;
        if term
            .targets
            .iter()
            .chain(term.cases.iter().map(|(_, target)| target))
            .any(|target| !blocks.contains_key(target))
        {
            invalid!("{prefix}: block {} has an unknown target", block.id);
        }
        let result = types[&function.result_type];
        let operand_count = match term.kind {
            model::TerminatorKind::Jump => 0,
            model::TerminatorKind::Branch => 1,
            model::TerminatorKind::Switch => 1,
            model::TerminatorKind::Return => usize::from(result.kind != model::TypeKind::Void),
            model::TerminatorKind::Unreachable => 0,
        };
        let target_count = match term.kind {
            model::TerminatorKind::Jump => 1,
            model::TerminatorKind::Branch => 2,
            model::TerminatorKind::Switch => 1,
            model::TerminatorKind::Return => 0,
            model::TerminatorKind::Unreachable => 0,
        };
        if term.operands.len() != operand_count || term.targets.len() != target_count {
            invalid!("{prefix}: malformed {} terminator in block {}", term.kind, block.id);
        }
        for operand in &term.operands {
            let type_id = _operand_type(operand, &values, &places)?;
            if !types.contains_key(&type_id) {
                invalid!("{prefix}: terminator in block {} has an unknown operand type", block.id);
            }
        }
        if term.kind == model::TerminatorKind::Return
            && !term.operands.is_empty()
            && _operand_type(&term.operands[0], &values, &places)? != function.result_type
        {
            invalid!("{prefix}: return value type does not match the function");
        }
        if matches!(term.kind, model::TerminatorKind::Branch | model::TerminatorKind::Switch) {
            let condition = types[&_operand_type(&term.operands[0], &values, &places)?];
            if !matches!(condition.kind, model::TypeKind::Boolean | model::TypeKind::Integer) {
                invalid!("{prefix}: {} condition is not integral", term.kind);
            }
        }
    }
    let parameters: HashSet<i64> = function.parameters.iter().copied().collect();
    let invalid_parameters =
        parameters.len() != function.parameters.len() || function.parameters.iter().any(|one| !values.contains_key(one));
    if invalid_parameters {
        invalid!("{prefix}: invalid parameter values");
    }
    let undefined: HashSet<i64> = values.keys().copied().filter(|one| !defined.contains(one)).collect();
    if undefined != parameters {
        let mut missing: Vec<i64> = undefined.difference(&parameters).copied().collect();
        missing.sort();
        invalid!("{prefix}: undefined values {}", pyrepr::list(&missing));
    }
    Ok(())
}
