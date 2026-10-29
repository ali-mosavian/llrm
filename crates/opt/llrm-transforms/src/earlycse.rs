//! LLVM's EarlyCSE with MemorySSA (`early-cse<memssa>`): one walk down the
//! dominator tree, scoped tables of what is available, and a generation count
//! that memory operations advance.
//!
//! A translation of `llvm/lib/Transforms/Scalar/EarlyCSE.cpp` at d1106deb71cc;
//! each function carries the permalink of the one it translates. What is not
//! translated is marked DEVIATE with its reason:
//!
//! - DEVIATE no `invariant.start`, `guard`, `sideeffect`, `pseudoprobe`, masked
//!   or target memory intrinsics, memset forwarding, fences or atomics: MIR has
//!   none (or `fill` makes them later). `llvm.assume` is handled.
//! - DEVIATE flags are intersected whenever a value is reused (`combineIRFlags`
//!   does it only when poison-generating flags remain); a load's metadata not
//!   shared by both loads is dropped (`combineMetadataForCSE` merges it).
//! - DEVIATE a call's attributes are not intersected; a call that only reads or
//!   only writes memory (`memory::of`, which counts every location including
//!   `inaccessiblemem`) is numbered unless it is an `llvm.`/`llrm.` marker.
//! - DEVIATE a GEP's constant offset is taken for byte GEPs only.
//! - DEVIATE MemorySSA is the translated `Mssa`, built once for the function as
//!   it was and not updated (`MSSAUpdater`); an erased store stays a def in it,
//!   which is only conservative. Erasures wait for the end of the walk so that
//!   what it asks about still exists.
//! - the clobbering access is `llrm_analysis::mssa::view::Translated`'s: one
//!   access, the `MemoryPhi` where the nearest clobbers diverge, as LLVM's
//!   walker answers.
//! - DEVIATE (call walker) the clobbering access of a call is its defining
//!   access, not LLVM's call-vs-call query (the walker asks the modref of the
//!   call against each def): any write between two calls blocks the CSE, which
//!   is only conservative. It rests on the fact that llrm has no call-vs-call
//!   alias query until basicaa-port's S3 (an unknown callee's footprint omits
//!   inaccessible memory, #2219). Exit: S3 landed, then translate the walker's
//!   call query. The eligibility is pinned by
//!   `test_a_call_that_touches_inaccessible_memory_is_not_a_read_only_or_write_only_call`.
//!
//! # How it works
//! The dominator tree is walked depth first with a scope per node: the scoped
//! tables hold the expressions, loads, calls and GEPs available there, each
//! stamped with the generation of memory when it was made. An instruction that
//! `simplify_instruction` folds is replaced; one equal to an available entry
//! is replaced by it; a load is the stored or loaded value of an earlier
//! access when the MemorySSA clobber query says nothing wrote between them
//! (`isSameMemGeneration`); a store that rewrites what an earlier one wrote
//! goes. Entering a branch arm records the condition as known.
//!
//! # Example
//! Before (`test_a_commuted_operation_is_the_one_before`):
//! ```mir
//! define i32 @f(i32 %x, i32 %y) {
//! b0:
//!   %a = add i32 %x, %y
//!   %b = add i32 %y, %x
//!   %c = mul i32 %a, %b
//!   ret i32 %c
//! }
//! ```
//!
//! After:
//! ```mir
//! define i32 @f(i32 %x, i32 %y) {
//! b0:
//!   %a = add i32 %x, %y
//!   %c = mul i32 %a, %a
//!   ret i32 %c
//! }
//! ```
//!
//! # Used by
//! [`crate::pipeline`]'s `llvm_order` lists (`earlycse@649`, `earlycse@1180`).

use std::rc::Rc;

use llrm_analysis::manager::{MemorySsa, MssaGraph};
use llrm_analysis::memory::Unit as View;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::view::Translated;
use llrm_analysis::mssa::{Mssa, shadow};
use llrm_mir::context::Context;
use llrm_mir::domtree::DomTree;
use llrm_mir::memory;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::FunctionAnalyses;
use llrm_support::hash::HashMap;

/// `EarlyCSEMssaOptCap`, `-earlycse-mssa-optimization-cap`.
const MSSA_OPT_CAP: usize = 500;

/// ScopedHashTable: `insert` shadows, `lookup` finds the innermost, and
/// leaving a scope takes back what it inserted.
struct Scoped<K, V> {
    map: HashMap<K, V>,
    /// What each insert shadowed, to put back when its scope is left: a
    /// table of one value a key, not a stack of them.
    inserted: Vec<(K, Option<V>)>,
    marks: Vec<usize>,
}

impl<K: std::hash::Hash + Eq + Clone, V> Scoped<K, V> {
    fn new() -> Self {
        Self { map: HashMap::default(), inserted: Vec::new(), marks: Vec::new() }
    }

    fn enter(&mut self) {
        self.marks.push(self.inserted.len());
    }

    fn leave(&mut self) {
        let mark = self.marks.pop().expect("a scope entered");
        while self.inserted.len() > mark {
            let (key, shadowed) = self.inserted.pop().expect("a key");
            match shadowed {
                Some(value) => {
                    self.map.insert(key, value);
                }
                None => {
                    self.map.remove(&key);
                }
            }
        }
    }

