//! Interprocedural register use: a call to a function of the module destroys
//! the registers that function writes, not every caller-saved one.
//!
//! Translates LLVM's `RegUsageInfoCollector` (what each function leaves
//! different, recorded once it is allocated) and the use `RegUsageInfoPropagate`
//! gives it at the call (the call's register mask). The pin has it off
//! (`-enable-ipra`); GCC's `-fipa-ra` is on from -O1. Opt-in per target
//! (`Tti::interprocedural_register_use`).
//!
//! Functions are compiled callees first. A call inside a cycle of calls keeps
//! the convention's answer. So does a function's call to itself: a value kept
//! across it is in a register the function never writes (a callee-saved one),
//! and what the call destroys is already what the function writes.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::lir_view::{caller_saved, id, Class};
use crate::mfunc::{Callee, MFunc, Op};

/// What a function writes of the registers its caller may not rely on.
pub type Written = Arc<BTreeSet<RegId>>;

thread_local! {
    static KNOWN: RefCell<HashMap<String, Written>> = RefCell::new(HashMap::new());
}

/// Forgets every function; a compile of a new module starts here.
pub fn clear() {
    KNOWN.with(|known| known.borrow_mut().clear());
}

/// Records that calling `name` destroys `written`.
pub fn record(
    name: &str,
    written: Written,
) {
    KNOWN.with(|known| known.borrow_mut().insert(name.to_owned(), written));
}

/// Stops using what was recorded for `name`.
pub fn forget(name: &str) {
    KNOWN.with(|known| known.borrow_mut().remove(name));
}

/// What a call to `name` destroys, where it is known.
pub fn known(name: &str) -> Option<Written> {
    KNOWN.with(|known| known.borrow().get(name).cloned())
}

/// The registers `function`, allocated, leaves different for its caller: each
/// caller-saved one an operation defines, what the calls it makes destroy, and
/// `x16`/`x17`, which the lowerer and a linker veneer use without the
/// operations saying so.
pub fn written(function: &MFunc) -> Written {
    let all: BTreeSet<RegId> = caller_saved().into_iter().collect();
    let mut out: BTreeSet<RegId> = [id(Class::Gpr, 16), id(Class::Gpr, 17)].into_iter().collect();
    for block in &function.blocks {
        for op in &block.ops {
            let (defs, _) = op.defs_and_uses();
            for register in defs {
                let register = crate::lir_view::lir_id(register);
                if all.contains(&register) {
                    out.insert(register);
                }
            }
            let callee = match op {
                Op::Call { callee, .. } => Some(callee),
                Op::Return { tail: Some(callee), .. } => Some(callee),
                _ => None,
            };
            if let Some(callee) = callee {
                match callee {
                    Callee::Symbol(name) if *name == function.name => {}
                    Callee::Symbol(name) if known(name).is_some() => out.extend(known(name).unwrap().iter().copied()),
                    _ => out.extend(all.iter().copied()),
                }
            }
        }
    }
    Arc::new(out)
}
