//! Runs MIR with LLVM's semantics, poison included: the oracle a transformed
//! function is checked against, itself checked against `lli`.
//!
//! Memory is one flat, byte-addressed space laid out by the module's
//! datalayout; every address space maps onto it, so `addrspacecast` keeps
//! the address. Each byte also records whether it holds poison.

use std::collections::HashMap;

use crate::context::{ConstantExpr, ConstantId, ConstantKind, GlobalId, mask, signed};
use crate::datalayout::{DataLayout, float_bits};
use crate::intrinsics::Intrinsic;
use crate::module::{BlockId, Function, GlobalKind, Module, Operand, ValueId};
use crate::opcode::{BinaryOp, CastOp, FloatPredicate, Flags, IntPredicate, Opcode};
use crate::types::{FloatKind, Type, TypeId};

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Int { bits: u128, width: u32 },
    /// IEEE bits: a `float`'s in the low 32.
    Float(FloatKind, u64),
    Ptr(u64),
    Aggregate(Vec<Val>),
    Poison,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trap {
    /// Behaviour LLVM leaves undefined: the program has none.
    Undefined(String),
    /// What this interpreter does not model.
    Unsupported(String),
    OutOfFuel,
}

type Run<T> = Result<T, Trap>;

fn undefined<T>(why: impl Into<String>) -> Run<T> {
    Err(Trap::Undefined(why.into()))
}

fn unsupported<T>(why: impl Into<String>) -> Run<T> {
    Err(Trap::Unsupported(why.into()))
}

/// Runs `@name` on `arguments` with at most `fuel` instructions.
pub fn run(module: &Module, name: &str, arguments: Vec<Val>, fuel: u64) -> Run<Val> {
    let mut machine = Machine::new(module, fuel)?;
    let id = module.named(name).ok_or_else(|| Trap::Unsupported(format!("no @{name}")))?;
    machine.call(id, arguments)
}

struct Machine<'m> {
    module: &'m Module,
    layout: DataLayout,
    memory: Vec<u8>,
    poison: Vec<bool>,
    addresses: HashMap<GlobalId, u64>,
    fuel: u64,
}

impl<'m> Machine<'m> {
    fn new(module: &'m Module, fuel: u64) -> Run<Self> {
        let layout = match &module.datalayout {
            Some(text) => DataLayout::parse(text).map_err(Trap::Unsupported)?,
            None => DataLayout::default(),
        };
        // Address 0 is null; nothing is allocated there.
        let mut machine = Self { module, layout, memory: vec![0; 16], poison: vec![false; 16], addresses: HashMap::new(), fuel };
        for (at, global) in module.globals.iter().enumerate() {
            let (size, align) = match &global.kind {
                GlobalKind::Variable(variable) => {
                    let types = &module.context.types;
                    (machine.layout.alloc_size(types, variable.ty).max(1), variable.align.unwrap_or(1).max(machine.layout.align(types, variable.ty)))
                }
                GlobalKind::Function(_) => (1, 1),
            };
            let address = machine.allocate(size, align);
            machine.addresses.insert(GlobalId(at as u32), address);
        }
        for (at, global) in module.globals.iter().enumerate() {
            if let GlobalKind::Variable(variable) = &global.kind
                && let Some(initializer) = variable.initializer
            {
                let value = machine.constant(initializer)?;
                let address = machine.addresses[&GlobalId(at as u32)];
                machine.store(&value, variable.ty, address)?;
            }
        }
        Ok(machine)
    }

    fn allocate(&mut self, size: u64, align: u64) -> u64 {
        let start = (self.memory.len() as u64).next_multiple_of(align.max(1));
        self.memory.resize((start + size) as usize, 0);
        self.poison.resize((start + size) as usize, false);
        start
    }

