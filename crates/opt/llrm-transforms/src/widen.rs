//! A loop counter read through `sext` or `zext` becomes a counter of the wider
//! type, and the extensions go. LLVM's `WidenIV` (`SimplifyIndVar.cpp`).
//!
//! # How it works
//! `WidenIv::create_wide_iv` takes a header phi `i` that ScalarEvolution models
//! as an add recurrence and that some `sext`/`zext` in the loop widens to a
//! legal type. It expands the extended recurrence (`sext {0,+,1}` is
//! `{0,+,1}` of the wide type when the narrow counter does not wrap) as a new
//! header phi, then walks the users of `i` with a worklist:
//! - a user that is itself an extension is replaced by the wide value;
//! - a user whose widened form is an add recurrence of the loop (an add, sub or
//!   mul of the counter and an operand whose extension keeps it a recurrence)
//!   is cloned in the wide type and followed;
//! - a compare is rewritten on the wide counter (`widenLoopCompare`);
//! - an add, sub or mul that only extended users read is widened in place
//!   (`widenWithVariantUse`);
//! - any other user reads a `trunc` of the wide counter, which isolates the
//!   narrow phi so `dead` removes it.
//!
//! A narrow definition known not to be negative may use either extension.
//! Where an increment `x + c` (nsw, c >= 0) is guarded by a compare of `x`
//! inside the loop, the range that compare leaves the increment is kept per
//! use (`calculatePostIncRanges`), so a use below the guard proves
//! non-negative.
//!
//! # Example
//! `for (short i = 0; i < n; i++) a[i]` reads `sext i` to the index width on
//! each trip. Widened, the loop counts `i` as a wide recurrence, the `sext`
//! is replaced by it, and the compare is on the wide counter.
//!
//! # Used by
//! [`crate::simplifyindvar`] (`simplifyAndExtend`), from [`crate::indvars`].
//!
//! # See also
//! LLVM `WidenIV`, `createWideIV`, `visitIVCast`;
//! `docs/architecture/scev-port.md`.

use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::{Kind, Scev, Ty, Use};
use llrm_analysis::scev::{Cx, Flags as ScevFlags, ScalarEvolution};
use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::TypeId;

/// `ExtendKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExtendKind {
    Zero,
    Sign,
    Unknown,
}

/// `WideIVInfo`: the narrow counter, the widest legal type some extension of it
/// reaches, and whether a `sext` was seen before a `zext`.
#[derive(Clone, Copy, Debug)]
pub struct WideIvInfo {
    pub narrow_iv: InstId,
    pub widest_native_type: Option<Ty>,
    pub is_signed: bool,
}

/// `NarrowIVDefUse`: a link in the narrow counter's def-use chain, with the
/// wide value that computes the same as the narrow def.
#[derive(Clone, Copy, Debug)]
struct NarrowIvDefUse {
    narrow_def: InstId,
    narrow_use: InstId,
    wide_def: InstId,
    /// True if the narrow def is never negative: a sign extension then does
    /// for a zero extension and the other way round.
    never_negative: bool,
}

/// What the translation edits: the function and the analyses over it. The
/// control flow stays as it was, so `env`'s tree and loops stay valid.
pub struct Edit<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub function: &'a mut Function,
    pub declared: &'a mut Declared,
    pub outer: &'a llrm_mir::passes::Outer,
    pub se: &'a mut ScalarEvolution,
    pub env: Env<'a>,
    pub rewriter: &'a mut Expander,
    /// `DeadInsts`.
    pub dead: &'a mut Vec<InstId>,
}

/// The `Cx` over the edit's function as it stands.
macro_rules! cx {
    ($e:expr) => {
        $e.env.cx(&*$e.context, &*$e.function)
    };
}

/// `WidenedRecTy`.
type WidenedRec = (Option<Scev>, ExtendKind);

/// An interesting integer binary operation for `getExtendedOperandRecurrence`:
/// a `shl` treated as a multiply, an `or disjoint` as `add nsw nuw`
/// (`struct BinaryOp`).
struct IvBinaryOp {
    opcode: BinaryOp,
    operands: [Operand; 2],
    is_nsw: bool,
    is_nuw: bool,
}

/// `WidenIV`.
pub struct WidenIv {
    orig_phi: InstId,
    wide_type: Ty,
    /// The header of the loop.
    l: BlockId,
    has_guards: bool,
    use_post_increment_ranges: bool,
    pub num_elim_ext: u32,
    pub num_widened: u32,
    wide_phi: Option<InstId>,
    wide_inc: Option<InstId>,
    wide_inc_expr: Option<Scev>,
    widened: IdSet<InstId>,
    /// `ExtendKindMap`.
    extend_kind_map: IdMap<InstId, ExtendKind>,
    /// `PostIncRangeInfos`: the key is an IV def and a use of it denoting the
    /// context.
    post_inc_range_infos: IdMap<InstId, Vec<(InstId, ConstantRange)>>,
    narrow_iv_users: Vec<NarrowIvDefUse>,
}

pub(crate) fn result_of(
    function: &Function,
    inst: InstId,
) -> ValueId {
    function.instruction(inst).result.expect("an instruction with a value")
}

pub(crate) fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

pub(crate) fn operand_type(
    e: &Edit,
    operand: Operand,
) -> TypeId {
    match operand {
        Operand::Value(value) => e.function.value(value).ty,
        Operand::Constant(constant) => e.context.get(constant).ty,
        Operand::Block(_) => unreachable!("a block is not a value"),
    }
}

fn int_constant(
    e: &Edit,
    operand: Operand,
) -> Option<(u32, u128)> {
    let Operand::Constant(constant) = operand else { return None };
    let found = e.context.get(constant);
    let ConstantKind::Int(value) = found.kind else { return None };
    Some((e.context.types.int_bits(found.ty)?, value))
}

fn ir_ty(
    e: &mut Edit,
    ty: Ty,
) -> TypeId {
    match ty {
        Ty::Int(bits) => e.context.types.int(bits),
        Ty::Ptr(space) => e.context.types.ptr(space),
    }
}

/// `Builder.Create<op>(V, Ty)` before `before`: a constant folds, a value of
/// the type already is itself.
pub(crate) fn build_cast(
    e: &mut Edit,
    op: CastOp,
    value: Operand,
    to: TypeId,
    flags: Flags,
    before: InstId,
) -> Operand {
    if operand_type(e, value) == to {
        return value;
    }
    if let Operand::Constant(constant) = value {
        if let Some(folded) =
            llrm_mir::constantfolding::constant_fold_cast_operand(e.context, e.layout, op, constant, to)
        {
            return Operand::Constant(folded);
        }
    }
    let inst = e.function.create_instruction(Opcode::Cast(op), to, vec![value], flags, None);
    e.function.insert(inst, Position::Before(before)).expect("a placed instruction");
    Operand::Value(result_of(e.function, inst))
}

/// `BinaryOperator::Create(Opcode, LHS, RHS)` inserted before `before`, with
/// the flags of `like` (`copyIRFlags`).
fn build_binary_like(
    e: &mut Edit,
    like: InstId,
    lhs: Operand,
    rhs: Operand,
    before: InstId,
) -> InstId {
    let narrow = e.function.instruction(like);
    let (opcode, ty, flags) = (narrow.opcode.clone(), narrow.ty, narrow.flags);
    let ty = if operand_type(e, lhs) == ty { ty } else { operand_type(e, lhs) };
    let inst = e.function.create_instruction(opcode, ty, vec![lhs, rhs], flags, None);
    e.function.insert(inst, Position::Before(before)).expect("a placed instruction");
    inst
}

