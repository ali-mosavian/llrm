//! `SCEVWrapPredicate` and the predicate sum `SCEVPredicate` that holds it
//! beside `SCEVComparePredicate`, with `SCEVUnionPredicate`'s `implies`,
//! `add` and `isAlwaysTrue` over both (P1 of
//! docs/working/scev-predicates-plan.md).
//!
//! # How it works
//! A [`WrapPredicate`] says an add recurrence's increment `{start,+,step}` is
//! checked at run time not to wrap unsigned (`nusw`) or signed (`nssw`).
//! [`PredicateUnion`] is the list of predicates a client has assumed, none
//! implied by another. `getEqualPredicate`/`getComparePredicate`/
//! `getWrapPredicate` unique their predicates: here a predicate is a value
//! over interned expressions, so structural equality is the pin's pointer
//! equality.
//!
//! # Deviations from LLVM
//! - D1: [`PredicateUnion::implies`] lacks the last clause of
//!   `SCEVUnionPredicate::implies` (a wrap predicate implied after rewriting
//!   its recurrence by the equal predicates) until the predicate rewriter (P2,
//!   2026-12-11) exists; it answers false there.
//! - D2: `depanalysis` still holds its assumptions in
//!   [`UnionPredicate`](super::compare_predicate::UnionPredicate), compare
//!   predicates only; it moves to this union with P4.
//!
//! # Example
//! `{0,+,4}<nusw>` implies `{0,+,2}<nusw>`: both steps are positive, the
//! implied one's start and step are `ule` this one's.

use llrm_mir::opcode::IntPredicate;

use super::compare_predicate::ComparePredicate;
use super::expr::{Kind, Scev, flags};
use super::{Cx, ScalarEvolution};

/// `SCEVWrapPredicate::IncrementWrapFlags`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncrementWrapFlags(pub u8);

impl IncrementWrapFlags {
    /// `IncrementAnyWrap`: the increment may wrap.
    pub const ANY_WRAP: Self = Self(0);
    /// `IncrementNUSW`: no unsigned wrap of the increment.
    pub const NUSW: Self = Self(1);
    /// `IncrementNSSW`: no signed wrap of the increment.
    pub const NSSW: Self = Self(2);
    const MASK: u8 = 3;

    /// `SCEVWrapPredicate::setFlags`.
    pub fn set(
        self,
        flags: Self,
    ) -> Self {
        Self((self.0 | flags.0) & Self::MASK)
    }

    /// `SCEVWrapPredicate::clearFlags`.
    pub fn clear(
        self,
        flags: Self,
    ) -> Self {
        Self(self.0 & !(flags.0 & Self::MASK))
    }

    /// `SCEVWrapPredicate::maskFlags`.
    pub fn mask(
        self,
        flags: Self,
    ) -> Self {
        Self(self.0 & flags.0 & Self::MASK)
    }
}

/// `SCEVWrapPredicate`: the increment of `ar` has the wrap `flags` checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapPredicate {
    pub ar: Scev,
    pub flags: IncrementWrapFlags,
}

/// `SCEVPredicate`, but a union: a compare predicate or a wrap predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Predicate {
    Compare(ComparePredicate),
    Wrap(WrapPredicate),
}

/// `SCEVUnionPredicate`: the predicates, none implied by another.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PredicateUnion {
    pub preds: Vec<Predicate>,
}

impl ScalarEvolution {
    /// `getEqualPredicate`.
    pub fn get_equal_predicate(
        &self,
        lhs: Scev,
        rhs: Scev,
    ) -> Predicate {
        self.get_compare_predicate(IntPredicate::Eq, lhs, rhs)
    }

