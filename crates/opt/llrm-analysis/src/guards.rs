//! What the branches that dominate a block prove on entry to it.
//!
//! # How it works
//! `guards(unit, at)` climbs the dominator chain from block `at`, at most 64
//! blocks (`MAX_DOMINATORS`), and at each dominator `above` collects two
//! things. The first is the conditions the block assumes (`assume` calls, from
//! [`crate::assumptions`]), which hold below it wherever its terminator goes.
//! The second is the compare of `above`'s conditional branch, when the edge
//! toward `at` alone enters the block it leads to: that block dominates `at`
//! and every other predecessor of it is dominated by it (it is a back edge), so
//! the branch outcome holds on every path to `at`. A compare is read as `Guard
//! { predicate, left, right }` over [`Scev`]s. `_proven` also opens an `and`
//! that is true or an `or` that is false into both parts, and flips the
//! predicate when the edge is the false one.
//!
//! ```text
//!    b0: br (n > 2), b1, b9          guards(b2) = { n > 2, n != 0 }
//!     |
//!    b1: br (n != 0), b2, b9         b1 and b2 each have one predecessor, so
//!     |                              each branch holds on every path to b2
//!    b2:  <- here
//! ```
//!
//! `on_edge(unit, from, to)` is the guards of `from` plus what its own branch
//! says of the edge to `to`. `decides(unit, facts, condition)` answers a
//! compare with true or false when a guard implies it or its inverse.
//!
//! `implies(guard, predicate, left, right)` is direct implication: the same two
//! sides (either way round) under a predicate at least as strong (`_stronger`:
//! `eq` over `ule`, `ult` over `ule` and `ne`, ...); `a != 0` proves `0 <u a`;
//! equality and disequality hold at any offset, as a wrapping sum is a
//! bijection. `implies_at_offset` does the signed case where the sides differ
//! only in constants (`n > 2` proves `1 < n - 1`), exact only where the caller
//! knows nothing wraps.
//!
//! `holds(unit, at, predicate, left, right)` is true when both sides are
//! constants and the compare is true (`evaluated`), when a guard implies it,
//! when it is an unsigned compare of zero extensions that a guard proves in the
//! narrow type (`narrowed`), or when it is a compare of a phi against a value
//! fixed over the loop, where each incoming edge proves it of the value it
//! brings (`through_phi`: a loop entered from a guard and from its own latch
//! test). `holds_given` and `Given` add the `assumed` guards a caller supplies,
//! the guards on each loop's followers of its counters (`start <= follower <=
//! counter`) and the program's stated ranges, and ask
//! [`crate::difference::proves`]; `Given` finds the guards of a block once for
//! as many questions as are asked of it.
//!
//! # Example
//! The edge into `loop` is the only way there, so its branch holds in `loop`
//! but not in `done`:
//!
//! ```mir
//! define i16 @f(i16 %n, i16 %len) {
//! entry:
//!   %fits = icmp ult i16 %n, %len
//!   br i1 %fits, label %loop, label %done
//! loop:
//!   br label %done
//! done:
//!   ret i16 0
//! }
//! ```
//!
//! | Query | Result |
//! | --- | --- |
//! | `holds` in `loop`: `n <u len`, `n <=u len`, `len >u n` | true |
//! | `holds` in `loop`: `len <u n` | false |
//! | `guards(done)` | empty (the other edge reaches `done`) |
//!
//! From unit test `test_a_branch_proves_its_compare_where_its_edge_alone_leads`
//! in `guards_tests.rs`.
//!
//! # Provides
//! Guard facts and the implication queries above. Consumers: `decide`,
//! `exitfold` and `jumpthreading` in `llrm-transforms`, and
//! [`crate::induction`] (loop entry guards).
//!
//! # Needs
//! [`crate::cfg::Shape`], [`crate::assumptions`], [`crate::induction`]'s `Scev`
//! (`term`, `followers_of`), [`crate::ranges::declared`] and
//! [`crate::difference`].
//!
//! # See also
//! LLVM `isBasicBlockEntryGuardedByCond`, as far as direct implication goes;
//! `ScalarEvolution::collectFromBlock`, which climbs the unique-predecessor
//! chain only
//! ([`ScalarEvolution.cpp:16199`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L16199)).

