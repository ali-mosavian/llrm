//! Fixed-size bit sets over dense indices, for fixed points over a body's
//! values or blocks.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bits {
    words: Vec<u64>,
}

impl Bits {
    pub fn new(size: usize) -> Self {
        Self {
            words: vec![0; size.div_ceil(64)],
        }
    }

    pub fn insert(&mut self, index: usize) {
        self.words[index / 64] |= 1 << (index % 64);
    }

    pub fn remove(&mut self, index: usize) {
        self.words[index / 64] &= !(1 << (index % 64));
    }

    pub fn len(&self) -> usize {
        self.words.iter().map(|word| word.count_ones() as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.words.iter().zip(&other.words).any(|(one, more)| one & more != 0)
    }

    pub fn contains(&self, index: usize) -> bool {
        self.words[index / 64] & (1 << (index % 64)) != 0
    }

    pub fn union_with(&mut self, other: &Self) {
        for (word, more) in self.words.iter_mut().zip(&other.words) {
            *word |= more;
        }
    }

    pub fn intersect_with(&mut self, other: &Self) {
        for (word, kept) in self.words.iter_mut().zip(&other.words) {
            *word &= kept;
        }
    }

    pub fn subtract(&mut self, other: &Self) {
        for (word, gone) in self.words.iter_mut().zip(&other.words) {
            *word &= !gone;
        }
    }

    /// Set indices, ascending.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(at, word)| {
            let mut left = *word;
            std::iter::from_fn(move || {
                (left != 0).then(|| {
                    let bit = left.trailing_zeros() as usize;
                    left &= left - 1;
                    at * 64 + bit
                })
            })
        })
    }
}