    fn insert(
        &mut self,
        key: K,
        value: V,
    ) {
        let shadowed = self.map.insert(key.clone(), value);
        self.inserted.push((key, shadowed));
    }

    fn lookup(
        &self,
        key: &K,
    ) -> Option<&V> {
        self.map.get(key)
    }
}

/// `SelectPatternFlavor` of min/max.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Flavor {
    UMax,
    UMin,
    SMax,
    SMin,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Tag {
    Binary(BinaryOp),
    ICmp(IntPredicate),
    FCmp(FloatPredicate),
    Cast(CastOp),
    FNeg,
    Freeze,
    Select,
    MinMax(Flavor),
    SelectCmp(IntPredicate),
    ExtractValue(Vec<u32>),
    InsertValue(Vec<u32>),
    Call(TypeId, u32),
}

/// What `SimpleValue`'s hash and `isEqual` agree on: two instructions with one
/// key are the same value. The key is the canonical form `getHashValueImpl`
/// picks for commuted operands, swapped compares and inverted selects.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SimpleKey {
    tag: Tag,
    ty: TypeId,
    operands: Ops,
}

/// A key's operands: up to four held in place, as the operations a key is made
/// of have two or three; more (a call's arguments) go to a vector. A vector
/// for each key was 1,386 allocations on d_faces' largest body.
#[derive(Clone, Debug)]
enum Ops {
    Inline(u8, [Operand; 4]),
    Heap(Vec<Operand>),
}

impl Ops {
    fn of(operands: &[Operand]) -> Self {
        match operands {
            [] => Self::Heap(Vec::new()),
            [first, ..] if operands.len() <= 4 => {
                let mut items = [*first; 4];
                items[..operands.len()].copy_from_slice(operands);
                Self::Inline(operands.len() as u8, items)
            }
            _ => Self::Heap(operands.to_vec()),
        }
    }

    fn as_slice(&self) -> &[Operand] {
        match self {
            Self::Inline(len, items) => &items[..*len as usize],
            Self::Heap(items) => items,
        }
    }
}

impl PartialEq for Ops {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for Ops {}

impl std::hash::Hash for Ops {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.as_slice().hash(state);
    }
}

/// `GEPValue`: the pointer and the constant offset, or the whole GEP.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum GepKey {
    Offset(TypeId, Operand, i64),
    Whole(TypeId, TypeId, Vec<Operand>),
}

/// `EarlyCSE::LoadValue`.
#[derive(Clone, Copy)]
struct LoadValue {
    def: InstId,
    generation: u32,
    is_load: bool,
}

/// LLVM's numbering of `CmpInst::Predicate`, which orders the hash's choices.
fn rank(predicate: IntPredicate) -> u8 {
    match predicate {
        IntPredicate::Eq => 32,
        IntPredicate::Ne => 33,
        IntPredicate::Ugt => 34,
        IntPredicate::Uge => 35,
        IntPredicate::Ult => 36,
        IntPredicate::Ule => 37,
        IntPredicate::Sgt => 38,
        IntPredicate::Sge => 39,
        IntPredicate::Slt => 40,
        IntPredicate::Sle => 41,
    }
}

fn commutes(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::FAdd | BinaryOp::FMul
    )
}

/// What EarlyCSE asks of MemorySSA (`MemorySSA::getMemoryAccess`,
/// `getWalker()->getClobberingMemoryAccess`,
/// `MemoryUseOrDef::getDefiningAccess`, `MemorySSA::dominates`,
/// `MemorySSAUpdater::removeMemoryAccess`), so that a translation of
/// MemorySSA.cpp plugs in behind it.
pub trait MemorySsaView {
    /// The access number of `inst`; none where it touches no memory.
    fn memory_access(
        &self,
        inst: InstId,
    ) -> Option<usize>;
    /// `getClobberingMemoryAccess(inst)`: the one access that clobbers, the
    /// `MemoryPhi` where the nearest clobbers diverge.
    fn clobbering_access(
        &self,
        inst: InstId,
    ) -> Option<usize>;
    /// `inst`'s access's defining access.
    fn defining_access(
        &self,
        inst: InstId,
    ) -> Option<usize>;
    /// Whether access `ahead` dominates access `access`.
    fn dominates(
        &self,
        ahead: usize,
        access: usize,
    ) -> bool;
    /// `removeMemoryAccess(inst)`, before `inst` is erased from `function`.
    fn remove_memory_access(
        &mut self,
        _function: &Function,
        _inst: InstId,
    ) {
    }
    /// The graph as the removals left it, where the view holds one.
    fn into_mssa(self: Box<Self>) -> Option<Mssa> {
        None
    }
    /// Whether there is a MemorySSA behind the view (`MSSA` is not null).
    fn present(&self) -> bool {
        true
    }
}

/// No MemorySSA (`MSSA == nullptr`): `removeMSSA` does nothing and
/// `isSameMemGeneration` is false unless the generations are equal.
struct Detached;

impl MemorySsaView for Detached {
    fn memory_access(
        &self,
        _inst: InstId,
    ) -> Option<usize> {
        None
    }

    fn clobbering_access(
        &self,
        _inst: InstId,
    ) -> Option<usize> {
        None
    }

    fn defining_access(
        &self,
        _inst: InstId,
    ) -> Option<usize> {
        None
    }

    fn dominates(
        &self,
        _ahead: usize,
        _access: usize,
    ) -> bool {
        false
    }

