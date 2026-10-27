> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalforward.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalforward.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalforward.rs sha256:75d504cdd4d36fff0032a8c1c02367fd4a5072ff69fb8e14a296b1f013d91bd2 -->

# globalforward

Loads of an internal variable that its only store dominates read the
stored constant: LLVM GlobalOpt's `forwardStoredOnceStore`
(<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1448>).

## How it works
A variable qualifies when it is internal, initialized, not `constant`, not
named by another global's initializer, and every use of it is a plain load
or store. It must be stored once, anywhere in the program, with a constant
other than its initializer. A load of the variable's own address in the
storing function, of the stored type, that the store dominates reads that
constant, even when the function recurses; the load is replaced by it.
Loads elsewhere, or not dominated, still read the initializer or the
constant and stay.

## Example
```mir
@c = internal global float 0.0
define float @f() {          define float @f() {
b0:                          b0:
  store float 18.0, ptr @c     store float 18.0, ptr @c
  br label %b1                 br label %b1
b1:                    =>    b1:
  %a = load float, ptr @c      ret float 18.0
  ret float %a               }
}
```

## Runs
After `globalsra`, which localizes a variable one non-recursive body uses
before this sees it.

## See also
`globalsra`, which splits a variable into the pieces this forwards.
