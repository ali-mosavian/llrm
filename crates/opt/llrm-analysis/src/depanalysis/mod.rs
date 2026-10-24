//! `DependenceAnalysis`: whether, and how, two memory accesses of a function
//! depend on one another, `DependenceAnalysis.cpp` at `d1106deb71cc`.
//!
//! # How it works
//! [`DependenceInfo::depends`] takes two loads or stores and answers no
//! dependence, or a [`Dependence`]: confused (the accesses are not simple, may
//! alias, or have different sizes) or full, with a direction and distance for
//! each loop the two share (`DVEntry`). Alias analysis decides first
//! (different identified objects never depend); the two addresses are then
//! split into one subscript per array dimension where delinearization can
//! ([`ScalarEvolution::delinearize`], the fixed-size heuristic first), and each
//! pair of subscripts is classified by the loops it moves with:
//! zero (ZIV), one (SIV), two with one on each side (RDIV), or more (MIV). The
//! tests of Goff, Kennedy and Tseng, "Practical Dependence Testing", prove
//! independence or narrow the vector: strong, weak-zero and weak-crossing SIV
//! and the exact SIV and RDIV tests ([`siv`]), GCD and Banerjee for MIV
//! ([`miv`]).
//!
//! # Deviations from LLVM
//! - D1: widths: `Ap` stops at 128 bits, so Banerjee's `2 * B + L + 1`-bit
//!   arithmetic is not run where that is wider; the test is off by default in
//!   the pin (`-da-enable-dependence-test`).
//! - D2: `SCEVUnionPredicate` is a list of `SCEVComparePredicate`, the only
//!   kind the tests add ([`crate::scev::compare_predicate`]).
//! - D3: loop-invariant `SCEVUnknown` bases are compared as SCEVs, as LLVM
//!   does; there is no `DependenceGraph`.
//!
//! # Example
//! `a[i + 1] = a[i]`: the store is the source of a flow dependence of
//! distance 1, `[1]`; `a[i] = a[i]` is independent across iterations, `[0|<]`.

pub mod depends;
pub mod miv;
pub mod overflow;
pub mod print;
pub mod siv;
#[cfg(test)]
mod tests;

use std::fmt::Write as _;

use llrm_mir::module::{BlockId, InstId};
use llrm_mir::opcode::{IntPredicate, Opcode};

use crate::aa::{AAResults, AliasResult, BatchAAResults, LocationSize, MemoryLocation};
use crate::basicaa::BasicAA;
use crate::memory::Unit;
pub use crate::scev::compare_predicate::{ComparePredicate, UnionPredicate};
use crate::scev::expr::{Kind, Scev};
use crate::scev::print::Labels;
use crate::scev::{Cx, ScalarEvolution};

/// `Dependence::DVEntry` directions, as bits.
pub mod dir {
    pub const NONE: u8 = 0;
    pub const LT: u8 = 1;
    pub const EQ: u8 = 2;
    pub const LE: u8 = 3;
    pub const GT: u8 = 4;
    pub const NE: u8 = 5;
    pub const GE: u8 = 6;
    pub const ALL: u8 = 7;
}

/// `Dependence::DVEntry`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DvEntry {
    /// Init to ALL, then refine.
    pub direction: u8,
    /// Init to true.
    pub scalar: bool,
    /// None implies no distance available.
    pub distance: Option<Scev>,
}

impl Default for DvEntry {
    fn default() -> Self {
        Self { direction: dir::ALL, scalar: true, distance: None }
    }
}

/// `CmpInst::Predicate`'s name, which `SCEVComparePredicate::print` writes.
pub(crate) fn predicate_name(pred: IntPredicate) -> &'static str {
    match pred {
        IntPredicate::Eq => "eq",
        IntPredicate::Ne => "ne",
        IntPredicate::Ugt => "ugt",
        IntPredicate::Uge => "uge",
        IntPredicate::Ult => "ult",
        IntPredicate::Ule => "ule",
        IntPredicate::Sgt => "sgt",
        IntPredicate::Sge => "sge",
        IntPredicate::Slt => "slt",
        IntPredicate::Sle => "sle",
    }
}

