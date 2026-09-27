fn expr_contains_token(expr: &GrammarExpr, name: &str) -> bool {
    match expr {
        GrammarExpr::TokenRef(candidate) => candidate == name,
        GrammarExpr::Sequence(items) | GrammarExpr::Alternative(items) => {
            items.iter().any(|item| expr_contains_token(item, name))
        }
        GrammarExpr::Group(inner) | GrammarExpr::Optional(inner) | GrammarExpr::Repeat(inner) => {
            expr_contains_token(inner, name)
        }
        _ => false,
    }
}

fn expr_contains_mark_slot(expr: &GrammarExpr, slot: u8) -> bool {
    match expr {
        GrammarExpr::Mark(mark) => mark.slot == slot,
        GrammarExpr::Sequence(items) | GrammarExpr::Alternative(items) => {
            items.iter().any(|item| expr_contains_mark_slot(item, slot))
        }
        GrammarExpr::Group(inner) | GrammarExpr::Optional(inner) | GrammarExpr::Repeat(inner) => {
            expr_contains_mark_slot(inner, slot)
        }
        _ => false,
    }
}

fn expr_is_emit_ident(expr: &GrammarExpr, name: &str) -> bool {
    let GrammarExpr::Emit(emit) = expr else {
        return false;
    };
    emit.args
        .iter()
        .any(|arg| matches!(arg, EmitArg::Ident(ident) if ident == name))
}

fn expr_contains_emit_ident(expr: &GrammarExpr, name: &str) -> bool {
    match expr {
        GrammarExpr::Emit(emit) => emit
            .args
            .iter()
            .any(|arg| matches!(arg, EmitArg::Ident(ident) if ident == name)),
        GrammarExpr::Sequence(items) | GrammarExpr::Alternative(items) => {
            items.iter().any(|item| expr_contains_emit_ident(item, name))
        }
        GrammarExpr::Group(inner) | GrammarExpr::Optional(inner) | GrammarExpr::Repeat(inner) => {
            expr_contains_emit_ident(inner, name)
        }
        _ => false,
    }
}

fn expr_suffix_key(items: &[GrammarExpr]) -> Vec<String> {
    items.iter().map(expr_key).collect()
}

fn expr_key(expr: &GrammarExpr) -> String {
    match expr {
        GrammarExpr::TokenRef(name) => format!("tk:{name}"),
        GrammarExpr::NonTerminalRef(name) => format!("nt:{name}"),
        GrammarExpr::Emit(emit) => format!("emit:{:?}", emit.args),
        GrammarExpr::Mark(mark) => format!("mark:{}", mark.slot),
        GrammarExpr::Optional(inner) => format!("optional:{}", expr_key(inner)),
        GrammarExpr::Repeat(inner) => format!("repeat:{}", expr_key(inner)),
        GrammarExpr::Group(inner) => expr_key(inner),
        GrammarExpr::Alternative(_) => "alternative".to_string(),
        GrammarExpr::Sequence(items) => format!("seq:{:?}", expr_suffix_key(items)),
        GrammarExpr::Empty => "empty".to_string(),
        GrammarExpr::External { .. } => "external".to_string(),
    }
}

fn ungroup_expr(expr: &GrammarExpr) -> &GrammarExpr {
    match expr {
        GrammarExpr::Group(inner) => ungroup_expr(inner),
        _ => expr,
    }
}

fn alternative_items(expr: &GrammarExpr) -> Option<&[GrammarExpr]> {
    match expr {
        GrammarExpr::Alternative(items) => Some(items),
        GrammarExpr::Group(inner) => alternative_items(inner),
        _ => None,
    }
}

fn contains_required_node(items: &[GrammarExpr]) -> bool {
    items.iter().any(|item| {
        matches!(
            ungroup_expr(item),
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
        )
    })
}

#[derive(Debug, Clone, Copy)]
struct AlternativeDispatchBranch<'a> {
    node: &'a GrammarExpr,
    tail: &'a [GrammarExpr],
}

#[derive(Debug, Clone)]
struct SuffixDispatchBranch {
    nodes: Vec<GrammarExpr>,
    tail: Vec<GrammarExpr>,
}

#[derive(Debug, Clone, Copy)]
struct NodeEmitFallbackBranch<'a> {
    node: &'a GrammarExpr,
    matched_emit: &'a EmitDirective,
    fallback_emit: &'a EmitDirective,
}

#[derive(Debug, Clone, Copy)]
struct NodeAcceptOrEmitSuffix<'a> {
    node: &'a GrammarExpr,
    emit: &'a EmitDirective,
    suffix: &'a GrammarExpr,
}

