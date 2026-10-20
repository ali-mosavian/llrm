> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/predicateinfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/predicateinfo.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/predicateinfo.rs sha256:46ebd6cb7a1fd23b2d0a553f17dc56d097c3eac6bc022c9c38668bd02b769f42 -->

# predicateinfo

PredicateInfo: what a branch, switch or assume says about a value where it
holds, as a copy of the value that only the code it holds for reads. A
translation of LLVM's `PredicateInfo.cpp` at d1106deb71cc.

## How it works
For each conditional branch (and each `llvm.assume`), the values the
condition compares are candidates: an operand of an `icmp` or `fcmp`, the
condition itself, the operand of a `trunc nuw`, and what an `and` (on the
true edge) or `or` (on the false edge) of conditions joins, at most eight
per branch. A switch gives its condition a predicate for each case that is
the only edge into its target. A value with one use is not a candidate.

Copies are made for the uses a predicate dominates only. The uses of the
value and the places a copy could stand are sorted in the dominator tree's
depth-first order, and one pass with a stack of the copies in scope
replaces each use with the innermost copy; a copy is made (a no-op
`bitcast` of the value, before the branch or after the assume) when a use
it dominates is found. `PredicateInfo::predicate_info_for` tells which
copies are such, and what they say.

## Example
A use the branch dominates reads a copy that carries the predicate:

```text
 b0: %c = icmp ult i32 %x, 10           b0: %c = icmp ult i32 %x, 10
     br i1 %c, b1, b2                       %x.0 = bitcast i32 %x
 b1: use %x                                 br i1 %c, b1, b2
                                        b1: use %x.0     ; 0 <= %x.0 < 10
```

## DEVIATEs
1. No operand-bundle assumes (`PredicateBundleAssume`, the `nonnull`
   bundle): MIR has no operand bundles.
2. No printer, verifier or `DebugCounter`: the printer pass is a debugging
   aid. The copies are not removed here; the caller does
   (`replaceCreatedSSACopys` in LLVM's printer, `removeSSACopies` in SCCP).
3. A use is `(user, operand index)`; its order in `Op->uses()` (newest first
   in LLVM) only matters among uses that sort as equal, which are replaced
   independently.

## See also
- Used by: [`crate::sccp`](sccp.md)