/// `FullDependence`: a dependence the analysis understands.
#[derive(Clone, Debug)]
pub struct FullDependence {
    pub src: InstId,
    pub dst: InstId,
    pub assumptions: UnionPredicate,
    pub levels: u32,
    pub same_sd_levels: u32,
    pub loop_independent: bool,
    pub dv: Vec<DvEntry>,
    /// DV entries on SameSD levels.
    pub dv_same_sd: Vec<DvEntry>,
}

impl FullDependence {
    /// `FullDependence(Source, Destination, Assumes, PossiblyLoopIndependent,
    /// CommonLevels)`.
    pub fn new(
        src: InstId,
        dst: InstId,
        assumptions: UnionPredicate,
        possibly_loop_independent: bool,
        common_levels: u32,
    ) -> Self {
        Self {
            src,
            dst,
            assumptions,
            levels: common_levels,
            same_sd_levels: 0,
            loop_independent: possibly_loop_independent,
            dv: vec![DvEntry::default(); common_levels as usize],
            dv_same_sd: Vec::new(),
        }
    }

    /// `getDVEntry`.
    pub fn dv_entry(
        &self,
        level: u32,
        is_same_sd: bool,
    ) -> DvEntry {
        if !is_same_sd {
            debug_assert!(0 < level && level <= self.levels, "Level out of range");
            self.dv[(level - 1) as usize]
        } else {
            debug_assert!(
                self.levels < level && level <= self.levels + self.same_sd_levels,
                "isSameSD level out of range"
            );
            self.dv_same_sd[(level - self.levels - 1) as usize]
        }
    }

    /// `getDirection`.
    pub fn direction(
        &self,
        level: u32,
        is_same_sd: bool,
    ) -> u8 {
        self.dv_entry(level, is_same_sd).direction
    }

    /// `getDistance`.
    pub fn distance(
        &self,
        level: u32,
        is_same_sd: bool,
    ) -> Option<Scev> {
        self.dv_entry(level, is_same_sd).distance
    }

    /// `isScalar`.
    pub fn is_scalar(
        &self,
        level: u32,
        is_same_sd: bool,
    ) -> bool {
        self.dv_entry(level, is_same_sd).scalar
    }

    /// `inSameSDLoops`: whether the level is across two separate loop nests
    /// that have the same iteration space and depth.
    pub fn in_same_sd_loops(
        &self,
        level: u32,
    ) -> bool {
        debug_assert!(0 < level && level <= self.levels + self.same_sd_levels, "Level out of range");
        level > self.levels
    }

    /// `isDirectionNegative`: whether the first direction that is not `=` is
    /// `>` or `>=`.
    pub fn is_direction_negative(&self) -> bool {
        for entry in &self.dv[..self.levels as usize] {
            if entry.direction == dir::EQ {
                continue;
            }
            return entry.direction == dir::GT || entry.direction == dir::GE;
        }
        false
    }

    /// `negate`: swap the source and the destination, reversing directions
    /// and distances.
    pub fn negate(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) {
        std::mem::swap(&mut self.src, &mut self.dst);
        for entry in &mut self.dv[..self.levels as usize] {
            let direction = entry.direction;
            // Reverse the direction vector, this means LT becomes GT and GT
            // becomes LT.
            let mut reversed = direction & dir::EQ;
            if direction & dir::LT != 0 {
                reversed |= dir::GT;
            }
            if direction & dir::GT != 0 {
                reversed |= dir::LT;
            }
            entry.direction = reversed;
            // Reverse the dependence distance as well.
            if let Some(distance) = entry.distance {
                entry.distance = Some(se.get_negative_scev(cx, distance, 0));
            }
        }
    }

    /// `normalize`: reverse a negative direction vector; whether it did.
    pub fn normalize(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) -> bool {
        if !self.is_direction_negative() {
            return false;
        }
        self.negate(se, cx);
        true
    }
}

/// `Dependence`: the dependence between two accesses.
#[derive(Clone, Debug)]
pub enum Dependence {
    /// Nothing is understood: the worst case is assumed.
    Confused {
        src: InstId,
        dst: InstId,
        assumptions: UnionPredicate,
    },
    Full(FullDependence),
}

