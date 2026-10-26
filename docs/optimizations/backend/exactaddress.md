> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/exactaddress.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/exactaddress.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/exactaddress.rs sha256:b23fb55a94ef4ca8cccfd0504e7d7069c3720515eab769fd9a6c48ea2a145956 -->

# exactaddress

Fold the chain computing an exact address into its 32-bit form.

## How it works
Where lowering proved a cell exact (`Mem::exact`), the 16-bit sum of its
registers equals the 32-bit sum of the leaves of the affine chain
(`affine::step`) that computed them, once those leaves' upper halves are
zero (`upperzero`). The chain then goes and the access reads the leaves
through the dword address form (the 67h prefix on a 16-bit target).

```text
  mov  si, ax                  (removed)
  shl  si, 1                   (removed)
  add  cx, S[si]        =>     add  cx, S[eax*2]       ; 67h prefix
```

The pass works in one block at a time. For each instruction that reads
one exact cell (`cell_of`: the cell is in a far, literal, segment or
external space, and the instruction reads its address registers nowhere
else), it takes the cell's registers as a sum of root, multiple and value
(`sum_of`; the allocator may name base and index in either order, so the
reaching writes decide which holds which). For each register that is part
of the cell's offset it then walks the chain backwards. Each step is
an `affine::step` (add a constant, scale, copy, add a register); it must
write only the register, set no flags that are live, carry no constraint
and leave its other input unwritten up to the last reader. Every cut of
the chain is a candidate stage:

```text
  stage 0:  readers use si           (nothing removed)
  stage 1:  readers use si*2         (shl removed, si as before it)
  stage 2:  readers use ax*2         (shl and mov removed)
```

Every reader of the register must be an exact cell and be rewritten for
the stage to count. A far cell keeps its origin as the base; only its
offset folds. A 32-bit frame base is refused where it would select the
stack segment the 16-bit cell did not. A stage is taken when its saving
is not negative: the removed instructions' cost, less each reader's extra
cost (the address form's `use_cost`, plus the partial-register stall for
each register of a narrow cell). The stage with the most saving wins,
unless the removed values are still read after the chain.

The 32-bit sum needs each leaf's upper half zero at the reader.
`upperzero::before` answers; where a leaf is written only below its upper
half inside the access's loop, one `movzx` in the loop's preheader
(`upperzero::preheaded`) makes it zero for every trip. A fold whose
requirement the analysis cannot prove over the extended body is dropped.
Folds that share an instruction are not made in the same round, and a
round folds one register of a cell; up to four rounds run, so the other
register folds next.

## Example
No honest LIR example. The LIR text (`lir_stage`, `ISEL_DUMP=1`) prints a
memory operand without its index registers and scale, so the dump of the
phase before and after it is the same text where the fold happened. Read the
unit test `test_a_copied_scaled_index_folds_into_every_symbolic_reader` in
this file: it builds the `mov si,ax; shl si,1` chain and asserts the cell's
registers and scale after the fold.

## Runs
Inside the peephole phase, after the pattern rewrites, before
`addresses`. Skipped when the body has no exact cell
(`needs::EXACT_CELL`) or when the CPU profile has no dword address form
(`Profile::dword_address_form`). No flag of its own.

## Needs
[`crate::backend::affine`](affine.md) (steps), [`crate::backend::upperzero`](upperzero.md),
[`crate::backend::liveness`](liveness.md) and [`crate::analysis::loops`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/analysis.rs).

## See also
The phase map: `crate::backend`
