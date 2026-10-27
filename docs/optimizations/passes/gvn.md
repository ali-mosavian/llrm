> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gvn.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gvn.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gvn.rs sha256:909bacd774fa01e9fd51dead7e55f7faf7fbbaee2589f9597d1e0ac9d6afc098 -->

# gvn

Reuses a value the function already has, so the same operation is not
computed twice and the same bytes are not read again: LLVM's GVN and gcc's
FRE/PRE. One pass owns reuse of a value, whether it came from arithmetic, a
load or a store. `Gvn` is the pass; `EqualCalls` is its call part alone.

## How it works
`Gvn` runs these steps on a body, in order.

1. The scalar pass (`pass::run_impl`, `table`: LLVM's `GVNPass` and
   `ValueTable` without the load and PRE parts). Blocks with one way in are
   merged into it, then each block is met in reverse postorder, its
   instructions simplified or numbered by opcode, type and operand numbers
   (a commutative pair ordered, a compare's predicate swapped with its
   operands, a GEP by the bytes it adds, flags in no key): one that a
   dominating leader has is replaced, keeping the flags both have. In what
   an edge of a branch, a switch case or an assume dominates, the condition
   is that edge's constant (`propagateEquality`), and the arm of a branch on
   a constant is dead: its join's phis read poison from it. Two calls with
   the same callee and arguments that read and write no memory are one.
2. Loads (`pass::loads`: `processLoad`, `processNonLocalLoad`). Memory
   dependence (`llrm_analysis::memdep`) names what a load depends on in its
   block, or in every block that reaches it. A load a store or an earlier
   load of the same bytes serves becomes that value, cut or widened to its
   type (`vncoercion`); where several blocks serve it, a phi (`SsaUpdater`)
   merges them. A load with an unavailable block is left for step 4. Equal
   calls that touch no memory (`transform::equal_calls`) are one after it.
3. Load PRE (`pass::loadpre`: `performLoadPRE`, `performLoopLoadPRE`). With
   `pre`, a load that is available on all but one of the ways into its block
   is loaded on that way too (an edge into the block with several ways out
   is split), and becomes a phi; a loop header's load of an address the loop
   does not change is loaded in the preheader and after the one clobber.
4. Scalar PRE (`pass::scalarpre`: `performPRE`). With `pre`, an expression
   that all but one predecessor of a join computes (none of them over a back
   edge) is computed in that one too, before its terminator, and a phi takes
   its place; an edge with several ways out is split first and the pass
   tries again. Compares, GEPs, loads and instructions with effects are
   left; so is any that could trap and may follow an exit.

A volatile access is never numbered. Floats are numbered like integers; a
value serves a read only of its own type, so a float store reaches a float
load exactly and nothing rounds. Which floats are exact constants is
[`crate::floatfold`](../backend/floatfold.md)'s question.

What serves a load is MemDep over `BasicAA`: the nearest store, load,
allocation or `memset` in its block that may touch its bytes, and a partial
overlap is a clobber the coercion may still cut a value from. The scan stops
at `BLOCK_SCAN_LIMIT` (100) instructions.

`EqualCalls` runs the call numbering alone, before `tailrec`, so two equal
calls are one before one of them becomes the loop's next trip (EarlyCSE
precedes tail-recursion elimination in LLVM, FRE in gcc). It asks for no
analysis unless the body has two such calls.

## Example
A store's value reaches the load after it, and the load below a join takes
the value each arm stored. Before:

```mir
@g = global i16 0

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
```

After:

```mir
define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  ret i16 %x
}
```

From unit test `test_gvn_forwards_a_stored_value_to_its_load` in
`gvn_tests.rs`. Before:

```mir
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  store i16 %y, ptr @g
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
```

After:

```mir
b3:
  %r1 = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %r1
```

`b0` to `b2` are unchanged. From unit test
`test_gvn_joins_the_values_each_arm_stored` in `gvn_tests.rs`.

## Runs
`EqualCalls` runs early in the body pipeline, before `tailrec`; `Gvn` after
`dse`, before `promote`. Both are on at every optimizing level and are
turned off together by `-fno-gcse`. `Gvn`'s PRE steps (`pre`)
are on from -O2, the -O1 level being FRE, and off with `-fno-tree-pre`. Each
runs once per body pass.

## Needs
[`crate::transform`](transform.md), `memdep`, `BasicAA`, `LoopInfo`.

## Keeps
Dominators and loops, unless a join split an edge.

## See also
Tests: `gvn_tests.rs`, `gvn_load_tests.rs`. Levels:
`docs/reference/levels.md`. LLVM `GVN.cpp`; gcc `tree-ssa-pre.cc`.
