//! The search for a constant multiplier's cheapest shift, add and `lea` chain.

use std::collections::BTreeMap;

use super::{Chain, Ops};

/// What the search has learned about each target, kept across the calls of one
/// multiplier.
#[derive(Default)]
pub(super) struct Memo(BTreeMap<i64, Known>);

/// The ways to reach an odd target, in the order they are tried.
#[derive(Clone, Copy)]
enum Way {
    Below,
    Above,
    Factor,
    BelowLea,
}

/// The outcome of the last search of a target.
enum Known {
    /// The cheapest chain, and its cost.
    Cheapest(Chain, i64),
    /// No chain costs less than this.
    NoneBelow(i64),
}

/// The cheapest chain computing `x * t` that costs strictly less than `limit`.
///
/// Branch and bound after Bernstein, "Multiplication by Integer Constants"
/// (Software: Practice and Experience 16(7), 1986). An even `t` is `t >> m`
/// shifted by its trailing zeros `m`. An odd `t` is reached from a smaller
/// target by one of: a factor step when `2^m +- 1` divides it; `t - 1` or
/// `t + 1`, each a shift of its odd part followed by an add or subtract of
/// `x`, which is a single `lea` when the shift is 1 to 3 on an add. Each
/// step's price is taken off the bound before the smaller target is searched,
/// and what is found is kept per target, so no target is searched twice for
/// the same bound. Every option shrinks the target, so the search ends.
pub(super) fn synth(
    t: i64,
    limit: i64,
    memo: &mut Memo,
    ops: &Ops,
) -> Option<(Chain, i64)> {
    if t < 1 || limit <= 0 {
        return None;
    }
    if t == 1 {
        return Some((Vec::new(), 0));
    }
    match memo.0.get(&t) {
        Some(Known::Cheapest(chain, cost)) => return (*cost < limit).then(|| (chain.clone(), *cost)),
        Some(Known::NoneBelow(bound)) if limit <= *bound => return None,
        _ => {}
    }
    #[cfg(test)]
    super::SEARCHED.with(|count| count.set(count.get() + 1));

    let mut best: Option<(Chain, i64)> = None;
    let mut bound = limit;
    let mut try_step = |from: i64, steps: &[((&'static str, i64), i64)], memo: &mut Memo, bound: &mut i64| {
        let price: i64 = steps.iter().map(|&(_, p)| p).sum();
        // The copy that seeds a chain not starting with a `lea`.
        let seed = if ops.seeded && from == 1 && !matches!(steps.first(), Some(((("lea" | "flea"), _), _))) {
            ops.mov
        } else {
            0
        };
        let price = price + seed;
        if let Some((mut chain, cost)) = synth(from, *bound - price, memo, ops) {
            chain.extend(steps.iter().map(|&(step, _)| step));
            *bound = cost + price;
            best = Some((chain, *bound));
        }
    };

    if t % 2 == 0 {
        let m = i64::from(t.trailing_zeros());
        try_step(t >> m, &[(("shl", m), (ops.shift)(m))], memo, &mut bound);
    } else {
        let odd_part = |n: i64| n >> n.trailing_zeros();
        // The largest `2^m +- 1` (m >= 2) below `t` that divides it is the only
        // factor tried.
        let factor = (2..=62)
            .rev()
            .find_map(
                |m| [true, false]
                    .into_iter()
                    .find_map(
                        |plus| {
                            let factor = (1i64 << m) + if plus { 1 } else { -1 };
                            (factor < t && t % factor == 0).then_some((m, plus, factor))
                        },
                    ),
            );
        // The neighbour with the smaller odd part goes first, the factor
        // between the two.
        let (near, far) =
            if odd_part(t + 1) < odd_part(t - 1) { (Way::Above, Way::Below) } else { (Way::Below, Way::Above) };
        let ways = [near, Way::Factor, Way::BelowLea, far];
        for way in ways {
            match way {
                Way::Factor => {
                    let Some((m, plus, factor)) = factor else { continue };
                    let step = (if plus { "fadd" } else { "fsub" }, m);
                    let price = ops.mov + (ops.shift)(m) + ops.alu;
                    if plus && m <= 3 {
                        if let Some(lea) = (ops.lea)(1 << m) {
                            try_step(t / factor, &[(("flea", 1 << m), lea)], memo, &mut bound);
                        }
                    }
                    try_step(t / factor, &[(step, price)], memo, &mut bound);
                }
                Way::Below | Way::Above | Way::BelowLea => {
                    let add = !matches!(way, Way::Above);
                    let neighbour = if add { t - 1 } else { t + 1 };
                    let m = i64::from(neighbour.trailing_zeros());
                    let from = neighbour >> m;
                    if matches!(way, Way::BelowLea) {
                        if let Some(price) = (1..=3).contains(&m).then(|| (ops.lea)(1 << m)).flatten() {
                            try_step(from, &[(("lea", 1 << m), price)], memo, &mut bound);
                        }
                    } else {
                        let name = if add { "add" } else { "sub" };
                        try_step(from, &[(("shl", m), (ops.shift)(m)), ((name, 0), ops.alu)], memo, &mut bound);
                    }
                }
            }
        }
    }

    match &best {
        Some((chain, cost)) => memo.0.insert(t, Known::Cheapest(chain.clone(), *cost)),
        None => memo.0.insert(t, Known::NoneBelow(limit)),
    };
    best
}