    fn types(&self) -> &'m crate::types::Types {
        &self.module.context.types
    }

    // ---- values and memory

    fn zero(&self, ty: TypeId) -> Val {
        let types = self.types();
        match types.get(ty) {
            Type::Int(width) => Val::Int { bits: 0, width: *width },
            Type::Float(kind) => Val::Float(*kind, 0),
            Type::Pointer(_) => Val::Ptr(0),
            Type::Array { element, count } => Val::Aggregate((0..*count).map(|_| self.zero(*element)).collect()),
            Type::Vector { element, count } => Val::Aggregate((0..*count).map(|_| self.zero(*element)).collect()),
            _ => Val::Aggregate(types.fields(ty).unwrap_or_default().iter().map(|&one| self.zero(one)).collect()),
        }
    }

    fn constant(&self, id: ConstantId) -> Run<Val> {
        let constant = self.module.context.get(id);
        let types = self.types();
        Ok(match &constant.kind {
            ConstantKind::Int(bits) => Val::Int { bits: *bits, width: types.int_bits(constant.ty).expect("an integer") },
            ConstantKind::Float(bits) => match types.get(constant.ty) {
                Type::Float(kind) => Val::Float(*kind, *bits),
                _ => unreachable!("a floating constant"),
            },
            ConstantKind::Null => Val::Ptr(0),
            ConstantKind::Poison => Val::Poison,
            ConstantKind::Zero => self.zero(constant.ty),
            ConstantKind::Aggregate(members) => Val::Aggregate(members.iter().map(|&one| self.constant(one)).collect::<Run<_>>()?),
            ConstantKind::Bytes(bytes) => Val::Aggregate(bytes.iter().map(|&b| Val::Int { bits: u128::from(b), width: 8 }).collect()),
            ConstantKind::Global(global) => Val::Ptr(self.addresses[global]),
            ConstantKind::Expr(ConstantExpr::Cast { op, value }) => {
                let from = self.module.context.get(*value).ty;
                cast(self.types(), &self.layout, *op, self.constant(*value)?, from, constant.ty, Flags::default())?
            }
            ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
                let values: Vec<Val> = operands.iter().map(|&one| self.constant(one)).collect::<Run<_>>()?;
                self.gep(*source, constant.ty, &values)?
            }
        })
    }

    fn store(&mut self, value: &Val, ty: TypeId, address: u64) -> Run<()> {
        let types = self.types();
        match (types.get(ty), value) {
            (Type::Array { element, .. } | Type::Vector { element, .. }, Val::Aggregate(members)) => {
                let step = self.layout.alloc_size(types, *element);
                for (at, member) in members.iter().enumerate() {
                    self.store(member, *element, address + step * at as u64)?;
                }
                Ok(())
            }
            (Type::Struct { .. } | Type::Named(_), Val::Aggregate(members)) => {
                let (_, offsets) = self.layout.struct_layout(types, ty);
                let fields = types.fields(ty).unwrap_or_default().to_vec();
                for ((member, field), offset) in members.iter().zip(fields).zip(offsets) {
                    self.store(member, field, address + offset)?;
                }
                Ok(())
            }
            _ => {
                let size = self.layout.store_size(types, ty);
                self.check_bounds(address, size)?;
                let (bits, poison) = match value {
                    Val::Poison => (0, true),
                    Val::Int { bits, .. } => (*bits, false),
                    Val::Float(_, bits) => (u128::from(*bits), false),
                    Val::Ptr(address) => (u128::from(*address), false),
                    Val::Aggregate(_) => return unsupported("an aggregate stored as a scalar"),
                };
                for at in 0..size {
                    let shift = if self.layout.big_endian { size - 1 - at } else { at };
                    self.memory[(address + at) as usize] = (bits >> (8 * shift)) as u8;
                    self.poison[(address + at) as usize] = poison;
                }
                Ok(())
            }
        }
    }

    fn load(&self, ty: TypeId, address: u64) -> Run<Val> {
        let types = self.types();
        match types.get(ty) {
            Type::Array { element, count } => {
                let step = self.layout.alloc_size(types, *element);
                (0..*count).map(|at| self.load(*element, address + step * at)).collect::<Run<_>>().map(Val::Aggregate)
            }
            Type::Vector { element, count } => {
                let step = self.layout.alloc_size(types, *element);
                (0..u64::from(*count)).map(|at| self.load(*element, address + step * at)).collect::<Run<_>>().map(Val::Aggregate)
            }
            Type::Struct { .. } | Type::Named(_) => {
                let (_, offsets) = self.layout.struct_layout(types, ty);
                let fields = types.fields(ty).unwrap_or_default();
                fields.iter().zip(offsets).map(|(&field, offset)| self.load(field, address + offset)).collect::<Run<_>>().map(Val::Aggregate)
            }
            _ => {
                let size = self.layout.store_size(types, ty);
                self.check_bounds(address, size)?;
                let mut bits = 0u128;
                for at in 0..size {
                    if self.poison[(address + at) as usize] {
                        return Ok(Val::Poison);
                    }
                    let shift = if self.layout.big_endian { size - 1 - at } else { at };
                    bits |= u128::from(self.memory[(address + at) as usize]) << (8 * shift);
                }
                Ok(match types.get(ty) {
                    Type::Int(width) => Val::Int { bits: bits & mask(*width), width: *width },
                    Type::Float(kind) => Val::Float(*kind, bits as u64),
                    Type::Pointer(_) => Val::Ptr(bits as u64),
                    other => return unsupported(format!("a load of {other:?}")),
                })
            }
        }
    }

    fn check_bounds(&self, address: u64, size: u64) -> Run<()> {
        if address < 16 || address + size > self.memory.len() as u64 {
            return undefined(format!("an access of {size} bytes at {address:#x}, outside every object"));
        }
        Ok(())
    }

    // ---- functions

    fn call(&mut self, id: GlobalId, arguments: Vec<Val>) -> Run<Val> {
        let global = self.module.global(id);
        let name = global.name.clone().unwrap_or_default();
        let GlobalKind::Function(function) = &global.kind else { return undefined(format!("a call to the variable @{name}")) };
        if function.is_declaration() {
            return match Intrinsic::named(&name) {
                Some(intrinsic) => self.intrinsic(intrinsic, self.module.signature(function.ty).0, arguments),
                None => unsupported(format!("a call to the external @{name}")),
            };
        }
        let mark = self.memory.len();
        let result = self.execute(function, arguments);
        self.memory.truncate(mark);
        self.poison.truncate(mark);
        result
    }

    /// An intrinsic, in terms of the instructions that define it.
    fn intrinsic(&mut self, intrinsic: Intrinsic, returns: TypeId, arguments: Vec<Val>) -> Run<Val> {
        let void = Val::Aggregate(Vec::new());
        let argument = |at: usize| arguments[at].clone();
        Ok(match intrinsic {
            Intrinsic::WithOverflow { op, signed } => {
                let wrapped = binary(op, Flags::default(), argument(0), argument(1))?;
                let exact = binary(op, if signed { Flags::NSW } else { Flags::NUW }, argument(0), argument(1))?;
                let overflowed = match (&wrapped, exact) {
                    (Val::Poison, _) => Val::Poison,
                    (_, exact) => Val::Int { bits: u128::from(exact == Val::Poison), width: 1 },
                };
                Val::Aggregate(vec![wrapped, overflowed])
            }
            Intrinsic::MinMax { signed, max } => {
                let predicate = match (signed, max) {
                    (true, true) => IntPredicate::Sgt,
                    (true, false) => IntPredicate::Slt,
                    (false, true) => IntPredicate::Ugt,
                    (false, false) => IntPredicate::Ult,
                };
                match icmp(predicate, argument(0), argument(1)) {
                    Val::Int { bits: 1, .. } => argument(0),
                    Val::Int { .. } => argument(1),
                    _ => Val::Poison,
                }
            }
            // Fused or not is the machine's choice, as the LangRef allows.
            Intrinsic::FMulAdd => {
                let product = binary(BinaryOp::FMul, Flags::default(), argument(0), argument(1))?;
                binary(BinaryOp::FAdd, Flags::default(), product, argument(2))?
            }
            // In the argument's own precision.
            Intrinsic::Unary(function) => match argument(0) {
                Val::Float(FloatKind::Float, bits) => Val::Float(FloatKind::Float, u64::from((function.apply(f64::from(f32::from_bits(bits as u32))) as f32).to_bits())),
                Val::Float(FloatKind::Double, bits) => Val::Float(FloatKind::Double, function.apply(f64::from_bits(bits)).to_bits()),
                _ => Val::Poison,
            },
            Intrinsic::LRint => {
                let Val::Float(kind, bits) = argument(0) else { return Ok(Val::Poison) };
                let x = match kind {
                    FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
                    FloatKind::Double => f64::from_bits(bits),
                }
                .round_ties_even();
                let width = self.types().int_bits(returns).expect("an integer result");
                let limit = 2f64.powi(width as i32 - 1);
                if !(-limit..limit).contains(&x) {
                    return unsupported("an lrint out of range, whose value LLVM leaves unspecified");
                }
                Val::Int { bits: (x as i128 as u128) & mask(width), width }
            }
            Intrinsic::MemSet => {
                let (Val::Ptr(address), Val::Int { bits: length, .. }) = (argument(0), argument(2)) else { return undefined("a memset of a poison address or length") };
                let length = length as u64;
                if length > 0 {
                    self.check_bounds(address, length)?;
                }
                let (byte, poison) = match argument(1) {
                    Val::Int { bits, .. } => (bits as u8, false),
                    _ => (0, true),
                };
                let range = address as usize..(address + length) as usize;
                self.memory[range.clone()].fill(byte);
                self.poison[range].fill(poison);
                void
            }
            Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd => void,
            Intrinsic::PortIn | Intrinsic::PortOut => return unsupported("an I/O port"),
        })
    }

    fn execute(&mut self, function: &'m Function, arguments: Vec<Val>) -> Run<Val> {
        let mut values: HashMap<ValueId, Val> = function.parameters().iter().copied().zip(arguments).collect();
        let mut block = function.entry().expect("a body");
        let mut came_from: Option<BlockId> = None;
        loop {
            let list = function.block(block).instructions();
            // Phis read together, along the edge taken.
            let mut chosen = Vec::new();
            for &inst in list.iter().take_while(|&&one| function.instruction(one).opcode == Opcode::Phi) {
                let instruction = function.instruction(inst);
                let from = came_from.expect("the entry has no phis");
                let pair = instruction.operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).expect("verified phis cover their edges");
                chosen.push((instruction.result.expect("a phi has a result"), self.operand(&values, pair[0])?));
            }
            values.extend(chosen);
            let mut next = None;
            for &inst in list.iter().skip_while(|&&one| function.instruction(one).opcode == Opcode::Phi) {
                self.fuel = self.fuel.checked_sub(1).ok_or(Trap::OutOfFuel)?;
                let instruction = function.instruction(inst);
                let ops = &instruction.operands;
                let value = |machine: &Self, at: usize| machine.operand(&values, ops[at]);
                let target = |at: usize| match ops[at] {
                    Operand::Block(block) => block,
                    _ => unreachable!("a block operand"),
                };
                let result: Option<Val> = match &instruction.opcode {
                    Opcode::Ret => return if ops.is_empty() { Ok(Val::Aggregate(Vec::new())) } else { value(self, 0) },
                    Opcode::Br if ops.len() == 1 => {
                        next = Some(target(0));
                        break;
                    }
                    Opcode::Br => {
                        next = Some(match value(self, 0)? {
                            Val::Poison => return undefined("a branch on poison"),
                            Val::Int { bits, .. } => target(if bits != 0 { 1 } else { 2 }),
                            other => unreachable!("a condition {other:?}"),
                        });
                        break;
                    }
                    Opcode::Switch => {
                        let condition = value(self, 0)?;
                        if condition == Val::Poison {
                            return undefined("a switch on poison");
                        }
                        let mut chosen = target(1);
                        for pair in ops[2..].chunks(2) {
                            if self.operand(&values, pair[0])? == condition
                                && let Operand::Block(block) = pair[1]
                            {
                                chosen = block;
                            }
                        }
                        next = Some(chosen);
                        break;
                    }
                    Opcode::Unreachable => return undefined("unreachable was reached"),
                    Opcode::Resume | Opcode::LandingPad { .. } => return unsupported("unwinding"),
                    Opcode::Call(_) | Opcode::Invoke(_) => {
                        let invoke = matches!(instruction.opcode, Opcode::Invoke(_));
                        let count = ops.len() - if invoke { 3 } else { 1 };
                        let arguments = (0..count).map(|at| value(self, at)).collect::<Run<Vec<_>>>()?;
                        let callee = match ops[ops.len() - 1] {
                            Operand::Constant(id) => match self.module.context.get(id).kind {
                                ConstantKind::Global(global) => global,
                                _ => return unsupported("a call through a computed pointer"),
                            },
                            _ => return unsupported("a call through a computed pointer"),
                        };
                        let returned = self.call(callee, arguments)?;
                        if let Some(result) = instruction.result {
                            values.insert(result, returned);
                        }
                        if invoke {
                            next = Some(target(ops.len() - 3));
                            break;
                        }
                        None
                    }
                    Opcode::Binary(op) => Some(binary(*op, instruction.flags, value(self, 0)?, value(self, 1)?)?),
                    Opcode::FNeg => Some(match value(self, 0)? {
                        Val::Float(kind, bits) => Val::Float(kind, bits ^ (1 << (float_bits(kind) - 1))),
                        other => other,
                    }),
                    Opcode::Cast(op) => {
                        let from = function.operand_type(&self.module.context, ops[0]).expect("a value");
                        Some(cast(self.types(), &self.layout, *op, value(self, 0)?, from, instruction.ty, instruction.flags)?)
                    }
                    Opcode::ICmp(predicate) => Some(icmp(*predicate, value(self, 0)?, value(self, 1)?)),
                    Opcode::FCmp(predicate) => Some(fcmp(*predicate, value(self, 0)?, value(self, 1)?)),
                    Opcode::Select => Some(match value(self, 0)? {
                        Val::Poison => Val::Poison,
                        Val::Int { bits, .. } => value(self, if bits != 0 { 1 } else { 2 })?,
                        other => unreachable!("a condition {other:?}"),
                    }),
                    Opcode::Freeze => Some(match value(self, 0)? {
                        Val::Poison => self.zero(instruction.ty),
                        other => other,
                    }),
                    Opcode::ExtractValue(indices) => {
                        let mut at = value(self, 0)?;
                        for &index in indices {
                            at = match at {
                                Val::Aggregate(mut members) => members.swap_remove(index as usize),
                                _ => Val::Poison,
                            };
                        }
                        Some(at)
                    }
                    Opcode::InsertValue(indices) => Some(insert(value(self, 0)?, indices, value(self, 1)?)),
                    Opcode::Alloca { allocated, align, .. } => {
                        let count = match ops.first() {
                            Some(&one) => match self.operand(&values, one)? {
                                Val::Int { bits, .. } => bits as u64,
                                _ => return undefined("an alloca of poison elements"),
                            },
                            None => 1,
                        };
                        let types = self.types();
                        let size = self.layout.alloc_size(types, *allocated) * count;
                        let align = align.unwrap_or(1).max(self.layout.align(types, *allocated));
                        let address = self.allocate(size.max(1), align);
                        // Fresh memory holds nothing yet.
                        (address..address + size.max(1)).for_each(|at| self.poison[at as usize] = true);
                        Some(Val::Ptr(address))
                    }
                    Opcode::Load { .. } => Some(match value(self, 0)? {
                        Val::Ptr(address) => self.load(instruction.ty, address)?,
                        _ => return undefined("a load through poison"),
                    }),
                    Opcode::Store { .. } => {
                        let stored = value(self, 0)?;
                        let ty = function.operand_type(&self.module.context, ops[0]).expect("a value");
                        match value(self, 1)? {
                            Val::Ptr(address) => self.store(&stored, ty, address)?,
                            _ => return undefined("a store through poison"),
                        }
                        None
                    }
                    Opcode::GetElementPtr { source } => {
                        let operands = (0..ops.len()).map(|at| value(self, at)).collect::<Run<Vec<_>>>()?;
                        Some(self.gep(*source, instruction.ty, &operands)?)
                    }
                    Opcode::Phi => unreachable!("phis come first"),
                };
                if let (Some(result), Some(value)) = (instruction.result, result) {
                    values.insert(result, value);
                }
            }
            came_from = Some(block);
            block = next.expect("a verified block ends in a terminator");
        }
    }

    fn operand(&self, values: &HashMap<ValueId, Val>, operand: Operand) -> Run<Val> {
        match operand {
            Operand::Value(id) => values.get(&id).cloned().ok_or_else(|| Trap::Unsupported(format!("value {} is read before it is set", id.0))),
            Operand::Constant(id) => self.constant(id),
            Operand::Block(_) => unreachable!("a block is not a value"),
        }
    }

    // ---- operations

    /// The address `operands[0] + indices`, stepping through `source`.
    fn gep(&self, source: TypeId, result: TypeId, operands: &[Val]) -> Run<Val> {
        let types = self.types();
        let Val::Ptr(base) = operands[0] else { return Ok(Val::Poison) };
        let Type::Pointer(space) = types.get(result) else { unreachable!("a pointer") };
        let index_bits = self.layout.pointer(*space).index_bits;
        let mut indices = Vec::new();
        for value in &operands[1..] {
            let Val::Int { bits, width } = value else { return Ok(Val::Poison) };
            indices.push(Some(signed(*bits, *width)));
        }
        let (offset, _) = self.layout.collect_offset(types, source, &indices);
        let address = (i128::from(base) + offset) as u128 & mask(index_bits);
        Ok(Val::Ptr(address as u64))
    }
}

