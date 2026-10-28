//! GVN's value table and leader map: a translation of `GVNPass::ValueTable`
//! and `GVNPass::LeaderMap` in
//! [`GVN.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp)
//! at d1106deb71cc, with the memory-dependence and MemorySSA parts that later
//! stages add marked where they go.
//!
//! A value number is a `u32` from 1. `Expression` is hashed on opcode, type and
//! operand numbers and equal on those and the call attributes, as LLVM's is:
//! LLVM's key has no IR flags: a merge intersects them
//! (`local::patch_replacement_instruction`). DEVIATE 16: here the flags of an
//! arithmetic or cast instruction are in the key, so instructions that differ
//! in `nsw` stay two; merging them dropped the `nsw` a later pass reads
//! (x_mergesort -O2 +5.9% memory operands, -Os +21%). The first expression
//! numbered has expression index 0, which `phi_translate_impl` reads as none,
//! as LLVM's does (`ExprIdx[Num] == 0`): one expression per table is not
//! translated through a phi.
//!
//! DEVIATE 11, MemorySSA: `NumberingBB`, `addMemoryStateToExp`,
//! `lookupOrAdd(MemoryAccess)` and the MemorySSA arms of `computeLoadStoreVN`
//! and `lookupOrAddCall` serve `enable-gvn-memoryssa`, which is off in the
//! pipeline being translated and is not translated.
//!
//! DEVIATE 12, no `ptrtoaddr`: MIR has no such cast, so `lookupPtrToInt` and
//! its caller in `processInstruction` are not written.
//!
//! DEVIATE 13, call attributes are equal, not intersectable: MIR has no
//! `AttributeList::intersectWith`; two calls are one expression only where
//! their attribute lists are equal.

use std::hash::{Hash, Hasher};

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::inline::InlineVec;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::HashMap;

/// `Expression::Opcode`: LLVM's opcode number, with a comparison's predicate
/// in it (`(Opcode << 8) | Predicate`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExprOp {
    FNeg,
    Binary(BinaryOp),
    Cast(CastOp),
    Select,
    Freeze,
    InsertValue,
    ExtractValue,
    GetElementPtr,
    ICmp(IntPredicate),
    FCmp(FloatPredicate),
    Call,
}

/// A call's attribute lists, which `Expression` compares (DEVIATE 13).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CallAttrs {
    function: Vec<Attribute>,
    returned: Vec<Attribute>,
    arguments: Vec<Vec<Attribute>>,
}

/// `GVNPass::Expression`
/// ([GVN.cpp:154](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L154)).
#[derive(Clone, Debug)]
pub struct Expression {
    pub opcode: ExprOp,
    pub commutative: bool,
    /// Not necessarily the result type: any type that tells expressions apart.
    pub ty: Option<TypeId>,
    pub var_args: InlineVec<u32, 4>,
    pub attrs: CallAttrs,
    /// DEVIATE 16: LLVM's key has no IR flags. Exit and owner:
    /// https://github.com/ali-mosavian/llrm/issues/2187
    pub flags: Flags,
}

impl Expression {
    fn new(opcode: ExprOp) -> Self {
        Self {
            opcode,
            commutative: false,
            ty: None,
            var_args: InlineVec::new(),
            attrs: CallAttrs::default(),
            flags: Flags::default(),
        }
    }
}

impl PartialEq for Expression {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.opcode == other.opcode
            && self.ty == other.ty
            && self.var_args == other.var_args
            && self.attrs == other.attrs
            && self.flags == other.flags
    }
}

impl Eq for Expression {}

impl Hash for Expression {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.opcode.hash(state);
        self.ty.hash(state);
        self.var_args.as_slice().hash(state);
        self.flags.hash(state);
    }
}

/// What the table reads of the function being numbered. The table is part of
/// the pass, which edits the function between calls, so it is not held.
pub struct Env<'a> {
    pub context: &'a mut Context,
    pub function: &'a Function,
    pub layout: &'a DataLayout,
    pub outer: &'a Outer,
}

