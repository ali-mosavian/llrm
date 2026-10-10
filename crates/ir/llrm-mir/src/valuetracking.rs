//! Facts about a value's bits, as LLVM's ValueTracking proves them for
//! every pass and selector to ask.

use crate::context::{ConstantExpr, ConstantKind, Context, GlobalId, signed};
use crate::datalayout::DataLayout;
use crate::hash::HashMap;
use crate::module::{Function, GlobalKind, GlobalValue, Module, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, CastOp, Opcode};
use crate::types::TypeId;

/// Where each bit of a shift's or width change's result comes from: result bit
/// `i` is operand bit `i + offset`, and a result bit with no such operand bit
/// is a copy of the sign (`Fill::Sign`) or zero. One table, read forward by
/// `known_zero` (which result bits are zero) and backward by
/// `demanded_operands` (which operand bits a result bit reads).
#[derive(Clone, Copy)]
struct Flow {
    width: u32,
    from: u32,
    offset: i32,
    fill: Fill,
}

#[derive(Clone, Copy, PartialEq)]
enum Fill {
    Zero,
    Sign,
}

fn all_bits(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1_u128 << width) - 1 }
}

impl Flow {
    /// The flow of `opcode` over an operand of `from` bits into a result of
    /// `width`, where `count` is a shift's constant amount.
    fn of(
        opcode: &Opcode,
        width: u32,
        from: u32,
        count: Option<u32>,
    ) -> Option<Self> {
        let (offset, fill) = match opcode {
            Opcode::Binary(BinaryOp::Shl) => (-i32::try_from(count?).ok()?, Fill::Zero),
            Opcode::Binary(BinaryOp::LShr) => (i32::try_from(count?).ok()?, Fill::Zero),
            Opcode::Binary(BinaryOp::AShr) => (i32::try_from(count?).ok()?, Fill::Sign),
            Opcode::Cast(CastOp::ZExt) => (0, Fill::Zero),
            Opcode::Cast(CastOp::SExt) => (0, Fill::Sign),
            Opcode::Cast(CastOp::Trunc) => (0, Fill::Zero),
            _ => return None,
        };
        Some(Self { width, from, offset, fill })
    }

    /// The result bits that read an operand bit.
    fn sourced(&self) -> u128 {
        let low = (-self.offset).max(0) as u32;
        let high = (i64::from(self.from) - i64::from(self.offset)).clamp(0, i64::from(self.width)) as u32;
        if low >= high { 0 } else { all_bits(high) & !all_bits(low) }
    }

    /// The result bits known zero given the operand bits `zero`: the bits moved
    /// over, and a zero fill. A sign fill says nothing here.
    fn forward(
        &self,
        zero: u128,
    ) -> u128 {
        let moved = if self.offset >= 0 { zero >> self.offset } else { zero << -self.offset };
        let fill = if self.fill == Fill::Zero { all_bits(self.width) & !self.sourced() } else { 0 };
        ((moved & self.sourced()) | fill) & all_bits(self.width)
    }

    /// The operand bits that the result bits `demanded` read: the bits moved
    /// back, and the operand's sign bit for a sign fill that is read.
    fn backward(
        &self,
        demanded: u128,
    ) -> u128 {
        let read = demanded & self.sourced();
        let moved = if self.offset >= 0 { read << self.offset } else { read >> -self.offset };
        let sign = self.fill == Fill::Sign && demanded & all_bits(self.width) & !self.sourced() != 0;
        (moved | if sign { 1_u128 << (self.from - 1) } else { 0 }) & all_bits(self.from)
    }
}

/// How deep a question recurses, as LLVM's `MaxAnalysisRecursionDepth`.
const DEPTH: u32 = 6;