/// What the instructions of a dependence do to memory: `(reads, writes)`.
pub struct Access {
    pub reads: bool,
    pub writes: bool,
}

impl Dependence {
    pub fn src(&self) -> InstId {
        match self {
            Self::Confused { src, .. } => *src,
            Self::Full(full) => full.src,
        }
    }

    pub fn dst(&self) -> InstId {
        match self {
            Self::Confused { dst, .. } => *dst,
            Self::Full(full) => full.dst,
        }
    }

    pub fn assumptions(&self) -> &UnionPredicate {
        match self {
            Self::Confused { assumptions, .. } => assumptions,
            Self::Full(full) => &full.assumptions,
        }
    }

    /// `isConfused`.
    pub fn is_confused(&self) -> bool {
        matches!(self, Self::Confused { .. })
    }

    /// `isLoopIndependent`.
    pub fn is_loop_independent(&self) -> bool {
        match self {
            Self::Confused { .. } => true,
            Self::Full(full) => full.loop_independent,
        }
    }

    /// `getLevels`: the number of common loops around the source and the
    /// destination.
    pub fn levels(&self) -> u32 {
        match self {
            Self::Confused { .. } => 0,
            Self::Full(full) => full.levels,
        }
    }

    /// `getSameSDLevels`.
    pub fn same_sd_levels(&self) -> u32 {
        match self {
            Self::Confused { .. } => 0,
            Self::Full(full) => full.same_sd_levels,
        }
    }

    /// `getDirection`.
    pub fn direction(
        &self,
        level: u32,
        same_sd: bool,
    ) -> u8 {
        match self {
            Self::Confused { .. } => dir::ALL,
            Self::Full(full) => full.direction(level, same_sd),
        }
    }

    /// `getDistance`.
    pub fn distance(
        &self,
        level: u32,
        same_sd: bool,
    ) -> Option<Scev> {
        match self {
            Self::Confused { .. } => None,
            Self::Full(full) => full.distance(level, same_sd),
        }
    }

    /// `isScalar`.
    pub fn is_scalar(
        &self,
        level: u32,
        same_sd: bool,
    ) -> bool {
        match self {
            Self::Confused { .. } => false,
            Self::Full(full) => full.is_scalar(level, same_sd),
        }
    }

    /// `inSameSDLoops`.
    pub fn in_same_sd_loops(
        &self,
        level: u32,
    ) -> bool {
        match self {
            Self::Confused { .. } => false,
            Self::Full(full) => full.in_same_sd_loops(level),
        }
    }

    /// `isDirectionNegative`.
    pub fn is_direction_negative(&self) -> bool {
        match self {
            Self::Confused { .. } => false,
            Self::Full(full) => full.is_direction_negative(),
        }
    }

    /// `normalize`.
    pub fn normalize(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
    ) -> bool {
        match self {
            Self::Confused { .. } => false,
            Self::Full(full) => full.normalize(se, cx),
        }
    }

    /// `Dependence::dump`, given what each end does to memory (`isFlow` and
    /// its kin).
    pub fn dump(
        &self,
        se: &ScalarEvolution,
        cx: &Cx,
        labels: &Labels,
        src: &Access,
        dst: &Access,
    ) -> String {
        let mut out = String::new();
        if self.is_confused() {
            out += "confused";
        } else {
            if src.writes && dst.reads {
                out += "flow";
            } else if src.writes && dst.writes {
                out += "output";
            } else if src.reads && dst.writes {
                out += "anti";
            } else if src.reads && dst.reads {
                out += "input";
            }
            self.dump_imp(se, cx, labels, false, &mut out);
            let same_sd_levels = self.same_sd_levels();
            if same_sd_levels > 0 {
                let _ = write!(out, " / assuming {same_sd_levels} loop level(s) fused: ");
                self.dump_imp(se, cx, labels, true, &mut out);
            }
        }
        out += "!\n";

        let assumptions = self.assumptions();
        if !assumptions.is_always_true() {
            out += "  Runtime Assumptions:\n";
            for one in &assumptions.preds {
                if one.pred == IntPredicate::Eq {
                    let _ = writeln!(
                        out,
                        "  Equal predicate: {} == {}",
                        se.print(cx, labels, one.lhs),
                        se.print(cx, labels, one.rhs)
                    );
                } else {
                    let _ = writeln!(
                        out,
                        "  Compare predicate: {} {}) {}",
                        se.print(cx, labels, one.lhs),
                        predicate_name(one.pred),
                        se.print(cx, labels, one.rhs)
                    );
                }
            }
        }
        out
    }

