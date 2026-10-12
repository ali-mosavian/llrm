> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/floatfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/floatfold.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/floatfold.rs sha256:b829b28c7c8ccba3a148659702c8f08aa957645015223616037b8bde7f309c3c -->

# floatfold

A float load that only one arithmetic instruction reads is that
instruction's memory operand: `fld [m]; faddp` is `fadd [m]`.

x87 arithmetic has a memory form for a float or double (`fadd qword ptr
[m]`), and LLVM folds a single-use load into
it (`X86InstrInfo::foldMemoryOperand`); gcc's output has it everywhere. The
allocator folds only a spilled value's reload (`floatassign::fused`). This
is the same fold for any load, before the values are placed: it saves the
load and the x87 register the loaded value held. Sound when nothing between
the load and its reader may write what it reads.
