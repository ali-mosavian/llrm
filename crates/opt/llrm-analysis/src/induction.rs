//! Which values are affine functions of a loop's counter, and how many trips a
//! loop makes.
//!
//! # How it works
//! Three layers, each built on the one before.
//!
//! Counters. `basics(unit, loop)` finds the header phis that one invariant or
//! constant step (`add`, or `sub` of a constant) advances on every way round,
//! from one start on every way in: an `Affine { value, start, step, header }`,
//! `start + step * trip`. `pointers` finds pointer phis stepped by constant
//! bytes through a `getelementptr`. `followers` finds header phis that advance
//! by at most one a trip wherever a counter advances by one without wrapping.
//!
//! Trip counts. `counted(unit, loop, ..)` returns a `CountedLoop` proof for a
//! loop with one latch and one conditional exit that goes on (`_control`: the
//! branch is in the header or the latch, a latch that only jumps back being the
//! end of its predecessor). The proof says the counter, its start and bound,
//! the `IntPredicate` that continues the loop, whether it is tested before or
//! after the trip (`posttested`) and on the stepped value (`stepped`), and how
//! the count is reached (`Reach`):
//!
//! - `Distance`: `bound - start`, one more if the test is inclusive.
//! - `Solved`: where `start + k * step == bound` modulo the width, as LLVM's
//!   `howFarToZero`.
//! - `Ceil`: an ordered test with a step above one that is promised not to
//!   wrap, the distance divided by the step, rounded up (`howManyLessThans`).
//!
//! `count` is the exact count when constant, `maximum` bounds an unknown one,
//! and `first` and `last` are the counter's signed values on the first and last
//! trip, given only when nothing wraps. A symbolic count is built on demand by
//! `trips` (and `exit_value`), which asks the caller to place each preheader
//! operation. `counted_unless_stopped` also accepts a loop that leaves into a
//! block that never returns (`stops`, using [`crate::noreturn::stranded`]);
//! `counted_leaving` also other exits that go on. `exits` counts every exit of
//! a single-latch loop, as LLVM's exit limits. The manager keeps `counted_all`
//! as the `Counted` analysis, and a unit that carries it is read without a
//! re-proof.
//!
//! Recurrences. A `Scev` is a polynomial in loop-invariant unknowns modulo a
//! width (sums of `Monomial`s, at most 16 terms and degree 4), in one canonical
//! form so equal values are equal. A `Recurrence` is `pointer + start + step *
//! trip` with `Scev` start and step: LLVM's add recurrence. `recurrences` walks
//! a loop and gives every value affine in a counter one (a narrow counter
//! extended with `zext` too, where its start is made of values the enclosing
//! loop computes as recurrences and the whole walk stays inside the narrow
//! width unsigned (the bound is the enclosing loop's own recurrence and trip
//! count, induction's domain: the signed `ranges::Interval` cannot hold a
//! 16-bit value past 32767, and a second range evaluator would duplicate it):
//! the wide start names those values as narrow unknowns,
//! which the expander reads zero-extended; an `nsw` operation past the signed
//! maximum is poison and proves nothing); `users` lists the
//! operands that read them, each as an `IvUse` of kind `Address`, `Compare` or
//! `Basic` with the low bits it observes (LLVM's IVUsers).
//!
//! ```text
//!   pre:    ...
//!   header: %i = phi [0, pre], [%n, latch]     Affine{start 0, step 1}
//!           %c = icmp slt %i, %len             test: Slt, bound %len
//!           br %c, body, exit                  Reach::Distance
//!   body:   %p = gep %a, %i                    Recurrence{pointer %a, step 4}
//!   latch:  %n = add %i, 1                     trips = %len - 0
//! ```
//!
//! Proofs of removal are `control_replacement` (the counter's phi is read only
//! by its compare, its step and the exit's phis, so the loop's own control may
//! go) and `zero_terminating_control` (a counter reaching its final value ends
//! control and takes no value twice in the trip count).
//!
//! # Example
//! A loop that steps by 24 until it reaches `%start + 768`, tested after the
//! step:
//!
//! ```mir
//! define i32 @f(i32 %start) {
//! b0:
//!   %end = add i32 %start, 768
//!   br label %b1
//!
//! b1:
//!   %i = phi i32 [ %start, %b0 ], [ %next, %b2 ]
//!   %trips = phi i32 [ 0, %b0 ], [ %up, %b2 ]
//!   br label %b2
//!
//! b2:
//!   %up = add i32 %trips, 1
//!   %next = add i32 %i, 24
//!   %c = icmp ne i32 %next, %end
//!   br i1 %c, label %b1, label %b3
//!
//! b3:
//!   %t = phi i32 [ %up, %b2 ]
//!   ret i32 %t
//! }
//! ```
//!
//! `trip_count` is 32, whatever `%start` is (768 / 24). The interpreter returns
//! 32 for starts 0, -5 and 0x7FFFFFF0.
//!
//! From unit test
//! `test_posttested_symbolic_sentinel_keeps_its_exact_trip_count` in
//! `induction_tests.rs`.
//!
//! # Provides
//! `AffineOperand`, `Affine`, `CountedLoop`, `Counted`, `ExitCount`, `Scev`,
//! `Recurrence`, `Users`, `LoopFollowers` and the functions above. Manager
//! entries: `Counted` (incremental) and `Followers`. Consumers:
//! [`crate::ranges`], [`crate::alias`], [`crate::peelsize`],
//! [`crate::difference`], [`crate::guards`], [`crate::floatfacts`], and in
//! llrm-transforms `indvars`, `lsr`, `loopexit`, `peel`, `unroll`,
//! `rotate`, `decide`, `hoist`, `expand`, `exitfold`, `counting`,
//! `window`, `loopidioms`, `floatloop`.
//!
//! # Needs
//! [`crate::consts`] (the registers the proofs read), [`crate::occurrence`],
//! [`crate::noreturn`], [`crate::cfg`] (loops, dominance).
//!
//! # See also
//! LLVM ScalarEvolution add recurrences and trip counts, InductionDescriptor,
//! IVUsers.

use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::signed;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::types::TypeId;
use llrm_support::hash::IndexMap;

use crate::cfg::{self, Around};
use crate::consts::{Known, masked};
use crate::graph::loops::Loop;
use crate::memory::{self, MemRef, Unit};
use crate::noreturn;
use crate::num::Num as BigInt;
use crate::occurrence::{operations_in, phis};

/// A recurrence's start or step: a value, or a number.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AffineOperand {
    /// A value `width` bits wide.
    Value(ValueId, u32),
    Const(Known),
}

impl AffineOperand {
    /// `n` modulo `width` bits.
    pub fn constant(
        n: impl Into<BigInt>,
        width: u32,
    ) -> Self {
        Self::Const(Known::new(masked(&n.into(), width), width))
    }

    pub fn width(&self) -> u32 {
        match self {
            Self::Value(_, width) => *width,
            Self::Const(known) => known.width,
        }
    }
}

/// An integer operand as a term.
pub fn term(
    unit: &Unit,
    operand: Operand,
) -> Option<AffineOperand> {
    let width = unit.int_bits(operand)?;
    match operand {
        Operand::Value(value) => Some(AffineOperand::Value(value, width)),
        Operand::Constant(_) => Some(AffineOperand::Const(Known::new(unit.int_constant(operand)?, width))),
        Operand::Block(_) => None,
    }
}

/// `start + step * iteration`, in the loop `header` heads.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Affine {
    pub value: ValueId,
    pub start: AffineOperand,
    pub step: AffineOperand,
    pub header: i64,
}

/// How a trip count without a constant value is found from its start and
/// bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reach {
    /// By the distance alone: `bound - start`, one more if the test is
    /// inclusive. A pre-tested unit step.
    Distance,
    /// Where the counter first equals the bound, `start + k * step == bound`
    /// solved modulo the width, as LLVM's `howFarToZero` does: `k` is
    /// `((bound - start) >> shift) * inverse`, in the low `bits` bits, and
    /// `shift` low bits of the distance are zero. The step is `2**shift` times
    /// an odd number, which `inverse` undoes.
    Solved { shift: u32, inverse: BigInt, bits: u32 },
    /// An ordered test with a step of more than one that is promised not to
    /// wrap, as LLVM's `howManyLessThans`: the distance to the bound, divided
    /// by the step, rounded up. `strict` is `<` or `>`, not `<=` or `>=`.
    Ceil { strict: bool },
}

/// The one proof of how many trips a loop makes, shared by every pass.
///
/// ```text
/// i = start; loop { [i test bound?] body; i += step; [i test bound?] }
/// ```
///
/// `test` continues the loop, counter first; `step` is a nonzero
/// constant. A pre-tested loop tests the header value before each trip; a
/// post-tested one tests after each trip, the stepped value when
/// `stepped`.
///
/// `count` is the exact trip count when constant. `trips` places it
/// when symbolic, which needs a pre-tested unit step: the only proofs with
/// no `count`. `first` and `last` are the header's signed values on
/// the first and last trip, given only when nothing up to the exit wraps.
/// `maximum` bounds the trips when the count is unknown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CountedLoop {
    pub counter: Affine,
    pub phi: InstId,
    pub compare: InstId,
    pub branch: InstId,
    pub start: AffineOperand,
    pub bound: AffineOperand,
    pub test: IntPredicate,
    pub preheader: Option<i64>,
    pub latch: i64,
    pub entered: i64,
    pub exit: i64,
    pub maximum: Option<BigInt>,
    pub step: BigInt,
    pub posttested: bool,
    pub stepped: bool,
    /// The latch compare reads the counter before its step, and `bound` is the
    /// one a compare of the stepped counter would test: `phi >= 1` is
    /// `update >= 0` for a step of -1. The count and `stepped` are those of
    /// that compare.
    pub shifted: bool,
    /// Some other exit stops the program; `count` is the trips when it goes on.
    pub stops: bool,
    /// Another exit goes on, to code that returns (only where `counted_leaving`
    /// asked); `count` is then the trips as long as it is not taken.
    pub leaves: bool,
    /// A loop tested after its trips whose symbolic trips assume the first
    /// would have continued, which the branches over its preheader prove.
    pub entry_guarded: bool,
    pub reach: Reach,
    pub count: Option<BigInt>,
    pub first: Option<BigInt>,
    pub last: Option<BigInt>,
}

impl CountedLoop {
    pub fn inclusive(&self) -> bool {
        _inclusive(self.test)
    }

    pub fn width(&self) -> u32 {
        self.bound.width()
    }

    /// Tested against zero for inequality: the test the step's own result
    /// answers, which no other recurrence ends the loop more cheaply on.
    pub fn zero_tested(&self) -> bool {
        self.test == IntPredicate::Ne && self.bound == AffineOperand::constant(0, self.width())
    }

    /// The header's unsigned values only rise from a constant start to the
    /// bound, or stay at the start, and never pass the width's largest: a unit
    /// step tested before each trip by `ult`, from any start: one past it the
    /// loop has left. Its zero extension is a counter of the wider width.
    pub fn rises_unsigned(&self) -> bool {
        !self.posttested && !self.stepped && self.test == IntPredicate::Ult && self.step == BigInt::from(1)
    }

    /// `rises_unsigned` for signed values: a unit step tested before each trip
    /// by `slt` from any start: the counter is below the bound when it is
    /// stepped, so the step never passes the width's largest signed value.
    /// Its sign extension is a counter of the wider width.
    pub fn rises_signed(&self) -> bool {
        !self.posttested && !self.stepped && self.test == IntPredicate::Slt && self.step == BigInt::from(1)
    }

    /// `rises_signed` for a loop tested after its trips, entered behind a copy
    /// of its test.
    pub fn rises_signed_after(&self) -> bool {
        self.posttested
            && self.stepped
            && !self.shifted
            && self.entry_guarded
            && matches!(self.reach, Reach::Distance)
            && self.test == IntPredicate::Slt
            && self.step == BigInt::from(1)
    }

    /// `rises_unsigned` for a loop tested after its trips, entered behind a
    /// copy of its test: the stepped counter is below the bound when the
    /// loop goes round, so the step never passes the width's largest either.
    pub fn rises_unsigned_after(&self) -> bool {
        self.posttested
            && self.stepped
            && !self.shifted
            && self.entry_guarded
            && matches!(self.reach, Reach::Distance)
            && self.test == IntPredicate::Ult
            && self.step == BigInt::from(1)
    }

    /// The signed values the header's counter takes on a trip, lowest first.
    pub fn span(&self) -> Option<(BigInt, BigInt)> {
        let (first, last) = (self.first.as_ref()?, self.last.as_ref()?);
        Some((first.min(last).clone(), first.max(last).clone()))
    }
}

fn _ascending(test: IntPredicate) -> bool {
    matches!(
        test,
        IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Ult | IntPredicate::Ule
    )
}

fn _descending(test: IntPredicate) -> bool {
    matches!(
        test,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Ugt | IntPredicate::Uge
    )
}

/// The test of the stepped counter that holds where the unstepped counter
/// passes `test` against the same bound, for a step of one.
fn _after_step(
    test: IntPredicate,
    step: &BigInt,
) -> Option<IntPredicate> {
    match (test, step > &BigInt::from(0)) {
        (IntPredicate::Slt, true) => Some(IntPredicate::Sle),
        (IntPredicate::Ult, true) => Some(IntPredicate::Ule),
        (IntPredicate::Sgt, false) => Some(IntPredicate::Sge),
        (IntPredicate::Ugt, false) => Some(IntPredicate::Uge),
        _ => None,
    }
}

fn _inclusive(test: IntPredicate) -> bool {
    matches!(
        test,
        IntPredicate::Sle | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Uge
    )
}

fn _unsigned(test: IntPredicate) -> bool {
    matches!(
        test,
        IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
    )
}

/// Places one preheader operation and returns its result.
pub type Computed<'a> = dyn FnMut(BinaryOp, Vec<AffineOperand>) -> AffineOperand + 'a;

/// The preheader comparison, and the test on it, under which the loop runs no
/// trips.
pub fn skipped(proof: &CountedLoop) -> Option<((AffineOperand, AffineOperand), IntPredicate)> {
    if proof.posttested {
        return None;
    }
    Some(((proof.bound.clone(), proof.start.clone()), proof.test.inverse().swapped()))
}

/// Trips on the entered path, exact modulo the compare's width, or None where
/// not expressible.
pub fn trips(
    proof: &CountedLoop,
    computed: &mut Computed<'_>,
) -> Option<AffineOperand> {
    let width = proof.width();
    if let Reach::Solved { shift, inverse, bits } = &proof.reach {
        // The distance left after the first step where it is tested stepped,
        // solved modulo the period, and the trip a post-tested loop
        // makes before its test.
        let lead = AffineOperand::constant(&proof.step * u8::from(proof.posttested && proof.stepped), width);
        let distance = computed(BinaryOp::Sub, vec![proof.bound.clone(), proof.start.clone()]);
        let remaining = computed(BinaryOp::Sub, vec![distance, lead]);
        let divided = if *shift == 0 {
            remaining
        } else {
            computed(BinaryOp::LShr, vec![remaining, AffineOperand::constant(*shift, width)])
        };
        let solved = if *inverse == BigInt::from(1) {
            divided
        } else {
            computed(BinaryOp::Mul, vec![divided, AffineOperand::constant(inverse.clone(), width)])
        };
        let solved = if *bits < width {
            computed(BinaryOp::And, vec![solved, AffineOperand::constant((BigInt::from(1) << *bits) - 1, width)])
        } else {
            solved
        };
        return Some(if proof.posttested {
            computed(BinaryOp::Add, vec![solved, AffineOperand::constant(1, width)])
        } else {
            solved
        });
    }
    // Tested after its trips, the loop makes the trips a pre-tested one would
    // where its entry is guarded or its count is known: by unit steps and a
    // stepped test the trips from `start` to `bound` are the same (`entered`,
    // which sets `entry_guarded`).
    let guarded_unit = proof.entry_guarded && proof.stepped && matches!(proof.reach, Reach::Distance);
    if proof.posttested
        && proof.count.is_none()
        && !(proof.entry_guarded && matches!(proof.reach, Reach::Ceil { .. }))
        && !guarded_unit
    {
        return None;
    }
    if let Some(count) = &proof.count {
        return (count < &(BigInt::from(1) << width)).then(|| AffineOperand::constant(count.clone(), width));
    }
    if let Reach::Ceil { strict } = &proof.reach {
        // On the entered path the distance is at least one: its predecessor,
        // divided, plus one, cannot overflow where the distance rounded
        // up could.
        let (ahead, behind) =
            if proof.step > BigInt::from(0) { (&proof.bound, &proof.start) } else { (&proof.start, &proof.bound) };
        let distance = computed(BinaryOp::Sub, vec![ahead.clone(), behind.clone()]);
        let distance =
            if *strict { computed(BinaryOp::Sub, vec![distance, AffineOperand::constant(1, width)]) } else { distance };
        let whole = computed(BinaryOp::UDiv, vec![distance, AffineOperand::constant(abs(&proof.step), width)]);
        return Some(computed(BinaryOp::Add, vec![whole, AffineOperand::constant(1, width)]));
    }
    let (ahead, behind) =
        if proof.step > BigInt::from(0) { (&proof.bound, &proof.start) } else { (&proof.start, &proof.bound) };
    let count = computed(BinaryOp::Sub, vec![ahead.clone(), behind.clone()]);
    Some(computed(BinaryOp::Add, vec![count, AffineOperand::constant(u8::from(proof.inclusive()), width)]))
}