use std::collections::BTreeMap;

use llrm_mir::module::{Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};

use crate::cfg;
use crate::difference;
use crate::induction::{Scev, term};
use crate::memory::Unit;
use crate::num::Num as BigInt;
use crate::ranges::declared;

/// `left predicate right`, proven on some edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard {
    pub predicate: IntPredicate,
    pub left: Scev,
    pub right: Scev,
}

#[cfg(test)]
thread_local! {
    static FOUND: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has found the guards of a block, for a test that
/// a block asked of again is not found again.
#[cfg(test)]
pub(crate) fn found() -> usize {
    FOUND.with(std::cell::Cell::get)
}

/// How many dominators above a block its guards are read from. LLVM climbs
/// the unique-predecessor chain only (`ScalarEvolution::collectFromBlock`,
/// [`ScalarEvolution.cpp:16199`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L16199)),
/// which a join ends, and bounds its recursive collection
/// (`scalar-evolution-max-loop-guard-collection-depth`); here the whole
/// dominator chain was climbed for each branch asked about, the square of a
/// function of branches. Held to the same answers on the programs, QCport and
/// the bench at -m16 and -m32 down to 8.
const MAX_DOMINATORS: usize = 64;

/// The compares proven on entry to block `at`.
pub fn guards(
    unit: &Unit,
    at: i64,
) -> Vec<Guard> {
    #[cfg(test)]
    FOUND.with(|found| found.set(found.get() + 1));
    let function = unit.function;
    let shape = unit.shape();
    let assumed = unit.assumptions();
    let mut found = Vec::new();
    let mut reached = at;
    let mut climbed = 0;
    while let Some(above) = shape.dominance.immediate(unit.function, reached) {
        climbed += 1;
        if climbed > MAX_DOMINATORS {
            break;
        }
        // What the block assumes holds below it, wherever its terminator goes.
        for condition in assumed.here(above) {
            if let Operand::Value(condition) = *condition {
                _proven(unit, condition, true, &mut found);
            }
        }
        // The edge from `above` toward `at` that alone enters the block it
        // leads to.
        if let Some(branch) = function.terminator(cfg::block(above))
            && let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] =
                function.instruction(branch).operands[..]
            && yes != no
        {
            let alone = |to: llrm_mir::module::BlockId| {
                shape.dominance.dominates(unit.function, cfg::id(to), at)
                    && function.predecessors(to).iter().all(|&from| {
                        from == cfg::block(above)
                            || shape.dominance.dominates(unit.function, cfg::id(to), cfg::id(from))
                    })
            };
            if alone(yes) {
                _proven(unit, condition, true, &mut found);
            } else if alone(no) {
                _proven(unit, condition, false, &mut found);
            }
        }
        reached = above;
    }
    found
}

/// What holds at the end of `from` going on to `to`: the compares proven on
/// entry to `from`, and the one its own branch says of the edge.
pub fn on_edge(
    unit: &Unit,
    from: llrm_mir::module::BlockId,
    to: llrm_mir::module::BlockId,
) -> Vec<Guard> {
    let function = unit.function;
    let mut facts = guards(unit, cfg::id(from));
    if let Some(branch) = function.terminator(from)
        && let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] =
            function.instruction(branch).operands[..]
        && yes != no
        && (yes == to || no == to)
    {
        _proven(unit, condition, yes == to, &mut facts);
    }
    facts
}

/// Whether `facts` decide the compare `condition`: its answer, if they imply it
/// or its opposite.
pub fn decides(
    unit: &Unit,
    facts: &[Guard],
    condition: ValueId,
) -> Option<bool> {
    let function = unit.function;
    let ValueDef::Instruction(inst) = function.value(condition).def else { return None };
    let op = function.instruction(inst);
    let (Opcode::ICmp(predicate), [left, right]) = (&op.opcode, &op.operands[..]) else { return None };
    let (Some(left_term), Some(right_term), Some(width)) =
        (term(unit, *left), term(unit, *right), unit.int_bits(*left))
    else {
        return None;
    };
    let (left, right) = (Scev::of(&left_term, width), Scev::of(&right_term, width));
    if facts.iter().any(|guard| implies(guard, *predicate, &left, &right)) {
        return Some(true);
    }
    facts.iter().any(|guard| implies(guard, predicate.inverse(), &left, &right)).then_some(false)
}

