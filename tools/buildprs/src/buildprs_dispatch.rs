//! Derive nonterminal dispatch ordering from a typed grammar AST.
//!
//! `buildprs` splits `NonTerminals:` entries into two tables:
//! - Internal NTs (no `EXTERNAL` production) get `tIntNtDisp[id]` byte offsets into `tState`.
//! - External NTs get `tExtNtDisp[id]` function-pointer symbols (`Nt{Name}`) and parallel
//!   `tExtNtHelp[id]` message ids (`MSG_*` or `0`).
//!
//! Ordering follows grammar declaration order within each partition. Numeric internal offsets
//! are produced later by state lowering; this module only derives names, help hints, and counts.

use crate::buildprs_grammar::{GrammarFile, NonTerminalDef};

/// One internal nonterminal in `tIntNtDisp` emission order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalNonterminalEntry {
    pub name: String,
    /// Byte offset into `tState` where this NT's recursive-descent state begins.
    ///
    /// `NtParse` loads `child_pc = t_state[t_int_nt_disp[id] as usize]` before recursing.
    /// Populated by state lowering; `None` when only declaration order is known.
    pub state_offset: Option<u16>,
}

/// One external nonterminal in `tExtNtDisp` / `tExtNtHelp` emission order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalNonterminalEntry {
    /// Grammar name from `NonTerminals:` (e.g. `Assignment`).
    pub grammar_name: String,
    /// MASM symbol in `tExtNtDisp` (always `Nt` + grammar name).
    pub dispatch_symbol: String,
    /// Message id in `tExtNtHelp` (`MSG_*` hint or `"0"` when absent).
    pub help_symbol: String,
}

/// Grammar-derived dispatch metadata before state lowering fills internal offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonterminalDispatchOrder {
    pub internal: Vec<InternalNonterminalEntry>,
    pub external: Vec<ExternalNonterminalEntry>,
}

/// Dispatch tables captured from a golden `prsstate.asm` for parity checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenDispatchTables {
    pub internal_names: Vec<String>,
    pub internal_offsets: Vec<u16>,
    pub external_dispatch_symbols: Vec<String>,
    pub external_help_symbols: Vec<String>,
}

/// Partition `GrammarFile::nonterminals` into internal/external dispatch order.
pub fn derive_nonterminal_dispatch_order(grammar: &GrammarFile) -> NonterminalDispatchOrder {
    let mut internal = Vec::new();
    let mut external = Vec::new();

    for nt in &grammar.nonterminals {
        if nt.external {
            external.push(external_entry(nt));
        } else {
            internal.push(InternalNonterminalEntry {
                name: nt.name.clone(),
                state_offset: None,
            });
        }
    }

    NonterminalDispatchOrder { internal, external }
}

fn external_entry(nt: &NonTerminalDef) -> ExternalNonterminalEntry {
    ExternalNonterminalEntry {
        grammar_name: nt.name.clone(),
        dispatch_symbol: external_dispatch_symbol(&nt.name),
        help_symbol: external_help_symbol(nt.msg_hint.as_deref()),
    }
}

/// MASM `tExtNtDisp` symbol for a grammar nonterminal name.
pub fn external_dispatch_symbol(grammar_name: &str) -> String {
    format!("Nt{grammar_name}")
}

/// MASM `tExtNtHelp` operand for an external nonterminal.
pub fn external_help_symbol(msg_hint: Option<&str>) -> String {
    msg_hint.unwrap_or("0").to_string()
}

/// Parse golden `tIntNtDisp`, `tExtNtDisp`, and `tExtNtHelp` sections from `prsstate.asm`.
pub fn parse_golden_dispatch_tables(prsstate: &str) -> GoldenDispatchTables {
    GoldenDispatchTables {
        internal_names: parse_commented_dw_names(prsstate, "tIntNtDisp"),
        internal_offsets: parse_dw_number_operands(prsstate, "tIntNtDisp"),
        external_dispatch_symbols: parse_dw_symbol_operands(prsstate, "tExtNtDisp"),
        external_help_symbols: parse_dw_symbol_operands(prsstate, "tExtNtHelp"),
    }
}

fn parse_commented_dw_names(text: &str, label: &str) -> Vec<String> {
    dispatch_section_lines(text, label)
        .into_iter()
        .filter_map(|line| {
            line.comment_after_semicolon()
                .map(str::trim)
                .map(str::to_string)
        })
        .collect()
}

fn parse_dw_number_operands(text: &str, label: &str) -> Vec<u16> {
    dispatch_section_lines(text, label)
        .into_iter()
        .filter_map(|line| line.first_dw_operand().and_then(parse_u16_literal))
        .collect()
}

fn parse_dw_symbol_operands(text: &str, label: &str) -> Vec<String> {
    dispatch_section_lines(text, label)
        .into_iter()
        .filter_map(|line| line.first_dw_operand().map(str::trim).map(str::to_string))
        .collect()
}

fn parse_u16_literal(operand: &str) -> Option<u16> {
    let literal = operand.trim();
    if let Some(stem) = literal
        .strip_suffix('H')
        .or_else(|| literal.strip_suffix('h'))
    {
        return u16::from_str_radix(stem, 16).ok();
    }
    literal.parse::<u16>().ok()
}

