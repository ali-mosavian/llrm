//! A 32-bit fixed-point product whose operands' product fits 32 bits is a `mul`
//! and an `ashr`.
//!
//! # How it works
//! `llvm.smul.fix(a, b, s)` is `(a * b) >> s` computed in 64 bits. isel lowers
//! it to the one-operand `imul` and `shrd`, which take EAX and EDX; the narrow
//! form `mul a, b; ashr s` is `imul r, r` and `sar`. The two are the same value
//! where the 64-bit product never leaves 32 bits.
//!
//! The test is on the operands, from LazyValueInfo's `getConstantRange` of each
//! at the call, not on the result: `smul.fix(65536, 65536, 16)` is 65536, but
//! its product is 2^32 and keeps the wide form. The product's bounds are
//! `ConstantRange::multiply` of the two ranges taken in 64 bits; it must lie
//! in `[-2^31, 2^31)`. Only 32-bit products with a constant scale below 32 are
//! taken.
//!
//! DEVIATE 1: not an LLVM pass. LazyValueInfo answers from definitions and
//! the branches on the edges to the call, not from trip counts or from a
//! test on a derived value, so a factor bounded only by a loop's escape test
//! (`examples/mandel`: `zr^2 + zi^2 > 4`) would keep its wide form, where the
//! `ranges` engine this replaced narrowed it. [`Query`] adds that engine's
//! reach for Q-format squares only: a square is not negative and, where the
//! branch to the call bounds a sum of two squares, at most the bound; a
//! square at most `M` bounds its factor by `isqrt(((M + 1) << s) - 1)`; and
//! sums, differences, shifts, products, sign extensions and phis of bounded
//! values are bounded, a counter `start, start + 1, ...` that leaves on
//! `next == end` by `[start, end - 1]`.
//!
//! A factor `v << k` or `v + v` whose interval keeps it from wrapping leaves
//! the product for the scale: `(v * 2^k * b) >> s` is `(v * b) >> (s - k)`, so
//! `x * 2 * y` in Q8 is one `sar` by 7, and a fixed-point integer made by `<<
//! s` and scaled back costs nothing (the scale reaches 0 and the `ashr` is
//! dropped).
//!
//! ```text
//!  %x2 = shl %x, 1                          %m = mul %x, %y
//!  %p = call smul.fix(%x2, %y, 8)     ==>   %p = ashr %m, 7
//!  (x and y small enough that x*y fits)
//! ```
//!
//! DEVIATE LP-3 (no pin pass: SelectionDAG expands `smul.fix`
//! (`TargetLowering::expandFixedPointMul`); exit: isel does it and this is
//! deleted; owner w-6; review 2026-11-20).
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
//! b0:
//!   %t = add i32 %a, %a
//!   %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
//!   ret i32 %p
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
//! b0:
//!   %t = add i32 %a, %a
//!   %0 = mul i32 %a, %b
//!   %1 = ashr i32 %0, 7
//!   ret i32 %1
//! }
//! ```
//!
//! The doubling leaves the product and the scale drops from 8 to 7; `%t` is
//! left for dead-code removal.
//!
//! From unit test `a_doubled_factor_leaves_the_product_for_the_scale` in
//! `fixednarrow_tests.rs` (fixture `SHIFTED`; the printed text was taken from a
//! run).
//!
//! # Runs
//! Once, after the loop-header copy and before `lsr`, which would hide such a
//! shift inside an induction variable. Not at -O0.
//!
//! # Needs
//! LazyValueInfo (`LviCache`).
//!
//! # Keeps
//! Blocks and edges are unchanged; dominators and loops are preserved.