/// The compares `condition` being `holds` proves: its own, or those of
/// the `and` it is true of, or the `or` it is false of.
fn _proven(
    unit: &Unit,
    condition: ValueId,
    holds: bool,
    found: &mut Vec<Guard>,
) {
    let function = unit.function;
    let ValueDef::Instruction(inst) = function.value(condition).def else { return };
    let op = function.instruction(inst);
    match (&op.opcode, &op.operands[..]) {
        (Opcode::ICmp(predicate), [left, right]) => {
            let (Some(left), Some(right), Some(width)) = (term(unit, *left), term(unit, *right), unit.int_bits(*left))
            else {
                return;
            };
            let predicate = if holds { *predicate } else { predicate.inverse() };
            found.push(Guard { predicate, left: Scev::of(&left, width), right: Scev::of(&right, width) });
        }
        (Opcode::Binary(kind @ (BinaryOp::And | BinaryOp::Or)), [Operand::Value(one), Operand::Value(other)])
            if (*kind == BinaryOp::And) == holds =>
        {
            _proven(unit, *one, holds, found);
            _proven(unit, *other, holds, found);
        }
        _ => {}
    }
}

/// Whether `left predicate right` holds on entry to block `at`: both
/// constants, or a guard there proves it.
pub fn holds(
    unit: &Unit,
    at: i64,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    if let (Some(one), Some(other)) = (left.known(), right.known()) {
        return evaluated(predicate, &one, &other, left.width);
    }
    holds_in(unit, at, &guards(unit, at), predicate, left, right)
}

/// `holds`, the guards on entry to `at` being those given: a caller that asks
/// more than once of the block finds them once.
pub fn holds_with(
    unit: &Unit,
    at: i64,
    found: &[Guard],
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    holds_in(unit, at, found, predicate, left, right)
}

/// `holds`, given the guards on entry to `at` (`guards`), which a caller asking
/// of one block again and again finds once: they were found four times for each
/// branch `decide` asked of (half of its time on QCport's sc).
fn holds_in(
    unit: &Unit,
    at: i64,
    facts: &[Guard],
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    if let (Some(one), Some(other)) = (left.known(), right.known()) {
        return evaluated(predicate, &one, &other, left.width);
    }
    if facts.iter().any(|guard| implies(guard, predicate, left, right)) {
        return true;
    }
    // An unsigned compare of zero extensions is the compare of what they
    // extend: the guard may be in the narrow type.
    if let Some((left, right)) = narrowed(unit, predicate, left, right)
        && (facts.iter().any(|guard| implies(guard, predicate, &left, &right))
            || holds_in(unit, at, facts, predicate, &left, &right))
    {
        return true;
    }
    through_phi(unit, at, predicate, left, right)
}

