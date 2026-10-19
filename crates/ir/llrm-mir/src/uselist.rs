//! A value's uses as LLVM keeps them: a doubly linked list per value, the
//! newest use first.
//! [`Value::addUse`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Value.h#L515)
//! adds at the head and
//! [`Use::removeFromList`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Use.h#L95)
//! unlinks through the use's own links
//! ([`Use::addToList`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Use.h#L87)),
//! both O(1). MIRROR.
//!
//! DEVIATE 1: the links live in an arena node per operand slot, not in the
//! operand. MIR operands are `Vec<Operand>` on the instruction (D5 of
//! `target-architecture.md`); the slots of instruction `i` are the nodes from
//! `base[i]` for `cap[i]` entries, so the node of `Use { user, index }` is
//! found without a search. `node` asserts the slot exists; `check_uses` is the
//! test.
//!
//! A block is named by terminators and phis, so it has a list as a value does:
//! LLVM's `BasicBlock` is a `Value` and its predecessors are its use list.
//!
//! DEVIATE 2 (temporary): `Users::{len, to_vec, contains}` read a use list as
//! a slice, which LLVM cannot; the readers are counted by
//! `slice-users.txt` and the methods go at zero.

use crate::module::Use;

const NIL: u32 = u32::MAX;

#[cfg(test)]
thread_local! {
    /// List entries a walk has stepped over, for a test that an edit reads none
    /// to find the use it changes.
    pub(crate) static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq)]
struct Node {
    at: Use,
    prev: u32,
    next: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UseLists {
    heads: Vec<u32>,
    block_heads: Vec<u32>,
    nodes: Vec<Node>,
    base: Vec<u32>,
    cap: Vec<u32>,
}

/// What a use list belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Held {
    Value(u32),
    Block(u32),
}

impl Held {
    /// What `operand` names, none for a constant.
    pub fn of(operand: crate::module::Operand) -> Option<Held> {
        match operand {
            crate::module::Operand::Value(value) => Some(Held::Value(value.0)),
            crate::module::Operand::Block(block) => Some(Held::Block(block.0)),
            crate::module::Operand::Constant(_) => None,
        }
    }
}

/// The uses of one value or block, newest first.
#[derive(Clone, Copy, Debug)]
pub struct Users<'a> {
    lists: &'a UseLists,
    head: u32,
}

pub struct UserIter<'a> {
    lists: &'a UseLists,
    at: u32,
}

impl<'a> Iterator for UserIter<'a> {
    type Item = &'a Use;

    fn next(&mut self) -> Option<&'a Use> {
        (self.at != NIL).then(|| {
            #[cfg(test)]
            WALKED.with(|walked| walked.set(walked.get() + 1));
            let node = &self.lists.nodes[self.at as usize];
            self.at = node.next;
            &node.at
        })
    }
}

impl<'a> IntoIterator for Users<'a> {
    type Item = &'a Use;
    type IntoIter = UserIter<'a>;

    fn into_iter(self) -> UserIter<'a> {
        UserIter { lists: self.lists, at: self.head }
    }
}

impl<'a> Users<'a> {
    pub fn iter(&self) -> UserIter<'a> {
        (*self).into_iter()
    }

    /// LLVM's `use_empty`.
    pub fn is_empty(&self) -> bool {
        self.head == NIL
    }

    /// LLVM has no O(1) count; this walks the list.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn first(&self) -> Option<&'a Use> {
        self.iter().next()
    }

    /// The use, when it is the only one (LLVM's `hasOneUse` and `user_back`).
    pub fn only(&self) -> Option<&'a Use> {
        let mut uses = self.iter();
        let one = uses.next()?;
        uses.next().is_none().then_some(one)
    }

    pub fn to_vec(&self) -> Vec<Use> {
        self.iter().copied().collect()
    }

    pub fn contains(
        &self,
        one: &Use,
    ) -> bool {
        self.iter().any(|other| other == one)
    }
}

impl UseLists {
    pub fn reset(
        &mut self,
        values: usize,
        blocks: usize,
    ) {
        self.heads.clear();
        self.heads.resize(values, NIL);
        self.block_heads.clear();
        self.block_heads.resize(blocks, NIL);
        self.nodes.clear();
        self.base.clear();
        self.cap.clear();
    }

    pub fn push_value(&mut self) {
        self.heads.push(NIL);
    }

    pub fn push_block(&mut self) {
        self.block_heads.push(NIL);
    }

    fn head_of(
        &self,
        of: Held,
    ) -> u32 {
        match of {
            Held::Value(value) => self.heads[value as usize],
            Held::Block(block) => self.block_heads[block as usize],
        }
    }

    fn head_mut(
        &mut self,
        of: Held,
    ) -> &mut u32 {
        match of {
            Held::Value(value) => &mut self.heads[value as usize],
            Held::Block(block) => &mut self.block_heads[block as usize],
        }
    }

    pub fn users(
        &self,
        of: Held,
    ) -> Users<'_> {
        Users { lists: self, head: self.head_of(of) }
    }

    /// Room for `len` operand slots of `inst`. Its old slots must be unlinked.
    pub fn slots(
        &mut self,
        inst: u32,
        len: usize,
    ) {
        let inst = inst as usize;
        if self.base.len() <= inst {
            self.base.resize(inst + 1, 0);
            self.cap.resize(inst + 1, 0);
        }
        if (self.cap[inst] as usize) < len {
            let cap = len.max(self.cap[inst] as usize * 2);
            self.base[inst] = self.nodes.len() as u32;
            self.cap[inst] = cap as u32;
            self.nodes.resize(
                self.nodes.len() + cap,
                Node { at: Use { user: crate::module::InstId(0), index: 0 }, prev: NIL, next: NIL },
            );
        }
    }

    fn node(
        &self,
        at: Use,
    ) -> u32 {
        debug_assert!(at.index < self.cap[at.user.0 as usize], "{at:?} has no slot");
        self.base[at.user.0 as usize] + at.index
    }

    /// LLVM's `Value::addUse`.
    pub fn link(
        &mut self,
        of: Held,
        at: Use,
    ) {
        let node = self.node(at);
        let head = self.head_of(of);
        self.nodes[node as usize] = Node { at, prev: NIL, next: head };
        if head != NIL {
            self.nodes[head as usize].prev = node;
        }
        *self.head_mut(of) = node;
    }

    /// LLVM's `Use::removeFromList`.
    pub fn unlink(
        &mut self,
        of: Held,
        at: Use,
    ) {
        let node = self.node(at);
        let (prev, next) = (self.nodes[node as usize].prev, self.nodes[node as usize].next);
        if prev == NIL {
            *self.head_mut(of) = next;
        } else {
            self.nodes[prev as usize].next = next;
        }
        if next != NIL {
            self.nodes[next as usize].prev = prev;
        }
    }

    /// The newest use of `of`, if any.
    pub fn head(
        &self,
        of: Held,
    ) -> Option<Use> {
        let head = self.head_of(of);
        (head != NIL).then(|| self.nodes[head as usize].at)
    }

    pub fn clear_all(&mut self) {
        self.heads.fill(NIL);
        self.block_heads.fill(NIL);
    }
}