    fn present(&self) -> bool {
        false
    }
}

/// The translated MemorySSA (`llrm_analysis::mssa`) as that view.
impl MemorySsaView for Translated<'_> {
    fn memory_access(
        &self,
        inst: InstId,
    ) -> Option<usize> {
        Translated::memory_access(self, inst).map(|access| access as usize)
    }

    fn clobbering_access(
        &self,
        inst: InstId,
    ) -> Option<usize> {
        Translated::clobbering_access(self, inst).map(|access| access as usize)
    }

    fn defining_access(
        &self,
        inst: InstId,
    ) -> Option<usize> {
        Translated::defining_access(self, inst).map(|access| access as usize)
    }

    fn dominates(
        &self,
        ahead: usize,
        access: usize,
    ) -> bool {
        Translated::dominates(self, ahead as u32, access as u32)
    }

    fn remove_memory_access(
        &mut self,
        function: &Function,
        inst: InstId,
    ) {
        Translated::remove_memory_access(self, function, inst);
    }

    fn into_mssa(self: Box<Self>) -> Option<Mssa> {
        Some(Translated::into_mssa(*self))
    }
}

/// One walk of a function.
struct Walk<'a> {
    context: &'a Context,
    function: &'a mut Function,
    /// The function as it was, which `mssa` describes.
    view: &'a View<'a>,
    mssa: Box<dyn MemorySsaView + 'a>,
    callees: &'a memory::Callees,
    tree: &'a DomTree,
    /// `SQ.DT` and `SQ.AC` of the function as it was.
    analyses: &'a FunctionAnalyses,
    values: Scoped<SimpleKey, Operand>,
    loads: Scoped<Operand, LoadValue>,
    invariants: Scoped<(Operand, TypeId), u32>,
    calls: Scoped<SimpleKey, (InstId, u32)>,
    geps: Scoped<GepKey, Operand>,
    generation: u32,
    clobber_counter: usize,
    /// Instructions to erase when the walk is done.
    gone: Vec<InstId>,
}