/// `phi predicate right` below the phi's block where each edge into the block
/// proves it of the value it brings: no branch dominates the block then, as in
/// a loop entered from a guard and from its own latch test (LLVM's
/// `isImpliedCondition` over a header phi's incoming values).
fn through_phi(
    unit: &Unit,
    at: i64,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    let function = unit.function;
    let shape = unit.shape();
    let phi_of = |one: &Scev| -> Option<llrm_mir::module::InstId> {
        let [(product, factor)] = one.terms.iter().collect::<Vec<_>>()[..] else { return None };
        if one.constant != BigInt::from(0) || *factor != BigInt::from(1) {
            return None;
        }
        let ValueDef::Instruction(inst) = function.value(product.single()?).def else { return None };
        (function.instruction(inst).opcode == Opcode::Phi).then_some(inst)
    };
    let (phi, from_left) = match (phi_of(left), phi_of(right)) {
        (Some(phi), None) => (phi, true),
        (None, Some(phi)) => (phi, false),
        _ => return false,
    };
    let Some(block) = function.parent(phi) else { return false };
    let other = if from_left { right } else { left };
    // The other side stays what it is on every trip: a constant, or a value
    // defined above the phi's block.
    let fixed = other.known().is_some()
        || other.unknowns().all(|value| match function.value(value).def {
            ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|home| {
                home != block && shape.dominance.dominates(unit.function, cfg::id(home), cfg::id(block))
            }),
            _ => true,
        });
    if !fixed || !shape.dominance.dominates(unit.function, cfg::id(block), at) {
        return false;
    }
    let operands = function.instruction(phi).operands.clone();
    if operands.len() < 2 {
        return false;
    }
    let width = left.width;
    operands
        .chunks(2)
        .all(
            |arm| {
                let Operand::Block(from) = &arm[1] else { return false };
                let Some(brought) = term(unit, arm[0]) else { return false };
                let brought = Scev::of(&brought, width);
                let (l, r) = if from_left { (&brought, right) } else { (left, &brought) };
                if let (Some(one), Some(two)) = (l.known(), r.known()) {
                    return evaluated(predicate, &one, &two, width);
                }
                // What holds at the end of the incoming block, and what its
                // branch says of the edge into this one.
                let mut proven = guards(unit, cfg::id(*from));
                if let Some(branch) = function.terminator(*from)
                    && let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] =
                        function.instruction(branch).operands[..]
                    && yes != no
                    && (yes == block || no == block)
                {
                    _proven(unit, condition, yes == block, &mut proven);
                }
                proven.iter().any(|guard| implies(guard, predicate, l, r))
            },
        )
}

/// The two sides read before the zero extension that made one of them, where
/// each is that or a constant it holds: `zext a` against `64` is `a` against
/// `64` one width down. Unsigned and equality compares only, which a zero
/// extension keeps.
fn narrowed(
    unit: &Unit,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> Option<(Scev, Scev)> {
    use IntPredicate::*;
    if !matches!(predicate, Ult | Ule | Ugt | Uge | Eq | Ne) {
        return None;
    }
    let source = |one: &Scev| -> Option<(ValueId, u32)> {
        let [(product, factor)] = one.terms.iter().collect::<Vec<_>>()[..] else { return None };
        if one.constant != BigInt::from(0) || *factor != BigInt::from(1) {
            return None;
        }
        let value = product.single()?;
        let ValueDef::Instruction(inst) = unit.function.value(value).def else { return None };
        let op = unit.function.instruction(inst);
        let (Opcode::Cast(llrm_mir::opcode::CastOp::ZExt), [Operand::Value(from)]) = (&op.opcode, &op.operands[..])
        else {
            return None;
        };
        Some((*from, unit.int_bits(Operand::Value(*from))?))
    };
    let narrow = |one: &Scev, bits: u32| -> Option<Scev> {
        if let Some((from, width)) = source(one) {
            return (width == bits).then(|| Scev::of(&crate::induction::AffineOperand::Value(from, width), width));
        }
        let value = one.known().filter(|_| one.terms.is_empty())?;
        let unsigned = if value < BigInt::from(0) { value + (BigInt::from(1) << one.width) } else { value };
        (unsigned < (BigInt::from(1) << bits)).then(|| Scev::constant(unsigned, bits))
    };
    let bits = source(left).or_else(|| source(right))?.1;
    Some((narrow(left, bits)?, narrow(right, bits)?))
}

/// `holds`, given also `assumed` and what the program states of the
/// values the guards test: their ranges, by difference bounds.
pub fn holds_given(
    unit: &Unit,
    at: i64,
    assumed: &[Guard],
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    Given::at(unit, at, assumed).holds(unit, predicate, left, right)
}

/// What is given on entry to a block: the guards there, and with the `assumed`
/// ones and the loops' proofs of their counters' followers the facts a range
/// proof reads. Found once, for as many questions of the block as are asked.
pub struct Given<'a> {
    at: i64,
    guards: Vec<Guard>,
    assumed: &'a [Guard],
    facts: std::cell::OnceCell<Vec<Guard>>,
}

