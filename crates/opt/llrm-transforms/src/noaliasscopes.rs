//! `cloneAndAdaptNoAliasScopes`
//! ([CloneFunction.cpp#L1213](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CloneFunction.cpp#L1213)):
//! a pass that copies blocks gives the copy its own scopes for those declared
//! inside the copied region.
//!
//! # How it works
//! A `llvm.experimental.noalias.scope.decl` states a scope that starts there
//! and lives for one run of the code around it; a copy of that code is another
//! run, so its accesses must not share the scope with the original's. MIR
//! has no metadata call operand: the declaration is a call carrying its scope
//! list as `!noalias.scope.decl` ([`crate::inlinemetadata::DECL`]).
//! [`declared_scopes`] reads the lists of the declarations in the region
//! (`identifyNoAliasScopesToClone`); [`cloned_and_adapted`] makes a new scope
//! for each (the same domain, the name and `:ext`) and replaces the old one
//! in the `!alias.scope`, `!noalias` and declaration lists of the copies.
//!
//! # Example
//! A loop declaring scope `!2` and unrolled twice: the copy's accesses and
//! declaration name a clone of `!2`.
//!
//! # Provides
//! [`declared_scopes`], [`cloned_and_adapted`].
//!
//! # Needs
//! [`crate::inlinemetadata`] for the attachment kinds.

use std::collections::BTreeMap;

use llrm_mir::module::{Function, InstId, MetadataId, MetadataNode, MetadataOperand};
use llrm_mir::passes::Declared;

use crate::inlinemetadata::{DECL, NOALIAS, SCOPE};

fn attached(
    function: &Function,
    inst: InstId,
    kind: &str,
) -> Option<MetadataId> {
    function.instruction(inst).metadata.iter().find(|(one, _)| one == kind).map(|&(_, node)| node)
}

/// Whether `inst` is a `llvm.experimental.noalias.scope.decl`.
pub(crate) fn declares(
    function: &Function,
    inst: InstId,
) -> bool {
    attached(function, inst, DECL).is_some()
}

/// `identifyNoAliasScopesToClone`: the scope lists declared in `insts`.
pub(crate) fn declared_scopes(
    function: &Function,
    insts: impl IntoIterator<Item = InstId>,
) -> Vec<MetadataId> {
    insts.into_iter().filter_map(|inst| attached(function, inst, DECL)).collect()
}

struct Nodes<'a> {
    module: &'a [MetadataNode],
    first: u32,
}

impl Nodes<'_> {
    fn node<'n>(
        &'n self,
        declared: &'n Declared,
        id: MetadataId,
    ) -> Option<&'n MetadataNode> {
        match id.0.checked_sub(self.first) {
            Some(at) => declared.nodes().get(at as usize),
            None => self.module.get(id.0 as usize),
        }
    }
}

/// `cloneAndAdaptNoAliasScopes` over `insts`, the instructions of the copy;
/// `ext` names the clones.
pub(crate) fn cloned_and_adapted(
    function: &mut Function,
    scopes: &[MetadataId],
    insts: &[InstId],
    ext: &str,
    module: &[MetadataNode],
    declared: &mut Declared,
) {
    if scopes.is_empty() {
        return;
    }
    let nodes = Nodes { module, first: declared.next_node().0 - declared.nodes().len() as u32 };
    // `cloneNoAliasScopes`.
    let mut cloned: BTreeMap<MetadataId, MetadataId> = BTreeMap::new();
    for &list in scopes {
        let Some(list) = nodes.node(declared, list) else { continue };
        let members: Vec<MetadataId> = list
            .operands
            .iter()
            .filter_map(|one| match one {
                MetadataOperand::Node(one) => Some(*one),
                _ => None,
            })
            .collect();
        for scope in members {
            if cloned.contains_key(&scope) {
                continue;
            }
            let Some(old) = nodes.node(declared, scope) else { continue };
            let domain = old.operands.get(1).cloned().unwrap_or(MetadataOperand::Null);
            let name = match old.operands.get(2) {
                Some(MetadataOperand::String(name)) if !name.is_empty() => format!("{name}:{ext}"),
                _ => ext.to_owned(),
            };
            let this = declared.next_node();
            let made = declared.node(MetadataNode {
                distinct: true,
                operands: vec![MetadataOperand::Node(this), domain, MetadataOperand::String(name)],
            });
            cloned.insert(scope, made);
        }
    }
    // `adaptNoAliasScopes`.
    let mut lists: BTreeMap<Vec<MetadataId>, MetadataId> = BTreeMap::new();
    for &inst in insts {
        for kind in [DECL, NOALIAS, SCOPE] {
            let Some(list) = attached(function, inst, kind) else { continue };
            let Some(node) = nodes.node(declared, list) else { continue };
            let mut replaced = false;
            let members: Vec<MetadataId> = node
                .operands
                .iter()
                .filter_map(|one| match one {
                    MetadataOperand::Node(one) => Some(*one),
                    _ => None,
                })
                .map(|one| match cloned.get(&one) {
                    Some(&new) => {
                        replaced = true;
                        new
                    }
                    None => one,
                })
                .collect();
            if !replaced {
                continue;
            }
            let id = *lists.entry(members.clone()).or_insert_with(|| {
                let operands = members.iter().map(|&one| MetadataOperand::Node(one)).collect();
                declared.node(MetadataNode { distinct: false, operands })
            });
            function.set_metadata(inst, kind, Some(id));
        }
    }
}
