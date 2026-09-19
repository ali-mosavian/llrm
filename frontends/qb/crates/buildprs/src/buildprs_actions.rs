//! Syntax-directed graph builders for the DOS-style `buildprs` backend.

use std::collections::BTreeMap;

use crate::buildprs_encoder::ND_BRANCH;
use crate::buildprs_grammar::GrammarExpr;
use crate::buildprs_graph::{NodeId, StateFlags, StateGraph, StateKind, StateNode};
use crate::buildprs_lowering::{LoweringConfig, LoweringError, LoweringSymbols};

#[derive(Debug, Default)]
pub struct GraphSharedSuffixes {
    targets: BTreeMap<Vec<String>, NodeId>,
}

impl GraphSharedSuffixes {
    pub fn insert(&mut self, key: Vec<String>, target: NodeId) {
        self.targets.entry(key).or_insert(target);
    }

    pub fn target_for(&self, key: &[String]) -> Option<NodeId> {
        self.targets.get(key).copied()
    }
}

pub fn compile_expr_to_graph(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    expr: &GrammarExpr,
) -> Result<NodeId, LoweringError> {
    let accept = graph.add_node(StateNode::accept());
    let reject = graph.add_node(StateNode::reject());
    compile_expr(graph, symbols, config, expr, Some(accept), Some(reject))
        .map(|entry| entry.unwrap_or(accept))
}

pub fn compile_expr_to_graph_with_suffixes(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    expr: &GrammarExpr,
    suffixes: Option<&mut GraphSharedSuffixes>,
    capture_suffixes: bool,
) -> Result<NodeId, LoweringError> {
    let accept = graph.add_node(StateNode::accept());
    let reject = graph.add_node(StateNode::reject());
    compile_expr_with_suffixes(
        graph,
        symbols,
        config,
        expr,
        Some(accept),
        Some(reject),
        suffixes,
        capture_suffixes,
    )
    .map(|entry| entry.unwrap_or(accept))
}

pub fn compile_expr(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    expr: &GrammarExpr,
    success: Option<NodeId>,
    failure: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    match expr {
        GrammarExpr::Empty => Ok(success),
        GrammarExpr::Group(inner) => compile_expr(graph, symbols, config, inner, success, failure),
        GrammarExpr::Sequence(items) => {
            let mut next = success;
            for item in items.iter().rev() {
                next = compile_expr(graph, symbols, config, item, next, failure)?;
            }
            Ok(next)
        }
        GrammarExpr::Alternative(items) => {
            compile_alternative(graph, symbols, config, items, success, failure)
        }
        GrammarExpr::Optional(inner) => {
            compile_optional(graph, symbols, config, inner, success, failure)
        }
        GrammarExpr::Repeat(inner) => {
            compile_repeat(graph, symbols, config, inner, success, failure)
        }
        GrammarExpr::TokenRef(token) => {
            let node_id = symbols.node_id_for_token(token, config)?;
            Ok(Some(add_branch(graph, node_id, success, failure)))
        }
        GrammarExpr::NonTerminalRef(name) => {
            let node_id = symbols.node_id_for_nonterminal(name, config)?;
            Ok(Some(add_branch(graph, node_id, success, failure)))
        }
        GrammarExpr::Emit(emit) => {
            let opcode = symbols.resolve_emit_word(&emit.args)?;
            let node = graph.add_node(StateNode::emit(opcode));
            if let Some(success) = success {
                let success = action_success_target(graph, success);
                graph.add_true_link(node, success);
            }
            Ok(Some(node))
        }
        GrammarExpr::Mark(mark) => {
            let node = graph.add_node(StateNode::mark(mark.slot));
            if let Some(success) = success {
                let success = action_success_target(graph, success);
                graph.add_true_link(node, success);
            }
            Ok(Some(node))
        }
        GrammarExpr::External { .. } => Err(LoweringError::UnsupportedExpr("EXTERNAL")),
    }
}

fn compile_alternative(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    items: &[GrammarExpr],
    success: Option<NodeId>,
    failure: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    compile_alternative_from(graph, symbols, config, items, 0, success, failure)
}