/// The header's counter as a pre-tested loop that ran a trip leaves: the first
/// value failing its test.
pub fn exit_value(
    proof: &CountedLoop,
    computed: &mut Computed<'_>,
) -> Option<AffineOperand> {
    let width = proof.width();
    if proof.posttested {
        return None;
    }
    if proof.test == IntPredicate::Ne {
        return Some(proof.bound.clone());
    }
    if let (AffineOperand::Const(start), Some(count)) = (&proof.start, &proof.count) {
        return Some(AffineOperand::constant(&start.n + count * &proof.step, width));
    }
    let past = AffineOperand::constant(&proof.step * u8::from(proof.inclusive()), width);
    if let (AffineOperand::Const(bound), AffineOperand::Const(past)) = (&proof.bound, &past) {
        return Some(AffineOperand::constant(&bound.n + &past.n, width));
    }
    Some(computed(BinaryOp::Add, vec![proof.bound.clone(), past]))
}

/// Proof that a counted loop's own recurrence may be removed: its phi is
/// read only by the compare, the step, the caller's covered instructions
/// and `exits`, the exit block's phis reading it as the loop leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlReplacement<'a> {
    pub counted: &'a CountedLoop,
    pub stepping: InstId,
    pub update: ValueId,
    pub exits: Vec<InstId>,
}

/// Proof that `candidate` reaching its final value can end counted control:
/// it takes no value twice in `maximum` trips.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZeroTerminatingControl<'a> {
    pub replacement: ControlReplacement<'a>,
    pub candidate: Affine,
    pub step: BigInt,
    pub maximum: BigInt,
    pub period: BigInt,
}

/// The finite inclusive signed domain `affine` takes on a trip.
pub fn domain(
    unit: &Unit,
    loop_: &Loop,
    affine: &Affine,
    facts: &IndexMap<ValueId, Known>,
) -> Option<(BigInt, BigInt)> {
    controlling(unit, loop_, affine, facts)?.span()
}

/// Where a single-latch loop with one exit tests whether to go round again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct _Control {
    block: i64,
    preheader: Option<i64>,
    entered: i64,
    exit: i64,
    posttested: bool,
    stops: bool,
    leaves: bool,
    /// A header that holds its whole trip, its latch only a jump back: its
    /// test may read the stepped value, as after a trip.
    after: bool,
}

/// The block whose conditional branch is the loop's only exit that goes on: its
/// header, or its latch.
fn _control(
    function: &Function,
    loop_: &Loop,
    leaving: bool,
) -> Option<_Control> {
    // Only the blocks the loop and what enters it name are read, not the graph
    // of the whole body (found for each loop, that was N^2 in a function of
    // N loops).
    let block_of = |at: i64| cfg::Block { at, succ: cfg::successors_of(function, at) };
    if loop_.latches.len() != 1 {
        return None;
    }
    let latch = block_of(*loop_.latches.first()?);
    let header = block_of(loop_.header);
    let inside = &loop_.body;
    // A latch that only jumps on, split from a critical edge, is the end of
    // the block that branches to it: that block is tested after its trip.
    let forwarded = (latch.succ.as_slice() == [header.at]).then(|| {
        let only = function.block(cfg::block(latch.at));
        let preds =
            function.predecessors(cfg::block(latch.at)).into_iter().map(|at| block_of(cfg::id(at))).collect::<Vec<_>>();
        match (only.len(), &preds[..]) {
            (1, [pred])
                if pred.at != header.at
                    && inside.contains(&pred.at)
                    && pred.succ.len() == 2
                    && pred.succ.contains(&latch.at) =>
            {
                Some(pred.clone())
            }
            _ => None,
        }
    });
    // The latch jumps back and only the header branches to it: the header holds
    // the trip.
    let header_holds_trip = latch.succ.as_slice() == [header.at]
        && function.block(cfg::block(latch.at)).len() == 1
        && header.succ.len() == 2
        && header.succ.contains(&latch.at)
        && function.predecessors(cfg::block(latch.at)).len() == 1;
    let (control, entered) = if let Some(Some(pred)) = &forwarded {
        (pred.clone(), vec![latch.at])
    } else if latch.succ.as_slice() == [header.at] {
        (header.clone(), header.succ.iter().copied().filter(|at| inside.contains(at)).collect::<Vec<_>>())
    } else if latch.succ.contains(&header.at) {
        (latch.clone(), vec![header.at])
    } else {
        return None;
    };
    let exits = control.succ.iter().copied().filter(|at| !inside.contains(at)).collect::<Vec<_>>();
    let conditional = function
        .terminator(cfg::block(control.at))
        .is_some_and(
            |last| matches!(
                function.instruction(last),
                Instruction { opcode: Opcode::Br, operands, .. } if operands.len() == 3
            ),
        );
    if control.succ.len() != 2 || entered.len() != 1 || exits.len() != 1 || !conditional {
        return None;
    }
    // One pass over the loop's blocks, by their terminators' operands: a block
    // that goes nowhere ends the program, and any other way out must stop it:
    // the count holds whenever it goes on. (`successors` made a list for each
    // block, and a nest of loops asked it of every block of every loop.)
    let mut elsewhere = llrm_mir::dense::IdSet::<i64>::new();
    for &at in inside {
        let mut goes = false;
        if let Some(last) = function.terminator(cfg::block(at)) {
            for operand in &function.instruction(last).operands {
                if let Operand::Block(to) = *operand {
                    goes = true;
                    let to = cfg::id(to);
                    if at != control.at && !inside.contains(&to) {
                        elsewhere.insert(to);
                    }
                }
            }
        }
        if !goes {
            return None;
        }
    }
    // Or, where `leaving`, go on: the count then holds as long as the loop
    // does.
    let leaves = !elsewhere.is_empty() && !elsewhere.is_subset(&noreturn::stranded(function, header.at));
    if !leaving && leaves {
        return None;
    }
    let outside = loop_.entering(function);
    let preheader = match outside.first() {
        Some(&one) if outside.len() == 1 && block_of(one).succ.as_slice() == [header.at] => Some(one),
        _ => None,
    };
    Some(_Control {
        block: control.at,
        preheader,
        entered: entered[0],
        exit: exits[0],
        posttested: control.at == latch.at || matches!(forwarded, Some(Some(_))),
        stops: !elsewhere.is_empty(),
        leaves,
        after: header_holds_trip,
    })
}

/// The operand phi `inst` takes from `from`.
fn incoming(
    function: &Function,
    inst: InstId,
    from: BlockId,
) -> Option<Operand> {
    function.instruction(inst).operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).map(|pair| pair[0])
}

/// The instruction defining `value`.
fn defining(
    function: &Function,
    value: ValueId,
) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// Prove every counter that alone decides when a single-exit loop leaves.
///
/// Constant start and bound give an exact `count`, and so does an
/// equality sentinel a constant distance from the start. Otherwise the
/// proof is symbolic, and only for a pre-tested unit step whose loop is
/// proved finite: an exclusive or `!=` test always is; an inclusive one
/// runs forever where `bound` is the end of its type, so needs a
/// `maximum`: a step promised not to wrap, or with `inbounds` the loop's
/// memory accesses. That reads `derived`, which asks this for counts.
pub fn counted(
    unit: &Unit,
    loop_: &Loop,
    facts: Option<&IndexMap<ValueId, Known>>,
    inbounds: bool,
) -> Vec<CountedLoop> {
    counted_unless_stopped(unit, loop_, facts, inbounds).into_iter().filter(|proof| !proof.stops).collect()
}

/// Each loop's `counted_unless_stopped` proofs by header, from what is known
/// without memory.
pub type Counted = IndexMap<i64, Vec<CountedLoop>>;

thread_local! {
    static PROVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many loops this thread has proved the counts of, for a test that a
/// body's are proved once.
pub fn proved() -> usize {
    PROVED.with(std::cell::Cell::get)
}

/// `counted_unless_stopped` of every loop of `unit`'s shape, under the
/// registers it carries.
pub fn counted_all(unit: &Unit) -> Counted {
    counted_renewed(unit, &Counted::default(), |_| true)
}

/// `counted_all`, taking `previous`'s proofs of each loop `dirty` does not
/// name.
pub fn counted_renewed(
    unit: &Unit,
    previous: &Counted,
    dirty: impl Fn(&Loop) -> bool,
) -> Counted {
    let registers = unit.registers();
    let shape = unit.shape();
    // One body under one set of facts: what is known of its congruences is
    // found once for the loops, not once for each proof (610 times, 585 of
    // 6,449 Minstr, on 32 loops).
    let congruent = std::cell::RefCell::new(crate::alias::CongruenceMemo::default());
    let unit = &Unit { congruent: Some(&congruent), ..*unit };
    // A proof asks the proofs of the loops around it (`_ranged`), each of which
    // asks those around it: the outer loops first, so that each is asked of
    // what is already found and not proved again (a nest of D loops proved
    // the outermost 2^D times: 40% of the compile of fib at -O2 with header
    // copy).
    let mut order: Vec<&Loop> = shape.loops.iter().collect();
    order.sort_by_key(|one| std::cmp::Reverse(one.body.len()));
    let mut found = Counted::default();
    for loop_ in order {
        let proofs = match previous.get(&loop_.header) {
            Some(proofs) if !dirty(loop_) => proofs.clone(),
            _ => _counted_unless_stopped(&Unit { counted: Some(&found), ..*unit }, loop_, &registers, false, false),
        };
        found.insert(loop_.header, proofs);
    }
    shape.loops.iter().map(|loop_| (loop_.header, found[&loop_.header].clone())).collect()
}

thread_local! {
    static CHECK_COUNTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `LLRM_CHECK_COUNTED` for this thread, for a test.
pub fn check_counted(on: bool) {
    CHECK_COUNTED.with(|check| check.set(on));
}

/// `counted`, also for a loop that may leave into a block that never returns.
pub fn counted_unless_stopped(
    unit: &Unit,
    loop_: &Loop,
    facts: Option<&IndexMap<ValueId, Known>>,
    inbounds: bool,
) -> Vec<CountedLoop> {
    // The manager's, where the facts asked of are the unit's own registers.
    if let (false, Some(held), Some(registers)) = (inbounds, unit.counted, unit.registers) {
        if facts.is_none_or(|facts| std::ptr::eq(facts, registers)) {
            if let Some(found) = held.get(&loop_.header) {
                if llrm_support::env_flag!("LLRM_CHECK_COUNTED") || CHECK_COUNTED.with(std::cell::Cell::get) {
                    assert!(
                        *found == _counted_unless_stopped(unit, loop_, registers, false, false),
                        "the counted proofs a unit carries are not those of the body it stands over: stale"
                    );
                }
                return found.clone();
            }
        }
    }
    let local = std::cell::RefCell::new(Counted::default());
    let unit = &match unit.proved {
        Some(_) => *unit,
        None => Unit { proved: Some(&local), ..*unit },
    };
    let cache = unit.proved.filter(|_| !inbounds && facts.is_none());
    if let Some(found) = cache.and_then(|cache| cache.borrow().get(&loop_.header).cloned()) {
        return found;
    }
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = unit.registers();
            &*computed
        }
    };
    let proofs = _counted_unless_stopped(unit, loop_, facts, inbounds, false);
    if let Some(cache) = cache {
        cache.borrow_mut().insert(loop_.header, proofs.clone());
    }
    proofs
}

/// `counted_unless_stopped`, also for a loop that has other ways out that go
/// on, not only into a block that never returns: each proof has `stops` set
/// where there is one, and its count is the trips as long as the loop is not
/// left early. What rewrites the loop's own exit test and the counters it reads
/// (lsr) may use it, where a final value or a deleted loop may not.
pub fn counted_leaving(
    unit: &Unit,
    loop_: &Loop,
    facts: Option<&IndexMap<ValueId, Known>>,
    inbounds: bool,
) -> Vec<CountedLoop> {
    let local = std::cell::RefCell::new(Counted::default());
    let unit = &match unit.proved {
        Some(_) => *unit,
        None => Unit { proved: Some(&local), ..*unit },
    };
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = unit.registers();
            &*computed
        }
    };
    _counted_unless_stopped(unit, loop_, facts, inbounds, true)
}

fn _counted_unless_stopped(
    unit: &Unit,
    loop_: &Loop,
    facts: &IndexMap<ValueId, Known>,
    inbounds: bool,
    leaving: bool,
) -> Vec<CountedLoop> {
    PROVED.with(|count| count.set(count.get() + 1));
    let function = unit.function;
    let Some(shape) = _control(function, loop_, leaving) else { return Vec::new() };
    let branch = function.terminator(cfg::block(shape.block)).expect("_control proved a branch");
    let [condition, Operand::Block(taken), Operand::Block(_)] = function.instruction(branch).operands[..] else {
        return Vec::new();
    };
    let Some((compare, icmp)) = unit.defining(condition) else { return Vec::new() };
    let Opcode::ICmp(predicate) = icmp.opcode else { return Vec::new() };
    let continuing = if loop_.body.contains(&cfg::id(taken)) { predicate } else { predicate.inverse() };
    let mut proven = _proven(unit, loop_, facts, inbounds, &shape, branch, compare, continuing, false);
    if shape.after {
        proven.extend(
            _proven(
                unit,
                loop_,
                facts,
                inbounds,
                &_Control { posttested: true, ..shape },
                branch,
                compare,
                continuing,
                false,
            )
            .into_iter()
            .filter(|proof| proof.stepped),
        );
    }
    proven
}

