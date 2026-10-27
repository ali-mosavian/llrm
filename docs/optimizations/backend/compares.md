> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/compares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/compares.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/compares.rs sha256:a54ebf2f3f7857011f83de42613b968146ea49ee98e4fdd5be3e03893057843a -->

# compares

Expands `llvm.scmp` and `llvm.ucmp` into two compares and a subtract in MIR
before instruction selection, for the targets whose description lists them
under `expand`.

## How it works
The three-way compare is `(a > b) - (a < b)`, each side a compare widened
to the result's integer type: -1, 0 or 1 at that width.

```text
 %r = call i8 @llvm.scmp.i8.i16(i16 %a, i16 %b)
       ; zext (a > b) - zext (a < b), in i8
```

## Example
`ucmp` of two `i16` into an `i8`:

```mir
define i8 @f(i16 %a, i16 %b) {
  %r = call i8 @llvm.ucmp.i8.i16(i16 %a, i16 %b)
  ret i8 %r
}
```

After:

```mir
define i8 @f(i16 %a, i16 %b) {
  %1 = icmp ugt i16 %a, %b
  %2 = zext i1 %1 to i8
  %3 = icmp ult i16 %a, %b
  %4 = zext i1 %3 to i8
  %5 = sub i8 %2, %4
  ret i8 %5
}
```

From unit test `a_three_way_compare_is_two_compares_and_a_subtract` in
`compares.rs`.

## Runs
Once per module in `driver::optimized`, with the bit counts, for a target
that lists `scmp` or `ucmp` under `expand`. A call on a target that does not
list it reaches selection, which refuses it by name.

## See also
LLVM's `TargetLowering::expandCMP`
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)).