    /// `getComparePredicate`.
    pub fn get_compare_predicate(
        &self,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> Predicate {
        debug_assert!(self.type_of(lhs) == self.type_of(rhs), "Type mismatch between LHS and RHS");
        Predicate::Compare(ComparePredicate { pred, lhs, rhs })
    }

    /// `getWrapPredicate`.
    pub fn get_wrap_predicate(
        &self,
        ar: Scev,
        added: IncrementWrapFlags,
    ) -> Predicate {
        debug_assert!(matches!(self.node(ar).kind, Kind::AddRec(..)), "an add recurrence");
        Predicate::Wrap(WrapPredicate { ar, flags: added })
    }

    /// `SCEVWrapPredicate::isAlwaysTrue`.
    pub fn wrap_predicate_is_always_true(
        &self,
        pred: &WrapPredicate,
    ) -> bool {
        let mut increment = pred.flags;
        if self.node(pred.ar).flags & flags::NSW != 0 {
            increment = increment.clear(IncrementWrapFlags::NSSW);
        }
        increment == IncrementWrapFlags::ANY_WRAP
    }

    /// `SCEVWrapPredicate::implies`.
    pub fn wrap_predicate_implies(
        &mut self,
        cx: &Cx,
        this: &WrapPredicate,
        other: &WrapPredicate,
    ) -> bool {
        if this.flags.set(other.flags) != this.flags {
            return false;
        }
        if other.ar == this.ar {
            return true;
        }
        if this.flags != IncrementWrapFlags::NSSW && this.flags != IncrementWrapFlags::NUSW {
            return false;
        }
        let (mut start, mut other_start) = (self.operands_of(this.ar)[0].scev, self.operands_of(other.ar)[0].scev);
        if self.type_of(start).is_pointer() != self.type_of(other_start).is_pointer() {
            return false;
        }
        // Reject pointers to different address spaces.
        if self.type_of(start).is_pointer() && self.type_of(start) != self.type_of(other_start) {
            return false;
        }
        // NUSW/NSSW on a wider-type AddRec does not imply the same on a
        // narrower-type AddRec.
        if self.get_type_size_in_bits(cx, self.type_of(this.ar))
            > self.get_type_size_in_bits(cx, self.type_of(other.ar))
        {
            return false;
        }
        let mut step = self.get_step_recurrence(cx, this.ar);
        let mut other_step = self.get_step_recurrence(cx, other.ar);
        if !self.is_known_positive(cx, step) || !self.is_known_positive(cx, other_step) {
            return false;
        }
        // If both steps are positive, this implies N, if N's start and step are
        // ULE/SLE (for NSUW/NSSW) than this'.
        let wider = self.get_wider_type(cx, self.type_of(step), self.type_of(other_step));
        step = self.get_noop_or_zero_extend(cx, step, wider);
        other_step = self.get_noop_or_zero_extend(cx, other_step, wider);
        let unsigned = this.flags == IncrementWrapFlags::NUSW;
        let extend = |se: &mut Self, value: Scev| {
            if unsigned {
                se.get_noop_or_zero_extend(cx, value, wider)
            } else {
                se.get_noop_or_sign_extend(cx, value, wider)
            }
        };
        other_start = extend(self, other_start);
        start = extend(self, start);
        let pred = if unsigned { IntPredicate::Ule } else { IntPredicate::Sle };
        self.is_known_predicate_scev(cx, pred, other_step, step)
            && self.is_known_predicate_scev(cx, pred, other_start, start)
    }

    /// `SCEVPredicate::implies` of the one `this` is.
    pub fn predicate_implies(
        &mut self,
        cx: &Cx,
        this: &Predicate,
        other: &Predicate,
    ) -> bool {
        match (this, other) {
            (Predicate::Compare(this), Predicate::Compare(other)) => this.implies(other),
            (Predicate::Wrap(this), Predicate::Wrap(other)) => self.wrap_predicate_implies(cx, this, other),
            _ => false,
        }
    }

    /// `SCEVPredicate::isAlwaysTrue`.
    pub fn predicate_is_always_true(
        &self,
        pred: &Predicate,
    ) -> bool {
        match pred {
            Predicate::Compare(_) => false,
            Predicate::Wrap(wrap) => self.wrap_predicate_is_always_true(wrap),
        }
    }

    /// `SCEVUnionPredicate::isAlwaysTrue`.
    pub fn union_is_always_true(
        &self,
        union: &PredicateUnion,
    ) -> bool {
        union.preds.iter().all(|one| self.predicate_is_always_true(one))
    }

    /// `SCEVUnionPredicate::implies`.
    pub fn union_implies(
        &mut self,
        cx: &Cx,
        union: &PredicateUnion,
        other: &Predicate,
    ) -> bool {
        if union.preds.iter().any(|one| self.predicate_implies(cx, one, other)) {
            return true;
        }
        // A wrap predicate may be implied by a wrap predicate in the union
        // after applying the equal predicates.
        let Predicate::Wrap(wanted) = other else { return false };
        let Kind::AddRec(_, header) = self.node(wanted.ar).kind else { return false };
        for one in &union.preds {
            let Predicate::Wrap(held) = one else { continue };
            let rewritten = self.rewrite_using_predicate(cx, held.ar, header, union);
            if !matches!(self.node(rewritten).kind, Kind::AddRec(..)) {
                continue;
            }
            let Predicate::Wrap(again) = self.get_wrap_predicate(rewritten, held.flags) else { unreachable!() };
            if self.wrap_predicate_implies(cx, &again, wanted) {
                return true;
            }
        }
        false
    }

    /// `SCEVUnionPredicate::add`.
    pub fn union_add(
        &mut self,
        cx: &Cx,
        union: &mut PredicateUnion,
        other: Predicate,
    ) {
        // Implication checks are quadratic in the number of predicates. Stop
        // doing them if there are many predicates, as they should be too
        // expensive to use anyway at that point.
        let check_implies = union.preds.len() < 16;
        // Only add predicate if it is not already implied by this union
        // predicate.
        if check_implies && self.union_implies(cx, union, &other) {
            return;
        }
        // Build a new vector containing the current predicates, except the ones
        // that are implied by the new predicate.
        let mut pruned = Vec::with_capacity(union.preds.len() + 1);
        for one in std::mem::take(&mut union.preds) {
            if check_implies && self.predicate_implies(cx, &other, &one) {
                continue;
            }
            pruned.push(one);
        }
        pruned.push(other);
        union.preds = pruned;
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::module::Operand;
    use llrm_mir::valuetracking::AssumptionCache;

    use super::*;

    const LOOP: &str = "define void @f(i32 %n) {
entry:
  br label %loop
loop:
  %a = phi i32 [ 0, %entry ], [ %a.next, %loop ]
  %b = phi i32 [ 0, %entry ], [ %b.next, %loop ]
  %c = phi i64 [ 0, %entry ], [ %c.next, %loop ]
  %d = phi i32 [ 4, %entry ], [ %d.next, %loop ]
  %a.next = add i32 %a, 4
  %b.next = add i32 %b, 2
  %c.next = add i64 %c, 4
  %d.next = add i32 %d, 4
  %t = icmp ne i32 %a.next, %n
  br i1 %t, label %loop, label %exit
exit:
  ret void
}
";

    /// The recurrences of %a, %b, %c and %d of `LOOP`, handed to `check` with
    /// the expression set that made them.
    fn with_recurrences(check: impl FnOnce(&mut ScalarEvolution, &Cx, [Scev; 4])) {
        let module = llrm_mir::parse::module(LOOP).expect("a module");
        let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("@f");
        let layout = DataLayout::default();
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(&module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let mut se = ScalarEvolution::new();
        let named = |name: &str| {
            function
                .walk()
                .map(|(_, inst)| inst)
                .find_map(|inst| {
                    let result = function.instruction(inst).result?;
                    (function.value(result).name.as_deref() == Some(name)).then_some(Operand::Value(result))
                })
                .expect(name)
        };
        let recurrences = ["a", "b", "c", "d"].map(|name| se.get_scev(&cx, named(name)));
        check(&mut se, &cx, recurrences);
    }

    #[test]
    fn the_increment_flags_combine_as_the_header_says() {
        use IncrementWrapFlags as F;
        assert_eq!(F::NUSW.set(F::NSSW), F(3));
        assert_eq!(F(3).clear(F::NUSW), F::NSSW);
        assert_eq!(F(3).mask(F::NSSW), F::NSSW);
        assert_eq!(F::ANY_WRAP.set(F::ANY_WRAP), F::ANY_WRAP);
    }

    /// `{0,+,4}<nusw>` implies `{0,+,2}<nusw>` (positive steps, the implied
    /// start and step `ule`), not the reverse, not `nssw`, and a wider
    /// recurrence implies no narrower one.
    #[test]
    fn a_wrap_predicate_implies_the_one_with_smaller_positive_start_and_step() {
        with_recurrences(|se, cx, [a, b, c, d]| {
            let nusw = |se: &ScalarEvolution, ar| match se.get_wrap_predicate(ar, IncrementWrapFlags::NUSW) {
                Predicate::Wrap(wrap) => wrap,
                other => panic!("{other:?}"),
            };
            let (pa, pb, pc, pd) = (nusw(se, a), nusw(se, b), nusw(se, c), nusw(se, d));
            assert!(se.wrap_predicate_implies(cx, &pa, &pa));
            assert!(se.wrap_predicate_implies(cx, &pa, &pb), "{{0,+,4}} implies {{0,+,2}}");
            assert!(!se.wrap_predicate_implies(cx, &pb, &pa), "{{0,+,2}} does not imply {{0,+,4}}");
            assert!(!se.wrap_predicate_implies(cx, &pc, &pa), "a wider recurrence implies no narrower one");
            assert!(!se.wrap_predicate_implies(cx, &pa, &pd), "a start of 4 is not ule a start of 0");
            let nssw = WrapPredicate { ar: a, flags: IncrementWrapFlags::NSSW };
            assert!(!se.wrap_predicate_implies(cx, &pa, &nssw), "nusw does not imply nssw");
        });
    }

    /// An increment already proved `nsw` needs no `nssw` check; no check is
    /// needed for none asked.
    #[test]
    fn a_wrap_predicate_is_always_true_when_its_flags_hold() {
        with_recurrences(|se, _, [a, ..]| {
            let nsw = se.node(a).flags & flags::NSW != 0;
            let nssw = WrapPredicate { ar: a, flags: IncrementWrapFlags::NSSW };
            let any = WrapPredicate { ar: a, flags: IncrementWrapFlags::ANY_WRAP };
            let nusw = WrapPredicate { ar: a, flags: IncrementWrapFlags::NUSW };
            assert!(se.wrap_predicate_is_always_true(&any));
            assert!(!se.wrap_predicate_is_always_true(&nusw));
            assert_eq!(se.wrap_predicate_is_always_true(&nssw), nsw);
        });
    }

    /// `SCEVUnionPredicate::add` drops a predicate an earlier one implies and
    /// prunes the ones a later one implies.
    #[test]
    fn a_union_keeps_no_predicate_another_implies() {
        with_recurrences(|se, cx, [a, b, ..]| {
            let (big, small) = (
                se.get_wrap_predicate(a, IncrementWrapFlags::NUSW),
                se.get_wrap_predicate(b, IncrementWrapFlags::NUSW),
            );
            let mut union = PredicateUnion::default();
            se.union_add(cx, &mut union, big);
            se.union_add(cx, &mut union, small);
            assert_eq!(union.preds, [big], "the implied predicate is not added");
            let mut union = PredicateUnion::default();
            se.union_add(cx, &mut union, small);
            se.union_add(cx, &mut union, big);
            assert_eq!(union.preds, [big], "the implying predicate replaces the implied");
            assert!(!se.union_is_always_true(&union));
            assert!(se.union_is_always_true(&PredicateUnion::default()));
        });
    }
}
