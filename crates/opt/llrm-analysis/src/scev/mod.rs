//! ScalarEvolution: a translation of LLVM's `ScalarEvolution.cpp`,
//! `ScalarEvolutionExpressions.h` and their helpers at `d1106deb71cc`.
//!
//! # How it works
//! [`ScalarEvolution`] holds the arena of uniqued [`expr::Node`]s and the
//! caches LLVM keeps in the analysis object; it borrows nothing, so a pass can
//! edit the function between queries and say what it changed (`forget*`), as
//! with LLVM's. Each query is handed a [`Cx`]: the function, its loops and
//! dominator tree, the data layout and the module's globals, which LLVM's
//! object keeps as members.
//!
//! The folds (`getAddExpr`, `getMulExpr`, the casts, ...) are translated
//! function for function, each naming its C++ original, and verified against
//! LLVM's own constructors through `tools/scev/oracle.cpp`.

pub mod analysis;
pub mod ap;
pub mod arith;
pub mod backedge;
pub mod cast;
pub mod compare;
pub mod compare_predicate;
pub mod create;
pub mod create_addrec;
pub mod create_phi;
pub mod delinearize;
#[cfg(test)]
mod delinearize_tests;
pub mod disposition;
pub mod division;
#[cfg(test)]
mod division_tests;
pub mod divrec;
pub mod eval;
pub mod evaluate;
pub mod exit_limit;
pub mod exit_value;
#[cfg(test)]
mod exit_value_tests;
pub mod expand;
pub mod expand_cost;
#[cfg(test)]
mod expand_cost_tests;
pub mod expand_iv;
#[cfg(test)]
mod expand_tests;
pub mod expander;
pub mod expr;
pub mod forget;
#[cfg(test)]
mod golden_tests;
pub mod guards;
#[cfg(test)]
mod guards_tests;
pub mod how_many;
pub mod implied;
pub mod invariant;
pub mod lcssa_form;
pub mod minmax;
pub mod normalize;
pub mod pattern;
pub mod poison;
pub mod predicate;
pub mod predicate_rewrite;
pub mod predicated;
pub mod print;
pub mod quadratic;
pub mod range;
#[cfg(test)]
mod range_factoring_tests;
#[cfg(test)]
mod range_recurrence_tests;
pub mod rewrite;
pub mod scope;
#[cfg(test)]
mod scope_tests;
pub mod seams;
pub mod sequential;
pub mod trip;
#[cfg(test)]
mod trip_tests;
pub mod values;
#[cfg(test)]
mod values_tests;
pub mod verify;
pub mod wrap_predicate;
pub mod wrap_proof;

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::domtree::DomTree;
use llrm_mir::hash::{HashMap, HashSet};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, GlobalValue, MetadataNode, Operand, ValueDef, ValueId};
use llrm_mir::types::Type as MirType;
use llrm_mir::valuetracking::{AssumptionCache, Query};

use self::ap::Ap;
use self::expr::{Kind, Node, Scev, Ty, Type, Use, flags};

/// What LLVM's `ScalarEvolution` holds by reference: the module, function,
/// loops and dominator tree, and the data layout.
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    /// The module's context, globals and metadata, held apart so a pass that
    /// has a `&mut Context` and no `Module` can build one (as `Query` does).
    pub context: &'a Context,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub function: &'a Function,
    pub layout: &'a DataLayout,
    pub loops: &'a LoopInfo,
    pub tree: &'a DomTree,
    /// `AC`: the `llvm.assume` calls of the function.
    pub assumptions: &'a AssumptionCache,
}

impl<'a> Cx<'a> {
    /// The `SimplifyQuery{DL, DT, AC}` the ValueTracking calls of
    /// ScalarEvolution are made with, at `cxt`.
    pub fn query(
        &self,
        cxt: Option<llrm_mir::module::InstId>,
    ) -> Query<'a> {
        Query {
            globals: self.globals,
            metadata: self.metadata,
            dt: Some(self.tree),
            assumptions: Some(self.assumptions),
            ..Query::bare(self.context, self.function, self.layout).with_cxt(cxt)
        }
    }

    /// `Module::global`.
    pub fn global(
        &self,
        id: llrm_mir::context::GlobalId,
    ) -> &'a GlobalValue {
        &self.globals[id.0 as usize]
    }
}