fn compile_alternative_from(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    items: &[GrammarExpr],
    index: usize,
    success: Option<NodeId>,
    failure: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    let Some(item) = items.get(index) else {
        return Ok(failure);
    };

    if index + 1 < items.len() {
        if let Some((syntax_arm, action_arm)) = grouped_action_default_alternative(item) {
            let next_alternative = compile_alternative_from(
                graph,
                symbols,
                config,
                items,
                index + 1,
                success,
                failure,
            )?;
            let action_fallback = compile_action_alternative_arm(
                graph,
                symbols,
                config,
                action_arm,
                success,
                next_alternative,
                index + 1,
            )?;
            return compile_grouped_action_default_syntax_arm(
                graph,
                symbols,
                config,
                syntax_arm,
                success,
                action_fallback,
                next_alternative,
            );
        }
    }

    if index + 1 < items.len()
        && (index + 2 < items.len() || is_mark_expr(&items[index + 1]))
        && alternative_sequence_items(item)
            .and_then(|sequence| sequence.split_first())
            .is_some_and(|(first, rest)| !rest.is_empty() && is_node_expr(first))
        && is_action_expr(&items[index + 1])
    {
        let next_alternative = if index + 2 < items.len() {
            compile_alternative_from(graph, symbols, config, items, index + 2, success, failure)?
        } else {
            failure
        };
        let action_fallback = compile_action_alternative_arm(
            graph,
            symbols,
            config,
            &items[index + 1],
            success,
            next_alternative,
            index + 1,
        )?;
        return compile_grouped_action_default_syntax_arm(
            graph,
            symbols,
            config,
            item,
            success,
            action_fallback,
            next_alternative,
        );
    }

    let next_alternative =
        compile_alternative_from(graph, symbols, config, items, index + 1, success, failure)?;
    compile_alternative_arm(
        graph,
        symbols,
        config,
        item,
        success,
        failure,
        next_alternative,
        index,
    )
}

fn compile_alternative_arm(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    item: &GrammarExpr,
    success: Option<NodeId>,
    _arm_failure: Option<NodeId>,
    next_alternative: Option<NodeId>,
    source_index: usize,
) -> Result<Option<NodeId>, LoweringError> {
    let Some((first, rest)) =
        alternative_sequence_items(item).and_then(|items| items.split_first())
    else {
        return compile_action_alternative_arm(
            graph,
            symbols,
            config,
            item,
            success,
            next_alternative,
            source_index,
        );
    };
    if rest.is_empty() || !is_node_expr(first) {
        if let Some(sequence) = alternative_sequence_items(item) {
            if let Some(entry) = compile_sequence_with_late_commit(
                graph,
                symbols,
                config,
                sequence,
                success,
                next_alternative,
            )? {
                return Ok(Some(entry));
            }
        }
        return compile_expr(graph, symbols, config, item, success, next_alternative);
    }

    let rest_expr = GrammarExpr::Sequence(rest.to_vec());
    let rest_success = if source_index != 0
        && success_is_branch_or_action(graph, success)
        && action_sequence_ends_with_action(rest)
    {
        optional_skip_target(graph, success)
    } else {
        success
    };
    let committed_failure = Some(graph.add_node(StateNode::reject()));
    let rest_entry = compile_expr(
        graph,
        symbols,
        config,
        &rest_expr,
        rest_success,
        committed_failure,
    )?;
    compile_expr(graph, symbols, config, first, rest_entry, next_alternative)
}

fn compile_action_alternative_arm(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    item: &GrammarExpr,
    success: Option<NodeId>,
    next_alternative: Option<NodeId>,
    source_index: usize,
) -> Result<Option<NodeId>, LoweringError> {
    let arm_success =
        if source_index != 0 && success_is_branch_like(graph, success) && is_action_expr(item) {
            optional_skip_target(graph, success)
        } else {
            success
        };
    compile_expr(graph, symbols, config, item, arm_success, next_alternative)
}

fn compile_sequence_with_late_commit(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    items: &[GrammarExpr],
    success: Option<NodeId>,
    next_alternative: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    let Some(commit_index) = items.iter().position(is_node_expr) else {
        return Ok(None);
    };
    if commit_index == 0 {
        return Ok(None);
    }

    let committed_failure = Some(graph.add_node(StateNode::reject()));
    let mut next = success;
    for item in items.iter().skip(commit_index + 1).rev() {
        next = compile_expr(graph, symbols, config, item, next, committed_failure)?;
    }
    next = compile_expr(
        graph,
        symbols,
        config,
        &items[commit_index],
        next,
        next_alternative,
    )?;
    for item in items.iter().take(commit_index).rev() {
        next = compile_expr(graph, symbols, config, item, next, next_alternative)?;
    }
    Ok(next)
}

