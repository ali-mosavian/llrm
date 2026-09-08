# What the hoist is actually waiting on

Not a live-range split. Measured, three times, each answer replacing the
last.

## It is not the allocator refusing

76 bodies in the corpus refuse, all with the same message -- `interferes
with every register at once` -- and **none of them is one of the regressed
programs**. hotlpx, pressx, nested and matrix all colour.

## `_placement` had two bugs, and they are mine

`_placement` replaced `_insertion` when the hoist's allocator came out. It
asks the preheader for every value the run reads, and counted two kinds it
should not:

- a use that is only the previous contents of what the operation writes
  (`merges`), which is not an input
- a value an *earlier operation in the same run* defines -- a run is a
  chain, and the multiply reads the load standing in front of it

With both excluded the hoist fires again on every regressed program.

## And then the programs are wrong

hotlpx, pressx, nested, matrix and nbody miscompile on nine of twelve
configurations. The crossing value comes out of `interference()` with
**one neighbour** and keeps EAX -- which the preheader's own counter has.
A value defined in the preheader and read around the loop cannot have one
neighbour, so the graph is not seeing its range, and `colour()` has no
reason to move anything.

That is the thing to fix. The suspicion is `mir.resolved()` after the
hoist: the use inside the loop becomes a different version joined by a phi,
so the crossing value's own range is the preheader-to-header edge and
nothing more -- and whether the phi's congruence class carries the
interference is the question to answer next.

## The graph is right, and the MIR it is built on is not

Measured on hotlpx: the crossing value has one neighbour, which is the
counter, and they do not clash in the assignment. The graph is correct
about the body it is given.

The body is wrong. `mir.resolved()` re-derives SSA **per register**, and
the product and the counter both lived in ax -- so the phi at the loop
header joins them:

```
  0x0052  v1_6, v2_4 := v1_5 * C:2      the hoisted product
  0x0056    v1_7 := v1_6 + A:2          the loop reads it
  0x006b    v1_10 := v1_9 + 1           the counter
  0x006c    v1_1 := phi 0x30:v1_6, 0x56:v1_10
```

The loop reads `v1_6` directly while the phi says that place now holds the
counter. Nothing downstream can see a conflict, because in MIR there is
not one.

So the missing capability is **not a split**. It is that a value which
leaves a loop has to become a variable of its own -- which is exactly what
`_insertion` was doing when it picked a spare register, said without naming
one. Two changes, on branch `hoist-variable-rename`:

- `mir.resolved()` renames per MIR variable rather than per register.
  `Value.variable` exists for this.
- `hoisted()` gives each crossing value a fresh variable, so the
  re-derivation cannot join it to whatever else lived in the same place. A
  fresh variable has no origin, so the allocator places it freely.

487 of 487 rebuild, 640,829 bytes against 641,386, and lngmix comes back.
**hotlpx, matrix, nested, pressx and nbody are still wrong**, so the branch
is not shippable and main does not carry it. What is left is to find why
those five still disagree with a body whose SSA now separates the two.

## Done, and what the four remaining xfails are

The rename landed on main: hotlpx 4.9x -> 3.1x, pressx 6.4x -> 2.6x, two of
the six xfails back. What is left is not one thing.

**The copy guard.** Tried again after the rename and hotlop still printed 0
for 630 on nine of twelve configurations, so a copy leaving a loop was never
only about the register it needs. What refuses the counter's own initialiser
has to be found before that guard can be relaxed.

**press and lngmix get no run at all.** `_invariant_run` refuses any loop
holding a runtime call, and on the object as BC wrote it lngmix's loop holds
two -- `B$DVI4` and `B$RMI4`, the divide and the remainder. After the machine
arm absorbs them the loop is call-free and holds two `div` operations:

```
  load copy convert div join arg arg arg arg store store result result
  convert div copy add add store join load add store sub branch
```

Both are invariant and both compute the same thing from the same operands,
and x86's `idiv` yields quotient and remainder from one instruction. So
lngmix is a **cse** program first and a **licm** program second -- 1,844
against a target of 210 -- and neither is about registers.

**nested and matrix do hoist** and are still worse than they were: a run is
found, a crossing value is found, a placement is found. Whatever costs them
is downstream of the hoist and has not been looked at.

## And the allocator it was refused by is not the one that spills

Measured, and it corrects a detour this took twice. `regalloc.colour()`
refuses rather than spills, and it is the *fallback*: `wholeseg.rebuilt`
runs the LIR route first -- `flow.machine`, with `allocate.py`'s priced
branch-and-bound, `spiller.py`, `splitkit.py`, `frame.py` and
`prologue.py` -- and only drops to `layout.allocated` when that route
declines. hotlop, spill and nested go through it today.

lngmix and lngmxx did not, for one reason: `objwrite._carried` read "owns
no original bytes" as "inserted" and stripped the node off the half an
absorbed divide hands back. The node is what says which idiom it is, so
it reached the general encoder as an operation named `restore` with no
operands. Every body holding an absorbed divide left the route that can
spill and took the one that cannot.

With that fixed the next refusal is `0x0071: mov is not one select.py can
emit` -- the reused divide's copy arrives at emission still naming
`Held(21)` and `Held(19)` while the assignment objwrite hands down is
empty, so `allocate.applied()` did not rewrite it. That is where the
hoisted body's five crossings get priced and split, and nothing before it
needs a second spiller written into `regalloc.colour`.
