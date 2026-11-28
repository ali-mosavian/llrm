> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mergefunc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mergefunc.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mergefunc.rs sha256:bde29a3fec93b2764cd3f91b347a37f2e70d1436c3b23bb0908d6cb41239f972 -->

# mergefunc

Whether two functions are the same code: the equivalence of LLVM's
`FunctionComparator`, and the hash that buckets functions before any pair is
compared.

## How it works
A translation of LLVM
[`FunctionComparator.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/FunctionComparator.cpp)
at d1106deb71cc. `compare` (L1041) walks both functions from the entry, a
block pair at a time, taking the successor pairs in order (L1041-L1081).
Values are equal by serial number: an argument by its position, an
instruction or a block by the order the walk met it (`cmpValues`, L903), so
two bodies that differ in names and in layout but not in shape compare
equal. A constant is equal to itself (the context interns them), and the
address of the function being compared is equal to the address of the other
(`cmpValuesAllowingSelfRef`, L887), so two directly recursive copies of one
function match. `function_hash` is `FunctionComparator::functionHash`
(L1): the function type, the block and instruction counts and each
instruction's opcode, so it never separates two functions `equivalent` would
join.

## Deviation
DEVIATE MERGEFUNC-CMP (owner x86-m32, exit 2026-11-30): the pin answers a
three-way order (`int`) so that a `std::set` can sort functions; this
answers a symmetric yes/no, and the caller buckets by `function_hash` and
compares a function against one representative of each class in its bucket.
The pin's order is only a means to the same classes. A function with an
instruction the comparison does not know is equal to nothing: a
missed merge, never a wrong one.

## Example
```text
 define i32 @a(i32 %x) { %r = add i32 %x, 1  ret i32 %r }
 define i32 @b(i32 %y) { %s = add i32 %y, 1  ret i32 %s }   ; equivalent to @a
 define i32 @c(i32 %y) { %s = add i32 %y, 2  ret i32 %s }   ; not: another constant
```

## Used by
Named by `pipeline`.
