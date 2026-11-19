> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpmath.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpmath.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpmath.rs sha256:2c20c1fca07170abfd5073ea420fab28af7b6c6599c1c4831390667aff72e6d0 -->

# cgpmath

CodeGenPrepare's overflow-math combines: a compare that tests a sum or a
difference for wrapping, beside that sum or difference, becomes the
`llvm.uadd.with.overflow` / `llvm.usub.with.overflow` call that yields both
([CodeGenPrepare.cpp#L1747](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1747)).

## How it works
For each `icmp`, `combineToUAddWithOverflow` and then
`combineToUSubWithOverflow`: the first that acts ends the visit.

- add: `(a + b) u< a`, `a u> (a + b)` and the same with `b`, `~a u< b` and
  `b u> ~a`, `(a + 1) == 0`, and the edge cases `a == -1` beside `a + 1` and
  `a != 0` beside `a + -1`;
- sub: `a u< b` beside `a - b`, and `a u< C` beside `a + -C` (what the sub
  of a constant is canonicalized to), with `a u> b`, `a == 0` and `0 u< a`
  taken as `a u< 1`-style forms.

The target agrees (`Lowering::should_form_overflow_op`), and the math and
the compare share a block, or the math is the increment of a loop's counter
and the compare sits in that loop where the increment can move up to it
(`replaceMathCmpWithIntrinsic`). The call goes where the first of the two
was, and the compare and the math become its two results.

```text
 %s = add i16 %x, -32                      %o = call {i16, i1} @llvm.usub.with.overflow.i16(i16 %x, i16 32)
 %c = icmp ult i16 %x, 32          ==>     %s = extractvalue {i16, i1} %o, 0
                                           %c = extractvalue {i16, i1} %o, 1
```

MIRROR: `combineToUAddWithOverflow`, `matchUAddWithOverflowConstantEdgeCases`,
`combineToUSubWithOverflow`, `replaceMathCmpWithIntrinsic`, `matchIncrement`,
`getIVIncrement`, `isIVIncrement` and `m_UAddWithOverflow`.

## Example
`test_a_compare_with_the_add_of_its_negated_constant_is_a_usub_with_overflow`
in `cgpmath_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, after the compare sinks and before `cmpopt`'s steps.