#[derive(Debug, Clone, Copy)]
struct NodeAcceptOrEmitFallback<'a> {
    node: &'a GrammarExpr,
    fallback_emit: &'a EmitDirective,
}

#[derive(Debug, Clone, Copy)]
struct NodeBodyAcceptingFallback<'a> {
    node: &'a GrammarExpr,
    body: &'a [GrammarExpr],
    fallback: &'a GrammarExpr,
}

#[derive(Debug, Clone)]
struct AcceptingNodesEmitSuffixFallback<'a> {
    accepting_nodes: Vec<&'a GrammarExpr>,
    branches: Vec<AlternativeDispatchBranch<'a>>,
    fallback_emit: &'a EmitDirective,
    fallback_suffix: &'a GrammarExpr,
}

#[derive(Debug, Clone, Copy)]
struct SelectorOptionalAcceptFallback<'a> {
    primary_node: &'a GrammarExpr,
    selector_node: &'a GrammarExpr,
    selector_operand: &'a GrammarExpr,
    selector_emit: &'a EmitDirective,
    default_emit: &'a EmitDirective,
    optional_fallback_node: &'a GrammarExpr,
    fallback_terminal: &'a GrammarExpr,
}

#[derive(Debug, Clone, Copy)]
struct PrintItemDispatch<'a> {
    end_print: &'a GrammarExpr,
    tab_token: &'a GrammarExpr,
    spc_token: &'a GrammarExpr,
    arg_node: &'a GrammarExpr,
    tab_emit: &'a EmitDirective,
    spc_emit: &'a EmitDirective,
    comma_token: &'a GrammarExpr,
    semicolon_token: &'a GrammarExpr,
    comma_emit: Option<&'a EmitDirective>,
    semicolon_emit: &'a EmitDirective,
    exp_node: &'a GrammarExpr,
    item_comma_emit: &'a EmitDirective,
    item_semi_emit: &'a EmitDirective,
    end_print_exp: &'a GrammarExpr,
}

#[derive(Debug, Clone)]
struct PriorityEmitDispatch<'a> {
    branches: Vec<PriorityEmitBranch<'a>>,
    fallback: PriorityEmitFallback<'a>,
}

#[derive(Debug, Clone)]
struct PriorityEmitBranch<'a> {
    node: &'a GrammarExpr,
    body: PriorityEmitBody<'a>,
}

#[derive(Debug, Clone)]
enum PriorityEmitBody<'a> {
    Sequence(&'a [GrammarExpr]),
    NestedNodeFallback {
        matched_items: &'a [GrammarExpr],
        fallback_emit: &'a EmitDirective,
    },
}

#[derive(Debug, Clone)]
enum PriorityEmitFallback<'a> {
    Emit(&'a EmitDirective),
    NestedFinalEmit {
        first_node: &'a GrammarExpr,
        first_matched_node: &'a GrammarExpr,
        first_fallback_emit: &'a EmitDirective,
        second_prefix: Vec<GrammarExpr>,
        final_emit: &'a EmitDirective,
    },
}