impl Env<'_> {
    /// `AA->doesNotAccessMemory(C)`.
    fn does_not_access_memory(
        &self,
        call: InstId,
    ) -> bool {
        let effects = llrm_mir::memory::of(self.context, self.outer.callees(), self.function, call);
        !effects.reads && !effects.writes
    }

    /// The intrinsic `call` calls.
    fn intrinsic(
        &self,
        call: InstId,
    ) -> Option<Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context, self.function, call)?;
        self.outer.globals.get(callee.0 as usize)?.intrinsic()
    }

    /// `C->isConvergent()`.
    fn convergent(
        &self,
        call: InstId,
    ) -> bool {
        let Opcode::Call(info) = &self.function.instruction(call).opcode else { return false };
        info.attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == "convergent"))
    }
}

/// `Instruction::isCommutative`
/// ([Instruction.cpp:1411](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1411)):
/// the first two operands may be exchanged.
fn commutative(
    opcode: &Opcode,
    intrinsic: Option<Intrinsic>,
) -> bool {
    if let Some(intrinsic) = intrinsic {
        return match intrinsic {
            Intrinsic::WithOverflow { op, .. } => matches!(op, BinaryOp::Add | BinaryOp::Mul),
            Intrinsic::Saturating { op, .. } => matches!(op, BinaryOp::Add),
            Intrinsic::MinMax { .. } | Intrinsic::FMulAdd | Intrinsic::CopySign => true,
            Intrinsic::Fixed { divide } => !divide,
            _ => false,
        };
    }
    matches!(
        opcode,
        Opcode::Binary(
            BinaryOp::Add
                | BinaryOp::Mul
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Xor
                | BinaryOp::FAdd
                | BinaryOp::FMul
        )
    )
}

/// `ValueNumbering`, a `DenseMap<Value *, uint32_t>`: instruction and argument
/// values are dense ids, so they index a vector (0 is none); constants and
/// blocks are a map.
#[derive(Debug, Default)]
struct Numbering {
    values: Vec<u32>,
    /// Slots of `values` set since the last clear, so a clear is the size of
    /// the use.
    touched: Vec<u32>,
    others: HashMap<Operand, u32>,
}

impl Numbering {
    fn clear(&mut self) {
        for &slot in &self.touched {
            self.values[slot as usize] = 0;
        }
        self.touched.clear();
        self.others.clear();
    }

    fn get(
        &self,
        value: &Operand,
    ) -> Option<u32> {
        match value {
            Operand::Value(id) => self.values.get(id.0 as usize).copied().filter(|&one| one != 0),
            other => self.others.get(other).copied(),
        }
    }

    fn insert(
        &mut self,
        value: Operand,
        number: u32,
    ) {
        match value {
            Operand::Value(id) => {
                if self.values.len() <= id.0 as usize {
                    self.values.resize(id.0 as usize + 1 + self.values.len() / 2, 0);
                }
                if std::mem::replace(&mut self.values[id.0 as usize], number) == 0 {
                    self.touched.push(id.0);
                }
            }
            other => {
                self.others.insert(other, number);
            }
        }
    }

    fn remove(
        &mut self,
        value: &Operand,
    ) -> Option<u32> {
        match value {
            Operand::Value(id) => self.values.get_mut(id.0 as usize).and_then(|one| {
                let found = std::mem::take(one);
                (found != 0).then_some(found)
            }),
            other => self.others.remove(other),
        }
    }
}

