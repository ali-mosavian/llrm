//! Loads, stores and memory intrinsics get the alignment their pointer is
//! known to have, as LLVM's `InferAlignment`.
//!
//! # How it works
//! A translation of
//! [`InferAlignment.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/InferAlignment.cpp)
//! at d1106deb71cc. A first walk raises an alloca to the preferred alignment
//! of what a load or store through it accesses (`tryEnforceAlignment`). A
//! second walk, down the dominator tree, gives each load, store and
//! `memcpy`/`memmove`/`memset` pointer argument the alignment the low zero bits
//! of the pointer prove (`computeKnownBits`), or that another access to the
//! same base pointer higher in the tree proved. `ptrtoint p & mask` and
//! `trunc (ptrtoint p)` fold to what that alignment decides.
//!
//! # Example
//! ```text
//!  before                                after
//!  call @llvm.memcpy(ptr @b, ptr @a,     call @llvm.memcpy(ptr align 4 @b,
//!                    i32 100, i1 0)                        ptr align 4 @a, i32 100, i1 0)
//! ```
//! with `@a` and `@b` `align 4`: the backend then copies in dwords.
//!
//! DEVIATE 1: `tryEnforceAlignment` does not raise the alignment of a global
//! variable (it returns the global's own): a function pass cannot reach the
//! module's globals. Exit: a module-level step before the function passes.
//!
//! DEVIATE 2: `masked.load`/`masked.store` are not looked at (MIR has no
//! vectors), nor are atomic memory operations (MIR has none).
//!
//! DEVIATE 3: `isPassingUndefUB` reads the call's own parameter attributes,
//! not the callee declaration's.
//!
//! # Parity slot
//! `infer-alignment@1362` and `infer-alignment@1498`
//! (PassBuilderPipelines.cpp#L1362, #L1498).
//!
//! # Keeps
//! Only alignments, and the operands of the folds above, change: blocks and
//! edges are unchanged.

use llrm_mir::context::ConstantKind;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::facts::Fact;
use llrm_mir::hash::HashMap;
use llrm_mir::module::{BlockId, Function, GlobalKind, GlobalValue, InstId, MetadataNode, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::guarantee::strip_pointer_casts_same_representation;
use llrm_mir::valuetracking::known::{compute_known_bits, pointer_alignment};
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_mir::valuetracking::pattern::{IntrinsicId, call_arguments, intrinsic_id};
use llrm_mir::valuetracking::{FunctionAnalyses, Query};

/// `Value::MaxAlignmentExponent`.
const MAX_ALIGNMENT_EXPONENT: u32 = 32;

/// What the pass reads besides the function.
struct Env<'a> {
    globals: &'a [GlobalValue],
    metadata: &'a [MetadataNode],
    layout: &'a DataLayout,
}

/// Which of the two walks asks.
#[derive(Clone, Copy, PartialEq)]
enum Walk {
    /// The first: the preferred alignment, enforced where it can be.
    Enforce,
    /// The second: known bits and the base pointer.
    Known,
}

/// `ScopedHashTable<Value *, Align>`: one scope per dominator-tree node.
#[derive(Default)]
struct Scoped {
    table: HashMap<Operand, u64>,
    /// What each insert shadowed, undone when its scope ends.
    log: Vec<(Operand, Option<u64>)>,
}

impl Scoped {
    fn insert(
        &mut self,
        key: Operand,
        align: u64,
    ) {
        let shadowed = self.table.insert(key, align);
        self.log.push((key, shadowed));
    }

    fn undo_to(
        &mut self,
        mark: usize,
    ) {
        while self.log.len() > mark {
            let (key, shadowed) = self.log.pop().expect("a logged insert");
            match shadowed {
                Some(align) => self.table.insert(key, align),
                None => self.table.remove(&key),
            };
        }
    }
}

/// `commonAlignment(align, offset)`.
fn common_alignment(
    align: u64,
    offset: u64,
) -> u64 {
    if offset == 0 { align } else { align.min(1 << offset.trailing_zeros()) }
}

struct State<'a> {
    context: &'a mut Context,
    function: &'a mut Function,
    env: Env<'a>,
    analyses: FunctionAnalyses,
    bases: Scoped,
}

