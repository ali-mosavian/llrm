//! What a ValueTracking query is asked in: LLVM's `SimplifyQuery`. The module
//! (its globals name the intrinsics a call reaches), the function, the data
//! layout, and optionally the dominator tree, the assumptions and a context
//! instruction.
//!
//! # How it works
//! `Query` is `Copy` and read-only. A query without a dominator tree finds no
//! dominating condition, one without assumptions no `llvm.assume`: LLVM does
//! the same when `SQ.DT` or `SQ.AC` is null.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::dominators::DominatorTree;
use crate::intrinsics::Intrinsic;
use crate::module::{Function, GlobalKind, InstId, Instruction, Module, Operand, ValueDef};
use crate::opcode::Opcode;
use crate::types::{Type, TypeId};

/// `SimplifyQuery`.
#[derive(Clone, Copy)]
pub struct Query<'a> {
    pub module: &'a Module,
    pub function: &'a Function,
    pub layout: &'a DataLayout,
    pub dt: Option<&'a DominatorTree>,
    pub assumptions: Option<&'a AssumptionCache>,
    /// `CxtI`: the instruction the query is asked at.
    pub cxt: Option<InstId>,
}

impl<'a> Query<'a> {
    pub fn new(
        module: &'a Module,
        function: &'a Function,
        layout: &'a DataLayout,
    ) -> Self {
        Self { module, function, layout, dt: None, assumptions: None, cxt: None }
    }

    pub fn with_cxt(
        self,
        cxt: Option<InstId>,
    ) -> Self {
        Self { cxt, ..self }
    }

    pub fn context(&self) -> &'a Context {
        &self.module.context
    }

    pub fn type_of(
        &self,
        operand: Operand,
    ) -> Option<TypeId> {
        self.function.operand_type(self.context(), operand)
    }

    /// `getBitWidth(Ty, DL)`: an integer's width or a pointer's, 0 for anything
    /// else.
    pub fn scalar_bits(
        &self,
        operand: Operand,
    ) -> u32 {
        match self.type_of(operand).map(|ty| self.context().types.get(ty)) {
            Some(Type::Int(bits)) => *bits,
            Some(Type::Pointer(space)) => self.layout.pointer(*space).bits,
            _ => 0,
        }
    }

    pub fn is_pointer(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            self.type_of(operand).map(|ty| self.context().types.get(ty)),
            Some(Type::Pointer(_))
        )
    }

    /// The constant integer `operand` is, as bits and width.
    pub fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        let Operand::Constant(id) = operand else { return None };
        let constant = self.context().get(id);
        match constant.kind {
            ConstantKind::Int(bits) => Some((bits, self.context().types.int_bits(constant.ty)?)),
            ConstantKind::Zero | ConstantKind::Null => Some((0, self.context().types.int_bits(constant.ty)?)),
            _ => None,
        }
    }

    /// The instruction that defines `operand`, if one does.
    pub fn instruction_of(
        &self,
        operand: Operand,
    ) -> Option<(InstId, &'a Instruction)> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return None };
        Some((inst, self.function.instruction(inst)))
    }

    /// The global a call's callee names.
    pub fn callee_name(
        &self,
        inst: InstId,
    ) -> Option<&'a str> {
        let instruction = self.function.instruction(inst);
        let (Opcode::Call(_) | Opcode::Invoke(_)) = instruction.opcode else { return None };
        let Operand::Constant(id) = *instruction.operands.last()? else { return None };
        let ConstantKind::Global(global) = self.context().get(id).kind else { return None };
        match &self.module.global(global).kind {
            GlobalKind::Function(_) => self.module.global(global).name.as_deref(),
            GlobalKind::Variable(_) => None,
        }
    }

    /// The intrinsic a call reaches, when MIR has it.
    pub fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<Intrinsic> {
        self.callee_name(inst).and_then(Intrinsic::named)
    }

    /// Whether `a` is the same value as `b`.
    pub fn same(
        a: Operand,
        b: Operand,
    ) -> bool {
        a == b
    }
}

/// `AssumptionCache`: the `llvm.assume` calls of a function, and the values
/// each one tells of.
#[derive(Clone, Debug, Default)]
pub struct AssumptionCache {
    pub assumes: Vec<InstId>,
}

impl AssumptionCache {
    pub fn new(
        module: &Module,
        function: &Function,
    ) -> Self {
        let layout = DataLayout::default();
        let query = Query::new(module, function, &layout);
        let assumes = function
            .walk()
            .filter(|(_, inst)| query.intrinsic(*inst) == Some(Intrinsic::Assume))
            .map(|(_, inst)| inst)
            .collect();
        Self { assumes }
    }
}
