//! `DependenceInfo::depends` and what it needs: the classification of a
//! pair of subscripts, delinearization of the two addresses, and the
//! assembling of the answer.
//!
//! # How it works
//! See [`super`]. `classify_pair` fixes whether a pair of subscripts is ZIV,
//! SIV, RDIV, MIV or non-linear from the loops each side moves with;
//! `try_delinearize` splits the addresses of one array into per-dimension
//! pairs, the fixed-size heuristic first and the parametric one after.
//!
//! # Example
//! `a[i]` and `a[i + 1]` in one loop: one SIV pair, strong SIV with
//! `Delta = -1`, distance 1.

use llrm_mir::loopinfo::Loop;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_write_to_memory};

use super::{
    Classification, ComparePredicate, Dependence, DependenceInfo, FullDependence, Levels, Subscript, UnionPredicate,
    dir,
};
use crate::aa::{AAResults, AliasResult, LocationSize, MemoryLocation};
use crate::scev::expr::{Kind, Scev};
use crate::scev::{Cx, ScalarEvolution};

/// `getLoadStorePointerOperand`.
pub(crate) fn load_store_pointer(
    cx: &Cx,
    inst: InstId,
) -> llrm_mir::module::Operand {
    let instruction = cx.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => instruction.operands[0],
        Opcode::Store { .. } => instruction.operands[1],
        _ => unreachable!("a load or a store"),
    }
}

impl<'a, A: AAResults> DependenceInfo<'a, A> {
    /// `isLoopInvariant`: unlike `ScalarEvolution::isLoopInvariant` an access
    /// outside any loop counts as invariant, for the expression is evaluated at
    /// the access, not across the function. Invariant in the outermost loop of
    /// the nest is invariant anywhere in it.
    pub(crate) fn is_loop_invariant(
        &self,
        se: &mut ScalarEvolution,
        expression: Scev,
        loop_nest: Option<Loop<'a>>,
    ) -> bool {
        let Some(loop_nest) = loop_nest else { return true };
        se.is_loop_invariant(self.cx, expression, Some(loop_nest.get_outermost_loop().get_header()))
    }

    /// `collectCommonLoops`: the loops of the nest at a level up to
    /// `common_levels` that `expression` refers to.
    pub(crate) fn collect_common_loops(
        &self,
        se: &mut ScalarEvolution,
        expression: Scev,
        loop_nest: Option<Loop<'a>>,
        loops: &mut Levels,
    ) {
        let mut current = loop_nest;
        while let Some(one) = current {
            let level = one.get_loop_depth();
            if level <= self.common_levels && !se.is_loop_invariant(self.cx, expression, Some(one.get_header())) {
                loops.set(level);
            }
            current = one.get_parent_loop();
        }
    }

    /// `checkSubscript`: whether `expr` is affine; the loops it mentions are
    /// collected in `loops`.
    fn check_subscript(
        &self,
        se: &mut ScalarEvolution,
        expr: Scev,
        loop_nest: Option<Loop<'a>>,
        loops: &mut Levels,
        is_src: bool,
    ) -> bool {
        let Kind::AddRec(_, header) = se.node(expr).kind.clone() else {
            return self.is_loop_invariant(se, expr, loop_nest);
        };

        // The AddRec must depend on one of the containing loops. Otherwise,
        // mapSrcLoop and mapDstLoop return indices outside the intended range.
        // This can happen when a subscript in one loop references an IV from a
        // sibling loop that could not be replaced with a concrete exit value
        // by getSCEVAtScope.
        let mut containing = loop_nest;
        while let Some(one) = containing {
            if one.get_header() == header {
                break;
            }
            containing = one.get_parent_loop();
        }
        if containing.is_none() {
            return false;
        }

        if se.node(expr).flags & crate::scev::expr::flags::NSW == 0 {
            return false;
        }

        let start = se.addrec_start(expr);
        let step = se.get_step_recurrence(self.cx, expr);
        if !self.is_loop_invariant(se, step, loop_nest) {
            return false;
        }
        if is_src {
            loops.set(self.map_src_loop(header));
        } else {
            loops.set(self.map_dst_loop(header));
        }
        self.check_subscript(se, start, loop_nest, loops, is_src)
    }

