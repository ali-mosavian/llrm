# Aggregates passed by value

A struct or union argument larger than 16 bytes is passed as one `byval` pointer: the C front end gives the callee a `ptr byval([N x i8])`
parameter and the caller the address of the object. The caller's copy is the stack's bytes where the same number of words would lie, so
Open Watcom and Borland callees and callers read the same layout; the callee pops them (or the caller does, by the convention), and a
callee that pops 64 KB or more raises the stack over them with the return address moved to the top (`pop [esp+n]; add esp, n; ret`)
because `ret imm16` cannot. A callee's parameter is its own copy: it may write it.

Up to 16 bytes the words are passed as before. clang's `X86_32ABIInfo::classifyArgumentType` (clang/lib/CodeGen/Targets/X86.cpp)
expands an aggregate of at most 4 x 32 bits when every field is a 32- or 64-bit scalar with no padding (`canExpandIndirectArgument`),
and passes any other `byval`; llrm applies only the size test, since the front end has no field list. A variadic callee and an
interrupt handler keep the words (they walk their arguments as consecutive words), and an aggregate the caller holds behind a far
pointer goes as words (the callee's byval pointer is near).

The copy is the target's `memcpy` lowering (`copied` in isel: moves up to the limit, else `rep movsd`).
The inliner does not inline a call with a `byval` parameter yet (LLVM copies it into an alloca first).
