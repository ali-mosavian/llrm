//! LLVM's DSE: llrm-core's `DropStores` and `without_dead_stores`, from
//! `optimize/transform.rs`. A store is dead when every path overwrites its
//! cell before reading it, or when nothing after it can read the cell:
//! `avail::dead_stores` finds them, `observers::private` saying which cells
//! no call and no exit observes. `Promote` leaves every store; this takes
//! those to cells nothing reads any more.
//!
//! The pass needs `Summaries` required, for its calls' footprints.
//! Dropped with the BC frontend: the data-segment bounds and the error
//! handlers' contracts.

use llrm_analysis::avail;
use llrm_analysis::manager::Pointers;
use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::observers;
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

pub struct Dse;

impl FunctionPass for Dse {
    fn name(&self) -> &'static str {
        "dse"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        match dropped(unit, analyses) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("dse: {error}"),
        }
    }
}

/// `unit`'s function without its dead stores; whether any went.
fn dropped(unit: &mut passes::Unit, analyses: &mut Analyses) -> Result<bool, String> {
    let (context, layout) = (&*unit.context, unit.layout);
    let accesses = Accesses::managed(context, layout, unit.function, analyses)?;
    let pointers = analyses.get::<Pointers>(context, layout, unit.function);
    let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
    let memory = Unit::within(context, layout, unit.function, analyses.outer());
    let dead = avail::dead_stores(&memory, &accesses, Some(&observers::private(memory, pointers)));
    for &store in &dead {
        unit.function.erase(store)?;
    }
    Ok(!dead.is_empty())
}

#[cfg(test)]
#[path = "dse_tests.rs"]
mod tests;