/// Every counter whose compare `compare`, which keeps the loop going while
/// `continuing` holds, ends it at the branch `shape` places.
#[allow(clippy::too_many_arguments)]
fn _proven(
    unit: &Unit,
    loop_: &Loop,
    facts: &IndexMap<ValueId, Known>,
    inbounds: bool,
    shape: &_Control,
    branch: InstId,
    compare: InstId,
    continuing: IntPredicate,
    shifted: bool,
) -> Vec<CountedLoop> {
    let function = unit.function;
    let icmp = function.instruction(compare);
    let inside = &loop_.body;
    let latch = *loop_.latches.first().expect("_control proved one latch");
    let still = invariant(function, inside);

    let basic = basics(unit, loop_);
    let mut counters = basic.values().map(|one| (one.clone(), one.value)).collect::<Vec<_>>();
    // A counter plus a constant, tested before a trip, counts as a counter
    // that started that far on: `i + 8 < len` ends the loop at `len - 8`.
    if shifted && !shape.posttested {
        for (&result, of) in &recurrences(unit, loop_, &basic).values {
            let width = of.width();
            if of.pointer.is_some()
                || basic.contains_key(&result)
                || !of.start.terms.is_empty()
                || !of.step.terms.is_empty()
            {
                continue;
            }
            // Equal steps differ by a constant: the value is that counter,
            // shifted.
            let same = |one: &&Affine| {
                matches!(
                    (&one.start, &one.step),
                    (AffineOperand::Const(start), AffineOperand::Const(step)) if start.width == width && step.n == of.step.constant
                )
            };
            if let Some(source) = basic.values().find(same) {
                let start = AffineOperand::constant(of.start.constant.clone(), width);
                counters.push((
                    Affine { value: result, start, step: source.step.clone(), header: source.header },
                    source.value,
                ));
            }
        }
    }
    // A wide counter tested through a truncation, which equality sees as the
    // narrow counter of the same low bits: LSR keeps the wide value a store
    // reads and tests `trunc(i + 1) == 20000`. Only a loop tested at its
    // latch, for equality, so no wrap promise of the wide step is read.
    let mut narrow: llrm_mir::dense::IdMap<ValueId, bool> = llrm_mir::dense::IdMap::new();
    if shape.posttested && continuing == IntPredicate::Ne {
        for (counter, source) in counters.clone() {
            let Some(phi) = defining(function, source).filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
            else {
                continue;
            };
            let Some(Operand::Value(update)) = incoming(function, phi, cfg::block(latch)) else { continue };
            let (AffineOperand::Const(start), AffineOperand::Const(step)) = (&counter.start, &counter.step) else {
                continue;
            };
            if counter.value != source {
                continue;
            }
            for (of, stepped) in [(source, false), (update, true)] {
                for one in function.users(of) {
                    let user = function.instruction(one.user);
                    let Some(result) = user.result.filter(|_| user.opcode == Opcode::Cast(CastOp::Trunc)) else {
                        continue;
                    };
                    let Some(width) = unit.int_bits(Operand::Value(result)).filter(|&width| width < start.width) else {
                        continue;
                    };
                    if !function.parent(one.user).is_some_and(|block| inside.contains(&cfg::id(block))) {
                        continue;
                    }
                    narrow.insert(result, stepped);
                    counters.push((
                        Affine {
                            value: result,
                            start: AffineOperand::constant(start.n.clone(), width),
                            step: AffineOperand::constant(step.n.clone(), width),
                            header: counter.header,
                        },
                        source,
                    ));
                }
            }
        }
    }
    let mut proven = Vec::new();
    for (counter, source) in &counters {
        let Some(phi) = defining(function, *source).filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        else {
            continue;
        };
        let Some(Operand::Value(update)) = incoming(function, phi, cfg::block(latch)) else { continue };
        let narrowed = narrow.get(&counter.value).copied();
        let mut tested = BTreeMap::from([(counter.value, narrowed.unwrap_or(false))]);
        if shape.posttested && narrowed.is_none() {
            tested.insert(update, true);
        }
        let Some((width, bound, mirrored, stepped)) = _compared(unit, icmp, &tested) else { continue };
        let mut test = if mirrored { continuing.swapped() } else { continuing };
        let Some(step) = _signed(&counter.step, facts, counter.start.width()) else { continue };
        if step == BigInt::from(0) || width != counter.start.width() {
            continue;
        }
        let Some(bound) = term(unit, bound).filter(|bound| bound.width() == width) else { continue };
        if matches!(
            &bound,
            AffineOperand::Value(value, _) if !still.contains(*value)
        ) {
            continue;
        }
        if narrowed.is_some() && test != IntPredicate::Ne {
            continue;
        }
        let zero = BigInt::from(0);
        if !(test == IntPredicate::Ne || (_ascending(test) && step > zero) || (_descending(test) && step < zero)) {
            continue;
        }
        // Tested before its step at the latch, the counter against a constant
        // is the stepped counter against that constant one step on
        // (LLVM's exit count reads the test the same way): the one form the
        // proofs below count. For an ordered test the step must not
        // wrap; for equality a wrapping sum is a bijection.
        let (mut bound, mut stepped, mut shifted) = (bound, stepped, false);
        if shape.posttested
            && !stepped
            && function.parent(branch) == Some(cfg::block(latch))
            && let AffineOperand::Const(limit) = &bound
        {
            let unsigned = _unsigned(test);
            let (low, high) = _extent(unsigned, width);
            let moved = if unsigned {
                mod_floor(&limit.n, &(BigInt::from(1) << width))
            } else {
                _signed_value(&limit.n, width)
            } + &step;
            if test == IntPredicate::Ne {
                bound = AffineOperand::constant(masked(&moved, width), width);
                (stepped, shifted) = (true, true);
            } else if low <= moved
                && moved <= high
                && _promised(function, update, &step, unsigned, _signed(&counter.start, facts, width).as_ref())
            {
                bound = AffineOperand::constant(masked(&moved, width), width);
                (stepped, shifted) = (true, true);
            }
        } else if shape.posttested
            && !stepped
            && function.parent(branch) == Some(cfg::block(latch))
            && abs(&step) == BigInt::from(1)
            && let Some(inclusive) = _after_step(test, &step)
            && _promised(function, update, &step, _unsigned(test), _signed(&counter.start, facts, width).as_ref())
        {
            // A bound that is no constant stays: `i < b` before the step is
            // `i + 1 <= b` after it, when the step does not wrap.
            test = inclusive;
            (stepped, shifted) = (true, true);
        }
        // The same against `x - step`: `i < x - 1` before a step of 1 is the
        // stepped counter `< x`. The subtract must not wrap in the test's
        // signedness, or `x` is not one step on from the bound.
        if shape.posttested
            && !stepped
            && test != IntPredicate::Ne
            && function.parent(branch) == Some(cfg::block(latch))
            && let AffineOperand::Value(value, _) = &bound
            && let Some(on) = _one_step_on(unit, *value, &step, _unsigned(test), width)
            && _promised(function, update, &step, _unsigned(test), _signed(&counter.start, facts, width).as_ref())
        {
            (bound, stepped, shifted) = (on, true, true);
        }
        let start = counter.start.clone();
        let limit = _constant(&bound, facts, width);
        let counted_from = |start: &AffineOperand| {
            let begin = _constant(start, facts, width);
            let difference = _difference(unit, &bound, start, begin.as_ref(), limit.as_ref(), facts, width);
            let count = match (&difference, &begin, &limit) {
                (Some(difference), _, _) if test == IntPredicate::Ne => {
                    _equal_after(difference, &step, width, shape.posttested, stepped)
                }
                (_, Some(begin), Some(limit)) if test != IntPredicate::Ne || difference.is_none() => {
                    _ordered_after(begin, limit, &step, test, width, shape.posttested, stepped)
                }
                _ => None,
            };
            (begin, count)
        };
        let (mut begin, mut count) = counted_from(&start);
        // The start as counted: itself, or the entry value it is proven to
        // equal.
        let mut equal = start.clone();
        if count.is_none()
            && narrowed.is_none()
            && let Some((entry, rewinds)) = _carried(unit, loop_, &start, *source, update, width)
            && let (entered, Some(trips)) = counted_from(&entry)
            && rewinds.iter().all(|(stepped_root, offset)| {
                let exited = &trips - BigInt::from(u8::from(shape.posttested && !stepped_root));
                mod_floor(&(offset + exited * &step), &(BigInt::from(1) << width)) == BigInt::from(0)
            })
        {
            (begin, count, equal) = (entered, Some(trips), entry);
        }
        let (mut first, mut last) = (None, None);
        let mut entry_guarded = false;
        // Tested after its trips with a bound not known, it runs the trips a
        // pre-tested one would where its branch over the entry proves one.
        let entered = shape.posttested
            && stepped
            && test != IntPredicate::Ne
            && (begin.is_none() || limit.is_none())
            && _entered(unit, &shape, &start, &bound, test, width);
        let mut reach = Reach::Distance;
        let maximum = if let Some(count) = &count {
            (first, last) = _signed_span(&equal, facts, width, count, &step);
            Some(count.clone())
        } else if test == IntPredicate::Ne && (shape.posttested || abs(&step) != BigInt::from(1)) {
            // Tested for equality, the loop ends where the counter reaches the
            // bound.
            let Some(solved) = _solved(unit, facts, &start, &bound, &step, width) else { continue };
            let Reach::Solved { bits, .. } = &solved else { unreachable!("_solved solves") };
            let period = BigInt::from(1) << *bits;
            reach = solved;
            // Counted from a start the branch over the entry proves is not the
            // bound: `n == 0` skips a loop that ends at `n - 1 ==
            // 0`, which then runs `n` trips (the `(bound - start) / step` of a
            // loop tested after its trip is wrong only where it
            // starts at the bound).
            entry_guarded = shape.posttested
                && stepped
                && abs(&step) == BigInt::from(1)
                && _entered(unit, &shape, &start, &bound, IntPredicate::Ne, width);
            // An access through an `inbounds` pointer bounds the trips of a
            // loop tested for equality too, as of one tested for order.
            let bounded = if inbounds { inbounds_backedges(unit, loop_) } else { None };
            Some(period).into_iter().chain(bounded).min()
        } else if test != IntPredicate::Ne && abs(&step) != BigInt::from(1) {
            // An ordered test by more than one: promised not to wrap past the
            // bound, the counter reaches it in the distance divided by the
            // step.
            if !_promised(function, update, &step, _unsigned(test), _signed(&start, facts, width).as_ref())
                || (shape.posttested && !entered)
            {
                continue;
            }
            entry_guarded = shape.posttested;
            reach = Reach::Ceil { strict: !_inclusive(test) };
            Some((BigInt::from(1) << width) / abs(&step) + 1)
        } else if (shape.posttested && !entered) || abs(&step) != BigInt::from(1) {
            continue;
        } else {
            entry_guarded = shape.posttested;
            let promised = _promised(function, update, &step, _unsigned(test), _signed(&start, facts, width).as_ref());
            // A bound known only by its range still bounds the trips: by its
            // end the counter walks toward.
            let reached = limit.clone().or_else(|| _extent_toward(unit, &bound, facts, step > BigInt::from(0)));
            let found =
                _unit_maximum(unit, loop_, width, begin.as_ref(), reached.as_ref(), &step, test, inbounds, promised);
            if found.is_none() && _inclusive(test) {
                continue;
            }
            found
        };
        proven.push(CountedLoop {
            counter: counter.clone(),
            phi,
            compare,
            branch,
            start: begin.map_or_else(|| start.clone(), |begin| AffineOperand::constant(begin, width)),
            bound: limit.map_or_else(|| bound.clone(), |limit| AffineOperand::constant(limit, width)),
            test,
            preheader: shape.preheader,
            latch,
            entered: shape.entered,
            exit: shape.exit,
            maximum,
            step,
            posttested: shape.posttested,
            stepped,
            shifted,
            stops: shape.stops,
            leaves: shape.leaves,
            entry_guarded,
            reach,
            count,
            first,
            last,
        });
    }
    proven
}

/// Whether the branches over the loop's preheader prove its first test
/// would continue: `start test bound`, where the loop is entered.
fn _entered(
    unit: &Unit,
    shape: &_Control,
    start: &AffineOperand,
    bound: &AffineOperand,
    test: IntPredicate,
    width: u32,
) -> bool {
    let Some(preheader) = shape.preheader else { return false };
    let (value, limit) = (start, bound);
    // A side that is a constant off a value is that value and the constant: the
    // guards read in the same terms are what the two share, `a != 1` and `a
    // - 1 != 0`.
    let anchor = |one: &Scev| -> Scev {
        let [(product, factor)] = one.terms.iter().collect::<Vec<_>>()[..] else { return one.clone() };
        let Some(value) = product.single().filter(|_| *factor == BigInt::from(1)) else { return one.clone() };
        match anchored(unit, &AffineOperand::Value(value, width), width, None, None) {
            (Some(root), offset) => Scev::unknown(root, width)
                .plus(&Scev::constant(offset, width))
                .plus(&Scev::constant(one.constant.clone(), width)),
            _ => one.clone(),
        }
    };
    let (start, bound) = (Scev::of(start, width), Scev::of(bound, width));
    // The guards over the preheader, found once for the questions below.
    let guards = std::cell::OnceCell::new();
    let guards = || guards.get_or_init(|| crate::guards::guards(unit, preheader));
    let held = if start.known().is_some() && bound.known().is_some() {
        crate::guards::holds(unit, preheader, test, &start, &bound)
    } else {
        crate::guards::holds_with(unit, preheader, guards(), test, &start, &bound)
    };
    if held || _ranged(unit, preheader, value, limit, test, width) {
        return true;
    }
    // A signed ordered test reads them at an offset too (`n > 2` proves `1 < n
    // - 1`), where the sides are sums that cannot wrap.
    if matches!(
        test,
        IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge
    ) {
        let rooted = |one: &AffineOperand| {
            let (root, offset) = anchored(unit, one, width, None, Some(llrm_mir::opcode::Flags::NSW));
            let number = Scev::constant(offset, width);
            root.map_or(number.clone(), |root| Scev::unknown(root, width).plus(&number))
        };
        let (start, bound) = (rooted(value), rooted(limit));
        // A guard on a value that is a root and a constant (`n - 1 >= 0`) is
        // read in the same terms.
        let rooted_guard = |guard: &crate::guards::Guard| {
            let side = |one: &Scev| -> Option<Scev> {
                let [(product, factor)] = one.terms.iter().collect::<Vec<_>>()[..] else { return None };
                let value =
                    product.single().filter(|_| *factor == BigInt::from(1) && one.constant == BigInt::from(0))?;
                Some(rooted(&AffineOperand::Value(value, width)))
            };
            Some(crate::guards::Guard {
                predicate: guard.predicate,
                left: side(&guard.left)?,
                right: guard.right.clone(),
            })
        };
        return guards().iter().any(|guard| crate::guards::implies_at_offset(guard, test, &start, &bound))
            || guards().iter().any(|guard| {
                rooted_guard(guard)
                    .is_some_and(|rooted| crate::guards::implies_at_rooted_offset(&rooted, test, &start, &bound))
            });
    }
    // Only an equality test reads the guards at an offset (`a != 1` proves `a -
    // 1 != 0`), and only where the ones above do not prove it as they
    // stand.
    matches!(test, IntPredicate::Eq | IntPredicate::Ne) && {
        let (start, bound) = (anchor(&start), anchor(&bound));
        guards()
            .iter()
            .any(
                |guard| crate::guards::implies(
                    &crate::guards::Guard {
                        predicate: guard.predicate,
                        left: anchor(&guard.left),
                        right: anchor(&guard.right),
                    },
                    test,
                    &start,
                    &bound,
                ),
            )
    }
}

/// Whether the counters of the loops around the entry put the start of a
/// counter on the loop's side of a constant bound for good: a signed compare of
/// `outer + c`, `outer` the counter of an enclosing counted loop, whose values
/// the proof of that loop bounds. A guard the program no longer holds was
/// folded on that range, and a loop entered behind it is entered all the same.
fn _ranged(
    unit: &Unit,
    at: i64,
    start: &AffineOperand,
    bound: &AffineOperand,
    test: IntPredicate,
    width: u32,
) -> bool {
    let (AffineOperand::Value(value, _), AffineOperand::Const(limit)) = (start, bound) else { return false };
    let function = unit.function;
    // `start` is `outer + offset`, or `outer`.
    let (outer, offset) = match function.value(*value).def {
        ValueDef::Instruction(made) => {
            let made = function.instruction(made);
            match (&made.opcode, made.operands.as_slice()) {
                (Opcode::Binary(BinaryOp::Add), [Operand::Value(base), other])
                | (Opcode::Binary(BinaryOp::Add), [other, Operand::Value(base)]) => {
                    let Some(constant) = unit.int_constant(*other) else { return false };
                    (*base, BigInt::from(constant))
                }
                (Opcode::Binary(BinaryOp::Sub), [Operand::Value(base), other]) => {
                    let Some(constant) = unit.int_constant(*other) else { return false };
                    (*base, -BigInt::from(constant))
                }
                _ => (*value, BigInt::from(0)),
            }
        }
        ValueDef::Argument(_) => return false,
    };
    let shape = unit.shape();
    let limit = _signed_value(&limit.n, width);
    for around in shape.loops.iter().filter(|one| one.body.contains(&at)) {
        for proof in counted_unless_stopped(unit, around, None, false) {
            if proof.counter.value != outer || proof.width() != width {
                continue;
            }
            // Its values run from the start toward the bound: the start is the
            // lowest (highest) of an ascending (descending) one,
            // and the span, where the count is known, bounds the other end too.
            let AffineOperand::Const(origin) = &proof.start else { continue };
            let origin = _signed_value(&origin.n, width);
            let (low, high) = match (proof.span(), proof.step > BigInt::from(0)) {
                (Some((low, high)), _) => (low, high),
                (None, true) => (origin.clone(), BigInt::from(1) << (width - 1)),
                (None, false) => (-(BigInt::from(1) << (width - 1)), origin.clone()),
            };
            let (low, high) = (low + &offset, high + &offset);
            let holds = match test {
                IntPredicate::Sge => low >= limit,
                IntPredicate::Sgt => low > limit,
                IntPredicate::Sle => high <= limit,
                IntPredicate::Slt => high < limit,
                _ => false,
            };
            if holds {
                return true;
            }
        }
    }
    false
}

fn _signed_value(
    n: &BigInt,
    width: u32,
) -> BigInt {
    let modulus = BigInt::from(1) << width;
    let low = mod_floor(n, &modulus);
    if low >= (BigInt::from(1) << (width - 1)) { low - modulus } else { low }
}

/// Where a loop leaves, and after how many trips: an exiting block, and
/// the backedges taken before its branch leaves, where counted.
#[derive(Clone, Debug)]
pub struct ExitCount {
    pub block: i64,
    pub branch: InstId,
    pub exit: i64,
    /// The backedges taken before it leaves: the least of these. A branch
    /// leaving as soon as any of its compares fails takes the least of
    /// theirs; one leaving only when all fail, their one count.
    pub taken: Option<Vec<Scev>>,
    /// The proofs its compares were counted by.
    pub proofs: Vec<CountedLoop>,
}

/// Every exit of a single-latch loop, in dominance order where they
/// dominate its latch, each counted where its compares are: LLVM's exit
/// limits. An exit the latch does not follow is not counted.
pub fn exits(
    unit: &Unit,
    loop_: &Loop,
    facts: Option<&IndexMap<ValueId, Known>>,
    inbounds: bool,
) -> Vec<ExitCount> {
    let local = std::cell::RefCell::new(Counted::default());
    let unit = &match unit.proved {
        Some(_) => *unit,
        None => Unit { proved: Some(&local), ..*unit },
    };
    let function = unit.function;
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = unit.registers();
            &*computed
        }
    };
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return Vec::new() };
    let shape = unit.shape();
    let block_of = |at: i64| cfg::Block { at, succ: cfg::successors_of(function, at) };
    let inside = &loop_.body;
    let outside = loop_.entering(function);
    let preheader = match outside.first() {
        Some(&one) if outside.len() == 1 && block_of(one).succ.as_slice() == [loop_.header] => Some(one),
        _ => None,
    };
    let mut exiting = inside
        .iter()
        .copied()
        .filter(|at| block_of(*at).succ.iter().any(|to| !inside.contains(to)))
        .collect::<Vec<_>>();
    // Those the latch follows are in a chain: each dominates the next.
    exiting.sort_by_key(|&at| {
        (
            !shape.dominance.dominates(unit.function, at, latch),
            inside.iter().filter(|&&other| shape.dominance.dominates(unit.function, other, at)).count(),
        )
    });
    exiting
        .into_iter()
        .map(|at| {
            let branch = function.terminator(cfg::block(at)).expect("a terminated block");
            let block = block_of(at);
            let exit = block.succ.iter().copied().find(|to| !inside.contains(to)).expect("an exiting block");
            let mut found = ExitCount { block: at, branch, exit, taken: None, proofs: Vec::new() };
            let operands = &function.instruction(branch).operands;
            let [Operand::Value(condition), Operand::Block(taken), Operand::Block(_)] = operands[..] else {
                return found;
            };
            let (Some(entered), true) =
                (block.succ.iter().copied().find(|to| inside.contains(to)), block.succ.len() == 2)
            else {
                return found;
            };
            if !shape.dominance.dominates(unit.function, at, latch) {
                return found;
            }
            let shaped = |posttested: bool| _Control {
                block: at,
                preheader,
                entered,
                exit,
                posttested,
                stops: false,
                leaves: false,
                after: false,
            };
            let stays = inside.contains(&cfg::id(taken));
            let (leaves, first) = _leaves(function, condition, stays);
            let mut counts = Vec::new();
            for compare in leaves {
                let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { return found };
                let continuing = if stays { predicate } else { predicate.inverse() };
                // Tested after the trip where it is the latch; in the body, a
                // compare of the stepped value is tested after
                // the step, one of the header's phi before it.
                let tries: &[bool] = if at == latch { &[true] } else { &[false, true] };
                let Some(proof) = tries.iter().find_map(|&posttested| {
                    _proven(unit, loop_, facts, inbounds, &shaped(posttested), branch, compare, continuing, true)
                        .into_iter()
                        .next()
                        .filter(|proof| posttested == (at == latch) || proof.stepped == posttested)
                }) else {
                    return found;
                };
                let Some(count) = _backedges(&proof) else { return found };
                counts.push(count);
                found.proofs.push(proof);
            }
            // Leaving only once every compare fails is counted where they all
            // agree.
            if !first {
                counts.dedup();
                if counts.len() != 1 {
                    return found;
                }
            }
            found.taken = Some(counts);
            found
        })
        .collect()
}

