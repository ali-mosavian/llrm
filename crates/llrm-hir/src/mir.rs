//! HIR to MIR, as clang's CodeGen emits LLVM IR: data objects become
//! globals, local places allocas, HIR values SSA values. What MIR cannot
//! hold yet is refused whole with the reason: a function is left declared,
//! a data object an external global, so what refers to them still verifies.

use std::collections::HashMap;

use llrm_mir::build::Builder;
use llrm_mir::{
    BinaryOp, BlockId, CastOp, Constant, ConstantId, ConstantKind, FloatKind, FloatPredicate, Flags, GlobalId, GlobalVariable, IntPredicate,
    Linkage, Module, Operand as Value, Type, TypeId, Types,
};

use crate::model::{self, AddressKind, Number, Op, Operand, Storage, TerminatorKind, TypeKind};

/// The layout BC's objects fix: 16-bit near pointers, 32-bit far ones
/// indexing by 16 bits, and 16-bit alignment.
pub const DATALAYOUT: &str = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16";

/// The prefix of a runtime routine's name: a callee the module does not
/// declare.
pub const RUNTIME: &str = "llrm.qb.";

/// One HIR module's MIR, and what it could not hold.
pub struct Emitted {
    pub module: Module,
    /// Each refused global's name, and why.
    pub refused: Vec<(String, String)>,
}

pub fn emit(program: &model::Program) -> Vec<Emitted> {
    program.modules.iter().map(emit_module).collect()
}

type Emit<T> = Result<T, String>;

/// A HIR type's MIR type as a value.
fn value_type(types: &mut Types, hir: &model::Type) -> Emit<TypeId> {
    let bits = u32::try_from(hir.width * 8).map_err(|_| format!("type {} is {} bytes wide", hir.name, hir.width))?;
    Ok(match hir.kind {
        TypeKind::Void => types.void(),
        TypeKind::Boolean | TypeKind::Integer => types.int(bits),
        TypeKind::Float => match hir.width {
            4 => types.intern(Type::Float(FloatKind::Float)),
            8 => types.intern(Type::Float(FloatKind::Double)),
            _ => return Err(format!("a {}-byte float", hir.width)),
        },
        TypeKind::Pointer if hir.address == AddressKind::Segment => types.int(16),
        TypeKind::Pointer => types.ptr(if hir.width == 4 { 1 } else { 0 }),
        TypeKind::Array | TypeKind::Opaque => return Err(format!("a value of type {}", hir.name)),
    })
}

/// A HIR type's MIR type in memory: bytes where it is no value.
fn stored_type(types: &mut Types, hir: &model::Type) -> Emit<TypeId> {
    match hir.kind {
        TypeKind::Array | TypeKind::Opaque => {
            let byte = types.int(8);
            Ok(types.intern(Type::Array { element: byte, count: hir.width as u64 }))
        }
        _ => value_type(types, hir),
    }
}

struct Tables<'h> {
    types: HashMap<i64, &'h model::Type>,
    callables: HashMap<&'h str, &'h model::Callable>,
    /// Each data object's global, by its id: a place's symbol.
    data: HashMap<i64, ConstantId>,
    /// Each callee's function and its declared type, by HIR name.
    callees: HashMap<String, ConstantId>,
}

fn emit_module(hir: &model::Module) -> Emitted {
    let mut module = Module { datalayout: Some(DATALAYOUT.to_owned()), ..Module::default() };
    let mut refused = Vec::new();
    let mut tables = Tables {
        types: hir.types.iter().map(|one| (one.id, one)).collect(),
        callables: hir.callables.iter().map(|one| (one.name.as_str(), one)).collect(),
        data: HashMap::new(),
        callees: HashMap::new(),
    };
    for object in &hir.data {
        let (global, why) = data_object(&mut module, object);
        if let Some(why) = why {
            refused.push((object.name.clone(), why));
        }
        let reference = module.reference(global);
        tables.data.insert(object.id, reference);
    }
    let mut functions = Vec::new();
    for function in &hir.functions {
        match declare(&mut module, &tables, function) {
            Ok(global) => {
                let reference = module.reference(global);
                tables.callees.insert(function.name.clone(), reference);
                functions.push((function, Some(global)));
            }
            Err(why) => {
                refused.push((function.name.clone(), why));
                functions.push((function, None));
            }
        }
    }
    for (function, _) in &functions {
        if let Err(why) = declare_outside(&mut module, &mut tables, function) {
            refused.push((function.name.clone(), why));
        }
    }
    for (function, global) in functions {
        let Some(global) = global else { continue };
        let mut builder = module.builder(global);
        let emitted = Body::new(&mut builder, &tables, function).and_then(|mut body| body.run());
        if emitted.is_err() {
            builder.function.delete_body();
        }
        builder.function.take_changes();
        if let Err(why) = emitted {
            // A declaration is external, as LLVM's `deleteBody` leaves it.
            module.globals[global.0 as usize].linkage = Linkage::External;
            refused.push((function.name.clone(), why));
        }
    }
    Emitted { module, refused }
}