fn split_print_item_dispatch(alternatives: &[GrammarExpr]) -> Option<PrintItemDispatch<'_>> {
    match alternatives {
        [end_print, tab, spc, comma, semicolon, exp] => {
            let (tab_token, tab_arg, tab_emit) = split_token_arg_emit_arm(tab, "tkTAB")?;
            let (spc_token, spc_arg, spc_emit) = split_token_arg_emit_arm(spc, "tkSPC")?;
            if ungroup_expr(tab_arg) != ungroup_expr(spc_arg) {
                return None;
            }
            let (comma_token, comma_emit) = split_token_emit_arm(comma, "tkComma")?;
            let (semicolon_token, semicolon_emit) = split_token_emit_arm(semicolon, "tkSColon")?;
            let exp_arm = split_print_exp_arm(exp)?;
            if ungroup_expr(exp_arm.comma_token) != ungroup_expr(comma_token)
                || ungroup_expr(exp_arm.semicolon_token) != ungroup_expr(semicolon_token)
            {
                return None;
            }
            Some(PrintItemDispatch {
                end_print: print_end_node(end_print)?,
                tab_token,
                spc_token,
                arg_node: ungroup_expr(tab_arg),
                tab_emit,
                spc_emit,
                comma_token,
                semicolon_token,
                comma_emit: Some(comma_emit),
                semicolon_emit,
                exp_node: exp_arm.exp_node,
                item_comma_emit: exp_arm.comma_emit?,
                item_semi_emit: exp_arm.semi_emit,
                end_print_exp: exp_arm.end_print_exp,
            })
        }
        [end_print, tab_spc, exp] => {
            let tab_spc = split_tab_spc_optional_separator_arm(tab_spc)?;
            let exp_arm = split_print_exp_arm(exp)?;
            Some(PrintItemDispatch {
                end_print: print_end_node(end_print)?,
                tab_token: tab_spc.tab_token,
                spc_token: tab_spc.spc_token,
                arg_node: tab_spc.arg_node,
                tab_emit: tab_spc.tab_emit,
                spc_emit: tab_spc.spc_emit,
                comma_token: exp_arm.comma_token,
                semicolon_token: exp_arm.semicolon_token,
                comma_emit: None,
                semicolon_emit: exp_arm.semi_emit,
                exp_node: exp_arm.exp_node,
                item_comma_emit: exp_arm.semi_emit,
                item_semi_emit: exp_arm.semi_emit,
                end_print_exp: exp_arm.end_print_exp,
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct PrintExpArm<'a> {
    exp_node: &'a GrammarExpr,
    comma_token: &'a GrammarExpr,
    semicolon_token: &'a GrammarExpr,
    comma_emit: Option<&'a EmitDirective>,
    semi_emit: &'a EmitDirective,
    end_print_exp: &'a GrammarExpr,
}

fn split_print_exp_arm(expr: &GrammarExpr) -> Option<PrintExpArm<'_>> {
    let alternatives = alternative_items(expr)?;
    match alternatives {
        [exp_comma, semicolon, end_print_exp] => {
            let GrammarExpr::Sequence(exp_comma_items) = ungroup_expr(exp_comma) else {
                return None;
            };
            let [exp_node, comma] = exp_comma_items.as_slice() else {
                return None;
            };
            if !matches!(ungroup_expr(exp_node), GrammarExpr::NonTerminalRef(name) if name == "Exp")
            {
                return None;
            }
            let (comma_token, comma_emit) = split_token_emit_arm(comma, "tkComma")?;
            let (semicolon_token, semi_emit) = split_token_emit_arm(semicolon, "tkSColon")?;
            Some(PrintExpArm {
                exp_node: ungroup_expr(exp_node),
                comma_token,
                semicolon_token,
                comma_emit: Some(comma_emit),
                semi_emit,
                end_print_exp: print_end_exp_node(end_print_exp)?,
            })
        }
        [exp_separator, end_print_exp] => {
            let GrammarExpr::Sequence(exp_separator_items) = ungroup_expr(exp_separator) else {
                return None;
            };
            let [exp_node, separator] = exp_separator_items.as_slice() else {
                return None;
            };
            if !matches!(ungroup_expr(exp_node), GrammarExpr::NonTerminalRef(name) if name == "Exp")
            {
                return None;
            }
            let (comma_token, semicolon_token, semi_emit) = split_separator_group_emit(separator)?;
            Some(PrintExpArm {
                exp_node: ungroup_expr(exp_node),
                comma_token,
                semicolon_token,
                comma_emit: None,
                semi_emit,
                end_print_exp: print_end_exp_node(end_print_exp)?,
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct TabSpcOptionalSeparatorArm<'a> {
    tab_token: &'a GrammarExpr,
    spc_token: &'a GrammarExpr,
    arg_node: &'a GrammarExpr,
    tab_emit: &'a EmitDirective,
    spc_emit: &'a EmitDirective,
}

fn split_tab_spc_optional_separator_arm(
    expr: &GrammarExpr,
) -> Option<TabSpcOptionalSeparatorArm<'_>> {
    let GrammarExpr::Sequence(items) = ungroup_expr(expr) else {
        return None;
    };
    let [tab_spc_alternative, GrammarExpr::Optional(optional)] = items.as_slice() else {
        return None;
    };
    if !matches!(alternative_items(optional)?, [GrammarExpr::TokenRef(_), GrammarExpr::TokenRef(_)])
    {
        return None;
    }
    let [tab, spc] = alternative_items(tab_spc_alternative)? else {
        return None;
    };
    let (tab_token, tab_arg, tab_emit) = split_token_arg_emit_arm(tab, "tkTAB")?;
    let (spc_token, spc_arg, spc_emit) = split_token_arg_emit_arm(spc, "tkSPC")?;
    if ungroup_expr(tab_arg) != ungroup_expr(spc_arg) {
        return None;
    }
    Some(TabSpcOptionalSeparatorArm {
        tab_token,
        spc_token,
        arg_node: ungroup_expr(tab_arg),
        tab_emit,
        spc_emit,
    })
}

fn split_token_arg_emit_arm<'a>(
    expr: &'a GrammarExpr,
    token_name: &str,
) -> Option<(&'a GrammarExpr, &'a GrammarExpr, &'a EmitDirective)> {
    let GrammarExpr::Sequence(items) = ungroup_expr(expr) else {
        return None;
    };
    let [token_expr @ GrammarExpr::TokenRef(token), arg, GrammarExpr::Emit(emit)] = items.as_slice()
    else {
        return None;
    };
    if token != token_name || !is_node_expr(arg) {
        return None;
    }
    Some((ungroup_expr(token_expr), ungroup_expr(arg), emit))
}

fn split_token_emit_arm<'a>(
    expr: &'a GrammarExpr,
    token_name: &str,
) -> Option<(&'a GrammarExpr, &'a EmitDirective)> {
    let GrammarExpr::Sequence(items) = ungroup_expr(expr) else {
        return None;
    };
    let [token_expr @ GrammarExpr::TokenRef(token), GrammarExpr::Emit(emit)] = items.as_slice() else {
        return None;
    };
    (token == token_name).then_some((ungroup_expr(token_expr), emit))
}

fn split_separator_group_emit(
    expr: &GrammarExpr,
) -> Option<(&GrammarExpr, &GrammarExpr, &EmitDirective)> {
    let GrammarExpr::Sequence(items) = ungroup_expr(expr) else {
        return None;
    };
    let [separator_alternative, GrammarExpr::Emit(emit)] = items.as_slice() else {
        return None;
    };
    let [comma_expr @ GrammarExpr::TokenRef(comma), semicolon_expr @ GrammarExpr::TokenRef(semicolon)] =
        alternative_items(separator_alternative)?
    else {
        return None;
    };
    if comma != "tkComma" || semicolon != "tkSColon" {
        return None;
    }
    Some((ungroup_expr(comma_expr), ungroup_expr(semicolon_expr), emit))
}

fn print_end_node(expr: &GrammarExpr) -> Option<&GrammarExpr> {
    match ungroup_expr(expr) {
        node @ GrammarExpr::NonTerminalRef(name) if name == "EndPrint" => Some(node),
        _ => None,
    }
}

fn print_end_exp_node(expr: &GrammarExpr) -> Option<&GrammarExpr> {
    match ungroup_expr(expr) {
        node @ GrammarExpr::NonTerminalRef(name) if name == "EndPrintExp" => Some(node),
        _ => None,
    }
}

fn split_selector_with_optional_accept_fallback(
    alternatives: &[GrammarExpr],
) -> Option<SelectorOptionalAcceptFallback<'_>> {
    let [selected, fallback] = alternatives else {
        return None;
    };
    let GrammarExpr::Sequence(selected_items) = ungroup_expr(selected) else {
        return None;
    };
    let [primary_node, selector_alternative] = selected_items.as_slice() else {
        return None;
    };
    if !is_node_expr(primary_node) {
        return None;
    }
    let [selector_arm, default_arm] = alternative_items(selector_alternative)? else {
        return None;
    };
    let GrammarExpr::Emit(default_emit) = ungroup_expr(default_arm) else {
        return None;
    };
    let GrammarExpr::Sequence(selector_items) = ungroup_expr(selector_arm) else {
        return None;
    };
    let [
        selector_node,
        selector_operand,
        GrammarExpr::Emit(selector_emit),
    ] = selector_items.as_slice()
    else {
        return None;
    };
    if !is_node_expr(selector_node) || !is_node_expr(selector_operand) {
        return None;
    }
    let GrammarExpr::Sequence(fallback_items) = ungroup_expr(fallback) else {
        return None;
    };
    let [
        GrammarExpr::Optional(optional_fallback_node),
        fallback_terminal,
    ] = fallback_items.as_slice()
    else {
        return None;
    };
    if !is_node_expr(optional_fallback_node) || !is_node_expr(fallback_terminal) {
        return None;
    }
    Some(SelectorOptionalAcceptFallback {
        primary_node: ungroup_expr(primary_node),
        selector_node: ungroup_expr(selector_node),
        selector_operand: ungroup_expr(selector_operand),
        selector_emit,
        default_emit,
        optional_fallback_node: ungroup_expr(optional_fallback_node),
        fallback_terminal: ungroup_expr(fallback_terminal),
    })
}

fn split_accepting_nodes_with_emit_suffix_fallback(
    alternatives: &[GrammarExpr],
) -> Option<AcceptingNodesEmitSuffixFallback<'_>> {
    if alternatives.len() < 3 {
        return None;
    }
    let (fallback, prefix) = alternatives.split_last()?;
    let GrammarExpr::Sequence(fallback_items) = ungroup_expr(fallback) else {
        return None;
    };
    let [GrammarExpr::Emit(fallback_emit), fallback_suffix] = fallback_items.as_slice() else {
        return None;
    };
    if !is_node_expr(fallback_suffix) {
        return None;
    }

    let mut accepting_nodes = Vec::new();
    let mut branches = Vec::new();
    for alternative in prefix {
        match ungroup_expr(alternative) {
            node if is_node_expr(node) => accepting_nodes.push(node),
            GrammarExpr::Sequence(items) => {
                let (node, tail) = items.split_first()?;
                if !is_node_expr(node)
                    || tail.is_empty()
                    || !tail.iter().all(|item| matches!(item, GrammarExpr::Emit(_)))
                {
                    return None;
                }
                branches.push(AlternativeDispatchBranch {
                    node: ungroup_expr(node),
                    tail,
                });
            }
            _ => return None,
        }
    }
    if accepting_nodes.is_empty() || branches.is_empty() {
        return None;
    }
    Some(AcceptingNodesEmitSuffixFallback {
        accepting_nodes,
        branches,
        fallback_emit,
        fallback_suffix: ungroup_expr(fallback_suffix),
    })
}

fn split_node_body_with_accepting_fallback(
    alternatives: &[GrammarExpr],
) -> Option<NodeBodyAcceptingFallback<'_>> {
    let [body_arm, fallback] = alternatives else {
        return None;
    };
    if !is_node_expr(fallback) {
        return None;
    }
    let GrammarExpr::Sequence(body_items) = ungroup_expr(body_arm) else {
        return None;
    };
    let (node, body) = body_items.split_first()?;
    if body.is_empty() || !is_node_expr(node) || !matches!(body.last(), Some(GrammarExpr::Emit(_))) {
        return None;
    }
    Some(NodeBodyAcceptingFallback {
        node: ungroup_expr(node),
        body,
        fallback: ungroup_expr(fallback),
    })
}

