> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ree.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ree.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ree.rs sha256:0a1d3013c65c40180fcebdea3196a037a006c7d72342009ac14e3eaa0e850066 -->

# ree

Redundant extension elimination across a phi: gcc's
[`ree.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/ree.cc)
(`pass_ree`), as the rewrite of MIR before isel.

## How it works
`E = sext/zext P` where `P = phi [c, L, ..]` and every arm is a constant or
a load: the extension is made on each arm instead (`ext c` folds; `ext L`
follows the load, where `extload` makes one extending load of the pair), and
`E` is the phi of the wide arms. A loop-carried `short` that is loaded
narrow and extended again at the top of every trip (`movzx ebx, word [m];
...; movsx ebx, bx`) is loaded once, sign-extending.

Only a phi that nothing but extensions of one kind and type read is changed,
so the narrow value does not live beside the wide one; an arm that is
anything else (a call, an add) leaves the phi alone: gcc's `ree` also
requires every reaching definition to take the extension
(`combine_reaching_defs`).

| # | DEVIATE |
|---|---|
| D1 | Done on MIR before isel, not on RTL after it: the arms are loads and constants, which isel selects as one extending load and an immediate. Owner llrm-target-x86-m64-b; exit 2026-11-30. |

## Example
Before:

```mir
b1:
  %p = phi i16 [ 1, %b0 ], [ %v, %b1 ]
  %e = sext i16 %p to i32
  ...
  %v = load i16, ptr %q
```

After:

```mir
b1:
  %e = phi i32 [ 1, %b0 ], [ %v.ext, %b1 ]
  ...
  %v = load i16, ptr %q
  %v.ext = sext i16 %v to i32
```

From unit test `test_a_loop_carried_loaded_short_is_extended_once` in
`ree_tests.rs`.

## Runs
Once per body, after `cgpaddr` and before `extload`. With `wanted("ree")`.

## Needs
Nothing.

## Keeps
Blocks, dominators and loops.

## See also
Tests: `ree_tests.rs`. Levels: `docs/reference/levels.md`.
