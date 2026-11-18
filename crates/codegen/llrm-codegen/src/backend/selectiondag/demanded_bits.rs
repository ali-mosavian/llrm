//! `SimplifyDemandedBits` (the pin's `TargetLowering::SimplifyDemandedBits`,
//! [L1201][pin]): rewrite a value knowing only some of its bits are read.
//!
//! # How it works
//! [`DagCombiner::simplify_demanded_bits`] asks about one node with every bit
//! demanded. Each opcode passes a smaller demanded mask to its operands
//! (`and` asks its left operand only for the bits the right one does not
//! clear; a shift moves the mask; an extension or truncation resizes it), and
//! an operand that cannot change the demanded bits is replaced by the other
//! one, a constant, or a version with a narrower constant. The first rewrite
//! found is committed (all uses of the old value read the new one) and
//! reported, as `TargetLoweringOpt::CombineTo` does.
//!
//! # Runs
//! The pin's `DAGCombiner` asks it from `visitADDLike`
//! ([L3133](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3133)),
//! `visitMUL`
//! ([L5155](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5155)),
//! `visitUDIV` (L5511), `visitMULHU` (L5821), `visitIMINMAX` (L6580),
//! `visitAND`, `visitOR`, `visitXOR`, `visitRotate` (L10995), `visitSHL`,
//! `visitSRA` (L11707), `visitSRL`, `visitFunnelShift`, `visitTRUNCATE` and
//! `visitSIGN_EXTEND_INREG`; here every one is called last. DEVIATE SD212b:
//! in `visitADDLike`, `visitRotate` and `visitSRA` the pin's call stands
//! before folds that follow it, ours after all of them; `visitMULHU` has no
//! other fold here. Exit: those visits translated whole, with the rest of
//! their folds. Owner llrm-target-wasm, review 2026-12-10.
//! Covered opcodes: `and`, `or`, `xor`, `shl`, `srl`, `rotl` and `rotr` by a
//! constant, the three extensions' zero and any forms, `trunc`. The rest fall
//! back to `computeKnownBits`. A right shift by one of a sum of two extended
//! values is an `avgfloor` or `avgceil` of the narrow values where the target
//! has the node (`combine_avg.rs`, DEVIATE SD600).
//!
//! `ShrinkDemandedOp` and the half-width shift rewrites run where the target
//! hook `free_narrowing` says the casts are free (x86-64; at m32 the pin's
//! `isZExtFree` gate never opens).
//!
//! DEVIATE SD212: no `SRA`/`SELECT` cases, no `SimplifyMultipleUseDemandedBits`
//! beyond `and`/`or`/`xor` identities and all-demanded-known constants. Exit:
//! those. Owner sdag-b. Review 2026-12-10.
//!
//! [pin]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L1201

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::fold::mask;
use super::isd::ISD;
use super::known_bits::{KnownBits, known_bits, known_bits_at, num_sign_bits};
use super::mvt::EVT;

/// `TargetLoweringOpt`: the value to replace and what replaces it.
type Tlo = Option<(SDValue, SDValue)>;