impl State<'_> {
    fn query(&self) -> Query<'_> {
        Query {
            context: &*self.context,
            globals: self.env.globals,
            metadata: self.env.metadata,
            function: &*self.function,
            layout: self.env.layout,
            dt: Some(self.analyses.dominator_tree()),
            assumptions: Some(self.analyses.assumption_cache()),
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    /// `getLoadStorePointerOperand`, `getLoadStoreAlignment` (the ABI one where
    /// none is stated) and `DL.getPrefTypeAlign(getLoadStoreType(I))`.
    fn access(
        &self,
        inst: InstId,
    ) -> Option<(Operand, u64, u64)> {
        let instruction = self.function.instruction(inst);
        let (pointer, ty, stated) = match instruction.opcode {
            Opcode::Load { align, .. } => (instruction.operands[0], instruction.ty, align),
            Opcode::Store { align, .. } => {
                (instruction.operands[1], self.query().type_of(instruction.operands[0])?, align)
            }
            _ => return None,
        };
        let types = &self.context.types;
        let old = stated.unwrap_or_else(|| self.env.layout.align(types, ty));
        Some((pointer, old, self.env.layout.pref_align(types, ty)))
    }

    /// `setLoadStoreAlignment`.
    fn set_alignment(
        &mut self,
        inst: InstId,
        align: u64,
    ) {
        let opcode = match self.function.instruction(inst).opcode {
            Opcode::Load { volatile, .. } => Opcode::Load { align: Some(align), volatile },
            Opcode::Store { volatile, .. } => Opcode::Store { align: Some(align), volatile },
            _ => unreachable!("a load or a store"),
        };
        self.function.set_opcode(inst, opcode);
    }

    /// `tryEnforceAlignment(V, PrefAlign, DL)`.
    fn enforce(
        &mut self,
        pointer: Operand,
        pref: u64,
    ) -> u64 {
        let stripped = strip_pointer_casts_same_representation(&self.query(), pointer);
        let query = self.query();
        if let Some((inst, instruction)) = query.instruction_of(stripped)
            && let Opcode::Alloca { allocated, align, address_space } = instruction.opcode
        {
            let current = align.unwrap_or_else(|| self.env.layout.align(&self.context.types, allocated));
            if pref <= current {
                return current;
            }
            // If the preferred alignment is greater than the natural stack
            // alignment then don't round up: it avoids dynamic stack
            // realignment.
            if self.env.layout.stack_align.is_some_and(|stack| pref > stack) {
                return current;
            }
            self.function.set_opcode(inst, Opcode::Alloca { allocated, align: Some(pref), address_space });
            return pref;
        }
        if let Operand::Constant(id) = stripped
            && let ConstantKind::Global(global) = &query.context().get(id).kind
            && matches!(
                self.env.globals.get(global.0 as usize),
                Some(GlobalValue { kind: GlobalKind::Variable(_), .. })
            )
        {
            // DEVIATE 1: a global is not raised.
            return pointer_alignment(&query, stripped);
        }
        1
    }

    /// `computeKnownBits(PtrOp, DL, &AC, &I, &DT)` as an alignment.
    fn from_known_bits(
        &self,
        at: InstId,
        pointer: Operand,
    ) -> u64 {
        let query = self.query().with_cxt(Some(at));
        let known = compute_known_bits(&query, pointer, 0);
        let trailing = known.count_min_trailing_zeros().min(MAX_ALIGNMENT_EXPONENT);
        1 << trailing.min(known.width - 1)
    }

    /// `InferFromBasePointer`.
    fn from_base_pointer(
        &mut self,
        pointer: Operand,
        access_align: u64,
    ) -> u64 {
        let width = self.env.layout.pointer(0).index_bits;
        let (base, offset) = strip_and_accumulate_constant_offsets(&self.query(), pointer, width, true);
        let offset = u64::try_from(offset).unwrap_or(u64::MAX);
        // The base pointer's alignment from this access and its offset.
        let derived = common_alignment(access_align, offset);
        if let Some(best) = self.bases.table.get(&base).copied().filter(|&best| best != 1)
            && best > derived
        {
            return common_alignment(best, offset);
        }
        self.bases.insert(base, derived);
        access_align
    }

    /// The alignment `walk` gives `pointer`, from `old` and the preferred one.
    fn improved(
        &mut self,
        walk: Walk,
        at: InstId,
        pointer: Operand,
        old: u64,
        pref: u64,
    ) -> u64 {
        match walk {
            Walk::Enforce if pref > old => old.max(self.enforce(pointer, pref)),
            Walk::Enforce => old,
            Walk::Known => {
                let known = self.from_known_bits(at, pointer);
                known.max(self.from_base_pointer(pointer, old))
            }
        }
    }

    /// `tryToImproveAlign`.
    fn try_to_improve(
        &mut self,
        walk: Walk,
        inst: InstId,
    ) -> bool {
        if let Some((pointer, old, pref)) = self.access(inst) {
            let new = self.improved(walk, inst, pointer, old, pref);
            if new > old {
                self.set_alignment(inst, new);
                return true;
            }
        }
        let instruction = self.function.instruction(inst).clone();
        if let Some(changed) = self.fold_mask_or_trunc(walk, inst, &instruction) {
            return changed;
        }
        self.improve_memory_intrinsic(walk, inst, &instruction)
    }

    /// `ptrtoint p & Const` and `trunc (ptrtoint p)`, decided by the alignment.
    fn fold_mask_or_trunc(
        &mut self,
        walk: Walk,
        inst: InstId,
        instruction: &llrm_mir::module::Instruction,
    ) -> Option<bool> {
        let ptr_to_int = |this: &Self, operand: Operand| {
            let query = this.query();
            let (_, producer) = query.instruction_of(operand)?;
            (producer.opcode == Opcode::Cast(CastOp::PtrToInt)).then(|| producer.operands[0])
        };
        let result = instruction.result?;
        match instruction.opcode {
            Opcode::Binary(BinaryOp::And) => {
                let pointer = ptr_to_int(self, instruction.operands[0])?;
                let (mask, width) = self.query().int_constant(instruction.operands[1])?;
                let actual = self.improved(walk, inst, pointer, 1, 1);
                let zero = self.int_zero(instruction.ty);
                if mask < u128::from(actual) {
                    self.function.replace_all_uses_with(result, zero);
                    return Some(true);
                }
                // `APInt::getBitsSetFrom(width, Log2(actual))`.
                let from = actual.trailing_zeros();
                let all = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                let bits_set_from = all & !((1u128 << from) - 1);
                if mask >= bits_set_from {
                    self.function.replace_all_uses_with(result, instruction.operands[0]);
                    return Some(true);
                }
                None
            }
            Opcode::Cast(CastOp::Trunc) => {
                let pointer = ptr_to_int(self, instruction.operands[0])?;
                let actual = self.improved(walk, inst, pointer, 1, 1);
                let bits = self.context.types.int_bits(instruction.ty)?;
                if actual.trailing_zeros() >= bits {
                    let zero = self.int_zero(instruction.ty);
                    self.function.replace_all_uses_with(result, zero);
                    return Some(true);
                }
                None
            }
            _ => None,
        }
    }

    fn int_zero(
        &mut self,
        ty: llrm_mir::types::TypeId,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, 0))
    }

    /// The `MemIntrinsic` part: each pointer argument's `align` attribute.
    fn improve_memory_intrinsic(
        &mut self,
        walk: Walk,
        inst: InstId,
        instruction: &llrm_mir::module::Instruction,
    ) -> bool {
        let Opcode::Call(info) = &instruction.opcode else { return false };
        let is_memory = intrinsic_id(&self.query(), inst).is_some_and(|id| {
            matches!(
                id,
                IntrinsicId::Memcpy | IntrinsicId::MemcpyInline | IntrinsicId::Memmove | IntrinsicId::Memset
            )
        }) || self.query().callee_name(inst).is_some_and(|name| name.starts_with("llvm.memset.inline"));
        if !is_memory {
            return false;
        }
        let mut changed = false;
        for (arg, &value) in call_arguments(instruction).iter().enumerate() {
            if !self.query().is_pointer(value) {
                continue;
            }
            let attrs = info.argument_attrs.get(arg).map(Vec::as_slice).unwrap_or_default();
            let old = attrs
                .iter()
                .find_map(|one| match one {
                    Attribute::Int(name, align) if name == "align" => Some(*align),
                    _ => None,
                })
                .unwrap_or(1);
            // The align attribute proves the alignment only if passing poison
            // is UB (DEVIATE 3 on where `noundef` is read).
            let undef_ub = attrs
                .iter()
                .any(
                    |one| match one {
                        Attribute::Flag(_) if Fact::of_attribute(one) == Some(Fact::NoUndef) => true,
                        Attribute::Int(name, _) => name == "dereferenceable" || name == "dereferenceable_or_null",
                        _ => false,
                    },
                );
            let known = if undef_ub { old } else { 1 };
            let new = self.improved(walk, inst, value, known, 1);
            if new <= old {
                continue;
            }
            self.function.set_argument_align(inst, arg, new);
            changed = true;
        }
        changed
    }
}

