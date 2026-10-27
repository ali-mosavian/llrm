> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/comparefold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/comparefold.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/comparefold.rs sha256:ab78f22c3a638f58a1b11c76dddf891128d7361038a20afe213f13fdc611632f -->

# comparefold

Fold a one-use load into the comparison that reads it, as a memory operand.

## How it works
Isel emits `cmp` on a loaded value as a load plus a register compare. When
the loaded value has no other reader, the compare can read the cell
itself and the load, with its virtual register, goes away before
allocation takes a register for it.

```text
  mov  v3, [bp-4]                  nop            (anchor, no defs/uses)
  cmp  v3, v4              =>      cmp  [bp-4], v4
  jl   L                           jl   L
```

The load is a candidate when it is a plain `mov` of a register from
memory, defines exactly its value, the value has one use
and is not exposed, and the load carries no register constraint, clobber,
group or spill marker. A load made again where it is read counts as plain.
The compare is the next instruction after any anchors; it must be a
two-source `cmp` that names the value once and whose other operand is not
memory and has the same width. A compare cannot take two memory operands.

A `movzx` or `movsx` from a narrower cell folds only for an equality
test against zero (the compare is followed by `je` or `jne`). The compare
is then done at the cell's width. Narrowing preserves ZF, but a signed
condition would observe the narrow cell's top bit through SF.

The load is replaced by an anchor, keeping source ownership, so the
instruction numbering stays. The folded compare reads the cell's address
values. The pair is left alone when both instructions own a relocation,
since LIR names one owner per instruction.

## Example
No honest example: the fold runs inside selection, no dump holds the body
before it, and the unit tests build their fixtures in Rust. `comparefold.rs`
`test_one_use_memory_comparison_is_selected_before_allocation` asserts the
result: the load becomes an anchor and the `cmp` reads the cell.

## Runs
Per block, inside isel's cross-instruction selection
(`isel/combined.rs`), after [`crate::backend::farload`](farload.md) and before
`stepflags`. Every optimization level; there is no flag.

## See also
The phase map: `crate::backend`
