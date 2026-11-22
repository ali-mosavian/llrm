//! InstCombine's `visitBitCast`, from
//! [`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L3426)
//! at d1106deb71cc: its first step, a cast from a type to itself is its
//! operand.
//!
//! The cast-pair table turns `fptrunc (fpext x)` back to `x`'s type into a
//! `bitcast` to the same type, which this step removes (`commonCastTransforms`
//! creates it, the next visit folds it).
//!
//! MIRROR: the order of the steps.
//!
//! DEVIATE 1: the rest of `visitBitCast` is not translated: the vector steps
//! (MIR has no vectors), `optimizeBitCastFromPhi`, `canonicalizeBitCastExtElt`,
//! `foldBitCastBitwiseLogic` and `foldBitCastSelect`. Exit: 2026-12-06. The
//! closing `commonCastTransforms` is: `bitcast (bitcast x)` was kept, 24 extra
//! `float`/`i32` round trips in QB `exec_loop`.
//!
//! # Example
//! ```text
//! %r = bitcast float %x to float           ; %r is %x
//! %i = bitcast float %x to i32
//! %f = bitcast i32 %i to float              ; %f is %x
//! ```
//!
//! # Used by
//! `combine` (its `visit`, the `BitCast` opcode).

use llrm_mir::module::InstId;

use crate::combine::{Combiner, Visit};

/// `visitBitCast`.
pub fn visit_bitcast(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let src = c.function.instruction(inst).operands[0];
    // Get rid of casts from one type to the same type. These are useless and
    // can be replaced by the operand.
    if c.function.instruction(inst).ty == c.type_of(src) {
        return Visit::Replace(src);
    }
    // `return commonCastTransforms(CI)`: a cast of a cast, of a select, of a phi.
    crate::combine_casts::common_cast_transforms(c, inst).unwrap_or(Visit::None)
}