/// The compares a branch on `condition` reads through `and`s and `or`s,
/// and whether it leaves as soon as one fails: a branch staying while
/// `condition` holds (`stays`) leaves at the first to fail of an `and`'s,
/// one leaving while it holds at the first to hold of an `or`'s.
fn _leaves(
    function: &Function,
    condition: ValueId,
    stays: bool,
) -> (Vec<InstId>, bool) {
    let joined = |value: ValueId| -> Option<(BinaryOp, [Operand; 2])> {
        let inst = defining(function, value)?;
        let op = function.instruction(inst);
        match (&op.opcode, &op.operands[..]) {
            (Opcode::Binary(kind @ (BinaryOp::And | BinaryOp::Or)), [left, right]) => Some((*kind, [*left, *right])),
            _ => None,
        }
    };
    let Some((kind, _)) = joined(condition) else { return (defining(function, condition).into_iter().collect(), true) };
    let mut leaves = Vec::new();
    let mut pending = vec![condition];
    while let Some(value) = pending.pop() {
        match joined(value) {
            Some((one, parts)) if one == kind => pending.extend(parts.iter().filter_map(|part| match part {
                Operand::Value(value) => Some(*value),
                _ => None,
            })),
            _ => leaves.extend(defining(function, value)),
        }
    }
    let first = (kind == BinaryOp::And) == stays;
    (leaves, first)
}

/// The backedges a proof's loop takes before it leaves at the proof's
/// branch: its trips, one fewer where tested after them.
fn _backedges(proof: &CountedLoop) -> Option<Scev> {
    let width = proof.width();
    if let Some(count) = &proof.count {
        return Some(Scev::constant(count - u8::from(proof.posttested), width));
    }
    if proof.posttested {
        return None;
    }
    let (ahead, behind) =
        if proof.step > BigInt::from(0) { (&proof.bound, &proof.start) } else { (&proof.start, &proof.bound) };
    Some(
        Scev::of(ahead, width)
            .minus(&Scev::of(behind, width))
            .plus(&Scev::constant(u8::from(proof.inclusive()), width)),
    )
}

/// The backedges a loop takes: the least of every exit's, where each is
/// counted and the latch follows them all. LLVM's exact backedge-taken count.
pub fn backedges(exits: &[ExitCount]) -> Option<Vec<Scev>> {
    let mut least = Vec::new();
    for one in exits {
        least.extend(one.taken.clone()?);
    }
    (!least.is_empty()).then_some(least)
}

/// The most backedges a loop takes: the least of the counted exits'.
/// LLVM's symbolic maximum.
pub fn most_backedges(exits: &[ExitCount]) -> Vec<Scev> {
    exits.iter().filter_map(|one| one.taken.clone()).flatten().collect()
}

/// A start carried round an enclosing loop: a phi outside `loop_` whose
/// arms are one entry value, or the counter (`phi` or its `update`) as it
/// left plus a constant. The entry value and, per other arm, whether it
/// reads the update and its constant.
///
/// If each such constant takes the counter from its exit value back to
/// where it began, the start always equals the entry value: by induction
/// over the start's evaluations, each run began at it and so left where
/// its trips from it say. The arm is read outside `loop_`, after the run
/// that the loop's header, dominating it, proves came in between.
fn _carried(
    unit: &Unit,
    loop_: &Loop,
    start: &AffineOperand,
    phi: ValueId,
    update: ValueId,
    width: u32,
) -> Option<(AffineOperand, Vec<(bool, BigInt)>)> {
    let function = unit.function;
    let &AffineOperand::Value(value, _) = start else { return None };
    let (inst, op) = unit.defining(Operand::Value(value))?;
    if op.opcode != Opcode::Phi || loop_.body.contains(&cfg::id(function.parent(inst)?)) {
        return None;
    }
    let (mut entry, mut rewinds) = (None, Vec::new());
    for pair in op.operands.chunks(2) {
        match _rewind(unit, loop_, pair[0], phi, update) {
            Some((root, offset)) => rewinds.push((root == update, offset)),
            None if entry.is_none_or(|one| one == pair[0]) => entry = Some(pair[0]),
            None => return None,
        }
    }
    (!rewinds.is_empty()).then_some(())?;
    Some((term(unit, entry?).filter(|entry| entry.width() == width)?, rewinds))
}

/// `operand` as `phi` or `update` plus a constant, through adds and
/// single-arm phis outside `loop_`.
fn _rewind(
    unit: &Unit,
    loop_: &Loop,
    mut operand: Operand,
    phi: ValueId,
    update: ValueId,
) -> Option<(ValueId, BigInt)> {
    let mut offset = BigInt::from(0);
    loop {
        let Operand::Value(value) = operand else { return None };
        if value == phi || value == update {
            return Some((value, offset));
        }
        let (inst, op) = unit.defining(operand)?;
        if loop_.body.contains(&cfg::id(unit.function.parent(inst)?)) {
            return None;
        }
        operand = match (&op.opcode, &op.operands[..]) {
            (Opcode::Phi, [one, _]) => *one,
            (Opcode::Binary(BinaryOp::Add), &[left, right]) => {
                let (other, constant) = match (unit.int_constant(left), unit.int_constant(right)) {
                    (_, Some(constant)) => (left, constant),
                    (Some(constant), _) => (right, constant),
                    _ => return None,
                };
                offset += BigInt::from(constant);
                other
            }
            _ => return None,
        };
    }
}

/// `(width, bound, mirrored, stepped)` where `icmp` tests a counter value in
/// `tested`.
fn _compared(
    unit: &Unit,
    icmp: &Instruction,
    tested: &BTreeMap<ValueId, bool>,
) -> Option<(u32, Operand, bool, bool)> {
    for (index, operand) in icmp.operands.iter().enumerate().take(2) {
        let Operand::Value(value) = operand else { continue };
        let Some(&stepped) = tested.get(value) else { continue };
        return Some((unit.int_bits(*operand)?, icmp.operands[1 - index], index == 1, stepped));
    }
    None
}

/// Whether the step `update` makes is promised not to wrap as `unsigned` or
/// signed integers. A counter that starts at or above zero and only goes up
/// without a signed wrap stays in `0 ..= signed max`, where a signed sum is the
/// unsigned one: `nsw` there is `nuw` too, as LLVM's SCEV infers it. `start` is
/// the counter's signed start where known.
/// `x` where `value` is `x - step` and the difference does not wrap in the
/// test's signedness: the bound one step on.
fn _one_step_on(
    unit: &Unit,
    value: ValueId,
    step: &BigInt,
    unsigned: bool,
    width: u32,
) -> Option<AffineOperand> {
    let function = unit.function;
    let op = function.instruction(defining(function, value)?);
    let stated = llrm_mir::facts::Facts::of_flags(op.flags);
    let no_wrap = if unsigned { stated.no_unsigned_wrap() } else { stated.no_signed_wrap() };
    let (Operand::Value(base), offset) = (op.operands.first().copied()?, unit.int_constant(*op.operands.get(1)?)?)
    else {
        return None;
    };
    let offset = BigInt::from(offset);
    let offset = if unsigned { offset } else { _signed_value(&offset, width) };
    let wanted = match op.opcode {
        Opcode::Binary(BinaryOp::Add) => -step,
        Opcode::Binary(BinaryOp::Sub) => step.clone(),
        _ => return None,
    };
    let wanted = if unsigned { mod_floor(&wanted, &(BigInt::from(1) << width)) } else { _signed_value(&wanted, width) };
    (no_wrap && offset == wanted).then(|| AffineOperand::Value(base, width))
}

fn _promised(
    function: &Function,
    update: ValueId,
    step: &BigInt,
    unsigned: bool,
    start: Option<&BigInt>,
) -> bool {
    let Some(inst) = defining(function, update) else { return false };
    let op = function.instruction(inst);
    let upward = step > &BigInt::from(0);
    let stated = llrm_mir::facts::Facts::of_flags(op.flags);
    let climbing = stated.no_signed_wrap() && start.is_some_and(|start| start >= &BigInt::from(0));
    match op.opcode {
        Opcode::Binary(BinaryOp::Add) if unsigned => upward && (stated.no_unsigned_wrap() || climbing),
        Opcode::Binary(BinaryOp::Sub) if unsigned => !upward && stated.no_unsigned_wrap(),
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => stated.no_signed_wrap(),
        _ => false,
    }
}

/// `bound - start` modulo the width, when constant: both constant, or `bound =
/// start + c`.
fn _difference(
    unit: &Unit,
    bound: &AffineOperand,
    start: &AffineOperand,
    begin: Option<&BigInt>,
    limit: Option<&BigInt>,
    facts: &IndexMap<ValueId, Known>,
    width: u32,
) -> Option<BigInt> {
    if let (Some(begin), Some(limit)) = (begin, limit) {
        return Some(masked(&(limit - begin), width));
    }
    let (root, ahead) = anchored(unit, bound, width, Some(facts), None);
    let (other, behind) = anchored(unit, start, width, Some(facts), None);
    (root == other).then(|| masked(&(ahead - behind), width))
}

/// `term` as terms times coefficients, through adds and subtracts. A
/// constant is a term; `headers` stay terms. A value no rule expands is a
/// term too when `opaque`, else None.
pub fn linear(
    unit: &Unit,
    term_: &AffineOperand,
    headers: &BTreeSet<ValueId>,
    width: u32,
    opaque: bool,
    visiting: &BTreeSet<ValueId>,
    cached: &mut IndexMap<AffineOperand, IndexMap<AffineOperand, BigInt>>,
) -> Option<IndexMap<AffineOperand, BigInt>> {
    let value = match term_ {
        AffineOperand::Const(constant) if constant.width == width => None,
        AffineOperand::Value(value, one) if *one == width => Some(*value),
        _ => return None,
    };
    let leaf = || Some(IndexMap::from_iter([(term_.clone(), BigInt::from(1))]));
    let made = value
        .filter(|value| !headers.contains(value))
        .and_then(|value| unit.defining(Operand::Value(value)))
        .filter(|(_, op)| op.opcode != Opcode::Phi);
    let (Some(value), Some((_, op))) = (value, made) else { return leaf() };
    if visiting.contains(&value) {
        return None;
    }
    if let Some(found) = cached.get(term_) {
        return Some(found.clone());
    }
    let parts = match op.opcode {
        Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub)) => {
            let (Some(left), Some(right)) = (term(unit, op.operands[0]), term(unit, op.operands[1])) else {
                return if opaque { leaf() } else { None };
            };
            vec![(left, BigInt::from(1)), (right, BigInt::from(if kind == BinaryOp::Sub { -1 } else { 1 }))]
        }
        _ => return if opaque { leaf() } else { None },
    };
    let mut result = IndexMap::<AffineOperand, BigInt>::default();
    let mut deeper = visiting.clone();
    deeper.insert(value);
    for (source, coefficient) in parts {
        for (one, factor) in linear(unit, &source, headers, width, opaque, &deeper, cached)? {
            *result.entry(one).or_insert_with(|| BigInt::from(0)) += &coefficient * factor;
        }
    }
    let kept = result.into_iter().filter(|(_, factor)| *factor != BigInt::from(0)).collect::<IndexMap<_, _>>();
    cached.insert(term_.clone(), kept.clone());
    Some(kept)
}

/// How far `one` lies above `other`, where their terms other than constants
/// agree.
///
/// Strength reduction starts `a[i].x` at `n + (m + 600)` and `a[i].y` at
/// `n + (m + 606)`: no single root, but 6 apart.
pub fn distance(
    unit: &Unit,
    one: &AffineOperand,
    other: &AffineOperand,
    width: u32,
) -> Option<BigInt> {
    let terms_of = |of: &AffineOperand| {
        linear(unit, of, &BTreeSet::new(), width, true, &BTreeSet::new(), &mut IndexMap::default())
    };
    let mut terms = terms_of(one)?;
    for (one, factor) in terms_of(other)? {
        *terms.entry(one).or_insert_with(|| BigInt::from(0)) -= factor;
    }
    let mut apart = BigInt::from(0);
    for (one, factor) in terms {
        match one {
            AffineOperand::Const(constant) => apart += &constant.n * factor,
            _ if factor == BigInt::from(0) => {}
            _ => return None,
        }
    }
    Some(masked(&apart, width))
}

/// `term` as a root value plus a constant, through constant adds; a number has
/// no root.
///
/// Two values with one root are a constant apart, which is how a loop from
/// `x - 32` to `x` is counted and how two counters starting 4 apart share one.
/// `anchored`, through the adds and subtractions that carry `no_wrap` only
/// where it is given: an ordered compare reads a value as its root plus a
/// constant only where the sum cannot wrap.
pub fn anchored(
    unit: &Unit,
    term_: &AffineOperand,
    width: u32,
    facts: Option<&IndexMap<ValueId, Known>>,
    no_wrap: Option<llrm_mir::opcode::Flags>,
) -> (Option<ValueId>, BigInt) {
    let empty = IndexMap::default();
    let facts = facts.unwrap_or(&empty);
    let mut term_ = term_.clone();
    let mut offset = BigInt::from(0);
    while let AffineOperand::Value(value, one) = term_ {
        if one != width {
            break;
        }
        if let Some(known) = _constant(&term_, facts, width) {
            term_ = AffineOperand::constant(known, width);
            break;
        }
        let Some((_, op)) = unit.defining(Operand::Value(value)) else { break };
        if !matches!(op.opcode, Opcode::Binary(BinaryOp::Add | BinaryOp::Sub))
            || no_wrap.is_some_and(|flag| !op.flags.contains(flag))
        {
            break;
        }
        let (Some(left), Some(right)) = (term(unit, op.operands[0]), term(unit, op.operands[1])) else { break };
        if op.opcode == Opcode::Binary(BinaryOp::Sub) {
            // `x - c`: only the constant on the right.
            let (other, AffineOperand::Const(constant)) = (left, right) else { break };
            offset -= &constant.n;
            term_ = other;
            continue;
        }
        let ((AffineOperand::Const(constant), other) | (other, AffineOperand::Const(constant))) = (left, right) else {
            break;
        };
        offset += &constant.n;
        term_ = other;
    }
    match term_ {
        AffineOperand::Const(constant) => (None, masked(&(&constant.n + offset), width)),
        AffineOperand::Value(value, _) => (Some(value), masked(&offset, width)),
    }
}

/// `start + k * step == bound` solved for the first `k` modulo `width`
/// bits, where the low bits of the distance the step's power of two needs
/// are zero: none for an odd step, else what congruences prove of `start`
/// and `bound`.
fn _solved(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
    start: &AffineOperand,
    bound: &AffineOperand,
    step: &BigInt,
    width: u32,
) -> Option<Reach> {
    let modulus = BigInt::from(1) << width;
    let magnitude = mod_floor(step, &modulus);
    let shift = u32::try_from(magnitude.trailing_zeros()?).ok().filter(|&shift| shift < width)?;
    let bits = width - shift;
    let inverse = modular_inverse(&(&magnitude >> shift), &(BigInt::from(1) << bits))?;
    if shift > 0 && !_low_bits_zero(unit, facts, start, bound, shift) {
        return None;
    }
    Some(Reach::Solved { shift, inverse, bits })
}