/// How many of an integer's top bits are copies of its sign bit, at least
/// one: LLVM's `ComputeNumSignBits`.
pub fn sign_bits(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> u32 {
    _sign_bits(context, function, operand, 0)
}

fn _sign_bits(
    context: &Context,
    function: &Function,
    operand: Operand,
    depth: u32,
) -> u32 {
    let Some(width) = function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)) else {
        return 1;
    };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    if let Some(bits) = constant(operand) {
        let value = signed(bits, width);
        let magnitude = if value < 0 { !value } else { value };
        return (magnitude.leading_zeros() - (128 - width)).max(1);
    }
    let Operand::Value(value) = operand else { return 1 };
    let ValueDef::Instruction(inst) = function.value(value).def else { return 1 };
    if depth == DEPTH {
        return 1;
    }
    let instruction = function.instruction(inst);
    let operands = &instruction.operands;
    let of = |operand: Operand| _sign_bits(context, function, operand, depth + 1);
    let from = |operand: Operand| {
        function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).unwrap_or(width)
    };
    let amount = |operand: Operand| constant(operand).map(|bits| bits as u32).filter(|&count| count < width);
    match instruction.opcode {
        Opcode::Cast(CastOp::SExt) => of(operands[0]) + (width - from(operands[0])),
        Opcode::Cast(CastOp::ZExt) if width > from(operands[0]) => width - from(operands[0]),
        Opcode::Cast(CastOp::Trunc) => of(operands[0]).saturating_sub(from(operands[0]) - width).max(1),
        Opcode::Binary(BinaryOp::AShr) => match amount(operands[1]) {
            Some(count) => (of(operands[0]) + count).min(width),
            None => of(operands[0]),
        },
        Opcode::Binary(BinaryOp::Shl) => match amount(operands[1]) {
            Some(count) => of(operands[0]).saturating_sub(count).max(1),
            None => 1,
        },
        // The product has no more significant bits than its factors together.
        Opcode::Binary(BinaryOp::Mul) => {
            let significant = (width - of(operands[0]) + 1) + (width - of(operands[1]) + 1);
            if significant > width { 1 } else { width - significant + 1 }
        }
        _ => 1,
    }
}

/// The bits of an integer proven zero, one bit per position: the zero half
/// of LLVM's `computeKnownBits`. A value wider than 128 bits proves none.
pub fn known_zero(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> u128 {
    _known_zero(context, function, operand, 0)
}

fn _known_zero(
    context: &Context,
    function: &Function,
    operand: Operand,
    depth: u32,
) -> u128 {
    let Some(width) =
        function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).filter(|&width| width <= 128)
    else {
        return 0;
    };
    let all = if width == 128 { u128::MAX } else { (1_u128 << width) - 1 };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits & all),
            _ => None,
        },
        _ => None,
    };
    if let Some(bits) = constant(operand) {
        return !bits & all;
    }
    let Operand::Value(value) = operand else { return 0 };
    let ValueDef::Instruction(inst) = function.value(value).def else { return 0 };
    if depth == DEPTH {
        return 0;
    }
    let instruction = function.instruction(inst);
    let operands = &instruction.operands;
    let of = |operand: Operand| _known_zero(context, function, operand, depth + 1);
    let from = |operand: Operand| {
        function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).unwrap_or(width)
    };
    let amount =
        |operand: Operand| constant(operand).filter(|&count| count < u128::from(width)).map(|count| count as u32);
    match instruction.opcode {
        Opcode::Binary(BinaryOp::And) => of(operands[0]) | of(operands[1]),
        Opcode::Binary(BinaryOp::Or | BinaryOp::Xor) => of(operands[0]) & of(operands[1]),
        Opcode::Cast(CastOp::ZExt | CastOp::Trunc) | Opcode::Binary(BinaryOp::LShr | BinaryOp::Shl) => {
            let count = operands.get(1).copied().and_then(amount);
            match Flow::of(&instruction.opcode, width, from(operands[0]), count) {
                Some(flow) => flow.forward(of(operands[0])),
                None => 0,
            }
        }
        Opcode::Select => of(operands[1]) & of(operands[2]),
        // The low bits a product of multiples of 2^a and 2^b leaves clear are
        // a + b; a sum's, the least of a and b.
        Opcode::Binary(BinaryOp::Mul) => low(of(operands[0]).trailing_ones() + of(operands[1]).trailing_ones(), width),
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => {
            low(of(operands[0]).trailing_ones().min(of(operands[1]).trailing_ones()), width)
        }
        _ => 0,
    }
}

/// The bits of each operand of `inst` that its result's `demanded` bits read,
/// where the instruction is one whose result bit depends on a few operand bits
/// and the table says which: LLVM's `DemandedBits::determineLiveOperandBits`,
/// the dual of `known_zero`'s forward table. None for the rest, which read
/// every bit of every operand. An operand that is not an integer is given 0.
pub fn demanded_operands(
    context: &Context,
    function: &Function,
    inst: crate::module::InstId,
    demanded: u128,
) -> Option<Vec<u128>> {
    let instruction = function.instruction(inst);
    let width = context.types.int_bits(instruction.ty).filter(|&width| width <= 128)?;
    let all = |width: u32| if width >= 128 { u128::MAX } else { (1_u128 << width) - 1 };
    let demanded = demanded & all(width);
    let operands = &instruction.operands;
    let from = |operand: Operand| {
        function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).filter(|&w| w <= 128)
    };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits & all(width)),
            _ => None,
        },
        _ => None,
    };
    // A sum, difference or product bit depends on the operand bits at or below
    // it.
    let through_carries = if demanded == 0 { 0 } else { u128::MAX >> demanded.leading_zeros() };
    match instruction.opcode {
        Opcode::Binary(BinaryOp::And) => Some(match (constant(operands[0]), constant(operands[1])) {
            (_, Some(mask)) => vec![demanded & mask, 0],
            (Some(mask), _) => vec![0, demanded & mask],
            _ => vec![demanded, demanded],
        }),
        Opcode::Binary(BinaryOp::Or) => Some(match (constant(operands[0]), constant(operands[1])) {
            (_, Some(mask)) => vec![demanded & !mask, 0],
            (Some(mask), _) => vec![0, demanded & !mask],
            _ => vec![demanded, demanded],
        }),
        Opcode::Binary(BinaryOp::Xor) => Some(vec![demanded, demanded]),
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) => {
            Some(vec![through_carries & all(width), through_carries & all(width)])
        }
        Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
        | Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) => {
            let count = operands.get(1).copied().and_then(constant).and_then(|count| u32::try_from(count).ok());
            let flow = Flow::of(&instruction.opcode, width, from(operands[0])?, count.filter(|&count| count < width))?;
            let mut read = vec![flow.backward(demanded)];
            if operands.len() > 1 {
                read.push(all(width));
            }
            Some(read)
        }
        Opcode::Select => Some(vec![1, demanded, demanded]),
        _ => None,
    }
}

