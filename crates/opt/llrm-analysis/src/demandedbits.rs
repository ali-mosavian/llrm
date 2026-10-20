//! Which bits of each integer value anything reads: a translation of LLVM's
//! `DemandedBits` (`DemandedBits.cpp` at d1106deb71cc).
//!
//! # How it works
//! A backward fixed point over the use-def graph. Every instruction that must
//! stay (a terminator, a landing pad, one with a side effect) is a root: its
//! integer operands are read in full, and a root that is itself an integer
//! value reads what its own operands need of nothing. A worklist then asks each
//! visited instruction, given the bits of its result that are read, which bits
//! of each operand are (`determine_live_operand_bits`: a table per opcode,
//! using `compute_known_bits` for `and`, `or`, `add`, `sub` and variable
//! shifts). Demand only grows and is bounded by the width, so loops settle.
//!
//! An instruction nothing visits is dead; a use whose operand bits are all
//! unread is a dead use. Both survive edits that do not touch the ones asked.
//!
//! ```text
//!   %a = load i32          read: 0x000000ff   (only the low byte)
//!   %m = and i32 %a, 255   read: 0xffffffff   <- stored in full
//!   store i32 %m, ptr %p
//! ```
//!
//! # DEVIATEs
//! 1. Vectors, `bswap`, `bitreverse`, `ctlz`, `cttz`, `fshl`, `fshr`, `clmul`
//!    and the vector element operations are not MIR constructs: their cases are
//!    absent, which is what LLVM's `default` does for an operand it has no rule
//!    for. Of the intrinsics only `{s,u}{min,max}` exists.
//! 2. Integers of more than 128 bits are not tracked (MIR has none).
//! 3. The known-bits queries carry no dominator tree, assumption cache or
//!    dominating-condition cache (`Query` has them as `None`) until the pass
//!    manager hands them out: LLVM's answers can be stronger, so a demanded
//!    mask can be wider than LLVM's, never narrower.
//!
//! # Example
//! See the tests in `demandedbits_tests.rs`; `and i32 %a, 255` demands only
//! the low byte of `%a`.
//!
//! # Provides
//! [`DemandedBits::of`], then `demanded_bits`, `demanded_bits_of_use`,
//! `is_instruction_dead` and `is_use_dead`. Consumer: `bdce` in
//! `llrm-transforms`.

use std::collections::BTreeSet;

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::knownbits::KnownBits;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{Function, GlobalValue, InstId, MetadataNode, Operand, Use, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::{Query, known};

/// What the questions read besides the function: the types and constants, the
/// effects of each callee and the module's globals (for an intrinsic's name).
#[derive(Clone, Copy)]
pub struct Environment<'a> {
    pub context: &'a Context,
    pub layout: &'a DataLayout,
    pub callees: &'a Callees,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
}

/// `DemandedBits` of one function.
#[derive(Debug, Default)]
pub struct DemandedBits {
    alive_bits: IdMap<InstId, u128>,
    /// Instructions whose result is not an integer and that something reads.
    visited: IdSet<InstId>,
    dead_uses: BTreeSet<Use>,
}

fn mask(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1_u128 << width) - 1 }
}

/// `APInt::getHighBitsSet(width, count)`.
fn high_bits_set(
    width: u32,
    count: u32,
) -> u128 {
    if count == 0 { 0 } else { mask(width) & !mask(width - count.min(width)) }
}

/// `APInt::getLowBitsSet(width, count)`.
fn low_bits_set(
    width: u32,
    count: u32,
) -> u128 {
    mask(count.min(width))
}

/// `APInt::getBitsSetFrom(width, from)`.
fn bits_set_from(
    width: u32,
    from: u32,
) -> u128 {
    mask(width) & !mask(from.min(width))
}

fn shl(
    value: u128,
    amount: u32,
    width: u32,
) -> u128 {
    if amount >= width { 0 } else { (value << amount) & mask(width) }
}