fn compile_grouped_action_default_syntax_arm(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    syntax_arm: &GrammarExpr,
    success: Option<NodeId>,
    action_fallback: Option<NodeId>,
    next_alternative: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    let Some((first, rest)) =
        alternative_sequence_items(syntax_arm).and_then(|items| items.split_first())
    else {
        return compile_expr(
            graph,
            symbols,
            config,
            syntax_arm,
            success,
            next_alternative,
        );
    };
    if rest.is_empty() || !is_node_expr(first) {
        return compile_expr(
            graph,
            symbols,
            config,
            syntax_arm,
            success,
            next_alternative,
        );
    }

    if !rest
        .first()
        .is_some_and(|item| is_node_expr(first_expr_in_sequence(item)))
    {
        let rest_expr = GrammarExpr::Sequence(rest.to_vec());
        let committed_reject = graph.add_node(StateNode::reject());
        let rest_entry = compile_expr(
            graph,
            symbols,
            config,
            &rest_expr,
            success,
            Some(committed_reject),
        )?;
        return compile_expr(graph, symbols, config, first, rest_entry, action_fallback);
    }

    let rest_entry = compile_tail_with_first_item_fallback(
        graph,
        symbols,
        config,
        rest,
        success,
        action_fallback,
    )?;
    compile_expr(graph, symbols, config, first, rest_entry, next_alternative)
}

fn compile_tail_with_first_item_fallback(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    tail: &[GrammarExpr],
    success: Option<NodeId>,
    first_failure: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    let tail = flatten_single_sequence_group(tail);
    let Some((first, rest)) = tail.split_first() else {
        return Ok(success);
    };
    if rest.is_empty() {
        return compile_expr(graph, symbols, config, first, success, first_failure);
    }

    let committed_reject = graph.add_node(StateNode::reject());
    let rest_expr = GrammarExpr::Sequence(rest.to_vec());
    let rest_success =
        if success_is_branch_or_action(graph, success) && action_sequence_ends_with_action(rest) {
            optional_skip_target(graph, success)
        } else {
            success
        };
    let rest_entry = compile_expr(
        graph,
        symbols,
        config,
        &rest_expr,
        rest_success,
        Some(committed_reject),
    )?;
    compile_expr(graph, symbols, config, first, rest_entry, first_failure)
}

fn flatten_single_sequence_group(items: &[GrammarExpr]) -> &[GrammarExpr] {
    let [item] = items else {
        return items;
    };
    match ungroup_expr(item) {
        GrammarExpr::Sequence(nested) => nested,
        _ => items,
    }
}

fn alternative_sequence_items(expr: &GrammarExpr) -> Option<&[GrammarExpr]> {
    match expr {
        GrammarExpr::Group(group) => alternative_sequence_items(group),
        GrammarExpr::Sequence(items) => Some(items),
        _ => None,
    }
}

fn action_sequence_ends_with_action(items: &[GrammarExpr]) -> bool {
    let Some(last) = items.last() else {
        return false;
    };
    is_action_expr(last)
}

fn grouped_action_default_alternative(expr: &GrammarExpr) -> Option<(&GrammarExpr, &GrammarExpr)> {
    let GrammarExpr::Alternative(items) = ungroup_expr(expr) else {
        return None;
    };
    let [syntax_arm, action_arm] = items.as_slice() else {
        return None;
    };
    let Some((first, rest)) =
        alternative_sequence_items(syntax_arm).and_then(|items| items.split_first())
    else {
        return None;
    };
    if rest.is_empty() || !is_node_expr(first) || !is_action_expr(action_arm) {
        return None;
    }
    Some((syntax_arm, action_arm))
}

fn is_action_expr(expr: &GrammarExpr) -> bool {
    matches!(
        single_expr(expr),
        GrammarExpr::Emit(_) | GrammarExpr::Mark(_)
    )
}

fn is_mark_expr(expr: &GrammarExpr) -> bool {
    matches!(single_expr(expr), GrammarExpr::Mark(_))
}

fn single_expr(expr: &GrammarExpr) -> &GrammarExpr {
    match ungroup_expr(expr) {
        GrammarExpr::Sequence(items) if items.len() == 1 => single_expr(&items[0]),
        expr => expr,
    }
}

fn first_expr_in_sequence(expr: &GrammarExpr) -> &GrammarExpr {
    match ungroup_expr(expr) {
        GrammarExpr::Sequence(items) if !items.is_empty() => first_expr_in_sequence(&items[0]),
        expr => expr,
    }
}