/// A data object's bytes as a global, or an external one and the reason.
fn data_object(module: &mut Module, object: &model::DataObject) -> (GlobalId, Option<String>) {
    let types = &mut module.context.types;
    let byte = types.int(8);
    let ty = types.intern(Type::Array { element: byte, count: object.bytes.len() as u64 });
    let why = object.relocations.first().map(|one| format!("a {} relocation in its data", one.address));
    let bytes: Vec<u8> = object.bytes.iter().map(|&one| one as u8).collect();
    let initializer = why.is_none().then(|| {
        let kind = if bytes.iter().all(|&one| one == 0) { ConstantKind::Zero } else { ConstantKind::Bytes(bytes) };
        module.context.constant(Constant { ty, kind })
    });
    let linkage = match (why.is_some(), object.linkage) {
        (true, _) | (false, model::DataLinkage::External) => Linkage::External,
        (false, model::DataLinkage::Internal) => Linkage::Internal,
    };
    let variable = GlobalVariable { ty, constant: object.readonly && why.is_none(), initializer, align: None };
    let global = add_unique(module, &object.name, |module, name| module.add_variable(name, variable.clone(), linkage));
    module.globals[global.0 as usize].address_space = if object.address == AddressKind::Far { 1 } else { 0 };
    (global, why)
}

/// `name`, or `name.N` for the least N free, as LLVM uniques names.
fn add_unique(module: &mut Module, name: &str, mut add: impl FnMut(&mut Module, &str) -> Emit<GlobalId>) -> GlobalId {
    std::iter::once(name.to_owned()).chain((1..).map(|n| format!("{name}.{n}"))).find_map(|one| add(module, &one).ok()).expect("some name is free")
}

fn function_type(types: &mut Types, returns: TypeId, parameters: Vec<TypeId>) -> TypeId {
    types.intern(Type::Function { returns, parameters, variadic: false })
}