/// `inferAlignment(F, AC, DT)`. Whether anything changed.
fn infer_alignment(
    context: &mut Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
) -> bool {
    let analyses = FunctionAnalyses::of(context, globals, metadata, layout, function).with_lazy_dom_conditions();
    let mut state =
        State { context, function, env: Env { globals, metadata, layout }, analyses, bases: Scoped::default() };
    let mut changed = false;

    // Enforce the preferred type alignment first: it may improve what is
    // inferred below.
    let blocks: Vec<BlockId> = state.function.layout().to_vec();
    for &block in &blocks {
        let insts: Vec<InstId> = state.function.iter_block(block).collect();
        for inst in insts {
            changed |= state.try_to_improve(Walk::Enforce, inst);
        }
    }

    // The alloca the first walk raised changes what `computeKnownBits` says.
    state.analyses =
        FunctionAnalyses::of(state.context, globals, metadata, layout, state.function).with_lazy_dom_conditions();
    let tree: &DomTree = &state.analyses.dominator_tree().clone();
    enum Step {
        Enter(BlockId),
        Leave(usize),
    }
    let mut stack = vec![Step::Enter(tree.root())];
    while let Some(step) = stack.pop() {
        match step {
            Step::Leave(mark) => state.bases.undo_to(mark),
            Step::Enter(block) => {
                let mark = state.bases.log.len();
                let insts: Vec<InstId> = state.function.iter_block(block).collect();
                for inst in insts {
                    changed |= state.try_to_improve(Walk::Known, inst);
                }
                stack.push(Step::Leave(mark));
                for child in tree.children(Some(block)).into_iter().rev() {
                    stack.push(Step::Enter(child));
                }
            }
        }
    }
    changed
}

pub struct InferAlignment;

impl FunctionPass for InferAlignment {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "inferalignment"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("infer-alignment")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        if infer_alignment(unit.context, &outer.globals, unit.metadata, unit.layout, unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "inferalignment_tests.rs"]
mod tests;