fn success_is_branch_or_action(graph: &StateGraph, success: Option<NodeId>) -> bool {
    let Some(success) = success else {
        return false;
    };
    matches!(
        graph.node(success).kind,
        StateKind::Branch | StateKind::Emit | StateKind::Mark
    )
}

fn success_is_branch_like(graph: &StateGraph, success: Option<NodeId>) -> bool {
    let Some(success) = success else {
        return false;
    };
    graph.node(success).kind == StateKind::Branch
}

fn success_is_accept(graph: &StateGraph, success: Option<NodeId>) -> bool {
    let Some(success) = success else {
        return false;
    };
    graph.node(success).kind == StateKind::Accept
}

fn compile_expr_with_suffixes(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    expr: &GrammarExpr,
    success: Option<NodeId>,
    failure: Option<NodeId>,
    mut suffixes: Option<&mut GraphSharedSuffixes>,
    capture_suffixes: bool,
) -> Result<Option<NodeId>, LoweringError> {
    let GrammarExpr::Sequence(items) = expr else {
        return compile_expr(graph, symbols, config, expr, success, failure);
    };

    if let Some(target) = shared_suffix_sequence_target(suffixes.as_deref(), items) {
        let node_id = node_id_for_expr(symbols, config, &items[0])?;
        let node = add_branch(graph, node_id, Some(target), failure);
        graph.node_mut(node).flags.insert(StateFlags::TRUE_SHARED);
        return Ok(Some(node));
    }

    let mut next = success;
    for index in (0..items.len()).rev() {
        next = compile_expr_with_suffixes(
            graph,
            symbols,
            config,
            &items[index],
            next,
            failure,
            suffixes.as_deref_mut(),
            capture_suffixes,
        )?;
        if capture_suffixes && index > 0 {
            if let (Some(suffixes), Some(entry)) = (suffixes.as_deref_mut(), next) {
                suffixes.insert(expr_suffix_key(&items[index..]), entry);
            }
        }
    }
    Ok(next)
}

fn shared_suffix_sequence_target(
    suffixes: Option<&GraphSharedSuffixes>,
    items: &[GrammarExpr],
) -> Option<NodeId> {
    let suffixes = suffixes?;
    if items.len() != 2 || !is_node_expr(&items[0]) {
        return None;
    }
    suffixes.target_for(&expr_suffix_key(&items[1..]))
}

fn node_id_for_expr(
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    expr: &GrammarExpr,
) -> Result<u16, LoweringError> {
    match ungroup_expr(expr) {
        GrammarExpr::TokenRef(token) => symbols.node_id_for_token(token, config),
        GrammarExpr::NonTerminalRef(name) => symbols.node_id_for_nonterminal(name, config),
        _ => Err(LoweringError::UnsupportedExpr("shared suffix prefix")),
    }
}

fn compile_repeat(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    inner: &GrammarExpr,
    success: Option<NodeId>,
    failure: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    let body_start = graph.len();
    let placeholder = graph.add_node(StateNode::branch_node(u16::from(ND_BRANCH)));
    if let Some(items) = optional_sequence_items(inner) {
        if let Some((first, rest)) = items.split_first() {
            if !rest.is_empty() {
                let rest_expr = GrammarExpr::Sequence(rest.to_vec());
                let rest_entry = compile_expr(
                    graph,
                    symbols,
                    config,
                    &rest_expr,
                    Some(placeholder),
                    failure,
                )?;
                if rest.len() == 1 {
                    if let Some(rest_entry) = rest_entry {
                        graph
                            .node_mut(rest_entry)
                            .flags
                            .insert(StateFlags::TRUE_SHARED);
                    }
                }
                let entry = compile_expr(
                    graph,
                    symbols,
                    config,
                    first,
                    rest_entry,
                    success.or(failure),
                )?
                .unwrap_or(placeholder);
                rewire_repeat_placeholder(graph, placeholder, entry);
                rewire_repeat_immediate_successes(graph, body_start, entry);
                return Ok(Some(entry));
            }
        }
    }
    let entry = compile_expr(
        graph,
        symbols,
        config,
        inner,
        Some(placeholder),
        success.or(failure),
    )?
    .unwrap_or(placeholder);
    rewire_repeat_placeholder(graph, placeholder, entry);
    rewire_repeat_immediate_successes(graph, body_start, entry);
    Ok(Some(entry))
}