fn lshr(
    value: u128,
    amount: u32,
) -> u128 {
    if amount >= 128 { 0 } else { value >> amount }
}

fn reverse_bits(
    value: u128,
    width: u32,
) -> u128 {
    value.reverse_bits() >> (128 - width)
}

fn active_bits(value: u128) -> u32 {
    128 - value.leading_zeros()
}

/// `APInt::isMask()`: a non-empty run of ones from bit 0.
fn is_mask(value: u128) -> bool {
    value != 0 && value & value.wrapping_add(1) == 0
}

/// `getLimitedValue(limit)`.
fn limited(
    value: u128,
    limit: u32,
) -> u32 {
    value.min(u128::from(limit)) as u32
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L48>
fn is_always_live(
    environment: &Environment,
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).opcode.is_terminator()
        || !memory::only_value(environment.context, environment.callees, function, inst)
}

/// The width of an integer operand, or none for anything else.
fn integer_width(
    environment: &Environment,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    function
        .operand_type(environment.context, operand)
        .and_then(|ty| environment.context.types.int_bits(ty))
        .filter(|&width| width <= 128)
}

fn constant_value(
    environment: &Environment,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match environment.context.get(id).kind {
            llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

/// The two operands' known bits, worked out once for all the operands of one
/// user (`KnownBitsComputed`).
#[derive(Default)]
struct KnownCache(Option<(KnownBits, Option<KnownBits>)>);

impl KnownCache {
    /// `ComputeKnownBits(BitWidth, V1, V2)`.
    fn of(
        &mut self,
        environment: &Environment,
        function: &Function,
        user: InstId,
        first: Operand,
        second: Option<Operand>,
    ) -> (KnownBits, Option<KnownBits>) {
        *self
            .0
            .get_or_insert_with(
                || {
                    let query = Query {
                        globals: environment.globals,
                        metadata: environment.metadata,
                        ..Query::bare(environment.context, function, environment.layout)
                    }
                    .with_cxt(Some(user));
                    let known = |operand| known::compute_known_bits(&query, operand, 0);
                    (known(first), second.map(known))
                },
            )
    }
}

/// Which intrinsic `inst` calls.
fn intrinsic(
    environment: &Environment,
    function: &Function,
    inst: InstId,
) -> Option<Intrinsic> {
    let callee = memory::callee(environment.context, function, inst)?;
    let global = environment.globals.get(callee.0 as usize)?;
    global.intrinsic()
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L52>
fn determine_live_operand_bits(
    environment: &Environment,
    function: &Function,
    user: InstId,
    operand_no: usize,
    out: u128,
    out_width: u32,
    width: u32,
    cache: &mut KnownCache,
) -> u128 {
    let instruction = function.instruction(user);
    let operands = &instruction.operands;
    let mut live = mask(width);
    let flags = instruction.flags;
    // The shift amount is clamped to `width - 1` (`getLimitedValue`).
    let shifted_range = |minimum: u32, maximum: u32, left: bool| {
        let shift = |value: u128, amount: u32| if left { shl(value, amount, width) } else { lshr(value, amount) };
        let mut range = maximum - minimum;
        let (mut accumulated, mut shifted) = (out, out);
        let mut amount = 1_u32;
        while amount <= range {
            if range & amount != 0 {
                accumulated |= shift(shifted, range - amount + 1);
                range -= amount;
            }
            shifted |= shift(shifted, amount);
            amount <<= 1;
        }
        shift(accumulated, minimum)
    };
    let known_amount = |cache: &mut KnownCache| {
        let (known, _) = cache.of(environment, function, user, operands[1], None);
        (limited(known.min_value(), width - 1), limited(known.max_value(), width - 1))
    };
    match &instruction.opcode {
        Opcode::Call(_) | Opcode::Invoke(_) => {
            if let Some(Intrinsic::MinMax { .. }) = intrinsic(environment, function, user) {
                live = bits_set_from(width, out.trailing_zeros().min(out_width));
            }
        }
        Opcode::Binary(BinaryOp::Add) => {
            live = if is_mask(out) {
                out
            } else {
                let (lhs, rhs) = cache.of(environment, function, user, operands[0], Some(operands[1]));
                add_carry(operand_no, out, &lhs, &rhs.expect("two operands"), true, false)
            };
        }
        Opcode::Binary(BinaryOp::Sub) => {
            live = if is_mask(out) {
                out
            } else {
                let (lhs, rhs) = cache.of(environment, function, user, operands[0], Some(operands[1]));
                let rhs = rhs.expect("two operands");
                let negated = KnownBits { zero: rhs.one, one: rhs.zero, width };
                add_carry(operand_no, out, &lhs, &negated, false, true)
            };
        }
        Opcode::Binary(BinaryOp::Mul) => live = low_bits_set(width, active_bits(out)),
        Opcode::Binary(BinaryOp::Shl) if operand_no == 0 => {
            let high = match constant_value(environment, operands[1]) {
                Some(amount) => {
                    let amount = limited(amount, width - 1);
                    live = lshr(out, amount);
                    amount
                }
                None => {
                    let (minimum, maximum) = known_amount(cache);
                    live = shifted_range(minimum, maximum, false);
                    maximum
                }
            };
            // `nsw` reads one more high bit than `nuw`.
            let extra = if flags.contains(Flags::NSW) {
                high + 1
            } else if flags.contains(Flags::NUW) {
                high
            } else {
                0
            };
            live |= high_bits_set(width, extra);
        }
        Opcode::Binary(BinaryOp::LShr) if operand_no == 0 => {
            let exact = flags.contains(Flags::EXACT);
            match constant_value(environment, operands[1]) {
                Some(amount) => {
                    let amount = limited(amount, width - 1);
                    live = shl(out, amount, width);
                    if exact {
                        live |= low_bits_set(width, amount);
                    }
                }
                None => {
                    let (minimum, maximum) = known_amount(cache);
                    live = shifted_range(minimum, maximum, true);
                    if exact {
                        live |= low_bits_set(width, maximum);
                    }
                }
            }
        }
        Opcode::Binary(BinaryOp::AShr) if operand_no == 0 => {
            let exact = flags.contains(Flags::EXACT);
            let (low, sign) = match constant_value(environment, operands[1]) {
                Some(amount) => {
                    let amount = limited(amount, width - 1);
                    live = shl(out, amount, width);
                    (amount, out & high_bits_set(width, amount) != 0)
                }
                None => {
                    let (minimum, maximum) = known_amount(cache);
                    live = shifted_range(minimum, maximum, true);
                    (maximum, maximum != 0 && out & high_bits_set(width, maximum) != 0)
                }
            };
            if sign {
                live |= 1_u128 << (width - 1);
            }
            if exact {
                live |= low_bits_set(width, low);
            }
        }
        Opcode::Binary(BinaryOp::And) => {
            let (first, second) = cache.of(environment, function, user, operands[0], Some(operands[1]));
            let second = second.expect("two operands");
            live = out;
            if operand_no == 0 {
                live &= !second.zero;
            } else {
                live &= !(first.zero & !second.zero);
            }
        }
        Opcode::Binary(BinaryOp::Or) => {
            let (first, second) = cache.of(environment, function, user, operands[0], Some(operands[1]));
            let second = second.expect("two operands");
            live = out;
            if operand_no == 0 {
                live &= !second.one;
            } else {
                live &= !(first.one & !second.one);
            }
        }
        Opcode::Binary(BinaryOp::Xor) | Opcode::Phi => live = out,
        Opcode::Cast(CastOp::Trunc) => live = out,
        Opcode::Cast(CastOp::ZExt) => live = out & mask(width),
        Opcode::Cast(CastOp::SExt) => {
            live = out & mask(width);
            if lshr(out, width) != 0 {
                live |= 1_u128 << (width - 1);
            }
        }
        Opcode::Select if operand_no != 0 => live = out,
        _ => {}
    }
    live & mask(width)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L564>
fn add_carry(
    operand_no: usize,
    out: u128,
    lhs: &KnownBits,
    rhs: &KnownBits,
    carry_zero: bool,
    carry_one: bool,
) -> u128 {
    assert!(!(carry_zero && carry_one), "Carry can't be zero and one at the same time");
    let width = lhs.width;
    let all = mask(width);
    let bound = (lhs.zero & rhs.zero) | (lhs.one & rhs.one);
    let reversed_bound = reverse_bits(bound, width);
    let reversed_out = reverse_bits(out, width);
    let reversed_propagate = reversed_out.wrapping_add(reversed_out | !reversed_bound) & all;
    let reversed_carry = reversed_propagate ^ (!reversed_bound & all);
    let carry = reverse_bits(reversed_carry, width);
    let (maintain_zero, maintain_one) = if operand_no == 0 {
        ((lhs.zero | !rhs.zero) & all, (lhs.one | !rhs.one) & all)
    } else {
        ((rhs.zero | !lhs.zero) & all, (rhs.one | !lhs.one) & all)
    };
    let possible_sum_zero = (!lhs.zero & all).wrapping_add(!rhs.zero & all).wrapping_add(u128::from(!carry_zero)) & all;
    let possible_sum_one = lhs.one.wrapping_add(rhs.one).wrapping_add(u128::from(carry_one)) & all;
    let maintain_carry = (!possible_sum_zero | maintain_zero) & (possible_sum_one | maintain_one) & all;
    (out | (carry & maintain_carry)) & all
}

impl DemandedBits {
    /// `DemandedBits::performAnalysis`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L359>
    pub fn of(
        environment: &Environment,
        function: &Function,
    ) -> Self {
        let mut found = Self::default();
        let mut worklist = Worklist::default();
        let result_width = |inst: InstId| {
            function
                .instruction(inst)
                .result
                .and_then(|result| integer_width(environment, function, Operand::Value(result)))
        };
        for (_, inst) in function.walk() {
            if !is_always_live(environment, function, inst) {
                continue;
            }
            if let Some(_width) = result_width(inst) {
                if found.alive_bits.get(&inst).is_none() {
                    found.alive_bits.insert(inst, 0);
                    worklist.insert(inst);
                }
                continue;
            }
            for &operand in &function.instruction(inst).operands {
                let Operand::Value(value) = operand else { continue };
                let ValueDef::Instruction(def) = function.value(value).def else { continue };
                match result_width(def) {
                    Some(width) => {
                        found.alive_bits.insert(def, mask(width));
                    }
                    None => {
                        found.visited.insert(def);
                    }
                }
                worklist.insert(def);
            }
        }
        while let Some(user) = worklist.pop() {
            let instruction = function.instruction(user);
            let out_width = result_width(user);
            let mut out = 0;
            let mut input_is_known_dead = false;
            if out_width.is_some() {
                out = found.alive_bits.get(&user).copied().unwrap_or(0);
                input_is_known_dead = out == 0 && !is_always_live(environment, function, user);
            }
            let mut cache = KnownCache::default();
            for (operand_no, &operand) in instruction.operands.iter().enumerate() {
                let (def, argument) = match operand {
                    Operand::Value(value) => match function.value(value).def {
                        ValueDef::Instruction(def) => (Some(def), false),
                        ValueDef::Argument(_) => (None, true),
                    },
                    _ => (None, false),
                };
                if def.is_none() && !argument {
                    continue;
                }
                let at = Use { user, index: operand_no as u32 };
                match integer_width(environment, function, operand) {
                    Some(width) => {
                        let live = if input_is_known_dead {
                            0
                        } else {
                            let live = determine_live_operand_bits(
                                environment,
                                function,
                                user,
                                operand_no,
                                out,
                                out_width.unwrap_or(1),
                                width,
                                &mut cache,
                            );
                            if live == 0 {
                                found.dead_uses.insert(at);
                            } else {
                                found.dead_uses.remove(&at);
                            }
                            live
                        };
                        if let Some(def) = def {
                            let held = found.alive_bits.get(&def).copied();
                            let grown = live | held.unwrap_or(0);
                            if held.is_none() || grown != held.unwrap_or(0) {
                                found.alive_bits.insert(def, grown);
                                worklist.insert(def);
                            }
                        }
                    }
                    None => {
                        if let Some(def) = def
                            && found.visited.insert(def)
                        {
                            worklist.insert(def);
                        }
                    }
                }
            }
        }
        found
    }

    /// The bits of `inst`'s result that are read; all of them where nothing
    /// said.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L472>
    pub fn demanded_bits(
        &self,
        environment: &Environment,
        function: &Function,
        inst: InstId,
    ) -> u128 {
        if let Some(&bits) = self.alive_bits.get(&inst) {
            return bits;
        }
        let width = function
            .instruction(inst)
            .result
            .and_then(|result| integer_width(environment, function, Operand::Value(result)))
            .unwrap_or(1);
        mask(width)
    }

    /// The bits of the operand `at` names that its user reads; none for a
    /// non-integer.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L483>
    pub fn demanded_bits_of_use(
        &self,
        environment: &Environment,
        function: &Function,
        at: Use,
    ) -> Option<u128> {
        let operand = function.instruction(at.user).operands[at.index as usize];
        let width = integer_width(environment, function, operand)?;
        if self.is_use_dead(environment, function, at) {
            return Some(0);
        }
        let user = function.instruction(at.user);
        let (out, out_width) = match user.result.and_then(|r| integer_width(environment, function, Operand::Value(r))) {
            Some(out_width) => (self.demanded_bits(environment, function, at.user), out_width),
            None => (0, 1),
        };
        Some(determine_live_operand_bits(
            environment,
            function,
            at.user,
            at.index as usize,
            out,
            out_width,
            width,
            &mut KnownCache::default(),
        ))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L510>
    pub fn is_instruction_dead(
        &self,
        environment: &Environment,
        function: &Function,
        inst: InstId,
    ) -> bool {
        !self.visited.contains(&inst)
            && !self.alive_bits.contains_key(&inst)
            && !is_always_live(environment, function, inst)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/DemandedBits.cpp#L516>
    pub fn is_use_dead(
        &self,
        environment: &Environment,
        function: &Function,
        at: Use,
    ) -> bool {
        let operand = function.instruction(at.user).operands[at.index as usize];
        if integer_width(environment, function, operand).is_none() {
            return false;
        }
        if is_always_live(environment, function, at.user) {
            return false;
        }
        if self.dead_uses.contains(&at) {
            return true;
        }
        let user = function.instruction(at.user);
        user.result.and_then(|r| integer_width(environment, function, Operand::Value(r))).is_some()
            && self.alive_bits.get(&at.user) == Some(&0)
    }
}

/// `SmallSetVector`: last in, first out, and an element already waiting is not
/// put in twice.
#[derive(Default)]
struct Worklist {
    stack: Vec<InstId>,
    waiting: IdSet<InstId>,
}

impl Worklist {
    fn insert(
        &mut self,
        inst: InstId,
    ) {
        if self.waiting.insert(inst) {
            self.stack.push(inst);
        }
    }

    fn pop(&mut self) -> Option<InstId> {
        let inst = self.stack.pop()?;
        self.waiting.remove(&inst);
        Some(inst)
    }
}

#[cfg(test)]
#[path = "demandedbits_tests.rs"]
mod tests;