    /// `Dependence::dumpImp`.
    fn dump_imp(
        &self,
        se: &ScalarEvolution,
        cx: &Cx,
        labels: &Labels,
        is_same_sd: bool,
        out: &mut String,
    ) {
        let levels = self.levels();
        let same_sd_levels = self.same_sd_levels();
        let mut on_same_sd = false;
        let mut level_num = levels;
        if is_same_sd {
            level_num += same_sd_levels;
        }
        out.push_str(" [");
        for level in 1..=level_num {
            if !on_same_sd && self.in_same_sd_loops(level) {
                on_same_sd = true;
            }
            if let Some(distance) = self.distance(level, on_same_sd) {
                out.push_str(&se.print(cx, labels, distance));
            } else if self.is_scalar(level, on_same_sd) {
                out.push('S');
            } else {
                let direction = self.direction(level, on_same_sd);
                if direction == dir::ALL {
                    out.push('*');
                } else {
                    if direction & dir::LT != 0 {
                        out.push('<');
                    }
                    if direction & dir::EQ != 0 {
                        out.push('=');
                    }
                    if direction & dir::GT != 0 {
                        out.push('>');
                    }
                }
            }
            if level < level_num {
                out.push(' ');
            }
        }
        if self.is_loop_independent() {
            out.push_str("|<");
        }
        out.push(']');
    }
}

/// `SmallBitVector`, as far as the tests use it: bits by level, grown on
/// demand.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Levels(Vec<bool>);

impl Levels {
    pub fn new(size: usize) -> Self {
        Self(vec![false; size])
    }

    pub fn set(
        &mut self,
        at: u32,
    ) {
        let at = at as usize;
        if at >= self.0.len() {
            self.0.resize(at + 1, false);
        }
        self.0[at] = true;
    }

    pub fn get(
        &self,
        at: u32,
    ) -> bool {
        self.0.get(at as usize).copied().unwrap_or(false)
    }

    pub fn count(&self) -> usize {
        self.0.iter().filter(|&&one| one).count()
    }

    pub fn or(
        &mut self,
        other: &Self,
    ) {
        if other.0.len() > self.0.len() {
            self.0.resize(other.0.len(), false);
        }
        for (mine, theirs) in self.0.iter_mut().zip(&other.0) {
            *mine |= *theirs;
        }
    }

    pub fn resize(
        &mut self,
        size: usize,
    ) {
        self.0.resize(size, false);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// `DependenceInfo::Subscript::ClassificationKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Ziv,
    Siv,
    Rdiv,
    Miv,
    NonLinear,
}

/// `DependenceInfo::Subscript`: a pair of subscripts of a pair of potentially
/// multi-dimensional array references.
#[derive(Clone, Debug)]
pub struct Subscript {
    pub src: Scev,
    pub dst: Scev,
    pub classification: Classification,
    pub loops: Levels,
}

/// `DependenceTestType`: which tests run (`-da-enable-dependence-test`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestType {
    /// All tests except BanerjeeMIV.
    Default,
    All,
    StrongSiv,
    WeakCrossingSiv,
    ExactSiv,
    WeakZeroSiv,
    ExactRdiv,
    GcdMiv,
    BanerjeeMiv,
}

impl TestType {
    /// The value of `-da-enable-dependence-test`.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "default" => Self::Default,
            "all" => Self::All,
            "strong-siv" => Self::StrongSiv,
            "weak-crossing-siv" => Self::WeakCrossingSiv,
            "exact-siv" => Self::ExactSiv,
            "weak-zero-siv" => Self::WeakZeroSiv,
            "exact-rdiv" => Self::ExactRdiv,
            "gcd-miv" => Self::GcdMiv,
            "banerjee-miv" => Self::BanerjeeMiv,
            _ => return None,
        })
    }
}