fn dispatch_section_lines<'a>(text: &'a str, label: &str) -> Vec<DispatchLine<'a>> {
    let mut in_section = false;
    let mut lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
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
        let directive = fields.next().unwrap_or_default();
        if !directive.eq_ignore_ascii_case("dw") {
            continue;
        }

        let rest = fields.next().unwrap_or_default();
        lines.push(DispatchLine::parse(rest));
    }

    lines
}

#[derive(Debug, Clone, Copy)]
struct DispatchLine<'a> {
    operands: &'a str,
}

impl<'a> DispatchLine<'a> {
    fn parse(rest: &'a str) -> Self {
        Self { operands: rest }
    }

    fn comment_after_semicolon(&self) -> Option<&'a str> {
        self.operands.split_once(';').map(|(_, comment)| comment)
    }

    fn first_dw_operand(&self) -> Option<&'a str> {
        let data = self
            .operands
            .split_once(';')
            .map(|(before, _)| before)
            .unwrap_or(self.operands);
        data.split(',')
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildprs_grammar::parse_grammar;
    use crate::buildprs_grammar::parse_grammar_file;

    const SAMPLE: &str = r#"TOKENS:
   tkBEEP ("BEEP"),

Statements:
   tkBEEP EMIT(opStBeep);

Functions:

NonTerminals:

Assignment:
   EXTERNAL MSG_ExpAssignment;
AsClausePrim:
   ((tkINTEGER EMIT(opAsTypeExp) EMIT(ET_I2)) |
    (tkSTRING EMIT(opAsTypeExp) EMIT(ET_SD)));
    <INDEX>
caseItem:
   (Exp EMIT(opStCase));
Exp:
   EXTERNAL MSG_ExpExp;
"#;

    fn grammar_path() -> &'static str {
        "../../src/frontend/qb/grammar/qbasbnf.prs"
    }

    fn golden_prsstate() -> &'static str {
        include_str!("../../../frontends/qb/fixtures/buildprs/qbasic-1.1/prsstate.asm")
    }

    #[test]
    fn derive_partitions_internal_and_external_in_declaration_order() {
        let grammar = parse_grammar(SAMPLE).expect("sample grammar should parse");
        let order = derive_nonterminal_dispatch_order(&grammar);

        assert_eq!(order.internal.len(), 2);
        assert_eq!(order.internal[0].name, "AsClausePrim");
        assert!(order.internal[0].state_offset.is_none());
        assert_eq!(order.internal[1].name, "caseItem");

        assert_eq!(order.external.len(), 2);
        assert_eq!(order.external[0].grammar_name, "Assignment");
        assert_eq!(order.external[0].dispatch_symbol, "NtAssignment");
        assert_eq!(order.external[0].help_symbol, "MSG_ExpAssignment");
        assert_eq!(order.external[1].grammar_name, "Exp");
        assert_eq!(order.external[1].help_symbol, "MSG_ExpExp");
    }

    #[test]
    fn external_help_symbol_defaults_to_zero_without_msg_hint() {
        assert_eq!(external_help_symbol(None), "0");
        assert_eq!(external_help_symbol(Some("MSG_ExpExp")), "MSG_ExpExp");
        assert_eq!(external_dispatch_symbol("IdAry"), "NtIdAry");
    }

    #[test]
    fn golden_parser_extracts_dispatch_sections_from_fixture() {
        let golden = parse_golden_dispatch_tables(golden_prsstate());

        assert_eq!(golden.internal_names.len(), 29);
        assert_eq!(golden.internal_offsets.len(), 29);
        assert_eq!(golden.external_dispatch_symbols.len(), 49);
        assert_eq!(golden.external_help_symbols.len(), 49);

        assert_eq!(golden.internal_names[0], "AsClausePrim");
        assert_eq!(golden.internal_offsets[0], 2463);
        assert_eq!(golden.external_dispatch_symbols[0], "NtACTIONidCommon");
        assert_eq!(golden.external_help_symbols[0], "0");
        assert_eq!(golden.external_help_symbols[3], "MSG_ExpAssignment");
    }

    #[test]
    fn real_grammar_dispatch_order_matches_prsstate_fixture() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let derived = derive_nonterminal_dispatch_order(&grammar);
        let golden = parse_golden_dispatch_tables(golden_prsstate());

        assert_eq!(derived.internal.len(), golden.internal_names.len());
        assert_eq!(
            derived.external.len(),
            golden.external_dispatch_symbols.len()
        );

        let derived_internal_names: Vec<_> = derived
            .internal
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(derived_internal_names, golden.internal_names);

        let derived_dispatch: Vec<_> = derived
            .external
            .iter()
            .map(|entry| entry.dispatch_symbol.as_str())
            .collect();
        assert_eq!(derived_dispatch, golden.external_dispatch_symbols);

        let derived_help: Vec<_> = derived
            .external
            .iter()
            .map(|entry| entry.help_symbol.as_str())
            .collect();
        assert_eq!(derived_help, golden.external_help_symbols);
    }

    #[test]
    fn real_grammar_dispatch_counts_match_prstab_equates() {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let derived = derive_nonterminal_dispatch_order(&grammar);

        assert_eq!(derived.internal.len(), 29);
        assert_eq!(derived.external.len(), 49);
        assert_eq!(
            grammar.nonterminals.len(),
            derived.internal.len() + derived.external.len()
        );
    }
}
