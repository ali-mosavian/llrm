> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dropunnecessaryassumes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dropunnecessaryassumes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dropunnecessaryassumes.rs sha256:0d00e3d1be5b7a40b0d8e783de7387372358554990e8cffb981139f7a5ddcecf -->

# dropunnecessaryassumes

LLVM's `DropUnnecessaryAssumes` (`drop-unnecessary-assumes`;
DropUnnecessaryAssumes.cpp at `d1106deb71cc`): an `llvm.assume` whose
facts nothing else uses is deleted, with the condition only it read.

## How it works
For each `llvm.assume` in layout order, `findValuesAffectedByCondition`
lists the values its condition says something about. If every transitive
user of those values is an assume or a side-effect-free, non-terminator
instruction (the values are ephemeral), the assume informs no code and goes,
then the condition and whatever only it read
(`RecursivelyDeleteTriviallyDeadInstructions`). The walk reads at most 32
users and gives up past that; an affected global keeps the assume, as its
use list reaches other functions. A `type_test` or `public_type_test`
condition is never dropped.

## Example
`%x = add i16 %a, 1; %c = icmp ult i16 %x, 10; assume(%c); ret i16 %a`
becomes `ret i16 %a`: `%x` is read only by `%c`, which only the assume
reads.

## Runs
Twice in the pin, in `drop-unnecessary-assumes@1577` (the start of the
optimizer function passes) and `drop-unnecessary-assumes@1360` (after the
vectorizer, `DropDereferenceable`). Late, because inlining and CSE add
users.

## Limits
O(n) in the users of the affected values (at most 32 per assume).

DEVIATE 1: operand-bundle assumes are not translated (the IR has no operand
bundles), so `DropDereferenceable` has no effect. Exit 2026-11-14.
