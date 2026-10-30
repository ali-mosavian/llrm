//! `PredicatedScalarEvolution`: a client's view of one loop's SCEVs that
//! accumulates the predicates it needed (P4 of
//! docs/working/scev-predicates-plan.md).
//!
//! # How it works
//! The view keeps a [`PredicateUnion`] of the run-time checks assumed so far
//! and a map from each expression it was asked for to that expression
//! rewritten under the union (`rewriteUsingPredicate`). Adding a predicate the
//! union does not already imply bumps a generation number, which makes every
//! mapped rewrite stale: it is rewritten again, from its last result, the next
//! time it is asked for. The backedge-taken counts are asked of the predicated
//! forms once, and the predicates they needed are added.
//!
//! # Deviations from LLVM
//! - D1: no `print` (the `[PSE]` lines of the loop printer): nothing prints a
//!   view yet.
//! - D2: the generation is a `u32` that wraps as the pin's `unsigned` does, and
//!   the wrap recomputes the map from each entry's last result, as the pin
//!   does.
//!
//! # Example
//! A loop `for (i16 i = 0; (i32)i + 1 < n; ++i)` has no count outright; asked
//! through the view, its count is `(-1 + (1 smax %n))` and the view holds the
//! predicate `{1,+,1}<%loop> Added Flags: <nssw>`.

use llrm_mir::hash::HashMap;
use llrm_mir::module::{BlockId, Operand};

use super::expr::Scev;
use super::wrap_predicate::{Predicate, PredicateUnion};
use super::{Cx, ScalarEvolution};

/// `PredicatedScalarEvolution`.
#[derive(Clone, Debug)]
pub struct PredicatedScalarEvolution {
    header: BlockId,
    preds: PredicateUnion,
    /// `RewriteMap`: the expression, the generation it was rewritten at and its
    /// rewrite.
    rewrite_map: HashMap<Scev, (u32, Scev)>,
    generation: u32,
    backedge_count: Option<Scev>,
    symbolic_max_backedge_count: Option<Scev>,
    small_constant_max_trip_count: Option<u32>,
}

impl PredicatedScalarEvolution {
    /// `PredicatedScalarEvolution(SE, L)`: no predicates yet.
    pub fn new(header: BlockId) -> Self {
        Self {
            header,
            preds: PredicateUnion::default(),
            rewrite_map: HashMap::default(),
            generation: 0,
            backedge_count: None,
            symbolic_max_backedge_count: None,
            small_constant_max_trip_count: None,
        }
    }

    /// `getPredicate`: the predicates assumed so far.
    pub fn predicate(&self) -> &PredicateUnion {
        &self.preds
    }

    /// `getSCEV(V)`.
    pub fn get_scev(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        value: Operand,
    ) -> Scev {
        let expr = se.get_scev(cx, value);
        self.get_predicated_scev(se, cx, expr)
    }

    /// `getPredicatedSCEV(Expr)`.
    pub fn get_predicated_scev(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        mut expr: Scev,
    ) -> Scev {
        let key = expr;
        // If we already have an entry and the version matches, return it.
        let entry = self.rewrite_map.get(&key).copied();
        if let Some((generation, rewrite)) = entry {
            if generation == self.generation {
                return rewrite;
            }
            // We found an entry but it's stale. Rewrite the stale entry
            // according to the current predicate.
            expr = rewrite;
        }
        let new = se.rewrite_using_predicate(cx, expr, self.header, &self.preds);
        self.rewrite_map.insert(key, (self.generation, new));
        new
    }

    /// `getBackedgeTakenCount`.
    pub fn get_backedge_taken_count(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) -> Scev {
        if let Some(found) = self.backedge_count {
            return found;
        }
        let mut preds = Vec::new();
        let count = se.get_predicated_backedge_taken_count(cx, self.header, &mut preds);
        self.backedge_count = Some(count);
        self.add_predicates(se, cx, &preds);
        count
    }

    /// `getSymbolicMaxBackedgeTakenCount`.
    pub fn get_symbolic_max_backedge_taken_count(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) -> Scev {
        if let Some(found) = self.symbolic_max_backedge_count {
            return found;
        }
        let mut preds = Vec::new();
        let count = se.get_predicated_symbolic_max_backedge_taken_count(cx, self.header, &mut preds);
        self.symbolic_max_backedge_count = Some(count);
        self.add_predicates(se, cx, &preds);
        count
    }

    /// `getSmallConstantMaxTripCount`.
    pub fn get_small_constant_max_trip_count(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) -> u32 {
        if let Some(found) = self.small_constant_max_trip_count {
            return found;
        }
        let mut preds = Vec::new();
        let count = se.get_small_constant_max_trip_count_with_predicates(cx, self.header, &mut preds);
        self.small_constant_max_trip_count = Some(count);
        self.add_predicates(se, cx, &preds);
        count
    }