impl Walk<'_> {
    fn enter(&mut self) {
        self.values.enter();
        self.loads.enter();
        self.invariants.enter();
        self.calls.enter();
        self.geps.enter();
    }

    fn leave(&mut self) {
        self.values.leave();
        self.loads.leave();
        self.invariants.leave();
        self.calls.leave();
        self.geps.leave();
    }

    /// The instruction that defines `operand`, if one does.
    fn definer(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<u128> {
        let Operand::Constant(id) = operand else { return None };
        match self.context.get(id).kind {
            llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
            _ => None,
        }
    }

    fn int_width(
        &self,
        ty: TypeId,
    ) -> Option<u32> {
        self.context.types.int_bits(ty)
    }

    /// `m_Not(x)`: `xor x, -1`.
    fn negated(
        &self,
        operand: Operand,
    ) -> Option<Operand> {
        let inst = self.definer(operand)?;
        let op = self.function.instruction(inst);
        let Opcode::Binary(BinaryOp::Xor) = op.opcode else { return None };
        let width = self.int_width(op.ty)?;
        let ones = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
        match (op.operands[0], op.operands[1]) {
            (kept, constant) | (constant, kept) if self.int_constant(constant) == Some(ones) => Some(kept),
            _ => None,
        }
    }

    /// `matchSelectWithOptionalNotCond`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L158):
    /// the select's condition, true value and false value with a `not` of the
    /// condition looked through, and the min/max it is.
    fn select_form(
        &self,
        inst: InstId,
    ) -> Option<(Operand, Operand, Operand, Option<Flavor>)> {
        let op = self.function.instruction(inst);
        if op.opcode != Opcode::Select {
            return None;
        }
        let (mut cond, mut a, mut b) = (op.operands[0], op.operands[1], op.operands[2]);
        if let Some(inner) = self.negated(cond) {
            cond = inner;
            std::mem::swap(&mut a, &mut b);
        }
        let flavor = (|| {
            let compare = self.function.instruction(self.definer(cond)?);
            let Opcode::ICmp(mut predicate) = compare.opcode else { return None };
            if compare.operands[0] == a && compare.operands[1] == b {
            } else if compare.operands[0] == b && compare.operands[1] == a {
                predicate = predicate.swapped();
            } else {
                return None;
            }
            match predicate {
                IntPredicate::Ugt | IntPredicate::Uge => Some(Flavor::UMax),
                IntPredicate::Ult | IntPredicate::Ule => Some(Flavor::UMin),
                IntPredicate::Sgt | IntPredicate::Sge => Some(Flavor::SMax),
                IntPredicate::Slt | IntPredicate::Sle => Some(Flavor::SMin),
                _ => None,
            }
        })();
        Some((cond, a, b, flavor))
    }

    /// `SimpleValue::canHandle` and the hash/equality of `getHashValueImpl` and
    /// `isEqualImpl`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L100):
    /// the key of `inst`, or none where it is not a simple value.
    fn simple_key(
        &self,
        inst: InstId,
    ) -> Option<SimpleKey> {
        let op = self.function.instruction(inst);
        let ty = op.ty;
        let operands = op.operands.as_slice();
        let key = |tag, operands: &[Operand]| Some(SimpleKey { tag, ty, operands: Ops::of(operands) });
        match &op.opcode {
            Opcode::Binary(binary) => {
                let mut pair = [operands[0], operands[1]];
                if commutes(*binary) && pair[0] > pair[1] {
                    pair.swap(0, 1);
                }
                key(Tag::Binary(*binary), &pair)
            }
            Opcode::ICmp(predicate) => {
                let (mut left, mut right, mut predicate) = (operands[0], operands[1], *predicate);
                let swapped = predicate.swapped();
                if (left, rank(predicate)) > (right, rank(swapped)) {
                    std::mem::swap(&mut left, &mut right);
                    predicate = swapped;
                }
                key(Tag::ICmp(predicate), &[left, right])
            }
            Opcode::FCmp(predicate) => key(Tag::FCmp(*predicate), operands),
            Opcode::Cast(cast) => key(Tag::Cast(*cast), operands),
            Opcode::FNeg => key(Tag::FNeg, operands),
            Opcode::Freeze => key(Tag::Freeze, operands),
            Opcode::ExtractValue(indices) => key(Tag::ExtractValue(indices.clone()), operands),
            Opcode::InsertValue(indices) => key(Tag::InsertValue(indices.clone()), operands),
            Opcode::Select => {
                let (cond, mut a, mut b, flavor) = self.select_form(inst)?;
                if let Some(flavor) = flavor {
                    if a > b {
                        std::mem::swap(&mut a, &mut b);
                    }
                    return key(Tag::MinMax(flavor), &[a, b]);
                }
                let compare = self.definer(cond).map(|one| self.function.instruction(one));
                match compare {
                    Some(compare) => match compare.opcode {
                        Opcode::ICmp(predicate) => {
                            let inverse = predicate.inverse();
                            let (x, y) = (compare.operands[0], compare.operands[1]);
                            if rank(inverse) < rank(predicate) {
                                std::mem::swap(&mut a, &mut b);
                                key(Tag::SelectCmp(inverse), &[x, y, a, b])
                            } else {
                                key(Tag::SelectCmp(predicate), &[x, y, a, b])
                            }
                        }
                        _ => key(Tag::Select, &[cond, a, b]),
                    },
                    None => key(Tag::Select, &[cond, a, b]),
                }
            }
            Opcode::Call(info) => {
                if op.result.is_none() || self.marker(inst) {
                    return None;
                }
                let effects = memory::of(self.context, self.callees, self.function, inst);
                if effects.reads || effects.writes {
                    return None;
                }
                key(Tag::Call(info.function_type, info.calling_convention), operands)
            }
            _ => None,
        }
    }

    /// A call to an `llvm.` or `llrm.` function: a marker, not a computation.
    fn marker(
        &self,
        inst: InstId,
    ) -> bool {
        memory::callee(self.context, self.function, inst)
            .and_then(|global| self.view.globals.get(global.0 as usize))
            .and_then(|global| global.name.as_deref())
            .is_some_and(|name| name.starts_with("llvm.") || name.starts_with("llrm."))
    }

    fn named(
        &self,
        inst: InstId,
        name: &str,
    ) -> bool {
        memory::callee(self.context, self.function, inst)
            .and_then(|global| self.view.globals.get(global.0 as usize))
            .and_then(|global| global.name.as_deref())
            .is_some_and(|one| one == name)
    }

    /// `CallValue::canHandle`: a call that only reads memory or only writes it
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L468).
    fn call_key(
        &self,
        inst: InstId,
    ) -> Option<SimpleKey> {
        let op = self.function.instruction(inst);
        let Opcode::Call(info) = &op.opcode else { return None };
        if self.marker(inst) {
            return None;
        }
        let effects = memory::of(self.context, self.callees, self.function, inst);
        if effects.reads == effects.writes {
            return None;
        }
        Some(SimpleKey {
            tag: Tag::Call(info.function_type, info.calling_convention),
            ty: op.ty,
            operands: Ops::of(&op.operands),
        })
    }

    /// `GEPValue` with `accumulateConstantOffset`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L521).
    fn gep_key(
        &self,
        inst: InstId,
    ) -> Option<GepKey> {
        let op = self.function.instruction(inst);
        let Opcode::GetElementPtr { source } = op.opcode else { return None };
        if let ([base, index], Some(8)) = (&op.operands[..], self.int_width(source)) {
            if let (Operand::Constant(id), Some(bits)) = (index, self.int_constant(*index)) {
                let width = self.int_width(self.context.get(*id).ty)?;
                let shift = 128 - width;
                let signed = ((bits << shift) as i128) >> shift;
                return Some(GepKey::Offset(op.ty, *base, signed as i64));
            }
        }
        Some(GepKey::Whole(source, op.ty, op.operands.clone()))
    }

    fn is_load(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(self.function.instruction(inst).opcode, Opcode::Load { .. })
    }

    fn is_store(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(self.function.instruction(inst).opcode, Opcode::Store { .. })
    }

    /// `ParseMemoryInst::getPointerOperand` of a plain load or store.
    fn pointer(
        &self,
        inst: InstId,
    ) -> Option<Operand> {
        let op = self.function.instruction(inst);
        match op.opcode {
            Opcode::Load { .. } => Some(op.operands[0]),
            Opcode::Store { .. } => Some(op.operands[1]),
            _ => None,
        }
    }

    fn volatile(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.function.instruction(inst).opcode,
            Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
        )
    }

    /// `ParseMemoryInst::getValueType`.
    fn value_type(
        &self,
        inst: InstId,
    ) -> Option<TypeId> {
        let op = self.function.instruction(inst);
        match op.opcode {
            Opcode::Load { .. } => Some(op.ty),
            Opcode::Store { .. } => self.function.operand_type(self.context, op.operands[0]),
            _ => None,
        }
    }

    fn may_read(
        &self,
        inst: InstId,
    ) -> bool {
        match self.function.instruction(inst).opcode {
            Opcode::Load { .. } => true,
            Opcode::Call(_) | Opcode::Invoke(_) => memory::of(self.context, self.callees, self.function, inst).reads,
            _ => false,
        }
    }

    fn may_write(
        &self,
        inst: InstId,
    ) -> bool {
        match self.function.instruction(inst).opcode {
            Opcode::Store { .. } => true,
            Opcode::Load { volatile, .. } => volatile,
            Opcode::Call(_) | Opcode::Invoke(_) => memory::of(self.context, self.callees, self.function, inst).writes,
            _ => false,
        }
    }

    /// `EarlyCSE::isOperatingOnInvariantMemAt`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1105).
    fn invariant_at(
        &self,
        inst: InstId,
        generation: u32,
    ) -> bool {
        let op = self.function.instruction(inst);
        if matches!(op.opcode, Opcode::Load { .. }) && op.metadata.iter().any(|(kind, _)| kind == "invariant.load") {
            return true;
        }
        let (Some(pointer), Some(ty)) = (self.pointer(inst), self.value_type(inst)) else { return false };
        self.invariants.lookup(&(pointer, ty)).is_some_and(|&became| became <= generation)
    }

    /// `EarlyCSE::isSameMemGeneration`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1066).
    fn same_generation(
        &mut self,
        earlier_generation: u32,
        later_generation: u32,
        earlier: InstId,
        later: InstId,
    ) -> bool {
        if earlier_generation == later_generation {
            return true;
        }
        // `if (!MSSA) return false;` (EarlyCSE.cpp#L1074).
        if !self.mssa.present() {
            return false;
        }
        let (Some(earlier_access), Some(_)) = (self.mssa.memory_access(earlier), self.mssa.memory_access(later)) else {
            return true;
        };
        let later_def = if self.clobber_counter < MSSA_OPT_CAP {
            self.clobber_counter += 1;
            self.mssa.clobbering_access(later)
        } else {
            self.mssa.defining_access(later)
        };
        later_def.is_some_and(|def| self.mssa.dominates(def, earlier_access))
    }

    /// `EarlyCSE::getOrCreateResult` without target intrinsics: the load, or
    /// the store's value, if it has `expected`'s type.
    fn result_of(
        &self,
        inst: InstId,
        expected: TypeId,
    ) -> Option<Operand> {
        let op = self.function.instruction(inst);
        let value = match op.opcode {
            Opcode::Load { .. } => Operand::Value(op.result?),
            Opcode::Store { .. } => op.operands[0],
            _ => return None,
        };
        (self.function.operand_type(self.context, value) == Some(expected)).then_some(value)
    }

    /// `EarlyCSE::getMatchingValue`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1183).
    fn matching_value(
        &mut self,
        in_val: LoadValue,
        mem: InstId,
        generation: u32,
    ) -> Option<Operand> {
        // We don't yet handle removing loads with ordering of any kind.
        if self.volatile(mem) {
            return None;
        }
        let mem_is_load = self.is_load(mem);
        let (matching, other) = if mem_is_load { (in_val.def, mem) } else { (mem, in_val.def) };
        // For stores the result is checked before the memory generation.
        let other_type = match self.function.instruction(other).opcode {
            Opcode::Load { .. } => self.function.instruction(other).ty,
            _ => {
                // A store has no value type: a store is never "matching" a
                // store.
                if mem_is_load {
                    self.function.instruction(other).ty
                } else {
                    return None;
                }
            }
        };
        let result = if !mem_is_load { self.result_of(matching, other_type) } else { None };
        if !mem_is_load && Some(Operand::Value(self.function.instruction(in_val.def).result?)) != result {
            return None;
        }
        if !self.invariant_at(mem, in_val.generation)
            && !self.same_generation(in_val.generation, generation, in_val.def, mem)
        {
            return None;
        }
        result.or_else(|| self.result_of(matching, other_type))
    }

    /// `EarlyCSE::overridingStores`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1288).
    fn overriding_stores(
        &self,
        earlier: InstId,
        later: InstId,
    ) -> bool {
        self.pointer(earlier) == self.pointer(later)
            && self.value_type(earlier).is_some()
            && self.value_type(earlier) == self.value_type(later)
            && !self.volatile(earlier)
            && !self.volatile(later)
    }

    /// `combineIRFlags`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1257):
    /// what both have stays on `to`.
    fn combine_flags(
        &mut self,
        from: InstId,
        to: Operand,
    ) {
        let Some(to) = self.definer(to) else { return };
        let flags = self.function.instruction(to).flags.intersect(self.function.instruction(from).flags);
        if flags != self.function.instruction(to).flags {
            self.function.set_flags(to, flags);
        }
    }

    /// Whether `inst` is trivially dead: nothing reads its value and nothing
    /// else would notice it going.
    fn dead(
        &self,
        inst: InstId,
    ) -> bool {
        let op = self.function.instruction(inst);
        if op.opcode.is_terminator() || matches!(op.opcode, Opcode::Store { .. }) {
            return false;
        }
        op.result.is_none_or(|result| self.function.users(result).is_empty())
            && !crate::dead::_kept(self.context, self.callees, self.function, inst)
    }

    /// `replaceDominatedUsesWith(Curr, TorF, DT, BasicBlockEdge(Pred, BB))`:
    /// the uses the edge dominates.
    fn replace_dominated(
        &mut self,
        value: ValueId,
        with: Operand,
        pred: BlockId,
        block: BlockId,
    ) -> bool {
        let mut changed = false;
        for one in self.function.users(value).snapshot() {
            let user = self.function.instruction(one.user);
            let dominated = if user.opcode == Opcode::Phi {
                let Operand::Block(from) = user.operands[one.index as usize + 1] else { continue };
                (from == pred && self.function.parent(one.user) == Some(block)) || self.tree.dominates(block, from)
            } else {
                self.function.parent(one.user).is_some_and(|home| self.tree.dominates(block, home))
            };
            if dominated && !self.function.is_erased(one.user) {
                self.function.set_operand(one.user, one.index as usize, with);
                changed = true;
            }
        }
        changed
    }

    /// `EarlyCSE::handleBranchCondition`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1126).
    fn branch_condition(
        &mut self,
        cond: InstId,
        branch: InstId,
        block: BlockId,
        pred: BlockId,
    ) -> bool {
        let taken = self.function.instruction(branch).operands[1] == Operand::Block(block);
        let ty = self.function.instruction(cond).ty;
        let known = Operand::Constant(self.context.int(ty, i128::from(taken)));
        let mut changed = false;
        let mut work = vec![cond];
        let mut seen = vec![cond];
        while let Some(current) = work.pop() {
            if let Some(key) = self.simple_key(current) {
                self.values.insert(key, known);
            }
            if let Some(result) = self.function.instruction(current).result {
                changed |= self.replace_dominated(result, known, pred, block);
            }
            // If the condition is an AND operation, its operands hold in the
            // true branch; an OR, in the false branch.
            let op = self.function.instruction(current);
            let logical = match (&op.opcode, &op.operands[..]) {
                (Opcode::Binary(BinaryOp::And), [a, b]) if taken => Some((*a, *b)),
                (Opcode::Binary(BinaryOp::Or), [a, b]) if !taken => Some((*a, *b)),
                (Opcode::Select, [a, b, c]) if taken && self.int_constant(*c) == Some(0) => Some((*a, *b)),
                (Opcode::Select, [a, b, c]) if !taken && self.int_constant(*b) == Some(1) => Some((*a, *c)),
                _ => None,
            };
            if let Some((a, b)) = logical {
                for operand in [a, b] {
                    if let Some(inner) = self.definer(operand)
                        && self.simple_key(inner).is_some()
                        && !seen.contains(&inner)
                    {
                        seen.push(inner);
                        work.push(inner);
                    }
                }
            }
        }
        changed
    }

    /// The predecessor of `block` where the only edge into it is one.
    fn single_predecessor(
        &self,
        block: BlockId,
    ) -> Option<BlockId> {
        let mut edges = self
            .function
            .block_users(block)
            .iter()
            .filter(|one| self.function.instruction(one.user).opcode.is_terminator())
            .map(|one| self.function.parent(one.user));
        let first = edges.next()??;
        edges.next().is_none().then_some(first)
    }

    /// Erase now-unneeded `inst` once the walk is over.
    fn remove(
        &mut self,
        inst: InstId,
    ) {
        self.gone.push(inst);
    }

    /// `EarlyCSE::processNode`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1322).
    fn process_node(
        &mut self,
        block: BlockId,
    ) -> bool {
        let mut changed = false;
        // Several predecessors could have invalidated the live-out memory
        // values of the parent.
        let single = self.single_predecessor(block);
        if single.is_none() {
            self.generation += 1;
        }
        // The single predecessor's conditional branch tells the condition.
        if let Some(pred) = single
            && let Some(branch) = self.function.terminator(pred)
        {
            let op = self.function.instruction(branch);
            if op.opcode == Opcode::Br
                && op.operands.len() == 3
                && let Some(cond) = self.definer(op.operands[0])
                && self.simple_key(cond).is_some()
            {
                changed |= self.branch_condition(cond, branch, block, pred);
            }
        }
        // The last non-volatile store seen, for as long as nothing reads
        // memory.
        let mut last_store: Option<InstId> = None;
        for inst in self.function.iter_block(block).collect::<Vec<_>>() {
            if self.function.is_erased(inst) {
                continue;
            }
            // Dead instructions should just be removed.
            if self.dead(inst) {
                self.remove(inst);
                changed = true;
                continue;
            }
            // An assumption makes its condition true for all dominated blocks.
            if matches!(self.function.instruction(inst).opcode, Opcode::Call(_)) && self.named(inst, "llvm.assume") {
                let cond = self.function.instruction(inst).operands[0];
                if let Some(cond) = self.definer(cond)
                    && let Some(key) = self.simple_key(cond)
                {
                    let ty = self.function.instruction(cond).ty;
                    let truth = Operand::Constant(self.context.int(ty, 1));
                    self.values.insert(key, truth);
                }
                continue;
            }
            // If the instruction can be simplified, replace it with its
            // simpler value.
            if let Some(value) = simplify_instruction_in(
                self.context,
                self.view.globals,
                self.view.layout,
                Some(self.analyses),
                self.function,
                inst,
            ) {
                let result = self.function.instruction(inst).result;
                if let Some(result) = result
                    && !self.function.users(result).is_empty()
                {
                    self.function.replace_value(result, value);
                    changed = true;
                }
                if self.dead(inst) {
                    self.remove(inst);
                    changed = true;
                    continue;
                }
            }
            // Stores before a potential unwind are not removed.
            if matches!(
                self.function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            ) {
                last_store = None;
            }
            // A simple instruction that can be value numbered.
            if let Some(key) = self.simple_key(inst) {
                if let Some(&found) = self.values.lookup(&key) {
                    self.combine_flags(inst, found);
                    if let Some(result) = self.function.instruction(inst).result {
                        self.function.replace_value(result, found);
                    }
                    self.remove(inst);
                    changed = true;
                    continue;
                }
                if let Some(result) = self.function.instruction(inst).result {
                    self.values.insert(key, Operand::Value(result));
                }
                continue;
            }
            // A load or a store.
            let pointer = self.pointer(inst);
            if self.is_load(inst) {
                let pointer = pointer.expect("a load has a pointer");
                // We can't peek past the ordering of a volatile load.
                if self.volatile(inst) {
                    last_store = None;
                    self.generation += 1;
                }
                if self.function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "invariant.load") {
                    let ty = self.function.instruction(inst).ty;
                    if self.invariants.lookup(&(pointer, ty)).is_none() {
                        self.invariants.insert((pointer, ty), self.generation);
                    }
                }
                if let Some(in_val) = self.loads.lookup(&pointer).copied()
                    && let Some(value) = self.matching_value(in_val, inst, self.generation)
                {
                    if in_val.is_load {
                        self.combine_metadata(in_val.def, inst);
                    }
                    if let Some(result) = self.function.instruction(inst).result {
                        self.function.replace_value(result, value);
                    }
                    self.remove(inst);
                    changed = true;
                    continue;
                }
                self.loads.insert(pointer, LoadValue { def: inst, generation: self.generation, is_load: true });
                last_store = None;
                continue;
            }
            // An instruction that may read memory forgets the last store.
            if self.may_read(inst) {
                last_store = None;
            }
            // A read-only or write-only call.
            if let Some(key) = self.call_key(inst) {
                if let Some(&(earlier, generation)) = self.calls.lookup(&key)
                    && self.same_generation(generation, self.generation, earlier, inst)
                    && self.may_read(earlier) == self.may_read(inst)
                {
                    if let Some(result) = self.function.instruction(inst).result
                        && let Some(kept) = self.function.instruction(earlier).result
                    {
                        self.function.replace_value(result, Operand::Value(kept));
                    }
                    self.remove(inst);
                    changed = true;
                    continue;
                }
                // The call has the generation after the write.
                if self.may_write(inst) {
                    self.generation += 1;
                }
                self.calls.insert(key, (inst, self.generation));
                continue;
            }
            // GEPs by offset.
            if let Some(key) = self.gep_key(inst) {
                if let Some(&found) = self.geps.lookup(&key) {
                    self.combine_flags(inst, found);
                    if let Some(result) = self.function.instruction(inst).result {
                        self.function.replace_value(result, found);
                    }
                    self.remove(inst);
                    changed = true;
                    continue;
                }
                if let Some(result) = self.function.instruction(inst).result {
                    self.geps.insert(key, Operand::Value(result));
                }
                continue;
            }
            // Write back DSE: a store of the value just loaded from there.
            if self.is_store(inst) {
                let pointer = pointer.expect("a store has a pointer");
                if let Some(in_val) = self.loads.lookup(&pointer).copied()
                    && Some(in_val.def) == self.matching_def(in_val, inst)
                {
                    self.remove(inst);
                    changed = true;
                    continue;
                }
            }
            // Anything else that writes memory ends what was loaded.
            if self.may_write(inst) {
                self.generation += 1;
                if self.is_store(inst) {
                    let pointer = pointer.expect("a store has a pointer");
                    // Two stores to one location with no load between: the
                    // earlier is dead.
                    if let Some(earlier) = last_store
                        && self.overriding_stores(earlier, inst)
                    {
                        self.remove(earlier);
                        changed = true;
                    }
                    // The stored value is a live version of the pointer.
                    self.loads.insert(pointer, LoadValue { def: inst, generation: self.generation, is_load: false });
                    last_store = (!self.volatile(inst)).then_some(inst);
                }
            }
        }
        changed
    }

    /// The instruction `getMatchingValue` finds for the store `mem`, as the
    /// definition it matches: the load the store writes back.
    fn matching_def(
        &mut self,
        in_val: LoadValue,
        mem: InstId,
    ) -> Option<InstId> {
        let value = self.matching_value(in_val, mem, self.generation)?;
        let result = self.function.instruction(in_val.def).result?;
        (value == Operand::Value(result)).then_some(in_val.def)
    }

    /// `combineMetadataForCSE`: what the loads do not share is dropped from the
    /// one that stays.
    fn combine_metadata(
        &mut self,
        kept: InstId,
        removed: InstId,
    ) {
        let theirs = self.function.instruction(removed).metadata.clone();
        let mine = self.function.instruction(kept).metadata.clone();
        for (kind, node) in mine {
            if !theirs.iter().any(|(other, same)| *other == kind && *same == node) {
                self.function.unannotate(kept, &kind);
            }
        }
    }

    /// `EarlyCSE::run`
    /// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1758):
    /// the dominator tree depth first, a scope for each node.
    fn run(&mut self) -> bool {
        let Some(entry) = self.function.entry() else { return false };
        let mut children: llrm_mir::dense::IdMap<BlockId, Vec<BlockId>> = llrm_mir::dense::IdMap::new();
        for &block in self.function.layout() {
            if let Some(parent) = self.tree.immediate_dominator(block) {
                children.get_or_insert_with(parent, Vec::new).push(block);
            }
        }
        struct Frame {
            block: BlockId,
            next: usize,
            current_generation: u32,
            child_generation: u32,
            processed: bool,
        }
        let mut changed = false;
        self.enter();
        let mut stack =
            vec![Frame { block: entry, next: 0, current_generation: 0, child_generation: 0, processed: false }];
        while let Some(frame) = stack.last_mut() {
            self.generation = frame.current_generation;
            if !frame.processed {
                let block = frame.block;
                changed |= self.process_node(block);
                let frame = stack.last_mut().expect("a frame");
                frame.child_generation = self.generation;
                frame.processed = true;
            } else if frame.next < children.get(&frame.block).map_or(0, Vec::len) {
                let child = children.get(&frame.block).expect("it has children")[frame.next];
                frame.next += 1;
                let generation = frame.child_generation;
                self.enter();
                stack.push(Frame {
                    block: child,
                    next: 0,
                    current_generation: generation,
                    child_generation: generation,
                    processed: false,
                });
            } else {
                self.leave();
                stack.pop();
            }
        }
        changed
    }
}