use std::rc::Rc;

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo, LviCache};
use llrm_mir::module::{GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::valuetracking::AssumptionCache;

pub struct FixedNarrow;

impl FunctionPass for FixedNarrow {
    fn name(&self) -> &'static str {
        "fixednarrow"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let globals = Rc::clone(&analyses.outer().globals);
        let calls: Vec<InstId> = unit
            .function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| {
                llrm_mir::memory::callee(unit.context, unit.function, inst)
                    .and_then(|callee| globals.get(callee.0 as usize)?.intrinsic())
                    == Some(Intrinsic::Fixed { divide: false })
            })
            .collect();
        if calls.is_empty() {
            return PreservedAnalyses::all();
        }
        let assumptions = AssumptionCache::of(unit.context, &globals, unit.metadata, unit.function);
        let shared = analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        let mut lvi = shared.take();
        let mut changed = false;
        let mut squares: Vec<Square> = Vec::new();
        // One product at a time, rewritten before the next is asked: a product
        // that reads an earlier one's result asks LazyValueInfo about the
        // `mul` and `ashr` that replaced it.
        for inst in calls {
            let Some(product) = narrowable(unit, &globals, &assumptions, &mut lvi, &squares, inst) else { continue };
            let result = rewrite(unit, &mut lvi, &product);
            if product.square
                && let Operand::Value(value) = result
            {
                squares.push(Square { value, factor: product.a, scale: product.scale });
            }
            changed = true;
        }
        shared.put(lvi);
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `smul.fix` `inst` as `(a * b) >> scale`.
struct Narrow {
    inst: InstId,
    a: Operand,
    b: Operand,
    scale: u32,
    /// Both factors were one value: the product is not negative.
    square: bool,
}

/// A square already narrowed: `smul.fix(factor, factor, scale)` is `value`.
struct Square {
    value: llrm_mir::module::ValueId,
    factor: Operand,
    scale: u32,
}

/// `product` replaced by the `mul` and `ashr` that compute it.
fn rewrite(
    unit: &mut passes::Unit,
    lvi: &mut LazyValueInfo,
    product: &Narrow,
) -> Operand {
    let ty = unit.function.instruction(product.inst).ty;
    let make = |unit: &mut passes::Unit, opcode, operands| {
        let new = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        unit.function.insert(new, Position::Before(product.inst)).expect("a placed instruction");
        Operand::Value(unit.function.instruction(new).result.expect("a value"))
    };
    let mut result = make(unit, Opcode::Binary(BinaryOp::Mul), vec![product.a, product.b]);
    if product.scale > 0 {
        let by = Operand::Constant(unit.context.int(ty, i128::from(product.scale)));
        result = make(unit, Opcode::Binary(BinaryOp::AShr), vec![result, by]);
    }
    let old = unit.function.instruction(product.inst).result.expect("a value");
    unit.function.replace_all_uses_with(old, result);
    lvi.forget_value(Operand::Value(old));
    unit.function.erase(product.inst).expect("its uses were replaced");
    result
}

/// The 32-bit width the product is taken in: only dwords are narrowed.
const WIDTH: u32 = 32;

/// `inst` as a `mul` and a shift, if its operands, less the shifts they
/// carry, multiply within their width.
fn narrowable(
    unit: &mut passes::Unit,
    globals: &[GlobalValue],
    assumptions: &AssumptionCache,
    lvi: &mut LazyValueInfo,
    squares: &[Square],
    inst: InstId,
) -> Option<Narrow> {
    let operands = unit.function.instruction(inst).operands.clone();
    let mut scale = u32::try_from(int_constant(unit, operands[2])?).ok().filter(|&scale| scale < WIDTH)?;
    let mut range = |unit: &mut passes::Unit, one: Operand| -> Option<ConstantRange> {
        let mut query = Query { globals, assumptions, squares, lvi: &mut *lvi };
        Some(query.range(unit, one, inst, GUARD_DEPTH)).filter(|range| range.width == WIDTH)
    };
    // `v << k` or `v + v` where `v`'s range keeps it from wrapping: `v * 2^k`.
    let doubled = |unit: &mut passes::Unit,
                   range: &mut dyn FnMut(&mut passes::Unit, Operand) -> Option<ConstantRange>,
                   one: Operand,
                   scale: u32|
     -> Option<(Operand, u32)> {
        let Operand::Value(value) = one else { return None };
        let ValueDef::Instruction(def) = unit.function.value(value).def else { return None };
        let (source, bits) =
            match (&unit.function.instruction(def).opcode, &unit.function.instruction(def).operands[..]) {
                (Opcode::Binary(BinaryOp::Shl), &[source, count]) => {
                    (source, u32::try_from(int_constant(unit, count)?).ok()?)
                }
                (Opcode::Binary(BinaryOp::Add), &[source, other]) if source == other => (source, 1),
                _ => return None,
            };
        (1..=scale).contains(&bits).then_some(())?;
        let v = range(unit, source)?;
        fits(&v.sign_extend(2 * WIDTH).multiply(&ConstantRange::single(2 * WIDTH, 1 << bits), 0))?
            .then_some((source, bits))
    };
    let mut taken = [operands[0], operands[1]];
    for one in &mut taken {
        while let Some((source, bits)) = doubled(unit, &mut range, *one, scale) {
            *one = source;
            scale -= bits;
        }
    }
    let wide = |range: ConstantRange| range.sign_extend(2 * WIDTH);
    let product = wide(range(unit, taken[0])?).multiply(&wide(range(unit, taken[1])?), 0);
    fits(&product)?.then_some(Narrow { inst, a: taken[0], b: taken[1], scale, square: operands[0] == operands[1] })
}

/// How many phis and sums a bound is followed through.
const GUARD_DEPTH: u32 = 10;

/// A closed interval of signed values.
#[derive(Clone, Copy)]
struct Interval {
    low: i128,
    high: i128,
}

impl Interval {
    const MIN: i128 = -(1_i128 << (WIDTH - 1));
    const MAX: i128 = (1_i128 << (WIDTH - 1)) - 1;

    fn of(range: &ConstantRange) -> Self {
        if range.is_full_set() || range.is_empty_set() {
            return Interval { low: Self::MIN, high: Self::MAX };
        }
        let sign = |bits: u128| {
            let bits = bits & ((1_u128 << range.width) - 1);
            bits as i128 - if bits >> (range.width - 1) == 1 { 1_i128 << range.width } else { 0 }
        };
        Interval { low: sign(range.get_signed_min()), high: sign(range.get_signed_max()) }
    }

    fn range(
        self,
        width: u32,
    ) -> ConstantRange {
        ConstantRange::non_empty(width, self.low as u128, (self.high + 1) as u128)
    }

    fn within(self) -> bool {
        Self::MIN <= self.low && self.high <= Self::MAX
    }

    fn meet(
        self,
        other: Interval,
    ) -> Interval {
        Interval { low: self.low.max(other.low), high: self.high.min(other.high) }
    }

    fn join(
        self,
        other: Interval,
    ) -> Interval {
        Interval { low: self.low.min(other.low), high: self.high.max(other.high) }
    }

    /// The interval of `a op b` for `op` a monotone combination of corners.
    fn corners(
        self,
        other: Interval,
        op: impl Fn(i128, i128) -> i128,
    ) -> Interval {
        let all =
            [op(self.low, other.low), op(self.low, other.high), op(self.high, other.low), op(self.high, other.high)];
        Interval { low: *all.iter().min().unwrap(), high: *all.iter().max().unwrap() }
    }
}

/// The largest `r` with `r * r <= n`.
fn isqrt(n: i128) -> i128 {
    let mut root = (n as f64).sqrt() as i128;
    while root * root > n {
        root -= 1;
    }
    while (root + 1) * (root + 1) <= n {
        root += 1;
    }
    root
}

/// What LazyValueInfo is asked, and what the fixed-point squares add to it.
///
/// A square `smul.fix(p, p, s)` is not negative, and where `sq + other` is a
/// sum of two squares the branch to the call bounds (`sq + other > C` left),
/// each square is at most `C`: the escape test of a Mandelbrot loop. A
/// square at most `M` bounds its factor by `isqrt(((M + 1) << s) - 1)`, and
/// a phi, sum or difference of bounded values is bounded by its inputs.
struct Query<'a> {
    globals: &'a [GlobalValue],
    assumptions: &'a AssumptionCache,
    /// The squares already narrowed, which no longer are calls.
    squares: &'a [Square],
    lvi: &'a mut LazyValueInfo,
}

