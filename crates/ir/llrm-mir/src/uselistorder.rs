//! The `uselistorder` directives of a function: where a value's use list is
//! not the order a reader would build, the shuffle that says so. A
//! translation of
//! [`predictValueUseListOrderImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Bitcode/Writer/ValueEnumerator.cpp#L170),
//! so a printed function reads like `opt -S -preserve-ll-uselistorder`'s, for
//! the per-pass comparison with the pin's order. MIRROR.
//!
//! DEVIATE: values only. The lists of blocks (`uselistorder_bb`), of constants
//! and of globals are not predicted.

use crate::module::{Function, InstId, Use, ValueDef, ValueId};
use crate::uselist::Held;

impl Function {
    /// The values whose use list differs from the reader's default, each with
    /// its shuffle: the `i`th use of the default order is at place `shuffle[i]`
    /// of the list as it stands. In value order.
    pub fn use_list_orders(&self) -> Vec<(ValueId, Vec<u32>)> {
        let mut place = vec![None; self.instructions.len()];
        for (at, (_, inst)) in self.walk().enumerate() {
            place[inst.0 as usize] = Some(at as i64);
        }
        let place_of = |inst: InstId| place[inst.0 as usize];
        let mut out = Vec::new();
        for value in 0..self.values.len() {
            // An argument stands before every instruction.
            let id = match self.values[value].def {
                ValueDef::Argument(_) => -1,
                ValueDef::Instruction(def) => match place_of(def) {
                    Some(id) => id,
                    None => continue,
                },
            };
            let mut list: Vec<(Use, u32)> = self
                .uses
                .users(Held::Value(value as u32))
                .iter()
                .filter(|one| place_of(one.user).is_some())
                .zip(0..)
                .map(|(one, at)| (*one, at))
                .collect();
            if list.len() < 2 {
                continue;
            }
            // If ID is 4, then expect: 7 6 5 1 2 3.
            let before = |left: &Use, right: &Use| {
                let (l, r) = (place_of(left.user).unwrap(), place_of(right.user).unwrap());
                if l != r {
                    return if l < r { r <= id } else { l > id };
                }
                if l <= id { left.index < right.index } else { left.index > right.index }
            };
            list.sort_by(|left, right| {
                if before(&left.0, &right.0) {
                    std::cmp::Ordering::Less
                } else if before(&right.0, &left.0) {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });
            if list.iter().map(|one| one.1).is_sorted() {
                continue;
            }
            out.push((ValueId(value as u32), list.into_iter().map(|one| one.1).collect()));
        }
        out
    }
}
