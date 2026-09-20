//! Host-side `buildprs` generation boundary.
//!
//! This module is intentionally split from `build.rs`: tests can exercise the
//! reverse-engineered generator directly, and `build.rs` can later call the same
//! code once grammar-backed state lowering reaches full parity.

use std::collections::BTreeMap;

use crate::buildprs_dispatch::{derive_nonterminal_dispatch_order, NonterminalDispatchOrder};
use crate::buildprs_emit::out_state;
use crate::buildprs_grammar::{GrammarExpr, GrammarFile, TokenDef};
use crate::buildprs_graph::{NodeId, OptLevel, StateGraph};
use crate::buildprs_integrate::integrate;
use crate::buildprs_layout::sort_state_values;
use crate::buildprs_lowering::{
    lower_whole_grammar_with_shared_suffixes, qbasic_11_shared_suffix_registry, LoweringConfig,
    LoweringSymbols,
};
use crate::buildprs_tokens::{self, TokenArtifacts};

pub const ENCODE1BYTE_QBASIC_11: u8 = 224;
pub const IRW_ALPHA_FIRST_QBASIC_11: u16 = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTokenArtifacts {
    pub irw_equates: BTreeMap<String, u16>,
    pub irw_to_char: Vec<u8>,
    pub irw_to_iop: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedDispatchTables {
    pub int_nt_disp: Vec<u16>,
    pub ext_nt_disp: Vec<String>,
    pub ext_nt_help: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedParserTables {
    pub state: Vec<u8>,
    pub dispatch: GeneratedDispatchTables,
    /// Offsets into `state` for each statement anchor keyword name (e.g. "BEEP" → 0).
    /// Only stores the FIRST offset when multiple rules share an anchor.
    pub statement_offsets: BTreeMap<String, u16>,
    /// All (anchor, offset) pairs in grammar order.  Multiple entries may share the same
    /// anchor when two grammar rules start with the same keyword (e.g. file GET vs graphics GET).
    pub statement_offset_list: Vec<(String, u16)>,
    /// Offsets into `state` for each function anchor keyword name.
    pub function_offsets: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateError {
    ArtifactParse(String),
    MissingEquate(String),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArtifactParse(message) => write!(f, "artifact parse error: {message}"),
            Self::MissingEquate(symbol) => write!(f, "missing required equate {symbol}"),
        }
    }
}

impl std::error::Error for GenerateError {}

pub fn generate_token_artifacts(grammar: &GrammarFile) -> GeneratedTokenArtifacts {
    generate_token_artifacts_from_decls(&grammar.tokens)
}

/// Derive internal/external nonterminal ordering and external help table from grammar.
///
/// Internal `int_nt_disp` offsets are left at zero until state lowering assigns each NT's
/// `tState` byte start (see [`NonterminalDispatchOrder`] and `STI_*` equates in `prstab.inc`).
pub fn generate_dispatch_order_from_grammar(grammar: &GrammarFile) -> GeneratedDispatchTables {
    dispatch_tables_from_order(&derive_nonterminal_dispatch_order(grammar))
}

/// Generate QBasic 1.1 runtime parser tables directly from `qbasbnf.prs`.
pub fn generate_tables_from_grammar(
    grammar: &GrammarFile,
    peropcod: &str,
) -> Result<GeneratedParserTables, GenerateError> {
    let config = lowering_config_for_grammar(grammar);
    let symbols = LoweringSymbols::from_qbasic_11_grammar(grammar, peropcod);
    let shared_suffixes = qbasic_11_shared_suffix_registry(grammar, &symbols, &config);
    let lowered =
        lower_whole_grammar_with_shared_suffixes(grammar, symbols, config, shared_suffixes);

    if lowered.report.unsupported_shapes.total_failures() != 0 {
        return Err(GenerateError::ArtifactParse(format!(
            "grammar lowering left unsupported shapes: {:?}",
            lowered.report.unsupported_shapes
        )));
    }

    let dispatch_order = derive_nonterminal_dispatch_order(grammar);
    let int_nt_disp = dispatch_order
        .internal
        .iter()
        .map(|entry| {
            lowered
                .int_nt_disp
                .get(&entry.name)
                .copied()
                .ok_or_else(|| GenerateError::MissingEquate(format!("STI_{}", entry.name)))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GeneratedParserTables {
        state: lowered.state,
        dispatch: GeneratedDispatchTables {
            int_nt_disp,
            ext_nt_disp: dispatch_order
                .external
                .iter()
                .map(|entry| entry.dispatch_symbol.clone())
                .collect(),
            ext_nt_help: dispatch_order
                .external
                .iter()
                .map(|entry| entry.help_symbol.clone())
                .collect(),
        },
        statement_offsets: lowered.statement_offsets,
        statement_offset_list: lowered.statement_offset_list,
        function_offsets: lowered.function_offsets,
    })
}

/// Generate parser tables through the DOS-style graph backend.
///
/// This is intentionally parallel to [`generate_tables_from_grammar`]. The
/// existing pattern backend remains the default production path until graph
/// parity is proven.
pub fn generate_tables_from_graph(
    grammar: &GrammarFile,
    peropcod: &str,
    opt_level: OptLevel,
) -> Result<GeneratedParserTables, GenerateError> {
    let config = lowering_config_for_grammar(grammar);
    let symbols = LoweringSymbols::from_qbasic_11_grammar(grammar, peropcod);
    let _shared_suffixes = qbasic_11_shared_suffix_registry(grammar, &symbols, &config);
    let mut graph = StateGraph::new();
    let mut internal_roots = BTreeMap::<String, NodeId>::new();
    let mut shared_suffixes = crate::buildprs_actions::GraphSharedSuffixes::default();
    let use_shared_suffixes = opt_level.combines_states();

    for nt in &grammar.nonterminals {
        if nt.external {
            continue;
        }
        let Some(production) = nt.body.productions.first() else {
            continue;
        };
        let root = crate::buildprs_actions::compile_expr_to_graph_with_suffixes(
            &mut graph,
            &symbols,
            &config,
            &normalize_grouped_alternative_tail(&production.expr),
            use_shared_suffixes.then_some(&mut shared_suffixes),
            use_shared_suffixes,
        )
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
        internal_roots.insert(nt.name.clone(), root);
    }

    for rule in &grammar.statements.rules {
        let expr = statement_expr(rule);
        let root = crate::buildprs_actions::compile_expr_to_graph_with_suffixes(
            &mut graph,
            &symbols,
            &config,
            &expr,
            use_shared_suffixes.then_some(&mut shared_suffixes),
            false,
        )
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
        integrate(&mut graph, Some(root), &rule.anchor, opt_level)
            .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
    }

    for rule in &grammar.functions.rules {
        let expr = statement_expr(rule);
        let root = crate::buildprs_actions::compile_expr_to_graph_with_suffixes(
            &mut graph,
            &symbols,
            &config,
            &expr,
            use_shared_suffixes.then_some(&mut shared_suffixes),
            false,
        )
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
        integrate(&mut graph, Some(root), &rule.anchor, opt_level)
            .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
    }

    for nt in &grammar.nonterminals {
        if nt.external {
            continue;
        }
        let Some(root) = internal_roots.get(&nt.name).copied() else {
            continue;
        };
        let integrated = integrate(&mut graph, Some(root), &nt.name, opt_level)
            .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?
            .unwrap_or(root);
        internal_roots.insert(nt.name.clone(), integrated);
    }

    sort_state_values(&mut graph);
    let state =
        out_state(&graph).map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
    let dispatch_order = derive_nonterminal_dispatch_order(grammar);
    let int_nt_disp = dispatch_order
        .internal
        .iter()
        .map(|entry| {
            internal_roots
                .get(&entry.name)
                .map(|root| graph.node(*root).sort_index as u16)
                .unwrap_or(0)
        })
        .collect();

    Ok(GeneratedParserTables {
        state,
        dispatch: GeneratedDispatchTables {
            int_nt_disp,
            ext_nt_disp: dispatch_order
                .external
                .iter()
                .map(|entry| entry.dispatch_symbol.clone())
                .collect(),
            ext_nt_help: dispatch_order
                .external
                .iter()
                .map(|entry| entry.help_symbol.clone())
                .collect(),
        },
        statement_offsets: BTreeMap::new(),
        statement_offset_list: Vec::new(),
        function_offsets: BTreeMap::new(),
    })
}

fn lowering_config_for_grammar(grammar: &GrammarFile) -> LoweringConfig {
    let dispatch_order = derive_nonterminal_dispatch_order(grammar);
    LoweringConfig {
        encode1byte: ENCODE1BYTE_QBASIC_11,
        num_nt_int: dispatch_order.internal.len() as u16,
        num_nt_ext: dispatch_order.external.len() as u16,
    }
}

fn statement_expr(
    rule: &crate::buildprs_grammar::GrammarRule,
) -> crate::buildprs_grammar::GrammarExpr {
    normalize_grouped_alternative_tail(&rule.production.expr)
}

fn normalize_grouped_alternative_tail(expr: &GrammarExpr) -> GrammarExpr {
    match expr {
        GrammarExpr::Alternative(items) => {
            let normalized_items = items
                .iter()
                .map(normalize_grouped_alternative_tail)
                .collect::<Vec<_>>();
            if let Some((last, previous)) = normalized_items.split_last() {
                if let GrammarExpr::Sequence(sequence) = last {
                    if let Some((GrammarExpr::Group(group), tail)) = sequence.split_first() {
                        if !tail.is_empty() {
                            let mut alternatives = previous.to_vec();
                            alternatives.push(normalize_grouped_alternative_tail(group));
                            let mut items = vec![GrammarExpr::Alternative(alternatives)];
                            items.extend(tail.iter().cloned());
                            return normalize_prefixed_alternative_sequence(items);
                        }
                    }
                }
            }
            normalize_prefixed_alternative(normalized_items)
        }
        GrammarExpr::Sequence(items) => {
            let normalized_items = items
                .iter()
                .map(normalize_grouped_alternative_tail)
                .collect::<Vec<_>>();
            normalize_prefixed_alternative_sequence(normalized_items)
        }
        GrammarExpr::Group(inner) => {
            GrammarExpr::Group(Box::new(normalize_grouped_alternative_tail(inner)))
        }
        GrammarExpr::Optional(inner) => {
            GrammarExpr::Optional(Box::new(normalize_grouped_alternative_tail(inner)))
        }
        GrammarExpr::Repeat(inner) => {
            GrammarExpr::Repeat(Box::new(normalize_grouped_alternative_tail(inner)))
        }
        other => other.clone(),
    }
}

fn normalize_prefixed_alternative(alternatives: Vec<GrammarExpr>) -> GrammarExpr {
    let Some((GrammarExpr::Sequence(first_sequence), other_alternatives)) =
        alternatives.split_first()
    else {
        return GrammarExpr::Alternative(alternatives);
    };
    let Some((prefix, first_tail)) = first_sequence.split_first() else {
        return GrammarExpr::Alternative(alternatives);
    };
    if first_tail.is_empty() || !is_node_expr(prefix) {
        return GrammarExpr::Alternative(alternatives);
    }
    if !matches!(first_tail.first(), Some(GrammarExpr::Group(_))) {
        return GrammarExpr::Alternative(alternatives);
    }
    if other_alternatives.iter().any(is_mark_only_expr) {
        return GrammarExpr::Alternative(alternatives);
    }

    let mut sequence = Vec::with_capacity(2);
    sequence.push(prefix.clone());
    let mut nested_alternatives = Vec::with_capacity(other_alternatives.len() + 1);
    nested_alternatives.push(sequence_expr(first_tail));
    nested_alternatives.extend(other_alternatives.iter().cloned());
    sequence.push(GrammarExpr::Alternative(nested_alternatives));
    GrammarExpr::Sequence(sequence)
}

fn normalize_prefixed_alternative_sequence(items: Vec<GrammarExpr>) -> GrammarExpr {
    let Some((GrammarExpr::Alternative(alternatives), tail)) = items.split_first() else {
        return GrammarExpr::Sequence(items);
    };
    let Some((GrammarExpr::Sequence(first_sequence), other_alternatives)) =
        alternatives.split_first()
    else {
        return GrammarExpr::Sequence(items);
    };
    let Some((prefix, first_tail)) = first_sequence.split_first() else {
        return GrammarExpr::Sequence(items);
    };
    if first_tail.is_empty() || !is_node_expr(prefix) {
        return GrammarExpr::Sequence(items);
    }
    if !matches!(first_tail.first(), Some(GrammarExpr::Group(_))) {
        return GrammarExpr::Sequence(items);
    }
    if other_alternatives.iter().any(is_mark_only_expr) {
        return GrammarExpr::Sequence(items);
    }

    let mut sequence = Vec::with_capacity(tail.len() + 2);
    sequence.push(prefix.clone());
    let mut nested_alternatives = Vec::with_capacity(other_alternatives.len() + 1);
    nested_alternatives.push(sequence_expr(first_tail));
    nested_alternatives.extend(other_alternatives.iter().cloned());
    sequence.push(GrammarExpr::Alternative(nested_alternatives));
    sequence.extend(tail.iter().cloned());
    GrammarExpr::Sequence(sequence)
}

fn sequence_expr(items: &[GrammarExpr]) -> GrammarExpr {
    match items {
        [item] => item.clone(),
        _ => GrammarExpr::Sequence(items.to_vec()),
    }
}

fn is_node_expr(expr: &GrammarExpr) -> bool {
    matches!(
        ungroup_expr(expr),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    )
}

fn is_mark_only_expr(expr: &GrammarExpr) -> bool {
    matches!(single_expr(expr), GrammarExpr::Mark(_))
}

fn single_expr(expr: &GrammarExpr) -> &GrammarExpr {
    match ungroup_expr(expr) {
        GrammarExpr::Sequence(items) if items.len() == 1 => single_expr(&items[0]),
        expr => expr,
    }
}

fn ungroup_expr(expr: &GrammarExpr) -> &GrammarExpr {
    match expr {
        GrammarExpr::Group(inner) => ungroup_expr(inner),
        other => other,
    }
}

fn dispatch_tables_from_order(order: &NonterminalDispatchOrder) -> GeneratedDispatchTables {
    GeneratedDispatchTables {
        int_nt_disp: order
            .internal
            .iter()
            .map(|entry| entry.state_offset.unwrap_or(0))
            .collect(),
        ext_nt_disp: order
            .external
            .iter()
            .map(|entry| entry.dispatch_symbol.clone())
            .collect(),
        ext_nt_help: order
            .external
            .iter()
            .map(|entry| entry.help_symbol.clone())
            .collect(),
    }
}

/// Generate token artifacts directly from parsed `TOKENS:` declarations.
pub fn generate_token_artifacts_from_decls(tokens: &[TokenDef]) -> GeneratedTokenArtifacts {
    token_artifacts_from_generated(&buildprs_tokens::generate_token_artifacts(tokens))
}

fn token_artifacts_from_generated(artifacts: &TokenArtifacts) -> GeneratedTokenArtifacts {
    GeneratedTokenArtifacts {
        irw_equates: artifacts.irw_ids.clone(),
        irw_to_char: artifacts.mp_irw_to_char.clone(),
        irw_to_iop: artifacts
            .mp_irw_to_iop
            .iter()
            .map(|value| iop_symbol_from_byte(*value))
            .collect(),
    }
}

fn iop_symbol_from_byte(value: u8) -> Option<String> {
    match value {
        0xFF => None,
        1 => Some("IOP_RParen".to_string()),
        2 => Some("IOP_Imp".to_string()),
        3 => Some("IOP_Eqv".to_string()),
        4 => Some("IOP_Xor".to_string()),
        5 => Some("IOP_Or".to_string()),
        6 => Some("IOP_And".to_string()),
        7 => Some("IOP_Not".to_string()),
        8 => Some("IOP_EQ".to_string()),
        9 => Some("IOP_LT".to_string()),
        10 => Some("IOP_GT".to_string()),
        14 => Some("IOP_Add".to_string()),
        15 => Some("IOP_Minus".to_string()),
        16 => Some("IOP_Mod".to_string()),
        17 => Some("IOP_Idiv".to_string()),
        18 => Some("IOP_Mult".to_string()),
        19 => Some("IOP_Div".to_string()),
        22 => Some("IOP_Pwr".to_string()),
        23 => Some("IOP_LParen".to_string()),
        other => Some(format!("IOP_{other}")),
    }
}

/// Parse the captured MASM artifact into the runtime tables `NtParse` consumes.
///
/// This is the current compatibility bridge. The grammar-backed lowering pass
/// replaces this by producing the same `GeneratedParserTables` directly.
pub fn generate_tables_from_prsstate(
    prsstate: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<GeneratedParserTables, GenerateError> {
    let state = parse_db_dw_state_bytes(prsstate, "tState", opcode_equates)?;
    let int_nt_disp = parse_dw_numbers(prsstate, "tIntNtDisp")?;
    let ext_nt_disp = parse_dw_symbols(prsstate, "tExtNtDisp")?;
    let ext_nt_help = parse_dw_symbols(prsstate, "tExtNtHelp")?;

    Ok(GeneratedParserTables {
        state,
        dispatch: GeneratedDispatchTables {
            int_nt_disp,
            ext_nt_disp,
            ext_nt_help,
        },
        statement_offsets: BTreeMap::new(),
        statement_offset_list: Vec::new(),
        function_offsets: BTreeMap::new(),
    })
}

pub fn parse_opcode_equates_from_peropcod(source: &str) -> BTreeMap<String, u16> {
    source
        .lines()
        .filter_map(parse_opcode_line)
        .enumerate()
        .map(|(index, opcode)| (opcode, index as u16))
        .collect()
}

fn parse_opcode_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with('@') || trimmed.is_empty() {
        return None;
    }

    let (name, _) = trimmed.split_once('|')?;
    let name = name.trim();
    if name.starts_with("op") {
        Some(name.to_string())
    } else {
        None
    }
}

fn parse_db_dw_state_bytes(
    text: &str,
    label: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<Vec<u8>, GenerateError> {
    let mut bytes = Vec::new();
    for (directive, operands) in data_lines_in_section(text, label) {
        match directive.as_str() {
            "db" => {
                for operand in operands {
                    bytes.push(parse_byte_operand(&operand, opcode_equates)?);
                }
            }
            "dw" => {
                for operand in operands {
                    bytes.extend(parse_word_operand(&operand, opcode_equates)?.to_le_bytes());
                }
            }
            _ => {}
        }
    }
    Ok(bytes)
}

fn parse_dw_numbers(text: &str, label: &str) -> Result<Vec<u16>, GenerateError> {
    data_lines_in_section(text, label)
        .into_iter()
        .filter(|(directive, _)| directive == "dw")
        .flat_map(|(_, operands)| operands)
        .map(|operand| parse_word_literal(&operand))
        .collect()
}

fn parse_dw_symbols(text: &str, label: &str) -> Result<Vec<String>, GenerateError> {
    data_lines_in_section(text, label)
        .into_iter()
        .filter(|(directive, _)| directive == "dw")
        .flat_map(|(_, operands)| operands)
        .map(|operand| Ok(operand.trim().to_string()))
        .collect()
}

fn data_lines_in_section(text: &str, label: &str) -> Vec<(String, Vec<String>)> {
    let mut in_section = false;
    let mut lines = Vec::new();

    for line in text.lines() {
        let trimmed = strip_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if !in_section {
            if label_declares(trimmed, label) {
                in_section = true;
            }
            continue;
        }

        if is_label_declaration(trimmed) || trimmed.eq_ignore_ascii_case("sEnd\tCP") {
            break;
        }

        let mut fields = trimmed.splitn(2, char::is_whitespace);
        let directive = fields.next().unwrap_or_default().to_ascii_lowercase();
        if directive != "db" && directive != "dw" {
            continue;
        }
        let operands = fields.next().map(split_data_items).unwrap_or_default();
        lines.push((directive, operands));
    }

    lines
}

fn label_declares(line: &str, label: &str) -> bool {
    let mut fields = line.split_whitespace();
    matches!(
        (fields.next(), fields.next()),
        (Some(name), Some(keyword))
            if name.eq_ignore_ascii_case(label) && keyword.eq_ignore_ascii_case("label")
    )
}

fn is_label_declaration(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    matches!(fields.nth(1), Some(keyword) if keyword.eq_ignore_ascii_case("label"))
}

fn split_data_items(data: &str) -> Vec<String> {
    data.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn strip_comment(line: &str) -> &str {
    line.split_once(';')
        .map(|(source, _)| source)
        .unwrap_or(line)
}

fn parse_byte_operand(
    operand: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<u8, GenerateError> {
    let value = parse_word_operand(operand, opcode_equates)?;
    if value <= u8::MAX as u16 {
        Ok(value as u8)
    } else {
        Err(GenerateError::ArtifactParse(format!(
            "byte operand is out of range: {operand}"
        )))
    }
}

fn parse_word_operand(
    operand: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<u16, GenerateError> {
    if let Some(value) = lookup_opcode_equate(opcode_equates, operand.trim()) {
        return Ok(*value);
    }
    eval_word_expression(operand, opcode_equates).map_err(|error| {
        GenerateError::ArtifactParse(format!("failed to evaluate `{operand}`: {error}"))
    })
}

fn parse_word_literal(operand: &str) -> Result<u16, GenerateError> {
    let literal = operand.trim();
    if let Some(stem) = literal
        .strip_suffix('H')
        .or_else(|| literal.strip_suffix('h'))
    {
        return u16::from_str_radix(stem, 16)
            .map_err(|error| GenerateError::ArtifactParse(error.to_string()));
    }
    literal
        .parse::<u16>()
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))
}

fn eval_word_expression(
    expression: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<u16, GenerateError> {
    let value = eval_sum(expression.trim(), opcode_equates)?;
    if (0..=u16::MAX as i64).contains(&value) {
        Ok(value as u16)
    } else {
        Err(GenerateError::ArtifactParse(format!(
            "word expression is out of range: {expression}"
        )))
    }
}

fn eval_sum(
    expression: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<i64, GenerateError> {
    let parts = split_top_level(expression, '+');
    if parts.len() > 1 {
        return parts
            .into_iter()
            .map(|part| eval_product(part.trim(), opcode_equates))
            .sum();
    }
    eval_product(expression, opcode_equates)
}

fn eval_product(
    expression: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<i64, GenerateError> {
    let parts = split_top_level(expression, '*');
    if parts.len() > 1 {
        return parts
            .into_iter()
            .map(|part| eval_factor(part.trim(), opcode_equates))
            .try_fold(1, |acc, value| value.map(|value| acc * value));
    }
    eval_factor(expression, opcode_equates)
}

fn eval_factor(
    expression: &str,
    opcode_equates: &BTreeMap<String, u16>,
) -> Result<i64, GenerateError> {
    let expression = expression.trim();
    if let Some(inner) = strip_outer_parens(expression) {
        return eval_sum(inner, opcode_equates);
    }
    if let Some(value) = known_word_symbol(expression, opcode_equates) {
        return Ok(value);
    }
    parse_word_literal(expression).map(i64::from)
}

fn known_word_symbol(symbol: &str, opcode_equates: &BTreeMap<String, u16>) -> Option<i64> {
    match symbol {
        _ if symbol.eq_ignore_ascii_case("OPCODE_MASK") => Some(0x03ff),
        _ if symbol.eq_ignore_ascii_case("UNDEFINED") => Some(0xffff),
        _ if symbol.eq_ignore_ascii_case("ET_IMP") => Some(0),
        _ if symbol.eq_ignore_ascii_case("ET_I2") => Some(1),
        _ if symbol.eq_ignore_ascii_case("ET_I4") => Some(2),
        _ if symbol.eq_ignore_ascii_case("ET_R4") => Some(3),
        _ if symbol.eq_ignore_ascii_case("ET_R8") => Some(4),
        _ if symbol.eq_ignore_ascii_case("ET_SD") => Some(5),
        _ if symbol.eq_ignore_ascii_case("ET_FS") => Some(6),
        _ => lookup_opcode_equate(opcode_equates, symbol)
            .copied()
            .map(i64::from),
    }
}

fn lookup_opcode_equate<'a>(
    opcode_equates: &'a BTreeMap<String, u16>,
    symbol: &str,
) -> Option<&'a u16> {
    opcode_equates.get(symbol).or_else(|| {
        opcode_equates
            .iter()
            .find_map(|(name, value)| name.eq_ignore_ascii_case(symbol).then_some(value))
    })
}

fn split_top_level(expression: &str, separator: char) -> Vec<&str> {
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();

    for (idx, ch) in expression.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if ch == separator && depth == 0 => {
                parts.push(&expression[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(&expression[start..]);
    parts
}

fn strip_outer_parens(expression: &str) -> Option<&str> {
    if !expression.starts_with('(') || !expression.ends_with(')') {
        return None;
    }

    let mut depth = 0;
    for (idx, ch) in expression.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && idx != expression.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }

    Some(&expression[1..expression.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildprs_artifacts::parse_irw_equates;
    use crate::buildprs_artifacts::ArtifactSet;
    use crate::buildprs_dispatch::parse_golden_dispatch_tables;
    use crate::buildprs_grammar::{parse_grammar_file, parse_token_decls_file};

    fn grammar_path() -> &'static str {
        "../../src/frontend/qb/grammar/qbasbnf.prs"
    }

    fn peropcod_source() -> String {
        std::fs::read_to_string("../../src/frontend/qb/grammar/peropcod.txt")
            .expect("vendored peropcod.txt should read")
    }

    #[test]
    fn token_artifacts_match_irw_order_and_special_chars() {
        let tokens = parse_token_decls_file(grammar_path()).expect("qbasbnf tokens should parse");
        let generated = generate_token_artifacts_from_decls(&tokens);
        let golden = parse_irw_equates(include_str!(
            "../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsirw.inc"
        ))
        .expect("golden prsirw should parse");

        assert_eq!(generated.irw_equates.len(), 246);
        assert_eq!(
            generated.irw_equates["IRW_PRINT"],
            golden["IRW_PRINT"] as u16
        );
        assert_eq!(generated.irw_equates["IRW_NewLine"], 7);
        assert_eq!(generated.irw_to_char[0], b'%');
        assert_eq!(generated.irw_to_char[7], 0x0A);
        assert_eq!(generated.irw_to_iop[6].as_deref(), Some("IOP_Idiv"));
    }

    #[test]
    fn fixture_bridge_decodes_runtime_tables_from_prsstate() {
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
        let generated = generate_tables_from_prsstate(
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm"),
            &opcodes,
        )
        .expect("prsstate bridge should decode");

        assert_eq!(generated.state.len(), 2941);
        assert_eq!(generated.dispatch.int_nt_disp.len(), 29);
        assert_eq!(generated.dispatch.ext_nt_disp.len(), 49);
        assert_eq!(generated.dispatch.ext_nt_help.len(), 49);
        assert_eq!(generated.dispatch.ext_nt_disp[0], "NtACTIONidCommon");
    }

    #[test]
    fn fixture_bridge_decodes_default_mode_runtime_tables_from_prsstate() {
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
        let default_mode = generate_tables_from_prsstate(
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1-o0/prsstate.asm"),
            &opcodes,
        )
        .expect("default-mode prsstate bridge should decode");
        let optimized = generate_tables_from_prsstate(
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm"),
            &opcodes,
        )
        .expect("optimized prsstate bridge should decode");

        assert!(default_mode.state.len() > optimized.state.len());
        assert_eq!(default_mode.dispatch.int_nt_disp.len(), 29);
        assert_eq!(default_mode.dispatch.ext_nt_disp.len(), 49);
        assert_eq!(default_mode.dispatch.ext_nt_help.len(), 49);
        assert_eq!(
            default_mode.dispatch.ext_nt_disp,
            optimized.dispatch.ext_nt_disp
        );
        assert_eq!(
            default_mode.dispatch.ext_nt_help,
            optimized.dispatch.ext_nt_help
        );
    }

    #[test]
    fn default_mode_artifacts_satisfy_buildprs_contract() {
        let artifacts = ArtifactSet::new(
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1-o0/prstab.inc"),
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1-o0/prsirw.inc"),
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1-o0/prsstate.asm"),
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1-o0/prsrwt.asm"),
        )
        .validate()
        .expect("default-mode artifacts should validate");

        assert_eq!(artifacts.prstab_equates["ENCODE1BYTE"], 224);
        assert_eq!(artifacts.prstab_equates["NUMNTINT"], 29);
        assert_eq!(artifacts.prstab_equates["NUMNTEXT"], 49);
        assert_eq!(artifacts.prsirw_equates.len(), 246);
        assert!(artifacts.prsirw_equates.contains_key("IRW_PRINT"));
    }

    #[test]
    fn grammar_backed_generator_emits_qbasic_11_runtime_tables() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let generated = generate_tables_from_grammar(&grammar, &peropcod_source())
            .expect("grammar-backed lowering should generate parser tables");

        assert_eq!(generated.state.len(), 2941);
        assert_eq!(generated.dispatch.int_nt_disp.len(), 29);
        assert_eq!(generated.dispatch.ext_nt_disp.len(), 49);
        assert_eq!(generated.dispatch.ext_nt_help.len(), 49);
        assert_eq!(generated.dispatch.ext_nt_disp[0], "NtACTIONidCommon");
    }

    #[test]
    fn graph_backend_generates_runtime_tables_for_supported_snippet() {
        let grammar = crate::buildprs_grammar::parse_grammar(
            r#"TOKENS:
   tkBEEP ("BEEP"),

Statements:
   tkBEEP EMIT(opStBeep);

Functions:

NonTerminals:
AsClausePrim:
   tkBEEP;
"#,
        )
        .expect("snippet should parse");
        let generated = generate_tables_from_graph(&grammar, "opStBeep|x\n", OptLevel::O1)
            .expect("graph backend should generate tables");

        assert!(!generated.state.is_empty());
        assert_eq!(&generated.state[..4], &[3, 0, 0, 0]);
        assert_eq!(generated.dispatch.int_nt_disp.len(), 1);
        assert!(generated.dispatch.ext_nt_disp.is_empty());
        assert!(generated.dispatch.int_nt_disp[0] < generated.state.len() as u16);
    }

    #[test]
    fn graph_backend_opt_levels_preserve_dispatch_and_do_not_increase_o2_size() {
        let grammar = crate::buildprs_grammar::parse_grammar(
            r#"TOKENS:
   tkBEEP ("BEEP"),

Statements:
   tkBEEP EMIT(opStBeep);

Functions:

NonTerminals:
First:
   tkBEEP EMIT(opStBeep);
Second:
   tkBEEP EMIT(opStBeep);
"#,
        )
        .expect("snippet should parse");
        let peropcod = "opStBeep|x\n";
        let o0 = generate_tables_from_graph(&grammar, peropcod, OptLevel::O0)
            .expect("O0 graph backend should generate tables");
        let o1 = generate_tables_from_graph(&grammar, peropcod, OptLevel::O1)
            .expect("O1 graph backend should generate tables");
        let o2 = generate_tables_from_graph(&grammar, peropcod, OptLevel::O2)
            .expect("O2 graph backend should generate tables");

        assert_eq!(o0.dispatch.int_nt_disp.len(), o1.dispatch.int_nt_disp.len());
        assert_eq!(o1.dispatch.int_nt_disp.len(), o2.dispatch.int_nt_disp.len());
        assert_eq!(o0.dispatch.ext_nt_disp, o1.dispatch.ext_nt_disp);
        assert_eq!(o1.dispatch.ext_nt_disp, o2.dispatch.ext_nt_disp);
        assert_eq!(o0.dispatch.ext_nt_help, o1.dispatch.ext_nt_help);
        assert_eq!(o1.dispatch.ext_nt_help, o2.dispatch.ext_nt_help);
        assert!(o1.state.len() <= o0.state.len());
        assert!(o2.state.len() <= o1.state.len());
    }

    #[test]
    #[ignore = "graph action lowering is not complete enough for full qbasbnf parity yet"]
    fn graph_backend_o1_matches_qbasic_11_prsstate_fixture() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
        let golden = generate_tables_from_prsstate(
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm"),
            &opcodes,
        )
        .expect("golden prsstate should decode");

        let generated = generate_tables_from_graph(&grammar, &peropcod_source(), OptLevel::O1)
            .expect("graph backend should generate full qbasbnf tables");

        assert_eq!(generated.dispatch.ext_nt_disp, golden.dispatch.ext_nt_disp);
        assert_eq!(generated.dispatch.ext_nt_help, golden.dispatch.ext_nt_help);
        assert_eq!(
            generated.dispatch.int_nt_disp.len(),
            golden.dispatch.int_nt_disp.len()
        );
        assert_state_bytes_match(&generated.state, &golden.state);
        assert_eq!(generated.dispatch.int_nt_disp, golden.dispatch.int_nt_disp);
    }

    #[test]
    #[ignore = "diagnostic: prints all graph-vs-DOS O1 parity diffs"]
    fn graph_backend_o1_reports_all_diffs_against_qbasic_11_prsstate_fixture() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
        let golden_source =
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm");
        let golden = generate_tables_from_prsstate(golden_source, &opcodes)
            .expect("golden prsstate should decode");
        let generated = generate_tables_from_graph(&grammar, &peropcod_source(), OptLevel::O1)
            .expect("graph backend should generate full qbasbnf tables");
        let golden_dispatch = parse_golden_dispatch_tables(golden_source);

        report_graph_backend_diffs(&grammar, &generated, &golden, &golden_dispatch);
    }

    #[test]
    #[ignore = "diagnostic: prints all graph-vs-DOS O2 parity diffs against the O1 semantic baseline"]
    fn graph_backend_o2_reports_all_diffs_against_qbasic_11_prsstate_fixture() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
        let golden_source =
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm");
        let golden = generate_tables_from_prsstate(golden_source, &opcodes)
            .expect("golden prsstate should decode");
        let generated = generate_tables_from_graph(&grammar, &peropcod_source(), OptLevel::O2)
            .expect("graph backend should generate full qbasbnf tables");
        let golden_dispatch = parse_golden_dispatch_tables(golden_source);

        report_graph_backend_diffs(&grammar, &generated, &golden, &golden_dispatch);
    }

    #[test]
    #[ignore = "diagnostic: prints all graph-vs-DOS O0 parity diffs"]
    fn graph_backend_o0_reports_all_diffs_against_qbasic_11_default_prsstate_fixture() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
        let golden_source =
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1-o0/prsstate.asm");
        let golden = generate_tables_from_prsstate(golden_source, &opcodes)
            .expect("golden default-mode prsstate should decode");
        let generated = generate_tables_from_graph(&grammar, &peropcod_source(), OptLevel::O0)
            .expect("graph backend should generate full qbasbnf tables");
        let golden_dispatch = parse_golden_dispatch_tables(golden_source);

        report_graph_backend_diffs(&grammar, &generated, &golden, &golden_dispatch);
    }

    fn report_graph_backend_diffs(
        grammar: &crate::buildprs_grammar::GrammarFile,
        generated: &GeneratedParserTables,
        golden: &GeneratedParserTables,
        golden_dispatch: &crate::buildprs_dispatch::GoldenDispatchTables,
    ) {
        println!(
            "state lengths: generated={}, golden={}, delta={}",
            generated.state.len(),
            golden.state.len(),
            generated.state.len() as isize - golden.state.len() as isize
        );
        println!(
            "external dispatch symbols match: {}",
            generated.dispatch.ext_nt_disp == golden.dispatch.ext_nt_disp
        );
        println!(
            "external help symbols match: {}",
            generated.dispatch.ext_nt_help == golden.dispatch.ext_nt_help
        );

        println!("internal dispatch offset diffs:");
        let mut internal_offset_diff_count = 0;
        for (index, (generated_offset, golden_offset)) in generated
            .dispatch
            .int_nt_disp
            .iter()
            .zip(&golden.dispatch.int_nt_disp)
            .enumerate()
        {
            if generated_offset != golden_offset {
                internal_offset_diff_count += 1;
                let name = golden_dispatch
                    .internal_names
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("<unknown>");
                println!(
                    "  {index:02} {name}: generated={generated_offset}, golden={golden_offset}, delta={}",
                    i32::from(*generated_offset) - i32::from(*golden_offset)
                );
            }
        }
        println!("internal dispatch offset diff count: {internal_offset_diff_count}");

        let ranges = byte_diff_ranges(&generated.state, &golden.state);
        println!("byte diff ranges: {}", ranges.len());
        for (index, (start, end)) in ranges.iter().enumerate() {
            println!("  {index:03}: {start}..{end} len={}", end - start);
        }

        println!("byte diff windows for first 20 ranges:");
        for (index, (start, end)) in ranges.iter().take(20).enumerate() {
            println!(
                "  {index:03}: {start}..{end} generated_window={:?} golden_window={:?}",
                diff_window(&generated.state, *start, *end),
                diff_window(&golden.state, *start, *end)
            );
        }

        let generated_entries = decode_state_entries(&generated.state);
        let golden_entries = decode_state_entries(&golden.state);
        println!(
            "decoded entry counts: generated={}, golden={}, delta={}",
            generated_entries.len(),
            golden_entries.len(),
            generated_entries.len() as isize - golden_entries.len() as isize
        );

        let compared_entries = generated_entries.len().min(golden_entries.len());
        let exact_entry_diffs = generated_entries
            .iter()
            .zip(&golden_entries)
            .filter(|((_, generated_entry), (_, golden_entry))| generated_entry != golden_entry)
            .count();
        let shape_entry_diffs = generated_entries
            .iter()
            .zip(&golden_entries)
            .filter(|((_, generated_entry), (_, golden_entry))| {
                entry_shape(generated_entry) != entry_shape(golden_entry)
            })
            .count();
        println!(
            "decoded entry diffs: exact={exact_entry_diffs}/{compared_entries}, shape={shape_entry_diffs}/{compared_entries}"
        );

        println!("first 60 decoded entry diffs:");
        let mut printed = 0;
        for (entry_index, ((generated_offset, generated_entry), (golden_offset, golden_entry))) in
            generated_entries.iter().zip(&golden_entries).enumerate()
        {
            if generated_entry == golden_entry {
                continue;
            }
            println!(
                "  entry {entry_index:03}: generated@{generated_offset} {generated_entry} | golden@{golden_offset} {golden_entry}"
            );
            printed += 1;
            if printed == 60 {
                break;
            }
        }

        let cg_hints = grammar_cg_hint_inventory(&grammar);
        println!("cg_hint inventory (all currently ignored by graph backend):");
        println!("  unique hint kinds: {}", cg_hints.len());
        println!(
            "  productions with hints: {}",
            cg_hints.iter().map(|(_, count)| count).sum::<usize>()
        );
        for (hint, count) in cg_hints {
            println!("  {hint}: {count}");
        }
    }

    fn assert_state_bytes_match(generated: &[u8], golden: &[u8]) {
        let first_diff = generated
            .iter()
            .zip(golden)
            .position(|(left, right)| left != right)
            .or_else(|| {
                (generated.len() != golden.len()).then_some(generated.len().min(golden.len()))
            });
        assert!(
            first_diff.is_none(),
            "state bytes differ: generated_len={}, golden_len={}, first_diff={:?}, generated_window={:?}, golden_window={:?}",
            generated.len(),
            golden.len(),
            first_diff,
            first_diff.map(|index| &generated[index.saturating_sub(8)..generated.len().min(index + 16)]),
            first_diff.map(|index| &golden[index.saturating_sub(8)..golden.len().min(index + 16)]),
        );
    }

    fn byte_diff_ranges(generated: &[u8], golden: &[u8]) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let max_len = generated.len().max(golden.len());
        let mut index = 0;
        while index < max_len {
            if generated.get(index) == golden.get(index) {
                index += 1;
                continue;
            }

            let start = index;
            while index < max_len && generated.get(index) != golden.get(index) {
                index += 1;
            }
            ranges.push((start, index));
        }
        ranges
    }

    fn diff_window(bytes: &[u8], start: usize, end: usize) -> &[u8] {
        let window_start = start.saturating_sub(8);
        let window_end = bytes.len().min(end + 16);
        &bytes[window_start..window_end]
    }

    fn decode_state_entries(bytes: &[u8]) -> Vec<(usize, String)> {
        let mut entries = Vec::new();
        let mut pc = 0;
        while pc < bytes.len() {
            let offset = pc;
            let Some(entry) = decode_state_entry(bytes, &mut pc) else {
                entries.push((offset, "truncated".to_string()));
                break;
            };
            entries.push((offset, entry));
        }
        entries
    }

    fn decode_state_entry(bytes: &[u8], pc: &mut usize) -> Option<String> {
        let first = read_diag_byte(bytes, pc)?;
        match first {
            0 => Some("accept".to_string()),
            1 => Some("reject".to_string()),
            2 => Some(format!("mark({})", read_diag_byte(bytes, pc)?)),
            3 => {
                let lo = read_diag_byte(bytes, pc)?;
                let hi = read_diag_byte(bytes, pc)?;
                Some(format!("emit({})", u16::from_le_bytes([lo, hi])))
            }
            _ => {
                *pc -= 1;
                let node_id = decode_diag_node_id(bytes, pc)?;
                let branch = decode_diag_branch(bytes, pc)?;
                if node_id == 4 {
                    Some(format!("branch({branch})"))
                } else {
                    Some(format!("node({node_id},{branch})"))
                }
            }
        }
    }

    fn read_diag_byte(bytes: &[u8], pc: &mut usize) -> Option<u8> {
        let value = *bytes.get(*pc)?;
        *pc += 1;
        Some(value)
    }

    fn decode_diag_node_id(bytes: &[u8], pc: &mut usize) -> Option<u16> {
        let first = read_diag_byte(bytes, pc)?;
        if first < ENCODE1BYTE_QBASIC_11 {
            return Some(u16::from(first));
        }
        let second = read_diag_byte(bytes, pc)?;
        Some(((u16::from(first) << 8) | u16::from(second)) - 255 * u16::from(ENCODE1BYTE_QBASIC_11))
    }

    fn decode_diag_branch(bytes: &[u8], pc: &mut usize) -> Option<String> {
        let first = read_diag_byte(bytes, pc)?;
        if first == 255 {
            return Some("accept".to_string());
        }
        let cursor_after = *pc as isize;
        if first < ENCODE1BYTE_QBASIC_11 {
            let target = if first < ENCODE1BYTE_QBASIC_11 / 2 {
                cursor_after + isize::from(first)
            } else {
                cursor_after + isize::from(first) - isize::from(ENCODE1BYTE_QBASIC_11)
            };
            return Some(format!("relative({target})"));
        }
        let second = read_diag_byte(bytes, pc)?;
        let raw = (u16::from(first) << 8) | u16::from(second);
        let target = i32::from(raw) - 255 * i32::from(ENCODE1BYTE_QBASIC_11);
        Some(format!("absolute({target})"))
    }

    fn entry_shape(entry: &str) -> String {
        if entry == "accept" || entry == "reject" || entry == "truncated" {
            return entry.to_string();
        }
        if entry.starts_with("mark(") {
            return "mark".to_string();
        }
        if entry.starts_with("emit(") {
            return "emit".to_string();
        }
        if entry.starts_with("branch(") {
            return "branch".to_string();
        }
        entry
            .split_once(',')
            .map_or_else(|| entry.to_string(), |(node, _)| format!("{node},branch)"))
    }

    fn grammar_cg_hint_inventory(
        grammar: &crate::buildprs_grammar::GrammarFile,
    ) -> Vec<(String, usize)> {
        let mut hints = BTreeMap::<String, usize>::new();
        for rule in grammar
            .statements
            .rules
            .iter()
            .chain(&grammar.functions.rules)
        {
            if let Some(hint) = &rule.production.cg_hint {
                *hints.entry(cg_hint_name(hint).to_string()).or_default() += 1;
            }
        }
        hints.into_iter().collect()
    }

    fn cg_hint_name(hint: &str) -> &str {
        hint.split_once('(').map_or(hint, |(name, _)| name)
    }

    #[test]
    fn grammar_derived_dispatch_names_and_help_match_prsstate_fixture() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let derived = generate_dispatch_order_from_grammar(&grammar);
        let golden = parse_golden_dispatch_tables(include_str!(
            "../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm"
        ));

        assert_eq!(derived.int_nt_disp.len(), golden.internal_names.len());
        assert_eq!(
            derived.ext_nt_disp.len(),
            golden.external_dispatch_symbols.len()
        );
        assert_eq!(
            derived.ext_nt_help.len(),
            golden.external_help_symbols.len()
        );
        assert_eq!(derived.ext_nt_disp, golden.external_dispatch_symbols);
        assert_eq!(derived.ext_nt_help, golden.external_help_symbols);
        assert_eq!(derived.int_nt_disp.len(), 29);
        assert_eq!(derived.ext_nt_disp.len(), 49);
    }

    #[test]
    fn generated_special_char_table_matches_prsrwt_fixture() {
        let tokens = parse_token_decls_file(grammar_path()).expect("qbasbnf tokens should parse");
        let generated = generate_token_artifacts_from_decls(&tokens);
        let golden = buildprs_tokens::parse_mp_byte_table(
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsrwt.asm"),
            "mpIRWtoChar",
        )
        .expect("golden char table should parse");

        assert_eq!(generated.irw_to_char, golden);
    }

    #[test]
    fn reserved_word_tables_match_golden_with_scaffold_lowering() {
        let tokens = parse_token_decls_file(grammar_path()).expect("qbasbnf tokens should parse");
        let artifacts = buildprs_tokens::generate_token_artifacts(&tokens);
        let prsrwt = include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsrwt.asm");
        let (lowering, codec) =
            buildprs_tokens::extract_rw_lowering_from_golden(prsrwt, &artifacts)
                .expect("golden lowering should parse");
        let tables = buildprs_tokens::generate_reserved_word_tables(&artifacts, &lowering, &codec);

        buildprs_tokens::validate_rw_tables_against_golden(
            &tables,
            include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsorw.inc"),
            prsrwt,
            &codec,
        )
        .expect("generated reserved-word tables should match golden fixtures");
    }

    #[test]
    fn check_irw_to_value() {
        use crate::buildprs_artifacts::parse_irw_equates;
        use crate::buildprs_grammar::parse_token_decls_file;
        let tokens = parse_token_decls_file("../../src/frontend/qb/grammar/qbasbnf.prs").unwrap();
        let generated = generate_token_artifacts_from_decls(&tokens);
        let golden = parse_irw_equates(include_str!(
            "../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsirw.inc"
        ))
        .unwrap();
        let gen_to = generated.irw_equates["IRW_TO"];
        let gold_to = golden["IRW_TO"] as u16;
        assert_eq!(
            gen_to, gold_to,
            "IRW_TO mismatch: generated={gen_to}, golden={gold_to}"
        );
    }
}

#[test]
fn check_irw_to_node_id() {
    use crate::buildprs_artifacts::parse_irw_equates;
    use crate::buildprs_grammar::parse_token_decls_file;
    let tokens = parse_token_decls_file("../../src/frontend/qb/grammar/qbasbnf.prs").unwrap();
    let generated = generate_token_artifacts_from_decls(&tokens);
    let irw_to = generated.irw_equates["IRW_TO"];
    let irw_for = generated.irw_equates["IRW_FOR"];
    // node_id formula: 5 + num_nt_int + num_nt_ext + irw = 83 + irw (for qbasic-11 with 29+49)
    let node_id_to = 5u16 + 29 + 49 + irw_to;
    let node_id_for = 5u16 + 29 + 49 + irw_for;
    panic!("IRW_TO={irw_to} node_id_to={node_id_to} | IRW_FOR={irw_for} node_id_for={node_id_for}");
}
