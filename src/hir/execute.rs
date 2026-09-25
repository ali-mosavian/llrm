//! Port of `qbopt/hir/execute.py`: small executable reference semantics for
//! verified common HIR.
//!
//! This is an oracle for frontend and target-backend tests, not a target
//! runtime or an alternative optimization path. It executes the typed
//! operations the frontend committed to at the HIR boundary and makes no
//! machine decisions.
//!
//! Deviations from Python: integers are i128 wrapped to their declared width
//! (exact for widths up to 8 bytes); f32 overflow rounds to infinity where
//! Python's `struct` raises; ZERO_EXTEND and SIGN_EXTEND reinterpret the
//! source at its own width; addresses compare by identity, not content.

use crate::abi::nib as rt;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::hir::model::{self, Number, Op, Operand, Storage, TerminatorKind, TypeKind};
use crate::hir::verify::{InvalidHIR, verify};
use crate::support::hash::HashMap;
use crate::support::pyrepr;

pub const STEP_LIMIT: u64 = 10_000_000;
/// What the DOS runtime's INT 0 handler prints.
const DIVIDE_FAULT: &str = "division by zero or overflow";

/// Verified HIR uses an operation the reference executor cannot run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionError(pub String);

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExecutionError {}

impl From<InvalidHIR> for ExecutionError {
    fn from(error: InvalidHIR) -> Self {
        Self(error.0)
    }
}

/// Python's `execute.Result`.
#[derive(Clone, Debug, PartialEq)]
pub struct Executed {
    pub output: String,
    pub value: Option<Number>,
    /// Heap buffers the program allocated and never dropped.
    pub leaked: usize,
    /// What the program panicked with; `output` is what it printed first.
    pub panic: Option<String>,
}

type Outcome<T> = Result<T, ExecutionError>;

fn fail<T>(message: impl Into<String>) -> Outcome<T> {
    Err(ExecutionError(message.into()))
}

/// Execute one function and return its captured output and result value.
pub fn run(program: &model::Program, entry: &str, arguments: &[Number]) -> Outcome<Executed> {
    run_limited(program, entry, arguments, STEP_LIMIT)
}

/// `run` with an explicit bound on executed blocks.
pub fn run_limited(
    program: &model::Program,
    entry: &str,
    arguments: &[Number],
    step_limit: u64,
) -> Outcome<Executed> {
    let mut machine = Machine::new(program, step_limit)?;
    let arguments = arguments
        .iter()
        .map(|one| match *one {
            Number::Int(value) => Scalar::Int(i128::from(value)),
            Number::Float(value) => Scalar::Float(value),
        })
        .collect();
    let value = match machine.invoke(entry, arguments) {
        Err(_) if machine.ended => None,
        Err(_) if machine.panicked.is_some() => {
            return Ok(Executed {
                output: machine.output,
                value: None,
                leaked: 0,
                panic: machine.panicked,
            });
        }
        other => other?,
    };
    let value = match value {
        None => None,
        Some(Scalar::Int(value)) => match i64::try_from(value) {
            Ok(value) => Some(Number::Int(value)),
            Err(_) => return fail(format!("{entry}: result {value} does not fit i64")),
        },
        Some(Scalar::Float(value)) => Some(Number::Float(value)),
        Some(Scalar::Address(_)) => return fail(format!("{entry} returned an address")),
    };
    let leaked = machine.leaked();
    Ok(Executed {
        output: machine.output,
        value,
        leaked,
        panic: None,
    })
}

type Memory = Rc<RefCell<Cells>>;

#[derive(Default)]
struct Cells {
    bytes: Vec<u8>,
    // Host bytes have no segmented numeric address, so a stored pointer keeps
    // its address object here, keyed by its cell's (offset, width).
    pointers: HashMap<(i64, i64), Address>,
    /// A returned call's frame: a pointer into it dangles.
    dead: bool,
}

fn memory(size: i64) -> Memory {
    Rc::new(RefCell::new(Cells {
        bytes: vec![0; usize::try_from(size).unwrap_or(0)],
        pointers: HashMap::default(),
        dead: false,
    }))
}

#[derive(Clone)]
struct Address {
    memory: Memory,
    offset: i64,
    length: Option<i64>,
    capacity: Option<i64>,
}

#[derive(Clone)]
enum Scalar {
    Int(i128),
    Float(f64),
    Address(Address),
}

impl Scalar {
    fn float(&self) -> Outcome<f64> {
        match self {
            Self::Int(value) => Ok(*value as f64),
            Self::Float(value) => Ok(*value),
            Self::Address(_) => fail("address used as a numeric value"),
        }
    }

    /// Python's `int(value)`.
    fn whole(&self) -> Outcome<i128> {
        match self {
            Self::Int(value) => Ok(*value),
            Self::Float(value) => {
                if !value.is_finite() {
                    return fail(format!(
                        "cannot convert {} to an integer",
                        pyrepr::float(*value)
                    ));
                }
                // From 2**120 a double is a multiple of 2**68: zero at every width here.
                Ok(if value.abs() >= 2f64.powi(120) {
                    0
                } else {
                    value.trunc() as i128
                })
            }
            Self::Address(_) => fail("address used as a numeric value"),
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Self::Int(value) => *value != 0,
            Self::Float(value) => *value != 0.0,
            Self::Address(_) => true,
        }
    }
}

