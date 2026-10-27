> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simplifyindvar.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simplifyindvar.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simplifyindvar.rs sha256:7299bb43d4494e6314a91bba4d97b04f3a9f7308ffac693827422006dc7fbec1 -->

# simplifyindvar

SimplifyIndVar's comparison elimination: a comparison with an induction
variable that ScalarEvolution proves true or false is that constant.

## How it works
For each loop, innermost first, and each phi of its header that
ScalarEvolution models, `simplifyUsers` walks the users of the phi that are
in the loop: a user is queued once, with the value it was reached from; a
comparison is asked `evaluatePredicateAt` of the values of its two operands
at the comparison's own loop (`getSCEVAtScope`), at the common dominator of
its users; a user that ScalarEvolution models as a recurrence of the loop
is walked through in turn. A comparison with an answer has every use
replaced by it; `decide` and `dead` then do the rest.

Not translated, each its own step: `foldIVUser`,
`replaceIVUserWithLoopInvariant`, the `samesign` canonicalisation,
`eliminateSDiv`, `simplifyIVRemainder`, `strengthenBinaryOp`,
`eliminateTrunc` and the float-induction replacement.

```text
 loop (n <= 100):  %i = phi [0, ...], [%i.next, ...]
                   %big = icmp ult i16 %i, 200      ==> true
                   br i1 %big, ...                      br i1 true, ...
```

## Example
Before, `n <= 100` is checked on entry:

```mir
body:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %big = icmp ult i16 %i, 200
  br i1 %big, label %ok, label %crash
```

After, `%big` is `true` wherever it is read.

From unit test `test_a_compare_the_trip_count_decides_is_a_constant` in
`simplifyindvar_tests.rs`.

## Runs
Step 0 of [`crate::indvars`](indvars.md), with the widening.

## Needs
`llrm_analysis::scev` (`ScevSession`): `evaluatePredicateAt`,
`getSCEVAtScope`, trip counts and guards.

## Keeps
Blocks and edges are as they were: only the uses of compares change.

## See also
[`crate::decide`](decide.md); LLVM SimplifyIndVar
(`simplifyUsers`, `eliminateIVComparison`).
