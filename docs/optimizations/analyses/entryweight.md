> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/entryweight.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/entryweight.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/entryweight.rs sha256:7d4ec7a031897d134933a71877e82a5853ef7522d4d928012c666540359a1e6c -->

# entryweight

How often a function runs: the frequency of a block over its function's
entry (`BlockFrequencyInfo` over `BranchProbabilityInfo`, as the inliner's
`GetBFI` has it) and, from the direct calls to a function, how many times it
runs per run of the program.

## Example
A local function called from a block four times as frequent as its caller's
entry, the caller running once, runs about four times.

```text
define internal void @f()      ; only caller: @main, runs once
@main: loop body (freq 4 x entry) -> call @f
entry_weight(@f) = 4 * 1 = 4.0   ; an outside caller gives at least 1
```

## Used by
Named by `inlinecost`, `walk`.
