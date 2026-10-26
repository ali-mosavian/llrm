> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/trunc_inst_combine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/trunc_inst_combine.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/trunc_inst_combine.rs sha256:6368792b95049e84cf8e1d2f60b75ac525aafa4ca314b3b1094d527aace58176 -->

# trunc_inst_combine

An expression graph that ends in a `trunc` is evaluated at the narrow
width, as LLVM's `TruncInstCombine` (a part of `AggressiveInstCombine`).

## How it works
A translation of LLVM `d1106deb71cc` `TruncInstCombine.cpp`, each function
linking the lines it translates. For each `trunc` of the function, in
reverse order of the walk that found them, `build_trunc_expression_graph`
collects the instructions it reads, as long as they are `add`, `sub`,
`mul`, the bit operations, shifts, `udiv`, `urem`, `select`, `phi`, `umin`
or `umax`, bounded by `trunc`, `zext`, `sext` and constants.
`get_min_bit_width` finds the narrowest legal width every one of them can be
evaluated at, from the shift amounts and the known bits;
`reduce_expression_graph` then makes the graph again at that width, replaces
the `trunc` and erases the old instructions.

```text
  before                          after
  %a = zext i8 %x to i32          %c = add i8 %x, %y
  %b = zext i8 %y to i32          (the trunc's uses take %c)
  %s = add i32 %a, %b
  %c = trunc i32 %s to i8
```

DEVIATE (anything MIR or Rust lacks), numbered after `float2int`'s:
34. MIR has no lane operations, so `insertelement`, `extractelement` and
    `shufflevector` are not in the graph, and a `trunc` of a vector is not
    tried (its width rules are the scalar's only when no vector is made).
35. `IRBuilder` folds an operation on constants; MIR has none (as DEVIATE 30
    of `float2int`): an operation of two constants is made as an
    instruction. An integer wider than 128 bits is not tried (`KnownBits` is
    128 bits wide).
36. `computeKnownBits(V)` is asked with the data layout alone: no assumption
    cache and no context instruction.
37. A `umin`/`umax` made here is declared through `Declared`, which the
    module has after the pass: a later `trunc` of the same run that reads it
    sees an unknown call and leaves the graph alone.

## Example
The graph above is one `add i8`; see `trunc_inst_combine_tests.rs`.

## Runs
As `aggressive-instcombine@678`, after `instcombine` and before `tailrec`
(`PassBuilderPipelines.cpp#L678`); not at -O1, whose simplification pipeline
has none. The folds of `foldUnusualPatterns` are not translated yet: the
slot is Partial.

## Needs
The dominator tree, to leave unreachable blocks alone.

## Keeps
The dominators, loops and shape: the CFG is not touched.
