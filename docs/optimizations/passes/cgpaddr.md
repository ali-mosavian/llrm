> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpaddr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpaddr.rs sha256:fcfa806e8eb8e1e34d5f4d46c4a2a5001b42cc271dcf98e11a9b42a50f248552 -->

# cgpaddr

Sinks the arithmetic of an address into the block that uses it, where the
target folds it into the access: CodeGenPrepare's `optimizeMemoryInst`
([CodeGenPrepare.cpp#L5903](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L5903)).

## How it works
For a `load` or `store` the address is matched as `AddressingModeMatcher`
does: a `gep` chain read as `base + constant + scale * index` (one variable
index in all, to depth 4), each step kept only while the target's
`isLegalAddressingMode` says the whole is one access. When some
instruction of the match is in another block, the address is made again
just before the access, from the match alone (`sunkaddr`: a `gep` of the
scaled index, a `gep` of the constant), one copy for each block and
address. The `gep`s left unread go.

```text
 b1: %p = gep i32, ptr %a, i16 %i       b1: br b2
     br b2                              b2: %s = mul i16 %i, 4
 b2: store i32 0, ptr %p                    %q = gep i8, ptr %a, i16 %s
                                            store i32 0, ptr %q
```

MIRROR: the depth limit, the one-index-and-a-scale form of the match,
the legality test at each step, the rule that nothing is made when every
matched instruction is already in the access's block, and one `sunkaddr`
for each (block, address).

DEVIATE CGP1: no promoted extensions (`TypePromotionTransaction`), no
`AddressingModeCombiner` for a phi or select of addresses (`addresssink`
makes the phi one address first), no base-register-and-global form
(a global stays the base) and no matching through `ptrtoint`/`inttoptr`.
Exit: those parts translated.

## Example
The test `test_an_address_made_in_another_block_is_made_where_it_is_used`
in `cgpaddr_tests.rs` is the sketch above.

## Runs
Once, after `addresssink`, in the same gate. Off with `-fno-strength-reduce`
and at -O0.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.