/// Whether `bound - start` is a multiple of `2**shift`, by what
/// `congruences` know of each: a value is its residue modulo a modulus, an
/// exact constant a modulus of 0.
fn _low_bits_zero(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
    start: &AffineOperand,
    bound: &AffineOperand,
    shift: u32,
) -> bool {
    // Only the two values' congruences are asked: the body's whole were found
    // for each version of a body of loops (7.2 M, 255 times at 128 loops).
    // A unit that carries the body's has them found already.
    let wanted: Vec<ValueId> = [start, bound]
        .into_iter()
        .filter_map(|one| if let AffineOperand::Value(value, _) = one { Some(*value) } else { None })
        .collect();
    let known = crate::alias::congruences_of(unit, facts, Some(&wanted), unit.congruent);
    if llrm_support::env_flag!("LLRM_CHECK_CONGRUENCES") {
        let whole = crate::alias::congruences_of(unit, facts, None, None);
        for value in &wanted {
            assert_eq!(
                known.get(value),
                whole.get(value),
                "LLRM_CHECK_CONGRUENCES: the congruence of {value:?} asked alone is not the body's"
            );
        }
    }
    let of = |one: &AffineOperand| match one {
        AffineOperand::Const(constant) => Some((BigInt::from(0), constant.n.clone())),
        AffineOperand::Value(value, _) => {
            known.get(value).cloned().or_else(|| facts.get(value).map(|fact| (BigInt::from(0), fact.n.clone())))
        }
    };
    let (start_fact, bound_fact) = (of(start), of(bound));
    if let Some(memo) = unit.congruent {
        memo.borrow_mut().recycle(known);
    }
    let (Some((start_modulus, start_residue)), Some((bound_modulus, bound_residue))) = (start_fact, bound_fact) else {
        return false;
    };
    let power = BigInt::from(1) << shift;
    let common = gcd(start_modulus, bound_modulus);
    (common == BigInt::from(0) || mod_floor(&common, &power) == BigInt::from(0))
        && mod_floor(&(bound_residue - start_residue), &power) == BigInt::from(0)
}

/// Trips until `start + k*step`, tested as the loop is shaped, first equals
/// `start + difference`.
fn _equal_after(
    difference: &BigInt,
    step: &BigInt,
    width: u32,
    posttested: bool,
    stepped: bool,
) -> Option<BigInt> {
    let modulus = BigInt::from(1) << width;
    let lead = u8::from(posttested && stepped);
    let divisor = gcd(mod_floor(step, &modulus), modulus.clone());
    let remaining = mod_floor(&(difference - step * lead), &modulus);
    if mod_floor(&remaining, &divisor) != BigInt::from(0) {
        return None; // never equal: the loop does not end
    }
    let period = &modulus / &divisor;
    let inverse = modular_inverse(&floor_div(&mod_floor(step, &modulus), &divisor), &period)?;
    Some(BigInt::from(u8::from(posttested)) + mod_floor(&(floor_div(&remaining, &divisor) * inverse), &period))
}

/// The low and high of a test's integers: unsigned or signed at `width`.
fn _extent(
    unsigned: bool,
    width: u32,
) -> (BigInt, BigInt) {
    if unsigned {
        (BigInt::from(0), (BigInt::from(1) << width) - 1)
    } else {
        (-(BigInt::from(1) << (width - 1)), (BigInt::from(1) << (width - 1)) - 1)
    }
}

/// Trips of an ordered test with constant ends, or None where a tested value
/// would wrap first.
fn _ordered_after(
    begin: &BigInt,
    limit: &BigInt,
    step: &BigInt,
    test: IntPredicate,
    width: u32,
    posttested: bool,
    stepped: bool,
) -> Option<BigInt> {
    let unsigned = _unsigned(test);
    let (low, high) = _extent(unsigned, width);
    let first =
        (if unsigned { begin.clone() } else { _as_signed(begin, width) }) + step * u8::from(posttested && stepped);
    let bound = if unsigned { limit.clone() } else { _as_signed(limit, width) };
    if !(low <= first && first <= high) {
        return None;
    }
    let inclusive = u8::from(_inclusive(test));
    let zero = BigInt::from(0);
    let tested = if step > &zero {
        let edge = &bound + inclusive;
        max(zero, -floor_div(&(&first - &edge), step))
    } else {
        let edge = &bound - inclusive;
        max(zero, -floor_div(&(&edge - &first), &-step))
    };
    let reached = &first + &tested * step;
    (low <= reached && reached <= high).then(|| BigInt::from(u8::from(posttested)) + tested)
}

/// The first and last signed header values over `count` trips, where none up to
/// the exit wraps.
fn _signed_span(
    start: &AffineOperand,
    facts: &IndexMap<ValueId, Known>,
    width: u32,
    count: &BigInt,
    step: &BigInt,
) -> (Option<BigInt>, Option<BigInt>) {
    let Some(begin) = _signed(start, facts, width) else { return (None, None) };
    if count < &BigInt::from(1) {
        return (None, None);
    }
    let last = &begin + (count - 1) * step;
    let sign = BigInt::from(1) << (width - 1);
    let after = &last + step;
    if -&sign <= last && last < sign && -&sign <= after && after < sign {
        (Some(begin), Some(last))
    } else {
        (None, None)
    }
}

/// The end of `bound`'s range a counter walking up (`ascending`) or down
/// meets, where the range is the same signed or not: `n & 3` bounds a walk
/// up from zero by three trips.
fn _extent_toward(
    unit: &Unit,
    bound: &AffineOperand,
    facts: &IndexMap<ValueId, Known>,
    ascending: bool,
) -> Option<BigInt> {
    let AffineOperand::Value(value, width) = bound else { return None };
    let range = match unit.function.value(*value).def {
        ValueDef::Instruction(inst) => {
            crate::ranges::_computed(unit, inst, &crate::ranges::Intervals::default(), facts)
        }
        _ => crate::ranges::_operand(unit, Operand::Value(*value), &crate::ranges::Intervals::default(), facts),
    }?;
    let half = BigInt::from(1) << (width - 1);
    (range.width == *width && range.low >= BigInt::from(0) && range.high < half)
        .then(|| if ascending { range.high } else { range.low })
}

/// Most trips of a symbolic unit-step loop, where proved; None for an inclusive
/// test that may never end.
#[allow(clippy::too_many_arguments)]
fn _unit_maximum(
    unit: &Unit,
    loop_: &Loop,
    width: u32,
    begin: Option<&BigInt>,
    limit: Option<&BigInt>,
    step: &BigInt,
    test: IntPredicate,
    inbounds: bool,
    promised: bool,
) -> Option<BigInt> {
    // Both bound the trips; keep the tighter.
    let bounded = if inbounds { inbounds_backedges(unit, loop_) } else { None };
    if test == IntPredicate::Ne {
        return Some((BigInt::from(1) << width) - 1).into_iter().chain(bounded).min();
    }
    let (unsigned, inclusive) = (_unsigned(test), _inclusive(test));
    let (low, high) = _extent(unsigned, width);
    let ascending = step > &BigInt::from(0);
    // Walked toward `bound`, as integers in the test's own signedness.
    let signed = |value: &BigInt| if unsigned { value.clone() } else { _as_signed(value, width) };
    let mut origin = begin.map(signed);
    let mut target = limit.map(signed);
    let end = if ascending { &high } else { &low };
    // Stepping past the width's end wraps back inside an inclusive bound,
    // unless the step promised it never wraps: then the program stops first.
    let endless = inclusive && !promised;
    if endless && target.as_ref() == Some(end) {
        return None;
    }
    if !endless && inclusive {
        // Promised: the counter runs between the width's ends whatever it is
        // given.
        target = target.or_else(|| Some(end.clone()));
        origin = origin.or_else(|| Some(if ascending { low.clone() } else { high.clone() }));
    }
    if !inclusive {
        // An exclusive bound lies inside the width: the counter stops by its
        // end.
        target = target.or_else(|| Some(end.clone()));
    }
    let ranged = match (&origin, &target) {
        (Some(origin), Some(target)) if low <= *origin.min(target) && *origin.max(target) <= high => {
            Some(max(BigInt::from(0), (target - origin) * step + u8::from(inclusive)))
        }
        _ => None,
    };
    ranged.into_iter().chain(bounded).min()
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// Loops `advances` was asked of: a pass that wants one value should not
    /// ask every loop of the body.
    pub static ADVANCES_ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How far each integer recurrence of the loop advances per iteration.
pub fn advances(
    unit: &Unit,
    loop_: &Loop,
) -> IndexMap<ValueId, BigInt> {
    #[cfg(any(test, feature = "testing"))]
    ADVANCES_ASKED.with(|asked| asked.set(asked.get() + 1));
    let found = basics(unit, loop_);
    recurrences(unit, loop_, &found)
        .values
        .into_iter()
        .filter(|(_, of)| of.pointer.is_none())
        .filter_map(|(value, of)| of.step.known().map(|step| (value, step)))
        .filter(|(_, step)| *step != BigInt::from(0))
        .collect()
}

/// The most backedges a loop takes that an access made every trip allows,
/// as LLVM's inbounds does, whatever ends the loop.
///
/// Iteration i reaches `b + i*s` inside one object, and an index `w` bits
/// wide addresses at most 2**w bytes of it, so i*s + width <= 2**w. Only
/// an access through `inbounds` GEPs is promised that.
pub fn inbounds_backedges(
    unit: &Unit,
    loop_: &Loop,
) -> Option<BigInt> {
    _by_index(unit, loop_, &advances(unit, loop_))
}

fn _by_index(
    unit: &Unit,
    loop_: &Loop,
    step: &IndexMap<ValueId, BigInt>,
) -> Option<BigInt> {
    accesses(unit, loop_)
        .filter_map(|(at, reference)| {
            let advance = step.get(&reference.base?)? * reference.scale;
            let room = (BigInt::from(1) << reference.index_bits) - reference.width;
            _allowed(loop_, at, room, advance)
        })
        .min()
}

/// `inbounds_backedges`, and where the object is known to be `n` bytes, so
/// is i*s + width <= n: GCC's `infer_loop_bounds_from_array`
/// ([tree-ssa-loop-niter.cc:4225](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-loop-niter.cc#L4225)),
/// which reads the extent off the array's type, here off the object. Every
/// GEP to the object counts, not the one `MemRef` keeps: `A[i][k]` is two
/// indexes, and `k` bounds the trips by the row.
pub fn object_backedges(
    unit: &Unit,
    loop_: &Loop,
) -> Option<BigInt> {
    let step = advances(unit, loop_);
    let still = invariant(unit.function, &loop_.body);
    let by_extent = accesses(unit, loop_)
        .filter_map(|(at, reference)| {
            let walked = memory::indexed(unit, reference.pointer?);
            if !walked.inbounds {
                return None;
            }
            let extent = memory::extent_of(unit, walked.root).filter(|&extent| extent >= i64::from(reference.width))?;
            let mut advance = BigInt::from(0);
            for (index, scale) in walked.indices {
                match step.get(&index) {
                    Some(per_trip) => advance += per_trip * scale,
                    None if still.contains(index) => {}
                    None => return None,
                }
            }
            _allowed(loop_, at, BigInt::from(extent - i64::from(reference.width)), advance)
        })
        .min();
    by_extent.into_iter().chain(_by_index(unit, loop_, &step)).min()
}

/// The inbounds accesses a loop makes every trip, each with its block.
fn accesses<'a>(
    unit: &'a Unit,
    loop_: &'a Loop,
) -> impl Iterator<Item = (i64, MemRef)> + 'a {
    let shape = unit.shape();
    let latch = loop_.latches.first().copied();
    loop_
        .body
        .iter()
        .filter(move |at| latch.is_some_and(|latch| shape.dominance.dominates(unit.function, **at, latch)))
        .flat_map(|&at| unit.function.iter_block(cfg::block(at)).map(move |inst| (at, inst)))
        .filter_map(|(at, inst)| Some((at, MemRef::of(unit, inst)?)))
        .filter(|(_, reference)| reference.inbounds)
}

/// The backedges an access allows whose address moves `advance` bytes a trip
/// within `room` bytes: it runs at least once a backedge, and the header once
/// more.
fn _allowed(
    loop_: &Loop,
    at: i64,
    room: BigInt,
    advance: BigInt,
) -> Option<BigInt> {
    (advance != BigInt::from(0)).then(|| room / abs(&advance) + 1 - BigInt::from(u8::from(at == loop_.header)))
}

/// `term`'s number at `width`, where known.
fn _constant(
    term_: &AffineOperand,
    facts: &IndexMap<ValueId, Known>,
    width: u32,
) -> Option<BigInt> {
    if term_.width() != width {
        return None;
    }
    match term_ {
        AffineOperand::Value(value, _) => {
            let fact = facts.get(value)?;
            (fact.width >= width).then(|| masked(&fact.n, width))
        }
        AffineOperand::Const(constant) => Some(masked(&constant.n, width)),
    }
}

fn _as_signed(
    value: &BigInt,
    width: u32,
) -> BigInt {
    let sign = BigInt::from(1) << (width - 1);
    (value ^ &sign) - sign
}

/// `term`'s signed number at `width`, where known.
pub fn _signed(
    term_: &AffineOperand,
    facts: &IndexMap<ValueId, Known>,
    width: u32,
) -> Option<BigInt> {
    _constant(term_, facts, width).map(|value| _as_signed(&value, width))
}

/// The one positive trip count every counter of a loop proves, if they prove
/// one.
///
/// A loop may carry several counters at once. They are evidence for the
/// same trip count, not alternatives a transform may pick from; refusing
/// disagreement keeps transforms independent of visiting order.
pub fn agreed_count(proofs: &[CountedLoop]) -> Option<BigInt> {
    let zero = BigInt::from(0);
    let counts =
        proofs.iter().filter_map(|proof| proof.count.clone()).filter(|count| *count != zero).collect::<BTreeSet<_>>();
    if counts.len() == 1 { counts.into_iter().next() } else { None }
}

/// Whether two loops make the same trips, tested at the same place: LLVM's
/// `haveIdenticalTripCounts`. Both count trips by constants that are equal, or
/// by the same test, step and `bound - start`, as symbolic values.
pub fn same_trips(
    first: &CountedLoop,
    second: &CountedLoop,
) -> bool {
    if (first.posttested, first.stepped, first.shifted) != (second.posttested, second.stepped, second.shifted) {
        return false;
    }
    match (&first.count, &second.count) {
        (Some(one), Some(other)) => one == other,
        (None, None) => {
            let distance = |proof: &CountedLoop| {
                let width = proof.width();
                Scev::of(&proof.bound, width).minus(&Scev::of(&proof.start, width))
            };
            first.test == second.test
                && first.step == second.step
                && first.reach == second.reach
                && first.width() == second.width()
                && distance(first) == distance(second)
        }
        _ => false,
    }
}

/// The loop's trips, where its counters agree on a positive count.
pub fn trip_count(
    unit: &Unit,
    loop_: &Loop,
    facts: &IndexMap<ValueId, Known>,
) -> Option<BigInt> {
    agreed_count(&counted(unit, loop_, Some(facts), false))
}

/// `trip_count` for a loop that may also leave by another exit: the trips as
/// long as it does not, so at most the trips it makes.
pub fn trip_bound(
    unit: &Unit,
    loop_: &Loop,
    facts: &IndexMap<ValueId, Known>,
) -> Option<BigInt> {
    agreed_count(&counted_leaving(unit, loop_, Some(facts), false))
}

/// `trip_count` for a loop that may also stop the program: its trips whenever
/// it does not.
pub fn trips_unless_stopped(
    unit: &Unit,
    loop_: &Loop,
    facts: &IndexMap<ValueId, Known>,
) -> Option<BigInt> {
    agreed_count(&counted_unless_stopped(unit, loop_, Some(facts), false))
}

/// A counted loop whose first iteration and finite exit are proven.
pub fn nonempty(
    unit: &Unit,
    loop_: &Loop,
) -> bool {
    trip_count(unit, loop_, &unit.registers()).is_some()
}

/// The proof in which `counter` decides when `loop` leaves.
pub fn controlling(
    unit: &Unit,
    loop_: &Loop,
    counter: &Affine,
    facts: &IndexMap<ValueId, Known>,
) -> Option<CountedLoop> {
    counted(unit, loop_, Some(facts), false).into_iter().find(|proof| proof.counter.value == counter.value)
}

fn abs(value: &BigInt) -> BigInt {
    if value < &BigInt::from(0) { -value } else { value.clone() }
}

/// Floor division for a nonzero divisor; `BigInt` truncates.
pub fn floor_div(
    numerator: &BigInt,
    denominator: &BigInt,
) -> BigInt {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder != BigInt::from(0) && ((remainder < BigInt::from(0)) != (denominator < &BigInt::from(0))) {
        quotient - 1
    } else {
        quotient
    }
}

/// The remainder with the positive modulus's sign.
pub fn mod_floor(
    value: &BigInt,
    modulus: &BigInt,
) -> BigInt {
    let remainder = value % modulus;
    if remainder < BigInt::from(0) { remainder + modulus } else { remainder }
}

/// `value`'s inverse modulo `modulus`, if it has one.
fn modular_inverse(
    value: &BigInt,
    modulus: &BigInt,
) -> Option<BigInt> {
    let (mut old_r, mut r) = (mod_floor(value, modulus), modulus.clone());
    let (mut old_s, mut s) = (BigInt::from(1), BigInt::from(0));
    while r != BigInt::from(0) {
        let quotient = &old_r / &r;
        let next_r = &old_r - &quotient * &r;
        old_r = std::mem::replace(&mut r, next_r);
        let next_s = &old_s - &quotient * &s;
        old_s = std::mem::replace(&mut s, next_s);
    }
    (old_r == BigInt::from(1)).then(|| mod_floor(&old_s, modulus))
}

pub fn gcd(
    mut one: BigInt,
    mut other: BigInt,
) -> BigInt {
    while other != BigInt::from(0) {
        let remainder = one % &other;
        one = std::mem::replace(&mut other, remainder);
    }
    one
}

/// The values a loop does not define: asked of the value's own definition, not
/// of a set made of every instruction of the loop (each proof made it several
/// times: a nest of loops each held the whole nest's values).
#[derive(Clone, Copy, Debug, Default)]
pub struct Invariant<'a> {
    over: Option<(&'a Function, &'a llrm_mir::forest::Body)>,
}