    /// `classifyPair`: ZIV, SIV, RDIV, MIV or non-linear, with the loops the
    /// pair mentions.
    pub(crate) fn classify_pair(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        src_loop_nest: Option<Loop<'a>>,
        dst: Scev,
        dst_loop_nest: Option<Loop<'a>>,
    ) -> (Classification, Levels) {
        let mut src_loops = Levels::new(self.max_levels as usize + 1);
        let mut dst_loops = Levels::new(self.max_levels as usize + 1);
        if !self.check_subscript(se, src, src_loop_nest, &mut src_loops, true) {
            return (Classification::NonLinear, Levels::default());
        }
        if !self.check_subscript(se, dst, dst_loop_nest, &mut dst_loops, false) {
            return (Classification::NonLinear, Levels::default());
        }
        let mut loops = src_loops.clone();
        loops.or(&dst_loops);
        let n = loops.count();
        let kind = if n == 0 {
            Classification::Ziv
        } else if n == 1 {
            Classification::Siv
        } else if n == 2 && src_loops.count() == 1 && dst_loops.count() == 1 {
            Classification::Rdiv
        } else {
            Classification::Miv
        };
        (kind, loops)
    }

    /// `tryDelinearize`: split the addresses of one array into a pair of
    /// subscripts per dimension, where the two recurrences allow it.
    fn try_delinearize(
        &self,
        se: &mut ScalarEvolution,
        src: InstId,
        dst: InstId,
    ) -> Option<Vec<(Scev, Scev)>> {
        let cx = self.cx;
        debug_assert!(self.is_load_or_store(src) && self.is_load_or_store(dst), "instruction is not load or store");
        let src_ptr = load_store_pointer(cx, src);
        let dst_ptr = load_store_pointer(cx, dst);
        let src_loop = cx.loops.get_loop_for(self.block_of(src));
        let dst_loop = cx.loops.get_loop_for(self.block_of(dst));
        let src_access = se.get_scev_at_scope_of_value(cx, src_ptr, src_loop.map(|one| one.get_header())).scev;
        let dst_access = se.get_scev_at_scope_of_value(cx, dst_ptr, dst_loop.map(|one| one.get_header())).scev;
        let src_base = se.get_pointer_base(src_access);
        let dst_base = se.get_pointer_base(dst_access);

        if !matches!(se.node(src_base).kind, Kind::Unknown(_))
            || !matches!(se.node(dst_base).kind, Kind::Unknown(_))
            || src_base != dst_base
        {
            return None;
        }

        let (src_subscripts, dst_subscripts) =
            match self.try_delinearize_fixed_size(se, src, dst, src_access, dst_access) {
                Some(found) => found,
                None => self.try_delinearize_parametric_size(se, src, dst, src_access, dst_access)?,
            };

        debug_assert!(
            self.is_loop_invariant(se, src_base, src_loop) && self.is_loop_invariant(se, dst_base, dst_loop),
            "Expected SrcBase and DstBase to be loop invariant"
        );

        // The delinearization transforms a single-subscript MIV dependence
        // test into a multi-subscript SIV dependence test that is easier to
        // compute: initialize a pair of subscripts for each the
        // delinearization has found.
        Some(
            src_subscripts
                .into_iter()
                .zip(dst_subscripts)
                .inspect(|&(s, d)| {
                    debug_assert_eq!(se.type_of(s), se.type_of(d), "Unexpected different types for the subscripts")
                })
                .collect(),
        )
    }

    /// `tryDelinearizeFixedSize`: delinearize if the arrays accessed are
    /// fixed-size arrays.
    fn try_delinearize_fixed_size(
        &self,
        se: &mut ScalarEvolution,
        src: InstId,
        dst: InstId,
        src_access: Scev,
        dst_access: Scev,
    ) -> Option<(Vec<Scev>, Vec<Scev>)> {
        let cx = self.cx;
        let elem_size = se.get_element_size(cx, src);
        debug_assert_eq!(elem_size, se.get_element_size(cx, dst), "Different element sizes");
        let src_offset = se.remove_pointer_base(cx, src_access);
        let (src_subscripts, src_sizes) = se.delinearize_fixed_size_array(cx, src_offset, Some(elem_size));
        if src_subscripts.is_empty() {
            return None;
        }
        let dst_offset = se.remove_pointer_base(cx, dst_access);
        let (dst_subscripts, dst_sizes) = se.delinearize_fixed_size_array(cx, dst_offset, Some(elem_size));
        if dst_subscripts.is_empty() {
            return None;
        }

        // Check that the two size arrays are non-empty and equal in length and
        // value. SCEV expressions are uniqued, so we can compare ids.
        if src_sizes != dst_sizes {
            return None;
        }
        debug_assert_eq!(src_subscripts.len(), dst_subscripts.len());

        // In general we cannot safely assume that the subscripts recovered from
        // GEPs are in the range of values defined for their corresponding array
        // dimensions. As such we can only delinearize iff the subscripts are
        // positive and are less than the range of the dimension.
        if !self.options.disable_delinearization_checks
            && (!se.validate_delinearization_result(cx, &src_sizes, &src_subscripts)
                || !se.validate_delinearization_result(cx, &dst_sizes, &dst_subscripts))
        {
            return None;
        }
        Some((src_subscripts, dst_subscripts))
    }

