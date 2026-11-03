//! `SortRegionInfo`: the loops and exceptions the CFG sort and the stackifier
//! keep contiguous, seen as one kind of region.
//!
//! # See also
//! `WebAssemblySortRegion.cpp`, pin d1106deb71cc.

use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::BlockId;

use crate::exception_info::Exceptions;

/// A loop, by its header, or an exception, by its index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Region {
    Loop(usize),
    Exception(usize),
}

pub struct Regions<'a> {
    pub loops: &'a LoopInfo<BlockId>,
    pub exceptions: &'a Exceptions,
}

impl Regions<'_> {
    /// `getRegionFor`: the smallest loop or exception that contains `block`. A
    /// loop is the smaller when its header is inside the exception (an
    /// exception holds the blocks of every region under it, a loop does not
    /// hold the blocks that cannot reach its header).
    pub fn region_for(
        &self,
        block: usize,
    ) -> Option<Region> {
        let looped = self.loops.get_loop_for(BlockId(block as u32));
        let exception = self.exceptions.exception_for(block);
        match (looped, exception) {
            (None, None) => None,
            (Some(one), None) => Some(Region::Loop(one.get_header().0 as usize)),
            (Some(one), Some(e)) if self.exceptions.contains(e, one.get_header().0 as usize) => {
                Some(Region::Loop(one.get_header().0 as usize))
            }
            (_, Some(e)) => Some(Region::Exception(e)),
        }
    }

    pub fn header(
        &self,
        region: Region,
    ) -> usize {
        match region {
            Region::Loop(header) => header,
            Region::Exception(e) => self.exceptions.all[e].header,
        }
    }

    pub fn is_loop(
        &self,
        region: Region,
    ) -> bool {
        matches!(region, Region::Loop(_))
    }

    pub fn contains(
        &self,
        region: Region,
        block: usize,
    ) -> bool {
        match region {
            Region::Loop(header) => {
                self.loops.get_loop_for(BlockId(header as u32)).is_some_and(|one| one.contains(BlockId(block as u32)))
            }
            Region::Exception(e) => self.exceptions.contains(e, block),
        }
    }

    pub fn num_blocks(
        &self,
        region: Region,
    ) -> usize {
        match region {
            Region::Loop(header) => {
                self.loops.get_loop_for(BlockId(header as u32)).map_or(0, |one| one.get_num_blocks())
            }
            Region::Exception(e) => self.exceptions.all[e].blocks.len(),
        }
    }

    /// `getBottom`: the last block of everything the header dominates that the
    /// region holds. A loop does not hold the exceptions it surrounds the pads
    /// of, so theirs count.
    pub fn bottom(
        &self,
        region: Region,
    ) -> usize {
        match region {
            Region::Exception(e) => self.exceptions.bottom(e),
            Region::Loop(header) => {
                let Some(one) = self.loops.get_loop_for(BlockId(header as u32)) else { return header };
                let mut bottom = header;
                for &block in one.get_blocks() {
                    let block = block.0 as usize;
                    bottom = bottom.max(block);
                    if let Some(e) =
                        self.exceptions.exception_for(block).filter(|&e| self.exceptions.all[e].header == block)
                    {
                        bottom = bottom.max(self.exceptions.bottom(e));
                    }
                }
                bottom
            }
        }
    }
}