impl Query<'_> {
    /// `one`'s range at `at`, narrowed by what the squares say.
    fn range(
        &mut self,
        unit: &mut passes::Unit,
        one: Operand,
        at: InstId,
        depth: u32,
    ) -> ConstantRange {
        let mut env =
            Env::new(&mut *unit.context, self.globals, unit.metadata, &*unit.function, unit.layout, self.assumptions);
        let known = self.lvi.get_constant_range(&mut env, one, at, false);
        if known.width > WIDTH || depth == 0 {
            return known;
        }
        match self.derived(unit, one, at, depth, known.width) {
            Some(derived) => Interval::of(&known).meet(derived).range(known.width),
            None => known,
        }
    }

    fn interval(
        &mut self,
        unit: &mut passes::Unit,
        one: Operand,
        at: InstId,
        depth: u32,
    ) -> Interval {
        Interval::of(&self.range(unit, one, at, depth))
    }

    /// The factor `smul.fix(p, p, s)` of a square: `p` and the scale.
    fn square_of(
        &self,
        unit: &passes::Unit,
        inst: InstId,
    ) -> Option<(Operand, u32)> {
        let instruction = unit.function.instruction(inst);
        let Some(callee) = llrm_mir::memory::callee(unit.context, unit.function, inst) else {
            let result = instruction.result?;
            return self
                .squares
                .iter()
                .find(|square| square.value == result)
                .map(|square| (square.factor, square.scale));
        };
        (self.globals.get(callee.0 as usize)?.intrinsic() == Some(Intrinsic::Fixed { divide: false })).then_some(())?;
        let &[a, b, scale, ..] = &instruction.operands[..] else { return None };
        (a == b).then_some(())?;
        Some((a, u32::try_from(int_constant(unit, scale)?).ok()?))
    }

    fn derived(
        &mut self,
        unit: &mut passes::Unit,
        one: Operand,
        at: InstId,
        depth: u32,
        width: u32,
    ) -> Option<Interval> {
        let Operand::Value(value) = one else { return None };
        // A factor of a square the escape test bounds.
        let users: Vec<InstId> = unit.function.users(value).into_iter().map(|one| one.user).collect();
        for user in users {
            if let Some((factor, scale)) = self.square_of(unit, user)
                && factor == one
                && let Some(high) = self.square_high(unit, user, at, depth)
            {
                let root = isqrt(((high + 1) << scale) - 1);
                return Some(Interval { low: -root, high: root });
            }
        }
        let narrowed: Vec<(llrm_mir::module::ValueId, u32)> =
            self.squares.iter().filter(|square| square.factor == one).map(|s| (s.value, s.scale)).collect();
        for (square, scale) in narrowed {
            let ValueDef::Instruction(def) = unit.function.value(square).def else { continue };
            if let Some(high) = self.square_high(unit, def, at, depth) {
                let root = isqrt(((high + 1) << scale) - 1);
                return Some(Interval { low: -root, high: root });
            }
        }
        let ValueDef::Instruction(def) = unit.function.value(value).def else { return None };
        if self.square_of(unit, def).is_some() {
            let high = self.square_high(unit, def, at, depth).unwrap_or(Interval::MAX);
            return Some(Interval { low: 0, high });
        }
        let instruction = unit.function.instruction(def);
        let (opcode, operands) = (instruction.opcode.clone(), instruction.operands.clone());
        match (opcode, &operands[..]) {
            (Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub)), &[a, b]) => {
                let (a, b) = (self.interval(unit, a, at, depth - 1), self.interval(unit, b, at, depth - 1));
                let sum = if op == BinaryOp::Add { a.corners(b, |a, b| a + b) } else { a.corners(b, |a, b| a - b) };
                sum.within().then_some(sum)
            }
            (Opcode::Cast(CastOp::SExt), &[from]) => Some(self.interval(unit, from, at, depth - 1)),
            (Opcode::Binary(op @ (BinaryOp::AShr | BinaryOp::Shl)), &[a, by]) => {
                let by = i128::from(u32::try_from(int_constant(unit, by)?).ok().filter(|&by| by < WIDTH)?);
                let a = self.interval(unit, a, at, depth - 1);
                let shifted = if op == BinaryOp::AShr {
                    Interval { low: a.low >> by, high: a.high >> by }
                } else {
                    Interval { low: a.low << by, high: a.high << by }
                };
                shifted.within().then_some(shifted)
            }
            (Opcode::Binary(BinaryOp::Mul), &[a, b]) => {
                let (a, b) = (self.interval(unit, a, at, depth - 1), self.interval(unit, b, at, depth - 1));
                let product = a.corners(b, |a, b| a * b);
                product.within().then_some(product)
            }
            (Opcode::Phi, _) => {
                if let Some(counter) = self.counts_up(unit, def, width) {
                    return Some(counter);
                }
                (width == WIDTH).then_some(())?;
                let incoming: Vec<(Operand, llrm_mir::module::BlockId)> =
                    llrm_mir::valuetracking::pattern::phi_incoming(unit.function.instruction(def)).collect();
                let mut joined: Option<Interval> = None;
                for (from, block) in incoming {
                    let edge = unit.function.terminator(block)?;
                    let one = self.interval(unit, from, edge, depth - 1);
                    joined = Some(joined.map_or(one, |earlier| earlier.join(one)));
                }
                joined
            }
            (Opcode::Call(_), &[a, b, scale, ..]) => {
                let scale = u32::try_from(int_constant(unit, scale)?).ok()?;
                let callee = llrm_mir::memory::callee(unit.context, unit.function, def)?;
                (self.globals.get(callee.0 as usize)?.intrinsic() == Some(Intrinsic::Fixed { divide: false }))
                    .then_some(())?;
                let (a, b) = (self.interval(unit, a, at, depth - 1), self.interval(unit, b, at, depth - 1));
                let product = a.corners(b, |a, b| a * b);
                let scaled = Interval { low: product.low >> scale, high: product.high >> scale };
                (product.within() && scaled.within()).then_some(scaled)
            }
            _ => None,
        }
    }

    /// `bits` of a `width`-bit constant, signed.
    fn signed_constant(
        unit: &passes::Unit,
        operand: Operand,
        width: u32,
    ) -> Option<i128> {
        let bits = int_constant(unit, operand)? as i128;
        Some(if bits >= 1_i128 << (width - 1) { bits - (1_i128 << width) } else { bits })
    }

    /// A counter `start, start + 1, ...` that leaves when `next == end`: a
    /// phi of a constant and `phi + 1` (no signed wrap) whose latch branches
    /// back on `next != end`.
    fn counts_up(
        &self,
        unit: &passes::Unit,
        phi: InstId,
        width: u32,
    ) -> Option<Interval> {
        let incoming: Vec<(Operand, llrm_mir::module::BlockId)> =
            llrm_mir::valuetracking::pattern::phi_incoming(unit.function.instruction(phi)).collect();
        let [(first, first_block), (second, second_block)] = incoming[..] else { return None };
        let (start, next, latch) =
            match (Self::signed_constant(unit, first, width), Self::signed_constant(unit, second, width)) {
                (Some(start), None) => (start, second, second_block),
                (None, Some(start)) => (start, first, first_block),
                _ => return None,
            };
        let phi_value = unit.function.instruction(phi).result?;
        let Operand::Value(next_value) = next else { return None };
        let ValueDef::Instruction(step) = unit.function.value(next_value).def else { return None };
        let step = unit.function.instruction(step);
        let by_one = step.opcode == Opcode::Binary(BinaryOp::Add)
            && step.flags.contains(Flags::NSW)
            && step.operands[0] == Operand::Value(phi_value)
            && Self::signed_constant(unit, step.operands[1], width) == Some(1);
        by_one.then_some(())?;
        let branch = unit.function.instruction(unit.function.terminator(latch)?);
        let &[Operand::Value(condition), Operand::Block(leave), Operand::Block(stay)] = &branch.operands[..] else {
            return None;
        };
        let ValueDef::Instruction(compare) = unit.function.value(condition).def else { return None };
        let compare = unit.function.instruction(compare);
        let end = Self::signed_constant(unit, compare.operands[1], width)?;
        (compare.opcode == Opcode::ICmp(IntPredicate::Eq)
            && compare.operands[0] == next
            && unit.function.parent(phi) == Some(stay)
            && leave != stay
            && start < end)
            .then_some(Interval { low: start, high: end - 1 })
    }

    /// The most a square `sq` can be at `at`: where it is summed with another
    /// square and the sum is bounded there, the bound of the sum.
    fn square_high(
        &mut self,
        unit: &mut passes::Unit,
        sq: InstId,
        at: InstId,
        depth: u32,
    ) -> Option<i128> {
        let result = unit.function.instruction(sq).result?;
        let sums: Vec<InstId> = unit.function.users(result).into_iter().map(|one| one.user).collect();
        for sum in sums {
            let instruction = unit.function.instruction(sum);
            let (Opcode::Binary(BinaryOp::Add), &[a, b]) = (&instruction.opcode, &instruction.operands[..]) else {
                continue;
            };
            let other = if a == Operand::Value(result) { b } else { a };
            let Operand::Value(other) = other else { continue };
            let ValueDef::Instruction(other) = unit.function.value(other).def else { continue };
            if self.square_of(unit, other).is_none() {
                continue;
            }
            let total = unit.function.instruction(sum).result?;
            // Not `range`: the sum's own bound is asked of LazyValueInfo.
            let mut env = Env::new(
                &mut *unit.context,
                self.globals,
                unit.metadata,
                &*unit.function,
                unit.layout,
                self.assumptions,
            );
            let known = Interval::of(&self.lvi.get_constant_range(&mut env, Operand::Value(total), at, false));
            let _ = depth;
            if known.high < Interval::MAX && known.high >= 0 {
                return Some(known.high);
            }
        }
        None
    }
}

/// Whether `range`, taken in twice the width, lies within the signed width.
fn fits(range: &ConstantRange) -> Option<bool> {
    let limit = 1_i128 << (WIDTH - 1);
    let sign = |bits: u128| i128::from(bits as u64 as i64);
    Some(-limit <= sign(range.get_signed_min()) && sign(range.get_signed_max()) < limit)
}

/// An integer constant's bits, if `operand` is one.
fn int_constant(
    unit: &passes::Unit,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match unit.context.get(id).kind {
            llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "fixednarrow_tests.rs"]
mod tests;
