//! LLVM's `IRBuilder`: a module constructed in code, each instruction's
//! result type derived as LLVM derives it.

use crate::context::{Constant, ConstantId, ConstantKind, Context, GlobalId};
use crate::edit::Position;
use crate::intrinsics;
use crate::module::{BlockId, Function, GlobalKind, GlobalValue, GlobalVariable, Linkage, Module, Operand, ValueData, ValueDef, ValueId};
use crate::opcode::{BinaryOp, CallInfo, CastOp, FloatPredicate, Flags, IntPredicate, Opcode};
use crate::types::{Type, TypeId};

impl Module {
    fn add(&mut self, name: &str, linkage: Linkage, kind: GlobalKind) -> Result<GlobalId, String> {
        if self.named(name).is_some() {
            return Err(format!("@{name} is already defined"));
        }
        let id = GlobalId(self.globals.len() as u32);
        self.globals.push(GlobalValue { name: Some(name.to_owned()), linkage, unnamed_addr: Default::default(), address_space: 0, kind });
        Ok(id)
    }

    /// A function `@name` of type `ty`, a declaration until a builder gives
    /// it a block.
    pub fn add_function(&mut self, name: &str, ty: TypeId, linkage: Linkage) -> Result<GlobalId, String> {
        let Type::Function { parameters, .. } = self.context.types.get(ty).clone() else { return Err(format!("@{name}'s type is no function's")) };
        let void = self.context.types.void();
        let mut function = Function::new(ty, void);
        for (at, ty) in parameters.into_iter().enumerate() {
            function.parameters.push(ValueId(function.values.len() as u32));
            function.values.push(ValueData { ty, name: None, def: ValueDef::Argument(at as u32) });
            function.value_uses.push(Vec::new());
            function.parameter_attrs.push(Vec::new());
        }
        intrinsics::declare(&mut function, name);
        self.add(name, linkage, GlobalKind::Function(Box::new(function)))
    }

    pub fn add_variable(&mut self, name: &str, variable: GlobalVariable, linkage: Linkage) -> Result<GlobalId, String> {
        self.add(name, linkage, GlobalKind::Variable(variable))
    }

    /// The constant pointer to a global.
    pub fn reference(&mut self, global: GlobalId) -> ConstantId {
        let ty = self.context.types.ptr(self.global(global).address_space);
        self.context.constant(Constant { ty, kind: ConstantKind::Global(global) })
    }

    /// A builder for `function`'s body, placed nowhere until `position`.
    pub fn builder(&mut self, function: GlobalId) -> Builder<'_> {
        let GlobalKind::Function(body) = &mut self.globals[function.0 as usize].kind else { panic!("@{function:?} is a variable") };
        Builder { context: &mut self.context, function: body, block: None }
    }
}

/// Appends instructions at the end of one block.
pub struct Builder<'m> {
    pub context: &'m mut Context,
    pub function: &'m mut Function,
    block: Option<BlockId>,
}