struct Location<'p> {
    memory: Memory,
    offset: i64,
    type_: &'p model::Type,
}

/// `value` in two's complement at `bits` bits.
/// A comparison's result: `true` is all ones, as the machine materializes it.
fn truth(value: bool) -> Scalar {
    Scalar::Int(-i128::from(value))
}

fn wrap(value: i128, bits: i64, signed: bool) -> i128 {
    if bits >= 128 {
        return value;
    }
    let masked = value & ((1i128 << bits) - 1);
    if signed && (masked >> (bits - 1)) & 1 != 0 {
        masked - (1i128 << bits)
    } else {
        masked
    }
}

fn integer(value: i128, type_: &model::Type) -> i128 {
    wrap(value, type_.width * 8, type_.signed == Some(true))
}

fn unsigned(value: i128, type_: &model::Type) -> i128 {
    wrap(value, type_.width * 8, false)
}

fn normalized(value: Scalar, type_: &model::Type) -> Outcome<Scalar> {
    match type_.kind {
        TypeKind::Pointer => match value {
            // Null, as a moved-from string holds.
            Scalar::Address(_) | Scalar::Int(0) => Ok(value),
            _ => fail(format!("{}: expected an address", type_.name)),
        },
        TypeKind::Float => {
            let number = value.float()?;
            Ok(Scalar::Float(if type_.width == 4 {
                f64::from(number as f32)
            } else {
                number
            }))
        }
        // A word copy of an aggregate carries its addresses.
        TypeKind::Integer if matches!(value, Scalar::Address(_)) => Ok(value),
        // Widened as `movsx` widens it: the all-ones `true` is -1.
        TypeKind::Boolean => Ok(Scalar::Int(wrap(value.whole()?, type_.width * 8, true))),
        TypeKind::Integer => Ok(Scalar::Int(integer(value.whole()?, type_))),
        _ => fail(format!(
            "{}: aggregate values are not first-class",
            type_.name
        )),
    }
}

/// The truncated quotient, or `None` where the machine's divide faults:
/// a zero divisor, or a quotient `type_` cannot hold.
fn trunc_div(left: i128, right: i128, type_: &model::Type) -> Option<i128> {
    let quotient = left.checked_div(right)?;
    (integer(quotient, type_) == quotient).then_some(quotient)
}

fn fixed_text(raw: i128, fraction: i128) -> Outcome<String> {
    let Some(power) = u32::try_from(fraction)
        .ok()
        .and_then(|one| 5u128.checked_pow(one))
    else {
        return fail(format!("fixed fraction {fraction} is out of range"));
    };
    let fraction = fraction as u32;
    let magnitude = raw.unsigned_abs();
    let (whole, remainder) = (magnitude >> fraction, magnitude & ((1u128 << fraction) - 1));
    let digits = if remainder == 0 {
        "0".to_owned()
    } else {
        let Some(scaled) = remainder.checked_mul(power) else {
            return fail(format!("fixed fraction {fraction} is out of range"));
        };
        let padded = format!("{scaled:0>width$}", width = fraction as usize);
        padded.trim_end_matches('0').to_owned()
    };
    Ok(format!(
        "{}{whole}.{digits}",
        if raw < 0 { "-" } else { "" }
    ))
}

/// `bytes` as the screen shows them.
fn cp437(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| crate::support::codepage::decoded(*byte)).collect()
}

/// A function's static tables, built once per function.
struct Layout<'p> {
    value_types: HashMap<i64, &'p model::Type>,
    places: HashMap<i64, &'p model::Place>,
    blocks: HashMap<i64, &'p model::Block>,
    base: i64,
    size: i64,
}

struct Activation<'p> {
    name: &'p str,
    layout: Rc<Layout<'p>>,
    values: HashMap<i64, Scalar>,
    // One frame, so places that overlap share their bytes.
    frame: Memory,
    locals: HashMap<i64, Memory>,
}

/// A call's frame and locals, dead once it returns.
struct Expiring(Vec<Memory>);

impl Drop for Expiring {
    fn drop(&mut self) {
        for memory in &self.0 {
            memory.borrow_mut().dead = true;
        }
    }
}

struct Machine<'p> {
    program: &'p model::Program,
    types: HashMap<i64, &'p model::Type>,
    functions: HashMap<&'p str, &'p model::Function>,
    layouts: HashMap<&'p str, Rc<Layout<'p>>>,
    data: HashMap<i64, Memory>,
    output: String,
    remaining: u64,
    /// Every heap buffer the runtime allocated.
    heap: Vec<Memory>,
    /// The string an f-string is building, which formatters write into.
    sink: Option<Address>,
    /// The field the next formatted value fills: width, radix, fill, left-aligned.
    field: Option<(usize, u32, u8, bool)>,
    /// Set by a runtime panic routine, which ends the program.
    panicked: Option<String>,
    /// Open files by DOS handle, less the five DOS opens for every program.
    files: Vec<Option<std::fs::File>>,
    /// The QB console's cursor column, for PRINT's comma and TAB.
    column: usize,
    /// A QB END stopped the program.
    ended: bool,
}

