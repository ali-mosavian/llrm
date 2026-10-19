> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/uncondicmp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/uncondicmp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/uncondicmp.rs sha256:c6ef79c1d20c8de9a78246af50ff63dfa5abca16c12e21b9c13c8ce4e6f6d69e -->

# uncondicmp

A block that holds an equality compare of a switch's value with a constant
(and perhaps a `select` on it), and jumps on, folded into the switch:
LLVM's
[`tryToSimplifyUncondBranchWithICmpSelectInIt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5328)
and its `ICmpInIt` form (a compare alone), the parts of
`simplifyUncondBranch`
([L8800](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8800))
after the empty block's removal.

The block's only predecessor is a `switch` on the compared value. Reached
by a case, the value is that case's, and the compare folds; reached by the
default, a compare with a value some case has is known, else the compare's
constant becomes a case of its own, to a block that jumps to the join with
the value the compare chose:

```text
  switch i8 %a, label %d [ 0, %end  1, %c ]        switch i8 %a, label %d [ 0, %end  1, %c  2, %edge ]
  d:  %t = icmp eq i8 %a, 2                  ==>   d:    br %end
      %v = select i1 %t, i8 C3, i8 C4              edge: br %end
      br %end                                      end: phi [ C1, ..], [ C4, %d ], [ C3, %edge ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights: the new case carries none. |
| D2 | A block reached by two cases of the switch is left (LLVM asserts a unique case value). |