fn split_node_accept_or_emit_fallback(
    alternatives: &[GrammarExpr],
) -> Option<NodeAcceptOrEmitFallback<'_>> {
    let [node, fallback] = alternatives else {
        return None;
    };
    if !is_node_expr(node) {
        return None;
    }
    let GrammarExpr::Emit(fallback_emit) = ungroup_expr(fallback) else {
        return None;
    };
    Some(NodeAcceptOrEmitFallback {
        node: ungroup_expr(node),
        fallback_emit,
    })
}

fn split_node_accept_or_emit_suffix(
    alternatives: &[GrammarExpr],
) -> Option<NodeAcceptOrEmitSuffix<'_>> {
    let [node, fallback] = alternatives else {
        return None;
    };
    if !is_node_expr(node) {
        return None;
    }
    let GrammarExpr::Sequence(fallback_items) = ungroup_expr(fallback) else {
        return None;
    };
    let [GrammarExpr::Emit(emit), suffix] = fallback_items.as_slice() else {
        return None;
    };
    if !is_node_expr(suffix) {
        return None;
    }
    Some(NodeAcceptOrEmitSuffix {
        node: ungroup_expr(node),
        emit,
        suffix: ungroup_expr(suffix),
    })
}

fn split_priority_emit_dispatch(alternatives: &[GrammarExpr]) -> Option<PriorityEmitDispatch<'_>> {
    let (fallback, branch_alternatives) = alternatives.split_last()?;
    if branch_alternatives.len() < 2 {
        return None;
    }
    let fallback = priority_emit_fallback(fallback)?;

    let branches = branch_alternatives
        .iter()
        .map(priority_emit_branch)
        .collect::<Option<Vec<_>>>()?;
    Some(PriorityEmitDispatch {
        branches,
        fallback,
    })
}

