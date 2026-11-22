> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopsplit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopsplit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopsplit.rs sha256:01780c168172f79b49c72e82ffe24eda5d5ff12c250155c599bb0a3d2fa56c93 -->

# loopsplit

`loopsplit`: a loop whose branch tests a counter against an invariant runs
as two loops, one on each side of the border.

Pass: GCC's `-fsplit-loops`; the pin has no such pass (DEVIATE LSPLIT,
`Tti::loop_split`, off unless the target asks).

Translation: `split_loop` and its helpers (`split_at_bb_p`,
`patch_loop_exit`, `connect_loops`, `connect_loop_phis`,
`compute_new_first_bound`) in `gcc/tree-ssa-loop-split.cc` of GCC 15, not
`split_loop_on_cond`. `loop_version` is the copy of the loop beside the
original (`loopclone`), with the guard's own branch forced one way in each.

General solution: the first loop keeps going while the counter stays on the
guard's first side of the border and the original exit would also stay;
the second starts from the first's exit values. Both are reached from the
preheader test of the guard on the counter's first value.

Not ported from `split_loop`: a guard that is not a header phi (GCC makes
the phi), `==` and `!=` against the first value or a range bound, a border
or bound computed inside the loop, `<=` and `>=` exits, a stride other than
one on a `!=` exit. Beyond GCC: the guard may be a compare inside a logical
and/or (the mid-end folds nested ifs into a `select`), and a `<` or `>` exit
must be entry-guarded (`beg < end`) so the new bound cannot wrap.

## Example
`for (i = 2; i < n; i++) { if (i <= 31) A; B; }` becomes
`for (i = 2; i < min (n, 32); i++) { A; B; }` then
`for (; i < n; i++) { B; }`.

## Used by
Named by `pipeline`.