/// The key `UniqueSCEVs` finds a node by (`FoldingSetNodeID`): the kind, the
/// operands as pointers with their use bits, the type, and what else the kind
/// holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Key<'a> {
    pub(super) tag: Type,
    pub(super) ops: std::borrow::Cow<'a, [Use]>,
    pub(super) ty: Option<Ty>,
    pub(super) constant: Option<Ap>,
    pub(super) value: Option<Operand>,
    pub(super) header: Option<BlockId>,
}

impl Key<'_> {
    /// The key that owns its operands, for a node that is made.
    fn into_owned(self) -> Key<'static> {
        llrm_mir::counts::bump(llrm_mir::counts::Event::ScevKeysOwned);
        Key {
            tag: self.tag,
            ops: std::borrow::Cow::Owned(self.ops.into_owned()),
            ty: self.ty,
            constant: self.constant,
            value: self.value,
            header: self.header,
        }
    }
}

/// A key by what it holds, so that `UniqueSCEVs` is asked with operands
/// borrowed from the caller (`FoldingSetNodeID` over a `SmallVector` the
/// caller keeps): a lookup that finds its node builds nothing.
pub(super) trait KeyLike {
    fn key(&self) -> &Key<'_>;
}

impl KeyLike for Key<'_> {
    fn key(&self) -> &Key<'_> {
        self
    }
}

impl<'a> std::borrow::Borrow<dyn KeyLike + 'a> for Key<'static> {
    fn borrow(&self) -> &(dyn KeyLike + 'a) {
        self
    }
}

impl std::hash::Hash for dyn KeyLike + '_ {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        std::hash::Hash::hash(self.key(), state);
    }
}

impl PartialEq for dyn KeyLike + '_ {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.key() == other.key()
    }
}

impl Eq for dyn KeyLike + '_ {}

/// What `FoldingSetNodeID` does: the key goes into the hasher as a few words,
/// the operands one word each, not the fields one write at a time.
impl std::hash::Hash for Key<'_> {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        let ty = match self.ty {
            None => 0,
            Some(Ty::Int(bits)) => 1 | u64::from(bits) << 2,
            Some(Ty::Ptr(space)) => 2 | u64::from(space) << 2,
        };
        state.write_u64(self.tag as u64 | (self.ops.len() as u64) << 8 | ty << 32);
        for op in self.ops.iter() {
            state.write_u64(u64::from(op.scev.0) | u64::from(op.bits) << 32);
        }
        if let Some(constant) = self.constant {
            state.write_u64(constant.bits as u64);
            state.write_u64((constant.bits >> 64) as u64);
        }
        if let Some(value) = self.value {
            std::hash::Hash::hash(&value, state);
        }
        if let Some(header) = self.header {
            state.write_u32(header.0);
        }
    }
}

/// `SCEVFlags`: the no-wrap flags of an expression and of its use.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flags {
    pub expr: u8,
    pub use_: u8,
}

impl Flags {
    /// Expression flags only.
    pub fn expr(expr: u8) -> Self {
        Self { expr, use_: 0 }
    }
}

