> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/calleepop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/calleepop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/calleepop.rs sha256:f71ea7f3dfd3f8598c1aa5244cb30e5a2a0166f26930858987ff792283f94850 -->

# calleepop

LLVM's GlobalOpt gives an internal function whose every caller it sees a
calling convention of its own (`fastcc`). Here that is the callee popping
its arguments: `ret N` once, and no cleanup of the arguments at each call.
The convention is assigned here and nowhere else: isel takes both sides'
cleanup from it (`passing`), so a call and the function it names
agree. Priced by the target for the level being built, in clocks or in
bytes.

Where the target's description gives a private function a convention of its
own (`private`), every such function takes that one, with the calls of it:
no one outside sees it, so the target's best serves, whatever ABI the
program has. No price is asked: registers cost no more than the stack at
either end. LLVM's `fastcc` here is x86's ECX/EDX; gcc's `regparm(3)` for a
`local` function, EAX/EDX/ECX; this is the description's.