/// An integer or float operation on two values, as LLVM defines it: the
/// interpreter's and constant folding's one answer.
pub(crate) fn binary(op: BinaryOp, flags: Flags, a: Val, b: Val) -> Run<Val> {
    if let (Val::Float(kind, x), Val::Float(_, y)) = (&a, &b) {
        return Ok(float_binary(op, *kind, *x, *y));
    }
    let (Val::Int { bits: x, width }, Val::Int { bits: y, .. }) = (&a, &b) else {
        if matches!(op, BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) && b == Val::Poison {
            return undefined("a division by poison");
        }
        return Ok(Val::Poison);
    };
    let (x, y, width) = (*x, *y, *width);
    let m = mask(width);
    let (sx, sy) = (signed(x, width), signed(y, width));
    let poison = |flag: Flags, overflowed: bool| flags.contains(flag) && overflowed;
    let int = |bits: u128| Val::Int { bits: bits & m, width };
    let signed_fits = |value: i128| width >= 128 || value == signed(value as u128 & m, width);
    Ok(match op {
        BinaryOp::Add => {
            if poison(Flags::NUW, x.checked_add(y).is_none_or(|sum| sum > m)) || poison(Flags::NSW, sx.checked_add(sy).is_none_or(|sum| !signed_fits(sum))) {
                return Ok(Val::Poison);
            }
            int(x.wrapping_add(y))
        }
        BinaryOp::Sub => {
            if poison(Flags::NUW, y > x) || poison(Flags::NSW, sx.checked_sub(sy).is_none_or(|one| !signed_fits(one))) {
                return Ok(Val::Poison);
            }
            int(x.wrapping_sub(y))
        }
        BinaryOp::Mul => {
            if poison(Flags::NUW, x.checked_mul(y).is_none_or(|one| one > m)) || poison(Flags::NSW, sx.checked_mul(sy).is_none_or(|one| !signed_fits(one))) {
                return Ok(Val::Poison);
            }
            int(x.wrapping_mul(y))
        }
        BinaryOp::UDiv | BinaryOp::URem if y == 0 => return undefined("a division by zero"),
        BinaryOp::SDiv | BinaryOp::SRem if y == 0 => return undefined("a division by zero"),
        BinaryOp::SDiv | BinaryOp::SRem if sy == -1 && sx == signed(1u128 << (width - 1), width) && width > 1 => return undefined("a signed division overflows"),
        BinaryOp::UDiv => {
            if poison(Flags::EXACT, x % y != 0) {
                return Ok(Val::Poison);
            }
            int(x / y)
        }
        BinaryOp::SDiv => {
            if poison(Flags::EXACT, sx % sy != 0) {
                return Ok(Val::Poison);
            }
            int((sx / sy) as u128)
        }
        BinaryOp::URem => int(x % y),
        BinaryOp::SRem => int((sx % sy) as u128),
        BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if y >= u128::from(width) => Val::Poison,
        BinaryOp::Shl => {
            let shifted = (x << y) & m;
            if poison(Flags::NUW, shifted >> y != x) || poison(Flags::NSW, signed(shifted, width) >> y != sx) {
                return Ok(Val::Poison);
            }
            int(shifted)
        }
        BinaryOp::LShr => {
            if poison(Flags::EXACT, x & ((1 << y) - 1) != 0) {
                return Ok(Val::Poison);
            }
            int(x >> y)
        }
        BinaryOp::AShr => {
            if poison(Flags::EXACT, x & ((1 << y) - 1) != 0) {
                return Ok(Val::Poison);
            }
            int((sx >> y) as u128)
        }
        BinaryOp::And => int(x & y),
        BinaryOp::Or => {
            if poison(Flags::DISJOINT, x & y != 0) {
                return Ok(Val::Poison);
            }
            int(x | y)
        }
        BinaryOp::Xor => int(x ^ y),
        _ => return unsupported("floating arithmetic on integers"),
    })
}