/// The command-line options of the pass (`-da-delinearize`,
/// `-da-disable-delinearization-checks`, `-da-miv-max-level-threshold`,
/// `-da-enable-dependence-test`).
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub delinearize: bool,
    pub disable_delinearization_checks: bool,
    pub miv_max_level_threshold: u32,
    pub enable_test: TestType,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delinearize: true,
            disable_delinearization_checks: false,
            miv_max_level_threshold: 7,
            enable_test: TestType::Default,
        }
    }
}

/// `DependenceInfo`: the main dependence-analysis driver.
pub struct DependenceInfo<'a, A: AAResults> {
    pub(crate) aa: &'a mut A,
    pub(crate) unit: &'a Unit<'a>,
    pub(crate) cx: &'a Cx<'a>,
    pub(crate) options: Options,
    pub(crate) common_levels: u32,
    pub(crate) src_levels: u32,
    pub(crate) max_levels: u32,
    pub(crate) same_sd_levels: u32,
}

impl<'a, A: AAResults> DependenceInfo<'a, A> {
    pub fn new(
        aa: &'a mut A,
        unit: &'a Unit<'a>,
        cx: &'a Cx<'a>,
        options: Options,
    ) -> Self {
        Self { aa, unit, cx, options, common_levels: 0, src_levels: 0, max_levels: 0, same_sd_levels: 0 }
    }

