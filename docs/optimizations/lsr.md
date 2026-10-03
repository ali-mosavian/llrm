# Loop strength reduction

`lsr` chooses a loop's induction variables once, by cost, after the other
loop passes. It is `crates/opt/llrm-transforms/src/lsr.rs`.

Every read of a counter, or of a value affine in one, is one recurrence:
`pointer + start + step * trip`. `start` and `step` are one form, `Scev`: a
polynomial in invariant unknowns modulo the width, as SCEV's n-ary add and
mul. A term is a coefficient times a monomial (`m*w*x`), so `i*m`, `(i+k)*m`
and `i*m*w` are recurrences whatever their start. Equal values are equal
forms: coefficients are masked, zero ones dropped, monomials sorted.
`induction::recurrences` is the one place that builds them, by one fold per
opcode; a recurrence times a recurrence (`i*i`) is not one, as it is a
second-order add recurrence nothing here uses. A product past 16 terms or
degree 4 is refused. `induction::users` adds how each is read: as an
address, compared with an invariant, or otherwise. A truncation and `c - r`
stay reads there: a candidate has one width, and the pass realizes each
site alone, where the loop computed one negation and shared it.

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
A loop tested after its trips needs no guard where a branch over its entry
already proves the first trip: `induction::guards` reads what the branches
over a block prove, as LLVM's `isLoopEntryGuardedByCond` does.

## What the pass is told

The pass prices; it does not know the machine. The facts it prices from each
have one home:

- The target (`llrm-x86-code16`) holds the registers a value may take and the
  ones a C callee keeps. `Machine::kept_across` says how many a call to a
  named callee keeps, from its ABI contract, and `Machine::multiply_by` what
  a multiply by a constant costs, from the backend's shift and add chains.
- `spill` is the one spill model: a spilled counter costs a memory update a
  trip, an invariant a load per read, a far pointer two stores, and a truth
  value only its own branch reads none. Both `lsr` and `gvn` price pressure
  through it.
- The frontend states what only it knows: an indexed borrow's address is
  `inbounds`, a borrowed slice descriptor is `noalias readonly`, and the
  runtime's panic routines end the program. The pass reads those.

## Several exits

`induction::exits` counts each exit the latch follows, and the loop's
backedges are the least of them, as LLVM's exit limits do. `exitfold` then
does, in `indvars`, what LLVM's IndVarSimplify does with them: an exit no
count can reach is never taken, an exit tested against an invariant is
tested once before the loop, and exits that only crash are tested once ahead
of it. One rule is beyond LLVM and GCC, which never merge live exits: exits
that leave to the same place with the same values, with nothing seen or
trapping between them, become one test on the least of their counts. That is
what gives Nib's `zip` of two slices the loop C's dot has.

## The constant under a scale

`gepoffset` (LLVM's SeparateConstOffsetFromGEP) runs last, after `hoist`. It
splits `gep T, p, (i + 8) * 2` into a `gep` of the constant bytes and one of
the rest, which isel folds into a displacement. The walk goes through add,
sub, a constant multiply or shift, truncation, and `sext`/`zext` only over
adds whose `nsw`/`nuw` keeps them from wrapping. The split is kept when it
frees an instruction and makes none that cost more; a sum a compare also
reads stays whole, as the address would hold one register more. The new
`gep`s are not `inbounds`.

## Huge pointers

A huge pointer's displacement carries into its selector. Built from a
counter, each use pays the whole carry (`OperationCosts::carry`); stepped by
a constant, the step pays a borrow spread to a mask, cut to the selector's
stride and added to it (`carry_step`). So LSR walks a huge array with a huge
pointer, in C, BASIC and Nib alike.

`window` runs after LSR. A counted loop whose huge pointer reaches less than
the target's window (`Machine::huge_window`: 64K less 15 bytes on DOS) has
its start normalized once before the loop (`llrm.ia16.window`), and the loop
runs on a far pointer with no carry. A 2D array's rows get this; a walk past
64K keeps its huge steps.

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

A counter stepping by more than one has no symbolic count. A loop tested
on a value other than a counter plus a constant (a scaled compare, as
`bubble`'s bounds checks on `j` beside addresses on `2j`) keeps a counter for
the compare. Past the target's registers the pressure estimate is coarse: a
few programs trade instructions for memory operands. A symbolic stride
(matmul's `k * dim1`) is a counter with a held step the pass prices but
seldom chooses under pressure, where main's strength pass walked a pointer.
Choosing which value is the base and which the index of a two-register
address is the backend's, and its choice decides some of these.
