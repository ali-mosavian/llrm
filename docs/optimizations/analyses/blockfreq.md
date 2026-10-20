> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/blockfreq.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/blockfreq.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/blockfreq.rs sha256:b63a5061423a964412f3a5cd514d44854fd3e43036ae9469e29f178ba27942c2 -->

# blockfreq

`BlockFrequencyInfo` of a MIR function: LLVM's `BlockFrequencyInfoImpl`
(`llrm_mir::blockfrequency`) over `BranchProbabilityInfo` (`crate::bpi`),
as `BlockFrequencyInfoWrapperPass` runs it. `trip_adjusted` and
`of_probabilities` serve llrm's own estimates, which let a loop's proven
trips set its exit (`profit`).

## Used by
`inlinecost` (cold and hot call sites, D1), `profit`.

## Example
A diamond whose branch weighs 3 to 1 gives the arms 0.75 and 0.25 of the
entry's frequency, and the join the entry's:

```text
entry: br i1 %c, label %a, label %b   ; !{branch_weights, 3, 1}
; entry 1.0, a 0.75, b 0.25, join 1.0
```
