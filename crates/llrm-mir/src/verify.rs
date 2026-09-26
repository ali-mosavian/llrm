//! LLVM's `Verifier` rules for MIR's subset. Each finding names the
//! function, and the block or instruction at fault. `opt -passes=verify`
//! is its oracle: `tools/mir-oracle.sh` holds the two to the same verdicts.

use std::collections::{HashMap, HashSet};

use crate::context::{ConstantKind, Context};
use crate::dominators::DominatorTree;
use crate::intrinsics::{self, Intrinsic};
use crate::module::{BlockId, Function, GlobalKind, InstId, Module, Operand, ValueDef};
use crate::opcode::{BinaryOp, CastOp, Opcode};
use crate::types::{Type, TypeId};

pub fn verify(module: &Module) -> Vec<String> {
    let mut out = Vec::new();
    for global in &module.globals {
        if let GlobalKind::Function(function) = &global.kind {
            let name = global.name.as_deref().unwrap_or("<unnamed>");
            if intrinsics::is_reserved(name) {
                let problem = match Intrinsic::named(name) {
                    _ if !function.is_declaration() => Err("llvm intrinsics cannot be defined!".to_owned()),
                    None => Err("an intrinsic MIR does not have".to_owned()),
                    Some(intrinsic) => intrinsic.check(name, &module.context.types, function.ty),
                };
                if let Err(problem) = problem {
                    out.push(format!("@{name}: {problem}"));
                }
            }
            let mut checker = Checker { module, context: &module.context, function, errors: Vec::new() };
            checker.function();
            out.extend(checker.errors.into_iter().map(|one| format!("@{name}: {one}")));
        }
    }
    out
}

struct Checker<'a> {
    module: &'a Module,
    context: &'a Context,
    function: &'a Function,
    errors: Vec<String>,
}

