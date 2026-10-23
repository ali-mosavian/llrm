# Far and huge pointers in the DAG builder (sdag-a)

7919 m16 blocks stop at "pointer in another address space". Proposal, for sdag-b (legalizer) and sdag-c (matcher) to confirm; builder date 11-24, sdag-b's far ops 11-22.

## Address spaces (isel.rs `spaces`)

0 near (`i16`), 1 far, 3 huge (both two words, `selector:offset`), 2 a bare segment (one word). Frame objects are in SS, globals in DGROUP.

## Types

`ptr addrspace(1|3)` is `MVT::Far` (SD2). `ptr addrspace(2)` is `i16`.

## Forms

| IR | DAG |
|---|---|
| `load`/`store` through a far pointer | ordinary `load`/`store` whose pointer operand has type `far` |
| `icmp` on far pointers | `setcc` on `far` (SD2 expands to both halves) |
| `phi`, `select`, arguments, results | `far` virtual registers (SD2 `CopyToReg`/`CopyFromReg`) |
| `gep` on a far pointer | `build_pair(add(offset, index), selector)`: 16-bit wrap, selector kept |
| `gep` on a huge pointer | `X86ISD::HUGE_ADD far, i32 delta` -> far (delta: the whole byte displacement, sign-extended and scaled) |
| `addrspacecast` far to huge, huge to far | no node: same two words |
| `addrspacecast` far to near | `truncate` (offset) |
| `addrspacecast` far to bare segment | `extract_element` (selector) |
| `addrspacecast` near to far | `build_pair(offset, selector)`, selector `CopyFromReg $ss` for a frame object, else the DGROUP segment of the symbol |
| `inttoptr`/`ptrtoint` i32 | `build_pair(offset, selector)` / `extract_element` 0, 1 |
| far null / constants | `Constant` of type `far` (selector << 16 or offset) |
| `llrm.ia16.window.p1.p3` | `X86ISD::LLRM_WINDOW far -> far` (a delta-0 `HUGE_ADD`; sdag-b expands it once emitted) |

## Settled with sdag-b

A far is `build_pair(offset, selector)`: `extract_element` 0 is the offset, 1 the selector (memory: offset at +0). `$ss` by `CopyFromReg`. A far global is a `build_pair` of two target addresses.

## Open (sdag-c)

The DGROUP selector of a global: a segment-of-symbol form needs the global node to carry a part flag; sdag-c owns the fixup and confirms the shape.