pub(crate) fn icmp(predicate: IntPredicate, a: Val, b: Val) -> Val {
    let bits = |value: &Val| match value {
        Val::Int { bits, width } => Some((*bits, *width)),
        Val::Ptr(address) => Some((u128::from(*address), 64)),
        _ => None,
    };
    let (Some((x, width)), Some((y, _))) = (bits(&a), bits(&b)) else { return Val::Poison };
    let (sx, sy) = (signed(x, width), signed(y, width));
    let truth = match predicate {
        IntPredicate::Eq => x == y,
        IntPredicate::Ne => x != y,
        IntPredicate::Ugt => x > y,
        IntPredicate::Uge => x >= y,
        IntPredicate::Ult => x < y,
        IntPredicate::Ule => x <= y,
        IntPredicate::Sgt => sx > sy,
        IntPredicate::Sge => sx >= sy,
        IntPredicate::Slt => sx < sy,
        IntPredicate::Sle => sx <= sy,
    };
    Val::Int { bits: u128::from(truth), width: 1 }
}


pub(crate) fn cast(types: &crate::types::Types, layout: &DataLayout, op: CastOp, value: Val, from: TypeId, to: TypeId, flags: Flags) -> Run<Val> {
    if value == Val::Poison {
        return Ok(Val::Poison);
    }
    let to_width = |types: &crate::types::Types| match types.get(to) {
        Type::Int(width) => *width,
        Type::Pointer(space) => layout.pointer(*space).bits,
        _ => 0,
    };
    let width = to_width(types);
    Ok(match (op, value) {
        (CastOp::Trunc, Val::Int { bits, width: from_width }) => {
            let kept = bits & mask(width);
            if flags.contains(Flags::NUW) && kept != bits || flags.contains(Flags::NSW) && signed(kept, width) != signed(bits, from_width) {
                return Ok(Val::Poison);
            }
            Val::Int { bits: kept, width }
        }
        (CastOp::ZExt, Val::Int { bits, width: from_width }) => {
            if flags.contains(Flags::NNEG) && signed(bits, from_width) < 0 {
                return Ok(Val::Poison);
            }
            Val::Int { bits, width }
        }
        (CastOp::SExt, Val::Int { bits, width: from_width }) => Val::Int { bits: signed(bits, from_width) as u128 & mask(width), width },
        (CastOp::FPTrunc | CastOp::FPExt, Val::Float(kind, bits)) => {
            let value = to_f64(kind, bits);
            let Type::Float(target) = types.get(to) else { unreachable!("a floating type") };
            from_f64(*target, value)
        }
        (CastOp::FPToSI | CastOp::FPToUI, Val::Float(kind, bits)) => {
            let value = to_f64(kind, bits).trunc();
            let (low, high) = if op == CastOp::FPToSI {
                (-(2f64.powi(width as i32 - 1)), 2f64.powi(width as i32 - 1))
            } else {
                (0.0, 2f64.powi(width as i32))
            };
            if value.is_nan() || value < low || value >= high {
                return Ok(Val::Poison);
            }
            Val::Int { bits: (value as i128) as u128 & mask(width), width }
        }
        (CastOp::SIToFP | CastOp::UIToFP, Val::Int { bits, width: from_width }) => {
            let value = if op == CastOp::SIToFP { signed(bits, from_width) as f64 } else { bits as f64 };
            let Type::Float(target) = types.get(to) else { unreachable!("a floating type") };
            from_f64(*target, value)
        }
        (CastOp::PtrToInt, Val::Ptr(address)) => Val::Int { bits: u128::from(address) & mask(width), width },
        (CastOp::IntToPtr, Val::Int { bits, .. }) => Val::Ptr(bits as u64),
        (CastOp::AddrSpaceCast | CastOp::BitCast, Val::Ptr(address)) => Val::Ptr(address),
        (CastOp::BitCast, Val::Int { bits, .. }) => match types.get(to) {
            Type::Float(kind) => Val::Float(*kind, bits as u64),
            _ => Val::Int { bits, width },
        },
        (CastOp::BitCast, Val::Float(_, bits)) => match types.get(to) {
            Type::Int(width) => Val::Int { bits: u128::from(bits), width: *width },
            Type::Float(kind) => Val::Float(*kind, bits),
            _ => return unsupported("a bitcast of a float"),
        },
        (op, value) => return unsupported(format!("{op:?} of {value:?} from {}", types.display(from))),
    })
}


