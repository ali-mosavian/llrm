//! A value's uses as LLVM keeps them (`Value::UseList`): a doubly linked
//! list, the newest use first. A use is added at the head and unlinked
//! through its own links, both O(1).
//!
//! A node per operand slot: the slots of instruction `i` are the nodes from
//! `base[i]` for `cap[i]` entries, so the node of `Use { user, index }` is
//! found without a search.

use crate::module::Use;

const NIL: u32 = u32::MAX;

#[derive(Clone, Debug, PartialEq)]
struct Node {
    at: Use,
    prev: u32,
    next: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UseLists {
    heads: Vec<u32>,
    nodes: Vec<Node>,
    base: Vec<u32>,
    cap: Vec<u32>,
}

/// The uses of one value, newest first.
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
    ) {
        self.heads.clear();
        self.heads.resize(values, NIL);
        self.nodes.clear();
        self.base.clear();
        self.cap.clear();
    }

    pub fn push_value(&mut self) {
        self.heads.push(NIL);
    }

    pub fn users(
        &self,
        value: u32,
    ) -> Users<'_> {
        Users { lists: self, head: self.heads[value as usize] }
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
        self.base[at.user.0 as usize] + at.index
    }

    /// LLVM's `Value::addUse`.
    pub fn link(
        &mut self,
        value: u32,
        at: Use,
    ) {
        let node = self.node(at);
        let head = self.heads[value as usize];
        self.nodes[node as usize] = Node { at, prev: NIL, next: head };
        if head != NIL {
            self.nodes[head as usize].prev = node;
        }
        self.heads[value as usize] = node;
    }

    /// LLVM's `Use::removeFromList`.
    pub fn unlink(
        &mut self,
        value: u32,
        at: Use,
    ) {
        let node = self.node(at);
        let (prev, next) = (self.nodes[node as usize].prev, self.nodes[node as usize].next);
        if prev == NIL {
            self.heads[value as usize] = next;
        } else {
            self.nodes[prev as usize].next = next;
        }
        if next != NIL {
            self.nodes[next as usize].prev = prev;
        }
    }

    /// The newest use of `value`, if any.
    pub fn head(
        &self,
        value: u32,
    ) -> Option<Use> {
        let head = self.heads[value as usize];
        (head != NIL).then(|| self.nodes[head as usize].at)
    }

    pub fn clear_all(&mut self) {
        self.heads.fill(NIL);
    }
}
