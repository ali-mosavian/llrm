//! `simplifySwitchLookup`: `switchToLookupTable` and `convertSwitchToArithmetic`, a step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).
//!
//! Not translated yet: [`folded`] finds nothing. [`crate::switchopt`] calls it
//! where LLVM does.

use llrm_mir::context::Context;
use llrm_mir::module::{BlockId, Function};

use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    _options: &Options,
    _context: &Context,
    _spec: &Speculation,
    _function: &mut Function,
    _block: BlockId,
) -> bool {
    false
}
