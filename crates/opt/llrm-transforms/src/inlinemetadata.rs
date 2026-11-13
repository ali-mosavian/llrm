//! `PropagateCallSiteMetadata`
//! ([L998](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L998)):
//! a call site's `!llvm.mem.parallel_loop_access`, `!llvm.access.group`,
//! `!alias.scope` and `!noalias` go to every memory-accessing instruction of
//! the inlined copy, joined with what the instruction has.
//!
//! `ScopedAliasMetadataDeepCloner`
//! ([L1108](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1108)):
//! a callee that carries scope lists is inlined with its own copy of them, so
//! two copies of one body are not in one scope.
//!
//! `AddAlignmentAssumptions` (L1785) is
//! `preserve-alignment-assumptions-during-inlining`, hidden and off in the pin:
//! nothing to translate.
//!
//! # Deviations
//! - A node made here is not uniqued against the module's: two equal lists may
//!   have two ids. Nothing compares ids of non-distinct lists.

use std::collections::BTreeMap;

use llrm_mir::module::{Function, InstId, Metadata, MetadataId, MetadataNode, MetadataOperand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;

const PARALLEL: &str = "llvm.mem.parallel_loop_access";
const GROUP: &str = "llvm.access.group";
pub(crate) const SCOPE: &str = "alias.scope";
pub(crate) const NOALIAS: &str = "noalias";

/// What a call site states, taken before the call is replaced.
pub struct SiteMetadata {
    parallel: Option<MetadataId>,
    groups: Option<MetadataId>,
    scope: Option<MetadataId>,
    noalias: Option<MetadataId>,
}

fn attached(
    function: &Function,
    inst: InstId,
    kind: &str,
) -> Option<MetadataId> {
    function.instruction(inst).metadata.iter().find(|(one, _)| one == kind).map(|&(_, node)| node)
}

impl SiteMetadata {
    /// The metadata of `call`, if it states any of the four.
    pub fn of(
        function: &Function,
        call: InstId,
    ) -> Option<Self> {
        let site = Self {
            parallel: attached(function, call, PARALLEL),
            groups: attached(function, call, GROUP),
            scope: attached(function, call, SCOPE),
            noalias: attached(function, call, NOALIAS),
        };
        let any = site.parallel.or(site.groups).or(site.scope).or(site.noalias);
        any.map(|_| site)
    }
}

/// `Instruction::mayReadOrWriteMemory`.
fn accesses_memory(opcode: &Opcode) -> bool {
    llrm_mir::touched::touches_memory(opcode) || matches!(opcode, Opcode::Atomic(..))
}

pub(crate) struct Nodes<'a> {
    module: &'a [MetadataNode],
    first: u32,
    made: BTreeMap<Vec<MetadataId>, MetadataId>,
}

impl<'a> Nodes<'a> {
    pub(crate) fn new(
        module: &'a [MetadataNode],
        first: u32,
    ) -> Self {
        Self { module, first, made: BTreeMap::new() }
    }

    fn operands(
        &self,
        declared: &Declared,
        node: MetadataId,
    ) -> Vec<MetadataOperand> {
        match node.0.checked_sub(self.first) {
            Some(at) => declared.nodes().get(at as usize),
            None => self.module.get(node.0 as usize),
        }
        .map_or_else(Vec::new, |one| one.operands.clone())
    }

    pub(crate) fn list(
        &mut self,
        declared: &mut Declared,
        members: Vec<MetadataId>,
    ) -> MetadataId {
        if let Some(&one) = self.made.get(&members) {
            return one;
        }
        let operands = members.iter().map(|&one| MetadataOperand::Node(one)).collect();
        let node = declared.node(MetadataNode { distinct: false, operands });
        self.made.insert(members, node);
        node
    }

    /// `MDNode::concatenate`: `a`'s operands then `b`'s, each once.
    pub(crate) fn concatenated(
        &mut self,
        declared: &mut Declared,
        a: Option<MetadataId>,
        b: Option<MetadataId>,
    ) -> Option<MetadataId> {
        let (Some(first), Some(second)) = (a, b) else { return a.or(b) };
        let mut members = Vec::new();
        for operand in self.operands(declared, first).into_iter().chain(self.operands(declared, second)) {
            if let MetadataOperand::Node(one) = operand {
                if !members.contains(&one) {
                    members.push(one);
                }
            }
        }
        Some(self.list(declared, members))
    }

    /// `addToAccessGroupList`: a list's members, or the group itself.
    fn groups(
        &self,
        declared: &Declared,
        node: MetadataId,
    ) -> Vec<MetadataId> {
        let operands = self.operands(declared, node);
        match operands.first() {
            Some(MetadataOperand::Node(_)) => operands
                .into_iter()
                .filter_map(|one| match one {
                    MetadataOperand::Node(one) => Some(one),
                    _ => None,
                })
                .collect(),
            _ => vec![node],
        }
    }

