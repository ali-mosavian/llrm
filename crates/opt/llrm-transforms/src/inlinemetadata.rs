//! `PropagateCallSiteMetadata`
//! ([L998](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L998)):
//! a call site's `!llvm.mem.parallel_loop_access`, `!llvm.access.group`,
//! `!alias.scope` and `!noalias` go to every memory-accessing instruction of the
//! inlined copy, joined with what the instruction has.
//!
//! `AddAlignmentAssumptions` (L1785) is `preserve-alignment-assumptions-during-inlining`,
//! hidden and off in the pin: nothing to translate.
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
const SCOPE: &str = "alias.scope";
const NOALIAS: &str = "noalias";

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

struct Nodes<'a> {
    module: Metadata<'a>,
    declared: &'a mut Declared,
    first: u32,
    made: BTreeMap<Vec<MetadataId>, MetadataId>,
}

impl Nodes<'_> {
    fn operands(
        &self,
        node: MetadataId,
    ) -> Vec<MetadataOperand> {
        match node.0.checked_sub(self.first) {
            Some(at) => self.declared.nodes().get(at as usize),
            None => self.module.nodes.get(node.0 as usize),
        }
        .map_or_else(Vec::new, |one| one.operands.clone())
    }

    fn list(
        &mut self,
        members: Vec<MetadataId>,
    ) -> MetadataId {
        if let Some(&one) = self.made.get(&members) {
            return one;
        }
        let operands = members.iter().map(|&one| MetadataOperand::Node(one)).collect();
        let node = self.declared.node(MetadataNode { distinct: false, operands });
        self.made.insert(members, node);
        node
    }

    /// `MDNode::concatenate`: `a`'s operands then `b`'s, each once.
    fn concatenated(
        &mut self,
        a: Option<MetadataId>,
        b: Option<MetadataId>,
    ) -> Option<MetadataId> {
        let (Some(first), Some(second)) = (a, b) else { return a.or(b) };
        let mut members = Vec::new();
        for operand in self.operands(first).into_iter().chain(self.operands(second)) {
            if let MetadataOperand::Node(one) = operand {
                if !members.contains(&one) {
                    members.push(one);
                }
            }
        }
        Some(self.list(members))
    }

    /// `addToAccessGroupList`: a list's members, or the group itself.
    fn groups(
        &self,
        node: MetadataId,
    ) -> Vec<MetadataId> {
        let operands = self.operands(node);
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
        a: Option<MetadataId>,
        b: Option<MetadataId>,
    ) -> Option<MetadataId> {
        let (Some(first), Some(second)) = (a, b) else { return a.or(b) };
        if first == second {
            return Some(first);
        }
        let mut members = self.groups(first);
        for one in self.groups(second) {
            if !members.contains(&one) {
                members.push(one);
            }
        }
        match members.len() {
            0 => None,
            1 => Some(members[0]),
            _ => Some(self.list(members)),
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
    let mut nodes = Nodes { module, declared, first, made: BTreeMap::new() };
    // The pin joins into the site's own list as it goes (its TODO): a later
    // instruction gets the earlier ones' lists too.
    let mut parallel = site.parallel;
    for &copy in copies {
        if !accesses_memory(&function.instruction(copy).opcode) {
            continue;
        }
        if parallel.is_some() {
            parallel = nodes.concatenated(attached(function, copy, PARALLEL), parallel);
            function.set_metadata(copy, PARALLEL, parallel);
        }
        if site.groups.is_some() {
            let united = nodes.united(attached(function, copy, GROUP), site.groups);
            function.set_metadata(copy, GROUP, united);
        }
        if site.scope.is_some() {
            let joined = nodes.concatenated(attached(function, copy, SCOPE), site.scope);
            function.set_metadata(copy, SCOPE, joined);
        }
        if site.noalias.is_some() {
            let joined = nodes.concatenated(attached(function, copy, NOALIAS), site.noalias);
            function.set_metadata(copy, NOALIAS, joined);
        }
    }
}