fn priority_emit_fallback(expr: &GrammarExpr) -> Option<PriorityEmitFallback<'_>> {
    if let GrammarExpr::Emit(emit) = ungroup_expr(expr) {
        return Some(PriorityEmitFallback::Emit(emit));
    }
    let alternatives = alternative_items(expr)?;
    let [matched, fallback] = alternatives else {
        return None;
    };
    let GrammarExpr::Sequence(matched_items) = ungroup_expr(matched) else {
        return None;
    };
    let GrammarExpr::Sequence(fallback_items) = ungroup_expr(fallback) else {
        return None;
    };
    let (GrammarExpr::Emit(final_emit), fallback_prefix) = fallback_items.split_last()? else {
        return None;
    };
    let (first_node, matched_body) = matched_items.split_first()?;
    if !is_node_expr(first_node) {
        return None;
    }
    let [nested] = matched_body else {
        return None;
    };
    let nested_alternative = alternative_items(nested)?;
    let [nested_matched, nested_fallback] = nested_alternative else {
        return None;
    };
    let GrammarExpr::Emit(nested_fallback_emit) = ungroup_expr(nested_fallback) else {
        return None;
    };
    if !is_node_expr(nested_matched) {
        return None;
    }
    let second_prefix = flatten_single_group_sequence(fallback_prefix);
    if second_prefix.is_empty()
        || !is_node_expr(second_prefix.first()?)
        || !second_prefix.last().is_some_and(is_node_expr)
    {
        return None;
    }
    Some(PriorityEmitFallback::NestedFinalEmit {
        first_node: ungroup_expr(first_node),
        first_matched_node: ungroup_expr(nested_matched),
        first_fallback_emit: nested_fallback_emit,
        second_prefix,
        final_emit,
    })
}

