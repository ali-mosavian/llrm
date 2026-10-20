> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/compares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/compares.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/compares.rs sha256:d9115db7f60e9283fabf2d5543ba7bbd205997dbacd4ee022cb05cd16d06f79a -->

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
  %0 = icmp ugt i16 %a, %b
  %1 = zext i1 %0 to i8
  %2 = icmp ult i16 %a, %b
  %3 = zext i1 %2 to i8
  %4 = sub i8 %1, %3
  ret i8 %4
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
