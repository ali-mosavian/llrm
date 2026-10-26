> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/extpromotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/extpromotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/extpromotion.rs sha256:6725af11029b79dc0a3e2148f94d27d94bafdfb97f86dd27fe554e62b8821e42 -->

# extpromotion

CodeGenPrepare's extension promotion: `optimizeExt`, `tryToPromoteExts`,
`TypePromotionHelper` and `canFormExtLd`
([CodeGenPrepare.cpp#L7250](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7250)).

## How it works
An extension `ext (op a, b)` is moved up through its operand when the
operand can be promoted: `op (ext a), (ext b)` computes the same value in
the wide type. Done speculatively in a `Transaction` and followed up the
chain until an extension reaches a load, where the target folds the pair
into one extending load; if none reaches one, or the instructions made cost
more than the extension saved, the transaction is rolled back.

```text
 b1: %a = load i8, ptr %p              b0: %a = load i8, ptr %p
     %b = add nsw i8 %a, 1                 %e = sext i8 %a to i32  ; an extending load
 b2: %e = sext i8 %b to i32                %w = add nsw i32 %e, 1
```

MIRROR: `canGetThrough`, `getAction`, the two promote functions, the
cost rule (`TotalCreatedInstsCost > 1`, legality of the promoted
instruction, one new extension at most where the extension was not free),
`canFormExtLd` and `isExtLoad`, `hasSameExtUse`.

DEVIATE EXT1: `isExtLoad` asks `isLoadExtLegal` alone; the pin's
`isLoadLegal` also asks whether the access is aligned for the target, which
every x86 target allows. Exit: the alignment hook with the DAG switch,
2027-02-04.

DEVIATE EXT2: `performAddressTypePromotion` is not translated: only AArch64
and RISC-V answer `shouldConsiderAddressTypePromotion` true, x86 does not.

## Example
`test_a_sign_extended_sum_of_a_load_is_promoted_to_an_extending_load` in
`extpromotion_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, before `optimizeExtUses`, where the target enables the
promotion (`enableExtLdPromotion`).
