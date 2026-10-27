> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/booleans.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/booleans.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/booleans.rs sha256:65780b56d68b2412aa79a86239d0cee7952344efb736b7af8865d689de19893b -->

# booleans

Setcc chains the target's cost table prices above a branch, made selects
for `selects::lowered` to branch.

## How it works
Without a conditional move a comparison used as a value is `cmp; setcc` and
its combiners are more instructions: `x + zext(c)` is `setcc; movzx; add`.
A branch does the same in a jump and the work only one arm needs. That
chain is priced both ways from the target's cost table (`setcc`, `movzx`,
`alu_rr`, `jcc`, `jcc_not_taken`, the description's `timings.times`) and
rewritten as a `select` where the branch is cheaper; the arm's instruction
(the `add 1`) moves into the taken block with the diamond.

An `and` of two compares used as a value is not branched: the pin keeps two
`setcc` and an `and` (`FindMergedConditions` short-circuits only a branch's
condition). Main branched it by price; that was STRISTMP-1.

```text
 %z = zext i1 %c to i32           %y = add %x, 1        ; moved into taken
 %r = add i32 %x, %z       ==>    %r = select %c, %y, %x    -> br %c, taken, join
```

DEVIATE B2 (no number yet), the `add` chain: the pin lowers it as a `setcc`
chain too (`pin.py llc -m 32` on x_mergesort's merge loop: 21 instructions
with the flags spilled), where main branches. The price is the one cost
table, not the shape of a loop. Exit: the pin's lowering equals or beats
main on the rows (x_mergesort, stft bq32, nhstutil). Plan: docs/working/
setcc-chain-plan.md.

Rows (own-worktree builds 4d72d84065 / 49a8d96bfb, tools/pairs, m32 i486):
x_mergesort clocks -25.6% at -O1..-O3, -25.0% -Os (code -9.8% -Os), x_rle
-7.9%, x_primes -1.4%; no row worse. QCport (tools/rows/qcport.py, m16)
-Os 260284 -> 260240 (-44 bytes); above base: host +7 (0.028%), `lights` +3
(0.708%: `sete; movzx` of a 6-byte pair became `mov cx, 1; jne` and a
second `mov`, a branch the cycle table prices cheaper that is not shorter).
Row LIGHTS-OS-1, owner llrm-w-2, 2026-11-06: the gate prices bytes too
(encoded size of the forms, from the description) so the chain stays where
it is shorter at -Os. Compare of operands wider than a register, a chain a
branch reads, and an `and` with a non-compare side are left alone (each
grew a QCport module at -Os: snd, menu, console).

## Example
`p >= lo && p < hi` as a value, on a target with no conditional move.

Schematic, from the fixture of test
`test_an_and_of_two_compares_as_a_value_is_a_branch_around_the_second`
(`selects.rs`; not printer output):
```mir
%a = icmp uge i16 %p, %lo            ; before: two setcc and an and
%b = icmp ult i16 %p, %hi
%r = and i1 %a, %b

%a = icmp uge i16 %p, %lo            ; after: the second compare only when the first holds
br i1 %a, label %taken, label %join
taken:  %b = icmp ult i16 %p, %hi ; br label %join
join:   %r = phi i1 [ %b, %taken ], [ false, %entry ]
```

From unit test
`test_an_and_of_two_compares_as_a_value_is_a_branch_around_the_second`
in `selects.rs`.

## Runs
Inside `selects::priced`, after the selects of the module were folded and
before they are branched, on a target that expands `select` of the
chain's type.