impl<'p> Machine<'p> {
    fn new(program: &'p model::Program, limit: u64) -> Outcome<Self> {
        verify(program)?;
        let [module] = program.modules.as_slice() else {
            return fail("the reference executor accepts one HIR module");
        };
        let data = module
            .data
            .iter()
            .map(|one| {
                let bytes = one.bytes.iter().map(|byte| *byte as u8).collect();
                (
                    one.id,
                    Rc::new(RefCell::new(Cells {
                        bytes,
                        pointers: HashMap::default(),
                        dead: false,
                    })),
                )
            })
            .collect::<HashMap<i64, Memory>>();
        // A near or far relocation stores its target's address in the cell.
        for object in &module.data {
            for relocation in object.relocations.iter().filter(|one| !one.code) {
                let width = match relocation.address {
                    model::AddressKind::Near => 2,
                    model::AddressKind::Far => 4,
                    _ => continue,
                };
                let Some(target) = data.get(&relocation.target).cloned() else {
                    continue;
                };
                let address = Address { memory: target, offset: relocation.addend, length: None, capacity: None };
                data[&object.id].borrow_mut().pointers.insert((relocation.at, width), address);
            }
        }
        Ok(Self {
            program,
            types: module.types.iter().map(|one| (one.id, one)).collect(),
            functions: module
                .functions
                .iter()
                .map(|one| (one.name.as_str(), one))
                .collect(),
            layouts: HashMap::default(),
            data,
            output: String::new(),
            panicked: None,
            remaining: limit,
            heap: Vec::new(),
            sink: None,
            field: None,
            files: Vec::new(),
            column: 0,
            ended: false,
        })
    }

