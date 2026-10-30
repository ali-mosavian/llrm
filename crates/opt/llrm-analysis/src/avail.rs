//! The cells loads and stores name: what a load reads and lands in, what a
//! store writes.
//!
//! # How it works
//! The memory model is `MemRef` cells and [`crate::memoryssa`]'s `Accesses`.
//! Load forwarding is `llrm-transforms`' `gvn` (MemoryDependence), not here;
//! dead stores are `deadstore` there (LLVM's DSE).
//!
//! # Example
//! A load of a cell, and the value it lands in:
//!
//! ```mir
//! %v = load i16, ptr %p   ; `loaded_into`: the cell of %p, and %v
//! store i16 %v, ptr %q    ; `stored_from`: the cell of %q, and %v
//! ```
//!
//! # Provides
//! The accessors `loaded_into`, `stored_from`, `stored_cell`.
//!
//! # Needs
//! [`crate::memoryssa`] (`Accesses`).
//!
//! # See also
//! LLVM `MemoryDependenceAnalysis`, GVN `processLoad`.

use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use crate::memory::{MemRef, Unit};
use crate::memoryssa::Accesses;

/// The cell `inst` purely loads, and the value it lands in.
pub fn loaded_into(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, ValueId)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile: false, .. } => Some((accesses.references.get(&inst)?.clone(), instruction.result?)),
        _ => None,
    }
}

/// The cell `inst` purely stores, and what it wrote there.
pub fn stored_from(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, Operand)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Store { volatile: false, .. } => {
            Some((accesses.references.get(&inst)?.clone(), instruction.operands[0]))
        }
        _ => None,
    }
}

/// The cell `inst` purely stores to, whatever it put there: a store's, or
/// the bytes a `memset` fills.
pub fn stored_cell(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<MemRef> {
    stored_from(unit, accesses, inst).map(|(cell, _)| cell).or_else(|| {
        MemRef::filled(unit, inst).filter(|one| !one.volatile).and_then(|_| accesses.references.get(&inst).cloned())
    })
}