thread_local! {
    static CLEARS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has cleared a value table, for a test that a
/// run clears it once per iteration over the function and no more.
pub fn clears() -> usize {
    CLEARS.with(std::cell::Cell::get)
}

/// `GVNPass::ValueTable`
/// ([GVN.h:156](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Scalar/GVN.h#L156)).
#[derive(Debug, Default)]
pub struct ValueTable {
    value_numbering: Numbering,
    expression_numbering: HashMap<Expression, u32>,
    next_expr_number: u32,
    /// Whether `expressions` are kept for `phi_translate`, which PRE is the
    /// only user of.
    pub translating: bool,
    expressions: Vec<Expression>,
    expr_idx: Vec<u32>,
    /// Value number to the phi that has it.
    numbering_phi: HashMap<u32, InstId>,
    phi_translate_table: HashMap<(u32, BlockId), u32>,
    next_value_number: u32,
}

impl ValueTable {
    pub fn new() -> Self {
        Self { next_value_number: 1, ..Self::default() }
    }

    pub fn next_unused_value_number(&self) -> u32 {
        self.next_value_number
    }

    fn fresh(&mut self) -> u32 {
        let number = self.next_value_number;
        self.next_value_number += 1;
        number
    }

    /// `createExpr`
    /// ([GVN.cpp:334](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L334)).
    fn create_expr(
        &mut self,
        env: &mut Env,
        inst: InstId,
        opcode: ExprOp,
    ) -> Expression {
        let instruction = env.function.instruction(inst);
        let mut e = Expression::new(opcode);
        e.ty = Some(instruction.ty);
        e.flags = instruction.flags;
        for &operand in &instruction.operands {
            e.var_args.push(self.lookup_or_add(env, operand));
        }
        let intrinsic = if matches!(instruction.opcode, Opcode::Call(_)) { env.intrinsic(inst) } else { None };
        if commutative(&instruction.opcode, intrinsic) {
            // Commutative operands are the first two.
            if e.var_args[0] > e.var_args[1] {
                e.var_args.swap(0, 1);
            }
            e.commutative = true;
        }
        match &instruction.opcode {
            Opcode::InsertValue(indices) => e.var_args.extend_from_slice(indices),
            Opcode::Call(info) => {
                e.attrs = CallAttrs {
                    function: info.attrs.clone(),
                    returned: info.return_attrs.clone(),
                    arguments: info.argument_attrs.clone(),
                };
            }
            _ => {}
        }
        e
    }

    /// `createCmpExpr`
    /// ([GVN.cpp:372](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L372)).
    fn create_cmp_expr(
        &mut self,
        env: &mut Env,
        opcode: ExprOp,
        lhs: Operand,
        rhs: Operand,
    ) -> Expression {
        let mut e = Expression::new(opcode);
        let operand_ty = env.function.operand_type(env.context, lhs).expect("a value");
        e.ty = Some(cmp_result_type(env.context, operand_ty));
        e.var_args.push(self.lookup_or_add(env, lhs));
        e.var_args.push(self.lookup_or_add(env, rhs));
        // Sort the operand value numbers so x<y and y>x get the same number.
        if e.var_args[0] > e.var_args[1] {
            e.var_args.swap(0, 1);
            e.opcode = swap_cmp(e.opcode);
        }
        e.commutative = true;
        e
    }

    /// `createExtractValueExpr`
    /// ([GVN.cpp:392](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L392)).
    fn create_extract_value_expr(
        &mut self,
        env: &mut Env,
        inst: InstId,
        indices: &[u32],
    ) -> Expression {
        let instruction = env.function.instruction(inst);
        let aggregate = instruction.operands[0];
        let ty = instruction.ty;
        // An extract of the result of a `with.overflow` intrinsic is the
        // operation itself.
        if let Operand::Value(value) = aggregate
            && let ValueDef::Instruction(call) = env.function.value(value).def
            && let Some(Intrinsic::WithOverflow { op, .. }) = env.intrinsic(call)
            && indices == [0]
        {
            let [lhs, rhs, ..] = env.function.instruction(call).operands[..] else { unreachable!("two operands") };
            let mut e = Expression::new(ExprOp::Binary(op));
            e.ty = Some(ty);
            e.var_args.push(self.lookup_or_add(env, lhs));
            e.var_args.push(self.lookup_or_add(env, rhs));
            return e;
        }
        // Not a recognised intrinsic: an extract value expression.
        let mut e = Expression::new(ExprOp::ExtractValue);
        e.ty = Some(ty);
        e.var_args.push(self.lookup_or_add(env, aggregate));
        e.var_args.extend_from_slice(indices);
        e
    }

    /// `createGEPExpr`
    /// ([GVN.cpp:420](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L420)).
    fn create_gep_expr(
        &mut self,
        env: &mut Env,
        inst: InstId,
        source: TypeId,
    ) -> Expression {
        let instruction = env.function.instruction(inst);
        let mut e = Expression::new(ExprOp::GetElementPtr);
        let space = match env.context.types.get(instruction.ty) {
            Type::Pointer(space) => *space,
            other => unreachable!("a GEP of {other:?}"),
        };
        let bit_width = env.layout.pointer(space).index_bits;
        let operands = instruction.operands.clone();
        // Convert into offset representation, to recognize equivalent address
        // calculations that use different type encoding.
        let (variable, constant) = collect_offset(env, source, &operands[1..], bit_width);
        e.ty = None;
        e.var_args.push(self.lookup_or_add(env, operands[0]));
        let index_ty = env.context.types.int(bit_width);
        for (value, scale) in variable {
            e.var_args.push(self.lookup_or_add(env, value));
            let scale = Operand::Constant(env.context.int(index_ty, scale as i128));
            e.var_args.push(self.lookup_or_add(env, scale));
        }
        if constant != 0 {
            let constant = Operand::Constant(env.context.int(index_ty, constant as i128));
            e.var_args.push(self.lookup_or_add(env, constant));
        }
        e
    }

    /// `add`
    /// ([GVN.cpp:464](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L464)).
    pub fn add(
        &mut self,
        value: Operand,
        number: u32,
        phi: bool,
        inst: Option<InstId>,
    ) {
        if self.value_numbering.get(&value).is_none() {
            self.value_numbering.insert(value, number);
        }
        if let (true, Some(inst)) = (phi, inst) {
            self.numbering_phi.insert(number, inst);
        }
    }

    /// `lookupOrAddCall`
    /// ([GVN.cpp:483](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L483)).
    /// Stage 4 adds the arms that ask memory dependence.
    fn lookup_or_add_call(
        &mut self,
        env: &mut Env,
        call: InstId,
    ) -> u32 {
        let value = Operand::Value(env.function.instruction(call).result.expect("a call with a value"));
        // Do not combine convergent calls since they implicitly depend on the
        // set of threads that is currently executing, and they might be in
        // different basic blocks.
        if env.convergent(call) {
            let number = self.fresh();
            self.value_numbering.insert(value, number);
            return number;
        }
        if env.does_not_access_memory(call) {
            let exp = self.create_expr(env, call, ExprOp::Call);
            let number = self.assign_exp_new_value_num(exp).0;
            self.value_numbering.insert(value, number);
            return number;
        }
        let number = self.fresh();
        self.value_numbering.insert(value, number);
        number
    }

    /// `exists`
    /// ([GVN.cpp:643](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L643)).
    pub fn exists(
        &self,
        value: Operand,
    ) -> bool {
        self.value_numbering.get(&value).is_some()
    }

    /// `lookupOrAdd`
    /// ([GVN.cpp:655](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L655)).
    pub fn lookup_or_add(
        &mut self,
        env: &mut Env,
        value: Operand,
    ) -> u32 {
        if let Some(number) = self.value_numbering.get(&value) {
            return number;
        }
        let inst = match value {
            Operand::Value(id) => match env.function.value(id).def {
                ValueDef::Instruction(inst) => inst,
                // An argument.
                ValueDef::Argument(_) => {
                    let number = self.fresh();
                    self.value_numbering.insert(value, number);
                    return number;
                }
            },
            // A constant or a block.
            _ => {
                let number = self.fresh();
                self.value_numbering.insert(value, number);
                return number;
            }
        };
        let function = env.function;
        let exp = match &function.instruction(inst).opcode {
            Opcode::Call(_) => return self.lookup_or_add_call(env, inst),
            Opcode::FNeg => self.create_expr(env, inst, ExprOp::FNeg),
            Opcode::Binary(op) => self.create_expr(env, inst, ExprOp::Binary(*op)),
            Opcode::Cast(op) => self.create_expr(env, inst, ExprOp::Cast(*op)),
            Opcode::Select => self.create_expr(env, inst, ExprOp::Select),
            Opcode::Freeze => self.create_expr(env, inst, ExprOp::Freeze),
            Opcode::InsertValue(_) => self.create_expr(env, inst, ExprOp::InsertValue),
            Opcode::ICmp(predicate) => {
                let [lhs, rhs] = env.function.instruction(inst).operands[..] else { unreachable!("two operands") };
                self.create_cmp_expr(env, ExprOp::ICmp(*predicate), lhs, rhs)
            }
            Opcode::FCmp(predicate) => {
                let [lhs, rhs] = env.function.instruction(inst).operands[..] else { unreachable!("two operands") };
                self.create_cmp_expr(env, ExprOp::FCmp(*predicate), lhs, rhs)
            }
            Opcode::GetElementPtr { source } => self.create_gep_expr(env, inst, *source),
            Opcode::ExtractValue(indices) => self.create_extract_value_expr(env, inst, indices),
            Opcode::Phi => {
                let number = self.fresh();
                self.value_numbering.insert(value, number);
                self.numbering_phi.insert(number, inst);
                return number;
            }
            // `computeLoadStoreVN` without MemorySSA (DEVIATE 11), and the
            // rest: a number of its own.
            _ => {
                let number = self.fresh();
                self.value_numbering.insert(value, number);
                return number;
            }
        };
        let number = self.assign_exp_new_value_num(exp).0;
        self.value_numbering.insert(value, number);
        number
    }

    /// `lookup`
    /// ([GVN.cpp:743](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L743)).
    /// 0 for a value not numbered, where `verify` is false.
    pub fn lookup(
        &self,
        value: Operand,
        verify: bool,
    ) -> u32 {
        let found = self.value_numbering.get(&value);
        if verify {
            return found.expect("Value not numbered?");
        }
        found.unwrap_or(0)
    }

    /// `lookupOrAddCmp`
    /// ([GVN.cpp:756](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L756)).
    pub fn lookup_or_add_cmp(
        &mut self,
        env: &mut Env,
        opcode: ExprOp,
        lhs: Operand,
        rhs: Operand,
    ) -> u32 {
        let exp = self.create_cmp_expr(env, opcode, lhs, rhs);
        self.assign_exp_new_value_num(exp).0
    }

    /// `clear`
    /// ([GVN.cpp:772](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L772)).
    pub fn clear(&mut self) {
        CLEARS.with(|count| count.set(count.get() + 1));
        self.value_numbering.clear();
        self.expression_numbering.clear();
        self.expressions.clear();
        self.expr_idx.clear();
        self.numbering_phi.clear();
        self.phi_translate_table.clear();
        self.next_expr_number = 0;
        self.next_value_number = 1;
    }

    /// `erase`
    /// ([GVN.cpp:785](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L785)).
    pub fn erase(
        &mut self,
        value: Operand,
        is_phi: bool,
    ) {
        let number = self.value_numbering.remove(&value);
        // If V is PHINode, V <--> value number is an one-to-one mapping.
        if let (true, Some(number)) = (is_phi, number) {
            self.numbering_phi.remove(&number);
        }
    }

    /// `verifyRemoved`
    /// ([GVN.cpp:797](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L797)).
    pub fn verify_removed(
        &self,
        value: Operand,
    ) {
        assert!(!self.value_numbering.get(&value).is_some(), "Inst still occurs in value numbering map!");
    }

    /// `assignExpNewValueNum`
    /// ([GVN.cpp:2908](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2908)):
    /// the number of `exp` and whether it is new.
    fn assign_exp_new_value_num(
        &mut self,
        exp: Expression,
    ) -> (u32, bool) {
        if let Some(&number) = self.expression_numbering.get(&exp) {
            return (number, false);
        }
        let number = self.next_value_number;
        self.next_value_number += 1;
        if self.translating {
            self.expression_numbering.insert(exp.clone(), number);
            self.expressions.push(exp);
            if self.expr_idx.len() < number as usize + 1 {
                self.expr_idx.resize(number as usize * 2 + 1, 0);
            }
            self.expr_idx[number as usize] = self.next_expr_number;
            self.next_expr_number += 1;
        } else {
            self.expression_numbering.insert(exp, number);
        }
        (number, true)
    }

    /// `areAllValsInBB`
    /// ([GVN.cpp:2923](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2923)).
    fn are_all_vals_in_bb(
        number: u32,
        block: BlockId,
        leaders: &LeaderMap,
    ) -> bool {
        leaders.get_leaders(number).all(|leader| leader.block == block)
    }

    /// `phiTranslate`
    /// ([GVN.cpp:2931](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2931)).
    pub fn phi_translate(
        &mut self,
        env: &mut Env,
        leaders: &LeaderMap,
        pred: BlockId,
        phi_block: BlockId,
        number: u32,
    ) -> u32 {
        if let Some(&found) = self.phi_translate_table.get(&(number, pred)) {
            return found;
        }
        let new_number = self.phi_translate_impl(env, leaders, pred, phi_block, number);
        self.phi_translate_table.insert((number, pred), new_number);
        new_number
    }

    /// `areCallValsEqual`
    /// ([GVN.cpp:2944](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2944)),
    /// with no memory dependence (stage 4): a call that reads memory is not
    /// known equal.
    fn are_call_vals_equal(
        env: &Env,
        leaders: &LeaderMap,
        number: u32,
        phi_block: BlockId,
    ) -> bool {
        let call = leaders
            .get_leaders(number)
            .find_map(
                |leader| {
                    let Operand::Value(value) = leader.value else { return None };
                    let ValueDef::Instruction(inst) = env.function.value(value).def else { return None };
                    (matches!(env.function.instruction(inst).opcode, Opcode::Call(_))
                        && env.function.parent(inst) == Some(phi_block))
                    .then_some(inst)
                },
            );
        call.is_some_and(|call| env.does_not_access_memory(call))
    }

    /// `phiTranslateImpl`
    /// ([GVN.cpp:2979](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2979)):
    /// `number` translated through the phis of `phi_block`, from `pred`.
    fn phi_translate_impl(
        &mut self,
        env: &mut Env,
        leaders: &LeaderMap,
        pred: BlockId,
        phi_block: BlockId,
        number: u32,
    ) -> u32 {
        // See if we can refine the value number by looking at the phi incoming
        // value for the given predecessor.
        if let Some(&phi) = self.numbering_phi.get(&number) {
            if env.function.parent(phi) != Some(phi_block) {
                return number;
            }
            let operands = env.function.instruction(phi).operands.clone();
            for pair in operands.chunks(2) {
                if pair[1] != Operand::Block(pred) {
                    continue;
                }
                let translated = self.lookup(pair[0], false);
                if translated != 0 {
                    return translated;
                }
            }
            return number;
        }

        // If there is any value related with Num is defined in a BB other than
        // PhiBlock, it cannot depend on a phi in PhiBlock without going through
        // a backedge. We can do an early exit in that case to save compile
        // time.
        if !Self::are_all_vals_in_bb(number, phi_block, leaders) {
            return number;
        }
        if number as usize >= self.expr_idx.len() || self.expr_idx[number as usize] == 0 {
            return number;
        }
        let mut exp = self.expressions[self.expr_idx[number as usize] as usize].clone();

        for at in 0..exp.var_args.len() {
            // For InsertValue and ExtractValue, some varargs are index numbers
            // instead of value numbers. Those index numbers should not be
            // translated.
            if (at > 1 && exp.opcode == ExprOp::InsertValue) || (at > 0 && exp.opcode == ExprOp::ExtractValue) {
                continue;
            }
            exp.var_args[at] = self.phi_translate(env, leaders, pred, phi_block, exp.var_args[at]);
        }

        if exp.commutative {
            assert!(exp.var_args.len() >= 2, "Unsupported commutative instruction!");
            if exp.var_args[0] > exp.var_args[1] {
                exp.var_args.swap(0, 1);
                exp.opcode = swap_cmp(exp.opcode);
            }
        }

        if let Some(&new_number) = self.expression_numbering.get(&exp) {
            if exp.opcode == ExprOp::Call && new_number != number {
                return if Self::are_call_vals_equal(env, leaders, number, phi_block) { new_number } else { number };
            }
            return new_number;
        }
        number
    }

    /// `eraseTranslateCacheEntry`
    /// ([GVN.cpp:3061](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3061)).
    pub fn erase_translate_cache_entry(
        &mut self,
        function: &Function,
        number: u32,
        block: BlockId,
    ) {
        for pred in function.predecessors(block) {
            self.phi_translate_table.remove(&(number, pred));
        }
    }
}

/// `CmpInst::makeCmpResultType`: `i1`.
fn cmp_result_type(
    context: &Context,
    operand: TypeId,
) -> TypeId {
    let bit = context.types.int(1);
    match context.types.get(operand).clone() {
        Type::Vector { count, .. } => context.types.intern(Type::Vector { element: bit, count }),
        _ => bit,
    }
}

/// The expression opcode of a comparison with its operands exchanged;
/// any other is itself.
fn swap_cmp(opcode: ExprOp) -> ExprOp {
    match opcode {
        ExprOp::ICmp(predicate) => ExprOp::ICmp(predicate.swapped()),
        ExprOp::FCmp(predicate) => ExprOp::FCmp(predicate.swapped()),
        other => other,
    }
}

/// `GEPOperator::collectOffset`
/// ([Operator.cpp:226](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Operator.cpp#L226)):
/// the variable indices with their summed scale, and the constant offset,
/// modulo `2^bit_width`. `indices` are the GEP's operands after the pointer.
fn collect_offset(
    env: &Env,
    source: TypeId,
    indices: &[Operand],
    bit_width: u32,
) -> (Vec<(Operand, u128)>, u128) {
    let mask = |value: u128| if bit_width >= 128 { value } else { value & ((1_u128 << bit_width) - 1) };
    let types = &env.context.types;
    let mut variable: Vec<(Operand, u128)> = Vec::new();
    let mut constant: u128 = 0;
    let mut current = source;
    for (at, &index) in indices.iter().enumerate() {
        // The type this index steps within: the source for the first index.
        let (stride, field_of) = if at == 0 {
            (env.layout.alloc_size(types, current), None)
        } else {
            match types.get(current) {
                Type::Array { element, .. } | Type::Vector { element, .. } => {
                    current = *element;
                    (env.layout.alloc_size(types, current), None)
                }
                _ => (0, Some(current)),
            }
        };
        let number = match index {
            Operand::Constant(id) => {
                let width = env.context.types.int_bits(env.context.get(id).ty).unwrap_or(64);
                match env.context.get(id).kind {
                    ConstantKind::Int(bits) => Some((bits, width)),
                    ConstantKind::Zero | ConstantKind::Null => Some((0, width)),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some((bits, width)) = number {
            if bits == 0 {
                if field_of.is_some() {
                    current = types.member(current, 0).expect("a field");
                }
                continue;
            }
            // Handle a struct index, which adds its field offset to the
            // pointer.
            if let Some(struct_ty) = field_of {
                let (_, offsets) = env.layout.struct_layout(types, struct_ty);
                constant = mask(constant.wrapping_add(u128::from(offsets[bits as usize])));
                current = types.member(struct_ty, bits as u64).expect("a field");
                continue;
            }
            let signed = llrm_mir::context::signed(bits, width) as u128;
            constant = mask(constant.wrapping_add(mask(signed).wrapping_mul(mask(u128::from(stride)))));
            continue;
        }
        assert!(field_of.is_none(), "a struct's index is a constant");
        let scale = mask(u128::from(stride));
        if scale != 0 {
            match variable.iter_mut().find(|(value, _)| *value == index) {
                Some((_, sum)) => *sum = mask(sum.wrapping_add(scale)),
                None => variable.push((index, scale)),
            }
        }
    }
    (variable, constant)
}

/// One entry of the leader table: a value and the block it is available from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaderTableEntry {
    pub value: Operand,
    pub block: BlockId,
}

/// `GVNPass::LeaderMap`
/// ([GVN.h:262](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Scalar/GVN.h#L262)):
/// per number a list whose head is first inserted; a new entry goes after the
/// head. LLVM's nodes are in a bump allocator and its heads in a `DenseMap`;
/// here the nodes are a `Vec` and the heads a `Vec` indexed by the (dense)
/// number.
#[derive(Debug, Default)]
pub struct LeaderMap {
    /// Node index + 1 of each number's head; 0 for none.
    heads: Vec<u32>,
    nodes: Vec<(LeaderTableEntry, u32)>,
    /// Numbers whose head is set, so a clear is the size of the use.
    used: Vec<u32>,
}

impl LeaderMap {
    /// The entries of `number`, head first.
    pub fn get_leaders(
        &self,
        number: u32,
    ) -> impl Iterator<Item = LeaderTableEntry> + '_ {
        let mut at = self.heads.get(number as usize).copied().unwrap_or(0);
        std::iter::from_fn(move || {
            if at == 0 {
                return None;
            }
            let (entry, next) = self.nodes[at as usize - 1];
            at = next;
            Some(entry)
        })
    }

    /// `insert`
    /// ([GVN.cpp:807](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L807)).
    pub fn insert(
        &mut self,
        number: u32,
        value: Operand,
        block: BlockId,
    ) {
        let entry = LeaderTableEntry { value, block };
        if self.heads.len() <= number as usize {
            self.heads.resize(number as usize * 2 + 1, 0);
        }
        let head = self.heads[number as usize];
        if head == 0 {
            self.nodes.push((entry, 0));
            self.heads[number as usize] = self.nodes.len() as u32;
            self.used.push(number);
        } else {
            // Key already exists: insert the new node after the head.
            let after = self.nodes[head as usize - 1].1;
            self.nodes.push((entry, after));
            self.nodes[head as usize - 1].1 = self.nodes.len() as u32;
        }
    }

    /// `erase`
    /// ([GVN.cpp:819](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L819)).
    pub fn erase(
        &mut self,
        number: u32,
        value: Operand,
        block: BlockId,
    ) {
        let head = self.heads.get(number as usize).copied().unwrap_or(0);
        let mut previous = 0;
        let mut current = head;
        while current != 0
            && !{
                let (entry, _) = self.nodes[current as usize - 1];
                entry.value == value && entry.block == block
            }
        {
            previous = current;
            current = self.nodes[current as usize - 1].1;
        }
        if current == 0 {
            return;
        }
        let next = self.nodes[current as usize - 1].1;
        if previous != 0 {
            // Non-head node: unlink.
            self.nodes[previous as usize - 1].1 = next;
        } else if next == 0 {
            // Only node: the number has no list.
            self.heads[number as usize] = 0;
        } else {
            // Move the second node's data into the head.
            self.nodes[current as usize - 1] = (self.nodes[next as usize - 1].0, self.nodes[next as usize - 1].1);
        }
    }

    /// Empties the map, keeping its storage.
    pub fn clear(&mut self) {
        for &number in &self.used {
            self.heads[number as usize] = 0;
        }
        self.used.clear();
        self.nodes.clear();
    }
}
