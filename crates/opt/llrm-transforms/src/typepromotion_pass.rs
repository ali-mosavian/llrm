//! Opcode-based type promotion of small integers the legalizer would promote
//! anyway, where promoting them in the IR spares the masks the DAG puts
//! around a value in a cycle. Translated from
//! [TypePromotion.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TypePromotion.cpp)
//! (`TypePromotionImpl`, `IRPromoter`).
//!
//! # How it works
//! The search starts at the operands of an unsigned `icmp` and of a scalar
//! `trunc` to `i1`, and at a loop's phi that a `zext` reads. From that value a
//! tree of instructions is grown: *sources* make narrow values (arguments,
//! loads, `zeroext` calls, an equal-width `trunc`), *sinks* observe them
//! (stores, returns, calls, compares, a `switch`, a widening `zext`). The tree
//! is promoted only if every instruction in it cannot wrap (`nuw`, or a
//! constant add or sub whose compare cannot tell: `isSafeWrap`). Sources are
//! zero extended once, the tree changes type in place, and each sink gets a
//! `trunc` back where it needs the original type.
//!
//! # Example
//! ```text
//!  before                            after
//!  %c = phi i16 [0, %a], [%n, %b]    %c = phi i32 [0, %a], [%n, %b]
//!  %n = add nuw nsw i16 %c, 1        %n = add nuw nsw i32 %c, 1
//!  %k = icmp ult i16 %c, 7           %k = icmp ult i32 %c, 7
//! ```
//!
//! # Runs
//! Before `cgpaddr`, as the pin's `addCodeGenPrepare` runs it, on a target
//! whose `Lowering::runs_type_promotion` says so (arm64).
//!
//! DEVIATE TP1 (no number): `Call::hasRetAttr(ZExt)` also reads the callee's
//! declaration; here only the call site's attributes are read (a pass has the
//! function, not the module). clang states `zeroext` at both. Exit: none.
//!
//! DEVIATE TP2 (no number): `getTypeToTransformTo` is one step to the next
//! power of two; here the next legal width above the type. The two agree for
//! every integer below the widest legal one on a target whose legal widths are
//! powers of two (arm64: 32 and 64). Exit: none.
//!
//! DEVIATE TP3 (no number): `IRBuilder` folds the `and` of a constant in
//! `ConvertTruncs`; here only an integer constant is folded, anything else
//! gets the instruction. Exit: none.

use std::collections::{HashMap, HashSet};

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::tti::Tti;
use llrm_mir::types::{FloatKind, Type, TypeId};

/// The widths an integer may be promoted to, narrowest first.
const WIDTHS: [u32; 5] = [8, 16, 32, 64, 128];

pub struct TypePromotion;

impl FunctionPass for TypePromotion {
    fn name(&self) -> &'static str {
        "typepromotion"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let (lowering, tti) = {
            let target = analyses.outer().target();
            (target.lowering(), target.tti())
        };
        let (Some(lowering), Some(tti)) = (lowering, tti) else { return PreservedAnalyses::all() };
        if !lowering.runs_type_promotion() {
            return PreservedAnalyses::all();
        }
        let in_loop = {
            let info = analyses.get::<Loops>(&*unit.context, unit.layout, &*unit.function);
            let mut in_loop = vec![false; unit.function.block_capacity()];
            for &block in unit.function.layout() {
                in_loop[block.0 as usize] = info.get_loop_for(block).is_some();
            }
            in_loop
        };
        let mut pass = Promotion::new(&*unit.context, &*lowering, &*tti, unit.function, in_loop);
        if pass.run() { PreservedAnalyses::none().preserve_cfg() } else { PreservedAnalyses::all() }
    }
}

/// A `Value *` of the pin: an instruction (a store is one), an argument, a
/// constant or a block.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Node {
    Inst(InstId),
    Arg(ValueId),
    Const(llrm_mir::context::ConstantId),
    Block(BlockId),
}

/// `SetVector`: insertion order, no repeats.
struct Ordered<T: Copy + Eq + std::hash::Hash> {
    items: Vec<T>,
    seen: HashSet<T>,
}

impl<T: Copy + Eq + std::hash::Hash> Ordered<T> {
    fn new() -> Self {
        Self { items: Vec::new(), seen: HashSet::new() }
    }

    fn insert(
        &mut self,
        item: T,
    ) -> bool {
        let fresh = self.seen.insert(item);
        if fresh {
            self.items.push(item);
        }
        fresh
    }

    fn contains(
        &self,
        item: &T,
    ) -> bool {
        self.seen.contains(item)
    }

    fn pop_back(&mut self) -> Option<T> {
        let item = self.items.pop()?;
        self.seen.remove(&item);
        Some(item)
    }

    fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.items.iter().copied()
    }
}