impl Invariant<'_> {
    pub fn contains(
        &self,
        value: ValueId,
    ) -> bool {
        let Some((function, inside)) = self.over else { return true };
        match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(inst) => {
                !function.parent(inst).is_some_and(|block| inside.contains(&cfg::id(block)))
            }
            llrm_mir::module::ValueDef::Argument(_) => true,
        }
    }

    /// A constant, or a value the loop does not define.
    pub fn operand(
        &self,
        operand: Operand,
    ) -> bool {
        match operand {
            Operand::Value(value) => self.contains(value),
            Operand::Constant(_) => true,
            Operand::Block(_) => false,
        }
    }
}

/// What the blocks `inside` do not define.
pub fn invariant<'a>(
    function: &'a Function,
    inside: &'a llrm_mir::forest::Body,
) -> Invariant<'a> {
    Invariant { over: Some((function, inside)) }
}

/// The header phis that one invariant or constant step advances on every
/// way round, from one start on every way in.
pub fn basics(
    unit: &Unit,
    loop_: &Loop,
) -> IndexMap<ValueId, Affine> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let mut out = IndexMap::default();
    if !function.is_placed(header) {
        return out;
    }
    let inside = &loop_.body;
    let still = invariant(function, inside);
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| inside.contains(&cfg::id(block)));
    // The value each counter takes round the loop, by counter; the phis that
    // are not `phi + step`, with their start and what they take round.
    let mut backs: Vec<(ValueId, ValueId)> = Vec::new();
    let mut derived: Vec<(ValueId, AffineOperand, ValueId)> = Vec::new();
    for inst in function.iter_block(header) {
        let phi = function.instruction(inst);
        if phi.opcode != Opcode::Phi {
            break;
        }
        let Some(result) = phi.result.filter(|&result| unit.int_bits(Operand::Value(result)).is_some()) else {
            continue;
        };
        let pairs = phi
            .operands
            .chunks(2)
            .filter_map(
                |pair| match pair[1] {
                    Operand::Block(from) => Some((pair[0], inside.contains(&cfg::id(from)))),
                    _ => None,
                },
            );
        let (around, into): (Vec<_>, Vec<_>) = pairs.partition(|(_, back)| *back);
        let Some(&(start, _)) = into.first() else { continue };
        if into.iter().any(|(one, _)| *one != start) {
            continue;
        }
        let steps = around
            .iter()
            .map(|(one, _)| match one {
                Operand::Value(value) => defining(function, *value)
                    .filter(|&made| within(made))
                    .and_then(|made| _stepped(unit, made, result, &still)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let (Some(Some(step)), Some(start)) = (steps.first(), term(unit, start)) else {
            // Not `phi + step`: the value round may be another counter's, and
            // a constant more.
            if let (&[(Operand::Value(back), _)], Some(start)) = (&around[..], term(unit, start)) {
                derived.push((result, start, back));
            }
            continue;
        };
        if steps.iter().all(|one| one.as_ref() == Some(step)) {
            if let [(Operand::Value(back), _)] = around[..] {
                backs.push((back, result));
            }
            out.insert(result, Affine { value: result, start, step: step.clone(), header: loop_.header });
        }
    }
    // A header phi that is a counter and a constant: round the loop it takes
    // that counter's next value and a constant more, from a start that is its
    // start and the constant. A header copy leaves one where the test read
    // `i + 2` (SCEV's add recurrence `{2,+,3}` beside `{0,+,3}`).
    for (result, start, back) in derived {
        let Some(made) = defining(function, back).filter(|&made| within(made)) else { continue };
        let Some((stepped, constant)) = stepping(unit, function.instruction(made)) else { continue };
        let (AffineOperand::Value(next, _), AffineOperand::Const(more)) = (&stepped, &constant) else { continue };
        let Some(counter) = backs.iter().find(|(back, _)| back == next).and_then(|(_, phi)| out.get(phi)).cloned()
        else {
            continue;
        };
        let (AffineOperand::Const(was), AffineOperand::Const(now)) = (&counter.start, &start) else { continue };
        if masked(&(&was.n + &more.n), now.width) != masked(&now.n, now.width) {
            continue;
        }
        out.insert(result, Affine { value: result, start, step: counter.step.clone(), header: loop_.header });
    }
    out
}

/// What a header phi proves that advances by at most one a trip where a
/// counter advances by one without wrapping, from the counter's own start:
/// `start <= follower <= counter`, signed, wherever the header dominates.
/// Where the follower advances is a branch, `i = i + 1` on some ways round
/// and `i` on the others (quicksort's partition index), the counter still
/// leads: `i + 1 <= j + 1`, which the counter's step does not wrap.
/// Each loop's followers, by header: `followers` of every loop of the unit's
/// shape, found once for the body (a block asks them of each loop around it).
pub type LoopFollowers = IndexMap<i64, Vec<(ValueId, ValueId, AffineOperand)>>;

pub fn followers_all(unit: &Unit) -> LoopFollowers {
    unit.shape().loops.iter().map(|loop_| (loop_.header, followers(unit, loop_))).collect()
}

/// The followers of `loop_`: the manager's where the unit carries them, else
/// worked out.
pub fn followers_of(
    unit: &Unit,
    loop_: &Loop,
) -> Vec<(ValueId, ValueId, AffineOperand)> {
    match unit.followers.and_then(|held| held.get(&loop_.header)) {
        Some(found) => found.clone(),
        None => followers(unit, loop_),
    }
}

pub fn followers(
    unit: &Unit,
    loop_: &Loop,
) -> Vec<(ValueId, ValueId, AffineOperand)> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return Vec::new() };
    let counters = basics(unit, loop_);
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    let mut found = Vec::new();
    // `value` is `follower` or `follower + 1`, or a phi of such values.
    fn advances(
        unit: &Unit,
        within: &dyn Fn(InstId) -> bool,
        follower: ValueId,
        value: Operand,
        depth: u32,
    ) -> bool {
        let function = unit.function;
        let Operand::Value(value) = value else { return false };
        if value == follower {
            return true;
        }
        let Some(inst) = defining(function, value).filter(|&inst| within(inst)) else { return false };
        let op = function.instruction(inst);
        match (&op.opcode, &op.operands[..]) {
            (Opcode::Binary(BinaryOp::Add), [one, other]) => {
                let one_step = |left: &Operand, right: &Operand| {
                    *left == Operand::Value(follower) && unit.int_constant(*right).is_some_and(|bits| bits == 1)
                };
                one_step(one, other) || one_step(other, one)
            }
            (Opcode::Phi, arms) if depth < 4 => {
                arms.chunks(2).all(|pair| advances(unit, within, follower, pair[0], depth + 1))
            }
            _ => false,
        }
    }
    for (&counter, affine) in &counters {
        let (AffineOperand::Const(step), Some(phi)) = (&affine.step, defining(function, counter)) else { continue };
        if step.n != BigInt::from(1) {
            continue;
        }
        // The counter's step does not wrap.
        let Some(Operand::Value(update)) = incoming(function, phi, cfg::block(latch)) else { continue };
        let Some(update) = defining(function, update) else { continue };
        if !llrm_mir::facts::Facts::of_flags(function.instruction(update).flags).no_signed_wrap() {
            continue;
        }
        for inst in function.iter_block(header) {
            let phi = function.instruction(inst);
            if phi.opcode != Opcode::Phi {
                break;
            }
            let Some(result) = phi.result.filter(|&result| {
                result != counter
                    && !counters.contains_key(&result)
                    && unit.int_bits(Operand::Value(result)) == Some(step.width)
            }) else {
                continue;
            };
            let (around, into): (Vec<_>, Vec<_>) = phi
                .operands
                .chunks(2)
                .partition(
                    |pair| matches!(
                        pair[1],
                        Operand::Block(from) if loop_.body.contains(&cfg::id(from))
                    ),
                );
            let starts_at_counter =
                !into.is_empty() && into.iter().all(|pair| term(unit, pair[0]).as_ref() == Some(&affine.start));
            if starts_at_counter
                && !around.is_empty()
                && around.iter().all(|pair| advances(unit, &within, result, pair[0], 0))
            {
                found.push((result, counter, affine.start.clone()));
            }
        }
    }
    found
}

/// A pointer phi at a loop's header stepped by constant bytes: a
/// `getelementptr` of it with constant indices, the same on every latch.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerRecurrence {
    pub value: ValueId,
    pub phi: InstId,
    pub start: Operand,
    pub step: BigInt,
    pub stepping: InstId,
}

/// The loop's pointer recurrences, which `basics` leaves out.
pub fn pointers(
    unit: &Unit,
    loop_: &Loop,
) -> Vec<PointerRecurrence> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let inside = &loop_.body;
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| inside.contains(&cfg::id(block)));
    let mut out = Vec::new();
    if !function.is_placed(header) {
        return out;
    }
    for inst in function.iter_block(header) {
        let phi = function.instruction(inst);
        if phi.opcode != Opcode::Phi {
            break;
        }
        let Some(result) = phi.result.filter(|&result| unit.space(Operand::Value(result)).is_some()) else { continue };
        let (mut start, mut stepping) = (None, None);
        let mut agreed = true;
        for pair in phi.operands.chunks(2) {
            let Operand::Block(from) = pair[1] else {
                agreed = false;
                break;
            };
            let slot = if inside.contains(&cfg::id(from)) { &mut stepping } else { &mut start };
            agreed &= slot.is_none_or(|one| one == pair[0]);
            *slot = Some(pair[0]);
        }
        let (true, Some(start), Some(Operand::Value(update))) = (agreed, start, stepping) else { continue };
        let Some(made) = defining(function, update).filter(|&made| within(made)) else { continue };
        let op = function.instruction(made);
        let Opcode::GetElementPtr { source } = op.opcode else { continue };
        if op.operands[0] != Operand::Value(result) {
            continue;
        }
        let indices = op.operands[1..]
            .iter()
            .map(|&one| unit.int_constant(one).map(|bits| signed(bits, unit.int_bits(one).unwrap_or(128))))
            .collect::<Vec<_>>();
        if indices.iter().any(Option::is_none) {
            continue;
        }
        let (step, variable) = unit.layout.collect_offset(&unit.context.types, source, &indices);
        if variable.is_empty() && step != 0 {
            out.push(PointerRecurrence { value: result, phi: inst, start, step: BigInt::from(step), stepping: made });
        }
    }
    out
}

/// `(stepped, step)` where `op` adds `step` to `stepped`: an `add`, or a `sub`
/// of a constant.
pub fn stepping(
    unit: &Unit,
    op: &Instruction,
) -> Option<(AffineOperand, AffineOperand)> {
    let Opcode::Binary(kind) = op.opcode else { return None };
    let (left, right) = (term(unit, op.operands[0])?, term(unit, op.operands[1])?);
    match (kind, right) {
        (BinaryOp::Add, right) => Some((left, right)),
        (BinaryOp::Sub, AffineOperand::Const(constant)) => {
            Some((left, AffineOperand::constant(-&constant.n, constant.width)))
        }
        _ => None,
    }
}

/// The invariant or constant step `inst` adds to `value`.
fn _stepped(
    unit: &Unit,
    inst: InstId,
    value: ValueId,
    still: &Invariant,
) -> Option<AffineOperand> {
    let (mut stepped, mut step) = stepping(unit, unit.function.instruction(inst))?;
    let is_value = |one: &AffineOperand| matches!(one, AffineOperand::Value(found, _) if *found == value);
    if !is_value(&stepped) {
        if !is_value(&step) {
            return None;
        }
        std::mem::swap(&mut stepped, &mut step);
    }
    match step {
        AffineOperand::Const(_) => Some(step),
        AffineOperand::Value(one, _) if still.contains(one) => Some(step),
        AffineOperand::Value(..) => None,
    }
}

/// Whether the counted loop's own recurrence is read by nothing but its
/// control, its step, `covered` and the exit's phis: then it may go.
///
/// A proof, not a transform: the caller names the instructions it will
/// replace.
pub fn control_replacement<'a>(
    unit: &Unit,
    loop_: &Loop,
    proof: &'a CountedLoop,
    covered: &BTreeSet<InstId>,
) -> Option<ControlReplacement<'a>> {
    let function = unit.function;
    if proof.posttested || proof.preheader.is_none() || proof.width() != proof.counter.start.width() {
        return None;
    }
    let (header, latch) = (cfg::block(loop_.header), cfg::block(proof.latch));
    let first = function.block(latch).first().map(|one| &function.instruction(one).opcode);
    if loop_.body.len() != 2
        || proof.entered != proof.latch
        || first == Some(&Opcode::Phi)
        || function.predecessors(latch) != [header]
    {
        return None;
    }
    let tests_only = function
        .iter_block(header)
        .all(
            |inst| inst == proof.compare
                || inst == proof.branch
                || matches!(
                    function.instruction(inst).opcode,
                    Opcode::Phi | Opcode::ICmp(_)
                ),
        );
    if !tests_only {
        return None;
    }
    let counter = proof.counter.value;
    let Some(Operand::Value(update)) = incoming(function, proof.phi, latch) else { return None };
    let stepping_inst = defining(function, update)?;
    let is_counter = |one: &AffineOperand| matches!(one, AffineOperand::Value(value, _) if *value == counter);
    if !stepping(unit, function.instruction(stepping_inst))
        .is_some_and(|(one, other)| is_counter(&one) || is_counter(&other))
    {
        return None;
    }
    let is_phi = |inst: InstId| function.instruction(inst).opcode == Opcode::Phi;
    let allowed = |inst: InstId| covered.contains(&inst) || inst == proof.compare || inst == stepping_inst;
    if function.users(counter).iter().any(|one| !is_phi(one.user) && !allowed(one.user))
        || function.users(update).iter().any(|one| !is_phi(one.user))
    {
        return None;
    }
    let exit = cfg::block(proof.exit);
    let expected = [Operand::Value(counter), Operand::Block(header)];
    let exits = phis(function)
        .filter(|(_, block, phi)| *block == exit && phi.operands == expected)
        .map(|(inst, _, _)| inst)
        .collect::<Vec<_>>();
    let reads = |phi: &Instruction| {
        phi.operands.iter().any(|one| *one == Operand::Value(counter) || *one == Operand::Value(update))
    };
    if phis(function).any(|(inst, _, phi)| inst != proof.phi && !exits.contains(&inst) && reads(phi)) {
        return None;
    }
    let result = function.instruction(proof.compare).result?;
    if function.users(result).iter().any(|one| one.user != proof.branch) {
        return None;
    }
    Some(ControlReplacement { counted: proof, stepping: stepping_inst, update, exits })
}

/// Prove that `candidate` can supply a counted loop's terminating test.
///
/// `covered` are the counter's reads the caller rebases; the counter itself
/// is a candidate when they are all its data reads.
pub fn zero_terminating_control<'a>(
    unit: &Unit,
    loop_: &Loop,
    proof: &'a CountedLoop,
    candidate: &Affine,
    covered: &BTreeSet<InstId>,
    facts: Option<&IndexMap<ValueId, Known>>,
) -> Option<ZeroTerminatingControl<'a>> {
    let replacement = control_replacement(unit, loop_, proof, covered)?;
    let maximum = proof.maximum.as_ref()?;
    let width = proof.counter.start.width();
    if candidate.start.width() != width || candidate.step.width() != width || maximum < &BigInt::from(0) {
        return None;
    }
    let computed;
    let facts = match facts {
        Some(facts) => facts,
        None => {
            computed = unit.registers();
            &*computed
        }
    };
    let step = _signed(&candidate.step, facts, width)?;
    if step == BigInt::from(0) {
        return None;
    }
    let modulus = BigInt::from(1) << width;
    let period = modulus.clone() / gcd(abs(&step), modulus);
    if maximum > &period {
        return None;
    }
    Some(ZeroTerminatingControl { replacement, candidate: candidate.clone(), step, maximum: maximum.clone(), period })
}

/// A product of unknowns, each as often as it is multiplied in: `a*b*b` is
/// `[a, b, b]`. Sorted, never empty (the empty product is `Scev`'s
/// constant), so equal products compare equal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Monomial(Vec<ValueId>);

impl Monomial {
    pub fn of(value: ValueId) -> Self {
        Self(vec![value])
    }

    /// The unknown, where the product is one alone.
    pub fn single(&self) -> Option<ValueId> {
        match self.0[..] {
            [value] => Some(value),
            _ => None,
        }
    }

    /// Each unknown once per factor, in order.
    pub fn values(&self) -> &[ValueId] {
        &self.0
    }

    fn times(
        &self,
        other: &Self,
    ) -> Self {
        let mut factors = self.0.iter().chain(&other.0).copied().collect::<Vec<_>>();
        factors.sort_unstable();
        Self(factors)
    }
}

/// The most terms and the highest degree a product keeps: distributing sums
/// grows exponentially, and an address of that size is not one to build.
const MOST_TERMS: usize = 16;
const MOST_DEGREE: usize = 4;

/// A polynomial in invariant unknowns, modulo `width` bits: SCEV's n-ary add
/// and mul of unknowns and constants, in one canonical form. Coefficients
/// are masked to the width and a zero one is dropped, so equal values are
/// equal forms whatever order built them. Truncation commutes with add and
/// mul; an extension does not, so an extended value is an unknown of its own.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Scev {
    pub constant: BigInt,
    pub terms: BTreeMap<Monomial, BigInt>,
    pub width: u32,
}

