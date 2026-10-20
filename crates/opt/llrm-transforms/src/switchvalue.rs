//! The value-comparison folds of `simplifySwitch`: `simplifyEqualityComparisonWithOnlyPredecessor`, `simplifySwitchOnSelect` and `foldValueComparisonIntoPredecessors`, a step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).
//!
//! Not translated yet: [`folded`] finds nothing. It will hold, from the pin
//! (`SimplifyCFG.cpp` at d1106deb71cc): `simplifyEqualityComparisonWithOnlyPredecessor`
//! (L973), `foldValueComparisonIntoPredecessors` (L1475), `simplifySwitchOnSelect`
//! (L5192) with `simplifySwitchOnSelectRemap` (L5083), and their helpers
//! (`valuesOverlap`, `eliminateBlockCases`, `performValueComparisonIntoPredecessorFolding`).
//! Owed by llrm-w-4, 2026-10-23; the registry row is in `parity.rs`. [`crate::switchopt`] calls it
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