/// `TypePromotionImpl` with the `IRPromoter`'s state.
struct Promotion<'a> {
    context: &'a Context,
    lowering: &'a dyn Lowering,
    tti: &'a dyn Tti,
    function: &'a mut Function,
    in_loop: Vec<bool>,
    type_size: u32,
    register_bits: u32,
    all_visited: HashSet<Node>,
    safe_to_promote: HashSet<InstId>,
    safe_wrap: HashSet<InstId>,
    insts_to_remove: Ordered<InstId>,
}

/// `GenerateSignBits`.
fn generate_sign_bits(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::AShr | BinaryOp::SDiv | BinaryOp::SRem) | Opcode::Cast(CastOp::SExt)
    )
}

fn is_signed(predicate: IntPredicate) -> bool {
    matches!(
        predicate,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

/// `isIdenticalTo`.
fn identical(
    function: &Function,
    one: InstId,
    other: InstId,
) -> bool {
    let (one, other) = (function.instruction(one), function.instruction(other));
    one.opcode == other.opcode && one.ty == other.ty && one.operands == other.operands && one.flags == other.flags
}

impl<'a> Promotion<'a> {
    fn new(
        context: &'a Context,
        lowering: &'a dyn Lowering,
        tti: &'a dyn Tti,
        function: &'a mut Function,
        in_loop: Vec<bool>,
    ) -> Self {
        Self {
            context,
            lowering,
            tti,
            function,
            in_loop,
            type_size: 0,
            register_bits: tti.scalar_register_bits(),
            all_visited: HashSet::new(),
            safe_to_promote: HashSet::new(),
            safe_wrap: HashSet::new(),
            insts_to_remove: Ordered::new(),
        }
    }

    // ---- the value model -------------------------------------------------

    fn node(
        &self,
        operand: Operand,
    ) -> Node {
        match operand {
            Operand::Value(value) => match self.function.value(value).def {
                ValueDef::Instruction(inst) => Node::Inst(inst),
                ValueDef::Argument(_) => Node::Arg(value),
            },
            Operand::Constant(constant) => Node::Const(constant),
            Operand::Block(block) => Node::Block(block),
        }
    }

    fn operand(
        &self,
        node: Node,
    ) -> Operand {
        match node {
            Node::Inst(inst) => Operand::Value(self.function.instruction(inst).result.expect("a value")),
            Node::Arg(value) => Operand::Value(value),
            Node::Const(constant) => Operand::Constant(constant),
            Node::Block(block) => Operand::Block(block),
        }
    }

    fn ty(
        &self,
        node: Node,
    ) -> Option<TypeId> {
        match node {
            Node::Inst(inst) => Some(self.function.instruction(inst).ty),
            Node::Arg(value) => Some(self.function.value(value).ty),
            Node::Const(constant) => Some(self.context.get(constant).ty),
            Node::Block(_) => None,
        }
    }

    fn opcode(
        &self,
        inst: InstId,
    ) -> &Opcode {
        &self.function.instruction(inst).opcode
    }

    fn operands(
        &self,
        inst: InstId,
    ) -> Vec<Operand> {
        self.function.instruction(inst).operands.clone()
    }

    fn is_integer(
        &self,
        node: Node,
    ) -> bool {
        self.ty(node).is_some_and(|ty| self.context.types.int_bits(ty).is_some())
    }

    /// `getScalarSizeInBits`: 0 for what has no size here (a pointer, void).
    fn scalar_bits(
        &self,
        ty: TypeId,
    ) -> u32 {
        match self.context.types.get(ty) {
            Type::Int(bits) => *bits,
            Type::Float(FloatKind::Float) => 32,
            Type::Float(FloatKind::Double) => 64,
            Type::Float(FloatKind::X86Fp80) => 80,
            Type::Vector { element, .. } => self.scalar_bits(*element),
            _ => 0,
        }
    }

    fn bits_of(
        &self,
        node: Node,
    ) -> u32 {
        self.ty(node).map_or(0, |ty| self.scalar_bits(ty))
    }

    fn equal_type_size(
        &self,
        node: Node,
    ) -> bool {
        self.bits_of(node) == self.type_size
    }

    fn less_or_equal_type_size(
        &self,
        node: Node,
    ) -> bool {
        self.bits_of(node) <= self.type_size
    }

    fn greater_than_type_size(
        &self,
        node: Node,
    ) -> bool {
        self.bits_of(node) > self.type_size
    }

    fn less_than_type_size(
        &self,
        node: Node,
    ) -> bool {
        self.bits_of(node) < self.type_size
    }

    fn is_pointer(
        &self,
        node: Node,
    ) -> bool {
        self.ty(node).is_some_and(|ty| matches!(self.context.types.get(ty), Type::Pointer(_)))
    }

    fn is_void(
        &self,
        node: Node,
    ) -> bool {
        self.ty(node).is_some_and(|ty| self.context.types.is_void(ty))
    }

    /// `isTruncToI1`.
    fn is_trunc_to_i1(
        &self,
        node: Node,
    ) -> bool {
        match node {
            Node::Inst(inst) => {
                *self.opcode(inst) == Opcode::Cast(CastOp::Trunc)
                    && self.context.types.int_bits(self.function.instruction(inst).ty) == Some(1)
            }
            _ => false,
        }
    }

    /// `isSupportedTruncToI1`.
    fn is_supported_trunc_to_i1(
        &self,
        node: Node,
    ) -> bool {
        let Node::Inst(inst) = node else { return false };
        self.is_trunc_to_i1(node) && self.equal_type_size(self.node(self.function.instruction(inst).operands[0]))
    }

    /// Whether the value has a return attribute `zeroext` (`Call::hasRetAttr`;
    /// DEVIATE TP1).
    fn returns_zeroext(
        &self,
        inst: InstId,
    ) -> bool {
        match self.opcode(inst) {
            Opcode::Call(info) => llrm_mir::memory::has(&info.return_attrs, "zeroext"),
            _ => false,
        }
    }

    // ---- the predicates --------------------------------------------------

    /// `isSource`.
    fn is_source(
        &self,
        node: Node,
    ) -> bool {
        if !self.is_integer(node) {
            return false;
        }
        match node {
            Node::Arg(_) => true,
            Node::Inst(inst) => match self.opcode(inst) {
                Opcode::Load { .. } => true,
                Opcode::Call(_) => self.returns_zeroext(inst),
                Opcode::Cast(CastOp::Trunc) => self.equal_type_size(node),
                _ => false,
            },
            _ => false,
        }
    }

    /// `isSink`.
    fn is_sink(
        &self,
        node: Node,
    ) -> bool {
        let Node::Inst(inst) = node else { return false };
        let operands = &self.function.instruction(inst).operands;
        match self.opcode(inst) {
            Opcode::Store { .. } => self.less_or_equal_type_size(self.node(operands[0])),
            Opcode::Ret => operands.first().is_some_and(|one| self.less_or_equal_type_size(self.node(*one))),
            Opcode::Cast(CastOp::ZExt) => self.greater_than_type_size(node),
            Opcode::Switch => self.less_than_type_size(self.node(operands[0])),
            Opcode::ICmp(predicate) => is_signed(*predicate) || self.less_than_type_size(self.node(operands[0])),
            Opcode::Call(_) => true,
            _ => false,
        }
    }

    /// `shouldPromote`.
    fn should_promote(
        &self,
        node: Node,
    ) -> bool {
        if !self.is_integer(node) || self.is_sink(node) {
            return false;
        }
        if self.is_source(node) {
            return true;
        }
        let Node::Inst(inst) = node else { return false };
        if matches!(self.opcode(inst), Opcode::ICmp(_)) || self.is_supported_trunc_to_i1(node) {
            return false;
        }
        true
    }

    /// `isSafeWrap`: an add or sub of a constant read only by an unsigned
    /// compare against a constant, which cannot tell the wrap.
    fn is_safe_wrap(
        &mut self,
        inst: InstId,
    ) -> bool {
        let opcode = self.opcode(inst).clone();
        let is_sub = match opcode {
            Opcode::Binary(BinaryOp::Add) => false,
            Opcode::Binary(BinaryOp::Sub) => true,
            _ => return false,
        };
        let instruction = self.function.instruction(inst);
        let Some(result) = instruction.result else { return false };
        let mut users = self.function.users(result).iter();
        let (Some(only), None) = (users.next(), users.next()) else { return false };
        let compare = only.user;
        let Opcode::ICmp(predicate) = *self.opcode(compare) else { return false };
        let Some(constant) = self.int_constant(instruction.operands[1]) else { return false };
        if is_signed(predicate) || matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
            return false;
        }
        let compare_operands = self.operands(compare);
        let Some(compare_constant) =
            self.int_constant(compare_operands[0]).or_else(|| self.int_constant(compare_operands[1]))
        else {
            return false;
        };
        let bits = self.bits_of(Node::Inst(inst));
        let mask = llrm_mir::context::mask(bits);
        let negate = |value: u128| value.wrapping_neg() & mask;
        let overflow = if is_sub { negate(constant) } else { constant };
        // `isNonPositive`: as an `iN` signed number.
        let non_positive = overflow == 0 || (overflow >> (bits - 1)) & 1 == 1;
        if !non_positive {
            // The promoted width is not known here; 64 gives an `i64`.
            if bits >= 64 {
                return false;
            }
            let wide = negate(overflow);
            let new_constant = (wide as i64).wrapping_neg();
            if !self.tti.is_legal_add_immediate(new_constant) {
                return false;
            }
        }
        self.safe_wrap.insert(inst);
        if overflow == 0 || overflow > compare_constant {
            return true;
        }
        self.safe_wrap.insert(compare);
        true
    }

    fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<u128> {
        match operand {
            Operand::Constant(constant) => match self.context.get(constant).kind {
                ConstantKind::Int(bits) => Some(bits),
                _ => None,
            },
            _ => None,
        }
    }

    /// `isPromotedResultSafe`.
    fn is_promoted_result_safe(
        &self,
        inst: InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        if generate_sign_bits(&instruction.opcode) {
            return false;
        }
        if !matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)
        )
        {
            return true;
        }
        instruction.flags.contains(Flags::NUW)
    }

    /// `isLegalToPromote`.
    fn is_legal_to_promote(
        &mut self,
        node: Node,
    ) -> bool {
        let Node::Inst(inst) = node else { return true };
        if self.safe_to_promote.contains(&inst) {
            return true;
        }
        if self.is_promoted_result_safe(inst) || self.is_safe_wrap(inst) {
            self.safe_to_promote.insert(inst);
            return true;
        }
        false
    }

    /// `isSupportedType`.
    fn is_supported_type(
        &self,
        node: Node,
    ) -> bool {
        if self.is_void(node) || self.is_pointer(node) {
            return true;
        }
        let Some(bits) = self.ty(node).and_then(|ty| self.context.types.int_bits(ty)) else { return false };
        if bits == 1 || bits > self.register_bits {
            return false;
        }
        self.less_or_equal_type_size(node)
    }

    /// `isSupportedValue`.
    fn is_supported_value(
        &self,
        node: Node,
    ) -> bool {
        match node {
            Node::Inst(inst) => {
                let instruction = self.function.instruction(inst);
                match &instruction.opcode {
                    Opcode::Binary(_) => self.is_supported_type(node) && !generate_sign_bits(&instruction.opcode),
                    Opcode::GetElementPtr { .. } | Opcode::Store { .. } | Opcode::Br | Opcode::Switch => true,
                    Opcode::Phi | Opcode::Select | Opcode::Ret | Opcode::Load { .. } => self.is_supported_type(node),
                    Opcode::Cast(CastOp::Trunc) => self.is_supported_trunc_to_i1(node) || self.is_supported_type(node),
                    Opcode::Cast(CastOp::BitCast) => {
                        self.function.operand_type(self.context, instruction.operands[0]) == Some(instruction.ty)
                    }
                    Opcode::Cast(CastOp::ZExt) => self.is_supported_type(self.node(instruction.operands[0])),
                    // Only an icmp of `TypeSize` is supported: a smaller one
                    // would need a trunc to be legalised.
                    Opcode::ICmp(_) => {
                        let first = self.node(instruction.operands[0]);
                        self.is_pointer(first) || self.equal_type_size(first)
                    }
                    Opcode::Call(_) => self.is_supported_type(node) && self.returns_zeroext(inst),
                    _ => false,
                }
            }
            Node::Const(constant) => {
                !matches!(self.context.get(constant).kind, ConstantKind::Expr(_)) && self.is_supported_type(node)
            }
            Node::Arg(_) => self.is_supported_type(node),
            Node::Block(_) => true,
        }
    }

    // ---- the promoter ----------------------------------------------------

    /// `ReplaceAllUsersOfWith`.
    fn replace_all_users_of_with(
        &mut self,
        from: Node,
        to: Operand,
        removed: &mut Vec<InstId>,
    ) {
        let value = match from {
            Node::Inst(inst) => self.function.instruction(inst).result,
            Node::Arg(value) => Some(value),
            _ => None,
        };
        let Some(value) = value else { return };
        let to_inst = match to {
            Operand::Value(one) => match self.function.value(one).def {
                ValueDef::Instruction(inst) => Some(inst),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        };
        let uses: Vec<(InstId, usize)> =
            self.function.users(value).iter().map(|one| (one.user, one.index as usize)).collect();
        let mut replaced_all = true;
        for (user, index) in uses {
            if to_inst.is_some_and(|made| identical(self.function, user, made)) {
                replaced_all = false;
                continue;
            }
            self.function.set_operand(user, index, to);
        }
        if replaced_all && let Node::Inst(inst) = from {
            removed.push(inst);
        }
    }

    /// `Mutate`: promote the tree whose parts are given, to `width` bits.
    fn mutate(
        &mut self,
        width: u32,
        visited: &Ordered<Node>,
        sources: &Ordered<Node>,
        sinks: &Ordered<InstId>,
    ) {
        let ext_ty = self.context.types.int(width);
        let mut new_insts: HashSet<Node> = HashSet::new();
        let mut promoted: HashSet<Node> = HashSet::new();
        let mut trunc_tys: HashMap<InstId, Vec<Option<TypeId>>> = HashMap::new();
        let mut removed: Vec<InstId> = Vec::new();

        // Cache the original types of the values that will likely need
        // truncating.
        for sink in sinks.iter() {
            let operands = self.operands(sink);
            let types: Vec<Option<TypeId>> = match self.opcode(sink) {
                Opcode::Call(_) => operands[..operands.len() - 1].iter().map(|&one| self.ty(self.node(one))).collect(),
                Opcode::Switch => vec![self.ty(self.node(operands[0]))],
                _ => operands.iter().map(|&one| self.ty(self.node(one))).collect(),
            };
            trunc_tys.insert(sink, types);
        }
        for node in visited.iter() {
            let Node::Inst(inst) = node else { continue };
            if *self.opcode(inst) != Opcode::Cast(CastOp::Trunc) || self.is_trunc_to_i1(node) || sources.contains(&node)
            {
                continue;
            }
            trunc_tys.insert(inst, vec![Some(self.function.instruction(inst).ty)]);
        }

        // ExtendSources: a zext between each source and its users.
        for source in sources.iter() {
            let (position, old) = match source {
                Node::Inst(inst) => (Position::After(inst), source),
                Node::Arg(_) => {
                    let entry = self.function.entry().expect("a function with an argument has a body");
                    let first = self.function.iter_block(entry).find(|&one| *self.opcode(one) != Opcode::Phi);
                    (first.map_or(Position::End(entry), Position::Before), source)
                }
                _ => unreachable!("a source that needs extending is an instruction or an argument"),
            };
            let zext = self
                .function
                .create_instruction(
                    Opcode::Cast(CastOp::ZExt),
                    ext_ty,
                    vec![self.operand(old)],
                    Flags::default(),
                    None,
                );
            self.function.insert(zext, position).expect("a position for the extension");
            new_insts.insert(Node::Inst(zext));
            let to = self.operand(Node::Inst(zext));
            self.replace_all_users_of_with(old, to, &mut removed);
            promoted.insert(source);
        }

        // PromoteTree: mutate the types of the instructions within the tree.
        for node in visited.iter() {
            if sources.contains(&node) {
                continue;
            }
            let Node::Inst(inst) = node else { continue };
            if sinks.contains(&inst) {
                continue;
            }
            let is_select = *self.opcode(inst) == Opcode::Select;
            for index in 0..self.function.instruction(inst).operands.len() {
                let operand = self.function.instruction(inst).operands[index];
                let Some(ty) = self.function.operand_type(self.context, operand) else { continue };
                if ty == ext_ty || self.context.types.int_bits(ty).is_none() {
                    continue;
                }
                // Skip the condition operand of select.
                if is_select && index == 0 {
                    continue;
                }
                let Operand::Constant(constant) = operand else { continue };
                match self.context.get(constant).kind {
                    ConstantKind::Int(value) => {
                        let bits = self.context.types.int_bits(ty).expect("an integer constant");
                        let mask = llrm_mir::context::mask(bits);
                        let negate = |one: u128| one.wrapping_neg() & mask;
                        let opcode = self.opcode(inst);
                        let wrapping = self.safe_wrap.contains(&inst)
                            && (matches!(opcode, Opcode::ICmp(_))
                                || (*opcode == Opcode::Binary(BinaryOp::Add) && index == 1));
                        // -((-C).zext(W)): the distance from the top of the
                        // old range, kept from the top of the new one.
                        let new = if wrapping { (negate(value) as i128).wrapping_neg() } else { value as i128 };
                        let made = self.context.int(ext_ty, new);
                        self.function.set_operand(inst, index, Operand::Constant(made));
                    }
                    ConstantKind::Undef | ConstantKind::Poison => {
                        let made = self.context.int(ext_ty, 0);
                        self.function.set_operand(inst, index, Operand::Constant(made));
                    }
                    _ => {}
                }
            }
            // For switch, also the case values, which are not value operands.
            if *self.opcode(inst) == Opcode::Switch {
                let cases = self.function.instruction(inst).operands.len();
                for at in (2..cases).step_by(2) {
                    let Operand::Constant(case) = self.function.instruction(inst).operands[at] else { continue };
                    if let ConstantKind::Int(value) = self.context.get(case).kind {
                        let made = self.context.int(ext_ty, value as i128);
                        self.function.set_operand(inst, at, Operand::Constant(made));
                    }
                }
            }
            // A trunc to i1 keeps its type while its operand is zero
            // extended. Drop nsw if nuw is not also set: an all-ones operand
            // may be zero extended.
            if self.is_trunc_to_i1(node) {
                let mut flags = self.function.instruction(inst).flags;
                if !flags.contains(Flags::NUW) {
                    flags.remove(Flags::NSW);
                    self.function.set_flags(inst, flags);
                }
                continue;
            }
            // Mutate the result type, unless this is an icmp or switch.
            if !matches!(self.opcode(inst), Opcode::ICmp(_) | Opcode::Switch) {
                self.function.set_type(inst, ext_ty);
                promoted.insert(node);
            }
        }

        // ConvertTruncs: any trunc that is not a source is an AND mask.
        for node in visited.iter() {
            let Node::Inst(trunc) = node else { continue };
            if *self.opcode(trunc) != Opcode::Cast(CastOp::Trunc)
                || self.is_trunc_to_i1(node)
                || sources.contains(&node)
            {
                continue;
            }
            let source = self.function.instruction(trunc).operands[0];
            let src_ty = self.function.operand_type(self.context, source).expect("a trunc of a value");
            let dest_ty = trunc_tys[&trunc][0].expect("a trunc type");
            let dest_bits = self.context.types.int_bits(dest_ty).expect("an integer trunc");
            let max = llrm_mir::context::mask(dest_bits);
            let masked: Operand = match source {
                Operand::Constant(constant) if matches!(self.context.get(constant).kind, ConstantKind::Int(_)) => {
                    let ConstantKind::Int(value) = self.context.get(constant).kind else { unreachable!() };
                    Operand::Constant(self.context.int(src_ty, (value & max) as i128))
                }
                _ => {
                    let mask = Operand::Constant(self.context.int(src_ty, max as i128));
                    let and = self
                        .function
                        .create_instruction(
                            Opcode::Binary(BinaryOp::And),
                            src_ty,
                            vec![source, mask],
                            Flags::default(),
                            None,
                        );
                    self.function.insert(and, Position::Before(trunc)).expect("a position before the trunc");
                    new_insts.insert(Node::Inst(and));
                    let mut masked = Operand::Value(self.function.instruction(and).result.expect("a value"));
                    if self.context.types.int_bits(src_ty).expect("an integer") > width {
                        let narrow = self
                            .function
                            .create_instruction(
                                Opcode::Cast(CastOp::Trunc),
                                ext_ty,
                                vec![masked],
                                Flags::default(),
                                None,
                            );
                        self.function.insert(narrow, Position::Before(trunc)).expect("a position before the trunc");
                        new_insts.insert(Node::Inst(narrow));
                        masked = Operand::Value(self.function.instruction(narrow).result.expect("a value"));
                    }
                    masked
                }
            };
            self.replace_all_users_of_with(node, masked, &mut removed);
        }

        // TruncateSinks: a trunc where a sink needs the original type.
        let mut insert_trunc = |this: &mut Self, operand: Operand, ty: TypeId, before: InstId| -> Option<Operand> {
            let Operand::Value(value) = operand else { return None };
            let ValueDef::Instruction(def) = this.function.value(value).def else { return None };
            let node = Node::Inst(def);
            if this.context.types.int_bits(this.function.value(value).ty).is_none() {
                return None;
            }
            if (!promoted.contains(&node) && !new_insts.contains(&node)) || sources.contains(&node) {
                return None;
            }
            if this.function.value(value).ty == ty {
                return None;
            }
            let trunc = this
                .function
                .create_instruction(Opcode::Cast(CastOp::Trunc), ty, vec![operand], Flags::default(), None);
            this.function.insert(trunc, Position::Before(before)).expect("a position before the sink");
            new_insts.insert(Node::Inst(trunc));
            Some(Operand::Value(this.function.instruction(trunc).result.expect("a value")))
        };
        for sink in sinks.iter() {
            let operands = self.operands(sink);
            // Calls: the arguments.
            if matches!(self.opcode(sink), Opcode::Call(_)) {
                for index in 0..operands.len() - 1 {
                    let Some(ty) = trunc_tys[&sink][index] else { continue };
                    if let Some(trunc) = insert_trunc(self, operands[index], ty, sink) {
                        self.function.set_operand(sink, index, trunc);
                    }
                }
                continue;
            }
            // Switches: the condition.
            if *self.opcode(sink) == Opcode::Switch {
                if let Some(ty) = trunc_tys[&sink][0]
                    && let Some(trunc) = insert_trunc(self, operands[0], ty, sink)
                {
                    self.function.set_operand(sink, 0, trunc);
                }
                continue;
            }
            // A zext which can still legally promote needs no trunc: its
            // input is promoted to its own width and it goes in Cleanup.
            if *self.opcode(sink) == Opcode::Cast(CastOp::ZExt) && self.bits_of(Node::Inst(sink)) >= width {
                continue;
            }
            for (index, &operand) in operands.iter().enumerate() {
                let Some(ty) = trunc_tys[&sink][index] else { continue };
                if let Some(trunc) = insert_trunc(self, operand, ty, sink) {
                    self.function.set_operand(sink, index, trunc);
                }
            }
        }

        // Cleanup: the zexts now redundant, with the truncs they read.
        for node in visited.iter() {
            let Node::Inst(zext) = node else { continue };
            if *self.opcode(zext) != Opcode::Cast(CastOp::ZExt) || self.function.instruction(zext).ty != ext_ty {
                continue;
            }
            let source = self.function.instruction(zext).operands[0];
            if self.function.operand_type(self.context, source) == Some(ext_ty) {
                self.replace_all_users_of_with(node, source, &mut removed);
                continue;
            }
            // A trunc inserted for a zext sink, whose input is known in range.
            if let Operand::Value(value) = source
                && let ValueDef::Instruction(trunc) = self.function.value(value).def
                && new_insts.contains(&Node::Inst(trunc))
                && *self.opcode(trunc) == Opcode::Cast(CastOp::Trunc)
            {
                let wide = self.function.instruction(trunc).operands[0];
                self.replace_all_users_of_with(node, wide, &mut removed);
            }
        }
        for inst in removed {
            self.insts_to_remove.insert(inst);
        }
        let pending: Vec<InstId> = self.insts_to_remove.iter().collect();
        for inst in pending {
            self.drop_all_references(inst);
        }
    }

    /// `dropAllReferences`: the instruction reads nothing, so what it read has
    /// one use fewer.
    fn drop_all_references(
        &mut self,
        inst: InstId,
    ) {
        let hidden: Vec<Operand> = self
            .operands(inst)
            .into_iter()
            .map(|operand| match self.function.operand_type(self.context, operand) {
                Some(ty) => Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison })),
                None => operand,
            })
            .collect();
        self.function.set_operands(inst, hidden);
    }

    // ---- the search ------------------------------------------------------

    /// `TryToPromote`.
    fn try_to_promote(
        &mut self,
        start: Node,
        promoted_width: u32,
    ) -> bool {
        let Some(orig) = self.ty(start) else { return false };
        self.type_size = self.scalar_bits(orig);
        self.safe_to_promote.clear();
        self.safe_wrap.clear();

        if !self.is_supported_value(start) || !self.should_promote(start) || !self.is_legal_to_promote(start) {
            return false;
        }

        let mut work_list: Ordered<Node> = Ordered::new();
        let mut sources: Ordered<Node> = Ordered::new();
        let mut sinks: Ordered<InstId> = Ordered::new();
        let mut visited: Ordered<Node> = Ordered::new();
        work_list.insert(start);

        // Iterate through, and add to, a tree of operands and users in the
        // use-def.
        while let Some(node) = work_list.pop_back() {
            if visited.contains(&node) {
                continue;
            }
            // Ignore non-instructions, other than arguments.
            if !matches!(node, Node::Inst(_)) && !self.is_source(node) {
                continue;
            }
            // Already explored from somewhere: the tree has been explored.
            if !self.all_visited.insert(node) {
                return false;
            }
            visited.insert(node);

            // Calls can be both sources and sinks.
            if self.is_sink(node) {
                let Node::Inst(inst) = node else { unreachable!("a sink is an instruction") };
                sinks.insert(inst);
            }
            if self.is_source(node) {
                sources.insert(node);
            }

            if !self.is_sink(node)
                && !self.is_source(node)
                && let Node::Inst(inst) = node
            {
                // Visit the operands of any instruction visited.
                let is_select = *self.opcode(inst) == Opcode::Select;
                for (index, operand) in self.operands(inst).into_iter().enumerate() {
                    // Skip the condition of selects.
                    if is_select && index == 0 {
                        continue;
                    }
                    let operand = self.node(operand);
                    if !self.add_legal_inst(operand, &visited, &mut work_list) {
                        return false;
                    }
                }
            }

            // Do not visit the users of a node that is not going to be mutated
            // unless it is a source.
            if self.is_source(node) || self.should_promote(node) {
                let value = match node {
                    Node::Inst(inst) => self.function.instruction(inst).result,
                    Node::Arg(value) => Some(value),
                    _ => None,
                };
                if let Some(value) = value {
                    let users: Vec<InstId> = self.function.users(value).iter().map(|one| one.user).collect();
                    for user in users {
                        if !self.add_legal_inst(Node::Inst(user), &visited, &mut work_list) {
                            return false;
                        }
                    }
                }
            }
        }

        let mut to_promote = 0usize;
        let mut non_free_args = 0usize;
        let mut non_loop_sources = 0usize;
        let mut loop_sinks = 0usize;
        let mut blocks: HashSet<BlockId> = HashSet::new();
        for node in visited.iter() {
            let block = match node {
                Node::Inst(inst) => self.function.parent(inst),
                _ => None,
            };
            if let Some(block) = block {
                blocks.insert(block);
            }
            if sources.contains(&node) {
                if let Node::Arg(value) = node
                    && let ValueDef::Argument(at) = self.function.value(value).def
                {
                    let attrs = self.function.parameter_attrs.get(at as usize);
                    let has = |name: &str| attrs.is_some_and(|attrs| llrm_mir::memory::has(attrs, name));
                    if !has("zeroext") && !has("signext") {
                        non_free_args += 1;
                    }
                }
                if !block.is_some_and(|block| self.in_loop[block.0 as usize]) {
                    non_loop_sources += 1;
                }
                continue;
            }
            let Node::Inst(inst) = node else { continue };
            if *self.opcode(inst) == Opcode::Phi {
                continue;
            }
            if block.is_some_and(|block| self.in_loop[block.0 as usize]) {
                loop_sinks += 1;
            }
            if sinks.contains(&inst) {
                continue;
            }
            to_promote += 1;
        }

        // DAG optimizations should handle these cases better, especially for
        // function arguments.
        let start_is_phi = matches!(start, Node::Inst(inst) if *self.opcode(inst) == Opcode::Phi);
        if !start_is_phi
            && !(loop_sinks > 0 && non_loop_sources > 0)
            && (to_promote < 2 || (blocks.len() == 1 && non_free_args > self.safe_wrap.len()))
        {
            return false;
        }

        self.mutate(promoted_width, &visited, &sources, &sinks);
        true
    }

    /// The `AddLegalInst` of `TryToPromote`: whether `node` was added to the
    /// work list, was already visited, or need not be explored.
    fn add_legal_inst(
        &mut self,
        node: Node,
        visited: &Ordered<Node>,
        work_list: &mut Ordered<Node>,
    ) -> bool {
        if visited.contains(&node) {
            return true;
        }
        // Skip promoting GEPs as their indices were canonicalized to pointer
        // width.
        if let Node::Inst(inst) = node
            && matches!(self.opcode(inst), Opcode::GetElementPtr { .. })
        {
            return false;
        }
        if !self.is_supported_value(node) || (self.should_promote(node) && !self.is_legal_to_promote(node)) {
            return false;
        }
        work_list.insert(node);
        true
    }

    /// `GetPromoteWidth`: the width to promote an instruction's result to, 0 if
    /// it is not to be.
    fn promote_width(
        &self,
        node: Node,
    ) -> u32 {
        let Some(bits) = self.ty(node).and_then(|ty| self.context.types.int_bits(ty)) else { return 0 };
        if self.lowering.is_type_legal(bits) {
            return 0;
        }
        // `TypePromoteInteger`: below the widest legal integer (DEVIATE TP2).
        let Some(promoted) = WIDTHS.into_iter().find(|&width| width > bits && self.lowering.is_type_legal(width))
        else {
            return 0;
        };
        if self.lowering.is_sext_cheaper_than_zext(bits, promoted) {
            return 0;
        }
        if self.register_bits < promoted {
            return 0;
        }
        promoted
    }

    fn run(&mut self) -> bool {
        let mut made_change = false;
        let blocks: Vec<BlockId> = self.function.layout().to_vec();
        for block in blocks {
            let mut current = self.function.iter_block(block).next();
            while let Some(inst) = current {
                let node = Node::Inst(inst);
                if !self.all_visited.contains(&node) {
                    made_change |= self.search_from(block, inst);
                }
                // The next of the instruction as it now is: what was inserted
                // after it is visited, as the pin's iterator does.
                current = self.function.next_in_block(inst);
            }
            for inst in std::mem::replace(&mut self.insts_to_remove, Ordered::new()).iter() {
                self.function.erase(inst).expect("a promoted-away instruction has no users");
            }
        }
        self.all_visited.clear();
        self.safe_to_promote.clear();
        self.safe_wrap.clear();
        made_change
    }

    /// The three starting points of `run` for one instruction.
    fn search_from(
        &mut self,
        block: BlockId,
        inst: InstId,
    ) -> bool {
        let node = Node::Inst(inst);
        let instruction = self.function.instruction(inst);
        let (opcode, operands) = (instruction.opcode.clone(), instruction.operands.clone());
        if opcode == Opcode::Cast(CastOp::ZExt)
            && let Node::Inst(phi) = self.node(operands[0])
            && *self.opcode(phi) == Opcode::Phi
            && self.is_integer(node)
            && self.in_loop[block.0 as usize]
        {
            let width = self.bits_of(node);
            if self.register_bits < width {
                return false;
            }
            return self.try_to_promote(Node::Inst(phi), width);
        }
        if let Opcode::ICmp(predicate) = opcode {
            // Search up from the compares, skipping signed ones.
            if is_signed(predicate) {
                return false;
            }
            for operand in operands {
                let operand = self.node(operand);
                if let Node::Inst(_) = operand {
                    let width = self.promote_width(operand);
                    if width != 0 {
                        return self.try_to_promote(operand, width);
                    }
                }
            }
            return false;
        }
        if self.is_trunc_to_i1(node) {
            // Like an unsigned icmp, a scalar trunc to i1 is a boolean
            // boundary.
            let operand = self.node(operands[0]);
            if let Node::Inst(_) = operand {
                let width = self.promote_width(operand);
                if width != 0 {
                    return self.try_to_promote(operand, width);
                }
            }
        }
        false
    }
}
