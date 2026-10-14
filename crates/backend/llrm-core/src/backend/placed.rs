//! Where the allocator has put each value: a register by value, and the order
//! the values were put in.
//!
//! An insertion-ordered map pays for taking a value out of its middle with a
//! shift of everything after it (`IndexMap::shift_remove`: 14% of the compile
//! of `branches` N=800, where last-chance recoloring takes out and puts back
//! the holders of a register at every try). The order is kept as a number each
//! value was given when it was put in, and made a list only where it is asked
//! for: the end of an allocation, where values that share a copy are moved to
//! join it in the order they were placed.

use llrm_lir::registers::RegId;

use crate::support::hash::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Placed {
    held: HashMap<u32, (RegId, u64)>,
    next: u64,
}

impl Placed {
    pub fn get(
        &self,
        value: &u32,
    ) -> Option<&RegId> {
        self.held.get(value).map(|(register, _)| register)
    }

    pub fn contains_key(
        &self,
        value: &u32,
    ) -> bool {
        self.held.contains_key(value)
    }

    /// Puts `value` in `register`: at the end of the order if it is new, and
    /// where it was if it is moved.
    pub fn insert(
        &mut self,
        value: u32,
        register: RegId,
    ) -> Option<RegId> {
        match self.held.get_mut(&value) {
            Some((was, _)) => Some(std::mem::replace(was, register)),
            None => {
                self.held.insert(value, (register, self.next));
                self.next += 1;
                None
            }
        }
    }

    /// Takes `value` out; the others keep their order.
    pub fn remove(
        &mut self,
        value: &u32,
    ) -> Option<RegId> {
        self.held.remove(value).map(|(register, _)| register)
    }

    pub fn len(&self) -> usize {
        self.held.len()
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// The values, in no order.
    pub fn values_unordered(&self) -> impl Iterator<Item = u32> + '_ {
        self.held.keys().copied()
    }

    /// The values in the order they were put in.
    pub fn in_order(&self) -> Vec<u32> {
        let mut all: Vec<(u64, u32)> = self.held.iter().map(|(value, (_, at))| (*at, *value)).collect();
        all.sort_unstable();
        all.into_iter().map(|(_, value)| value).collect()
    }

    /// As the map of values to registers in the order they were put in.
    pub fn ordered(&self) -> crate::support::hash::IndexMap<u32, RegId> {
        self.in_order().into_iter().map(|value| (value, self.held[&value].0)).collect()
    }
}

impl std::ops::Index<&u32> for Placed {
    type Output = RegId;

    fn index(
        &self,
        value: &u32,
    ) -> &RegId {
        &self.held[value].0
    }
}

impl From<&crate::support::hash::IndexMap<u32, RegId>> for Placed {
    fn from(map: &crate::support::hash::IndexMap<u32, RegId>) -> Self {
        let mut out = Self::default();
        for (value, register) in map {
            out.insert(*value, *register);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value taken out and put back goes to the end, as in an
    /// insertion-ordered map, and one moved to another register stays where
    /// it was.
    #[test]
    fn test_a_value_put_back_goes_last_and_one_moved_stays() {
        let mut placed = Placed::default();
        for (value, register) in [(1, RegId::EAX), (2, RegId::EBX), (3, RegId::ECX)] {
            placed.insert(value, register);
        }
        placed.insert(1, RegId::EDX);
        assert_eq!(placed.in_order(), [1, 2, 3]);
        assert_eq!(placed.remove(&2), Some(RegId::EBX));
        placed.insert(2, RegId::ESI);
        assert_eq!(placed.in_order(), [1, 3, 2]);
        assert_eq!((placed[&1], placed[&2]), (RegId::EDX, RegId::ESI));
    }
}