    /// `addPredicate`.
    pub fn add_predicate(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        pred: Predicate,
    ) {
        if se.union_implies(cx, &self.preds, &pred) {
            return;
        }
        let mut new = self.preds.preds.clone();
        new.push(pred);
        let mut union = PredicateUnion::default();
        for one in new {
            se.union_add(cx, &mut union, one);
        }
        self.preds = union;
        self.update_generation(se, cx);
    }

    /// `addPredicates`.
    pub fn add_predicates(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        preds: &[Predicate],
    ) {
        for &one in preds {
            self.add_predicate(se, cx, one);
        }
    }

    /// `updateGeneration`.
    fn update_generation(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) {
        // If the generation number wrapped recompute everything.
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            let stale: Vec<(Scev, Scev)> =
                self.rewrite_map.iter().map(|(key, (_, rewrite))| (*key, *rewrite)).collect();
            for (key, rewritten) in stale {
                let new = se.rewrite_using_predicate(cx, rewritten, self.header, &self.preds);
                self.rewrite_map.insert(key, (0, new));
            }
        }
    }

    /// `getAsAddRec(V, ExtraPreds)`: the value as an add recurrence of the
    /// loop under predicates, which go to `extra` when given and are added to
    /// the view otherwise.
    pub fn get_as_add_rec(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        value: Operand,
        extra: Option<&mut Vec<Predicate>>,
    ) -> Option<Scev> {
        let expr = self.get_scev(se, cx, value);
        let mut new_preds = Vec::new();
        let new = se.convert_scev_to_add_rec_with_predicates(cx, expr, self.header, &mut new_preds)?;
        if let Some(extra) = extra {
            extra.extend(new_preds);
            return Some(new);
        }
        self.add_predicates(se, cx, &new_preds);
        let plain = se.get_scev(cx, value);
        self.rewrite_map.insert(plain, (self.generation, new));
        Some(new)
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::valuetracking::AssumptionCache;

    use super::*;
    use crate::scev::expr::Kind;
    use crate::scev::wrap_predicate::IncrementWrapFlags;

    const COUNTED: &str = "define void @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %i.next = add i16 %i, 1
  %ext = sext i16 %i.next to i32
  %c = icmp slt i32 %ext, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";

    /// `check` with the view of the loop of `COUNTED`, the expression set, and
    /// the SCEV of `%i.next`.
    fn with_view(check: impl FnOnce(&mut PredicatedScalarEvolution, &mut ScalarEvolution, &Cx, Operand)) {
        let module = llrm_mir::parse::module(COUNTED).expect("a module");
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
        let header = function.entry().and_then(|entry| function.successors(entry).first().copied()).expect("the loop");
        let value = function
            .walk()
            .map(|(_, inst)| inst)
            .find_map(|inst| {
                let result = function.instruction(inst).result?;
                (function.value(result).name.as_deref() == Some("ext")).then_some(Operand::Value(result))
            })
            .expect("%ext");
        let mut se = ScalarEvolution::new();
        let mut view = PredicatedScalarEvolution::new(header);
        check(&mut view, &mut se, &cx, value);
    }

    /// The view's count is the predicated one, the predicate it needed is held
    /// (not duplicated by a second ask), and the unpredicated count is not.
    #[test]
    fn the_view_holds_the_predicates_its_counts_needed() {
        with_view(|view, se, cx, _| {
            assert!(view.predicate().preds.is_empty());
            let count = view.get_backedge_taken_count(se, cx);
            assert!(!se.is_could_not_compute(count));
            let held = view.predicate().preds.clone();
            assert_eq!(held.len(), 1, "{held:?}");
            assert!(matches!(held[0], Predicate::Wrap(wrap) if wrap.flags == IncrementWrapFlags::NSSW));
            // Again: the cached count, no new predicate.
            assert_eq!(view.get_backedge_taken_count(se, cx), count);
            assert_eq!(view.get_symbolic_max_backedge_taken_count(se, cx), count);
            assert_eq!(view.predicate().preds, held);
        });
    }

    /// An added predicate makes the rewrites stale: the sext of the counter
    /// is rewritten to a recurrence only once the nssw check is assumed.
    #[test]
    fn an_added_predicate_makes_the_rewrite_of_an_extension_a_recurrence() {
        with_view(|view, se, cx, ext| {
            let before = view.get_scev(se, cx, ext);
            assert!(!matches!(se.node(before).kind, Kind::AddRec(..)), "premise: not a recurrence yet");
            view.get_backedge_taken_count(se, cx);
            let after = view.get_scev(se, cx, ext);
            assert!(matches!(se.node(after).kind, Kind::AddRec(..)), "{:?}", se.node(after).kind);
        });
    }

    /// The max trip count asked through the view adds what it needed too.
    #[test]
    fn the_small_max_trip_count_is_asked_with_predicates() {
        with_view(|view, se, cx, _| {
            let max = view.get_small_constant_max_trip_count(se, cx);
            assert_eq!(max, 2_147_483_647, "i32 2147483646 backedges, one more trip");
            assert!(!view.predicate().preds.is_empty());
        });
    }
}