fn rewire_repeat_placeholder(graph: &mut StateGraph, placeholder: NodeId, entry: NodeId) {
    let ids = graph.ids().collect::<Vec<_>>();
    for id in ids {
        if id == placeholder || graph.node(id).kind != StateKind::Branch {
            continue;
        }
        if graph.node(id).true_link == Some(placeholder) {
            graph.change_true_link(id, Some(entry));
        }
        if graph.node(id).false_link == Some(placeholder) {
            graph.change_false_link(id, Some(entry));
        }
    }
    graph.add_true_link(placeholder, entry);
}

fn rewire_repeat_immediate_successes(graph: &mut StateGraph, body_start: usize, entry: NodeId) {
    for index in body_start..graph.len() {
        let id = NodeId(index);
        if graph.node(id).kind == StateKind::Branch && graph.node(id).true_link.is_none() {
            graph.add_true_link(id, entry);
        }
    }
}

fn compile_optional(
    graph: &mut StateGraph,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
    inner: &GrammarExpr,
    success: Option<NodeId>,
    failure: Option<NodeId>,
) -> Result<Option<NodeId>, LoweringError> {
    if success_is_accept(graph, success) {
        if let Some((prefix, tail_alternatives)) = optional_prefixed_tail_alternative(inner) {
            let tail_expr = GrammarExpr::Alternative(tail_alternatives);
            let tail_entry = compile_expr(graph, symbols, config, &tail_expr, success, failure)?;
            return compile_expr(graph, symbols, config, prefix, tail_entry, success);
        }
    }

    if matches!(ungroup_expr(inner), GrammarExpr::Alternative(_))
        && success_is_branch_or_action(graph, success)
    {
        let skip_entry = optional_skip_target(graph, success);
        let entry = compile_expr(graph, symbols, config, inner, success, skip_entry)?;
        return Ok(entry);
    }

    if let Some(items) = optional_sequence_items(inner) {
        if let Some((first, rest)) = items.split_first() {
            if !rest.is_empty() {
                let rest_expr = GrammarExpr::Sequence(rest.to_vec());
                let committed_failure = Some(graph.add_node(StateNode::reject()));
                let rest_entry = compile_expr(
                    graph,
                    symbols,
                    config,
                    &rest_expr,
                    success,
                    committed_failure,
                )?;
                let skip_entry = if optional_sequence_uses_empty_skip(first, success, graph) {
                    optional_skip_target(graph, success)
                } else {
                    success
                };
                return compile_expr(graph, symbols, config, first, rest_entry, skip_entry);
            }
        }
    }

    let entry = compile_expr(graph, symbols, config, inner, success, success)?;
    Ok(entry)
}

fn optional_prefixed_tail_alternative(
    inner: &GrammarExpr,
) -> Option<(&GrammarExpr, Vec<GrammarExpr>)> {
    let GrammarExpr::Alternative(items) = ungroup_expr(inner) else {
        return None;
    };
    let (first_arm, other_arms) = items.split_first()?;
    let (prefix, rest) = alternative_sequence_items(first_arm)?.split_first()?;
    if rest.is_empty() || !is_node_expr(prefix) {
        return None;
    }
    if rest.first().is_some_and(is_action_expr) {
        return None;
    }

    let mut tail_alternatives = Vec::with_capacity(other_arms.len() + 1);
    tail_alternatives.push(sequence_expr(rest));
    tail_alternatives.extend(other_arms.iter().cloned());
    Some((prefix, tail_alternatives))
}

fn sequence_expr(items: &[GrammarExpr]) -> GrammarExpr {
    match items {
        [item] => item.clone(),
        _ => GrammarExpr::Sequence(items.to_vec()),
    }
}

fn optional_sequence_items(inner: &GrammarExpr) -> Option<&[GrammarExpr]> {
    match inner {
        GrammarExpr::Group(group) => optional_sequence_items(group),
        GrammarExpr::Sequence(items) => Some(items),
        _ => None,
    }
}

fn optional_skip_target(graph: &mut StateGraph, success: Option<NodeId>) -> Option<NodeId> {
    let success = success?;
    let node = graph.add_node(StateNode::branch_node(u16::from(ND_BRANCH)));
    graph.add_true_link(node, success);
    Some(node)
}

fn optional_sequence_uses_empty_skip(
    _first: &GrammarExpr,
    success: Option<NodeId>,
    graph: &StateGraph,
) -> bool {
    let Some(success) = success else {
        return false;
    };
    if graph.node(success).kind == StateKind::Accept {
        return false;
    }
    true
}

