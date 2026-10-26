> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/jumptableswitch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/jumptableswitch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/jumptableswitch.rs sha256:836a495d4d310f6182cf9a361b51b8812bc295f03a5b244708db0f151eaad9c3 -->

# jumptableswitch

A call through a table of functions is a switch of direct calls: LLVM's
`JumpTableToSwitch` (`jump-table-to-switch`;
[JumpTableToSwitch.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpTableToSwitch.cpp)
at `d1106deb71cc`).

## How it works
A call of `load (gep @table, 0, %i)` is a candidate when `@table` is a
constant global with its initializer (`parseJumpTable`), the GEP has one
variable index and no constant offset (`collectOffset`), the table's size
is a whole number of strides and at most 10 entries
(`jump-table-to-switch-size-threshold`), and every entry is a function with
a body, of the call's type, of at most 50 instructions
(`jump-table-to-switch-function-size-threshold`). The call's block is split
at the call; the head ends in `switch %i` with a case `call.k` for each
entry and `default.switch.case.unreachable` as its default; `call.k` holds
a copy of the call of entry `k` and jumps to the tail, whose phi (for a
call with a result) takes the copies' results.

```text
  %p = gep [4 x ptr], @t, 0, %i        switch %i, unreachable [0: call.0 ...]
  %f = load ptr, %p            ==>     call.0: %r0 = call @f0(%x); br tail
  %r = call %f(%x)                     tail:   %r = phi [%r0, call.0] ...
```

The pass sits after `cvp` in the simplify phase (PassBuilderPipelines.cpp
[L672](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L672)),
so the inliner then takes the direct calls.

DEVIATE JT-1 (no number, owner w-6): no profile data (`!prof` value
profile) and no `!prof` on the switch: llrm has none.

## Example
The unit test `a_call_through_a_table_is_a_switch_of_direct_calls`.

## Runs
Slot `jumptableswitch@672`, after `cvp@669`. The pin reads
`getInstructionCount` of each callee, which a function pass cannot see
here: the pass `wants_sizes`, and the manager counts the functions a global
initializer names (a table's entries, the only ones asked about) before the
pass runs, and again for each it changes. A program with no function in a
constant counts nothing.