/// The `count` low bits of a `width`-bit integer.
fn low(
    count: u32,
    width: u32,
) -> u128 {
    if count >= width { if width == 128 { u128::MAX } else { (1_u128 << width) - 1 } } else { (1_u128 << count) - 1 }
}

/// The object `pointer` points into, through every GEP and address space
/// cast, and how far into it when every step is constant: LLVM's
/// `getUnderlyingObject` and `GetPointerBaseWithConstantOffset` in one.
pub fn underlying(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    pointer: Operand,
) -> (Operand, Option<i64>) {
    let int = |one: Operand| match one {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(signed(bits, context.types.int_bits(context.get(id).ty).unwrap_or(64))),
            _ => None,
        },
        _ => None,
    };
    let mut at = pointer;
    let mut offset = Some(0_i64);
    for _ in 0..DEPTH {
        // A GEP or an address-space cast, an instruction or a constant.
        let (source, operands): (Option<TypeId>, Vec<Operand>) = match at {
            Operand::Value(value) => {
                let ValueDef::Instruction(inst) = function.value(value).def else { break };
                let instruction = function.instruction(inst);
                match instruction.opcode {
                    Opcode::GetElementPtr { source } => (Some(source), instruction.operands.clone()),
                    Opcode::Cast(CastOp::AddrSpaceCast) => (None, instruction.operands.clone()),
                    _ => break,
                }
            }
            Operand::Constant(id) => match &context.get(id).kind {
                ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
                    (Some(*source), operands.iter().map(|&one| Operand::Constant(one)).collect())
                }
                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => {
                    (None, vec![Operand::Constant(*value)])
                }
                _ => break,
            },
            Operand::Block(_) => break,
        };
        if let Some(source) = source {
            let indices: Vec<Option<i128>> = operands[1..].iter().map(|&one| int(one)).collect();
            let (constant, variable) = layout.collect_offset(&context.types, source, &indices);
            offset = offset.filter(|_| variable.is_empty()).map(|one| one + constant as i64);
        }
        at = operands[0];
    }
    (at, offset)
}

/// The largest power of two `pointer` is known a multiple of: its object's
/// stated alignment, less what each GEP on the way may add. LLVM's
/// `getKnownAlignment`, stated alignment only: nothing places an object at
/// its type's alignment unless it says so.
pub fn alignment(
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &Function,
    pointer: Operand,
) -> u64 {
    let mut at = pointer;
    // log2 of what every step added is a multiple of.
    let mut added = u32::MAX;
    for _ in 0..DEPTH {
        let (source, operands): (Option<TypeId>, Vec<Operand>) = match at {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Argument(index) => {
                    let stated = function
                        .parameter_attrs[index as usize]
                        .iter()
                        .find_map(
                            |attr| match attr {
                                Attribute::Int(name, bytes) if name == "align" => Some(*bytes),
                                _ => None,
                            },
                        );
                    return _lesser(stated.unwrap_or(1), added);
                }
                ValueDef::Instruction(inst) => {
                    let instruction = function.instruction(inst);
                    match instruction.opcode {
                        Opcode::GetElementPtr { source } => (Some(source), instruction.operands.clone()),
                        Opcode::Cast(CastOp::AddrSpaceCast) => (None, instruction.operands.clone()),
                        _ => return 1,
                    }
                }
            },
            Operand::Constant(id) => match &context.get(id).kind {
                ConstantKind::Global(global) => {
                    let stated = match &globals[global.0 as usize].kind {
                        GlobalKind::Variable(variable) => variable.align.unwrap_or(1),
                        GlobalKind::Function(_) => 1,
                    };
                    return _lesser(stated, added);
                }
                ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
                    (Some(*source), operands.iter().map(|&one| Operand::Constant(one)).collect())
                }
                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => {
                    (None, vec![Operand::Constant(*value)])
                }
                _ => return 1,
            },
            Operand::Block(_) => return 1,
        };
        if let Some(source) = source {
            let int = |one: Operand| match one {
                Operand::Constant(id) => match context.get(id).kind {
                    ConstantKind::Int(bits) => {
                        Some(signed(bits, context.types.int_bits(context.get(id).ty).unwrap_or(64)))
                    }
                    _ => None,
                },
                _ => None,
            };
            let indices: Vec<Option<i128>> = operands[1..].iter().map(|&one| int(one)).collect();
            let (constant, variable) = layout.collect_offset(&context.types, source, &indices);
            if constant != 0 {
                added = added.min(constant.trailing_zeros());
            }
            for (position, scale) in variable {
                added = added.min(scale.trailing_zeros().saturating_add(_multiple(
                    context,
                    function,
                    operands[position + 1],
                    0,
                )));
            }
        }
        at = operands[0];
    }
    1
}