    /// `tryDelinearizeParametricSize`.
    fn try_delinearize_parametric_size(
        &self,
        se: &mut ScalarEvolution,
        src: InstId,
        dst: InstId,
        src_access: Scev,
        dst_access: Scev,
    ) -> Option<(Vec<Scev>, Vec<Scev>)> {
        let cx = self.cx;
        let src_base = se.get_pointer_base(src_access);
        let dst_base = se.get_pointer_base(dst_access);
        debug_assert!(src_base == dst_base, "expected src and dst scev unknowns to be equal");

        let element_size = se.get_element_size(cx, src);
        if element_size != se.get_element_size(cx, dst) {
            return None;
        }

        let src_scev =
            se.get_minus_scev(cx, crate::scev::expr::Use::of(src_access), crate::scev::expr::Use::of(src_base), 0, 0);
        let dst_scev =
            se.get_minus_scev(cx, crate::scev::expr::Use::of(dst_access), crate::scev::expr::Use::of(dst_base), 0, 0);

        let is_affine_add_rec = |se: &ScalarEvolution, s: Scev| {
            matches!(
                &se.node(s).kind,
                Kind::AddRec(operands, _) if operands.len() == 2
            )
        };
        if !is_affine_add_rec(se, src_scev) || !is_affine_add_rec(se, dst_scev) {
            return None;
        }

        // First step: collect parametric terms in both array references.
        let mut terms = Vec::new();
        se.collect_parametric_terms(cx, src_scev, &mut terms);
        se.collect_parametric_terms(cx, dst_scev, &mut terms);

        // Second step: find subscript sizes.
        let mut sizes = Vec::new();
        se.find_array_dimensions(cx, &mut terms, &mut sizes, Some(element_size));

        // Third step: compute the access functions for each subscript.
        let mut src_subscripts = Vec::new();
        let mut dst_subscripts = Vec::new();
        se.compute_access_functions(cx, src_scev, &mut src_subscripts, &mut sizes);
        se.compute_access_functions(cx, dst_scev, &mut dst_subscripts, &mut sizes);

        // Fail when there is only a subscript: that's a linearized access
        // function.
        if src_subscripts.len() < 2 || dst_subscripts.len() < 2 || src_subscripts.len() != dst_subscripts.len() {
            return None;
        }

        // Statically check that the array bounds are in-range. The first
        // subscript we don't have a size for and it cannot overflow into
        // another subscript, so is always safe. The others need to be 0 <=
        // subscript[i] < bound, for both src and dst.
        if !self.options.disable_delinearization_checks
            && (!se.validate_delinearization_result(cx, &sizes, &src_subscripts)
                || !se.validate_delinearization_result(cx, &sizes, &dst_subscripts))
        {
            return None;
        }

        Some((src_subscripts, dst_subscripts))
    }