/// `llvm::ScalarEvolution`.
pub struct ScalarEvolution {
    pub(super) nodes: Vec<Node>,
    /// `UniqueSCEVs`.
    pub(super) unique: HashMap<Key<'static>, Scev>,
    /// `FoldCache`: the extensions already folded.
    pub(super) fold_cache: HashMap<(Type, Use, Ty), Scev>,
    /// `ProvingSplitPredicate`.
    pub(super) proving_split_predicate: bool,
    /// `UnsignedRanges`.
    pub(super) unsigned_ranges: HashMap<Scev, llrm_mir::constantrange::ConstantRange>,
    /// `SignedRanges`.
    pub(super) signed_ranges: HashMap<Scev, llrm_mir::constantrange::ConstantRange>,
    /// `ConstantMultipleCache`.
    pub(super) constant_multiple_cache: HashMap<Scev, Ap>,
    /// `getNotSCEV` of a non-constant: built once per expression (it is
    /// asked 728 times for 426 builds on d_faces, 11 K Ir each).
    pub(super) not_cache: HashMap<Scev, Scev>,
    /// `PredicatedSCEVRewrites`: the recurrence a header phi is under
    /// predicates, by phi and loop; the phi itself where it is none.
    pub(super) predicated_rewrites: HashMap<(Scev, BlockId), (Scev, Vec<wrap_predicate::Predicate>)>,
    /// `LoopDispositions`: by expression, as LLVM's, so that forgetting one
    /// does not scan them all.
    pub(super) loop_dispositions: HashMap<Scev, disposition::Held<Option<BlockId>, disposition::LoopDisposition>>,
    /// `BlockDispositions`.
    pub(super) block_dispositions: HashMap<Scev, disposition::Held<BlockId, disposition::BlockDisposition>>,
    /// `ValueExprMap`: the SCEV of each value that has one. Constants are not
    /// held: their SCEV is a constant node, made again for the price of a
    /// lookup.
    pub(super) value_expr_map: IdMap<ValueId, Scev>,
    /// `ExprValueMap`: the values a SCEV was made for, in insertion order.
    pub(super) expr_value_map: HashMap<Scev, Vec<ValueId>>,
    /// `SCEVUsers`: the expressions that have a SCEV as an operand.
    pub(super) scev_users: HashMap<Scev, HashSet<Scev>>,
    /// `LoopUsers`: the add recurrences of each loop.
    pub(super) loop_users: IdMap<BlockId, Vec<Scev>>,
    /// `UnsignedWrapViaInductionTried` and `SignedWrapViaInductionTried`.
    pub(super) unsigned_wrap_via_induction_tried: HashSet<Scev>,
    pub(super) signed_wrap_via_induction_tried: HashSet<Scev>,
    /// `ValuesAtScopes`: each expression's value in the scope of a loop (none:
    /// the function body); the second is empty while it is being computed.
    pub(super) values_at_scopes: HashMap<Scev, Vec<(Option<BlockId>, Option<Use>)>>,
    /// `ValuesAtScopesUsers`: for each value at scope, the (loop, expression)
    /// pairs it is the value of.
    pub(super) values_at_scopes_users: HashMap<Scev, Vec<(Option<BlockId>, Scev)>>,
    /// A test's hook, run once as a value at scope starts to be computed.
    #[cfg(test)]
    pub(super) compute_hook: Option<fn(&mut ScalarEvolution, &Cx, Scev)>,
    /// `FoldCacheUser`: the `fold_cache` keys each SCEV is the answer of.
    pub(super) fold_cache_user: HashMap<Scev, Vec<(Type, Use, Ty)>>,
    /// `CouldNotCompute`.
    could_not_compute: Scev,
    /// `BackedgeTakenCounts` and `LoopPropertiesCache`.
    pub(super) trips: trip::TripState,
    /// What the guard queries keep between calls.
    pub(super) guards: implied::GuardState,
}

impl Default for ScalarEvolution {
    fn default() -> Self {
        Self::new()
    }
}

impl ScalarEvolution {
    pub fn new() -> Self {
        let mut se = Self {
            nodes: Vec::new(),
            unique: HashMap::with_capacity_and_hasher(256, Default::default()),
            fold_cache: HashMap::default(),
            proving_split_predicate: false,
            unsigned_ranges: HashMap::default(),
            signed_ranges: HashMap::default(),
            constant_multiple_cache: HashMap::default(),
            not_cache: HashMap::default(),
            predicated_rewrites: HashMap::default(),
            loop_dispositions: HashMap::default(),
            block_dispositions: HashMap::default(),
            value_expr_map: IdMap::new(),
            expr_value_map: HashMap::default(),
            scev_users: HashMap::default(),
            loop_users: IdMap::new(),
            unsigned_wrap_via_induction_tried: HashSet::default(),
            signed_wrap_via_induction_tried: HashSet::default(),
            values_at_scopes: HashMap::default(),
            values_at_scopes_users: HashMap::default(),
            #[cfg(test)]
            compute_hook: None,
            fold_cache_user: HashMap::default(),
            could_not_compute: Scev(0),
            trips: trip::TripState::default(),
            guards: implied::GuardState::default(),
        };
        se.nodes.push(Node { kind: Kind::CouldNotCompute, ty: None, size: 0, flags: 0, canonical: Scev(0) });
        se
    }

    pub fn node(
        &self,
        s: Scev,
    ) -> &Node {
        &self.nodes[s.0 as usize]
    }

    /// `getSCEVType`.
    pub fn scev_type(
        &self,
        s: Scev,
    ) -> Type {
        self.node(s).kind.tag()
    }

    /// `getType`.
    pub fn type_of(
        &self,
        s: Scev,
    ) -> Ty {
        self.node(s).ty.expect("a SCEV that has a type")
    }

    /// `isa<SCEVCouldNotCompute>`.
    pub fn is_could_not_compute(
        &self,
        s: Scev,
    ) -> bool {
        matches!(self.node(s).kind, Kind::CouldNotCompute)
    }

