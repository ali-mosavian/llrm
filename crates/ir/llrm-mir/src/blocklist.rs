//! A short list of blocks without a heap allocation: the successors and
//! predecessors of a block.
//!
//! Four blocks are held inline (see [`crate::inline::InlineVec`]); a longer
//! list (a switch, a join of many) spills to a `Vec`. `Function::successors`
//! and `predecessors` were 42 M of a d_faces -O2 compile, a `Vec` made, grown
//! and copied for every ask.
//!
//! # Used by
//! `Function::successors`, `Function::predecessors`.

use crate::inline::InlineVec;
use crate::module::BlockId;

pub type BlockList = InlineVec<BlockId, 4>;

#[cfg(test)]
mod tests {
    use super::BlockList;
    use crate::module::BlockId;

    #[test]
    fn test_a_list_reads_in_the_order_pushed_whether_held_inline_or_spilled() {
        for n in 0..9_u32 {
            let list: BlockList = (0..n).map(|k| BlockId(k * 3)).collect();
            let want: Vec<BlockId> = (0..n).map(|k| BlockId(k * 3)).collect();
            assert_eq!(list, want, "{n} blocks");
            assert_eq!(list.len() as u32, n);
            assert_eq!(list.clone().into_iter().collect::<Vec<_>>(), want);
            assert_eq!(Vec::from(list), want);
        }
    }
}