fn declare(module: &mut Module, tables: &Tables, function: &model::Function) -> Emit<GlobalId> {
    let values: HashMap<i64, i64> = function.values.iter().map(|one| (one.id, one.r#type)).collect();
    let types = &mut module.context.types;
    let returns = value_type(types, tables.types[&function.result_type])?;
    let parameters = function.parameters.iter().map(|one| value_type(types, tables.types[&values[one]])).collect::<Emit<_>>()?;
    let ty = function_type(types, returns, parameters);
    let linkage = match function.linkage {
        model::FunctionLinkage::Internal => Linkage::Internal,
        model::FunctionLinkage::External => Linkage::External,
    };
    module.add_function(&function.name, ty, linkage)
}

/// Declares what `function` calls or names outside the module: runtime
/// routines, typed as the first call gives them, and external places.
fn declare_outside(module: &mut Module, tables: &mut Tables, function: &model::Function) -> Emit<()> {
    let values: HashMap<i64, i64> = function.values.iter().map(|one| (one.id, one.r#type)).collect();
    let places: HashMap<i64, &model::Place> = function.places.iter().map(|one| (one.id, one)).collect();
    for place in &function.places {
        if !matches!(place.storage, Storage::Local | Storage::Parameter) && !tables.data.contains_key(&place.symbol) {
            let ty = stored_type(&mut module.context.types, tables.types[&place.r#type])?;
            let variable = GlobalVariable { ty, constant: false, initializer: None, align: None };
            let global = add_unique(module, &place.name, |module, name| module.add_variable(name, variable.clone(), Linkage::External));
            let reference = module.reference(global);
            tables.data.insert(place.symbol, reference);
        }
    }
    for instruction in function.blocks.iter().flat_map(|one| &one.instructions) {
        let Some(callee) = instruction.callee.as_deref().filter(|_| instruction.op == Op::Call) else { continue };
        if tables.callees.contains_key(callee) {
            continue;
        }
        let types = &mut module.context.types;
        let parameters = instruction.operands.iter().map(|one| value_type(types, tables.types[&operand_type(one, &values, &places)])).collect::<Emit<_>>()?;
        let returns = match instruction.results.first() {
            Some(result) => value_type(types, tables.types[&values[result]])?,
            None => types.void(),
        };
        let ty = function_type(types, returns, parameters);
        let name = if tables.callables.contains_key(callee) { callee.to_owned() } else { format!("{RUNTIME}{callee}") };
        let global = module.add_function(&name, ty, Linkage::External)?;
        let reference = module.reference(global);
        tables.callees.insert(callee.to_owned(), reference);
    }
    Ok(())
}

/// An operand's HIR type: a place's is what it holds.
fn operand_type(operand: &Operand, values: &HashMap<i64, i64>, places: &HashMap<i64, &model::Place>) -> i64 {
    match operand {
        Operand::ValueRef(one) => values[&one.value],
        Operand::Constant(one) => one.r#type,
        Operand::PlaceRef(one) => places[&one.place].r#type,
        Operand::ArrayElement(one) => places[&one.place].r#type,
        Operand::ProjectedPlace(one) => one.r#type,
        Operand::IndirectPlace(one) => one.r#type,
        Operand::DescriptorPlace(one) => one.r#type,
    }
}

struct Body<'b, 'm, 'h> {
    b: &'b mut Builder<'m>,
    tables: &'b Tables<'h>,
    function: &'h model::Function,
    value_types: HashMap<i64, i64>,
    places: HashMap<i64, &'h model::Place>,
    blocks: HashMap<i64, BlockId>,
    values: HashMap<i64, Value>,
    slots: HashMap<i64, Value>,
}

impl<'b, 'm, 'h> Body<'b, 'm, 'h> {
    fn new(b: &'b mut Builder<'m>, tables: &'b Tables<'h>, function: &'h model::Function) -> Emit<Self> {
        if function.error_handler.is_some() {
            return Err("an ON ERROR handler".to_owned());
        }
        if !function.external_entries.is_empty() {
            return Err("an alternate entry".to_owned());
        }
        let values = function.parameters.iter().enumerate().map(|(at, &one)| (one, b.parameter(at))).collect();
        Ok(Self {
            b,
            tables,
            function,
            value_types: function.values.iter().map(|one| (one.id, one.r#type)).collect(),
            places: function.places.iter().map(|one| (one.id, one)).collect(),
            blocks: HashMap::new(),
            values,
            slots: HashMap::new(),
        })
    }

    fn run(&mut self) -> Emit<()> {
        let entry = self.function.blocks.iter().find(|one| one.id == self.function.entry).ok_or("no entry block")?;
        let order = std::iter::once(entry).chain(self.function.blocks.iter().filter(|one| one.id != self.function.entry));
        for block in order.clone() {
            let id = self.b.block(&format!("b{}", block.id));
            self.blocks.insert(block.id, id);
        }
        for block in order {
            self.b.position(self.blocks[&block.id]);
            for instruction in &block.instructions {
                self.instruction(instruction)?;
            }
            self.terminator(&block.terminator)?;
        }
        Ok(())
    }

    fn hir_type(&self, id: i64) -> &'h model::Type {
        self.tables.types[&id]
    }

    fn ty(&mut self, id: i64) -> Emit<TypeId> {
        value_type(&mut self.b.context.types, self.tables.types[&id])
    }

    fn result_type(&mut self, result: i64) -> Emit<TypeId> {
        self.ty(self.value_types[&result])
    }

    fn operand_hir_type(&self, operand: &Operand) -> &'h model::Type {
        self.hir_type(operand_type(operand, &self.value_types, &self.places))
    }

    fn block(&self, id: i64) -> BlockId {
        self.blocks[&id]
    }

    /// An operand's value; a place's is loaded.
    fn value(&mut self, operand: &Operand) -> Emit<Value> {
        match operand {
            Operand::ValueRef(one) => self.values.get(&one.value).copied().ok_or_else(|| format!("value {} used before its definition", one.value)),
            Operand::Constant(one) => self.constant(one),
            place => {
                let (pointer, ty, volatile) = self.place(place)?;
                Ok(self.b.load(ty, pointer, volatile, ""))
            }
        }
    }

    fn constant(&mut self, constant: &model::Constant) -> Emit<Value> {
        let ty = self.ty(constant.r#type)?;
        let kind = match (self.b.context.types.get(ty), constant.value) {
            (Type::Int(_), Number::Int(n)) => ConstantKind::Int(n as u128),
            (Type::Float(FloatKind::Float), Number::Int(n)) => ConstantKind::Float(u64::from((n as f32).to_bits())),
            (Type::Float(FloatKind::Float), Number::Float(x)) => ConstantKind::Float(u64::from((x as f32).to_bits())),
            (Type::Float(FloatKind::Double), Number::Int(n)) => ConstantKind::Float((n as f64).to_bits()),
            (Type::Float(FloatKind::Double), Number::Float(x)) => ConstantKind::Float(x.to_bits()),
            (Type::Pointer(_), Number::Int(0)) => ConstantKind::Null,
            (other, value) => return Err(format!("a constant {value:?} of {other:?}")),
        };
        let kind = match kind {
            ConstantKind::Int(bits) => ConstantKind::Int(bits & llrm_mir::context::mask(self.b.context.types.int_bits(ty).expect("an integer"))),
            other => other,
        };
        Ok(Value::Constant(self.b.context.constant(Constant { ty, kind })))
    }

    /// A place's address, what it holds, and whether it is volatile.
    fn place(&mut self, operand: &Operand) -> Emit<(Value, TypeId, bool)> {
        match operand {
            Operand::PlaceRef(one) => {
                let place = self.places[&one.place];
                let ty = stored_type(&mut self.b.context.types, self.tables.types[&place.r#type])?;
                let pointer = match place.storage {
                    Storage::Local => match self.slots.get(&place.id) {
                        Some(&slot) => slot,
                        None => {
                            let slot = self.b.alloca(ty, "");
                            self.slots.insert(place.id, slot);
                            slot
                        }
                    },
                    Storage::Parameter => return Err("a parameter-storage place".to_owned()),
                    _ => {
                        let global = Value::Constant(self.tables.data[&place.symbol]);
                        self.offset(global, place.offset, false)
                    }
                };
                Ok((pointer, ty, place.volatile))
            }
            Operand::IndirectPlace(one) => {
                let base = self.values.get(&one.base).copied().ok_or_else(|| format!("value {} used before its definition", one.base))?;
                let ty = stored_type(&mut self.b.context.types, self.tables.types[&one.r#type])?;
                Ok((self.offset(base, one.offset, one.inbounds), ty, one.volatile))
            }
            Operand::ArrayElement(_) => Err("an array element".to_owned()),
            Operand::ProjectedPlace(_) => Err("a projected place".to_owned()),
            Operand::DescriptorPlace(_) => Err("a string descriptor field".to_owned()),
            Operand::ValueRef(_) | Operand::Constant(_) => Err("a value where a place belongs".to_owned()),
        }
    }

    /// `pointer` advanced by `offset` bytes.
    fn offset(&mut self, pointer: Value, offset: i64, inbounds: bool) -> Value {
        if offset == 0 {
            return pointer;
        }
        let byte = self.b.context.types.int(8);
        let index = self.b.int(16, i128::from(offset));
        let flags = if inbounds { Flags::INBOUNDS } else { Flags::default() };
        self.b.gep(byte, pointer, &[index], flags, "")
    }

    fn define(&mut self, instruction: &model::Instruction, value: Value) {
        if let Some(&result) = instruction.results.first() {
            self.values.insert(result, value);
        }
    }

    fn operands(&mut self, instruction: &model::Instruction) -> Emit<Vec<Value>> {
        instruction.operands.iter().map(|one| self.value(one)).collect()
    }

    fn instruction(&mut self, instruction: &model::Instruction) -> Emit<()> {
        let op = instruction.op;
        let binary = match op {
            Op::Add => Some(BinaryOp::Add),
            Op::Sub => Some(BinaryOp::Sub),
            Op::Mul => Some(BinaryOp::Mul),
            Op::Div => Some(BinaryOp::SDiv),
            Op::Rem => Some(BinaryOp::SRem),
            Op::Udiv => Some(BinaryOp::UDiv),
            Op::Urem => Some(BinaryOp::URem),
            Op::And => Some(BinaryOp::And),
            Op::Or => Some(BinaryOp::Or),
            Op::Xor => Some(BinaryOp::Xor),
            Op::Shl => Some(BinaryOp::Shl),
            Op::Shr => Some(BinaryOp::LShr),
            Op::Sar => Some(BinaryOp::AShr),
            Op::Fadd => Some(BinaryOp::FAdd),
            Op::Fsub => Some(BinaryOp::FSub),
            Op::Fmul => Some(BinaryOp::FMul),
            Op::Fdiv => Some(BinaryOp::FDiv),
            _ => None,
        };
        if let Some(binary) = binary {
            let [a, b] = self.operands(instruction)?[..] else { return Err(format!("{op} without two operands")) };
            // A shift count is its own width; LLVM's is the shifted value's.
            let b = if matches!(op, Op::Shl | Op::Shr | Op::Sar) { self.count(b, self.b.type_of(a))? } else { b };
            let result = self.b.binary(binary, a, b, Flags::default(), "");
            self.define(instruction, result);
            return Ok(());
        }
        if let Some((signed, unsigned, float)) = comparison(op) {
            let [a, b] = self.operands(instruction)?[..] else { return Err(format!("{op} without two operands")) };
            let is_float = matches!(self.b.context.types.get(self.b.type_of(a)), Type::Float(_));
            let truth = match (is_float, instruction.op) {
                (true, _) => self.b.fcmp(float, a, b, ""),
                (false, Op::Below | Op::BelowEq | Op::Above | Op::AboveEq) => self.b.icmp(unsigned, a, b, ""),
                (false, _) => self.b.icmp(signed, a, b, ""),
            };
            let ty = self.result_type(instruction.results[0])?;
            // BASIC's truth is all ones.
            let result = self.b.cast(CastOp::SExt, truth, ty, "");
            self.define(instruction, result);
            return Ok(());
        }
        match op {
            Op::Copy | Op::Load => {
                let value = self.value(&instruction.operands[0])?;
                self.define(instruction, value);
            }
            Op::Store => {
                let (pointer, _, volatile) = self.place(&instruction.operands[0])?;
                let value = self.value(&instruction.operands[1])?;
                self.b.store(value, pointer, volatile);
            }
            Op::Address => {
                let (pointer, _, _) = self.place(&instruction.operands[0])?;
                let ty = self.result_type(instruction.results[0])?;
                let pointer = match (self.b.context.types.get(self.b.type_of(pointer)), self.b.context.types.get(ty)) {
                    (Type::Pointer(from), Type::Pointer(to)) if from == to => pointer,
                    (Type::Pointer(_), Type::Pointer(_)) => self.b.cast(CastOp::AddrSpaceCast, pointer, ty, ""),
                    (_, other) => return Err(format!("an address as {other:?}")),
                };
                self.define(instruction, pointer);
            }
            Op::Truncate | Op::SignExtend | Op::ZeroExtend | Op::Convert => {
                let value = self.value(&instruction.operands[0])?;
                let signed = self.operand_hir_type(&instruction.operands[0]).signed != Some(false);
                let ty = self.result_type(instruction.results[0])?;
                let result = self.convert(value, signed, ty)?;
                self.define(instruction, result);
            }
            Op::Divmod | Op::Udivmod => {
                let [a, b] = self.operands(instruction)?[..] else { return Err(format!("{op} without two operands")) };
                let (quotient, remainder) = if op == Op::Divmod { (BinaryOp::SDiv, BinaryOp::SRem) } else { (BinaryOp::UDiv, BinaryOp::URem) };
                let q = self.b.binary(quotient, a, b, Flags::default(), "");
                let r = self.b.binary(remainder, a, b, Flags::default(), "");
                self.values.insert(instruction.results[0], q);
                self.values.insert(instruction.results[1], r);
            }
            Op::Neg => {
                let value = self.value(&instruction.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a negated non-integer")?;
                let zero = self.b.int(bits, 0);
                let result = self.b.binary(BinaryOp::Sub, zero, value, Flags::default(), "");
                self.define(instruction, result);
            }
            Op::Not => {
                let value = self.value(&instruction.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a complemented non-integer")?;
                let ones = self.b.int(bits, -1);
                let result = self.b.binary(BinaryOp::Xor, value, ones, Flags::default(), "");
                self.define(instruction, result);
            }
            Op::Fneg => {
                let value = self.value(&instruction.operands[0])?;
                let result = self.b.fneg(value, "");
                self.define(instruction, result);
            }
            Op::Call => {
                let callee = instruction.callee.as_deref().ok_or("a call without a callee")?;
                let arguments = self.operands(instruction)?;
                let returns = match instruction.results.first() {
                    Some(&result) => self.result_type(result)?,
                    None => self.b.context.types.void(),
                };
                let parameters = arguments.iter().map(|&one| self.b.type_of(one)).collect();
                let ty = function_type(&mut self.b.context.types, returns, parameters);
                let callee = Value::Constant(self.tables.callees[callee]);
                if let Some(result) = self.b.call(ty, callee, &arguments, "") {
                    self.define(instruction, result);
                }
            }
            other => return Err(format!("HIR {other}")),
        }
        Ok(())
    }

    /// A shift count at the shifted value's width.
    fn count(&mut self, value: Value, ty: TypeId) -> Emit<Value> {
        let from = self.b.type_of(value);
        if from == ty {
            return Ok(value);
        }
        let types = &self.b.context.types;
        match (types.int_bits(from), types.int_bits(ty)) {
            (Some(a), Some(b)) if a < b => Ok(self.b.cast(CastOp::ZExt, value, ty, "")),
            (Some(_), Some(_)) => Ok(self.b.cast(CastOp::Trunc, value, ty, "")),
            _ => Err(format!("operands of {} and {}", types.display(from), types.display(ty))),
        }
    }

    fn convert(&mut self, value: Value, signed: bool, to: TypeId) -> Emit<Value> {
        let from = self.b.type_of(value);
        if from == to {
            return Ok(value);
        }
        let types = &self.b.context.types;
        let op = match (types.get(from), types.get(to)) {
            (Type::Int(a), Type::Int(b)) if a > b => CastOp::Trunc,
            (Type::Int(_), Type::Int(_)) if signed => CastOp::SExt,
            (Type::Int(_), Type::Int(_)) => CastOp::ZExt,
            (Type::Int(_), Type::Float(_)) if signed => CastOp::SIToFP,
            (Type::Int(_), Type::Float(_)) => CastOp::UIToFP,
            (Type::Float(FloatKind::Float), Type::Float(FloatKind::Double)) => CastOp::FPExt,
            (Type::Float(FloatKind::Double), Type::Float(FloatKind::Float)) => CastOp::FPTrunc,
            (Type::Float(_), Type::Int(_)) => return Err("a float rounded to an integer".to_owned()),
            (a, b) => return Err(format!("a conversion from {a:?} to {b:?}")),
        };
        Ok(self.b.cast(op, value, to, ""))
    }

    fn terminator(&mut self, terminator: &model::Terminator) -> Emit<()> {
        match terminator.kind {
            TerminatorKind::Jump => self.b.br(self.block(terminator.targets[0])),
            TerminatorKind::Branch => {
                let value = self.value(&terminator.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a branch on a non-integer")?;
                let zero = self.b.int(bits, 0);
                let condition = self.b.icmp(IntPredicate::Ne, value, zero, "");
                self.b.cond_br(condition, self.block(terminator.targets[0]), self.block(terminator.targets[1]));
            }
            TerminatorKind::Switch => {
                let value = self.value(&terminator.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a switch on a non-integer")?;
                let cases: Vec<(Value, BlockId)> = terminator.cases.iter().map(|&(case, target)| (self.b.int(bits, i128::from(case)), self.block(target))).collect();
                self.b.switch(value, self.block(terminator.targets[0]), &cases);
            }
            TerminatorKind::Return => {
                let value = terminator.operands.first().map(|one| self.value(one)).transpose()?;
                self.b.ret(value);
            }
            TerminatorKind::Unreachable => self.b.unreachable(),
        }
        Ok(())
    }
}

/// A comparison's signed, unsigned and floating predicates.
fn comparison(op: Op) -> Option<(IntPredicate, IntPredicate, FloatPredicate)> {
    Some(match op {
        Op::Eq => (IntPredicate::Eq, IntPredicate::Eq, FloatPredicate::Oeq),
        Op::Ne => (IntPredicate::Ne, IntPredicate::Ne, FloatPredicate::Une),
        Op::Lt | Op::Below => (IntPredicate::Slt, IntPredicate::Ult, FloatPredicate::Olt),
        Op::Le | Op::BelowEq => (IntPredicate::Sle, IntPredicate::Ule, FloatPredicate::Ole),
        Op::Gt | Op::Above => (IntPredicate::Sgt, IntPredicate::Ugt, FloatPredicate::Ogt),
        Op::Ge | Op::AboveEq => (IntPredicate::Sge, IntPredicate::Uge, FloatPredicate::Oge),
        _ => return None,
    })
}
