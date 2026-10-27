//! Finding the calls of an intrinsic in a module and handing each to a rewrite:
//! what every MIR expansion before selection shares.
//!
//! # How it works
//! [`expand_calls`] walks every function of a module and gives each call of an
//! intrinsic the caller picks to a rewrite, with what the pick returned.
//!
//! # Example
//! `memsetpattern` picks `memset.pattern` and rewrites each call as a loop.
//!
//! # Runs
//! From the expansions in this crate and in `llrm-codegen`, before selection.
//!
//! # See also
//! LLVM's `PreISelIntrinsicLowering`
//! ([PreISelIntrinsicLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PreISelIntrinsicLowering.cpp)).

use llrm_mir::context::Context;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, GlobalKind, GlobalValue, InstId, Module};

/// Every call in `module` of an intrinsic `pick` returns something for, given
/// to `rewrite` with what `pick` returned.
pub fn expand_calls<T>(
    module: &mut Module,
    pick: impl Fn(Intrinsic) -> Option<T>,
    mut rewrite: impl FnMut(&mut Context, &mut Function, InstId, T) -> Result<(), String>,
) -> Result<(), String> {
    let intrinsics: Vec<Option<Intrinsic>> = module.globals.iter().map(GlobalValue::intrinsic).collect();
    let context = &mut module.context;
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        let found: Vec<(InstId, T)> = function
            .walk()
            .filter_map(|(_, inst)| {
                let callee = llrm_mir::memory::callee(context, function, inst)?;
                Some((inst, pick(intrinsics[callee.0 as usize]?)?))
            })
            .collect();
        for (inst, what) in found {
            rewrite(context, function, inst, what)?;
        }
    }
    Ok(())
}