fn insert(aggregate: Val, indices: &[u32], value: Val) -> Val {
    let Some((&first, rest)) = indices.split_first() else { return value };
    match aggregate {
        Val::Aggregate(mut members) => {
            let inner = std::mem::replace(&mut members[first as usize], Val::Poison);
            members[first as usize] = insert(inner, rest, value);
            Val::Aggregate(members)
        }
        _ => Val::Poison,
    }
}

fn to_f64(kind: FloatKind, bits: u64) -> f64 {
    match kind {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
        FloatKind::Double => f64::from_bits(bits),
    }
}

fn from_f64(kind: FloatKind, value: f64) -> Val {
    match kind {
        FloatKind::Float => Val::Float(kind, u64::from((value as f32).to_bits())),
        FloatKind::Double => Val::Float(kind, value.to_bits()),
    }
}

/// Floating arithmetic in the operands' own format.
fn float_binary(op: BinaryOp, kind: FloatKind, x: u64, y: u64) -> Val {
    match kind {
        FloatKind::Float => {
            let (a, b) = (f32::from_bits(x as u32), f32::from_bits(y as u32));
            let value = match op {
                BinaryOp::FAdd => a + b,
                BinaryOp::FSub => a - b,
                BinaryOp::FMul => a * b,
                BinaryOp::FDiv => a / b,
                _ => a % b,
            };
            Val::Float(kind, u64::from(value.to_bits()))
        }
        FloatKind::Double => {
            let (a, b) = (f64::from_bits(x), f64::from_bits(y));
            let value = match op {
                BinaryOp::FAdd => a + b,
                BinaryOp::FSub => a - b,
                BinaryOp::FMul => a * b,
                BinaryOp::FDiv => a / b,
                _ => a % b,
            };
            Val::Float(kind, value.to_bits())
        }
    }
}

fn fcmp(predicate: FloatPredicate, a: Val, b: Val) -> Val {
    let (Val::Float(kind, x), Val::Float(_, y)) = (a, b) else { return Val::Poison };
    let (x, y) = (to_f64(kind, x), to_f64(kind, y));
    let unordered = x.is_nan() || y.is_nan();
    let truth = match predicate {
        FloatPredicate::False => false,
        FloatPredicate::True => true,
        FloatPredicate::Ord => !unordered,
        FloatPredicate::Uno => unordered,
        FloatPredicate::Oeq => !unordered && x == y,
        FloatPredicate::Ogt => !unordered && x > y,
        FloatPredicate::Oge => !unordered && x >= y,
        FloatPredicate::Olt => !unordered && x < y,
        FloatPredicate::Ole => !unordered && x <= y,
        FloatPredicate::One => !unordered && x != y,
        FloatPredicate::Ueq => unordered || x == y,
        FloatPredicate::Ugt => unordered || x > y,
        FloatPredicate::Uge => unordered || x >= y,
        FloatPredicate::Ult => unordered || x < y,
        FloatPredicate::Ule => unordered || x <= y,
        FloatPredicate::Une => unordered || x != y,
    };
    Val::Int { bits: u128::from(truth), width: 1 }
}
