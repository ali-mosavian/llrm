//! A short list of blocks without a heap allocation: the successors and
//! predecessors of a block.
//!
//! # How it works
//! Four blocks are held inline; a longer list (a switch, a join of many) spills
//! to a `Vec`. Both read as the slice of blocks, in the order they were pushed.
//! `Function::successors` and `predecessors` were 42 M of a d_faces -O2
//! compile, a `Vec` made, grown and copied for every ask.
//!
//! # Example
//! `function.successors(block)` of a conditional branch is `[then, else]`, held
//! in the value returned.
//!
//! # Used by
//! `Function::successors`, `Function::predecessors`.

use crate::module::BlockId;

const INLINE: usize = 4;

#[derive(Clone, Debug)]
pub enum BlockList {
    Few(u8, [BlockId; INLINE]),
    Many(Vec<BlockId>),
}

impl Default for BlockList {
    fn default() -> Self {
        Self::Few(0, [BlockId(0); INLINE])
    }
}

impl BlockList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(
        &mut self,
        block: BlockId,
    ) {
        match self {
            Self::Few(len, held) if usize::from(*len) < INLINE => {
                held[usize::from(*len)] = block;
                *len += 1;
            }
            Self::Few(len, held) => {
                let mut all = held[..usize::from(*len)].to_vec();
                all.push(block);
                *self = Self::Many(all);
            }
            Self::Many(all) => all.push(block),
        }
    }
}

impl std::ops::Deref for BlockList {
    type Target = [BlockId];

    fn deref(&self) -> &[BlockId] {
        match self {
            Self::Few(len, held) => &held[..usize::from(*len)],
            Self::Many(all) => all,
        }
    }
}

impl FromIterator<BlockId> for BlockList {
    fn from_iter<I: IntoIterator<Item = BlockId>>(blocks: I) -> Self {
        let mut out = Self::new();
        for block in blocks {
            out.push(block);
        }
        out
    }
}

/// The blocks of a list, by value.
pub struct Blocks {
    list: BlockList,
    at: usize,
}

impl Iterator for Blocks {
    type Item = BlockId;

    fn next(&mut self) -> Option<BlockId> {
        let found = self.list.get(self.at).copied();
        self.at += usize::from(found.is_some());
        found
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.list.len() - self.at;
        (left, Some(left))
    }
}

impl ExactSizeIterator for Blocks {}

impl IntoIterator for BlockList {
    type Item = BlockId;
    type IntoIter = Blocks;

    fn into_iter(self) -> Blocks {
        Blocks { list: self, at: 0 }
    }
}

impl<'a> IntoIterator for &'a BlockList {
    type Item = &'a BlockId;
    type IntoIter = std::slice::Iter<'a, BlockId>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl PartialEq for BlockList {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        **self == **other
    }
}

impl Eq for BlockList {}

impl PartialEq<Vec<BlockId>> for BlockList {
    fn eq(
        &self,
        other: &Vec<BlockId>,
    ) -> bool {
        **self == other[..]
    }
}

impl<const N: usize> PartialEq<[BlockId; N]> for BlockList {
    fn eq(
        &self,
        other: &[BlockId; N],
    ) -> bool {
        **self == other[..]
    }
}

impl PartialEq<&[BlockId]> for BlockList {
    fn eq(
        &self,
        other: &&[BlockId],
    ) -> bool {
        **self == **other
    }
}

impl From<BlockList> for Vec<BlockId> {
    fn from(list: BlockList) -> Self {
        match list {
            BlockList::Few(len, held) => held[..usize::from(len)].to_vec(),
            BlockList::Many(all) => all,
        }
    }
}

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