impl Scev {
    pub fn constant(
        n: impl Into<BigInt>,
        width: u32,
    ) -> Self {
        Self { constant: masked(&n.into(), width), terms: BTreeMap::new(), width }
    }

    /// A term: a constant, or a value once.
    pub fn of(
        term_: &AffineOperand,
        width: u32,
    ) -> Self {
        match term_ {
            AffineOperand::Const(known) => Self::constant(known.n.clone(), width),
            AffineOperand::Value(value, _) => Self::unknown(*value, width),
        }
    }

    /// The unknown `value`.
    pub fn unknown(
        value: ValueId,
        width: u32,
    ) -> Self {
        Self::monomial(Monomial::of(value), BigInt::from(1), width)
    }

    /// `factor * product`.
    pub fn monomial(
        product: Monomial,
        factor: BigInt,
        width: u32,
    ) -> Self {
        Self { constant: BigInt::from(0), terms: BTreeMap::from([(product, factor)]), width }.normal()
    }

    fn normal(mut self) -> Self {
        let width = self.width;
        self.constant = masked(&self.constant, width);
        self.terms = self
            .terms
            .into_iter()
            .map(|(product, factor)| (product, masked(&factor, width)))
            .filter(|(_, factor)| *factor != BigInt::from(0))
            .collect();
        self
    }

    pub fn plus(
        &self,
        other: &Self,
    ) -> Self {
        let mut sum = self.clone();
        sum.constant += &other.constant;
        for (product, factor) in &other.terms {
            *sum.terms.entry(product.clone()).or_insert_with(|| BigInt::from(0)) += factor;
        }
        sum.normal()
    }

    pub fn times(
        &self,
        by: &BigInt,
    ) -> Self {
        Self {
            constant: &self.constant * by,
            terms: self.terms.iter().map(|(product, factor)| (product.clone(), factor * by)).collect(),
            width: self.width,
        }
        .normal()
    }

    pub fn minus(
        &self,
        other: &Self,
    ) -> Self {
        self.plus(&other.times(&BigInt::from(-1)))
    }

    /// The same sum in `width` bits: its low bits where narrower.
    pub fn truncated(
        &self,
        width: u32,
    ) -> Self {
        Self { width, ..self.clone() }.normal()
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty() && self.constant == BigInt::from(0)
    }

    /// Its value, where it has no terms, as a signed number.
    pub fn known(&self) -> Option<BigInt> {
        self.terms.is_empty().then(|| _as_signed(&self.constant, self.width))
    }

    /// Every unknown it mentions.
    pub fn unknowns(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.terms.keys().flat_map(|product| product.values().iter().copied())
    }

    /// `self * other`, distributed; None past `MOST_TERMS` terms or
    /// `MOST_DEGREE` factors, where it is not worth building.
    pub fn product(
        &self,
        other: &Self,
    ) -> Option<Self> {
        let other = other.truncated(self.width);
        let mut out = Self { constant: &self.constant * &other.constant, terms: BTreeMap::new(), width: self.width };
        let mut add = |product: Option<Monomial>, factor: BigInt| match product {
            None => out.constant += factor,
            Some(product) => *out.terms.entry(product).or_insert_with(|| BigInt::from(0)) += factor,
        };
        for (product, factor) in &self.terms {
            add(Some(product.clone()), factor * &other.constant);
        }
        for (product, factor) in &other.terms {
            add(Some(product.clone()), factor * &self.constant);
        }
        for (one, factor) in &self.terms {
            for (two, by) in &other.terms {
                add(Some(one.times(two)), factor * by);
            }
        }
        let out = out.normal();
        (out.terms.len() <= MOST_TERMS && out.terms.keys().all(|product| product.values().len() <= MOST_DEGREE))
            .then_some(out)
    }

    /// The terms alone.
    pub fn symbolic(&self) -> Self {
        Self { constant: BigInt::from(0), ..self.clone() }
    }

    /// `k` with `self = k * by`, the smallest in magnitude, where there is one.
    pub fn over(
        &self,
        by: &Self,
    ) -> Option<BigInt> {
        if self.width != by.width || by.is_zero() {
            return None;
        }
        let modulus = BigInt::from(1) << self.width;
        let first = by
            .terms
            .iter()
            .next()
            .map_or(
                (&by.constant, &self.constant),
                |(product, factor)| (factor, self.terms.get(product).unwrap_or(&modulus)),
            );
        let (divisor, dividend) =
            (_as_signed(first.0, self.width), _as_signed(&masked(first.1, self.width), self.width));
        if &dividend % &divisor != BigInt::from(0) {
            return None;
        }
        let k = dividend / divisor;
        (by.times(&k) == *self).then_some(k)
    }
}

/// A value on each trip of `header`'s loop, `pointer + start + step * trip`,
/// in bytes where it is an address, modulo `start`'s width: SCEV's add
/// recurrence. Every counter, and every value affine in one, is one.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Recurrence {
    pub pointer: Option<Operand>,
    pub start: Scev,
    pub step: Scev,
}

impl Recurrence {
    pub fn width(&self) -> u32 {
        self.start.width
    }

    /// The same values in `width` bits.
    pub fn truncated(
        &self,
        width: u32,
    ) -> Self {
        Self { pointer: self.pointer, start: self.start.truncated(width), step: self.step.truncated(width) }
    }

    /// `{a,+,b} + {c,+,d} = {a+c,+,b+d}`: integers of one trip.
    fn plus(
        &self,
        other: &Self,
    ) -> Self {
        Self { pointer: None, start: self.start.plus(&other.start), step: self.step.plus(&other.step) }
    }

    fn negated(&self) -> Self {
        let minus = BigInt::from(-1);
        Self { pointer: None, start: self.start.times(&minus), step: self.step.times(&minus) }
    }

    /// `{a,+,b} + c = {a+c,+,b}`.
    fn offset(
        &self,
        by: &Scev,
    ) -> Self {
        Self { start: self.start.plus(by), ..self.clone() }
    }

    /// `{a,+,b} * m = {a*m,+,b*m}`, `m` invariant: a product of recurrences is
    /// no recurrence of one step.
    fn scaled(
        &self,
        by: &Scev,
    ) -> Option<Self> {
        Some(Self { pointer: None, start: self.start.product(by)?, step: self.step.product(by)? })
    }
}

/// A counter as a recurrence of the trip.
pub fn counter_recurrence(counter: &Affine) -> Recurrence {
    let width = counter.start.width();
    Recurrence { pointer: None, start: Scev::of(&counter.start, width), step: Scev::of(&counter.step, width) }
}

/// How a use reads a recurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseKind {
    /// The address of a load or a store.
    Address,
    /// Compared with an invariant.
    Compare,
    /// Anything else.
    Basic,
}

/// An operand that reads a recurrence, of an instruction that is not one.
#[derive(Clone, Debug, PartialEq)]
pub struct IvUse {
    pub user: InstId,
    pub index: usize,
    pub value: ValueId,
    pub of: Recurrence,
    pub kind: UseKind,
    /// The low bits of the value the user observes.
    pub demanded: u32,
}

/// The low bits of operand `index` that `op` observes: an `and` with a
/// mask of low ones, or a `trunc`, reads fewer than all.
pub fn demanded(
    unit: &Unit,
    op: &Instruction,
    index: usize,
) -> Option<u32> {
    let width = unit.int_bits(op.operands[index])?;
    let low = match op.opcode {
        Opcode::Binary(BinaryOp::And) => {
            let mask = unit.int_constant(op.operands[1 - index])?;
            let ones = mask.trailing_ones();
            (mask >> ones == 0).then_some(ones)?
        }
        Opcode::Cast(CastOp::Trunc) => unit.int_bits(Operand::Value(op.result?))?,
        _ => width,
    };
    Some(low.min(width))
}

/// A loop's recurrences and their uses: LLVM's IVUsers.
#[derive(Clone, Debug, Default)]
pub struct Users {
    /// The counters' phis and every instruction whose value is a recurrence.
    pub web: BTreeSet<InstId>,
    pub values: BTreeMap<ValueId, Recurrence>,
    /// The header phis among `values`: integer counters and pointers alike.
    pub counters: Vec<ValueId>,
    pub uses: Vec<IvUse>,
}

/// The bytes the constant and invariant indices of `op`, a
/// `getelementptr`, add to its pointer, in `width` bits; None where an
/// index is neither, or narrower than the pointer's.
fn _gep_offset(
    unit: &Unit,
    op: &Instruction,
    width: u32,
    still: &Invariant,
) -> Option<Scev> {
    let Opcode::GetElementPtr { source } = op.opcode else { return None };
    let indices = op.operands[1..]
        .iter()
        .map(|&one| unit.int_constant(one).map(|bits| signed(bits, unit.int_bits(one).unwrap_or(128))))
        .collect::<Vec<_>>();
    let (constant, variable) = unit.layout.collect_offset(&unit.context.types, source, &indices);
    let mut offset = Scev::constant(constant, width);
    for (position, scale) in variable {
        let index = op.operands[1 + position];
        let Operand::Value(value) = index else { return None };
        if !still.contains(value) || unit.int_bits(index)? < width {
            return None;
        }
        offset = offset.plus(&Scev::unknown(value, width).times(&BigInt::from(scale)));
    }
    Some(offset)
}

/// `pointer` as the invariant it offsets and the offset, through the
/// `getelementptr`s over it the loop does not compute: SCEV's pointer as a
/// base plus an add. Two addresses into one object share a base.
pub fn rooted(
    unit: &Unit,
    pointer: Operand,
    width: u32,
    still: &Invariant,
) -> (Operand, Scev) {
    let mut offset = Scev::constant(0, width);
    let mut at = pointer;
    while still.operand(at)
        && let Some((_, op)) = unit.defining(at)
        && let Some(part) = _gep_offset(unit, op, width, still)
    {
        offset = offset.plus(&part);
        at = op.operands[0];
    }
    (at, offset)
}

/// `of` with its pointer rooted.
fn _rooted(
    unit: &Unit,
    of: Recurrence,
    still: &Invariant,
) -> Recurrence {
    let Some(pointer) = of.pointer else { return of };
    let (root, offset) = rooted(unit, pointer, of.width(), still);
    Recurrence { pointer: Some(root), start: of.start.plus(&offset), step: of.step }
}

/// Every integer and address of `loop_` that is a recurrence of its trip:
/// its `counters`, its pointer walks, and what sums, differences, products
/// by an invariant, shifts, truncations, range-proved extensions and
/// addresses make of them, to a fixed point. SCEV's add recurrences, built
/// by one fold per opcode; `uses` is empty.
pub fn recurrences(
    unit: &Unit,
    loop_: &Loop,
    counters: &IndexMap<ValueId, Affine>,
) -> Users {
    _recurrences(unit, loop_, counters, false)
}

/// `recurrences`; where `priced`, only the forms `lsr` prices well: a
/// truncation and the negation of a recurrence stay reads of one.
fn _recurrences(
    unit: &Unit,
    loop_: &Loop,
    counters: &IndexMap<ValueId, Affine>,
    priced: bool,
) -> Users {
    let function = unit.function;
    let walk = Walk { unit, loop_, counters, priced, still: invariant(function, &loop_.body), facts: unit.registers() };
    let mut found = Users::default();
    for counter in counters.values() {
        let Some(phi) = defining(function, counter.value) else { continue };
        found.web.insert(phi);
        found.values.insert(counter.value, counter_recurrence(counter));
        found.counters.push(counter.value);
    }
    for one in pointers(unit, loop_) {
        let (Some(space), Some(stepped)) =
            (unit.space(Operand::Value(one.value)), function.instruction(one.stepping).result)
        else {
            continue;
        };
        let width = unit.layout.pointer(space).index_bits;
        let (root, start) = rooted(unit, one.start, width, &walk.still);
        let of = Recurrence { pointer: Some(root), start, step: Scev::constant(one.step.clone(), width) };
        found.web.extend([one.phi, one.stepping]);
        found.values.insert(stepped, Recurrence { start: of.start.plus(&of.step), ..of.clone() });
        found.values.insert(one.value, of);
        found.counters.push(one.value);
    }
    let mut changed = true;
    while changed {
        changed = false;
        for (inst, block, op) in operations_in(function, &loop_.body) {
            let Some(result) = op.result else { continue };
            if found.web.contains(&inst) || found.values.contains_key(&result) {
                continue;
            }
            if let Some(of) = walk.fold(&found, cfg::id(block), op) {
                found.web.insert(inst);
                found.values.insert(result, of);
                changed = true;
            }
        }
    }
    found
}

struct Walk<'a> {
    unit: &'a Unit<'a>,
    loop_: &'a Loop,
    counters: &'a IndexMap<ValueId, Affine>,
    priced: bool,
    still: Invariant<'a>,
    facts: std::borrow::Cow<'a, IndexMap<ValueId, Known>>,
}

