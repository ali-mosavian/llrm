//! The address of a block (`blockaddress`) as a symbol: one per address-taken
//! block, named by its function and its MIR block, so code and data in any
//! module order can name it before the function that holds it is lowered.
//!
//! # How it works
//! The DAG carries it as a `GlobalAddress` of the function whose offset is
//! [`offset`] (a flag and the block), which every fold of a global plus an
//! offset keeps apart from a real offset into the function.
//! `GlobalMap::address` turns that into the [`key`] of the symbol;
//! `globals::names` names the key and the lowered function defines the name at
//! the block's label.
//!
//! # Example
//! ```text
//! blockaddress(@f, %b)  ==>  the symbol `$BA<f>_<b>`, defined at %b's label
//! indirectbr ptr %p, [%b, %c]  ==>  jmp *%p, successors %b and %c
//! ```
//!
//! # See also
//! LLVM `AsmPrinter::GetBlockAddressSymbol`, X86 `LowerBlockAddress`.

use llrm_mir::context::GlobalId;
use llrm_mir::module::BlockId;

/// Bits of the key that hold the block.
const BLOCK_BITS: u32 = 24;
/// The first `Addr` index of a block's symbol, above every global's.
const KEYS: i64 = 1 << 48;
/// Bits of an offset that hold the block.
const OFFSET_BITS: u32 = 23;
const BLOCK_MASK: i64 = (1 << OFFSET_BITS) - 1;
/// What the offset's other bits are: the one bit above the block, in the 16 MiB
/// the small code model allows a symbol's offset (`isOffsetSuitableForCodeModel`:
/// beyond it the DAG adds the offset after the symbol instead of folding it),
/// and positive, so no negative offset reads as a block. The function's own
/// symbol has no use for such an offset, so no real address is taken for one.
const TAG: i64 = 1 << OFFSET_BITS;

/// The offset of a `GlobalAddress` of the function that names `block`.
pub fn offset(block: BlockId) -> i64 {
    assert!(i64::from(block.0) <= BLOCK_MASK, "a function of {} blocks", block.0);
    TAG | i64::from(block.0)
}

/// The block a `GlobalAddress` offset names, if it names one: a negative
/// offset, or any other into an object, does not.
pub fn decode(offset: i64) -> Option<BlockId> {
    (offset & !BLOCK_MASK == TAG).then(|| BlockId((offset & BLOCK_MASK) as u32))
}

/// The `Addr` index of `block`'s symbol.
pub fn key(
    function: GlobalId,
    block: BlockId,
) -> i64 {
    assert!(block.0 < 1 << BLOCK_BITS, "a function of {} blocks", block.0);
    KEYS | (i64::from(function.0) << BLOCK_BITS) | i64::from(block.0)
}

/// What every [`symbol`] starts with.
pub const PREFIX: &str = "$BA";

/// The assembler name of `block`'s symbol.
pub fn symbol(
    function: GlobalId,
    block: BlockId,
) -> String {
    format!("{PREFIX}{}_{}", function.0, block.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Before: a flag in the sign bits read every negative offset as a block
    /// (`&p[-1]` of a global), and QB's objects panicked in `key`.
    #[test]
    fn an_offset_into_an_object_is_no_block() {
        for offset in [0, 8, -1, -8, i64::MIN, 1 << 40, (1 << 23) - 1, 1 << 24, 1 << 30, 1 << 31] {
            assert_eq!(decode(offset), None, "{offset}");
        }
        assert_eq!(decode(super::offset(BlockId(194))), Some(BlockId(194)));
    }
}