fn priority_emit_branch(expr: &GrammarExpr) -> Option<PriorityEmitBranch<'_>> {
    if let Some(branch) = priority_emit_sequence_branch(expr) {
        return Some(branch);
    }

    let alternatives = alternative_items(expr)?;
    let [matched, fallback] = alternatives else {
        return None;
    };
    let GrammarExpr::Emit(fallback_emit) = ungroup_expr(fallback) else {
        return None;
    };
    let GrammarExpr::Sequence(items) = ungroup_expr(matched) else {
        return None;
    };
    let (node, body) = items.split_first()?;
    if !matches!(
        ungroup_expr(node),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) {
        return None;
    }
    Some(PriorityEmitBranch {
        node: ungroup_expr(node),
        body: PriorityEmitBody::NestedNodeFallback {
            matched_items: body,
            fallback_emit,
        },
    })
}

fn priority_emit_sequence_branch(expr: &GrammarExpr) -> Option<PriorityEmitBranch<'_>> {
    let GrammarExpr::Sequence(items) = ungroup_expr(expr) else {
        return None;
    };
    let (node, body) = items.split_first()?;
    if body.is_empty()
        || !matches!(
            ungroup_expr(node),
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
        )
        || !matches!(body.last(), Some(GrammarExpr::Emit(_)))
    {
        return None;
    }
    Some(PriorityEmitBranch {
        node: ungroup_expr(node),
        body: PriorityEmitBody::Sequence(body),
    })
}

#[derive(Debug, Clone)]
struct SuffixSequenceAcceptingNodes<'a> {
    node: &'a GrammarExpr,
    body: &'a [GrammarExpr],
    suffix: &'a GrammarExpr,
    accepting_nodes: Vec<&'a GrammarExpr>,
}

fn split_suffix_sequence_with_accepting_nodes(
    alternatives: &[GrammarExpr],
) -> Option<SuffixSequenceAcceptingNodes<'_>> {
    if alternatives.len() < 2 {
        return None;
    }

    let mut sequence_branch = None;
    let mut accepting_nodes = Vec::new();
    for alternative in alternatives {
        match ungroup_expr(alternative) {
            GrammarExpr::Sequence(items) if items.len() >= 3 => {
                if sequence_branch.is_some() {
                    return None;
                }
                let (suffix, prefix) = items.split_last()?;
                let (node, body) = prefix.split_first()?;
                if !matches!(
                    ungroup_expr(node),
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
                ) || !matches!(
                    ungroup_expr(suffix),
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
                ) {
                    return None;
                }
                sequence_branch = Some((ungroup_expr(node), body, ungroup_expr(suffix)));
            }
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                accepting_nodes.push(ungroup_expr(alternative));
            }
            _ => return None,
        }
    }

    let (node, body, suffix) = sequence_branch?;
    if accepting_nodes.is_empty() {
        return None;
    }
    Some(SuffixSequenceAcceptingNodes {
        node,
        body,
        suffix,
        accepting_nodes,
    })
}

