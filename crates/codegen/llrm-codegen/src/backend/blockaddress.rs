//! The address of a block (`blockaddress`) as a symbol: one per address-taken
//! block, named by its function and its MIR block, so code and data in any
//! module order can name it before the function that holds it is lowered.
//!
//! # How it works
//! The DAG carries it as a `GlobalAddress` of the function whose offset is
//! [`offset`] (a flag and the block), which every fold of a global plus an
//! offset keeps apart from a real offset into the function. `GlobalMap::address`
//! turns that into the [`key`] of the symbol; `globals::names` names the key
//! and the lowered function defines the name at the block's label.
//!
//! # See also
//! LLVM `AsmPrinter::GetBlockAddressSymbol`, X86 `LowerBlockAddress`.

use llrm_mir::context::GlobalId;
use llrm_mir::module::BlockId;

/// The bit of an offset that says it names a block, not a byte.
const FLAG: i64 = 1 << 30;
/// The first `Addr` index of a block's symbol, above every global's.
const KEYS: i64 = 1 << 48;
/// Bits of the key that hold the block (and of an offset: it stays below the
/// flag, which a 32-bit target keeps whole).
const BLOCK_BITS: u32 = 24;

/// The offset of a `GlobalAddress` of the function that names `block`.
pub fn offset(block: BlockId) -> i64 {
    FLAG | i64::from(block.0)
}

/// The block a `GlobalAddress` offset names, if it names one.
pub fn decode(offset: i64) -> Option<BlockId> {
    (offset & FLAG != 0).then(|| BlockId((offset & !FLAG) as u32))
}

/// The `Addr` index of `block`'s symbol.
pub fn key(
    function: GlobalId,
    block: BlockId,
) -> i64 {
    assert!(block.0 < 1 << BLOCK_BITS, "a function of {} blocks", block.0);
    KEYS | (i64::from(function.0) << BLOCK_BITS) | i64::from(block.0)
}

/// The assembler name of `block`'s symbol.
pub fn symbol(
    function: GlobalId,
    block: BlockId,
) -> String {
    format!("$BA{}_{}", function.0, block.0)
}
