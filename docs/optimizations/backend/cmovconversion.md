> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/cmovconversion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/cmovconversion.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/cmovconversion.rs sha256:796180cc5515dc6956b9f1afd442bf7b69003e72e354a355a7ad2287fb878e02 -->

# cmovconversion

Conditional moves turned into branches where that is faster: LLVM's
`X86CmovConversion`
([X86CmovConversion.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CmovConversion.cpp)).

## How it works
A group is consecutive `cmovcc` in a block that read the same flags and
test one condition or its opposite (`collect_candidates`). Every group with
a load is converted, anywhere (`ForceMemOperand`: a `cmov` makes the load
wait for the condition). In each innermost loop the other groups are
converted when the loop's critical path shrinks enough and every `cmov`
waits longer for its condition than for its likelier value
(`profitable_groups`). A conversion is a diamond: the block ends in a
`jcc` over a block holding the loads, and the join starts with a phi per
`cmov` (`convert_group`).

```text
  b:  flags := cmp ...                  b:     flags := cmp ...
      v := cmovne f, t            =>           jne join   ; jmp skip
      rest                          skip: (loads)
                                    join:  v := phi [f, skip], [t, b]
                                           rest
```

## Example
The unit tests `a_cmov_of_a_load_in_a_loop_is_a_branch` and
`a_register_cmov_outside_a_loop_stays`.

## Runs
On SSA, after `combiner` and before `machinelicm` (the pin's `addILPOpts`),
at every -O level but -O0.

## Limits
DEVIATE CMC1 (exit 2026-11-20,
[permalink](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CmovConversion.cpp#L487)):
the latency of an instruction is the target description's row for its
form (`form_of`), not the scheduling model's; a copy takes 0, a form with
no row 1. The misprediction penalty is the target's `mispredict_penalty`;
with none stated only the load groups convert.

DEVIATE CMC2 (exit 2026-11-20,
[permalink](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CmovConversion.cpp#L342)):
the `SUBREG_TO_REG` that relies on a 32-bit `cmov` zeroing the upper half
is a reader of the result at a wider width.

Not ported: the unpredictable flag (`MIFlag::Unpredictable`, which nothing
sets here), and the debug-instruction numbers (`setDebugInstrNum`).
