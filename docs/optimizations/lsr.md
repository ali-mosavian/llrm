# Loop strength reduction

`lsr` chooses a loop's induction variables once, by cost, after the other
loop passes. It is `crates/opt/llrm-transforms/src/lsr.rs`.

Every read of a counter, or of a value affine in one, is one recurrence:
`pointer + start + step * trip`, with invariant symbols allowed in `start`
and `step`. `induction::users` finds them, pointer walks included, and
says how each is read: as an address, compared with an invariant, or
otherwise.

A candidate is a recurrence the loop could carry in a register: each use's
own, with its start's symbols split every way between counter and base, the
loop's counters, one per scale of each address form with a wider index, and
each step counted to zero at the exit. A use is realized from a candidate
`r` as `base + k * r + c`. Its price is the target's: the address form that
takes it, or the shift, multiply and add that compute it. A value only read
after the loop may instead come from the trip the candidate counted, or from
the known count. An equality with an invariant takes the candidate itself
and moves the bias into the invariant. The exit either keeps its compare or
tests one candidate for its last value, which costs nothing where that is
zero.

A set of candidates costs an add a trip each, its uses' prices, the work of
building starts and invariants before the loop, and a spill for each
register past what the target holds. Pressure is counted before each
instruction: the loop's own values, a product while it is read, and the
counters and invariants throughout. The search adds, removes and swaps
candidates from the loop's own counters and from nothing, and keeps the
cheaper set, fewer counters on a tie. The loop changes only where the set
beats the counters it has.

A symbolic count holds only once the loop is entered, so a loop tested for
its last value behind a symbolic count is guarded and entered at its body.

## What it replaced

`strength`, `ivshare`'s twin and offset counters, `affine`'s address
spelling, `indexing`, count-to-zero and `simplified` in `indvars`, and
`rotate`'s countdown. Each covered one relation between counters; each is a
fit or a candidate here. Exit values, exit sinking and `rewound` stay, as
IndVarSimplify's, in `indvars::IndVars`.

## From LLVM, GCC and Open Watcom

- LLVM's LoopStrengthReduce: the use kinds (address, compare with zero,
  other), the formula `base + scale * reg + offset`, reassociating a start's
  terms into the base, and running after the loop passes. Not taken: its
  exhaustive search with pruning, which our loops, a few uses each, do not
  need; and its ordering of costs registers first.
- GCC's ivopts: the candidates from each use with its base stripped, the
  set search that extends, narrows and swaps from two starts, and pricing a
  set with register pressure against the target's registers. Its address
  costs are probed per mode; ours are the target's `AddressForm`s.
- Open Watcom's loopopts: rewriting the exit to an equality against the
  counter's final value, and folding a constant difference into the
  displacement. It prices nothing: every derived variable is reduced, which
  on six registers spills. Its final value needs a constant start; here a
  symbolic count is guarded instead.

Where LLVM and GCC disagree, costs are summed as GCC does, with fewer
counters breaking a tie. Pricing each register over instructions made
`matrix` 1837 → 3017 instructions and `PARTICLE` worse.

## Limits

A loop with two exits has no count, so Nib's `zip` loop keeps its two
compares. A counter stepping by more than one has no symbolic count either.
Past the target's registers the pressure estimate is coarse: a few programs
trade instructions for memory operands.
