//! The cheapest shift, add and subtract chain for a multiply by a constant,
//! priced by the m68k tables.
//!
//! DEVIATE G2 (owner llrm-target-m68k, exit 2026-12-20): the pin's combine
//! decomposes `(2^n +- 1) << k` alone, so `x * 1000` stays a `muls.l`. This is
//! Bernstein's search ("Multiplication by Integer Constants", Software:
//! Practice and Experience 16(7), 1986), written from the algorithm: a
//! constant is reached from a smaller one by a shift, by a factor `2^m +- 1`,
//! by an add or subtract of the multiplicand, or by a power of two plus or
//! minus a smaller constant. The data registers are two-address, so a value
//! used twice costs a copy: each step carries the instructions it needs. The
//! level's [`Price`] picks the cheapest chain and then compares it with the
//! multiply.
//!
//! Complexity: O(32 * reached constants) per constant, memoized per call and
//! capped at [`BUDGET`] constants.

use std::collections::HashMap;

use llrm_codegen::backend::selectiondag::dag_combiner::MulPlan;

use super::hooks::Cpu;
use super::mulprice::{multiply, one, shift};
use crate::timing::Price;

/// Constants one search may reach before it gives the multiply back.
const BUDGET: usize = 4096;

type Cost = (usize, u32);

struct Search {
    cpu: Cpu,
    price: Price,
    memo: HashMap<u64, Option<(MulPlan, Cost)>>,
}

fn plus(a: Cost, b: Cost) -> Cost {
    (a.0 + b.0, a.1 + b.1)
}

impl Search {
    /// What the search minimizes: bytes then clocks at `-Os`, else clocks plus
    /// bytes at the CPU's byte price.
    fn key(&self, cost: Cost) -> (i64, i64) {
        crate::timing::order(self.cpu, self.price == Price::Size, cost.1, cost.0 as u32)
    }

    fn consider(&self, best: &mut Option<(MulPlan, Cost)>, plan: MulPlan, cost: Cost) {
        if best.as_ref().is_none_or(|(_, held)| self.key(cost) < self.key(*held)) {
            *best = Some((plan, cost));
        }
    }

    /// `x * t` in a register of its own, `x` kept.
    fn best(&mut self, t: u64) -> Option<(MulPlan, Cost)> {
        if let Some(known) = self.memo.get(&t) {
            return known.clone();
        }
        if self.memo.len() >= BUDGET {
            return None;
        }
        let found = self.search(t);
        self.memo.insert(t, found.clone());
        found
    }

    fn search(&mut self, t: u64) -> Option<(MulPlan, Cost)> {
        let cpu = self.cpu;
        let copy = one(cpu, "move.l %d0, %d1", 2);
        let arithmetic = one(cpu, "add.l %d1, %d0", 2);
        if t == 1 {
            return Some((MulPlan::Multiplicand, copy));
        }
        let mut best = None;
        if t % 2 == 0 {
            let m = t.trailing_zeros();
            let (inner, cost) = self.best(t >> m)?;
            let shifted = plus(cost, shift(cpu, m));
            return Some((MulPlan::Shl(Box::new(inner), m), shifted));
        }
        // t = u +- 1, u even: shift the odd part, then one add or subtract of x
        for (u, adding) in [(t - 1, true), (t + 1, false)] {
            if let Some((inner, cost)) = self.best(u) {
                let plan = if adding {
                    MulPlan::Add(Box::new(inner), Box::new(MulPlan::Multiplicand))
                } else {
                    MulPlan::Sub(Box::new(inner), Box::new(MulPlan::Multiplicand))
                };
                self.consider(&mut best, plan, plus(cost, arithmetic));
            }
        }
        // t = f * (2^m +- 1): copy, shift, add or subtract
        for m in 1..32u32 {
            for adding in [true, false] {
                let factor = if adding { (1u64 << m) + 1 } else { (1u64 << m) - 1 };
                if factor < 3 || factor >= t || t % factor != 0 {
                    continue;
                }
                if let Some((inner, cost)) = self.best(t / factor) {
                    let shifted = MulPlan::Shl(Box::new(inner.clone()), m);
                    let plan = if adding {
                        MulPlan::Add(Box::new(inner), Box::new(shifted))
                    } else {
                        MulPlan::Sub(Box::new(shifted), Box::new(inner))
                    };
                    self.consider(&mut best, plan, plus(plus(cost, copy), plus(shift(cpu, m), arithmetic)));
                }
            }
        }
        // t = 2^a +- s, the power of two nearest below and above
        let below = 63 - t.leading_zeros();
        for a in [below, below + 1] {
            let power = 1u64 << a;
            let (s, adding) = if power > t { (power - t, false) } else { (t - power, true) };
            if s == 0 || s >= t {
                continue;
            }
            if let Some((inner, cost)) = self.best(s) {
                let head = MulPlan::Shl(Box::new(MulPlan::Multiplicand), a);
                let plan = if adding {
                    MulPlan::Add(Box::new(head), Box::new(inner))
                } else {
                    MulPlan::Sub(Box::new(head), Box::new(inner))
                };
                self.consider(&mut best, plan, plus(plus(copy, shift(cpu, a)), plus(cost, arithmetic)));
            }
        }
        best
    }
}