fn split_node_emit_with_emit_fallback(
    alternatives: &[GrammarExpr],
) -> Option<NodeEmitFallbackBranch<'_>> {
    let [matched, fallback] = alternatives else {
        return None;
    };
    let GrammarExpr::Sequence(items) = ungroup_expr(matched) else {
        return None;
    };
    let [node, GrammarExpr::Emit(matched_emit)] = items.as_slice() else {
        return None;
    };
    if !matches!(
        ungroup_expr(node),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) {
        return None;
    }
    let GrammarExpr::Emit(fallback_emit) = ungroup_expr(fallback) else {
        return None;
    };
    Some(NodeEmitFallbackBranch {
        node: ungroup_expr(node),
        matched_emit,
        fallback_emit,
    })
}

fn suffix_dispatch_branches(alternatives: &[GrammarExpr]) -> Option<Vec<SuffixDispatchBranch>> {
    if alternatives.len() < 2 {
        return None;
    }

    alternatives
        .iter()
        .map(|alternative| match ungroup_expr(alternative) {
            GrammarExpr::Sequence(items) => {
                let (first, tail) = items.split_first()?;
                let nodes = match ungroup_expr(first) {
                    GrammarExpr::Alternative(nodes) => alternative_dispatch_branches(nodes)?
                        .into_iter()
                        .filter_map(|branch| {
                            if branch.tail.is_empty() {
                                Some(branch.node.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                        vec![ungroup_expr(first).clone()]
                    }
                    _ => return None,
                };
                if nodes.is_empty() {
                    return None;
                }
                if tail.is_empty() {
                    return None;
                }
                Some(SuffixDispatchBranch {
                    nodes,
                    tail: tail.to_vec(),
                })
            }
            _ => None,
        })
        .collect()
}

fn trailing_suffix_dispatch_branches(
    alternatives: &[GrammarExpr],
) -> Option<(Vec<SuffixDispatchBranch>, &GrammarExpr)> {
    if alternatives.len() < 2 {
        return None;
    }
    let (last, prefix_alternatives) = alternatives.split_last()?;
    let last_items = sequence_items(last)?;
    let (suffix, last_prefix) = last_items.split_last()?;
    if !matches!(
        ungroup_expr(suffix),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) {
        return None;
    }

    let mut normalized = prefix_alternatives.to_vec();
    normalized.push(sequence_from_items(flatten_single_group_sequence(last_prefix)));
    suffix_dispatch_branches(&normalized).map(|branches| (branches, ungroup_expr(suffix)))
}

fn sequence_items(expr: &GrammarExpr) -> Option<&[GrammarExpr]> {
    match ungroup_expr(expr) {
        GrammarExpr::Sequence(items) => Some(items),
        _ => None,
    }
}

fn flatten_single_group_sequence(items: &[GrammarExpr]) -> Vec<GrammarExpr> {
    let [item] = items else {
        return items.to_vec();
    };
    sequence_items(item).map_or_else(|| items.to_vec(), ToOwned::to_owned)
}

fn sequence_from_items(items: Vec<GrammarExpr>) -> GrammarExpr {
    GrammarExpr::Sequence(items)
}

fn alternative_dispatch_branches(
    alternatives: &[GrammarExpr],
) -> Option<Vec<AlternativeDispatchBranch<'_>>> {
    if alternatives.len() < 2 {
        return None;
    }

    alternatives
        .iter()
        .map(|alternative| match ungroup_expr(alternative) {
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                Some(AlternativeDispatchBranch {
                    node: ungroup_expr(alternative),
                    tail: &[],
                })
            }
            GrammarExpr::Sequence(items) => {
                let (node, tail) = items.split_first()?;
                match ungroup_expr(node) {
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                        Some(AlternativeDispatchBranch {
                            node: ungroup_expr(node),
                            tail,
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

fn split_node_arm_with_fallback(
    alternatives: &[GrammarExpr],
) -> Option<(AlternativeDispatchBranch<'_>, &GrammarExpr)> {
    let [first, second] = alternatives else {
        return None;
    };
    let first = match ungroup_expr(first) {
        GrammarExpr::Sequence(items) => {
            let (node, tail) = items.split_first()?;
            match ungroup_expr(node) {
                GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                    if tail.is_empty() {
                        return None;
                    }
                    AlternativeDispatchBranch {
                        node: ungroup_expr(node),
                        tail,
                    }
                }
                _ => return None,
            }
        }
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => return None,
        _ => return None,
    };

    match ungroup_expr(second) {
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => None,
        _ => Some((first, second)),
    }
}

fn split_node_arms_with_fallback(
    alternatives: &[GrammarExpr],
) -> Option<(Vec<AlternativeDispatchBranch<'_>>, &GrammarExpr)> {
    let (fallback, branch_alternatives) = alternatives.split_last()?;
    if branch_alternatives.len() < 2 {
        return None;
    }
    if matches!(
        ungroup_expr(fallback),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) {
        return None;
    }

    let branches = branch_alternatives
        .iter()
        .map(|alternative| match ungroup_expr(alternative) {
            GrammarExpr::Sequence(items) => {
                let (node, tail) = items.split_first()?;
                if tail.is_empty() {
                    return None;
                }
                match ungroup_expr(node) {
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                        Some(AlternativeDispatchBranch {
                            node: ungroup_expr(node),
                            tail,
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    Some((branches, fallback))
}

fn is_next_statement_alternative(alternatives: &[GrammarExpr]) -> bool {
    let [id_arm, default_arm] = alternatives else {
        return false;
    };

    expr_contains_emit_ident(id_arm, "opStNextId")
        && expr_contains_emit_ident(default_arm, "opStNext")
        && expr_contains_repeat(id_arm)
}

fn expr_contains_repeat(expr: &GrammarExpr) -> bool {
    match expr {
        GrammarExpr::Repeat(_) => true,
        GrammarExpr::Sequence(items) | GrammarExpr::Alternative(items) => {
            items.iter().any(expr_contains_repeat)
        }
        GrammarExpr::Group(inner) | GrammarExpr::Optional(inner) => expr_contains_repeat(inner),
        _ => false,
    }
}

fn is_on_statement_alternative(alternatives: &[GrammarExpr]) -> bool {
    let [event_arm, error_arm, exp_arm] = alternatives else {
        return false;
    };

    expr_starts_with(event_arm, "event")
        && expr_contains_emit_ident(event_arm, "opEvGosub")
        && expr_starts_with(error_arm, "tkERROR")
        && expr_contains_emit_ident(error_arm, "opStOnError")
        && expr_contains_nonterminal(exp_arm, "Exp")
        && expr_contains_mark_slot(exp_arm, 1)
        && expr_contains_mark_slot(exp_arm, 2)
}

fn expr_contains_nonterminal(expr: &GrammarExpr, name: &str) -> bool {
    match expr {
        GrammarExpr::NonTerminalRef(candidate) => candidate == name,
        GrammarExpr::Sequence(items) | GrammarExpr::Alternative(items) => {
            items.iter().any(|item| expr_contains_nonterminal(item, name))
        }
        GrammarExpr::Group(inner) | GrammarExpr::Optional(inner) | GrammarExpr::Repeat(inner) => {
            expr_contains_nonterminal(inner, name)
        }
        _ => false,
    }
}

fn expr_starts_with(expr: &GrammarExpr, name: &str) -> bool {
    let first = match ungroup_expr(expr) {
        GrammarExpr::Sequence(items) => items.first().map(ungroup_expr),
        other => Some(other),
    };
    matches!(
        first,
        Some(GrammarExpr::TokenRef(token) | GrammarExpr::NonTerminalRef(token)) if token == name
    )
}

fn is_node_expr(expr: &GrammarExpr) -> bool {
    matches!(
        ungroup_expr(expr),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    )
}

fn split_shared_tail_alternative(
    alternatives: &[GrammarExpr],
) -> Option<(Vec<GrammarExpr>, Vec<GrammarExpr>)> {
    let [first, second] = alternatives else {
        return None;
    };
    let GrammarExpr::Sequence(second_items) = ungroup_expr(second) else {
        return None;
    };
    let suffix_start = second_items.iter().position(|item| {
        matches!(
            ungroup_expr(item),
            GrammarExpr::NonTerminalRef(name) if name == "ACTIONidCommon"
        )
    })?;
    if suffix_start == 0 {
        return None;
    }

    let second_prefix = second_items[..suffix_start].to_vec();
    let suffix = second_items[suffix_start..].to_vec();
    Some((
        vec![first.clone(), GrammarExpr::Sequence(second_prefix)],
        suffix,
    ))
}

fn is_common_statement_alternative(alternatives: &[GrammarExpr]) -> bool {
    let [first, second] = alternatives else {
        return false;
    };
    let first_items = match ungroup_expr(first) {
        GrammarExpr::Sequence(items) => items.as_slice(),
        _ => return false,
    };
    let second_items = match ungroup_expr(second) {
        GrammarExpr::Sequence(items) => items.as_slice(),
        _ => return false,
    };

    matches!(first_items.first(), Some(GrammarExpr::TokenRef(token)) if token == "tkSHARED")
        && second_items.iter().any(|item| {
            matches!(
                ungroup_expr(item),
                GrammarExpr::NonTerminalRef(name) if name == "ACTIONidCommon"
            )
        })
}