/// `DominatorTree::findNearestCommonDominator(BB1, BB2)`.
fn nearest_common_dominator(
    cx: &Cx,
    a: BlockId,
    b: BlockId,
) -> BlockId {
    cx.tree.find_nearest_common_dominator(cx.function, a, b).expect("a forward tree has a common dominator")
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1115
/// `getInsertPointForUses`: the point before which `user`'s reading of `def`
/// can be written. By default immediately before the user. For a phi there may
/// be several uses, so the nearest common dominator of the incoming blocks.
/// None where no viable location is found: `def` reaches `user` only from
/// unreachable blocks.
fn get_insert_point_for_uses(
    e: &Edit,
    user: InstId,
    def: Operand,
) -> Option<InstId> {
    let cx = cx!(e);
    if cx.function.instruction(user).opcode != Opcode::Phi {
        return Some(user);
    }
    let mut insert_pt: Option<InstId> = None;
    for (value, from) in llrm_analysis::occurrence::arms(cx.function, user) {
        if value != def {
            continue;
        }
        // The insert block is the incoming block.
        if !cx.tree.is_reachable_from_entry(from) {
            continue;
        }
        insert_pt = Some(match insert_pt {
            None => cx.function.terminator(from).expect("a terminated block"),
            Some(at) => {
                let common = nearest_common_dominator(&cx, cx.function.parent(at).expect("placed"), from);
                cx.function.terminator(common).expect("a terminated block")
            }
        });
    }
    // If we have skipped all inputs, it means that Def only comes to Phi from
    // unreachable blocks.
    let insert_pt = insert_pt?;
    let Some(def_inst) = defining(cx.function, def) else { return Some(insert_pt) };
    debug_assert!(cx.tree.instruction_dominates(cx.function, def_inst, insert_pt), "def does not dominate all uses");
    let def_loop = cx.loops.get_loop_for(cx.function.parent(def_inst).expect("placed")).map(|one| one.id());
    let mut at = cx.function.parent(insert_pt);
    while let Some(block) = at {
        if cx.loops.get_loop_for(block).map(|one| one.id()) == def_loop {
            return cx.function.terminator(block);
        }
        at = cx.tree.immediate_dominator(block);
    }
    unreachable!("DefI dominates InsertPt!");
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1389
/// `matchBinaryOp`.
fn match_binary_op(
    e: &mut Edit,
    inst: InstId,
) -> Option<IvBinaryOp> {
    let instruction = e.function.instruction(inst);
    let Opcode::Binary(opcode) = instruction.opcode else { return None };
    let (lhs, rhs) = (instruction.operands[0], instruction.operands[1]);
    let flags = instruction.flags;
    match opcode {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => Some(IvBinaryOp {
            opcode,
            operands: [lhs, rhs],
            is_nsw: flags.contains(Flags::NSW),
            is_nuw: flags.contains(Flags::NUW),
        }),
        // Convert or disjoint into add nuw nsw.
        BinaryOp::Or if flags.contains(Flags::DISJOINT) => {
            Some(IvBinaryOp { opcode: BinaryOp::Add, operands: [lhs, rhs], is_nsw: true, is_nuw: true })
        }
        BinaryOp::Shl => {
            let (bit_width, amount) = int_constant(e, rhs)?;
            // If the shift count is not less than the bitwidth, the result of
            // the shift is undefined. Don't try to analyze it, because the
            // resolution chosen here may differ from the resolution chosen in
            // other parts of the compiler.
            if amount >= u128::from(bit_width) {
                return None;
            }
            // We can safely preserve the nuw flag in all cases. It's also safe
            // to turn a nuw nsw shl into a nuw nsw mul. However, nsw in
            // isolation requires special handling. It can be preserved as long
            // as we're not left shifting by bitwidth - 1.
            let is_nuw = flags.contains(Flags::NUW);
            let is_nsw = flags.contains(Flags::NSW) && (is_nuw || amount < u128::from(bit_width - 1));
            let ty = operand_type(e, rhs);
            let one_bit =
                e.context.constant(llrm_mir::context::Constant { ty, kind: ConstantKind::Int(1u128 << amount) });
            Some(IvBinaryOp { opcode: BinaryOp::Mul, operands: [lhs, Operand::Constant(one_bit)], is_nsw, is_nuw })
        }
        _ => None,
    }
}

/// Whether a `w`-bit pattern is non-negative as a signed value.
fn is_non_negative(
    value: u128,
    width: u32,
) -> bool {
    value >> (width - 1) & 1 == 0
}

fn is_signed_predicate(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn is_unsigned_predicate(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

/// `replaceUsesOfWith`: every operand of `user` that is `old` reads `new`.
fn replace_uses_of_with(
    e: &mut Edit,
    user: InstId,
    old: Operand,
    new: Operand,
) {
    let slots: Vec<usize> = e
        .function
        .instruction(user)
        .operands
        .iter()
        .enumerate()
        .filter(|(_, operand)| **operand == old)
        .map(|(at, _)| at)
        .collect();
    for at in slots {
        e.function.set_operand(user, at, new);
    }
}

pub(crate) fn value_op(
    e: &Edit,
    inst: InstId,
) -> Operand {
    Operand::Value(result_of(e.function, inst))
}

impl WidenIv {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1237
    /// `WidenIV::WidenIV`.
    pub fn new(
        e: &Edit,
        wi: &WideIvInfo,
        has_guards: bool,
        use_post_increment_ranges: bool,
    ) -> Self {
        let l = e.function.parent(wi.narrow_iv).expect("a placed phi");
        let mut extend_kind_map = IdMap::default();
        extend_kind_map.insert(wi.narrow_iv, if wi.is_signed { ExtendKind::Sign } else { ExtendKind::Zero });
        Self {
            orig_phi: wi.narrow_iv,
            wide_type: wi.widest_native_type.expect("a type to widen to"),
            l,
            has_guards,
            use_post_increment_ranges,
            num_elim_ext: 0,
            num_widened: 0,
            wide_phi: None,
            wide_inc: None,
            wide_inc_expr: None,
            widened: IdSet::new(),
            extend_kind_map,
            post_inc_range_infos: IdMap::default(),
            narrow_iv_users: Vec::new(),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1389
    /// `getExtendKind`.
    fn get_extend_kind(
        &self,
        inst: InstId,
    ) -> ExtendKind {
        *self.extend_kind_map.get(&inst).expect("Instruction not yet extended!")
    }

    /// Whether `s` is an add recurrence of this loop.
    fn is_add_rec_of_loop(
        &self,
        e: &Edit,
        s: Scev,
    ) -> bool {
        matches!(e.se.node(s).kind, Kind::AddRec(_, header) if header == self.l)
    }

    fn wide_ir_type(
        &self,
        e: &mut Edit,
    ) -> TypeId {
        ir_ty(e, self.wide_type)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1248
    /// `createExtendInst`: the extension of `narrow_oper` hoisted into the
    /// loop preheaders as far as possible.
    fn create_extend_inst(
        &self,
        e: &mut Edit,
        narrow_oper: Operand,
        is_signed: bool,
        use_: InstId,
    ) -> Operand {
        let wide = self.wide_ir_type(e);
        let mut at = use_;
        {
            let cx = cx!(e);
            let mut lp = cx.function.parent(use_).and_then(|block| cx.loops.get_loop_for(block));
            while let Some(found) = lp {
                let Some(preheader) = found.get_loop_preheader(cx.function) else { break };
                if !found.is_loop_invariant(cx.function, narrow_oper) {
                    break;
                }
                at = cx.function.terminator(preheader).expect("a terminated preheader");
                lp = found.get_parent_loop();
            }
        }
        build_cast(e, if is_signed { CastOp::SExt } else { CastOp::ZExt }, narrow_oper, wide, Flags::default(), at)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1265
    /// `cloneIVUser`: a wide operation to replace a narrow one; none for an
    /// operation not cloned.
    fn clone_iv_user(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
        wide_ar: Scev,
    ) -> Option<InstId> {
        let Opcode::Binary(opcode) = e.function.instruction(du.narrow_use).opcode else { return None };
        match opcode {
            BinaryOp::Add | BinaryOp::Mul | BinaryOp::UDiv | BinaryOp::Sub => {
                self.clone_arithmetic_iv_user(e, du, wide_ar)
            }
            BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => {
                Some(self.clone_bitwise_iv_user(e, du))
            }
            _ => None,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1287
    /// `cloneBitwiseIVUser`.
    fn clone_bitwise_iv_user(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> InstId {
        let (narrow_use, narrow_def, wide_def) = (du.narrow_use, du.narrow_def, du.wide_def);
        // Replace NarrowDef operands with WideDef. Otherwise, we don't know
        // anything about the narrow operand yet so must insert a [sz]ext. It is
        // probably loop invariant and will be folded or hoisted. If it actually
        // comes from a widened IV, it should be removed during a future call
        // to widenIVUse.
        let is_signed = self.get_extend_kind(narrow_def) == ExtendKind::Sign;
        let (narrow_def_op, wide_def_op) = (value_op(e, narrow_def), value_op(e, wide_def));
        let operands = e.function.instruction(narrow_use).operands.clone();
        let lhs = if operands[0] == narrow_def_op {
            wide_def_op
        } else {
            self.create_extend_inst(e, operands[0], is_signed, narrow_use)
        };
        let rhs = if operands[1] == narrow_def_op {
            wide_def_op
        } else {
            self.create_extend_inst(e, operands[1], is_signed, narrow_use)
        };
        build_binary_like(e, narrow_use, lhs, rhs, narrow_use)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1317
    /// `cloneArithmeticIVUser`.
    fn clone_arithmetic_iv_user(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
        wide_ar: Scev,
    ) -> Option<InstId> {
        let (narrow_use, narrow_def, wide_def) = (du.narrow_use, du.narrow_def, du.wide_def);
        let (narrow_def_op, wide_def_op) = (value_op(e, narrow_def), value_op(e, wide_def));
        let operands = e.function.instruction(narrow_use).operands.clone();
        let Opcode::Binary(op_code) = e.function.instruction(narrow_use).opcode else { unreachable!("a binary op") };
        let iv_op_idx = if operands[0] == narrow_def_op { 0 } else { 1 };

        // We're trying to find X such that
        //
        //  Widen(NarrowDef `op` NonIVNarrowDef) == WideAR == WideDef `op.wide`
        // X
        //
        // We guess two solutions to X, sext(NonIVNarrowDef) and
        // zext(NonIVNarrowDef), and check using SCEV if any of them are
        // correct.
        let guess_non_iv_operand = |e: &mut Edit, sign_ext: bool| -> bool {
            let cx = cx!(e);
            let (wide_lhs, wide_rhs);
            if iv_op_idx == 0 {
                wide_lhs = e.se.get_scev(&cx, wide_def_op);
                let narrow_rhs = e.se.get_scev(&cx, operands[1]);
                wide_rhs = self.get_extend(&cx, e.se, sign_ext, narrow_rhs);
            } else {
                let narrow_lhs = e.se.get_scev(&cx, operands[0]);
                wide_lhs = self.get_extend(&cx, e.se, sign_ext, narrow_lhs);
                wide_rhs = e.se.get_scev(&cx, wide_def_op);
            }
            // WideUse is "WideDef `op.wide` X" as described in the comment.
            let wide_use = self.get_scev_by_op_code(&cx, e.se, wide_lhs, wide_rhs, op_code);
            wide_use == wide_ar
        };

        let mut sign_extend = self.get_extend_kind(narrow_def) == ExtendKind::Sign;
        if !guess_non_iv_operand(e, sign_extend) {
            sign_extend = !sign_extend;
            if !guess_non_iv_operand(e, sign_extend) {
                return None;
            }
        }

        let lhs = if operands[0] == narrow_def_op {
            wide_def_op
        } else {
            self.create_extend_inst(e, operands[0], sign_extend, narrow_use)
        };
        let rhs = if operands[1] == narrow_def_op {
            wide_def_op
        } else {
            self.create_extend_inst(e, operands[1], sign_extend, narrow_use)
        };
        Some(build_binary_like(e, narrow_use, lhs, rhs, narrow_use))
    }

    /// `SE->getSignExtendExpr(S, WideType)` or the zero extension.
    fn get_extend(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        sign_ext: bool,
        s: Scev,
    ) -> Scev {
        if sign_ext {
            se.get_sign_extend_expr(cx, Use::of(s), self.wide_type, 0)
        } else {
            se.get_zero_extend_expr(cx, Use::of(s), self.wide_type, 0)
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1395
    /// `getSCEVByOpCode`.
    fn get_scev_by_op_code(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        lhs: Scev,
        rhs: Scev,
        op_code: BinaryOp,
    ) -> Scev {
        match op_code {
            BinaryOp::Add => se.get_add_expr(cx, vec![Use::of(lhs), Use::of(rhs)], ScevFlags::default(), 0).scev,
            BinaryOp::Sub => se.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), 0, 0),
            BinaryOp::Mul => se.get_mul_expr(cx, vec![Use::of(lhs), Use::of(rhs)], ScevFlags::default(), 0).scev,
            BinaryOp::UDiv => se.get_udiv_expr(cx, Use::of(lhs), Use::of(rhs)),
            _ => unreachable!("Unsupported opcode."),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1487
    /// `getExtendedOperandRecurrence`: no-wrap operations can transfer sign
    /// extension of their result to their operands. The SCEV of the widened
    /// operation without modifying the IR yet; if the expression after
    /// extending the operands is an AddRec for this loop, the AddRec and the
    /// kind of extension used.
    fn get_extended_operand_recurrence(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> WidenedRec {
        let Some(op) = match_binary_op(e, du.narrow_use) else { return (None, ExtendKind::Unknown) };
        debug_assert!(matches!(op.opcode, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul), "Unexpected opcode");

        // One operand (NarrowDef) has already been extended to WideDef. Now
        // determine if extending the other will lead to a recurrence.
        let narrow_def_op = value_op(e, du.narrow_def);
        let extend_oper_idx = if op.operands[0] == narrow_def_op { 1 } else { 0 };
        debug_assert!(op.operands[1 - extend_oper_idx] == narrow_def_op, "bad DU");

        let mut ext_kind = self.get_extend_kind(du.narrow_def);
        if !(ext_kind == ExtendKind::Sign && op.is_nsw) && !(ext_kind == ExtendKind::Zero && op.is_nuw) {
            ext_kind = ExtendKind::Unknown;

            // For a non-negative NarrowDef, we can choose either type of
            // extension. We want to use the current extend kind if legal (see
            // above), and we only hit this code if we need to check the
            // opposite case.
            if du.never_negative {
                if op.is_nsw {
                    ext_kind = ExtendKind::Sign;
                } else if op.is_nuw {
                    ext_kind = ExtendKind::Zero;
                }
            }
        }

        let cx = cx!(e);
        let mut extend_oper_expr = e.se.get_scev(&cx, op.operands[extend_oper_idx]);
        match ext_kind {
            ExtendKind::Sign => extend_oper_expr = self.get_extend(&cx, e.se, true, extend_oper_expr),
            ExtendKind::Zero => extend_oper_expr = self.get_extend(&cx, e.se, false, extend_oper_expr),
            ExtendKind::Unknown => return (None, ExtendKind::Unknown),
        }

        // When creating this SCEV expr, don't apply the current operations NSW
        // or NUW flags. This instruction may be guarded by control flow that
        // the no-wrap behavior depends on. Non-control-equivalent instructions
        // can be mapped to the same SCEV expression, and it would be incorrect
        // to transfer NSW/NUW semantics to those operations.
        let wide_def_op = value_op(e, du.wide_def);
        let mut lhs = e.se.get_scev(&cx, wide_def_op);
        let mut rhs = extend_oper_expr;

        // Let's swap operands to the initial order for the case of
        // non-commutative operations, like SUB. See PR21014.
        if extend_oper_idx == 0 {
            std::mem::swap(&mut lhs, &mut rhs);
        }
        let add_rec = self.get_scev_by_op_code(&cx, e.se, lhs, rhs, op.opcode);
        if !self.is_add_rec_of_loop(e, add_rec) {
            return (None, ExtendKind::Unknown);
        }
        (Some(add_rec), ext_kind)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1554
    /// `getWideRecurrence`: is this instruction potentially interesting for
    /// further simplification after widening it's type? In other words, can
    /// the extend be safely hoisted out of the loop with SCEV reducing the
    /// value to a recurrence on the same loop. If so, the extended recurrence
    /// and the kind of extension used.
    fn get_wide_recurrence(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> WidenedRec {
        // A user with no value (a store, a branch) is not an integer.
        let Some(narrow_use_result) = e.function.instruction(du.narrow_use).result else {
            return (None, ExtendKind::Unknown);
        };
        let narrow_use_op = Operand::Value(narrow_use_result);
        let narrow_ty = operand_type(e, narrow_use_op);
        if e.context.types.int_bits(narrow_ty).is_none() {
            return (None, ExtendKind::Unknown);
        }
        let cx = cx!(e);
        let narrow_expr = e.se.get_scev(&cx, narrow_use_op);
        let narrow_ty = e.se.type_of(narrow_expr);
        if e.se.get_type_size_in_bits(&cx, narrow_ty) >= e.se.get_type_size_in_bits(&cx, self.wide_type) {
            // NarrowUse implicitly widens its operand. e.g. a gep with a
            // narrow index. So don't follow this use.
            return (None, ExtendKind::Unknown);
        }

        let wide_expr;
        let ext_kind;
        if du.never_negative {
            let signed = self.get_extend(&cx, e.se, true, narrow_expr);
            if self.is_add_rec_any(e, signed) {
                wide_expr = signed;
                ext_kind = ExtendKind::Sign;
            } else {
                wide_expr = self.get_extend(&cx, e.se, false, narrow_expr);
                ext_kind = ExtendKind::Zero;
            }
        } else if self.get_extend_kind(du.narrow_def) == ExtendKind::Sign {
            wide_expr = self.get_extend(&cx, e.se, true, narrow_expr);
            ext_kind = ExtendKind::Sign;
        } else {
            wide_expr = self.get_extend(&cx, e.se, false, narrow_expr);
            ext_kind = ExtendKind::Zero;
        }
        if !self.is_add_rec_of_loop(e, wide_expr) {
            return (None, ExtendKind::Unknown);
        }
        (Some(wide_expr), ext_kind)
    }

    /// `isa<SCEVAddRecExpr>`.
    fn is_add_rec_any(
        &self,
        e: &Edit,
        s: Scev,
    ) -> bool {
        matches!(e.se.node(s).kind, Kind::AddRec(..))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1591
    /// `truncateIVUse`: this IV user cannot be widened. Replace this use of
    /// the original narrow IV with a truncation of the new wide IV to isolate
    /// and eliminate the narrow IV.
    fn truncate_iv_use(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) {
        let narrow_def_op = value_op(e, du.narrow_def);
        let Some(insert_pt) = get_insert_point_for_uses(e, du.narrow_use, narrow_def_op) else { return };
        let ext_kind = self.get_extend_kind(du.narrow_def);
        let narrow_ty = operand_type(e, narrow_def_op);
        let mut flags = Flags::default();
        if du.never_negative || ext_kind == ExtendKind::Zero {
            flags.insert(Flags::NUW);
        }
        if du.never_negative || ext_kind == ExtendKind::Sign {
            flags.insert(Flags::NSW);
        }
        let wide_def_op = value_op(e, du.wide_def);
        let trunc = build_cast(e, CastOp::Trunc, wide_def_op, narrow_ty, flags, insert_pt);
        replace_uses_of_with(e, du.narrow_use, narrow_def_op, trunc);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1609
    /// `widenLoopCompare`: if the narrow use is a compare instruction, then
    /// widen the compare (and possibly the other operand). The extend
    /// operation is hoisted into the loop preheader as far as possible.
    fn widen_loop_compare(
        &self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> bool {
        let compare = e.function.instruction(du.narrow_use);
        let Opcode::ICmp(pred) = compare.opcode else { return false };
        let (operands, same_sign) = (compare.operands.clone(), compare.flags.contains(Flags::SAMESIGN));

        // We can legally widen the comparison in the following two cases:
        //
        //  - The signedness of the IV extension and comparison match
        //
        //  - The narrow IV is always non-negative (and thus its sign extension
        //    is equal to its zero extension). For instance, let's say we're
        //    zero extending %narrow for the following use
        //
        //      icmp slt i32 %narrow, %val   ... (A)
        //
        //    and %narrow is always non-negative. Then
        //
        //      (A) == icmp slt i32 sext(%narrow), sext(%val)
        //          == icmp slt i32 zext(%narrow), sext(%val)
        let is_signed = self.get_extend_kind(du.narrow_def) == ExtendKind::Sign;
        let mut cmp_preferred_sign = if same_sign { is_signed } else { is_signed_predicate(pred) };
        if !du.never_negative && is_signed != cmp_preferred_sign {
            return false;
        }

        let narrow_def_op = value_op(e, du.narrow_def);
        let wide_def_op = value_op(e, du.wide_def);
        let op = operands[if operands[0] == narrow_def_op { 1 } else { 0 }];
        let op_ty = operand_type(e, op);
        let cast_width = e.context.types.int_bits(op_ty).expect("an integer compare");
        let iv_width = {
            let Ty::Int(bits) = self.wide_type else { unreachable!("a wide integer") };
            bits
        };
        debug_assert!(cast_width <= iv_width, "Unexpected width while widening compare.");

        // Widen the compare instruction.
        replace_uses_of_with(e, du.narrow_use, narrow_def_op, wide_def_op);

        // Widen the other operand of the compare, if necessary.
        if cast_width < iv_width {
            // If the narrow IV is always non-negative and the other operand is
            // sext, widen using sext so we can combine them. This works for all
            // non-signed comparison predicates.
            let op_is_sext = defining(e.function, op)
                .is_some_and(|inst| e.function.instruction(inst).opcode == Opcode::Cast(CastOp::SExt));
            if du.never_negative && op_is_sext && !is_signed_predicate(pred) {
                cmp_preferred_sign = true;
            }
            let ext_op = self.create_extend_inst(e, op, cmp_preferred_sign, du.narrow_use);
            replace_uses_of_with(e, du.narrow_use, op, ext_op);
        }
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1675
    /// `widenWithVariantUse`: the widenIVUse avoids generating trunc by
    /// evaluating the use as AddRec, this will not work when:
    ///    1) SCEV traces back to an instruction inside the loop that SCEV can
    ///       not expand, eg. add %indvar, (load %addr)
    ///    2) SCEV finds a loop variant, eg. add %indvar, %loopvariant
    /// While SCEV fails to avoid trunc, we can still try to use instruction
    /// combining approach to prove trunc is not required: as long as the
    /// result of add/sub/mul is extended to wide type, no trunc is required
    /// regardless of how %b is generated. This pattern is common when
    /// calculating address in 64 bit architecture.
    fn widen_with_variant_use(
        &mut self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> bool {
        let (narrow_use, narrow_def, wide_def) = (du.narrow_use, du.narrow_def, du.wide_def);
        // Handle the common case of add<nsw/nuw>
        let instruction = e.function.instruction(narrow_use);
        let Opcode::Binary(op_code) = instruction.opcode else { return false };
        // Only Add/Sub/Mul instructions are supported.
        if !matches!(op_code, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
            return false;
        }
        let operands = instruction.operands.clone();
        let (has_nsw, has_nuw) = (instruction.flags.contains(Flags::NSW), instruction.flags.contains(Flags::NUW));
        let narrow_def_op = value_op(e, narrow_def);
        let wide_def_op = value_op(e, wide_def);
        let narrow_use_result = result_of(e.function, narrow_use);

        let ext_kind = self.get_extend_kind(narrow_def);
        let can_sign_extend = ext_kind == ExtendKind::Sign && has_nsw;
        let can_zero_extend = ext_kind == ExtendKind::Zero && has_nuw;
        let mut another_op_ext_kind = ext_kind;

        // Check that all uses are either:
        // - narrow def (in case of we are widening the IV increment);
        // - single-input LCSSA Phis;
        // - comparison of the chosen type;
        // - extend of the chosen type (raison d'etre).
        let mut ext_users: Vec<InstId> = Vec::new();
        let mut lcssa_phi_users: Vec<InstId> = Vec::new();
        let mut icmp_users: Vec<InstId> = Vec::new();
        let wide_ir = self.wide_ir_type(e);
        let users: Vec<InstId> = e.function.users(narrow_use_result).iter().map(|used| used.user).collect();
        for user in users {
            if user == narrow_def {
                continue;
            }
            let (outside, user_opcode) = {
                let cx = cx!(e);
                let found = cx.loops.get_loop_for(self.l).expect("the loop");
                (!found.contains_instruction(cx.function, user), e.function.instruction(user).opcode.clone())
            };
            if outside {
                // Make sure there is only 1 input, so that we don't have to
                // split critical edges.
                if user_opcode != Opcode::Phi || e.function.instruction(user).operands.len() != 2 {
                    return false;
                }
                lcssa_phi_users.push(user);
                continue;
            }
            if let Opcode::ICmp(pred) = user_opcode {
                // We have 3 types of predicates: signed, unsigned and equality
                // predicates. For equality, it's legal to widen icmp for either
                // sign and zero extend. For sign extend, we can also do so for
                // signed predicates, likeweise for zero extend we can widen
                // icmp for unsigned predicates.
                if ext_kind == ExtendKind::Zero && is_signed_predicate(pred) {
                    return false;
                }
                if ext_kind == ExtendKind::Sign && is_unsigned_predicate(pred) {
                    return false;
                }
                icmp_users.push(user);
                continue;
            }
            let wanted = if ext_kind == ExtendKind::Sign { CastOp::SExt } else { CastOp::ZExt };
            if user_opcode != Opcode::Cast(wanted) || e.function.instruction(user).ty != wide_ir {
                return false;
            }
            ext_users.push(user);
        }
        if ext_users.is_empty() {
            e.dead.push(narrow_use);
            return true;
        }

        // We'll prove some facts that should be true in the context of ext
        // users. If there is no users, we are done now. If there are some,
        // pick their common dominator as context.
        let ctx_inst = {
            let cx = cx!(e);
            find_common_dominator(&cx, &ext_users)
        };

        if !can_sign_extend && !can_zero_extend {
            // Because InstCombine turns 'sub nuw' to 'add' losing the no-wrap
            // flag, we will most likely not see it. Let's try to prove it.
            if op_code != BinaryOp::Add {
                return false;
            }
            if ext_kind != ExtendKind::Zero {
                return false;
            }
            // TODO: Support case for NarrowDef = NarrowUse->getOperand(1).
            if operands[0] != narrow_def_op {
                return false;
            }
            // We cannot use a different extend kind for the same operand.
            if operands[1] == narrow_def_op {
                return false;
            }
            let cx = cx!(e);
            let lhs = e.se.get_scev(&cx, operands[0]);
            let rhs = e.se.get_scev(&cx, operands[1]);
            if !e.se.is_known_negative(&cx, rhs) {
                return false;
            }
            let negative = e.se.get_negative_scev(&cx, rhs, 0);
            let proved_sub_nuw = e.se.is_known_predicate_at(&cx, IntPredicate::Uge, lhs, negative, Some(ctx_inst));
            if !proved_sub_nuw {
                return false;
            }
            // In fact, our 'add' is 'sub nuw'. We will need to widen the 2nd
            // operand as neg(zext(neg(op))), which is basically sext(op).
            another_op_ext_kind = ExtendKind::Sign;
        }

        // Verifying that Defining operand is an AddRec
        {
            let cx = cx!(e);
            let op1 = e.se.get_scev(&cx, wide_def_op);
            if !self.is_add_rec_of_loop(e, op1) {
                return false;
            }
        }

        // Generating a widening use instruction.
        let another_signed = another_op_ext_kind == ExtendKind::Sign;
        let lhs = if operands[0] == narrow_def_op {
            wide_def_op
        } else {
            self.create_extend_inst(e, operands[0], another_signed, narrow_use)
        };
        let rhs = if operands[1] == narrow_def_op {
            wide_def_op
        } else {
            self.create_extend_inst(e, operands[1], another_signed, narrow_use)
        };
        let wide_bo = build_binary_like(e, narrow_use, lhs, rhs, narrow_use);
        let wide_bo_op = value_op(e, wide_bo);
        self.extend_kind_map.insert(narrow_use, ext_kind);

        for user in ext_users {
            // INDVARS: eliminating `user` replaced by the wide operation.
            self.num_elim_ext += 1;
            let result = result_of(e.function, user);
            e.function.replace_all_uses_with(result, wide_bo_op);
            e.dead.push(user);
        }

        for user in lcssa_phi_users {
            let (value_block, user_ty) = {
                let arm = llrm_analysis::occurrence::arms(e.function, user)[0];
                (arm.1, e.function.instruction(user).ty)
            };
            let wide_phi = e
                .function
                .create_instruction(
                    Opcode::Phi,
                    wide_ir,
                    vec![wide_bo_op, Operand::Block(value_block)],
                    Flags::default(),
                    None,
                );
            e.function.insert(wide_phi, Position::Before(user)).expect("a placed phi");
            let block = e.function.parent(user).expect("placed");
            let first = first_insertion_point(e.function, block);
            let wide_phi_op = value_op(e, wide_phi);
            let trunc_pn = build_cast(e, CastOp::Trunc, wide_phi_op, user_ty, Flags::default(), first);
            let result = result_of(e.function, user);
            e.function.replace_all_uses_with(result, trunc_pn);
            e.dead.push(user);
        }

        for user in icmp_users {
            let Opcode::ICmp(pred) = e.function.instruction(user).opcode else { unreachable!("a compare") };
            let compare_operands = e.function.instruction(user).operands.clone();
            let narrow_use_op = value_op(e, narrow_use);
            let mut extended = [wide_bo_op; 2];
            for (slot, v) in compare_operands.iter().enumerate() {
                extended[slot] = if *v == narrow_use_op {
                    wide_bo_op
                } else {
                    let op = if ext_kind == ExtendKind::Zero { CastOp::ZExt } else { CastOp::SExt };
                    build_cast(e, op, *v, wide_ir, Flags::default(), user)
                };
            }
            let bit = e.context.types.int(1);
            let wide_cmp = e
                .function
                .create_instruction(Opcode::ICmp(pred), bit, vec![extended[0], extended[1]], Flags::default(), None);
            e.function.insert(wide_cmp, Position::Before(user)).expect("a placed compare");
            let wide_cmp_op = value_op(e, wide_cmp);
            let result = result_of(e.function, user);
            e.function.replace_all_uses_with(result, wide_cmp_op);
            e.dead.push(user);
        }
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1855
    /// `widenIVUse`: determine whether an individual user of the narrow IV can
    /// be widened. If so, the wide clone of the user.
    fn widen_iv_use(
        &mut self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> Option<InstId> {
        debug_assert!(
            self.extend_kind_map.get(&du.narrow_def).is_some(),
            "Should already know the kind of extension used to widen NarrowDef"
        );
        let narrow_def_kind = self.get_extend_kind(du.narrow_def);
        // This narrow use can be widened by a sext if it's non-negative or its
        // narrow def was widened by a sext. Same for zext.
        let can_widen_by_sext = du.never_negative || narrow_def_kind == ExtendKind::Sign;
        let can_widen_by_zext = du.never_negative || narrow_def_kind == ExtendKind::Zero;
        let narrow_def_op = value_op(e, du.narrow_def);
        let wide_def_op = value_op(e, du.wide_def);

        // Stop traversing the def-use chain at inner-loop phis or post-loop
        // phis.
        if e.function.instruction(du.narrow_use).opcode == Opcode::Phi {
            let phi_loop = {
                let cx = cx!(e);
                cx.function.parent(du.narrow_use).and_then(|block| cx.loops.get_loop_for(block)).map(|l| l.get_header())
            };
            if phi_loop != Some(self.l) {
                // For LCSSA phis, sink the truncate outside the loop. After
                // SimplifyCFG most loop exit targets have a single
                // predecessor. Otherwise fall back to a truncate within the
                // loop.
                if e.function.instruction(du.narrow_use).operands.len() != 2 {
                    self.truncate_iv_use(e, du);
                } else {
                    let wide_ir = operand_type(e, wide_def_op);
                    let incoming_block = llrm_analysis::occurrence::arms(e.function, du.narrow_use)[0].1;
                    let wide_phi = e
                        .function
                        .create_instruction(
                            Opcode::Phi,
                            wide_ir,
                            vec![wide_def_op, Operand::Block(incoming_block)],
                            Flags::default(),
                            None,
                        );
                    e.function.insert(wide_phi, Position::Before(du.narrow_use)).expect("a placed phi");
                    let block = e.function.parent(wide_phi).expect("placed");
                    let first = first_insertion_point(e.function, block);
                    let mut flags = Flags::default();
                    if can_widen_by_zext {
                        flags.insert(Flags::NUW);
                    }
                    if can_widen_by_sext {
                        flags.insert(Flags::NSW);
                    }
                    let narrow_ty = operand_type(e, narrow_def_op);
                    let wide_phi_op = value_op(e, wide_phi);
                    let trunc = build_cast(e, CastOp::Trunc, wide_phi_op, narrow_ty, flags, first);
                    let result = result_of(e.function, du.narrow_use);
                    e.function.replace_all_uses_with(result, trunc);
                    e.dead.push(du.narrow_use);
                }
                return None;
            }
        }

        // Our raison d'etre! Eliminate sign and zero extension.
        let use_opcode = e.function.instruction(du.narrow_use).opcode.clone();
        let sext_like = use_opcode == Opcode::Cast(CastOp::SExt)
            || (use_opcode == Opcode::Cast(CastOp::ZExt)
                && e.function.instruction(du.narrow_use).flags.contains(Flags::NNEG));
        if (sext_like && can_widen_by_sext) || (use_opcode == Opcode::Cast(CastOp::ZExt) && can_widen_by_zext) {
            // An extension has a value.
            let narrow_use_op = value_op(e, du.narrow_use);
            let mut new_def = wide_def_op;
            let use_ty = e.function.instruction(du.narrow_use).ty;
            let wide_ir = self.wide_ir_type(e);
            if use_ty != wide_ir {
                let cast_width = e.context.types.int_bits(use_ty).expect("an integer extension");
                let Ty::Int(iv_width) = self.wide_type else { unreachable!("a wide integer") };
                if cast_width < iv_width {
                    // The cast isn't as wide as the IV, so insert a Trunc.
                    let mut flags = Flags::default();
                    if can_widen_by_zext {
                        flags.insert(Flags::NUW);
                    }
                    if can_widen_by_sext {
                        flags.insert(Flags::NSW);
                    }
                    new_def = build_cast(e, CastOp::Trunc, wide_def_op, use_ty, flags, du.narrow_use);
                } else {
                    // A wider extend was hidden behind a narrower one. This may
                    // induce another round of IV widening in which the
                    // intermediate IV becomes dead. It should be very rare.
                    replace_uses_of_with(e, du.narrow_use, narrow_def_op, wide_def_op);
                    new_def = narrow_use_op;
                }
            }
            if new_def != narrow_use_op {
                // INDVARS: eliminating the extension replaced by the wide def.
                self.num_elim_ext += 1;
                let result = result_of(e.function, du.narrow_use);
                e.function.replace_all_uses_with(result, new_def);
                e.dead.push(du.narrow_use);
            }
            // Now that the extend is gone, we want to expose it's uses for
            // potential further simplification. We don't need to directly
            // inform SimplifyIVUsers of the new users, because their parent IV
            // will be processed later as a new loop phi. If we preserved
            // IVUsers analysis, we would also want to push the uses
            // of WideDef here.

            // No further widening is needed. The deceased [sz]ext had done it
            // for us.
            return None;
        }

        // `tryAddRecExpansion`.
        if let Some(wide_use) = self.try_add_rec_expansion(e, du) {
            return Some(wide_use);
        }

        // If use is a loop condition, try to promote the condition instead of
        // truncating the IV first.
        if self.widen_loop_compare(e, du) {
            return None;
        }

        // We are here about to generate a truncate instruction that may hurt
        // performance because the scalar evolution expression computed
        // earlier in WideAddRec.first does not indicate a polynomial induction
        // expression. In that case, look at the operands of the use
        // instruction to determine if we can still widen the use instead of
        // truncating its operand.
        if self.widen_with_variant_use(e, du) {
            return None;
        }

        // This user does not evaluate to a recurrence after widening, so don't
        // follow it. Instead insert a Trunc to kill off the original use,
        // eventually isolating the original narrow IV so it can be removed.
        self.truncate_iv_use(e, du);
        None
    }

    /// The lambda `tryAddRecExpansion` of `widenIVUse`.
    fn try_add_rec_expansion(
        &mut self,
        e: &mut Edit,
        du: NarrowIvDefUse,
    ) -> Option<InstId> {
        // Does this user itself evaluate to a recurrence after widening?
        let mut wide_add_rec = self.get_extended_operand_recurrence(e, du);
        if wide_add_rec.0.is_none() {
            wide_add_rec = self.get_wide_recurrence(e, du);
        }
        debug_assert!(wide_add_rec.0.is_none() == (wide_add_rec.1 == ExtendKind::Unknown));
        let wide_add_rec_scev = wide_add_rec.0?;

        let can_use_wide_inc = |this: &Self, e: &mut Edit| -> bool {
            let Some(wide_inc) = this.wide_inc else { return false };
            // Reuse the IV increment that SCEVExpander created. Recompute
            // flags, unless the flags for both increments agree and it is safe
            // to use the ones from the original inc. In that case, the new use
            // of the wide increment won't be more poisonous.
            let wide_phi = this.wide_phi.expect("the wide phi");
            let narrow_flags = e.function.instruction(du.narrow_use).flags;
            let wide_flags = e.function.instruction(wide_inc).flags;
            let need_to_recompute_flags =
                !seam::can_reuse_flags_from_original_iv_inc(e, this.orig_phi, wide_phi, du.narrow_use, wide_inc)
                    || narrow_flags.contains(Flags::NUW) != wide_flags.contains(Flags::NUW)
                    || narrow_flags.contains(Flags::NSW) != wide_flags.contains(Flags::NSW);
            Some(wide_add_rec_scev) == this.wide_inc_expr
                && seam::hoist_iv_inc(e, wide_inc, du.narrow_use, need_to_recompute_flags)
        };

        let wide_use = if can_use_wide_inc(self, e) {
            self.wide_inc.expect("a wide increment")
        } else {
            self.clone_iv_user(e, du, wide_add_rec_scev)?
        };
        // Evaluation of WideAddRec ensured that the narrow expression could be
        // extended outside the loop without overflow. This suggests that the
        // wide use evaluates to the same expression as the extended narrow use,
        // but doesn't absolutely guarantee it. Hence the following failsafe
        // check. In rare cases where it fails, we simply throw away the newly
        // created wide use.
        let wide_use_op = value_op(e, wide_use);
        let cx = cx!(e);
        if Some(wide_add_rec_scev) != Some(e.se.get_scev(&cx, wide_use_op)) {
            e.dead.push(wide_use);
            return None;
        }

        self.extend_kind_map.insert(du.narrow_use, wide_add_rec.1);
        // Returning WideUse pushes it on the worklist.
        Some(wide_use)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L2021
    /// `pushNarrowIVUsers`: add eligible users of `narrow_def` to the
    /// worklist.
    fn push_narrow_iv_users(
        &mut self,
        e: &mut Edit,
        narrow_def: InstId,
        wide_def: InstId,
    ) {
        let narrow_def_op = value_op(e, narrow_def);
        let non_negative_def = {
            let cx = cx!(e);
            let narrow_scev = e.se.get_scev(&cx, narrow_def_op);
            let ty = e.se.type_of(narrow_scev);
            let zero = e.se.get_zero(&cx, ty);
            e.se.is_known_predicate_scev(&cx, IntPredicate::Sge, narrow_scev, zero)
        };
        let users: Vec<InstId> = e.function.users(result_of(e.function, narrow_def)).iter().map(|u| u.user).collect();
        for narrow_user in users {
            // Handle data flow merges and bizarre phi cycles.
            if !self.widened.insert(narrow_user) {
                continue;
            }

            let mut non_negative_use = false;
            if !non_negative_def {
                // We might have a control-dependent range information for this
                // context.
                if let Some(range) = self.post_inc_range(narrow_def, narrow_user) {
                    non_negative_use = is_non_negative(range.get_signed_min(), range.width);
                }
            }

            self.narrow_iv_users.push(NarrowIvDefUse {
                narrow_def,
                narrow_use: narrow_user,
                wide_def,
                never_negative: non_negative_def || non_negative_use,
            });
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L2053
    /// `createWideIV`: process a single induction variable. First use the
    /// SCEVExpander to create a wide induction variable that evaluates to the
    /// same recurrence as the original narrow IV. Then use a worklist to
    /// forward traverse the narrow IV's def-use chain. After widenIVUse has
    /// processed all interesting IV users, the narrow IV will be isolated for
    /// removal by DeleteDeadPHIs.
    ///
    /// It would be simpler to delete uses as they are processed, but we must
    /// avoid invalidating SCEV expressions.
    pub fn create_wide_iv(
        &mut self,
        e: &mut Edit,
    ) -> Option<InstId> {
        // Is this phi an induction variable?
        let orig_phi_op = value_op(e, self.orig_phi);
        let (add_rec, wide_iv_expr) = {
            let cx = cx!(e);
            let add_rec = e.se.get_scev(&cx, orig_phi_op);
            if !self.is_add_rec_any(e, add_rec) {
                return None;
            }
            // Widen the induction variable expression.
            let signed = self.get_extend_kind(self.orig_phi) == ExtendKind::Sign;
            (add_rec, self.get_extend(&cx, e.se, signed, add_rec))
        };
        let _ = add_rec;

        // Can the IV be extended outside the loop without overflow?
        if !self.is_add_rec_of_loop(e, wide_iv_expr) {
            return None;
        }
        let add_rec = wide_iv_expr;

        // Iterate over IV uses (including transitive ones) looking for IV
        // increments of the form 'add nsw %iv, <const>'. For each increment and
        // each use of the increment calculate control-dependent range
        // information basing on dominating conditions inside of the loop (e.g.
        // a range check inside of the loop). Calculated ranges are stored in
        // PostIncRangeInfos map.
        //
        // Control-dependent range information is later used to prove that a
        // narrow definition is not negative (see pushNarrowIVUsers). It's
        // difficult to do this on demand because when pushNarrowIVUsers needs
        // this information some of the dominating conditions might be already
        // widened.
        if self.use_post_increment_ranges {
            self.calculate_post_inc_ranges(e);
        }

        // The rewriter provides a value for the desired IV expression. This may
        // either find an existing phi or materialize a new one. Either way, we
        // expect a well-formed cyclic phi-with-increments. i.e. any operand not
        // part of the phi-SCC dominates the loop entry.
        let insert_pt = first_insertion_point(e.function, self.l);
        e.rewriter.set_insert_point(insert_pt);
        let expand_inst =
            e.rewriter.expand_code_for(&e.env, e.context, e.declared, e.function, e.se, add_rec, Some(self.wide_type));
        // If the wide phi is not a phi node, for example a cast node, like
        // bitcast, inttoptr, ptrtoint, just skip for now.
        let expand_def = defining(e.function, expand_inst);
        let wide_phi = match expand_def {
            Some(inst) if e.function.instruction(inst).opcode == Opcode::Phi => inst,
            _ => {
                // if the cast node is an inserted instruction without any user,
                // we should remove it to make sure the pass don't touch the
                // function as we can not wide the phi.
                if let Some(inst) = expand_def {
                    let unused = e.function.instruction(inst).result.is_some_and(|r| e.function.users(r).is_empty());
                    if unused && seam::is_inserted_instruction(e, inst) {
                        e.dead.push(inst);
                    }
                }
                return None;
            }
        };
        self.wide_phi = Some(wide_phi);

        // Remembering the WideIV increment generated by SCEVExpander allows
        // widenIVUse to reuse it when widening the narrow IV's increment. We
        // don't employ a general reuse mechanism because the call above is the
        // only call to SCEVExpander. Henceforth, we produce 1-to-1 narrow to
        // wide uses.
        let latch = {
            let cx = cx!(e);
            cx.loops.get_loop_for(self.l).and_then(|found| found.get_loop_latch(cx.function))
        };
        if let Some(latch) = latch {
            let incoming = |phi: InstId, e: &Edit| {
                llrm_analysis::occurrence::arms(e.function, phi).into_iter().find(|&(_, from)| from == latch)
            };
            self.wide_inc = incoming(wide_phi, e).and_then(|(value, _)| defining(e.function, value));
            if let Some(wide_inc) = self.wide_inc {
                let wide_inc_op = value_op(e, wide_inc);
                let cx = cx!(e);
                self.wide_inc_expr = Some(e.se.get_scev(&cx, wide_inc_op));
                let orig_inc = incoming(self.orig_phi, e)
                    .and_then(|(value, _)| defining(e.function, value))
                    .expect("the original increment");

                // We are replacing a narrow IV increment with a wider IV
                // increment. If the original (narrow) increment did not wrap,
                // the wider increment one should not wrap either. Set the
                // flags to be the union of both wide increment and original
                // increment; this ensures we preserve flags SCEV could infer
                // for the wider increment. Limit this only to cases where both
                // increments directly increment the corresponding PHI nodes and
                // have the same opcode. It is not safe to re-use the flags from
                // the original increment, if it is more complex and SCEV
                // expansion may have yielded a more simplified wider
                // increment.
                let overflowing = |inst: InstId, e: &Edit| {
                    matches!(
                        e.function.instruction(inst).opcode,
                        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)
                    )
                };
                if seam::can_reuse_flags_from_original_iv_inc(e, self.orig_phi, wide_phi, orig_inc, wide_inc)
                    && overflowing(orig_inc, e)
                    && overflowing(wide_inc, e)
                {
                    let orig_flags = e.function.instruction(orig_inc).flags;
                    let mut flags = e.function.instruction(wide_inc).flags;
                    if orig_flags.contains(Flags::NUW) {
                        flags.insert(Flags::NUW);
                    }
                    if orig_flags.contains(Flags::NSW) {
                        flags.insert(Flags::NSW);
                    }
                    e.function.set_flags(wide_inc, flags);
                }
            }
        }

        self.num_widened += 1;

        // Traverse the def-use chain using a worklist starting at the original
        // IV.
        debug_assert!(self.widened.is_empty() && self.narrow_iv_users.is_empty(), "expect initial state");

        self.widened.insert(self.orig_phi);
        self.push_narrow_iv_users(e, self.orig_phi, wide_phi);

        while let Some(du) = self.narrow_iv_users.pop() {
            // Process a def-use edge. This may replace the use, so don't hold a
            // use_iterator across it.
            let wide_use = self.widen_iv_use(e, du);

            // Follow all def-use edges from the previous narrow use.
            if let Some(wide_use) = wide_use {
                self.push_narrow_iv_users(e, du.narrow_use, wide_use);
            }

            // widenIVUse may have removed the def-use edge.
            let narrow_def_result = result_of(e.function, du.narrow_def);
            if e.function.users(narrow_def_result).is_empty() {
                e.dead.push(du.narrow_def);
            }
        }

        Some(wide_phi)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L2180
    /// `calculatePostIncRange`: the control-dependent range of the given def at
    /// the given context, by looking at dominating conditions inside of the
    /// loop.
    fn calculate_post_inc_range(
        &mut self,
        e: &mut Edit,
        narrow_def: InstId,
        narrow_user: InstId,
    ) {
        // `m_NSWAdd(m_Value(NarrowDefLHS), m_APInt(NarrowDefRHS))`.
        let def = e.function.instruction(narrow_def);
        if def.opcode != Opcode::Binary(BinaryOp::Add) || !def.flags.contains(Flags::NSW) {
            return;
        }
        let narrow_def_lhs = def.operands[0];
        let Some((width, rhs)) = int_constant(e, def.operands[1]) else { return };
        if !is_non_negative(rhs, width) {
            return;
        }

        // The conditions that bound the range, gathered with the places they
        // hold at; applied after the walk so the tree is only read.
        let mut conditions: Vec<(Operand, bool)> = Vec::new();
        let guards_from = |e: &Edit, ctx: InstId, conditions: &mut Vec<(Operand, bool)>| {
            if !self.has_guards {
                return;
            }
            let cx = cx!(e);
            let query = cx.query(None);
            for inst in std::iter::successors(Some(ctx), |&one| cx.function.prev_in_block(one)) {
                if llrm_mir::valuetracking::pattern::call_intrinsic(&query, inst)
                    == Some(llrm_mir::valuetracking::pattern::IntrinsicId::ExperimentalGuard)
                {
                    conditions.push((cx.function.instruction(inst).operands[0], true));
                }
            }
        };

        guards_from(e, narrow_user, &mut conditions);

        let user_block = e.function.parent(narrow_user).expect("placed");
        {
            let cx = cx!(e);
            // If NarrowUserBB is statically unreachable asking dominator
            // queries may yield surprising results. (e.g. the block may not
            // have a dom tree node)
            if !cx.tree.is_reachable_from_entry(user_block) {
                return;
            }
            let found = cx.loops.get_loop_for(self.l).expect("the loop");
            let mut dtb = cx.tree.immediate_dominator(user_block);
            while let Some(block) = dtb.filter(|block| found.contains(*block)) {
                let terminator = cx.function.terminator(block).expect("a terminated block");
                guards_from(e, terminator, &mut conditions);

                let branch = cx.function.instruction(terminator);
                if let (Opcode::Br, [condition, Operand::Block(taken), Operand::Block(other)]) =
                    (&branch.opcode, branch.operands.as_slice())
                {
                    if cx.tree.dominates_edge(cx.function, block, *taken, user_block) {
                        conditions.push((*condition, true));
                    }
                    if cx.tree.dominates_edge(cx.function, block, *other, user_block) {
                        conditions.push((*condition, false));
                    }
                }
                dtb = cx.tree.immediate_dominator(block);
            }
        }

        // `UpdateRangeFromCondition`, in the order LLVM meets them.
        for (condition, true_dest) in conditions {
            let Some(cmp) = defining(e.function, condition) else { continue };
            let instruction = e.function.instruction(cmp);
            let Opcode::ICmp(pred) = instruction.opcode else { continue };
            if instruction.operands[0] != narrow_def_lhs {
                continue;
            }
            let cmp_rhs = instruction.operands[1];
            let pred = if true_dest { pred } else { pred.inverse() };

            let cx = cx!(e);
            let rhs_scev = e.se.get_scev(&cx, cmp_rhs);
            let cmp_rhs_range = e.se.get_signed_range(&cx, rhs_scev);
            let constrained = ConstantRange::make_allowed_icmp_region(pred, &cmp_rhs_range);
            let narrow_def_range = constrained.add_with_no_wrap(
                &ConstantRange::single(width, rhs),
                llrm_mir::constantrange::NO_SIGNED_WRAP,
                llrm_mir::constantrange::PreferredRangeType::Smallest,
            );
            self.update_post_inc_range_info(narrow_def, narrow_user, narrow_def_range);
        }
    }

    /// `updatePostIncRangeInfo`.
    fn update_post_inc_range_info(
        &mut self,
        def: InstId,
        use_: InstId,
        range: ConstantRange,
    ) {
        if self.post_inc_range_infos.get(&def).is_none() {
            self.post_inc_range_infos.insert(def, Vec::new());
        }
        let ranges = self.post_inc_range_infos.get_mut(&def).expect("just made");
        match ranges.iter_mut().find(|(one, _)| *one == use_) {
            None => ranges.push((use_, range)),
            Some((_, old)) => *old = range.intersect_with(old, llrm_mir::constantrange::PreferredRangeType::Smallest),
        }
    }

    /// `getPostIncRangeInfo`.
    fn post_inc_range(
        &self,
        def: InstId,
        use_: InstId,
    ) -> Option<&ConstantRange> {
        self.post_inc_range_infos.get(&def)?.iter().find(|(one, _)| *one == use_).map(|(_, range)| range)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L2250
    /// `calculatePostIncRanges`: the map for the given IV.
    fn calculate_post_inc_ranges(
        &mut self,
        e: &mut Edit,
    ) {
        let mut visited: IdSet<InstId> = IdSet::new();
        let mut worklist = vec![self.orig_phi];
        visited.insert(self.orig_phi);

        while let Some(narrow_def) = worklist.pop() {
            let users: Vec<InstId> = match e.function.instruction(narrow_def).result {
                Some(result) => e.function.users(result).iter().map(|used| used.user).collect(),
                None => Vec::new(),
            };
            for narrow_user in users {
                // Don't go looking outside the current loop.
                let inside = {
                    let cx = cx!(e);
                    let user_loop = cx.function.parent(narrow_user).and_then(|block| cx.loops.get_loop_for(block));
                    let ours = cx.loops.get_loop_for(self.l).expect("the loop");
                    user_loop.is_some_and(|found| ours.contains_loop(Some(found)))
                };
                if !inside {
                    continue;
                }

                if !visited.insert(narrow_user) {
                    continue;
                }

                worklist.push(narrow_user);

                self.calculate_post_inc_range(e, narrow_def, narrow_user);
            }
        }
    }
}

/// `findCommonDominator`: a point in code which dominates all given
/// instructions.
fn find_common_dominator(
    cx: &Cx,
    instructions: &[InstId],
) -> InstId {
    let mut common: Option<InstId> = None;
    for &inst in instructions {
        common = Some(match common {
            None => inst,
            Some(one) => {
                let (a, b) = (cx.function.parent(one).expect("placed"), cx.function.parent(inst).expect("placed"));
                if a != b {
                    let block = nearest_common_dominator(cx, a, b);
                    cx.function.terminator(block).expect("a terminated block")
                } else {
                    // Use whichever instruction is first in the block.
                    if cx.function.comes_before(one, inst) { one } else { inst }
                }
            }
        });
    }
    common.expect("Common dominator not found?")
}

/// `BasicBlock::getFirstInsertionPt`.
pub(crate) fn first_insertion_point(
    function: &Function,
    block: BlockId,
) -> InstId {
    function
        .iter_block(block)
        .find(|&inst| !matches!(
            function.instruction(inst).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ))
        .expect("a terminated block")
}

/// The expander entry points `widenIVUse` and `createWideIV` call.
mod seam {
    use super::{Edit, Expander, InstId};

    pub fn can_reuse_flags_from_original_iv_inc(
        e: &Edit,
        orig_phi: InstId,
        wide_phi: InstId,
        orig_inc: InstId,
        wide_inc: InstId,
    ) -> bool {
        Expander::can_reuse_flags_from_original_iv_inc(e.function, orig_phi, wide_phi, orig_inc, wide_inc)
    }

    pub fn hoist_iv_inc(
        e: &mut Edit,
        inc: InstId,
        insert_pos: InstId,
        recompute_poison_flags: bool,
    ) -> bool {
        e.rewriter.hoist_iv_inc(&e.env, e.context, e.function, e.se, inc, insert_pos, recompute_poison_flags)
    }

    pub fn is_inserted_instruction(
        e: &Edit,
        inst: InstId,
    ) -> bool {
        e.rewriter.is_inserted_instruction(&inst)
    }
}

#[cfg(test)]
#[path = "widen_tests.rs"]
mod tests;