/// `EarlyCSEPass`
/// (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1809)
/// with MemorySSA.
pub struct EarlyCse {
    /// `UseMemorySSA` (`EarlyCSEPass(true)`, EarlyCSE.cpp#L1809): the pass at
    /// `earlycse@649` has it, EarlyFPM's at `earlycse@1180` has none
    /// (PassBuilderPipelines.cpp#L508, #L649, #L1180).
    pub memory_ssa: bool,
}

impl Default for EarlyCse {
    fn default() -> Self {
        Self { memory_ssa: true }
    }
}

impl FunctionPass for EarlyCse {
    fn name(&self) -> &'static str {
        "earlycse"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("early-cse")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let accesses = self.memory_ssa.then(|| std::rc::Rc::new(Accesses::lazy()));
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        // MemorySSA describes the function as it is now; the walk changes
        // `unit.function`, which this copy does not follow. The context needs
        // no copy: the walk only interns constants, which `Context::int` does
        // through `&self`.
        let before = unit.function.fork();
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let view = View::within(unit.context, unit.layout, &before, &outer).with_shape(&shape);
        let simplifier = FunctionAnalyses::of(unit.context, view.globals, view.metadata, view.layout, &before);
        let shape = view.shape();
        // The manager's graph of this body, cloned: the removals below update
        // the clone, which is put back (`MemorySSAUpdater`, as EarlyCSE.cpp
        // keeps MemorySSA through `PA.preserve<MemorySSAAnalysis>()`).
        let (view_of_memory, kept_accesses): (Box<dyn MemorySsaView + '_>, _) = match &accesses {
            Some(accesses) => {
                let held = analyses.get::<MemorySsa>(unit.context, unit.layout, unit.function);
                let (mssa, kept_accesses) = match &*held {
                    Ok(held) => {
                        (held.mssa.clone(), held.accesses(unit.context, unit.layout, unit.function, analyses).ok())
                    }
                    Err(_) => (Mssa::build(&before, &shape.dominance, &shadow::effect_of_in(accesses, &view)), None),
                };
                (Box::new(Translated::new(&view, accesses, mssa)), kept_accesses)
            }
            None => (Box::new(Detached), None),
        };
        let mut walk = Walk {
            context: unit.context,
            function: unit.function,
            view: &view,
            mssa: view_of_memory,
            callees: outer.callees(),
            tree: &tree,
            analyses: &simplifier,
            values: Scoped::new(),
            loads: Scoped::new(),
            invariants: Scoped::new(),
            calls: Scoped::new(),
            geps: Scoped::new(),
            generation: 0,
            clobber_counter: 0,
            gone: Vec::new(),
        };
        let changed = walk.run();
        let gone = std::mem::take(&mut walk.gone);
        let function = walk.function;
        for inst in gone.into_iter().rev() {
            if !function.is_erased(inst) {
                walk.mssa.remove_memory_access(function, inst);
                function.erase(inst).unwrap_or_else(|error| panic!("earlycse: {error}"));
            }
        }
        if !changed {
            return PreservedAnalyses::all();
        }
        let mut preserved = PreservedAnalyses::none().preserve::<Dominators>();
        if let (Some(updated), Some(accesses)) = (walk.mssa.into_mssa(), kept_accesses) {
            analyses.replace::<MemorySsa>(&*function, Ok(Rc::new(MssaGraph::new(updated, Some(accesses)))));
            preserved = preserved.preserve_updated::<MemorySsa>();
        }
        preserved
    }
}

#[cfg(test)]
#[path = "earlycse_tests.rs"]
mod tests;