impl Walk<'_> {
    /// `operand` as an integer recurrence of `width` bits.
    fn rec<'f>(
        &self,
        found: &'f Users,
        operand: Operand,
        width: u32,
    ) -> Option<&'f Recurrence> {
        let Operand::Value(value) = operand else { return None };
        found.values.get(&value).filter(|of| of.pointer.is_none() && of.width() == width)
    }

    /// `operand` as an invariant: a number, or an unknown the loop does not
    /// define.
    fn invariant(
        &self,
        operand: Operand,
        width: u32,
    ) -> Option<Scev> {
        if self.unit.int_bits(operand)? != width {
            return None;
        }
        match operand {
            Operand::Value(value) if self.still.contains(value) => {
                Some(match self.facts.get(&value).filter(|fact| fact.width >= width) {
                    Some(fact) => Scev::constant(fact.n.clone(), width),
                    None => Scev::unknown(value, width),
                })
            }
            Operand::Value(_) => None,
            _ => Some(Scev::constant(self.unit.int_constant(operand)?, width)),
        }
    }

    fn fold(
        &self,
        found: &Users,
        at: i64,
        op: &Instruction,
    ) -> Option<Recurrence> {
        let unit = self.unit;
        let result = op.result?;
        match op.opcode {
            Opcode::GetElementPtr { source } => self.address(found, op, source),
            Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)) => {
                let width = unit.int_bits(Operand::Value(result))?;
                let (first, second) = (self.rec(found, op.operands[0], width), self.rec(found, op.operands[1], width));
                let (left, right) = (self.invariant(op.operands[0], width), self.invariant(op.operands[1], width));
                match (kind, first, second, left, right) {
                    (BinaryOp::Add, Some(x), Some(y), ..) => Some(x.plus(y)),
                    (BinaryOp::Sub, Some(x), Some(y), ..) => Some(x.plus(&y.negated())),
                    (BinaryOp::Add, Some(x), None, _, Some(c)) | (BinaryOp::Add, None, Some(x), Some(c), _) => {
                        Some(x.offset(&c))
                    }
                    (BinaryOp::Sub, Some(x), None, _, Some(c)) => Some(x.offset(&c.times(&BigInt::from(-1)))),
                    (BinaryOp::Sub, None, Some(y), Some(c), _) if !self.priced => Some(y.negated().offset(&c)),
                    (BinaryOp::Mul, Some(x), None, _, Some(c)) | (BinaryOp::Mul, None, Some(x), Some(c), _) => {
                        x.scaled(&c)
                    }
                    (BinaryOp::Shl, Some(x), _, _, Some(count)) => {
                        let count =
                            count.known().filter(|count| *count >= BigInt::from(0) && *count < BigInt::from(width))?;
                        x.scaled(&Scev::constant(BigInt::from(1) << usize::try_from(count).ok()?, width))
                    }
                    _ => None,
                }
            }
            // Truncation commutes with add and mul: the low bits of a
            // recurrence are one.
            Opcode::Cast(CastOp::Trunc) if !self.priced => {
                let width = unit.int_bits(Operand::Value(result))?;
                let from = unit.int_bits(op.operands[0])?;
                self.rec(found, op.operands[0], from).filter(|_| from > width).map(|of| of.truncated(width))
            }
            // The header runs once more than the body, on the trip that
            // leaves: `extended` proves that one too.
            Opcode::Cast(cast @ (CastOp::SExt | CastOp::ZExt)) => {
                self.extended(found, op, cast, at == self.loop_.header)
            }
            Opcode::Binary(BinaryOp::SDiv) if at != self.loop_.header => self.quotient(op),
            _ => None,
        }
    }

    /// A narrow recurrence extended, as a wide one: only where the counted-loop
    /// proof shows the narrow value cannot wrap on any trip.
    fn extended(
        &self,
        found: &Users,
        op: &Instruction,
        cast: CastOp,
        in_header: bool,
    ) -> Option<Recurrence> {
        let unit = self.unit;
        let wide = unit.int_bits(Operand::Value(op.result?))?;
        let width = unit.int_bits(op.operands[0])?;
        let of = self.rec(found, op.operands[0], width)?;
        if width >= wide || !of.step.terms.is_empty() {
            return None;
        }
        if !of.start.terms.is_empty() {
            return (cast == CastOp::ZExt).then(|| self.extended_symbolic(found, of, width, wide, in_header)).flatten();
        }
        let (raw_start, raw_step) = (of.start.constant.clone(), of.step.constant.clone());
        let Some(count) = trip_count(unit, self.loop_, &self.facts).filter(|count| *count != BigInt::from(0)) else {
            return self
                .ascending(&raw_start, &raw_step, width, wide)
                .filter(|_| cast == CastOp::ZExt)
                .map(|(start, step)| Recurrence { pointer: None, start, step });
        };
        let step = _as_signed(&raw_step, width);
        if step == BigInt::from(0) {
            return None;
        }
        let mask = (BigInt::from(1) << width) - 1;
        let sign = BigInt::from(1) << (width - 1);
        let (initial, stride, low, high) = if cast == CastOp::SExt {
            (_as_signed(&raw_start, width), step, -sign.clone(), sign.clone())
        } else {
            // Half the modulus has two equally valid directions: choosing
            // either would invent a wide recurrence.
            if raw_step == sign && count > BigInt::from(1) {
                return None;
            }
            (raw_start, _as_signed(&raw_step, width), BigInt::from(0), mask + 1)
        };
        let last = if in_header { count } else { &count - 1 };
        let final_value = &initial + last * &stride;
        if initial < low || initial >= high || final_value < low || final_value >= high {
            return None;
        }
        Some(Recurrence { pointer: None, start: Scev::constant(initial, wide), step: Scev::constant(stride, wide) })
    }

    /// A narrow recurrence whose start is made of invariants, zero-extended:
    /// each invariant is itself a recurrence of the enclosing loop that runs
    /// from a constant start by a constant step and never leaves `[0, 2^w)`
    /// there (`outer_span`), so the start lies in a known unsigned interval and
    /// the walk from it, over this loop's trips, stays inside the width too:
    /// nothing wraps, and the extension is the wide recurrence whose start is
    /// the invariants' own zero-extended values (LLVM's `getZeroExtendExpr` of
    /// an add recurrence proved `nuw`). The invariants stay narrow unknowns
    /// of the wide `Scev`, which the expander reads zero-extended.
    fn extended_symbolic(
        &self,
        found: &Users,
        of: &Recurrence,
        width: u32,
        wide: u32,
        in_header: bool,
    ) -> Option<Recurrence> {
        if !of.step.terms.is_empty() {
            return None;
        }
        let count = trip_count(self.unit, self.loop_, &self.facts).filter(|count| *count != BigInt::from(0))?;
        let step = _as_signed(&of.step.constant, width);
        let mut low = _as_signed(&of.start.constant, width);
        let mut high = low.clone();
        let mut start = Scev::constant(low.clone(), wide);
        for (product, factor) in &of.start.terms {
            let [value] = product.values() else { return None };
            let (least, most) = self.outer_span(*value, width)?;
            let factor = _as_signed(factor, width);
            let (a, b) = (&factor * least, &factor * most);
            low += a.clone().min(b.clone());
            high += a.max(b);
            start = start.plus(&Scev::monomial(Monomial::of(*value), factor, wide));
        }
        let last = if in_header { count.clone() } else { &count - 1 };
        let reach = &step * last;
        // The values the walk takes run from `low` to `high` plus the reach.
        let (lowest, highest) = if step >= BigInt::from(0) { (low, high + reach) } else { (low + reach, high) };
        // An `nsw` operation of the narrow web that this range takes past the
        // signed maximum is poison there: the walk the proof describes never
        // happens, so nothing is said of it.
        let poisoned = self.nsw_overflows(found, width, &count, Some((of, &lowest, &highest)));
        (lowest >= BigInt::from(0) && highest < BigInt::from(1) << width && !poisoned).then(|| Recurrence {
            pointer: None,
            start,
            step: Scev::constant(step, wide),
        })
    }

    /// Whether an `nsw` instruction of `found`'s web, computing `width` bits,
    /// takes a value outside the signed range over `count` trips: poison. A
    /// recurrence with a constant start is evaluated; one with the same
    /// invariants as `anchor` (a recurrence and the least and greatest value
    /// it takes) is the anchor's span moved by the constants' difference.
    fn nsw_overflows(
        &self,
        found: &Users,
        width: u32,
        count: &BigInt,
        anchor: Option<(&Recurrence, &BigInt, &BigInt)>,
    ) -> bool {
        let top = BigInt::from(1) << (width - 1);
        found.web.iter().any(|&inst| {
            let one = self.unit.function.instruction(inst);
            let Some(result) = one.result else { return false };
            if !one.flags.contains(llrm_mir::opcode::Flags::NSW)
                || self.unit.int_bits(Operand::Value(result)) != Some(width)
            {
                return false;
            }
            let Some(of) = found.values.get(&result).filter(|of| of.pointer.is_none() && of.step.terms.is_empty())
            else {
                return false;
            };
            let step = _as_signed(&of.step.constant, width);
            let start = _as_signed(&of.start.constant, width);
            let (low, high) = match anchor {
                Some((base, lowest, highest)) if !of.start.terms.is_empty() && of.start.terms == base.start.terms => {
                    let moved = &start - _as_signed(&base.start.constant, width);
                    (lowest + &moved, highest + moved)
                }
                _ if of.start.terms.is_empty() => {
                    let end = &start + &step * count;
                    (start.clone().min(end.clone()), start.max(end))
                }
                _ => return false,
            };
            high >= top || low < -top.clone()
        })
    }

    /// The unsigned interval of `value`, a narrow value the enclosing loop
    /// computes as a recurrence from a constant start by a constant step: its
    /// values over that loop's trips, where none leaves `[0, 2^w)`.
    fn outer_span(
        &self,
        value: ValueId,
        width: u32,
    ) -> Option<(BigInt, BigInt)> {
        let unit = self.unit;
        let ValueDef::Instruction(inst) = unit.function.value(value).def else { return None };
        let at = cfg::id(unit.function.parent(inst)?);
        let shape = unit.shape();
        let outer = shape
            .loops
            .iter()
            .filter(|one| {
                one.header != self.loop_.header && one.body.contains(&self.loop_.header) && one.body.contains(&at)
            })
            .min_by_key(|one| one.body.len())?;
        let counters = basics(unit, outer);
        let users = recurrences(unit, outer, &counters);
        let of = users.values.get(&value)?;
        if of.pointer.is_some() || of.width() != width || !of.start.terms.is_empty() || !of.step.terms.is_empty() {
            return None;
        }
        let count = trip_count(unit, outer, &self.facts).filter(|count| *count != BigInt::from(0))?;
        let step = _as_signed(&of.step.constant, width);
        let start = of.start.constant.clone();
        let last = if at == outer.header { count.clone() } else { &count - 1 };
        let end = &start + &step * last;
        let (least, most) = (start.clone().min(end.clone()), start.max(end));
        // An `nsw` value or step past the signed maximum is poison.
        let poisoned = self.nsw_overflows(&users, width, &count, None);
        (least >= BigInt::from(0) && most < (BigInt::from(1) << width) && !poisoned).then_some((least, most))
    }

    /// A narrow unit-step counter that a symbolic bound ends with `ult`: its
    /// header values run from the start to the bound, or stay at the start, so
    /// none passes the width's largest and its zero extension is the wide
    /// counter.
    fn ascending(
        &self,
        start: &BigInt,
        step: &BigInt,
        width: u32,
        wide: u32,
    ) -> Option<(Scev, Scev)> {
        let ended = counted(self.unit, self.loop_, Some(&self.facts), false)
            .into_iter()
            .any(
                |proof| proof.rises_unsigned()
                    && proof.width() == width
                    && proof.start == AffineOperand::constant(start.clone(), width),
            );
        (ended && *step == BigInt::from(1)).then(|| (Scev::constant(start.clone(), wide), Scev::constant(1, wide)))
    }

    /// Exact signed division of a non-wrapping counter is another recurrence.
    fn quotient(
        &self,
        op: &Instruction,
    ) -> Option<Recurrence> {
        let unit = self.unit;
        let Operand::Value(dividend) = op.operands[0] else { return None };
        let counter = self.counters.get(&dividend)?;
        let width = counter.start.width();
        let facts = &self.facts;
        let (start, step, denominator) = (
            _signed(&counter.start, facts, width)?,
            _signed(&counter.step, facts, width)?,
            term(unit, op.operands[1]).and_then(|one| _signed(&one, facts, width))?,
        );
        if denominator == BigInt::from(0)
            || mod_floor(&start, &denominator) != BigInt::from(0)
            || mod_floor(&step, &denominator) != BigInt::from(0)
        {
            return None;
        }
        let (low, high) = domain(unit, self.loop_, counter, facts)?;
        let sign = BigInt::from(1) << (width - 1);
        if ![low, high].iter().all(|value| {
            let quotient = floor_div(value, &denominator);
            -&sign <= quotient && quotient < sign
        }) {
            return None;
        }
        Some(Recurrence {
            pointer: None,
            start: Scev::constant(floor_div(&start, &denominator), width),
            step: Scev::constant(floor_div(&step, &denominator), width),
        })
    }

    /// A `getelementptr` off an invariant pointer or a pointer recurrence,
    /// whose variable indices are recurrences or invariants: bytes as one
    /// recurrence. An index wider than the pointer's is truncated to it, so
    /// the recurrence is in the index's width.
    fn address(
        &self,
        found: &Users,
        op: &Instruction,
        source: TypeId,
    ) -> Option<Recurrence> {
        let unit = self.unit;
        let known = |one: Operand| match one {
            Operand::Value(value) => {
                self.facts.get(&value).filter(|fact| Some(fact.width) == unit.int_bits(one)).map(|fact| fact.n.clone())
            }
            _ => unit.int_constant(one).map(BigInt::from),
        };
        let indices = op.operands[1..]
            .iter()
            .map(|&one| {
                known(one)
                    .and_then(|bits| u128::try_from(bits).ok())
                    .map(|bits| signed(bits, unit.int_bits(one).unwrap_or(128)))
            })
            .collect::<Vec<_>>();
        let (constant, variable) = unit.layout.collect_offset(&unit.context.types, source, &indices);
        let mut width = unit.layout.pointer(unit.space(op.operands[0])?).index_bits;
        let mut wider = None::<u32>;
        for &(at, _) in &variable {
            let bits = unit.int_bits(op.operands[1 + at])?;
            if bits < width || wider.is_some_and(|wider| wider != bits) {
                return None;
            }
            wider = Some(bits);
        }
        width = wider.unwrap_or(width);
        let mut carried = false;
        let mut of = match op.operands[0] {
            base if self.still.operand(base) => {
                let (root, start) = rooted(unit, base, width, &self.still);
                Recurrence { pointer: Some(root), start, step: Scev::constant(0, width) }
            }
            Operand::Value(base) => {
                carried = true;
                found.values.get(&base).filter(|of| of.pointer.is_some() && of.width() == width)?.clone()
            }
            _ => return None,
        };
        for (at, scale) in variable {
            let (index, scale) = (op.operands[1 + at], BigInt::from(scale));
            let part = match self.rec(found, index, width) {
                Some(index) => {
                    carried = true;
                    index.scaled(&Scev::constant(scale, width))?
                }
                None => Recurrence {
                    pointer: None,
                    start: self.invariant(index, width)?.times(&scale),
                    step: Scev::constant(0, width),
                },
            };
            of = Recurrence { pointer: of.pointer, start: of.start.plus(&part.start), step: of.step.plus(&part.step) };
        }
        carried.then(|| of.offset(&Scev::constant(constant, width)))
    }
}

/// A header phi that some paths of the loop step and others leave, by
/// constants: its latch value is a join (phis) of adds of a constant, back to
/// the phi. `if (c) ++i;` is the common one. It is no `Affine` (its value is no
/// function of the trip alone), and what it is: on every path, the same phi
/// plus a constant.
#[derive(Clone, Debug)]
pub struct StepCycle {
    /// The header phi's value, then every phi and add the cycle holds.
    pub nodes: Vec<ValueId>,
    /// An add of a constant: its result, what it steps, and the constant.
    pub adds: Vec<(ValueId, ValueId, BigInt)>,
    pub phis: Vec<ValueId>,
    /// What the phi starts from, and its value from the latch.
    pub start: Operand,
    pub latch: Operand,
}

/// The `StepCycle` of `loop_`'s header phi `phi`, where it has one: some phi
/// joins its steps, and every value of the cycle is made in the loop.
pub fn step_cycle(
    unit: &Unit,
    loop_: &Loop,
    phi: InstId,
) -> Option<StepCycle> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let inside = &loop_.body;
    let f = function.instruction(phi).result?;
    let bits = unit.int_bits(Operand::Value(f))?;
    let [latch_at] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let (mut start, mut latch) = (None, None);
    for pair in function.instruction(phi).operands.chunks(2) {
        let Operand::Block(from) = pair[1] else { return None };
        if from == header || cfg::id(from) == latch_at {
            latch = Some(pair[0]);
        } else if !inside.contains(&cfg::id(from)) {
            start = Some(pair[0]);
        }
    }
    let (start, latch) = (start?, latch?);
    let step_of = |operand: Operand| unit.int_constant(operand).map(|value| signed(value, bits));
    let mut cycle = StepCycle { nodes: vec![f], adds: Vec::new(), phis: Vec::new(), start, latch };
    let mut pending = vec![latch];
    while let Some(operand) = pending.pop() {
        let Operand::Value(x) = operand else { return None };
        if cycle.nodes.contains(&x) {
            continue;
        }
        let ValueDef::Instruction(inst) = function.value(x).def else { return None };
        if !function.parent(inst).is_some_and(|block| inside.contains(&cfg::id(block))) {
            return None;
        }
        let op = function.instruction(inst);
        match op.opcode {
            Opcode::Phi => {
                cycle.nodes.push(x);
                cycle.phis.push(x);
                pending.extend(op.operands.chunks(2).map(|pair| pair[0]));
            }
            Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub)) => {
                let (left, right) = (op.operands[0], op.operands[1]);
                let (stepped, step) = match (step_of(left), step_of(right)) {
                    (None, Some(c)) => (left, if kind == BinaryOp::Add { c } else { -c }),
                    (Some(c), None) if kind == BinaryOp::Add => (right, c),
                    _ => return None,
                };
                let Operand::Value(on) = stepped else { return None };
                cycle.nodes.push(x);
                cycle.adds.push((x, on, BigInt::from(step)));
                pending.push(stepped);
            }
            _ => return None,
        }
    }
    (!cycle.phis.is_empty()).then_some(cycle)
}

/// The recurrences of `loop_`'s `counters`, and every read of one by
/// something else, in or after the loop. A truncation is a read, not a
/// recurrence (a candidate has one width, and a narrower use is priced from
/// the wider value), and so is `c - r`: `lsr` realizes each site of it
/// alone, a negation apiece where the loop computed one and shared it.
pub fn users(
    unit: &Unit,
    loop_: &Loop,
    counters: &IndexMap<ValueId, Affine>,
) -> Users {
    let function = unit.function;
    let still = invariant(function, &loop_.body);
    let mut found = _recurrences(unit, loop_, counters, true);
    for (value, of) in &found.values {
        for one in function.users(*value) {
            if found.web.contains(&one.user) {
                continue;
            }
            let op = function.instruction(one.user);
            let index = one.index as usize;
            let kind = match &op.opcode {
                Opcode::Load { .. } if index == 0 => UseKind::Address,
                Opcode::Store { .. } if index == 1 => UseKind::Address,
                Opcode::ICmp(_) if still.operand(op.operands[1 - index]) => UseKind::Compare,
                _ => UseKind::Basic,
            };
            let demanded = demanded(unit, op, index).unwrap_or(of.width());
            found.uses.push(IvUse { user: one.user, index, value: *value, of: of.clone(), kind, demanded });
        }
    }
    found.uses.sort_by_key(|one| (one.user, one.index));
    found
}

impl CountedLoop {
    /// The most trips the loop makes: its count, its `maximum`, or every
    /// value of a unit step's width.
    pub fn most(&self) -> Option<BigInt> {
        self.count
            .clone()
            .or_else(|| self.maximum.clone())
            .or_else(|| (abs(&self.step) == BigInt::from(1)).then(|| (BigInt::from(1) << self.width()) - 1))
    }

    /// Its trips on the entered path, as a sum of invariants: where `trips`
    /// places them.
    pub fn trips_linear(&self) -> Option<Scev> {
        let width = self.width();
        if let Reach::Solved { shift, inverse, .. } = &self.reach {
            // A polynomial where the solution is the distance, or its negation.
            let distance = Scev::of(&self.bound, width).minus(&Scev::of(&self.start, width));
            let lead = Scev::constant(&self.step * u8::from(self.posttested && self.stepped), width);
            let remaining = distance.minus(&lead);
            let solved = match (*shift, inverse) {
                (0, one) if *one == BigInt::from(1) => remaining,
                (0, minus) if *minus == (BigInt::from(1) << width) - 1 => remaining.times(&BigInt::from(-1)),
                _ => return None,
            };
            return Some(if self.posttested { solved.plus(&Scev::constant(1, width)) } else { solved });
        }
        if matches!(self.reach, Reach::Ceil { .. }) && self.count.is_none() {
            return None;
        }
        if self.posttested && !self.entry_guarded {
            return self.count.as_ref().map(|count| Scev::constant(count.clone(), width));
        }
        if let Some(count) = &self.count {
            return (count < &(BigInt::from(1) << width)).then(|| Scev::constant(count.clone(), width));
        }
        let (bound, start) = (Scev::of(&self.bound, width), Scev::of(&self.start, width));
        let (ahead, behind) = if self.step > BigInt::from(0) { (bound, start) } else { (start, bound) };
        Some(ahead.minus(&behind).plus(&Scev::constant(u8::from(self.inclusive()), width)))
    }
}

#[cfg(test)]
#[path = "induction_tests.rs"]
pub(crate) mod tests;