impl<'a> Given<'a> {
    pub fn at(
        unit: &Unit,
        at: i64,
        assumed: &'a [Guard],
    ) -> Self {
        Self { at, guards: guards(unit, at), assumed, facts: std::cell::OnceCell::new() }
    }

    /// `holds_given`.
    pub fn holds(
        &self,
        unit: &Unit,
        predicate: IntPredicate,
        left: &Scev,
        right: &Scev,
    ) -> bool {
        if holds_in(unit, self.at, &self.guards, predicate, left, right) {
            return true;
        }
        let facts = self
            .facts
            .get_or_init(
                || {
                    let mut facts = self.guards.clone();
                    facts.extend(self.assumed.iter().cloned());
                    // What the loops holding the block prove of the phis that
                    // follow their counters.
                    let shape = unit.shape();
                    for loop_ in shape.loops.iter().filter(|one| one.body.contains(&self.at)) {
                        for (follower, counter, start) in crate::induction::followers_of(unit, loop_) {
                            let Some(width) = unit.int_bits(Operand::Value(follower)) else { continue };
                            let (follower, counter) = (Scev::unknown(follower, width), Scev::unknown(counter, width));
                            facts.push(Guard {
                                predicate: IntPredicate::Sle,
                                left: Scev::of(&start, width),
                                right: follower.clone(),
                            });
                            facts.push(Guard { predicate: IntPredicate::Sle, left: follower, right: counter });
                        }
                    }
                    facts
                },
            );
        let mut ranges = BTreeMap::new();
        for side in facts.iter().flat_map(|one| [&one.left, &one.right]).chain([left, right]) {
            for monomial in side.terms.keys() {
                if let Some(value) = monomial.single()
                    && let Some(interval) = declared(unit, value).filter(|one| one.width == left.width)
                {
                    ranges.insert(monomial.clone(), (interval.low, interval.high));
                }
            }
        }
        difference::proves(left.width, facts, &ranges, predicate, left, right)
    }
}

/// Whether `guard` proves `left predicate right`.
pub fn implies(
    guard: &Guard,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    // `a != 0` proves `0 <u a`: the guard of a loop that counts up from zero to
    // `a`, as LLVM's isLoopEntryGuardedByCond finds it.
    if guard.predicate == IntPredicate::Ne {
        let zero = |one: &Scev| one.known().is_some_and(|value| value == BigInt::from(0));
        let nonzero = if zero(&guard.right) {
            Some(&guard.left)
        } else if zero(&guard.left) {
            Some(&guard.right)
        } else {
            None
        };
        if let Some(value) = nonzero {
            let below = (predicate == IntPredicate::Ult && zero(left) && right == value)
                || (predicate == IntPredicate::Ugt && zero(right) && left == value);
            if below {
                return true;
            }
        }
    }
    if guard.left == *left && guard.right == *right {
        return _stronger(guard.predicate, predicate);
    }
    if guard.left == *right && guard.right == *left && _stronger(guard.predicate.swapped(), predicate) {
        return true;
    }
    // Equality is the same at any offset: a wrapping sum is a bijection, so `a
    // != 1` proves `a - 1 != 0` (LLVM's isKnownPredicate compares the two
    // sides' difference). Sides that differ only in their constants, so that no
    // difference is built for the many guards a question meets.
    let equality = |one: IntPredicate| matches!(one, IntPredicate::Eq | IntPredicate::Ne);
    if equality(guard.predicate) && equality(predicate) && _stronger(guard.predicate, predicate) {
        let modulus = BigInt::from(1) << left.width;
        let wrapped = |n: BigInt| ((n % &modulus) + &modulus) % &modulus;
        let given = wrapped(&guard.left.constant - &guard.right.constant);
        let asked = wrapped(&left.constant - &right.constant);
        if guard.left.terms == left.terms && guard.right.terms == right.terms {
            return given == asked;
        }
        if guard.left.terms == right.terms && guard.right.terms == left.terms {
            return given == wrapped(-asked);
        }
    }
    false
}

