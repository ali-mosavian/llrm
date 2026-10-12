> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/phiopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/phiopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/phiopt.rs sha256:43a95a80f29634421170346f18bcd7986d43e6eddefb93f3417ee5d8ca0f79e8 -->

# phiopt

A phi of two constants that a branch chooses between becomes arithmetic of
the branch's condition: gcc's `pass_phiopt` (`conditional_replacement`,
`two_value_replacement`, and match.pd's `cond ? c : 0` as `-cond & c`
through `match_simplify_replacement`, tree-ssa-phiopt.cc) and LLVM's
`FoldTwoEntryPHINode` (SimplifyCFG). gcc runs it at -O1 and above
(`-fssa-phiopt`, opts.cc:609, passes.def:96, :230, :252, :354).

`br c, T, F` where each of `T` and `F` is empty or the other side is the
join `J`, and `J`'s phis each take a constant from either side. Each phi is
the form that costs the least on the target, against the branch it
replaces (`OperationCosts`: the branch, a jump from the arm that is not the
fall-through, a copy per phi, against the condition held as a value (`set`)
once and an extend and the operations per phi):
- `F + zext c` where the constants differ by one, `F + sext c` by minus one;
- `(zext c << k) + F` where they differ by a power of two;
- `zext c * (T - F) + F` where the target prices that multiply low;
- `(sext c & (T - F)) + F` otherwise.
There is no conditional move on a 486, so a select is a branch anyway (the
backend's `selects`); this is what the branch is worth when it is not one.
LLVM's `two-entry-phi-node-folding-threshold` (4) is the same test with
its own unit.