impl Builder<'_> {
    /// A new block, last in the layout.
    pub fn block(&mut self, name: &str) -> BlockId {
        let block = self.function.create_block(Some(name).filter(|one| !one.is_empty()));
        self.function.insert_block(block, None).expect("a new block");
        block
    }

    pub fn position(&mut self, block: BlockId) {
        self.block = Some(block);
    }

    pub fn parameter(&self, at: usize) -> Operand {
        Operand::Value(self.function.parameters()[at])
    }

    pub fn int(&mut self, bits: u32, value: i128) -> Operand {
        let ty = self.context.types.int(bits);
        Operand::Constant(self.context.int(ty, value))
    }

    pub fn type_of(&self, operand: Operand) -> TypeId {
        self.function.operand_type(self.context, operand).expect("a value")
    }

    fn emit(&mut self, opcode: Opcode, ty: TypeId, operands: Vec<Operand>, flags: Flags, name: &str) -> Option<Operand> {
        let inst = self.function.create_instruction(opcode, ty, operands, flags, Some(name).filter(|one| !one.is_empty()));
        self.function.insert(inst, Position::End(self.block.expect("a position"))).expect("a placed block");
        self.function.instruction(inst).result.map(Operand::Value)
    }

    fn value(&mut self, opcode: Opcode, ty: TypeId, operands: Vec<Operand>, flags: Flags, name: &str) -> Operand {
        self.emit(opcode, ty, operands, flags, name).expect("a result")
    }

    fn effect(&mut self, opcode: Opcode, operands: Vec<Operand>) {
        let void = self.context.types.void();
        self.emit(opcode, void, operands, Flags::default(), "");
    }

    pub fn binary(&mut self, op: BinaryOp, a: Operand, b: Operand, flags: Flags, name: &str) -> Operand {
        let ty = self.type_of(a);
        self.value(Opcode::Binary(op), ty, vec![a, b], flags, name)
    }

    pub fn fneg(&mut self, a: Operand, name: &str) -> Operand {
        let ty = self.type_of(a);
        self.value(Opcode::FNeg, ty, vec![a], Flags::default(), name)
    }

    pub fn icmp(&mut self, predicate: IntPredicate, a: Operand, b: Operand, name: &str) -> Operand {
        let ty = self.context.types.int(1);
        self.value(Opcode::ICmp(predicate), ty, vec![a, b], Flags::default(), name)
    }

    pub fn fcmp(&mut self, predicate: FloatPredicate, a: Operand, b: Operand, name: &str) -> Operand {
        let ty = self.context.types.int(1);
        self.value(Opcode::FCmp(predicate), ty, vec![a, b], Flags::default(), name)
    }

    pub fn cast(&mut self, op: CastOp, a: Operand, to: TypeId, name: &str) -> Operand {
        self.value(Opcode::Cast(op), to, vec![a], Flags::default(), name)
    }

    pub fn select(&mut self, condition: Operand, a: Operand, b: Operand, name: &str) -> Operand {
        let ty = self.type_of(a);
        self.value(Opcode::Select, ty, vec![condition, a, b], Flags::default(), name)
    }

    pub fn extract_value(&mut self, aggregate: Operand, index: u32, name: &str) -> Operand {
        let from = self.type_of(aggregate);
        let ty = self.context.types.member(from, u64::from(index)).expect("a member");
        self.value(Opcode::ExtractValue(vec![index]), ty, vec![aggregate], Flags::default(), name)
    }

    /// An `alloca` in the entry block, as LLVM's frontends place them.
    pub fn alloca(&mut self, allocated: TypeId, name: &str) -> Operand {
        let entry = self.function.entry().expect("an entry block");
        let ty = self.context.types.ptr(0);
        let opcode = Opcode::Alloca { allocated, align: None, address_space: 0 };
        let inst = self.function.create_instruction(opcode, ty, Vec::new(), Flags::default(), Some(name).filter(|one| !one.is_empty()));
        let first = self.function.block(entry).instructions().iter().copied().find(|&one| !matches!(self.function.instruction(one).opcode, Opcode::Alloca { .. }));
        let position = first.map_or(Position::End(entry), Position::Before);
        self.function.insert(inst, position).expect("the entry block");
        Operand::Value(self.function.instruction(inst).result.expect("a pointer"))
    }

    pub fn load(&mut self, ty: TypeId, pointer: Operand, volatile: bool, name: &str) -> Operand {
        self.value(Opcode::Load { align: None, volatile }, ty, vec![pointer], Flags::default(), name)
    }

    pub fn store(&mut self, value: Operand, pointer: Operand, volatile: bool) {
        self.effect(Opcode::Store { align: None, volatile }, vec![value, pointer]);
    }

    pub fn gep(&mut self, source: TypeId, pointer: Operand, indices: &[Operand], flags: Flags, name: &str) -> Operand {
        let ty = self.type_of(pointer);
        let operands = std::iter::once(pointer).chain(indices.iter().copied()).collect();
        self.value(Opcode::GetElementPtr { source }, ty, operands, flags, name)
    }

    /// A call through `callee` of type `function_type`; its result, unless
    /// `void`.
    pub fn call(&mut self, function_type: TypeId, callee: Operand, arguments: &[Operand], name: &str) -> Option<Operand> {
        self.call_as(0, function_type, callee, arguments, name)
    }

    /// A call by calling convention `convention`, which must be the callee's.
    pub fn call_as(&mut self, convention: u32, function_type: TypeId, callee: Operand, arguments: &[Operand], name: &str) -> Option<Operand> {
        let Type::Function { returns, .. } = self.context.types.get(function_type) else { panic!("a function type") };
        let returns = *returns;
        let info = CallInfo {
            function_type,
            calling_convention: convention,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); arguments.len()],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let operands = arguments.iter().copied().chain([callee]).collect();
        self.emit(Opcode::Call(Box::new(info)), returns, operands, Flags::default(), name)
    }

    pub fn phi(&mut self, ty: TypeId, incoming: &[(Operand, BlockId)], name: &str) -> Operand {
        let operands = incoming.iter().flat_map(|&(value, block)| [value, Operand::Block(block)]).collect();
        self.value(Opcode::Phi, ty, operands, Flags::default(), name)
    }

    pub fn br(&mut self, target: BlockId) {
        self.effect(Opcode::Br, vec![Operand::Block(target)]);
    }

    pub fn cond_br(&mut self, condition: Operand, taken: BlockId, otherwise: BlockId) {
        self.effect(Opcode::Br, vec![condition, Operand::Block(taken), Operand::Block(otherwise)]);
    }

    pub fn switch(&mut self, value: Operand, default: BlockId, cases: &[(Operand, BlockId)]) {
        let operands = [value, Operand::Block(default)].into_iter().chain(cases.iter().flat_map(|&(case, block)| [case, Operand::Block(block)])).collect();
        self.effect(Opcode::Switch, operands);
    }

    pub fn ret(&mut self, value: Option<Operand>) {
        self.effect(Opcode::Ret, value.into_iter().collect());
    }

    pub fn unreachable(&mut self) {
        self.effect(Opcode::Unreachable, Vec::new());
    }
}