/// `SelectionDAG::MaxRecursionDepth`.
const MAX_DEPTH: u32 = 6;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `SimplifyDemandedBits(SDValue(N, 0))`: all bits of `node` demanded; true
    /// when something was rewritten (and committed).
    pub(super) fn simplify_demanded_bits(
        &mut self,
        node: SDValue,
    ) -> bool {
        let vt = self.dag.value_type(node);
        self.simplify_demanded_bits_of(node, mask(vt.size_in_bits()))
    }

    /// `SimplifyDemandedBits(Op, DemandedBits)`.
    pub(super) fn simplify_demanded_bits_of(
        &mut self,
        node: SDValue,
        demanded: u128,
    ) -> bool {
        let vt = self.dag.value_type(node);
        if !vt.is_integer() || vt.size_in_bits() > 64 {
            return false;
        }
        let mut tlo = None;
        if self.sdb(node, demanded, 0, false, &mut tlo).is_ok() {
            return false;
        }
        let Some((old, new)) = tlo else { return false };
        // "Revisit the node."
        self.add_to_worklist(node.node);
        self.replace_value(old, new);
        self.add_to_worklist_with_users(new.node);
        // `CommitTargetLoweringOpt` is `CombineTo`: the replaced node goes, or it
        // stays a reader of its operands (a load with a second use is not folded).
        self.dag.delete_dead(old.node);
        self.queue_survivors();
        true
    }

    /// `CombineTo`: record the replacement; `Err` stops the walk.
    fn combine_to(
        tlo: &mut Tlo,
        old: SDValue,
        new: SDValue,
    ) -> Result<KnownBits, ()> {
        *tlo = Some((old, new));
        Err(())
    }

    /// `ShrinkDemandedConstant` of `and`, `or` and `xor` by a constant.
    fn shrink_demanded_constant(
        &mut self,
        op: SDValue,
        demanded: u128,
        tlo: &mut Tlo,
    ) -> Result<(), ()> {
        let opcode = self.dag.opcode(op);
        let (x, rhs) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
        if demanded == 0 || !matches!(opcode, ISD::And | ISD::Or | ISD::Xor) || self.dag.is_opaque(rhs) {
            return Ok(());
        }
        let Some(c) = self.dag.constant(rhs) else { return Ok(()) };
        // `targetShrinkDemandedConstant` first: the target may keep the
        // constant (X86 keeps an `and` mask a `movzx` matches).
        if let Some(handled) = self.hooks.target_shrink_demanded_constant(self.dag, op, demanded, self.level) {
            return match handled {
                Some(new) => Self::combine_to(tlo, op, new).map(|_| ()),
                None => Ok(()),
            };
        }
        if opcode == ISD::Xor && demanded & !c == 0 {
            return Ok(());
        }
        if c & !demanded == 0 {
            return Ok(());
        }
        let vt = self.dag.value_type(op);
        let narrow = self.dag.get_constant(c & demanded, vt);
        let new = self.dag.get_node(opcode, vt, &[x, narrow]);
        Self::combine_to(tlo, op, new).map(|_| ())
    }

    /// `ShrinkDemandedOp`: a one-use binary operation whose demanded bits fit a
    /// narrower type the target casts to and from for free is done there,
    /// `(any_extend (op (trunc x) (trunc y)))`
    /// ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L641)).
    fn shrink_demanded_op(
        &mut self,
        op: SDValue,
        demanded: u128,
        tlo: &mut Tlo,
    ) -> Result<(), ()> {
        let vt = self.dag.value_type(op);
        let width = vt.size_in_bits();
        if !self.dag.has_one_use(op.node) {
            return Ok(());
        }
        // DEVIATE SD257 (owner llrm-target-6502, exit 2026-12-05), `known_zero_narrowing`:
        // the pin sizes by the demanded bits alone, so an `or` of two bytes read as
        // a word was done as a word; the bits known zero above the narrow type come
        // back as a zero extension. An `and` keeps the pin's sizing: its zeros come
        // from its own mask, and `zext(and8(trunc x, c))` folds back into the `and`.
        let by_known_zero = self.hooks.known_zero_narrowing() && self.dag.opcode(op) != ISD::And;
        let live = if by_known_zero { demanded & !known_bits(self.dag, op).zero } else { demanded };
        let size = (128 - live.leading_zeros()).max(1);
        let mut small = size.next_power_of_two();
        while small < width {
            let narrow = EVT::integer(small);
            if self.hooks.free_narrowing(vt, narrow) {
                let opcode = self.dag.opcode(op);
                let (a, b) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let a = self.dag.get_node(ISD::Truncate, narrow, &[a]);
                let b = self.dag.get_node(ISD::Truncate, narrow, &[b]);
                let x = self.dag.get_node(opcode, narrow, &[a, b]);
                let kept = if small >= 128 { u128::MAX } else { (1u128 << small) - 1 };
                let extend = if demanded & !kept == 0 { ISD::AnyExtend } else { ISD::ZeroExtend };
                let z = self.dag.get_node(extend, vt, &[x]);
                return Self::combine_to(tlo, op, z).map(|_| ());
            }
            small *= 2;
        }
        Ok(())
    }

    /// `SimplifyMultipleUseDemandedBits`: a value that reads the same on the
    /// demanded bits and has no other effect, found without touching the
    /// other users of `op`.
    pub(super) fn simplify_multiple_use_demanded_bits(
        &mut self,
        op: SDValue,
        demanded: u128,
        depth: u32,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(op);
        if !vt.is_integer() || vt.size_in_bits() > 64 {
            return None;
        }
        if demanded == 0 {
            return Some(self.dag.get_undef(vt));
        }
        if depth >= MAX_DEPTH || self.dag.constant(op).is_some() {
            return None;
        }
        let known = known_bits_at(self.dag, op, depth);
        if demanded & !(known.zero | known.one) == 0 {
            return Some(self.dag.get_constant(known.one, vt));
        }
        let identity = |dag: &super::dag::SelectionDAG, ops: [SDValue; 2]| {
            let [a, b] = ops.map(|v| known_bits_at(dag, v, depth + 1));
            (a, b)
        };
        match self.dag.opcode(op) {
            ISD::And => {
                let (l, r) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let (lk, rk) = identity(self.dag, [l, r]);
                if demanded & !(lk.zero | rk.one) == 0 {
                    return Some(l);
                }
                if demanded & !(rk.zero | lk.one) == 0 {
                    return Some(r);
                }
                None
            }
            ISD::Or => {
                let (l, r) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let (lk, rk) = identity(self.dag, [l, r]);
                if demanded & !(lk.one | rk.zero) == 0 {
                    return Some(l);
                }
                if demanded & !(rk.one | lk.zero) == 0 {
                    return Some(r);
                }
                None
            }
            ISD::Xor => {
                let (l, r) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let (lk, rk) = identity(self.dag, [l, r]);
                if demanded & !rk.zero == 0 {
                    return Some(l);
                }
                if demanded & !lk.zero == 0 {
                    return Some(r);
                }
                None
            }
            _ => None,
        }
    }

    /// `SimplifyDemandedBits(Op, DemandedBits, Known, TLO, Depth,
    /// AssumeSingleUse)`. `Ok` is "nothing changed" with the known bits;
    /// `Err` is "rewritten".
    fn sdb(
        &mut self,
        op: SDValue,
        demanded_in: u128,
        depth: u32,
        assume_single_use: bool,
        tlo: &mut Tlo,
    ) -> Result<KnownBits, ()> {
        let result = self.sdb_inner(op, demanded_in, depth, assume_single_use, tlo);
        if result.is_err() {
            // "Disable the nsw and nuw flags. We can no longer guarantee that
            // we won't wrap after simplification": `Op->dropFlags`
            // where an operand was simplified, for `SHL`,
            // `TRUNCATE`, `ADD`, `SUB` and `MUL` (NoWrap), `OR`
            // (Disjoint) and `ZERO_EXTEND` (NonNeg).
            let dropped = match self.dag.opcode(op) {
                ISD::Shl | ISD::Truncate | ISD::Add | ISD::Sub | ISD::Mul => super::dag::SDNodeFlags(
                    super::dag::SDNodeFlags::NO_UNSIGNED_WRAP.0 | super::dag::SDNodeFlags::NO_SIGNED_WRAP.0,
                ),
                ISD::Or => super::dag::SDNodeFlags::DISJOINT,
                ISD::ZeroExtend => super::dag::SDNodeFlags::NON_NEG,
                _ => super::dag::SDNodeFlags::default(),
            };
            self.dag.drop_flags(op, dropped);
        }
        result
    }

    fn sdb_inner(
        &mut self,
        op: SDValue,
        demanded_in: u128,
        depth: u32,
        assume_single_use: bool,
        tlo: &mut Tlo,
    ) -> Result<KnownBits, ()> {
        let vt = self.dag.value_type(op);
        let width = vt.size_in_bits();
        let all = mask(width);
        let unknown = KnownBits::unknown(width);
        if !vt.is_integer() || width > 64 || self.dag.is_undef(op) || self.dag.opcode(op) == ISD::TargetConstant {
            return Ok(unknown);
        }
        if let Some(bits) = self.dag.constant(op) {
            return Ok(KnownBits::constant(bits, width));
        }
        let mut demanded = demanded_in & all;
        if !assume_single_use && !self.dag.has_one_use(op.node) {
            if depth >= MAX_DEPTH {
                return Ok(unknown);
            }
            demanded = all;
        } else if demanded == 0 {
            let undef = self.dag.get_undef(vt);
            return Self::combine_to(tlo, op, undef);
        } else if depth >= MAX_DEPTH {
            return Ok(unknown);
        }
        let opcode = self.dag.opcode(op);
        let legal_operations = self.level >= CombineLevel::AfterLegalizeVectorOps;
        let mut known = match opcode {
            ISD::And => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                if let Some(c) = self.dag.constant(op1) {
                    let lhs = known_bits_at(self.dag, op0, depth);
                    if lhs.zero & demanded == !c & demanded {
                        return Self::combine_to(tlo, op, op0);
                    }
                    self.shrink_demanded_constant(op, !lhs.zero & demanded, tlo)?;
                    if tlo.is_some() {
                        return Err(());
                    }
                }
                let k1 = self.sdb(op1, demanded, depth + 1, false, tlo)?;
                let k0 = self.sdb(op0, !k1.zero & demanded, depth + 1, false, tlo)?;
                if demanded & !(k0.zero | k1.one) == 0 {
                    return Self::combine_to(tlo, op, op0);
                }
                if demanded & !(k1.zero | k0.one) == 0 {
                    return Self::combine_to(tlo, op, op1);
                }
                if demanded & !(k1.zero | k0.zero) == 0 {
                    let zero = self.dag.get_constant(0, vt);
                    return Self::combine_to(tlo, op, zero);
                }
                self.shrink_demanded_constant(op, !k0.zero & demanded, tlo)?;
                if tlo.is_some() {
                    return Err(());
                }
                self.shrink_demanded_op(op, demanded, tlo)?;
                if tlo.is_some() {
                    return Err(());
                }
                if demanded != all {
                    let (d0, d1) = (
                        self.simplify_multiple_use_demanded_bits(op0, demanded, depth + 1),
                        self.simplify_multiple_use_demanded_bits(op1, demanded, depth + 1),
                    );
                    if d0.is_some() || d1.is_some() {
                        let new = self.dag.get_node(opcode, vt, &[d0.unwrap_or(op0), d1.unwrap_or(op1)]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                KnownBits { zero: k0.zero | k1.zero, one: k0.one & k1.one, width }
            }
            ISD::Or => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let k1 = self.sdb(op1, demanded, depth + 1, false, tlo)?;
                let k0 = self.sdb(op0, !k1.one & demanded, depth + 1, false, tlo)?;
                if demanded & !(k0.one | k1.zero) == 0 {
                    return Self::combine_to(tlo, op, op0);
                }
                if demanded & !(k1.one | k0.zero) == 0 {
                    return Self::combine_to(tlo, op, op1);
                }
                self.shrink_demanded_constant(op, demanded, tlo)?;
                if tlo.is_some() {
                    return Err(());
                }
                self.shrink_demanded_op(op, demanded, tlo)?;
                if tlo.is_some() {
                    return Err(());
                }
                if demanded != all {
                    let (d0, d1) = (
                        self.simplify_multiple_use_demanded_bits(op0, demanded, depth + 1),
                        self.simplify_multiple_use_demanded_bits(op1, demanded, depth + 1),
                    );
                    if d0.is_some() || d1.is_some() {
                        let new = self.dag.get_node(opcode, vt, &[d0.unwrap_or(op0), d1.unwrap_or(op1)]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                KnownBits { zero: k0.zero & k1.zero, one: k0.one | k1.one, width }
            }
            ISD::Xor => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let k1 = self.sdb(op1, demanded, depth + 1, false, tlo)?;
                let k0 = self.sdb(op0, demanded, depth + 1, false, tlo)?;
                if demanded & !k1.zero == 0 {
                    return Self::combine_to(tlo, op, op0);
                }
                if demanded & !k0.zero == 0 {
                    return Self::combine_to(tlo, op, op1);
                }
                if demanded & !(k1.zero | k0.zero) == 0 {
                    let or = self.dag.get_node(ISD::Or, vt, &[op0, op1]);
                    return Self::combine_to(tlo, op, or);
                }
                self.shrink_demanded_op(op, demanded, tlo)?;
                if tlo.is_some() {
                    return Err(());
                }
                if let Some(c) = self.dag.constant(op1) {
                    if c == k0.one {
                        let masked = self.dag.get_constant(!c & demanded, vt);
                        let and = self.dag.get_node(ISD::And, vt, &[op0, masked]);
                        return Self::combine_to(tlo, op, and);
                    }
                    if c != all && demanded & !c == 0 {
                        let ones = self.dag.get_constant(all, vt);
                        let not = self.dag.get_node(ISD::Xor, vt, &[op0, ones]);
                        return Self::combine_to(tlo, op, not);
                    }
                }
                // "If we can't turn this into a 'not', try to shrink the
                // constant."
                if self.dag.constant(op1) != Some(all) {
                    self.shrink_demanded_constant(op, demanded, tlo)?;
                    if tlo.is_some() {
                        return Err(());
                    }
                }
                if demanded != all {
                    let (d0, d1) = (
                        self.simplify_multiple_use_demanded_bits(op0, demanded, depth + 1),
                        self.simplify_multiple_use_demanded_bits(op1, demanded, depth + 1),
                    );
                    if d0.is_some() || d1.is_some() {
                        let new = self.dag.get_node(opcode, vt, &[d0.unwrap_or(op0), d1.unwrap_or(op1)]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                KnownBits {
                    zero: (k0.zero & k1.zero) | (k0.one & k1.one),
                    one: (k0.zero & k1.one) | (k0.one & k1.zero),
                    width,
                }
            }
            ISD::Shl | ISD::Srl => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let Some(amount) = self.dag.constant(op1).filter(|&a| a < u128::from(width)) else {
                    return self.finish(op, demanded, known_bits_at(self.dag, op, depth), tlo);
                };
                let amount = amount as u32;
                if amount == 0 {
                    return Self::combine_to(tlo, op, op0);
                }
                let is_shl = opcode == ISD::Shl;
                let inner_op = if is_shl { ISD::Srl } else { ISD::Shl };
                // `(shl (srl x, c1), c2)` where the low `c2` bits are not
                // demanded is one shift by the difference (likewise `srl`).
                let outside = if is_shl { mask(amount) } else { all & !(all >> amount) };
                if self.dag.opcode(op0) == inner_op && demanded & outside == 0 {
                    if let Some(c1) = self.dag.constant(self.dag.operand(op0, 1)).filter(|&c| c < u128::from(width))
                        && (self.dag.has_one_use(op0.node) || self.hooks.fold_shift_of_shared_shift(vt, c1 as u32))
                    {
                        let diff = i64::from(amount) - c1 as i64;
                        let shifted = if is_shl { diff >= 0 } else { diff >= 0 };
                        let (new_op, by) = if shifted { (opcode, diff as u128) } else { (inner_op, (-diff) as u128) };
                        let by = self.dag.get_constant(by, self.dag.value_type(op1));
                        let x = self.dag.operand(op0, 0);
                        let new = self.dag.get_node(new_op, vt, &[x, by]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                // Shrinking the shift while the amount stays in range, as
                // `ShrinkDemandedOp` does (TargetLowering.cpp#L1938).
                if is_shl
                    && u128::from(amount) < u128::from(128 - demanded.leading_zeros())
                    && self.dag.has_one_use(op.node)
                {
                    let mut small = (128 - demanded.leading_zeros()).next_power_of_two();
                    while small < width {
                        let narrow = EVT::integer(small);
                        if self.hooks.narrowing_profitable(vt, narrow) && self.hooks.free_narrowing(vt, narrow) {
                            let x = self.dag.get_node(ISD::Truncate, narrow, &[op0]);
                            let by = self.dag.get_constant(u128::from(amount), self.hooks.shift_amount_type(narrow));
                            let shl = self.dag.get_node(ISD::Shl, narrow, &[x, by]);
                            let new = self.dag.get_node(ISD::AnyExtend, vt, &[shl]);
                            return Self::combine_to(tlo, op, new);
                        }
                        small *= 2;
                    }
                }
                let in_demanded = if is_shl { demanded >> amount } else { (demanded << amount) & all };
                // `srl` to the lower half (TargetLowering.cpp#L2080): the low
                // half of the source, shifted, zero-extended.
                if !is_shl && width % 2 == 0 {
                    let half = EVT::integer(width / 2);
                    let high = all & !mask(width / 2);
                    if self.hooks.narrowing_profitable(vt, half)
                        && self.hooks.free_narrowing(vt, half)
                        && (in_demanded.leading_zeros() as i64 - (128 - width as i64) >= i64::from(width / 2)
                            || known_bits_at(self.dag, op0, depth).zero & high == high)
                    {
                        let x = self.dag.get_node(ISD::Truncate, half, &[op0]);
                        let by = self.dag.get_constant(u128::from(amount), self.hooks.shift_amount_type(half));
                        let srl = self.dag.get_node(ISD::Srl, half, &[x, by]);
                        let new = self.dag.get_node(ISD::ZeroExtend, vt, &[srl]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                let k = self.sdb(op0, in_demanded, depth + 1, false, tlo)?;
                // `shl` to the lower half (TargetLowering.cpp#L1964): only
                // where the upper half is demanded and the source's upper half
                // is zero, i.e. the narrow shift is `nuw`: the *result's* upper
                // half is known zero, not the source's.
                if is_shl && width % 2 == 0 && amount < width / 2 {
                    let half = EVT::integer(width / 2);
                    let top_demanded = demanded & !mask(width / 2) == all & !mask(width / 2);
                    if top_demanded
                        && self.hooks.narrowing_profitable(vt, half)
                        && self.hooks.free_narrowing(vt, half)
                        && ((k.zero << amount) | mask(amount)) & (all & !mask(width / 2)) == all & !mask(width / 2)
                    {
                        let x = self.dag.get_node(ISD::Truncate, half, &[op0]);
                        let by = self.dag.get_constant(u128::from(amount), self.hooks.shift_amount_type(half));
                        let shl = self
                            .dag
                            .get_node_flags(ISD::Shl, half, &[x, by], super::dag::SDNodeFlags::NO_UNSIGNED_WRAP);
                        let new = self.dag.get_node(ISD::ZeroExtend, vt, &[shl]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                if in_demanded != all {
                    if let Some(d0) = self.simplify_multiple_use_demanded_bits(op0, in_demanded, depth + 1) {
                        let new = self.dag.get_node(opcode, vt, &[d0, op1]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                // "Try to match AVG patterns (after shift simplification)."
                if !is_shl {
                    if let Some(avg) = self.combine_shift_to_avg(op, demanded) {
                        return Self::combine_to(tlo, op, avg);
                    }
                }
                if is_shl {
                    KnownBits { zero: ((k.zero << amount) | mask(amount)) & all, one: (k.one << amount) & all, width }
                } else {
                    KnownBits { zero: (k.zero >> amount) | (all & !(all >> amount)), one: k.one >> amount, width }
                }
            }
            // `SimplifyDemandedBits` of `FSHL` and `FSHR`, the tail for a
            // variable amount (TargetLowering.cpp#L2326-L2331): at a
            // power-of-two width only the bottom `log2 width` bits of the
            // amount are demanded, so `fshl x, y, (and n, 31)` is `fshl x, y,
            // n` (the pin's `MaskedShlrdAmountPats` would then have nothing
            // left to drop). Not translated: the constant-amount split of the
            // demanded bits and `fshr` as `srl`.
            ISD::FShl | ISD::FShr => {
                let amount = self.dag.operand(op, 2);
                if self.dag.constant(amount).is_none() && width.is_power_of_two() {
                    self.sdb(amount, u128::from(width) - 1, depth + 1, false, tlo)?;
                }
                unknown
            }
            ISD::Rotl | ISD::Rotr => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                if let Some(c) = self.dag.constant(op1) {
                    let is_rotl = opcode == ISD::Rotl;
                    let amount = (c % u128::from(width)) as u32;
                    let reverse = width - amount;
                    // The operand's demanded bits are ours rotated back.
                    let back = if is_rotl { amount } else { reverse } % width;
                    let in_demanded =
                        if back == 0 { demanded } else { ((demanded >> back) | (demanded << (width - back))) & all };
                    self.sdb(op0, in_demanded, depth + 1, false, tlo)?;
                    if amount == 0 {
                        return Self::combine_to(tlo, op, op0);
                    }
                    let (left, right) = if is_rotl { (amount, reverse) } else { (reverse, amount) };
                    let by_type = self.dag.value_type(op1);
                    let shl_legal = !legal_operations || self.tli.is_some_and(|t| t.is_operation_legal(ISD::Shl, vt));
                    let srl_legal = !legal_operations || self.tli.is_some_and(|t| t.is_operation_legal(ISD::Srl, vt));
                    if shl_legal && demanded.trailing_zeros().min(width) >= left {
                        let by = self.dag.get_constant(u128::from(left), by_type);
                        let new = self.dag.get_node(ISD::Shl, vt, &[op0, by]);
                        return Self::combine_to(tlo, op, new);
                    }
                    let leading = width - (128 - demanded.leading_zeros()).min(width);
                    if srl_legal && leading >= right {
                        let by = self.dag.get_constant(u128::from(right), by_type);
                        let new = self.dag.get_node(ISD::Srl, vt, &[op0, by]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                unknown
            }
            ISD::ZeroExtend | ISD::AnyExtend => {
                let src = self.dag.operand(op, 0);
                let in_bits = self.dag.value_type(src).size_in_bits();
                if opcode == ISD::ZeroExtend && 128 - demanded.leading_zeros() <= in_bits {
                    let any_legal =
                        !legal_operations || self.tli.is_some_and(|t| t.is_operation_legal(ISD::AnyExtend, vt));
                    if any_legal {
                        let new = self.dag.get_node(ISD::AnyExtend, vt, &[src]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                let in_demanded = demanded & mask(in_bits);
                let k = self.sdb(src, in_demanded, depth + 1, false, tlo)?;
                if let Some(new_src) = self.simplify_multiple_use_demanded_bits(src, in_demanded, depth + 1) {
                    let new = self.dag.get_node(opcode, vt, &[new_src]);
                    return Self::combine_to(tlo, op, new);
                }
                let high = all & !mask(in_bits);
                KnownBits { zero: k.zero | if opcode == ISD::ZeroExtend { high } else { 0 }, one: k.one, width }
            }
            ISD::Truncate => {
                let src = self.dag.operand(op, 0);
                let src_width = self.dag.value_type(src).size_in_bits();
                if src_width > 64 {
                    return Ok(unknown);
                }
                let k = self.sdb(src, demanded, depth + 1, false, tlo)?;
                if let Some(new_src) = self.simplify_multiple_use_demanded_bits(src, demanded, depth + 1) {
                    let new = self.dag.get_node(ISD::Truncate, vt, &[new_src]);
                    return Self::combine_to(tlo, op, new);
                }
                KnownBits { zero: k.zero & all, one: k.one & all, width }
            }
            // `SimplifyDemandedBits` of `SIGN_EXTEND_INREG`: the bits above the
            // narrow type are copies of its sign bit.
            ISD::SignExtendInReg => {
                let src = self.dag.operand(op, 0);
                let super::dag::NodeData::ValueType(inner) = self.dag.node(self.dag.operand(op, 1)).data else {
                    return Ok(known_bits_at(self.dag, op, depth));
                };
                let inner_bits = inner.size_in_bits();
                let in_mask = mask(inner_bits);
                let sign = 1u128 << (inner_bits - 1);
                // None of the extended bits is demanded: the operand will do.
                if demanded & !in_mask == 0 && self.dag.value_type(src) == vt {
                    self.sdb(src, demanded, depth + 1, false, tlo)?;
                    return Self::combine_to(tlo, op, src);
                }
                let in_demanded = (demanded & in_mask) | if demanded & !in_mask != 0 { sign } else { 0 };
                let k = self.sdb(src, in_demanded, depth + 1, false, tlo)?;
                if let Some(new_src) = self.simplify_multiple_use_demanded_bits(src, in_demanded, depth + 1) {
                    let new = self.dag.get_node(ISD::SignExtendInReg, vt, &[new_src, self.dag.operand(op, 1)]);
                    return Self::combine_to(tlo, op, new);
                }
                let high = all & !in_mask;
                let (zero, one) = if k.zero & sign != 0 {
                    ((k.zero & in_mask) | high, k.one & in_mask)
                } else if k.one & sign != 0 {
                    (k.zero & in_mask, (k.one & in_mask) | high)
                } else {
                    (k.zero & in_mask & !sign, k.one & in_mask & !sign)
                };
                KnownBits { zero, one, width }
            }
            ISD::BSwap => {
                let src = self.dag.operand(op, 0);
                // One demanded byte of the swap is the source byte shifted into
                // place, and the swap goes.
                let lead = (width - (128 - demanded.leading_zeros()).min(width)) & !7;
                let trail = demanded.trailing_zeros().min(width) & !7;
                if width - lead - trail == 8 {
                    let (shift, by) = if lead > trail { (ISD::Srl, lead - trail) } else { (ISD::Shl, trail - lead) };
                    if !legal_operations || self.tli.is_some_and(|t| t.is_operation_legal(shift, vt)) {
                        let by = self.dag.get_constant(u128::from(by), EVT::I8);
                        let new = self.dag.get_node(shift, vt, &[src, by]);
                        return Self::combine_to(tlo, op, new);
                    }
                }
                let swap = |v: u128| v.swap_bytes() >> (128 - width);
                let k = self.sdb(src, swap(demanded), depth + 1, false, tlo)?;
                KnownBits { zero: swap(k.zero), one: swap(k.one), width }
            }
            // `SimplifyDemandedBits` of `SRA` (TargetLowering.cpp#L2146): a
            // shift that only moves sign bits into the demanded ones is
            // dropped; one whose demanded bits cannot come from the sign is a
            // logical shift. Not translated: the `shl`/`sra` pair as
            // `sign_extend_inreg`, the exact flag, the multi-use operand and
            // the average patterns.
            ISD::Sra => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                if num_sign_bits(self.dag, op0) >= width - demanded.trailing_zeros().min(width) {
                    return Self::combine_to(tlo, op, op0);
                }
                if demanded == 1 {
                    let new = self.dag.get_node(ISD::Srl, vt, &[op0, op1]);
                    return Self::combine_to(tlo, op, new);
                }
                let Some(amount) = self.dag.constant(op1).filter(|&a| a < u128::from(width)) else {
                    return self.finish(op, demanded, known_bits_at(self.dag, op, depth), tlo);
                };
                let amount = amount as u32;
                if amount == 0 {
                    return Self::combine_to(tlo, op, op0);
                }
                let leading = demanded.leading_zeros() - (128 - width);
                let sign = 1u128 << (width - 1);
                let from = ((demanded << amount) & all) | if leading < amount { sign } else { 0 };
                let k = self.sdb(op0, from, depth + 1, false, tlo)?;
                if k.zero & sign != 0 || leading >= amount {
                    let new = self.dag.get_node(ISD::Srl, vt, &[op0, op1]);
                    return Self::combine_to(tlo, op, new);
                }
                if demanded.is_power_of_two() {
                    let by = self
                        .dag
                        .get_constant(u128::from(width - 1 - demanded.trailing_zeros()), self.dag.value_type(op1));
                    let new = self.dag.get_node(ISD::Srl, vt, &[op0, by]);
                    return Self::combine_to(tlo, op, new);
                }
                let high = if k.one & sign != 0 { all & !(all >> amount) } else { 0 };
                KnownBits { zero: k.zero >> amount, one: (k.one >> amount) | high, width }
            }
            // `SimplifyDemandedBits` of `SELECT` and `SELECT_CC`
            // (TargetLowering.cpp#L1758-L1797): the arms are asked for the
            // same bits, and what is known is what both arms agree on. Not
            // translated: `ShrinkDemandedConstant` of the arms.
            ISD::Select | ISD::SelectCC => {
                let (a, b) = if opcode == ISD::Select { (1, 2) } else { (2, 3) };
                let (a, b) = (self.dag.operand(op, a), self.dag.operand(op, b));
                let known_b = self.sdb(b, demanded, depth + 1, false, tlo)?;
                let known_a = self.sdb(a, demanded, depth + 1, false, tlo)?;
                known_a.intersect(known_b)
            }
            // `SimplifyDemandedBits` of `ADD`, `SUB` and `MUL`: they demand
            // nothing above their highest demanded bit; a constant addend whose
            // low bits are all ones becomes -1.
            ISD::Add | ISD::Sub | ISD::Mul => {
                let (op0, op1) = (self.dag.operand(op, 0), self.dag.operand(op, 1));
                let length = 128 - demanded.leading_zeros();
                let low = mask(length);
                let k1 = self.sdb(op1, low, depth + 1, false, tlo)?;
                let lhs_demanded =
                    if opcode == ISD::Mul { low & mask(width - k1.zero.trailing_ones().min(width)) } else { low };
                self.sdb(op0, lhs_demanded, depth + 1, false, tlo)?;
                self.shrink_demanded_op(op, demanded, tlo)?;
                if tlo.is_some() {
                    return Err(());
                }
                if opcode != ISD::Mul && !self.dag.is_opaque(op1) {
                    if let Some(c) = self.dag.constant(op1) {
                        let high = all & !low;
                        if c != all && c != 1 && (c | high) == all {
                            let minus_one = self.dag.get_constant(all, vt);
                            let new = self.dag.get_node(opcode, vt, &[op0, minus_one]);
                            return Self::combine_to(tlo, op, new);
                        }
                    }
                }
                known_bits_at(self.dag, op, depth)
            }
            // `SimplifyDemandedBits` of `CTPOP` (TargetLowering.cpp#L2478): one
            // demanded bit is the parity, taken before operation legalization (this
            // DAG has no vector types: the pin's scalar-only limit holds).
            ISD::CtPop if demanded == 1 && !legal_operations => {
                let new = self.dag.get_node(ISD::Parity, vt, &[self.dag.operand(op, 0)]);
                return Self::combine_to(tlo, op, new);
            }
            _ => known_bits_at(self.dag, op, depth),
        };
        known.width = width;
        self.finish(op, demanded, known, tlo)
    }

    /// The end of `SimplifyDemandedBits`: a value whose demanded bits are all
    /// known is that constant.
    fn finish(
        &mut self,
        op: SDValue,
        demanded: u128,
        known: KnownBits,
        tlo: &mut Tlo,
    ) -> Result<KnownBits, ()> {
        // [L3175]: "Avoid folding to a constant if any OpaqueConstant is
        // involved": a hoisted constant (`bitcast iN C`) stays a register.
        //
        // [L3175]: https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L3175
        let opaque_operand = self.dag.node(op).ops.iter().any(|&operand| self.dag.is_opaque(operand));
        if demanded & !(known.zero | known.one) == 0 && self.dag.constant(op).is_none() && !opaque_operand {
            let vt = self.dag.value_type(op);
            let constant = self.dag.get_constant(known.one, vt);
            return Self::combine_to(tlo, op, constant);
        }
        Ok(known)
    }
}