    /// `uniteAccessGroups`.
    fn united(
        &mut self,
        declared: &mut Declared,
        a: Option<MetadataId>,
        b: Option<MetadataId>,
    ) -> Option<MetadataId> {
        let (Some(first), Some(second)) = (a, b) else { return a.or(b) };
        if first == second {
            return Some(first);
        }
        let mut members = self.groups(declared, first);
        for one in self.groups(declared, second) {
            if !members.contains(&one) {
                members.push(one);
            }
        }
        match members.len() {
            0 => None,
            1 => Some(members[0]),
            _ => Some(self.list(declared, members)),
        }
    }
}

/// `PropagateCallSiteMetadata` for the copy of a body whose instructions are
/// `copies`.
pub fn propagated(
    function: &mut Function,
    site: &SiteMetadata,
    copies: &[InstId],
    module: Metadata,
    declared: &mut Declared,
) {
    let first = declared.next_node().0 - declared.nodes().len() as u32;
    let mut nodes = Nodes::new(module.nodes, first);
    // The pin joins into the site's own list as it goes (its TODO): a later
    // instruction gets the earlier ones' lists too.
    let mut parallel = site.parallel;
    for &copy in copies {
        if !accesses_memory(&function.instruction(copy).opcode) {
            continue;
        }
        if parallel.is_some() {
            parallel = nodes.concatenated(declared, attached(function, copy, PARALLEL), parallel);
            function.set_metadata(copy, PARALLEL, parallel);
        }
        if site.groups.is_some() {
            let united = nodes.united(declared, attached(function, copy, GROUP), site.groups);
            function.set_metadata(copy, GROUP, united);
        }
        if site.scope.is_some() {
            let joined = nodes.concatenated(declared, attached(function, copy, SCOPE), site.scope);
            function.set_metadata(copy, SCOPE, joined);
        }
        if site.noalias.is_some() {
            let joined = nodes.concatenated(declared, attached(function, copy, NOALIAS), site.noalias);
            function.set_metadata(copy, NOALIAS, joined);
        }
    }
}

/// The kinds of attachment that name scopes.
const SCOPE_KINDS: [&str; 3] = [SCOPE, NOALIAS, DECL];
/// A `llvm.experimental.noalias.scope.decl` is a call carrying its scope list
/// here (MIR has no metadata call operand).
pub const DECL: &str = "noalias.scope.decl";

/// `ScopedAliasMetadataDeepCloner`: every node reachable from the scope lists
/// of a callee, each cloned once.
pub struct DeepClone(BTreeMap<MetadataId, MetadataId>);

impl DeepClone {
    /// Clones the scope metadata of `body`, if it has any.
    pub fn of(
        body: &Function,
        module: Metadata,
        declared: &mut Declared,
    ) -> Option<Self> {
        let first = declared.next_node().0 - declared.nodes().len() as u32;
        let operands = |declared: &Declared, node: MetadataId| -> Vec<MetadataOperand> {
            match node.0.checked_sub(first) {
                Some(at) => declared.nodes().get(at as usize),
                None => module.nodes.get(node.0 as usize),
            }
            .map_or_else(Vec::new, |one| one.operands.clone())
        };
        let distinct = |declared: &Declared, node: MetadataId| -> bool {
            match node.0.checked_sub(first) {
                Some(at) => declared.nodes().get(at as usize),
                None => module.nodes.get(node.0 as usize),
            }
            .is_some_and(|one| one.distinct)
        };
        let mut found: Vec<MetadataId> = Vec::new();
        let mut queue: Vec<MetadataId> = Vec::new();
        for (_, inst) in body.walk() {
            for kind in SCOPE_KINDS {
                if let Some(list) = attached(body, inst, kind)
                    && !found.contains(&list)
                {
                    found.push(list);
                    queue.push(list);
                }
            }
        }
        if found.is_empty() {
            return None;
        }
        while let Some(node) = queue.pop() {
            for operand in operands(declared, node) {
                if let MetadataOperand::Node(one) = operand
                    && !found.contains(&one)
                {
                    found.push(one);
                    queue.push(one);
                }
            }
        }
        // Ids first, so a node that names itself or a later one can be written.
        let mut next = declared.next_node().0;
        let map: BTreeMap<MetadataId, MetadataId> = found
            .iter()
            .map(|&old| {
                next += 1;
                (old, MetadataId(next - 1))
            })
            .collect();
        for &old in &found {
            let cloned = operands(declared, old)
                .into_iter()
                .map(|operand| match operand {
                    MetadataOperand::Node(one) => MetadataOperand::Node(map[&one]),
                    other => other,
                })
                .collect();
            let made = declared.node(MetadataNode { distinct: distinct(declared, old), operands: cloned });
            debug_assert_eq!(made, map[&old]);
        }
        Some(Self(map))
    }

    /// `remap`: the lists of `copies` from the callee's nodes to the clones.
    pub fn remap(
        &self,
        function: &mut Function,
        copies: &[InstId],
    ) {
        for &copy in copies {
            for kind in SCOPE_KINDS {
                if let Some(old) = attached(function, copy, kind)
                    && let Some(&new) = self.0.get(&old)
                {
                    function.set_metadata(copy, kind, Some(new));
                }
            }
        }
    }
}