/// The multiply by the library routine before the 68020: the call and the
/// routine's floor. The bytes are the least a site costs (`pea` of the constant
/// 4, `move.l x, -(%sp)` 2, `jsr (d,%pc)` 4); the `addq` that pops them is
/// shared with the next call. A chain is taken at -Os only below that.
fn library_multiply(cpu: Cpu) -> Cost {
    (10, crate::routine::mulsi3_floor(cpu) + one(cpu, "jsr (__mulsi3,%pc)", 4).1)
}

/// The cheapest chain for `x * c` in `bits` bits that beats the multiply
/// under `price`, or None.
pub fn plan(cpu: Cpu, price: Price, bits: u32, c: u128) -> Option<MulPlan> {
    if bits != 32 {
        return None;
    }
    let c = (c & u128::from(u32::MAX)) as u32;
    thread_local! {
        static KNOWN: std::cell::RefCell<HashMap<(Cpu, Price, u32), Option<MulPlan>>> = Default::default();
    }
    if let Some(known) = KNOWN.with(|known| known.borrow().get(&(cpu, price, c)).cloned()) {
        return known;
    }
    let found = searched(cpu, price, c);
    KNOWN.with(|known| known.borrow_mut().insert((cpu, price, c), found.clone()));
    found
}

/// The cheapest chain for `x * c` and its cost, whatever the multiply costs.
fn chain_of(cpu: Cpu, price: Price, c: u32) -> Option<(MulPlan, Cost)> {
    let negative = c >> 31 == 1;
    let magnitude = u64::from(if negative { c.wrapping_neg() } else { c });
    if magnitude < 2 {
        return None;
    }
    let mut search = Search { cpu, price, memo: HashMap::new() };
    let (plan, cost) = search.best(magnitude)?;
    Some(if negative { (MulPlan::Neg(Box::new(plan)), plus(cost, one(cpu, "neg.l %d0", 2))) } else { (plan, cost) })
}

/// The cost of the cheapest chain for `x * c` in 32 bits, or None when there is none.
pub fn cheapest(cpu: Cpu, price: Price, c: u128) -> Option<(usize, u32)> {
    chain_of(cpu, price, (c & u128::from(u32::MAX)) as u32).map(|(_, cost)| cost)
}

fn searched(cpu: Cpu, price: Price, c: u32) -> Option<MulPlan> {
    let (plan, cost) = chain_of(cpu, price, c)?;
    let incumbent = if cpu.at_least(Cpu::M68020) { multiply(cpu) } else { library_multiply(cpu) };
    price.beats(cpu, cost, incumbent).then_some(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(plan: &MulPlan, x: u32) -> u32 {
        match plan {
            MulPlan::Multiplicand => x,
            MulPlan::Shl(inner, by) => value(inner, x).wrapping_shl(*by),
            MulPlan::Add(a, b) => value(a, x).wrapping_add(value(b, x)),
            MulPlan::Sub(a, b) => value(a, x).wrapping_sub(value(b, x)),
            MulPlan::Neg(inner) => value(inner, x).wrapping_neg(),
        }
    }

    /// A plan that computes another product is a miscompile: every chain
    /// the search returns, over the constants a program writes and edge ones.
    #[test]
    fn every_plan_computes_its_product() {
        let mut constants: Vec<u32> = (2..=400).collect();
        constants.extend([65_537, 6_700_417, 641, 1_000_000, 0x7fff_ffff, 0xffff_fffe, 0xffff_fc18, 1_103_515_245]);
        for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040] {
            for price in [Price::Size, Price::Strict, Price::Rate] {
                for &c in &constants {
                    if let Some(plan) = plan(cpu, price, 32, u128::from(c)) {
                        for x in [1u32, 3, 0x1234_5678, 0xffff_ffff, 0x8000_0000] {
                            assert_eq!(value(&plan, x), x.wrapping_mul(c), "{cpu:?} {price:?} x*{c}");
                        }
                    }
                }
            }
        }
    }

    /// `x * 1000` was a `muls.l` at -O2 on the 68020 where gcc has a chain
    /// of three shifts and adds: 125 << 3 is not `(2^n +- 1) << k`.
    #[test]
    fn a_thousand_is_a_chain_where_clocks_are_priced() {
        assert!(plan(Cpu::M68020, Price::Rate, 32, 1000).is_some());
        assert!(plan(Cpu::M68000, Price::Rate, 32, 1000).is_some());
    }

    /// A size build keeps the 4 B multiply where the chain is longer.
    #[test]
    fn a_size_build_keeps_the_multiply_where_the_chain_is_longer() {
        assert!(plan(Cpu::M68020, Price::Size, 32, 1000).is_none());
    }
}