fn add_branch(
    graph: &mut StateGraph,
    node_id: u16,
    success: Option<NodeId>,
    failure: Option<NodeId>,
) -> NodeId {
    let node = graph.add_node(StateNode::branch_node(node_id));
    if let Some(success) = success {
        if let Some(success) = branch_success_target(graph, success) {
            graph.add_true_link(node, success);
        }
    }
    if let Some(failure) = failure {
        if let Some(failure) = branch_failure_target(graph, failure) {
            graph.add_false_link(node, failure);
        }
    }
    node
}

fn branch_success_target(graph: &StateGraph, success: NodeId) -> Option<NodeId> {
    if graph.node(success).kind == StateKind::Accept {
        return None;
    }
    Some(success)
}

fn action_success_target(graph: &mut StateGraph, success: NodeId) -> NodeId {
    if graph.node(success).kind == StateKind::Accept {
        return graph.add_node(StateNode::accept());
    }
    success
}

fn branch_failure_target(graph: &mut StateGraph, failure: NodeId) -> Option<NodeId> {
    if graph.node(failure).kind == StateKind::Accept {
        return Some(graph.add_node(StateNode::accept()));
    }
    if graph.node(failure).kind == StateKind::Reject {
        return Some(graph.add_node(StateNode::reject()));
    }
    Some(failure)
}

fn expr_suffix_key(items: &[GrammarExpr]) -> Vec<String> {
    items.iter().map(expr_key).collect()
}

fn expr_key(expr: &GrammarExpr) -> String {
    match ungroup_expr(expr) {
        GrammarExpr::TokenRef(token) => format!("tk:{token}"),
        GrammarExpr::NonTerminalRef(name) => format!("nt:{name}"),
        GrammarExpr::Emit(emit) => format!("emit:{:?}", emit.args),
        GrammarExpr::Mark(mark) => format!("mark:{}", mark.slot),
        GrammarExpr::Empty => "empty".to_string(),
        other => format!("{other:?}"),
    }
}

fn ungroup_expr(expr: &GrammarExpr) -> &GrammarExpr {
    match expr {
        GrammarExpr::Group(inner) => ungroup_expr(inner),
        other => other,
    }
}

fn is_node_expr(expr: &GrammarExpr) -> bool {
    matches!(
        ungroup_expr(expr),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    )
}

#[cfg(test)]
mod tests {
    use crate::buildprs_grammar::parse_grammar;

    use super::*;

    fn sample_symbols(source: &str) -> (crate::buildprs_grammar::GrammarFile, LoweringSymbols) {
        let grammar = parse_grammar(source).expect("sample grammar should parse");
        let symbols = LoweringSymbols::from_qbasic_11_grammar(&grammar, "opStBeep|x\nopLit|x\n");
        (grammar, symbols)
    }

    #[test]
    fn sequence_links_tokens_and_emit_to_accept_or_reject() {
        let source = r#"TOKENS:
   tkBEEP ("BEEP"),

Statements:
   tkBEEP;

Functions:

NonTerminals:
sample:
   tkBEEP EMIT(opStBeep);
"#;
        let (grammar, symbols) = sample_symbols(source);
        let mut graph = StateGraph::new();
        let root = compile_expr_to_graph(
            &mut graph,
            &symbols,
            &LoweringConfig::qbasic_11(),
            &grammar.nonterminals[0].body.productions[0].expr,
        )
        .expect("graph should build");

        let root_node = graph.node(root);
        assert!(root_node.true_link.is_some());
        assert!(root_node.false_link.is_some());
        assert!(graph
            .ids()
            .any(|id| graph.node(id).kind == crate::buildprs_graph::StateKind::Emit));
    }

    #[test]
    fn alternatives_chain_failures_to_next_arm() {
        let source = r#"TOKENS:
   tkA ("A"),
   tkB ("B"),

Statements:

Functions:

NonTerminals:
sample:
   (tkA | tkB);
"#;
        let (grammar, symbols) = sample_symbols(source);
        let mut graph = StateGraph::new();
        let root = compile_expr_to_graph(
            &mut graph,
            &symbols,
            &LoweringConfig::qbasic_11(),
            &grammar.nonterminals[0].body.productions[0].expr,
        )
        .expect("graph should build");

        let first = graph.node(root);
        let second = first.false_link.expect("first arm should fall through");
        assert!(graph.node(second).false_link.is_some());
        assert_eq!(first.true_link, graph.node(second).true_link);
    }
}