    /// `isLoadOrStore`: a simple load or store; atomic and volatile
    /// operations have properties this analysis does not understand.
    pub(crate) fn is_load_or_store(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.cx.function.instruction(inst).opcode,
            Opcode::Load { volatile: false, .. } | Opcode::Store { volatile: false, .. }
        )
    }

    /// `underlyingObjectsAlias`: no alias for any reason (tbaa, non-overlapping
    /// regions), else the underlying objects: the same must alias, two
    /// identified ones are distinct, others may alias.
    pub(crate) fn underlying_objects_alias(
        &mut self,
        loc_a: &MemoryLocation,
        loc_b: &MemoryLocation,
    ) -> AliasResult {
        // Check the original locations (minus size) for noalias, which can
        // happen for tbaa, incompatible underlying object locations, etc.
        let loc_a_s = MemoryLocation { size: LocationSize::BeforeOrAfterPointer, ..*loc_a };
        let loc_b_s = MemoryLocation { size: LocationSize::BeforeOrAfterPointer, ..*loc_b };
        let mut baa = BatchAAResults::new(&mut *self.aa);
        baa.enable_cross_iteration_mode();
        if baa.is_no_alias(&loc_a_s, &loc_b_s) {
            return AliasResult::NoAlias;
        }

        // Check the underlying objects are the same.
        let query = self.cx.query(None);
        let a_object = llrm_mir::valuetracking::underlying::get_underlying_object(&query, loc_a.ptr, 0);
        let b_object = llrm_mir::valuetracking::underlying::get_underlying_object(&query, loc_b.ptr, 0);

        // If the underlying objects are the same, they must alias.
        if a_object == b_object {
            return AliasResult::MustAlias;
        }

        // We may have hit the recursion limit for underlying objects, or have
        // underlying objects where we don't know they will alias.
        let basic = BasicAA::new(self.unit);
        if !basic.is_identified_object(a_object) || !basic.is_identified_object(b_object) {
            return AliasResult::MayAlias;
        }

        // Otherwise we know the objects are different and both identified
        // objects so must not alias.
        AliasResult::NoAlias
    }

    /// `haveSameSD`: whether two loops have the same iteration space and
    /// depth: the same nesting depth and the same backedge count.
    fn have_same_sd(
        &self,
        se: &mut ScalarEvolution,
        src_loop: llrm_mir::loopinfo::Loop<'_>,
        dst_loop: llrm_mir::loopinfo::Loop<'_>,
    ) -> bool {
        use crate::scev::backedge::ExitCountKind;
        if src_loop.id() == dst_loop.id() {
            return true;
        }
        if src_loop.get_loop_depth() != dst_loop.get_loop_depth() {
            return false;
        }
        let function = self.cx.function;
        if src_loop.get_loop_latch(function).is_none() || dst_loop.get_loop_latch(function).is_none() {
            return false;
        }
        let src_ub = se.get_backedge_taken_count(self.cx, src_loop.get_header(), ExitCountKind::Exact);
        let dst_ub = se.get_backedge_taken_count(self.cx, dst_loop.get_header(), ExitCountKind::Exact);
        if matches!(se.node(src_ub).kind, Kind::CouldNotCompute)
            || matches!(se.node(dst_ub).kind, Kind::CouldNotCompute)
        {
            return false;
        }
        let wider = se.get_wider_type(self.cx, se.type_of(src_ub), se.type_of(dst_ub));
        let src_ub = se.get_noop_or_zero_extend(self.cx, src_ub, wider);
        let dst_ub = se.get_noop_or_zero_extend(self.cx, dst_ub, wider);
        src_ub == dst_ub
    }

    /// `establishNestingLevels`: the shared loops of the source and the
    /// destination, `common_levels`, `src_levels`, `max_levels` and
    /// `same_sd_levels`.
    pub(crate) fn establish_nesting_levels(
        &mut self,
        se: &mut ScalarEvolution,
        src: InstId,
        dst: InstId,
    ) {
        let src_block = self.block_of(src);
        let dst_block = self.block_of(dst);
        let loops = self.cx.loops;
        let mut src_level = loops.get_loop_depth(src_block);
        let mut dst_level = loops.get_loop_depth(dst_block);
        let mut src_loop = loops.get_loop_for(src_block);
        let mut dst_loop = loops.get_loop_for(dst_block);
        self.src_levels = src_level;
        self.max_levels = src_level + dst_level;
        self.same_sd_levels = 0;
        while src_level > dst_level {
            src_loop = src_loop.and_then(|one| one.get_parent_loop());
            src_level -= 1;
        }
        while dst_level > src_level {
            dst_loop = dst_loop.and_then(|one| one.get_parent_loop());
            dst_level -= 1;
        }

        let mut src_uncommon_frontier = None;
        let mut dst_uncommon_frontier = None;
        // Find the first uncommon level pair and check if the associated
        // levels have the SameSD.
        while src_loop.map(|one| one.id()) != dst_loop.map(|one| one.id()) {
            src_uncommon_frontier = src_loop;
            dst_uncommon_frontier = dst_loop;
            src_loop = src_loop.and_then(|one| one.get_parent_loop());
            dst_loop = dst_loop.and_then(|one| one.get_parent_loop());
            src_level -= 1;
        }
        if let (Some(src_frontier), Some(dst_frontier)) = (src_uncommon_frontier, dst_uncommon_frontier)
            && self.have_same_sd(se, src_frontier, dst_frontier)
        {
            self.same_sd_levels = 1;
        }
        self.common_levels = src_level;
        self.max_levels -= self.common_levels;
    }

    /// The block holding `inst`.
    pub(crate) fn block_of(
        &self,
        inst: InstId,
    ) -> BlockId {
        self.cx.function.parent(inst).expect("a placed instruction")
    }

    /// `mapSrcLoop`: a loop containing the source, as its level.
    pub(crate) fn map_src_loop(
        &self,
        header: BlockId,
    ) -> u32 {
        self.loop_depth(header)
    }

    /// `mapDstLoop`: a loop containing the destination, as its level; loops
    /// of the same depth that the two accesses do not share get distinct
    /// numbers.
    pub(crate) fn map_dst_loop(
        &self,
        header: BlockId,
    ) -> u32 {
        let depth = self.loop_depth(header);
        if depth > self.common_levels { depth - self.common_levels + self.src_levels } else { depth }
    }

    /// `Loop::getLoopDepth` of the loop with this header.
    pub(crate) fn loop_depth(
        &self,
        header: BlockId,
    ) -> u32 {
        self.cx.loops.get_loop_for(header).expect("a loop header").get_loop_depth()
    }
}