    /// `getCouldNotCompute`.
    pub fn get_could_not_compute(&self) -> Scev {
        self.could_not_compute
    }

    /// `ExpressionSize`.
    pub fn expression_size(
        &self,
        s: Scev,
    ) -> u16 {
        self.node(s).size
    }

    /// `SCEV::operands`.
    pub fn operands(
        &self,
        s: Scev,
    ) -> &[Use] {
        match &self.node(s).kind {
            Kind::Constant(_) | Kind::Unknown(_) => &[],
            Kind::Truncate(op) | Kind::ZeroExtend(op) | Kind::SignExtend(op) | Kind::PtrToAddr(op) => {
                std::slice::from_ref(op)
            }
            Kind::Add(ops)
            | Kind::Mul(ops)
            | Kind::AddRec(ops, _)
            | Kind::UMax(ops)
            | Kind::SMax(ops)
            | Kind::UMin(ops)
            | Kind::SMin(ops)
            | Kind::SequentialUMin(ops) => ops,
            Kind::UDiv(..) => {
                // The two operands are stored apart; `UDiv` callers use
                // `udiv_operands`.
                unreachable!("use udiv_operands")
            }
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    /// `SCEVUDivExpr::getLHS` and `getRHS`.
    pub fn udiv_operands(
        &self,
        s: Scev,
    ) -> (Use, Use) {
        match self.node(s).kind {
            Kind::UDiv(l, r) => (l, r),
            _ => unreachable!("not a udiv"),
        }
    }

    /// `SCEVNAryExpr::getNoWrapFlags(Mask)`.
    pub fn get_no_wrap_flags(
        &self,
        s: Scev,
        mask: u8,
    ) -> u8 {
        self.node(s).flags & mask
    }

    /// `SCEVUse::getNoWrapFlags(Mask)`: the use's flags and the node's.
    pub fn use_no_wrap_flags(
        &self,
        u: Use,
        mask: u8,
    ) -> u8 {
        let node = match self.node(u.scev).kind {
            Kind::Add(_) | Kind::Mul(_) | Kind::AddRec(..) => self.node(u.scev).flags,
            _ => 0,
        };
        (node | self.use_flags(u)) & mask
    }

    /// `SCEVUse::getUseNoWrapFlags`: `NW` follows from `NUW` or `NSW`.
    pub fn use_flags(
        &self,
        u: Use,
    ) -> u8 {
        let mut f = u.bits & (flags::NUW | flags::NSW);
        if f != 0 {
            f |= flags::NW;
        }
        f
    }

    /// `SCEVUse(S, Flags)`: only `NUW` and `NSW`, and only those `S` does not
    /// already have.
    pub fn with_use_flags(
        &self,
        s: Scev,
        f: u8,
    ) -> Use {
        let mut f = f;
        if f != 0 {
            debug_assert!(
                matches!(self.node(s).kind, Kind::Add(_) | Kind::Mul(_) | Kind::AddRec(..)),
                "use flags require an expression that can carry no-wrap flags"
            );
            f &= !self.node(s).flags;
        }
        Use { scev: s, bits: f & (flags::NUW | flags::NSW) }
    }

    /// `ScalarEvolution::getTypeSizeInBits`.
    pub fn get_type_size_in_bits(
        &self,
        cx: &Cx,
        ty: Ty,
    ) -> u32 {
        match ty {
            Ty::Int(bits) => bits,
            Ty::Ptr(space) => cx.layout.pointer(space).index_bits,
        }
    }

    /// `ScalarEvolution::isSCEVable`: an integer, or a pointer.
    pub fn is_scevable(
        &self,
        ty: &MirType,
    ) -> bool {
        match ty {
            MirType::Int(bits) => (1..=128).contains(bits),
            MirType::Pointer(_) => true,
            _ => false,
        }
    }

    /// `ScalarEvolution::getEffectiveSCEVType`: a pointer is the integer of
    /// its width.
    pub fn get_effective_scev_type(
        &self,
        cx: &Cx,
        ty: Ty,
    ) -> Ty {
        match ty {
            Ty::Int(_) => ty,
            Ty::Ptr(space) => Ty::Int(cx.layout.pointer(space).index_bits),
        }
    }

    /// `ScalarEvolution::getWiderType`.
    pub fn get_wider_type(
        &self,
        cx: &Cx,
        a: Ty,
        b: Ty,
    ) -> Ty {
        if self.get_type_size_in_bits(cx, a) > self.get_type_size_in_bits(cx, b) { a } else { b }
    }

    /// The SCEV type of an IR type.
    pub fn ty_of(
        &self,
        cx: &Cx,
        id: llrm_mir::types::TypeId,
    ) -> Option<Ty> {
        match cx.context.types.get(id) {
            MirType::Int(bits) if (1..=128).contains(bits) => Some(Ty::Int(*bits)),
            MirType::Pointer(space) => Some(Ty::Ptr(*space)),
            _ => None,
        }
    }

    /// The type of the value an unknown names.
    fn operand_ty(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> Ty {
        let id = match v {
            Operand::Value(value) => cx.function.value(value).ty,
            Operand::Constant(constant) => cx.context.get(constant).ty,
            Operand::Block(_) => unreachable!("a block is not a SCEVable value"),
        };
        self.ty_of(cx, id).expect("a SCEVable value")
    }

    /// Interns `kind`, with `size`, computing the canonical form. The one
    /// place a node is made (`new (SCEVAllocator) ...; UniqueSCEVs.insert`).
    pub(super) fn intern(
        &mut self,
        key: Key<'_>,
        kind: impl FnOnce() -> Kind,
        ty: Option<Ty>,
        size: u16,
    ) -> Scev {
        let at = Scev(self.nodes.len() as u32);
        // The find borrows the key's operands; a node that is made owns them.
        if let Some(&found) = self.unique.get(&key as &dyn KeyLike) {
            return found;
        }
        self.unique.insert(key.into_owned(), at);
        self.nodes.push(Node { kind: kind(), ty, size, flags: 0, canonical: at });
        // `LoopUsers[L].push_back(S)` and `registerUser(S, Ops)`, made where
        // each constructor makes a node.
        if let Kind::AddRec(_, header) = self.nodes[at.0 as usize].kind {
            self.loop_users.get_or_insert_with(header, Vec::new).push(at);
        }
        self.register_user(at);
        at
    }

    /// `computeExpressionSize`.
    pub(super) fn compute_expression_size(
        &self,
        ops: &[Use],
    ) -> u16 {
        ops.iter().fold(1u16, |size, op| size.saturating_add(self.expression_size(op.scev)))
    }

    /// `ScalarEvolution::getConstant(const APInt &)`.
    pub fn get_constant(
        &mut self,
        value: Ap,
    ) -> Scev {
        let key = Key {
            tag: Type::Constant,
            ops: std::borrow::Cow::Borrowed(&[]),
            ty: Some(Ty::Int(value.width)),
            constant: Some(value),
            value: None,
            header: None,
        };
        self.intern(key, || Kind::Constant(value), Some(Ty::Int(value.width)), 1)
    }

    /// `ScalarEvolution::getConstant(Type *, uint64_t, bool)`.
    pub fn get_constant_of_type(
        &mut self,
        cx: &Cx,
        ty: Ty,
        value: i128,
    ) -> Scev {
        let Ty::Int(bits) = self.get_effective_scev_type(cx, ty) else { unreachable!("an integer") };
        self.get_constant(Ap::from_signed(bits, value))
    }

    /// `getZero(Ty)`.
    pub fn get_zero(
        &mut self,
        cx: &Cx,
        ty: Ty,
    ) -> Scev {
        self.get_constant_of_type(cx, ty, 0)
    }

    /// `getOne(Ty)`.
    pub fn get_one(
        &mut self,
        cx: &Cx,
        ty: Ty,
    ) -> Scev {
        self.get_constant_of_type(cx, ty, 1)
    }

    /// `ScalarEvolution::getUnknown`.
    pub fn get_unknown(
        &mut self,
        cx: &Cx,
        v: Operand,
    ) -> Scev {
        let ty = self.operand_ty(cx, v);
        let key = Key {
            tag: Type::Unknown,
            ops: std::borrow::Cow::Borrowed(&[]),
            ty: Some(ty),
            constant: None,
            value: Some(v),
            header: None,
        };
        self.intern(key, || Kind::Unknown(v), Some(ty), 1)
    }

    /// The constant `s` is, if it is one.
    pub fn as_constant(
        &self,
        s: Scev,
    ) -> Option<Ap> {
        match self.node(s).kind {
            Kind::Constant(value) => Some(value),
            _ => None,
        }
    }

    /// The header the loop of an add recurrence has.
    pub fn addrec_header(
        &self,
        s: Scev,
    ) -> Option<BlockId> {
        match self.node(s).kind {
            Kind::AddRec(_, header) => Some(header),
            _ => None,
        }
    }

    /// Whether `s` is `ValueDef::Instruction`-defined unknown.
    pub fn unknown_value(
        &self,
        s: Scev,
    ) -> Option<Operand> {
        match self.node(s).kind {
            Kind::Unknown(v) => Some(v),
            _ => None,
        }
    }
}

/// `Value::getValueID` for the values `CompareValueComplexity` orders: the
/// constants, then the arguments, then the instructions by LLVM's opcode
/// number (`InstructionVal + Opcode`). Only the order is read.
pub fn value_id(
    cx: &Cx,
    v: Operand,
) -> u32 {
    use llrm_mir::context::ConstantKind as K;
    use llrm_mir::module::GlobalKind;
    match v {
        Operand::Constant(id) => match &cx.context.get(id).kind {
            K::Undef => 0,
            K::Poison => 1,
            K::Zero => 2,
            K::Bytes(_) => 3,
            K::Int(_) => 5,
            K::Float(_) | K::Float80(_) => 7,
            K::Null => 9,
            K::Aggregate(_) => match cx.context.types.get(cx.context.get(id).ty) {
                MirType::Array { .. } => 11,
                MirType::Vector { .. } => 13,
                _ => 12,
            },
            K::Global(global) => match cx.global(*global).kind {
                GlobalKind::Function(_) => 14,
                _ => 17,
            },
            K::Expr(_) => 19,
        },
        Operand::Value(id) => match cx.function.value(id).def {
            ValueDef::Argument(_) => 30,
            ValueDef::Instruction(inst) => INSTRUCTION_VAL + llvm_opcode(cx.function, inst),
        },
        Operand::Block(_) => 31,
    }
}

const INSTRUCTION_VAL: u32 = 100;

/// `Instruction.def`'s opcode numbers. `br` is `UncondBr` or `CondBr` there;
/// MIR's `FPRound` is an `fptrunc` to the same type.
fn llvm_opcode(
    function: &Function,
    inst: llrm_mir::module::InstId,
) -> u32 {
    use llrm_mir::opcode::{BinaryOp as B, CastOp as C, Opcode as O};
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        O::Ret => 1,
        O::Br => {
            if instruction.operands.len() == 1 {
                2
            } else {
                3
            }
        }
        O::Switch => 4,
        O::Invoke(_) => 6,
        O::Resume => 7,
        O::Unreachable => 8,
        O::FNeg => 13,
        O::Binary(op) => match op {
            B::Add => 14,
            B::FAdd => 15,
            B::Sub => 16,
            B::Mul => 18,
            B::UDiv => 20,
            B::SDiv => 21,
            B::FDiv => 22,
            B::URem => 23,
            B::SRem => 24,
            B::FRem => 25,
            B::Shl => 26,
            B::LShr => 27,
            B::AShr => 28,
            B::And => 29,
            B::Or => 30,
            B::Xor => 31,
            B::FSub => 17,
            B::FMul => 19,
        },
        O::Alloca { .. } => 32,
        O::Load { .. } => 33,
        O::Store { .. } => 34,
        // A load or store atomic is the load or store opcode in LLVM.
        O::Atomic(info) => match info.kind {
            llrm_mir::opcode::AtomicKind::Load => 33,
            llrm_mir::opcode::AtomicKind::Store => 34,
            llrm_mir::opcode::AtomicKind::Fence => 36,
            llrm_mir::opcode::AtomicKind::CmpXchg { .. } => 37,
            llrm_mir::opcode::AtomicKind::Rmw(_) => 38,
        },
        O::GetElementPtr { .. } => 35,
        O::Cast(op) => match op {
            C::Trunc => 39,
            C::ZExt => 40,
            C::SExt => 41,
            C::FPToUI => 42,
            C::FPToSI => 43,
            C::UIToFP => 44,
            C::SIToFP => 45,
            C::FPTrunc | C::FPRound => 46,
            C::FPExt => 47,
            C::PtrToInt => 48,
            C::IntToPtr => 50,
            C::BitCast => 51,
            #[allow(unreachable_patterns)]
            _ => 51,
        },
        O::ICmp(_) => 55,
        O::FCmp(_) => 56,
        O::Phi => 57,
        O::Call(_) => 58,
        O::Select => 59,
        O::ExtractValue(_) => 66,
        O::InsertValue(_) => 67,
        O::LandingPad { .. } => 68,
        O::Freeze => 69,
    }
}
