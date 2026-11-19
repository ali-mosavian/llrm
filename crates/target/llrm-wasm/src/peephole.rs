//! `WebAssemblyPeephole`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyPeephole.cpp)):
//! the `return` that ends the function is the implicit return of the body's
//! last `end`, so it becomes `FALLTHROUGH_RETURN`: no opcode, no byte.
//!
//! The pin's other rewrite (a `memcpy`/`memmove`/`memset` call whose result is
//! its destination becomes a drop) needs `WebAssemblyMemIntrinsicResults`,
//! which is not translated yet.

use crate::dagisel::post_ids::{END_FUNCTION, FALLTHROUGH_RETURN, RETURN};
use crate::mir::Function;

pub fn peephole(f: &mut Function) -> bool {
    let Some(last) = f.blocks.last_mut() else { return false };
    let [.., ret, end] = &mut last[..] else { return false };
    if end.opcode != END_FUNCTION || ret.opcode != RETURN {
        return false;
    }
    ret.opcode = FALLTHROUGH_RETURN;
    true
}