    fn layout(&mut self, function: &'p model::Function) -> Rc<Layout<'p>> {
        if let Some(layout) = self.layouts.get(function.name.as_str()) {
            return layout.clone();
        }
        let framed: Vec<&model::Place> = function
            .places
            .iter()
            .filter(|one| matches!(one.storage, Storage::Local | Storage::Parameter))
            .collect();
        let base = framed.iter().map(|one| one.offset).min().unwrap_or(0);
        let size = framed
            .iter()
            .map(|one| one.offset + one.extent.unwrap_or(0) - base)
            .max()
            .unwrap_or(0);
        let layout = Rc::new(Layout {
            value_types: function
                .values
                .iter()
                .map(|one| (one.id, self.types[&one.r#type]))
                .collect(),
            places: function.places.iter().map(|one| (one.id, one)).collect(),
            blocks: function.blocks.iter().map(|one| (one.id, one)).collect(),
            base,
            size,
        });
        self.layouts.insert(function.name.as_str(), layout.clone());
        layout
    }

    fn invoke(&mut self, name: &str, arguments: Vec<Scalar>) -> Outcome<Option<Scalar>> {
        let Some(&function) = self.functions.get(name) else {
            return fail(format!("unknown entry function {name:?}"));
        };
        if arguments.len() != function.parameters.len() {
            return fail(format!(
                "{name}: expected {} arguments, received {}",
                function.parameters.len(),
                arguments.len()
            ));
        }
        let layout = self.layout(function);
        let mut values = HashMap::default();
        for (value, argument) in function.parameters.iter().zip(arguments) {
            values.insert(*value, normalized(argument, layout.value_types[value])?);
        }
        let locals = function
            .places
            .iter()
            .filter(|one| {
                !matches!(
                    one.storage,
                    Storage::Module | Storage::Local | Storage::Parameter
                )
            })
            .map(|one| {
                (
                    one.id,
                    memory(one.extent.unwrap_or(self.types[&one.r#type].width)),
                )
            })
            .collect();
        let mut activation = Activation {
            name: &function.name,
            frame: memory(layout.size),
            layout,
            values,
            locals,
        };
        let _expires = Expiring(std::iter::once(&activation.frame).chain(activation.locals.values()).cloned().collect());
        let mut current = function.entry;
        loop {
            if self.remaining == 0 {
                return fail("execution step limit exceeded");
            }
            self.remaining -= 1;
            let block = activation.layout.blocks[&current];
            for instruction in &block.instructions {
                self.execute(&mut activation, instruction)?;
            }
            let terminator = &block.terminator;
            current = match terminator.kind {
                TerminatorKind::Jump => terminator.targets[0],
                TerminatorKind::Branch => {
                    terminator.targets[if self
                        .scalar(&activation, &terminator.operands[0])?
                        .truthy()
                    {
                        0
                    } else {
                        1
                    }]
                }
                TerminatorKind::Switch => {
                    let key = self.scalar(&activation, &terminator.operands[0])?.whole()?;
                    let case = terminator
                        .cases
                        .iter()
                        .rev()
                        .find(|(value, _)| i128::from(*value) == key);
                    case.map_or(terminator.targets[0], |(_, target)| *target)
                }
                TerminatorKind::Return => {
                    let Some(operand) = terminator.operands.first() else {
                        return Ok(None);
                    };
                    // An address returns too: a string is its data pointer.
                    return Ok(Some(self.scalar(&activation, operand)?));
                }
                TerminatorKind::Unreachable => {
                    return fail(format!("{name}: reached unreachable control flow"));
                }
            };
        }
    }

    fn operand_type(
        &self,
        activation: &Activation<'p>,
        operand: &Operand,
    ) -> Outcome<&'p model::Type> {
        match operand {
            Operand::Constant(constant) => Ok(self.types[&constant.r#type]),
            Operand::ValueRef(value) => Ok(activation.layout.value_types[&value.value]),
            _ => Ok(self.location(activation, operand)?.type_),
        }
    }

    fn value(&self, activation: &Activation<'p>, value: i64) -> Outcome<Scalar> {
        match activation.values.get(&value) {
            Some(one) => Ok(one.clone()),
            None => fail(format!(
                "{}: value {value} used before definition",
                activation.name
            )),
        }
    }

    fn scalar(&self, activation: &Activation<'p>, operand: &Operand) -> Outcome<Scalar> {
        match operand {
            Operand::Constant(constant) => {
                let value = match constant.value {
                    Number::Int(one) => Scalar::Int(i128::from(one)),
                    Number::Float(one) => Scalar::Float(one),
                };
                normalized(value, self.types[&constant.r#type])
            }
            Operand::ValueRef(value) => self.value(activation, value.value),
            Operand::DescriptorPlace(descriptor) => {
                let Some(Scalar::Address(address)) = activation.values.get(&descriptor.base) else {
                    return fail("descriptor place has no address value");
                };
                let length = descriptor.field == model::DescriptorField::Length;
                match if length { address.length } else { address.capacity } {
                    Some(value) if !self.scoped_view(activation, descriptor) => {
                        normalized(Scalar::Int(i128::from(value)), self.types[&descriptor.r#type])
                    }
                    _ => load(&self.descriptor_location(activation, descriptor)?),
                }
            }
            _ => load(&self.location(activation, operand)?),
        }
    }

    fn scoped_view(&self, activation: &Activation<'p>, descriptor: &model::DescriptorPlace) -> bool {
        activation.layout.value_types[&descriptor.base]
            .element
            .map(|one| self.types[&one])
            .is_some_and(|one| one.kind == TypeKind::Opaque && one.name.starts_with("$slice["))
    }

    /// The descriptor word `descriptor` names: a scoped view's own words, or
    /// the header before an owned buffer's data.
    fn descriptor_location(&self, activation: &Activation<'p>, descriptor: &model::DescriptorPlace) -> Outcome<Location<'p>> {
        let Some(Scalar::Address(address)) = activation.values.get(&descriptor.base) else {
            return fail("descriptor place has no address value");
        };
        let length = descriptor.field == model::DescriptorField::Length;
        let offset = match (self.scoped_view(activation, descriptor), length) {
            (true, true) => 0,
            (true, false) => 2,
            (false, true) => -4,
            (false, false) => -2,
        };
        Ok(Location { memory: address.memory.clone(), offset: address.offset + offset, type_: self.types[&descriptor.r#type] })
    }

    fn location(&self, activation: &Activation<'p>, operand: &Operand) -> Outcome<Location<'p>> {
        let (place, indices, projection) = match operand {
            Operand::IndirectPlace(indirect) => {
                let Some(Scalar::Address(address)) = activation.values.get(&indirect.base) else {
                    return fail("indirect place has no address value");
                };
                return Ok(Location {
                    memory: address.memory.clone(),
                    offset: address.offset + indirect.offset,
                    type_: self.types[&indirect.r#type],
                });
            }
            Operand::PlaceRef(one) => (one.place, None, None),
            Operand::ArrayElement(one) => (one.place, Some(&one.indices), None),
            Operand::ProjectedPlace(one) => (
                one.place,
                Some(&one.indices),
                Some((one.offset, self.types[&one.r#type])),
            ),
            Operand::DescriptorPlace(descriptor) => {
                return self.descriptor_location(activation, descriptor);
            }
            _ => return fail("operand is not a place"),
        };
        let place = activation.layout.places[&place];
        let (memory, offset) = match place.storage {
            Storage::Module => match self.data.get(&place.symbol) {
                Some(memory) => (memory.clone(), place.offset),
                None => {
                    return fail(format!(
                        "{}: unknown module object {}",
                        place.name, place.symbol
                    ));
                }
            },
            Storage::Local | Storage::Parameter => (
                activation.frame.clone(),
                place.offset - activation.layout.base,
            ),
            // A static backed by a data object, such as a float literal,
            // holds that object's bytes and keeps them across calls.
            Storage::Static if self.data.contains_key(&place.symbol) => {
                (self.data[&place.symbol].clone(), place.offset)
            }
            _ => (activation.locals[&place.id].clone(), 0),
        };
        let array = self.types[&place.r#type];
        let Some(indices) = indices else {
            return Ok(Location {
                memory,
                offset,
                type_: array,
            });
        };
        if let (Some((extra, type_)), false) = (projection, array.kind == TypeKind::Array) {
            if !indices.is_empty() {
                return fail(format!("{}: non-array projection has indices", place.name));
            }
            if extra + type_.width > array.width {
                return fail(format!("{}: projection exceeds its place", place.name));
            }
            return Ok(Location {
                memory,
                offset: offset + extra,
                type_,
            });
        }
        let (TypeKind::Array, Some(element)) = (array.kind, array.element) else {
            return fail(format!("{}: indexed place is not an array", place.name));
        };
        if indices.len() != array.bounds.len() {
            return fail(format!("{}: index rank mismatch", place.name));
        }
        let mut adjusted = Vec::with_capacity(indices.len());
        for (index, (low, high)) in indices.iter().zip(&array.bounds) {
            let Scalar::Int(index) = self.scalar(activation, index)? else {
                return fail("array index is not an integer");
            };
            let (index, extent) = (index - i128::from(*low), i128::from(high - low + 1));
            if index < 0 || index >= extent {
                return fail(format!("{}: array index out of bounds", place.name));
            }
            adjusted.push((index, extent));
        }
        let mut linear = 0i128;
        if self.program.array_order == model::ArrayOrder::RowMajor {
            for (index, extent) in adjusted {
                linear = linear * extent + index;
            }
        } else {
            let mut stride = 1i128;
            for (index, extent) in adjusted {
                linear += index * stride;
                stride *= extent;
            }
        }
        let element = self.types[&element];
        let (extra, type_) = projection.unwrap_or((0, element));
        let offset = i128::from(offset) + linear * i128::from(element.width) + i128::from(extra);
        Ok(Location {
            memory,
            offset: offset as i64,
            type_,
        })
    }

    fn define(
        &self,
        activation: &mut Activation<'p>,
        instruction: &model::Instruction,
        results: Vec<Scalar>,
    ) -> Outcome<()> {
        if results.len() != instruction.results.len() {
            return fail(format!("{}: result count mismatch", instruction.op));
        }
        for (value, result) in instruction.results.iter().zip(results) {
            let result = normalized(result, activation.layout.value_types[value])?;
            activation.values.insert(*value, result);
        }
        Ok(())
    }

    fn execute(
        &mut self,
        activation: &mut Activation<'p>,
        instruction: &model::Instruction,
    ) -> Outcome<()> {
        let op = instruction.op;
        let operands = &instruction.operands;
        match op {
            Op::Load => {
                let value = self.scalar(activation, &operands[0])?;
                return self.define(activation, instruction, vec![value]);
            }
            Op::Store => {
                let value = self.scalar(activation, &operands[1])?;
                return store(&self.location(activation, &operands[0])?, value);
            }
            Op::Address => {
                let where_ = self.location(activation, &operands[0])?;
                let mut extent = None;
                if let Operand::PlaceRef(place) = &operands[0] {
                    let type_ = self.types[&activation.layout.places[&place.place].r#type];
                    if let (TypeKind::Array, [(low, high)]) = (type_.kind, type_.bounds.as_slice())
                    {
                        extent = Some(high - low + 1);
                    }
                }
                let address = Address {
                    memory: where_.memory,
                    offset: where_.offset,
                    length: extent,
                    capacity: extent,
                };
                return self.define(activation, instruction, vec![Scalar::Address(address)]);
            }
            _ => {}
        }
        let args = operands
            .iter()
            .map(|one| self.scalar(activation, one))
            .collect::<Outcome<Vec<_>>>()?;
        let operand_type = |index: usize| self.operand_type(activation, &operands[index]);
        let results = match op {
            Op::PtrOffset => {
                let (Scalar::Address(address), Scalar::Int(displacement)) = (&args[0], &args[1])
                else {
                    return fail("pointer offset requires an address and an integer");
                };
                let mut address = address.clone();
                address.offset += *displacement as i64;
                vec![Scalar::Address(address)]
            }
            // Pointers into one object order by their offsets in it.
            Op::PointerOffset => {
                let Scalar::Address(address) = &args[0] else {
                    return fail("pointer_offset requires an address");
                };
                vec![Scalar::Int(i128::from(address.offset))]
            }
            Op::Asm => return fail("inline assembly is machine code: it does not run on the host"),
            Op::Call => {
                let returned = self.call(instruction.callee.as_deref(), args)?;
                match (returned, instruction.results.is_empty()) {
                    (_, true) => vec![],
                    (Some(value), false) => vec![value],
                    (None, false) => {
                        return fail(format!(
                            "{:?}: call did not return a value",
                            instruction.callee
                        ));
                    }
                }
            }
            // CONVERT rounds as the x87 does by default: to nearest, ties to even.
            Op::Convert => vec![match (&args[0], activation.layout.value_types[&instruction.results[0]].kind) {
                (Scalar::Float(value), TypeKind::Integer | TypeKind::Boolean) => Scalar::Float(value.round_ties_even()),
                (other, _) => other.clone(),
            }],
            Op::Copy => vec![args[0].clone()],
            Op::ZeroExtend => vec![Scalar::Int(unsigned(args[0].whole()?, operand_type(0)?))],
            Op::SignExtend => vec![Scalar::Int(wrap(
                args[0].whole()?,
                operand_type(0)?.width * 8,
                true,
            ))],
            Op::Truncate => {
                let target = activation.layout.value_types[&instruction.results[0]];
                let fits = match &args[0] {
                    Scalar::Float(value) => {
                        // Powers of two, so exact as doubles.
                        let half = 2f64.powi(target.width as i32 * 8 - 1);
                        let (low, top) = if target.signed == Some(true) {
                            (-half, half)
                        } else {
                            (0.0, 2.0 * half)
                        };
                        value.is_finite() && (low..top).contains(&value.trunc())
                    }
                    other => {
                        let value = other.whole()?;
                        integer(value, target) == value
                    }
                };
                if !fits {
                    return fail(format!("value is outside {}", target.name));
                }
                vec![Scalar::Int(args[0].whole()?)]
            }
            Op::Fneg | Op::Neg => vec![match &args[0] {
                Scalar::Int(value) => Scalar::Int(value.wrapping_neg()),
                other => Scalar::Float(-other.float()?),
            }],
            Op::Not => {
                let boolean = operand_type(0)?.kind == TypeKind::Boolean;
                vec![if boolean { truth(!args[0].truthy()) } else { Scalar::Int(!args[0].whole()?) }]
            }
            Op::FixedMul | Op::FixedDiv => {
                let (left, right, fraction) =
                    (args[0].whole()?, args[1].whole()?, args[2].whole()?);
                if !(0..64).contains(&fraction) {
                    return fail(format!("fixed fraction {fraction} is out of range"));
                }
                let result = if op == Op::FixedMul {
                    left.wrapping_mul(right) >> fraction
                } else {
                    let Some(quotient) = trunc_div(left << fraction, right, operand_type(0)?) else {
                        return self.panic(DIVIDE_FAULT);
                    };
                    quotient
                };
                vec![Scalar::Int(result)]
            }
            Op::Add | Op::Fadd => vec![arithmetic(&args, i128::wrapping_add, |a, b| a + b)?],
            Op::Sub | Op::Fsub => vec![arithmetic(&args, i128::wrapping_sub, |a, b| a - b)?],
            Op::Mul | Op::Fmul => vec![arithmetic(&args, i128::wrapping_mul, |a, b| a * b)?],
            Op::Fdiv => {
                // As the x87 with its exceptions masked: inf or nan, never a fault.
                vec![Scalar::Float(args[0].float()? / args[1].float()?)]
            }
            Op::And => vec![Scalar::Int(args[0].whole()? & args[1].whole()?)],
            Op::Or => vec![Scalar::Int(args[0].whole()? | args[1].whole()?)],
            Op::Xor => vec![Scalar::Int(args[0].whole()? ^ args[1].whole()?)],
            Op::Shl | Op::Shr | Op::Sar => {
                let left_type = operand_type(0)?;
                let (value, count) = (args[0].whole()?, args[1].whole()?);
                let bits = left_type.width * 8;
                if !(0..i128::from(bits)).contains(&count) {
                    return fail(format!("shift count {count} is not less than {bits}"));
                }
                let count = count as u32;
                vec![Scalar::Int(match op {
                    Op::Shl => value.wrapping_shl(count),
                    Op::Shr => unsigned(value, left_type) >> count,
                    _ => value >> count,
                })]
            }
            Op::Div | Op::Rem | Op::Divmod => {
                let (left, right) = (args[0].whole()?, args[1].whole()?);
                let Some(quotient) = trunc_div(left, right, operand_type(0)?) else {
                    return self.panic(DIVIDE_FAULT);
                };
                let remainder = left - quotient * right;
                match op {
                    Op::Div => vec![Scalar::Int(quotient)],
                    Op::Rem => vec![Scalar::Int(remainder)],
                    _ => vec![Scalar::Int(quotient), Scalar::Int(remainder)],
                }
            }
            Op::Udiv | Op::Urem | Op::Udivmod => {
                let left = unsigned(args[0].whole()?, operand_type(0)?);
                let right = unsigned(args[1].whole()?, operand_type(1)?);
                if right == 0 {
                    return self.panic(DIVIDE_FAULT);
                }
                let (quotient, remainder) = (left / right, left % right);
                match op {
                    Op::Udiv => vec![Scalar::Int(quotient)],
                    Op::Urem => vec![Scalar::Int(remainder)],
                    _ => vec![Scalar::Int(quotient), Scalar::Int(remainder)],
                }
            }
            Op::Eq | Op::Ne => {
                let equal = match (&args[0], &args[1]) {
                    (Scalar::Address(left), Scalar::Address(right)) => {
                        Rc::ptr_eq(&left.memory, &right.memory) && left.offset == right.offset
                    }
                    (Scalar::Address(_), _) | (_, Scalar::Address(_)) => false,
                    _ => compare(&args)? == Some(std::cmp::Ordering::Equal),
                };
                vec![truth(equal == (op == Op::Eq))]
            }
            Op::StringEq | Op::StringNe | Op::StringLt | Op::StringLe | Op::StringGt | Op::StringGe => {
                let order = Self::qb_string_order(&args[0], &args[1])?;
                let op = match op {
                    Op::StringEq => Op::Eq,
                    Op::StringNe => Op::Ne,
                    Op::StringLt => Op::Lt,
                    Op::StringLe => Op::Le,
                    Op::StringGt => Op::Gt,
                    _ => Op::Ge,
                };
                vec![truth(match op {
                    Op::Eq => order.is_eq(),
                    Op::Ne => order.is_ne(),
                    _ => ordered(op, order),
                })]
            }
            Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                let order = compare(&args)?;
                vec![truth(order.is_some_and(|one| ordered(op, one)))]
            }
            Op::Below | Op::BelowEq | Op::Above | Op::AboveEq => {
                let left = unsigned(args[0].whole()?, operand_type(0)?);
                let right = unsigned(args[1].whole()?, operand_type(1)?);
                let op = match op {
                    Op::Below => Op::Lt,
                    Op::BelowEq => Op::Le,
                    Op::Above => Op::Gt,
                    _ => Op::Ge,
                };
                vec![truth(ordered(op, left.cmp(&right)))]
            }
            Op::Fabs | Op::Fsqrt | Op::Fsin | Op::Fcos | Op::Fatan | Op::Flog2 | Op::Fexp2 | Op::Fround => {
                let value = args[0].float()?;
                let result = match op {
                    Op::Fabs => value.abs(),
                    Op::Fsqrt => value.sqrt(),
                    Op::Fsin => value.sin(),
                    Op::Fcos => value.cos(),
                    Op::Fatan => value.atan(),
                    Op::Fround => value.round_ties_even(),
                    Op::Flog2 => value.log2(),
                    _ => value.exp2(),
                };
                // Python's math raises where IEEE would invent a NaN or overflow.
                if (result.is_nan() && !value.is_nan())
                    || (result.is_infinite() && value.is_finite())
                {
                    return fail(format!("{op}: math domain error"));
                }
                vec![Scalar::Float(result)]
            }
            _ => {
                return fail(format!(
                    "{}: unsupported HIR operation {op}",
                    activation.name
                ));
            }
        };
        self.define(activation, instruction, results)
    }

    fn call(&mut self, name: Option<&str>, arguments: Vec<Scalar>) -> Outcome<Option<Scalar>> {
        let Some(name) = name else {
            return fail("call has no resolved callee");
        };
        if self.functions.contains_key(name) {
            return self.invoke(name, arguments);
        }
        if let Some(result) = self.runtime(name, &arguments)? {
            return Ok(result);
        }
        if let Some(result) = self.qb_runtime(name, &arguments)? {
            return Ok(result);
        }
        if name == rt::PRINT_FIELD {
            let [width, radix, fill, left] =
                [0, 1, 2, 3].map(|index| arguments[index].whole().unwrap_or(0));
            self.field = Some((width as usize, radix as u32, fill as u8, left != 0));
            return Ok(None);
        }
        let (width, radix, fill, left) = self.field.take().unwrap_or((0, 10, b' ', false));
        let text = match name {
            rt::PRINT_NEWLINE => "\n".to_owned(),
            rt::PRINT_STRING => cp437(&runtime::string_bytes(&arguments[0])?),
            rt::PRINT_VIEW => cp437(&runtime::descriptor_bytes(&arguments[0])?),
            rt::PRINT_Q2 | rt::PRINT_Q4 => fixed_text(arguments[0].whole()?, arguments[1].whole()?)?,
            rt::PRINT_BOOL => if arguments[0].truthy() {
                "true"
            } else {
                "false"
            }
            .to_owned(),
            rt::PRINT_CHAR => match u8::try_from(arguments[0].whole()?) {
                Ok(byte) => cp437(&[byte]),
                Err(_) => return fail(format!("{} byte must be in range(0, 256)", rt::PRINT_CHAR)),
            },
            rt::PRINT_I1 | rt::PRINT_U1 | rt::PRINT_I2 | rt::PRINT_U2 | rt::PRINT_I4 | rt::PRINT_U4 => {
                radix_text(arguments[0].whole()?, radix)
            }
            rt::PRINT_R4 => pyrepr::float32(arguments[0].float()? as f32),
            rt::PRINT_R8 => pyrepr::float(arguments[0].float()?),
            _ => return fail(format!("no reference implementation for external {name:?}")),
        };
        self.emit(&padded(text, width, fill, left))?;
        Ok(None)
    }
}

/// `value` in base `radix`, lowercase, its sign in front.
fn radix_text(value: i128, radix: u32) -> String {
    let mut magnitude = value.unsigned_abs();
    let mut digits = Vec::new();
    loop {
        digits.push(
            std::char::from_digit((magnitude % u128::from(radix)) as u32, radix).expect("a digit"),
        );
        magnitude /= u128::from(radix);
        if magnitude == 0 {
            break;
        }
    }
    let sign = if value < 0 { "-" } else { "" };
    format!("{sign}{}", digits.iter().rev().collect::<String>())
}

/// `text` filled out to `width`; a zero fill goes after the sign.
fn padded(text: String, width: usize, fill: u8, left: bool) -> String {
    let missing = width.saturating_sub(text.chars().count());
    let filler = (fill as char).to_string().repeat(missing);
    match (left, fill, text.strip_prefix('-')) {
        (true, ..) => text + &filler,
        (false, b'0', Some(digits)) => format!("-{filler}{digits}"),
        _ => filler + &text,
    }
}

fn arithmetic(
    args: &[Scalar],
    int: fn(i128, i128) -> i128,
    float: fn(f64, f64) -> f64,
) -> Outcome<Scalar> {
    match (&args[0], &args[1]) {
        (Scalar::Int(left), Scalar::Int(right)) => Ok(Scalar::Int(int(*left, *right))),
        (left, right) => Ok(Scalar::Float(float(left.float()?, right.float()?))),
    }
}

fn compare(args: &[Scalar]) -> Outcome<Option<std::cmp::Ordering>> {
    match (&args[0], &args[1]) {
        (Scalar::Int(left), Scalar::Int(right)) => Ok(Some(left.cmp(right))),
        (left, right) => Ok(left.float()?.partial_cmp(&right.float()?)),
    }
}

fn ordered(op: Op, order: std::cmp::Ordering) -> bool {
    match op {
        Op::Lt => order.is_lt(),
        Op::Le => order.is_le(),
        Op::Gt => order.is_gt(),
        _ => order.is_ge(),
    }
}

fn span(where_: &Location<'_>, length: usize) -> Outcome<std::ops::Range<usize>> {
    if where_.memory.borrow().dead {
        return fail("access through a pointer into a returned call's frame");
    }
    let size = where_.memory.borrow().bytes.len();
    match usize::try_from(where_.offset) {
        Ok(start) if start + length <= size => Ok(start..start + length),
        _ => fail("access falls outside its storage object"),
    }
}

fn load(where_: &Location<'_>) -> Outcome<Scalar> {
    let type_ = where_.type_;
    span(where_, type_.width as usize)?;
    let cells = where_.memory.borrow();
    if type_.kind == TypeKind::Pointer {
        return match cells.pointers.get(&(where_.offset, type_.width)) {
            Some(address) => Ok(Scalar::Address(address.clone())),
            // A null pointer, as a moved-from string holds.
            None if cells.bytes[span(where_, type_.width as usize)?]
                .iter()
                .all(|one| *one == 0) =>
            {
                Ok(Scalar::Int(0))
            }
            None => fail("pointer load reads an uninitialized address cell"),
        };
    }
    if let Some(address) = cells.pointers.get(&(where_.offset, type_.width)) {
        return Ok(Scalar::Address(address.clone()));
    }
    let data = &cells.bytes[span(where_, type_.width as usize)?];
    match type_.kind {
        TypeKind::Float => match data.len() {
            4 => Ok(Scalar::Float(f64::from(f32::from_le_bytes(
                data.try_into().unwrap(),
            )))),
            8 => Ok(Scalar::Float(f64::from_le_bytes(data.try_into().unwrap()))),
            _ => fail(format!("cannot load {}", type_.name)),
        },
        TypeKind::Integer | TypeKind::Boolean => {
            let raw = data
                .iter()
                .rev()
                .fold(0i128, |value, byte| value << 8 | i128::from(*byte));
            Ok(Scalar::Int(integer(raw, type_)))
        }
        _ => fail(format!("cannot load first-class {}", type_.name)),
    }
}

fn store(where_: &Location<'_>, value: Scalar) -> Outcome<()> {
    let type_ = where_.type_;
    let value = normalized(value, type_)?;
    let data = match (&value, type_.kind) {
        (Scalar::Address(_), _) => vec![0; type_.width as usize],
        (Scalar::Float(number), _) if type_.width == 4 => (*number as f32).to_le_bytes().to_vec(),
        (Scalar::Float(number), _) if type_.width == 8 => number.to_le_bytes().to_vec(),
        (Scalar::Int(number), _) => number.to_le_bytes()[..type_.width as usize].to_vec(),
        _ => return fail("unsupported scalar store"),
    };
    let range = span(where_, data.len())?;
    let mut cells = where_.memory.borrow_mut();
    cells.bytes[range].copy_from_slice(&data);
    let after = where_.offset + data.len() as i64;
    if let Scalar::Address(address) = value {
        cells.pointers.insert((where_.offset, type_.width), address);
    } else {
        cells
            .pointers
            .retain(|(offset, width), _| !(where_.offset < offset + width && *offset < after));
    }
    Ok(())
}

#[path = "execute_runtime.rs"]
mod runtime;

#[path = "execute_qb.rs"]
mod qb;

#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;