/// `stated`, or less where the offsets added are a multiple of only `2^added`.
fn _lesser(
    stated: u64,
    added: u32,
) -> u64 {
    if added >= stated.trailing_zeros() { stated } else { 1 << added }
}

/// log2 of the largest power of two `operand` is known a multiple of.
fn _multiple(
    context: &Context,
    function: &Function,
    operand: Operand,
    depth: u32,
) -> u32 {
    let Some(width) = function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)) else {
        return 0;
    };
    let constant = |operand: Operand| match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    if let Some(bits) = constant(operand) {
        return (bits & crate::context::mask(width)).trailing_zeros().min(width);
    }
    let Operand::Value(value) = operand else { return 0 };
    let ValueDef::Instruction(inst) = function.value(value).def else { return 0 };
    if depth == DEPTH {
        return 0;
    }
    let operands = &function.instruction(inst).operands;
    let of = |operand: Operand| _multiple(context, function, operand, depth + 1);
    match function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => of(operands[0]).min(of(operands[1])),
        Opcode::Binary(BinaryOp::Mul) => (of(operands[0]) + of(operands[1])).min(width),
        Opcode::Binary(BinaryOp::Shl) => match constant(operands[1]) {
            Some(count) if count < u128::from(width) => (of(operands[0]) + count as u32).min(width),
            _ => of(operands[0]),
        },
        Opcode::Cast(CastOp::SExt | CastOp::ZExt) => of(operands[0]),
        Opcode::Cast(CastOp::Trunc) => of(operands[0]).min(width),
        _ => 0,
    }
}

/// Each global variable's size in bytes.
pub type Sizes = HashMap<GlobalId, u64>;

pub fn sizes(
    module: &Module,
    layout: &DataLayout,
) -> Sizes {
    let variables = module
        .globals
        .iter()
        .enumerate()
        .filter_map(
            |(at, global)| match &global.kind {
                GlobalKind::Variable(variable) => {
                    Some((GlobalId(at as u32), layout.alloc_size(&module.context.types, variable.ty)))
                }
                GlobalKind::Function(_) => None,
            },
        );
    variables.collect()
}

/// Whether `bytes` bytes at `pointer` can be read whether or not the
/// program would: LLVM's `isDereferenceablePointer`.
pub fn dereferenceable(
    context: &Context,
    layout: &DataLayout,
    sizes: &Sizes,
    function: &Function,
    pointer: Operand,
    bytes: u64,
) -> bool {
    let (base, Some(offset)) = underlying(context, layout, function, pointer) else { return false };
    let value = match base {
        Operand::Value(value) => value,
        Operand::Constant(id) => {
            let ConstantKind::Global(global) = context.get(id).kind else { return false };
            return sizes.get(&global).is_some_and(|&size| offset >= 0 && offset as u64 + bytes <= size);
        }
        Operand::Block(_) => return false,
    };
    let size = match function.value(value).def {
        ValueDef::Argument(at) => function.parameter_attrs[at as usize].iter().find_map(|attr| match attr {
            Attribute::Int(name, bytes) if name == "dereferenceable" => Some(*bytes),
            _ => None,
        }),
        ValueDef::Instruction(inst) => match function.instruction(inst).opcode {
            Opcode::Alloca { allocated, .. } if function.instruction(inst).operands.is_empty() => {
                Some(layout.alloc_size(&context.types, allocated))
            }
            _ => None,
        },
    };
    size.is_some_and(|size| offset >= 0 && offset as u64 + bytes <= size)
}