    /// `depends`: no dependence between `src` and `dst`, or a [`Dependence`]
    /// with as much detail as could be gleaned (Practical Dependence Testing,
    /// section 3.1). By default the tests collect no runtime assumptions; with
    /// `under_runtime_assumptions`, those that cannot be solved at compile time
    /// are returned in the dependence.
    pub fn depends(
        &mut self,
        se: &mut ScalarEvolution,
        src: InstId,
        dst: InstId,
        under_runtime_assumptions: bool,
    ) -> Option<Dependence> {
        let cx = self.cx;
        let mut assume: Vec<ComparePredicate> = Vec::new();
        let mut possibly_loop_independent = true;
        if src == dst {
            possibly_loop_independent = false;
        }

        let query = cx.query(None);
        let reads_or_writes = |inst| may_read_from_memory(&query, inst) || may_write_to_memory(&query, inst);
        if !(reads_or_writes(src) && reads_or_writes(dst)) {
            // If both instructions don't reference memory, there's no
            // dependence.
            return None;
        }

        let confused = |assume: &[ComparePredicate]| {
            Some(Dependence::Confused { src, dst, assumptions: UnionPredicate::of(assume) })
        };

        if !self.is_load_or_store(src) || !self.is_load_or_store(dst) {
            // Can only analyze simple loads and stores, i.e., no calls,
            // invokes, etc.
            return confused(&assume);
        }

        let dst_loc = MemoryLocation::get(self.unit, dst).expect("a load or a store");
        let src_loc = MemoryLocation::get(self.unit, src).expect("a load or a store");

        match self.underlying_objects_alias(&dst_loc, &src_loc) {
            // Cannot analyse objects if we don't understand their aliasing.
            AliasResult::MayAlias | AliasResult::PartialAlias { .. } => return confused(&assume),
            // If the objects noalias, they are distinct, accesses are
            // independent.
            AliasResult::NoAlias => return None,
            // The underlying objects alias; test accesses for dependence.
            AliasResult::MustAlias => {}
        }

        if dst_loc.size != src_loc.size || !dst_loc.size.is_precise() || !src_loc.size.is_precise() {
            // The dependence test gets confused if the size of the memory
            // accesses differ.
            return confused(&assume);
        }

        let src_ptr = load_store_pointer(cx, src);
        let dst_ptr = load_store_pointer(cx, dst);
        let src_scev = se.get_scev(cx, src_ptr);
        let dst_scev = se.get_scev(cx, dst_ptr);
        let src_base = se.get_pointer_base(src_scev);
        let dst_base = se.get_pointer_base(dst_scev);
        if src_base != dst_base {
            // If two pointers have different bases, trying to analyze indexes
            // won't work; we can't compare them to each other. This can
            // happen, for example, if one is produced by an LCSSA PHI node.
            return confused(&assume);
        }

        // Even if the base pointers are the same, they may not be
        // loop-invariant. It could lead to incorrect results, as we're
        // analyzing loop-carried dependencies. Src and Dst can be in different
        // loops, so we need to check the base pointer is invariant in both.
        let src_loop = cx.loops.get_loop_for(self.block_of(src));
        let dst_loop = cx.loops.get_loop_for(self.block_of(dst));
        if !self.is_loop_invariant(se, src_base, src_loop) || !self.is_loop_invariant(se, dst_base, dst_loop) {
            return confused(&assume);
        }

        let LocationSize::Precise(elt_size) = src_loc.size else { unreachable!("a precise size") };
        let src_ev =
            se.get_minus_scev(cx, crate::scev::expr::Use::of(src_scev), crate::scev::expr::Use::of(src_base), 0, 0);
        let dst_ev =
            se.get_minus_scev(cx, crate::scev::expr::Use::of(dst_scev), crate::scev::expr::Use::of(dst_base), 0, 0);

        // Check that memory access offsets are multiples of element sizes. Add
        // to Assume if only runtime assumptions are allowed.
        let src_multiple =
            se.is_known_multiple_of(cx, src_ev, elt_size, under_runtime_assumptions.then_some(&mut assume));
        let dst_multiple = src_multiple
            && se.is_known_multiple_of(cx, dst_ev, elt_size, under_runtime_assumptions.then_some(&mut assume));
        if !src_multiple || !dst_multiple {
            return confused(&assume);
        }

        let mut pairs = vec![(src_ev, dst_ev)];
        if self.options.delinearize
            && let Some(found) = self.try_delinearize(se, src, dst)
        {
            pairs = found;
        }
        let pair_count = pairs.len();

        // Establish loop nesting levels considering SameSD loops as common.
        self.establish_nesting_levels(se, src, dst);

        // Modify common levels to consider the SameSD levels in the tests.
        self.common_levels += self.same_sd_levels;
        self.max_levels -= self.same_sd_levels;
        if self.same_sd_levels > 0 {
            // Not all tests are handled yet over SameSD loops: revoke if there
            // are any tests other than ZIV, SIV or RDIV.
            for &(pair_src, pair_dst) in &pairs {
                let (class, _) = self.classify_pair(se, pair_src, src_loop, pair_dst, dst_loop);
                if !matches!(
                    class,
                    Classification::Ziv | Classification::Siv | Classification::Rdiv
                ) {
                    // Revert the levels to not consider the SameSD levels.
                    self.common_levels -= self.same_sd_levels;
                    self.max_levels += self.same_sd_levels;
                    self.same_sd_levels = 0;
                    break;
                }
            }
        }

        let mut result =
            FullDependence::new(src, dst, UnionPredicate::of(&assume), possibly_loop_independent, self.common_levels);

        let mut subscripts: Vec<Subscript> = Vec::with_capacity(pair_count);
        for &(pair_src, pair_dst) in &pairs {
            let (classification, mut loops) = self.classify_pair(se, pair_src, src_loop, pair_dst, dst_loop);
            loops.resize(self.max_levels as usize + 1);
            subscripts.push(Subscript { src: pair_src, dst: pair_dst, classification, loops });
        }

        // Test each subscript individually.
        for subscript in &mut subscripts {
            // Attempt signed range test first.
            let src_range = se.get_signed_range(cx, subscript.src);
            let dst_range = se.get_signed_range(cx, subscript.dst);
            if src_range
                .intersect_with(&dst_range, llrm_mir::constantrange::PreferredRangeType::Smallest)
                .is_empty_set()
            {
                return None;
            }

            match subscript.classification {
                Classification::NonLinear => {
                    // Ignore these, but collect loops for later.
                    let (s, d) = (subscript.src, subscript.dst);
                    self.collect_common_loops(se, s, src_loop, &mut subscript.loops);
                    self.collect_common_loops(se, d, dst_loop, &mut subscript.loops);
                }
                Classification::Ziv => {
                    if self.test_ziv(se, subscript.src, subscript.dst) {
                        return None;
                    }
                }
                Classification::Siv => {
                    let (disproved, _) =
                        self.test_siv(se, subscript.src, subscript.dst, &mut result, under_runtime_assumptions);
                    if disproved {
                        return None;
                    }
                }
                Classification::Rdiv => {
                    if self.test_rdiv(se, subscript.src, subscript.dst, &mut result) {
                        return None;
                    }
                }
                Classification::Miv => {
                    if self.test_miv(se, subscript.src, subscript.dst, &subscript.loops, &mut result) {
                        return None;
                    }
                }
            }
        }

        // Make sure the Scalar flags are set correctly.
        let mut complete_loops = Levels::new(self.max_levels as usize + 1);
        for subscript in &subscripts {
            complete_loops.or(&subscript.loops);
        }
        for level in 1..=self.common_levels {
            if complete_loops.get(level) {
                result.dv[(level - 1) as usize].scalar = false;
            }
        }

        // Set the distance to zero if the direction is EQ.
        for level in 1..=result.levels {
            if result.direction(level, false) == dir::EQ {
                let entry = &mut result.dv[(level - 1) as usize];
                match entry.distance {
                    None => entry.distance = Some(se.get_zero(cx, se.type_of(src_scev))),
                    Some(distance) => {
                        debug_assert!(se.is_zero_scev(distance), "Inconsistency between distance and direction")
                    }
                }
            }
        }

        if self.same_sd_levels > 0 {
            // Extracting SameSD levels from the common levels. Reverting
            // CommonLevels and MaxLevels to their original values.
            debug_assert!(self.common_levels >= self.same_sd_levels);
            self.common_levels -= self.same_sd_levels;
            self.max_levels += self.same_sd_levels;
            let common = self.common_levels as usize;
            let same_sd = self.same_sd_levels as usize;
            result.dv_same_sd = result.dv[common..common + same_sd].to_vec();
            result.dv.truncate(common);
            result.levels = self.common_levels;
            result.same_sd_levels = self.same_sd_levels;
        }

        if possibly_loop_independent {
            // Make sure the LoopIndependent flag is set correctly. All
            // directions must include equal, otherwise no loop-independent
            // dependence is possible.
            for level in 1..=self.common_levels {
                if result.direction(level, false) & dir::EQ == 0 {
                    result.loop_independent = false;
                    break;
                }
            }
        } else {
            // On the other hand, if all directions are equal and there's no
            // loop-independent dependence possible, then no dependence exists.
            // However, if there are runtime assumptions, we must return the
            // result.
            let mut all_equal = true;
            for level in 1..=self.common_levels {
                if result.direction(level, false) != dir::EQ {
                    all_equal = false;
                    break;
                }
            }
            if all_equal && result.assumptions.preds.is_empty() {
                return None;
            }
        }

        Some(Dependence::Full(result))
    }
}
