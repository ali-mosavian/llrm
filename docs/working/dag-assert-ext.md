# DAG: `AssertZext` / `AssertSext` for extended arguments and results

Pin d1106deb71cc. Owner w9. Draft until the steps below land. One PR.

## Gap

`unsigned short f(unsigned short x) { return x / 10; }` at m64: the pin multiplies `edi`, because the argument is `zeroext`; ours `movzx`s it first (`dag-deviate-audit-2026-10-28.md`, +1 instruction per use of a narrow parameter). Cause: `getCopyFromParts` asserts the register holds the value already extended; we truncate without saying so (DEVIATE SD16, SD211).

## Pin

| Piece | Pin |
|---|---|
| Formal argument with `zeroext`/`signext` | `LowerArguments` picks `AssertOp`, [SelectionDAGBuilder.cpp L12440][args]; `getCopyFromParts` wraps the register-width part in it before the truncate, [L337][parts] |
| Call result with `zeroext`/`signext` | [L11902][ret] |
| Known bits | `computeKnownBits` [SelectionDAG.cpp L4160][kb]: bits at and above the type are zero (`AssertZext`) |
| Sign bits | `ComputeNumSignBits` [L4989][sb]: `bits - type + 1` (`AssertSext`), `bits - type` (`AssertZext`) |
| Selection | dropped for its operand, [SelectionDAGISel.cpp L3387][sel] |

[args]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L12440
[parts]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L337
[ret]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L11902
[kb]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L4160
[sb]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L4989
[sel]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3387

## Here

The combiner already turns `(zext (trunc x))` into `(and x, mask)` and `visitAND` drops a mask `known_bits` proves redundant. So the work is the node and its facts, not new folds.

1. **Node.** `ISD::AssertZext` / `AssertSext` (operands: value, `ValueType`), dump name, adapter id.
2. **Facts.** `known_bits`: zero above the type for `AssertZext`. `num_sign_bits`: as the pin.
3. **Selection.** Replaced by its operand before matching (the pin does it inside `SelectCodeCommon`; DEVIATE SD43: ahead of the matcher, same effect).
4. **Builder.** `LowerFormalArguments`: a one-part integer argument narrower than its register, with `zeroext`/`signext`, is asserted then truncated. Calls already extend by the call-site attributes.
5. **Results.** A call result with `zeroext`/`signext` (step 4's twin), if a listing shows a use.

Not here: `LiveOutRegInfo` asserts across blocks (SD16), the `AssertZext`-of-load and `visitAssertExt` combines (SD211).

## Risk

The assertion is a contract with the caller: a caller that does not extend breaks the callee. Callers built by this compiler extend by the same attributes. Foreign callers are as in the pin.

## Tests

Step 2: unit test on `known_bits`. Step 4: `unsigned short` divide at m64 has no `movzx`; a `signext` `char` compared with zero has no `movsx`. Each fails first.