impl Checker<'_> {
    fn fail(&mut self, message: String) {
        self.errors.push(message);
    }

    fn ty(&self, ty: TypeId) -> &Type {
        self.context.types.get(ty)
    }

    fn show(&self, ty: TypeId) -> String {
        self.context.types.display(ty)
    }

    fn operand_type(&self, operand: Operand) -> Option<TypeId> {
        match operand {
            Operand::Value(id) => Some(self.function.value(id).ty),
            Operand::Constant(id) => Some(self.context.get(id).ty),
            Operand::Block(_) => None,
        }
    }

    /// The type of the intrinsic `operand` names, if it names one.
    fn intrinsic(&self, operand: Operand) -> Option<TypeId> {
        let Operand::Constant(id) = operand else { return None };
        let ConstantKind::Global(global) = self.context.get(id).kind else { return None };
        let global = self.module.global(global);
        let function = global.function()?;
        global.name.as_deref().is_some_and(intrinsics::is_reserved).then_some(function.ty)
    }

    fn at(&self, inst: InstId) -> String {
        let block = self.function.parent(inst).map_or("?".to_owned(), |block| self.block_name(block));
        let mnemonic = self.function.instruction(inst).opcode.mnemonic();
        format!("{mnemonic} in {block}")
    }

    fn block_name(&self, block: BlockId) -> String {
        self.function.block(block).name.clone().map_or_else(|| format!("block {}", block.0), |name| format!("%{name}"))
    }

    fn function(&mut self) {
        let function = self.function;
        let (returns, parameters, _) = self.module.signature(function.ty);
        if parameters.len() != function.parameters().len() {
            self.fail(format!("{} parameters for a type of {}", function.parameters().len(), parameters.len()));
        }
        if function.is_declaration() {
            return;
        }
        for problem in function.check_uses() {
            self.fail(format!("use lists: {problem}"));
        }
        let tree = DominatorTree::new(function);
        let entry = function.entry().expect("a body has an entry");
        if !function.predecessors(entry).is_empty() {
            self.fail("the entry block has predecessors".to_owned());
        }
        for &block in function.layout() {
            self.block(block, block == entry);
            for &inst in function.block(block).instructions() {
                self.instruction(inst, returns);
                self.dominance(&tree, inst);
            }
        }
    }

    fn block(&mut self, block: BlockId, entry: bool) {
        let list = self.function.block(block).instructions();
        let name = self.block_name(block);
        match list.last() {
            None => return self.fail(format!("{name} is empty")),
            Some(&last) if !self.function.instruction(last).opcode.is_terminator() => self.fail(format!("{name} does not end in a terminator")),
            _ => {}
        }
        let mut phis_over = false;
        for (at, &inst) in list.iter().enumerate() {
            let opcode = &self.function.instruction(inst).opcode;
            if opcode.is_terminator() && at + 1 != list.len() {
                self.fail(format!("{name} has a terminator before its end"));
            }
            match opcode {
                Opcode::Phi if phis_over => self.fail(format!("{name} has a phi after its first other instruction")),
                Opcode::Phi if entry => self.fail("the entry block has a phi".to_owned()),
                Opcode::Phi => {}
                Opcode::LandingPad { .. } if phis_over => self.fail(format!("{name}'s landingpad is not its first instruction after phis")),
                _ => phis_over = true,
            }
            if let Opcode::LandingPad { .. } = opcode {
                phis_over = true;
            }
        }
        self.phi_inputs(block);
    }

    /// Each phi has one input per edge into its block, and inputs from one
    /// block agree, as LLVM requires.
    fn phi_inputs(&mut self, block: BlockId) {
        let mut edges: HashMap<BlockId, usize> = HashMap::new();
        for one in self.function.block_users(block) {
            let user = self.function.instruction(one.user);
            if user.opcode.is_terminator()
                && let Some(parent) = self.function.parent(one.user)
            {
                *edges.entry(parent).or_default() += 1;
            }
        }
        for &inst in self.function.block(block).instructions() {
            let instruction = self.function.instruction(inst);
            if instruction.opcode != Opcode::Phi {
                continue;
            }
            let mut inputs: HashMap<BlockId, Vec<Operand>> = HashMap::new();
            for pair in instruction.operands.chunks(2) {
                if let [value, Operand::Block(from)] = pair {
                    inputs.entry(*from).or_default().push(*value);
                }
            }
            let name = self.block_name(block);
            for (from, values) in &inputs {
                if !edges.contains_key(from) {
                    self.fail(format!("a phi in {name} has an input from {}, which does not branch there", self.block_name(*from)));
                } else if values.len() != edges[from] {
                    self.fail(format!("a phi in {name} has {} inputs from {} for {} edges", values.len(), self.block_name(*from), edges[from]));
                } else if values.iter().any(|one| *one != values[0]) {
                    self.fail(format!("a phi in {name} has different inputs from {}", self.block_name(*from)));
                }
            }
            for from in edges.keys() {
                if !inputs.contains_key(from) {
                    self.fail(format!("a phi in {name} has no input from {}", self.block_name(*from)));
                }
            }
        }
    }

    fn dominance(&mut self, tree: &DominatorTree, inst: InstId) {
        let instruction = self.function.instruction(inst);
        let is_phi = instruction.opcode == Opcode::Phi;
        for (index, operand) in instruction.operands.iter().enumerate() {
            let Operand::Value(value) = *operand else { continue };
            let ValueDef::Instruction(def) = self.function.value(value).def else { continue };
            if self.function.is_erased(def) {
                self.fail(format!("{} uses the result of an erased instruction", self.at(inst)));
                continue;
            }
            let dominated = if is_phi {
                // Along its edge: the definition dominates the incoming block's end.
                let Some(Operand::Block(from)) = instruction.operands.get(index + 1) else { continue };
                self.function.parent(def).is_some_and(|block| tree.dominates(block, *from))
            } else {
                def != inst && tree.instruction_dominates(self.function, def, inst) || !self.function.parent(inst).is_some_and(|one| tree.is_reachable(one))
            };
            if !dominated {
                self.fail(format!("{} uses a value whose definition does not dominate it", self.at(inst)));
            }
        }
    }

    fn instruction(&mut self, inst: InstId, returns: TypeId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        let types: Vec<Option<TypeId>> = instruction.operands.iter().map(|one| self.operand_type(*one)).collect();
        let ty = |at: usize| types[at].expect("a value operand");
        let result = instruction.ty;
        let at = self.at(inst);
        let calls = matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_));
        for (index, operand) in instruction.operands.iter().enumerate() {
            if self.intrinsic(*operand).is_some() && !(calls && index + 1 == instruction.operands.len()) {
                self.fail(format!("{at}: Cannot take the address of an intrinsic!"));
            }
        }
        let is_int = |checker: &Self, ty: TypeId| matches!(checker.ty(ty), Type::Int(_)) || matches!(checker.ty(ty), Type::Vector { element, .. } if matches!(checker.ty(*element), Type::Int(_)));
        let is_float = |checker: &Self, ty: TypeId| matches!(checker.ty(ty), Type::Float(_)) || matches!(checker.ty(ty), Type::Vector { element, .. } if matches!(checker.ty(*element), Type::Float(_)));
        let is_pointer = |checker: &Self, ty: TypeId| matches!(checker.ty(ty), Type::Pointer(_));
        match &instruction.opcode {
            Opcode::Ret => match (instruction.operands.first(), self.context.types.is_void(returns)) {
                (None, true) => {}
                (Some(_), false) if ty(0) == returns => {}
                _ => self.fail(format!("{at} does not return the function's type, {}", self.show(returns))),
            },
            Opcode::Binary(op) => {
                if ty(0) != ty(1) || ty(0) != result {
                    self.fail(format!("{at} mixes types"));
                } else {
                    let floating = matches!(op, BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem);
                    if floating && !is_float(self, result) || !floating && !is_int(self, result) {
                        self.fail(format!("{at} on {}", self.show(result)));
                    }
                }
            }
            Opcode::FNeg if !is_float(self, ty(0)) => self.fail(format!("{at} on {}", self.show(ty(0)))),
            Opcode::ICmp(_) if ty(0) != ty(1) || !(is_int(self, ty(0)) || is_pointer(self, ty(0))) => self.fail(format!("{at} compares {} with {}", self.show(ty(0)), self.show(ty(1)))),
            Opcode::FCmp(_) if ty(0) != ty(1) || !is_float(self, ty(0)) => self.fail(format!("{at} compares {} with {}", self.show(ty(0)), self.show(ty(1)))),
            Opcode::Select if ty(1) != ty(2) || ty(1) != result || !matches!(self.ty(ty(0)), Type::Int(1)) => self.fail(format!("{at} is ill-typed")),
            Opcode::Cast(op) => {
                if let Err(why) = self.cast(*op, ty(0), result) {
                    self.fail(format!("{at}: {why}"));
                }
            }
            Opcode::Switch => {
                let mut seen = HashSet::new();
                for pair in instruction.operands[2..].chunks(2) {
                    if let Operand::Constant(case) = pair[0] {
                        if self.context.get(case).ty != ty(0) || !matches!(self.context.get(case).kind, ConstantKind::Int(_)) {
                            self.fail(format!("{at} has a case of another type"));
                        }
                        if !seen.insert(case) {
                            self.fail(format!("{at} has a duplicate case"));
                        }
                    }
                }
                if !is_int(self, ty(0)) {
                    self.fail(format!("{at} switches on {}", self.show(ty(0))));
                }
            }
            Opcode::Load { .. } => {
                if !is_pointer(self, ty(0)) {
                    self.fail(format!("{at} loads through {}", self.show(ty(0))));
                }
                if matches!(self.ty(result), Type::Void | Type::Label | Type::Function { .. } | Type::Metadata | Type::Token) {
                    self.fail(format!("{at} loads a {}", self.show(result)));
                }
            }
            Opcode::Store { .. } if !is_pointer(self, ty(1)) => self.fail(format!("{at} stores through {}", self.show(ty(1)))),
            Opcode::Alloca { address_space, .. } => {
                if let Some(count) = types.first().copied().flatten()
                    && !is_int(self, count)
                {
                    self.fail(format!("{at} counts in {}", self.show(count)));
                }
                if *self.ty(result) != Type::Pointer(*address_space) {
                    self.fail(format!("{at} is not a pointer to its address space"));
                }
            }
            Opcode::GetElementPtr { source } => self.gep(inst, *source, &at),
            Opcode::Call(info) | Opcode::Invoke(info) => {
                let invoke = matches!(instruction.opcode, Opcode::Invoke(_));
                let (call_returns, parameters, variadic) = self.module.signature(info.function_type);
                let parameters = parameters.to_vec();
                let arguments = instruction.operands.len() - if invoke { 3 } else { 1 };
                let callee = *instruction.operands.last().expect("a callee");
                if !self.operand_type(callee).is_some_and(|one| is_pointer(self, one)) {
                    self.fail(format!("{at} calls through a non-pointer"));
                }
                if arguments < parameters.len() || arguments > parameters.len() && !variadic {
                    self.fail(format!("{at} passes {arguments} arguments for {} parameters", parameters.len()));
                }
                for (index, parameter) in parameters.iter().enumerate().take(arguments) {
                    if ty(index) != *parameter {
                        self.fail(format!("{at} passes {} as parameter {index}, a {}", self.show(ty(index)), self.show(*parameter)));
                    }
                }
                if let Some(declared) = self.intrinsic(callee)
                    && declared != info.function_type
                {
                    self.fail(format!("{at}: Intrinsic called with incompatible signature"));
                }
                if call_returns != result {
                    self.fail(format!("{at} returns {}, not its type's {}", self.show(result), self.show(call_returns)));
                }
                if invoke {
                    self.invoke(inst, &at);
                }
            }
            Opcode::LandingPad { .. } if self.function.personality.is_none() => self.fail(format!("{at} in a function without a personality")),
            _ => {}
        }
    }

    fn cast(&self, op: CastOp, from: TypeId, to: TypeId) -> Result<(), String> {
        let (a, b) = (self.ty(from), self.ty(to));
        let ok = match (op, a, b) {
            (CastOp::Trunc, Type::Int(x), Type::Int(y)) => y < x,
            (CastOp::ZExt | CastOp::SExt, Type::Int(x), Type::Int(y)) => y > x,
            (CastOp::FPTrunc, Type::Float(x), Type::Float(y)) => (*x as u8) > (*y as u8),
            (CastOp::FPExt, Type::Float(x), Type::Float(y)) => (*x as u8) < (*y as u8),
            (CastOp::FPToUI | CastOp::FPToSI, Type::Float(_), Type::Int(_)) => true,
            (CastOp::UIToFP | CastOp::SIToFP, Type::Int(_), Type::Float(_)) => true,
            (CastOp::PtrToInt, Type::Pointer(_), Type::Int(_)) => true,
            (CastOp::IntToPtr, Type::Int(_), Type::Pointer(_)) => true,
            (CastOp::AddrSpaceCast, Type::Pointer(x), Type::Pointer(y)) => x != y,
            (CastOp::BitCast, Type::Pointer(x), Type::Pointer(y)) => x == y,
            (CastOp::BitCast, Type::Int(bits), Type::Float(kind)) | (CastOp::BitCast, Type::Float(kind), Type::Int(bits)) => {
                *bits == if *kind == crate::types::FloatKind::Float { 32 } else { 64 }
            }
            (CastOp::BitCast, x, y) => x == y && matches!(x, Type::Int(_) | Type::Float(_)),
            _ => false,
        };
        if ok { Ok(()) } else { Err(format!("cannot take {} to {}", self.show(from), self.show(to))) }
    }

    /// Struct indices are constant `i32`s, and each index steps into its type.
    fn gep(&mut self, inst: InstId, source: TypeId, at: &str) {
        let instruction = self.function.instruction(inst);
        let base = self.operand_type(instruction.operands[0]).expect("a value");
        let Type::Pointer(space) = *self.ty(base) else { return self.fail(format!("{at} steps from a non-pointer")) };
        if *self.ty(instruction.ty) != Type::Pointer(space) {
            self.fail(format!("{at} leaves its address space"));
        }
        let mut current = source;
        for (step, &index) in instruction.operands[1..].iter().enumerate() {
            let index_ty = self.operand_type(index).expect("a value");
            if !matches!(self.ty(index_ty), Type::Int(_)) {
                return self.fail(format!("{at} indexes with {}", self.show(index_ty)));
            }
            if step == 0 {
                continue;
            }
            current = match self.ty(current).clone() {
                Type::Array { element, .. } | Type::Vector { element, .. } => element,
                Type::Struct { .. } | Type::Named(_) => {
                    let constant = match index {
                        Operand::Constant(id) if *self.ty(index_ty) == Type::Int(32) => self.context.get(id).kind.clone(),
                        _ => return self.fail(format!("{at} indexes a struct with other than a constant i32")),
                    };
                    let ConstantKind::Int(field) = constant else { return self.fail(format!("{at} indexes a struct with a non-integer")) };
                    match self.context.types.member(current, field as u64) {
                        Some(member) => member,
                        None => return self.fail(format!("{at} indexes past its struct")),
                    }
                }
                _ => return self.fail(format!("{at} indexes into {}", self.show(current))),
            };
        }
    }

    /// The unwind destination begins with a landingpad, after its phis.
    fn invoke(&mut self, inst: InstId, at: &str) {
        let operands = &self.function.instruction(inst).operands;
        let Operand::Block(unwind) = operands[operands.len() - 2] else { return };
        let first = self.function.block(unwind).instructions().iter().find(|&&one| self.function.instruction(one).opcode != Opcode::Phi);
        if !first.is_some_and(|&one| matches!(self.function.instruction(one).opcode, Opcode::LandingPad { .. })) {
            self.fail(format!("{at} unwinds to {}, which does not begin with a landingpad", self.block_name(unwind)));
        }
        if self.function.personality.is_none() {
            self.fail(format!("{at} in a function without a personality"));
        }
    }
}
