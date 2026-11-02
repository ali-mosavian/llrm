> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/typepromotion_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/typepromotion_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/typepromotion_pass.rs sha256:cbde2cfde68c770a2fdd40f96a32ca011bd376a1eba248279b3f7b91ecc6b00c -->

# typepromotion_pass

Opcode-based type promotion of small integers the legalizer would promote
anyway, where promoting them in the IR spares the masks the DAG puts
around a value in a cycle. Translated from
[TypePromotion.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TypePromotion.cpp)
(`TypePromotionImpl`, `IRPromoter`).

## How it works
The search starts at the operands of an unsigned `icmp` and of a scalar
`trunc` to `i1`, and at a loop's phi that a `zext` reads. From that value a
tree of instructions is grown: *sources* make narrow values (arguments,
loads, `zeroext` calls, an equal-width `trunc`), *sinks* observe them
(stores, returns, calls, compares, a `switch`, a widening `zext`). The tree
is promoted only if every instruction in it cannot wrap (`nuw`, or a
constant add or sub whose compare cannot tell: `isSafeWrap`). Sources are
zero extended once, the tree changes type in place, and each sink gets a
`trunc` back where it needs the original type.

## Example
```text
 before                            after
 %c = phi i16 [0, %a], [%n, %b]    %c = phi i32 [0, %a], [%n, %b]
 %n = add nuw nsw i16 %c, 1        %n = add nuw nsw i32 %c, 1
 %k = icmp ult i16 %c, 7           %k = icmp ult i32 %c, 7
```

## Runs
Before `cgpaddr`, as the pin's `addCodeGenPrepare` runs it, on a target
whose `Lowering::runs_type_promotion` says so (arm64).

DEVIATE TP1 (no number): `Call::hasRetAttr(ZExt)` also reads the callee's
declaration; here only the call site's attributes are read (a pass has the
function, not the module). clang states `zeroext` at both. Exit: none.

DEVIATE TP2 (no number): `getTypeToTransformTo` is one step to the next
power of two; here the next legal width above the type. The two agree for
every integer below the widest legal one on a target whose legal widths are
powers of two (arm64: 32 and 64). Exit: none.

DEVIATE TP3 (no number): `IRBuilder` folds the `and` of a constant in
`ConvertTruncs`; here only an integer constant is folded, anything else
gets the instruction. Exit: none.
