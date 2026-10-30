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
| `gep` on a far pointer | `build_pair(selector, add(offset, index))`: 16-bit wrap, selector kept |
| `gep` on a huge pointer | `llrm.ia16.huge.add` open question 3 |
| `addrspacecast` far to huge, huge to far | no node: same two words |
| `addrspacecast` far to near | `truncate` (offset) |
| `addrspacecast` far to bare segment | `extract_element` (selector) |
| `addrspacecast` near to far | `build_pair(selector, offset)`, selector from open question 1 |
| `inttoptr`/`ptrtoint` i32 | `build_pair`/`bitcast` of the halves |
| far null / constants | `Constant` of type `far` (selector << 16 or offset) |
| `llrm.ia16.window.p1.p3` | node `X86ISD::LLRM_WINDOW`, one operand, result `far` (as the asm shape: symbol = name) |

## Open questions

1. The selector of a near pointer: a global's is the DGROUP segment, a frame object's is `SS`. A node for "the segment of symbol S" (`TargetGlobalAddress` with a `segment` flag, as the asm fixup kinds `segment`/`offset`) and `CopyFromReg $ss`?
2. A far global (`@_a` in addrspace 1): its address as `build_pair(TargetGlobalAddress segment, TargetGlobalAddress offset)`.
3. A huge `gep` normalizes by the machine's stride (`Machine::huge_shift`): one node `X86ISD::LLRM_HUGE_ADD sel:off, delta` returning `far`, expanded by sdag-b?