/// Whether `guard` proves the signed `left predicate right` where the two read
/// the same values at other constants: `n > 2` proves `1 < n - 1`, as LLVM's
/// isKnownPredicate compares the sides' difference. Exact only where no side
/// wraps, which the caller checks of the asked sides (an `nsw` chain); each
/// guard side is a value or a number, so none of its constant came through a
/// sum.
pub fn implies_at_offset(
    guard: &Guard,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    offset_implies(guard, predicate, left, right, true)
}

/// [`implies_at_offset`] for a guard whose sides the caller has read as a
/// root and a constant through `nsw` sums (`n - 1 >= 0` as `n` and -1): its
/// constants are the guard's own.
pub fn implies_at_rooted_offset(
    guard: &Guard,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    offset_implies(guard, predicate, left, right, false)
}

fn offset_implies(
    guard: &Guard,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
    pure_only: bool,
) -> bool {
    // `a < b` or `a <= b` as `(a, b, strict)`.
    let ordered = |predicate: IntPredicate, left: &Scev, right: &Scev| match predicate {
        IntPredicate::Slt => Some((left.clone(), right.clone(), true)),
        IntPredicate::Sle => Some((left.clone(), right.clone(), false)),
        IntPredicate::Sgt => Some((right.clone(), left.clone(), true)),
        IntPredicate::Sge => Some((right.clone(), left.clone(), false)),
        _ => None,
    };
    let pure = |one: &Scev| one.terms.is_empty() || one.constant == BigInt::from(0);
    if pure_only && (!pure(&guard.left) || !pure(&guard.right)) {
        return false;
    }
    let (Some((given_a, given_b, given_strict)), Some((asked_a, asked_b, asked_strict))) =
        (ordered(guard.predicate, &guard.left, &guard.right), ordered(predicate, left, right))
    else {
        return false;
    };
    if given_a.terms != asked_a.terms || given_b.terms != asked_b.terms || given_a.width != asked_a.width {
        return false;
    }
    let width = given_a.width;
    let signed = |n: &BigInt| {
        let modulus = BigInt::from(1) << width;
        let low = ((n % &modulus) + &modulus) % &modulus;
        if low >= (BigInt::from(1) << (width - 1)) { low - modulus } else { low }
    };
    // `a < b` is `terms(a) - terms(b) + (ca - cb) + 1 <= 0`; the same terms
    // on both sides, the guard's constant is the lesser.
    let reach =
        |a: &Scev, b: &Scev, strict: bool| signed(&a.constant) - signed(&b.constant) + BigInt::from(u8::from(strict));
    reach(&asked_a, &asked_b, asked_strict) <= reach(&given_a, &given_b, given_strict)
}

/// Whether `strong` holding of two sides makes `weak` hold of them.
fn _stronger(
    strong: IntPredicate,
    weak: IntPredicate,
) -> bool {
    use IntPredicate::*;
    strong == weak
        || matches!(
            (strong, weak),
            (Eq, Ule | Uge | Sle | Sge) | (Ult, Ule | Ne) | (Ugt, Uge | Ne) | (Slt, Sle | Ne) | (Sgt, Sge | Ne)
        )
}

/// `one predicate other` of two signed numbers read at `width` bits.
pub fn evaluated(
    predicate: IntPredicate,
    one: &BigInt,
    other: &BigInt,
    width: u32,
) -> bool {
    use IntPredicate::*;
    let modulus = BigInt::from(1) << width;
    let unsigned = |n: &BigInt| ((n % &modulus) + &modulus) % &modulus;
    let (a, b) = (unsigned(one), unsigned(other));
    let half = BigInt::from(1) << (width - 1);
    let signed = |n: &BigInt| if *n >= half { n - &modulus } else { n.clone() };
    let (s, t) = (signed(&a), signed(&b));
    match predicate {
        Eq => a == b,
        Ne => a != b,
        Ult => a < b,
        Ule => a <= b,
        Ugt => a > b,
        Uge => a >= b,
        Slt => s < t,
        Sle => s <= t,
        Sgt => s > t,
        Sge => s >= t,
    }
}

#[cfg(test)]
#[path = "guards_tests.rs"]
mod tests;
