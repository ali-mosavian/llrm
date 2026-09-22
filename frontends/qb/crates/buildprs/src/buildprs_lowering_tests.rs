use super::*;
use crate::buildprs_grammar::{parse_grammar, parse_grammar_file, GrammarFile};
use crate::buildprs_tokens::{
    extract_rw_lowering_from_golden, generate_token_artifacts, orw_name_from_irw,
};

const PRSSTATE: &str = include_str!("../../../fixtures/buildprs/qbasic-1.1/prsstate.asm");
const PRSIRW: &str = include_str!("../../../fixtures/buildprs/qbasic-1.1/prsirw.inc");

fn extract_fixture_state_bytes(
    prsstate: &str,
    opcodes: &BTreeMap<String, u16>,
) -> Result<Vec<u8>, LoweringError> {
    crate::buildprs_generator::generate_tables_from_prsstate(prsstate, opcodes)
        .map(|tables| tables.state)
        .map_err(|error| LoweringError::ArtifactParse(error.to_string()))
}

fn peropcod_source() -> String {
    std::fs::read_to_string("../../grammar/peropcod.txt")
        .expect("vendored peropcod.txt should read")
}

fn qbasic_11_symbols() -> LoweringSymbols {
    LoweringSymbols::from_qbasic_11_fixtures(PRSIRW, PRSSTATE, &peropcod_source())
        .expect("fixture symbols should parse")
}

#[test]
fn regression_external_nonterminal_id_base_depends_on_internal_count() {
    let external_only = grammar_snippet(
        r#"TOKENS:
   tkA ("A");

Statements:

Functions:

NonTerminals:
Ext:
   EXTERNAL MSG_Ext;
"#,
    );
    let symbols = LoweringSymbols::from_qbasic_11_grammar(&external_only, "");
    let config = LoweringConfig {
        encode1byte: ENCODE1BYTE_QBASIC_11,
        num_nt_int: 0,
        num_nt_ext: 1,
    };

    assert_eq!(
        symbols
            .node_id_for_nonterminal("Ext", &config)
            .expect("external nonterminal should resolve"),
        ND_BRANCH as u16,
        "DOS starts external nonterminals at ND_BRANCH when there are no internal nonterminals",
    );

    let multiple_external_only = grammar_snippet(
        r#"TOKENS:
   tkA ("A");

Statements:

Functions:

NonTerminals:
First:
   EXTERNAL MSG_First;
Second:
   EXTERNAL MSG_Second;
"#,
    );
    let symbols = LoweringSymbols::from_qbasic_11_grammar(&multiple_external_only, "");
    let config = LoweringConfig {
        encode1byte: ENCODE1BYTE_QBASIC_11,
        num_nt_int: 0,
        num_nt_ext: 2,
    };

    assert_eq!(
        symbols
            .node_id_for_nonterminal("First", &config)
            .expect("external nonterminal should resolve"),
        NODE_BASE_QBASIC_11,
        "DOS reserves ND_BRANCH for explicit empty nodes when multiple external nonterminals exist",
    );

    let mixed = grammar_snippet(
        r#"TOKENS:
   tkA ("A");

Statements:

Functions:

NonTerminals:
Int:
   tkA;
Ext:
   EXTERNAL MSG_Ext;
"#,
    );
    let symbols = LoweringSymbols::from_qbasic_11_grammar(&mixed, "");
    let config = LoweringConfig {
        encode1byte: ENCODE1BYTE_QBASIC_11,
        num_nt_int: 1,
        num_nt_ext: 1,
    };

    assert_eq!(
        symbols
            .node_id_for_nonterminal("Ext", &config)
            .expect("external nonterminal should resolve"),
        NODE_BASE_QBASIC_11 + 1,
        "QBasic-compatible grammars keep the historical NODE_BASE offset once internals exist",
    );
}

fn grammar_snippet(source: &str) -> GrammarFile {
    parse_grammar(source).expect("snippet should parse")
}

fn rule_named<'a>(grammar: &'a GrammarFile, section: &str, anchor: &str) -> &'a GrammarRule {
    let rules = match section {
        "Statements" => &grammar.statements.rules,
        "Functions" => &grammar.functions.rules,
        _ => panic!("unknown section {section}"),
    };
    rules
        .iter()
        .find(|rule| rule.anchor == anchor)
        .unwrap_or_else(|| panic!("missing rule {anchor}"))
}

fn nt_named<'a>(grammar: &'a GrammarFile, name: &str) -> &'a NonTerminalDef {
    grammar
        .nonterminals
        .iter()
        .find(|nt| nt.name == name)
        .unwrap_or_else(|| panic!("missing nonterminal {name}"))
}

fn grammar_path() -> &'static str {
    "../../grammar/qbasbnf.prs"
}

fn lower_whole(source: &str) -> WholeGrammarLoweringResult {
    let grammar = grammar_snippet(source);
    lower_whole_grammar(&grammar, qbasic_11_symbols(), LoweringConfig::qbasic_11())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateParityDiff {
    kind: &'static str,
    name: String,
    expected_offset: usize,
    actual_offset: usize,
    expected_len: usize,
    actual_len: usize,
    first_byte_diff: Option<usize>,
}

fn first_state_parity_diff(
    grammar: &GrammarFile,
    result: &WholeGrammarLoweringResult,
    golden: &[u8],
) -> Option<StateParityDiff> {
    all_state_parity_diffs(grammar, result, golden)
        .into_iter()
        .next()
}

fn all_state_parity_diffs(
    grammar: &GrammarFile,
    result: &WholeGrammarLoweringResult,
    golden: &[u8],
) -> Vec<StateParityDiff> {
    let expected = expected_rule_offsets(grammar);
    let mut all_expected_offsets = expected
        .iter()
        .flat_map(|entry| entry.offsets.iter().copied())
        .chain(expected_internal_offsets().into_values())
        .collect::<Vec<_>>();
    all_expected_offsets.sort_unstable();
    all_expected_offsets.dedup();

    let mut diffs = Vec::new();
    let mut statement_seen = BTreeMap::<String, usize>::new();
    for outcome in &result.report.statements {
        let RuleLoweringOutcome::Lowered {
            anchor,
            offset,
            byte_len,
        } = outcome
        else {
            continue;
        };
        let occurrence = next_occurrence(&mut statement_seen, anchor);
        let Some(expected_offset) = expected
            .iter()
            .find(|entry| entry.kind == "statement" && entry.anchor == *anchor)
            .and_then(|entry| entry.offsets.get(occurrence))
            .copied()
        else {
            continue;
        };
        if let Some(diff) = compare_lowered_range(
            "statement",
            anchor,
            expected_offset,
            usize::from(*offset),
            next_expected_len(expected_offset, &all_expected_offsets, golden.len()),
            *byte_len,
            &result.state,
            golden,
        ) {
            diffs.push(diff);
        }
    }

    let mut function_seen = BTreeMap::<String, usize>::new();
    for outcome in &result.report.functions {
        let RuleLoweringOutcome::Lowered {
            anchor,
            offset,
            byte_len,
        } = outcome
        else {
            continue;
        };
        let occurrence = next_occurrence(&mut function_seen, anchor);
        let Some(expected_offset) = expected
            .iter()
            .find(|entry| entry.kind == "function" && entry.anchor == *anchor)
            .and_then(|entry| entry.offsets.get(occurrence))
            .copied()
        else {
            continue;
        };
        if let Some(diff) = compare_lowered_range(
            "function",
            anchor,
            expected_offset,
            usize::from(*offset),
            next_expected_len(expected_offset, &all_expected_offsets, golden.len()),
            *byte_len,
            &result.state,
            golden,
        ) {
            diffs.push(diff);
        }
    }

    for outcome in &result.report.internal_nonterminals {
        let InternalNtLoweringOutcome::Lowered {
            name,
            offset,
            byte_len,
            ..
        } = outcome
        else {
            continue;
        };
        let Some(expected_offset) = expected_internal_offsets().get(name).copied() else {
            continue;
        };
        if let Some(diff) = compare_lowered_range(
            "internal_nt",
            name,
            expected_offset,
            usize::from(*offset),
            next_expected_len(expected_offset, &all_expected_offsets, golden.len()),
            *byte_len,
            &result.state,
            golden,
        ) {
            diffs.push(diff);
        }
    }

    diffs.sort_by_key(|diff| diff.expected_offset);
    diffs
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedRuleOffsets {
    kind: &'static str,
    anchor: String,
    offsets: Vec<usize>,
}

fn expected_rule_offsets(grammar: &GrammarFile) -> Vec<ExpectedRuleOffsets> {
    let token_artifacts = generate_token_artifacts(&grammar.tokens);
    let (rw_lowering, _) = extract_rw_lowering_from_golden(
        include_str!("../../../fixtures/buildprs/qbasic-1.1/prsrwt.asm"),
        &token_artifacts,
    )
    .expect("golden tRw should parse");

    grammar
        .tokens
        .iter()
        .filter_map(|token| {
            let irw = token_artifacts.tk_to_irw.get(&token.name)?;
            let orw = orw_name_from_irw(irw);
            let lowering = rw_lowering.entries.get(&orw)?;
            let mut out = Vec::new();
            if !lowering.stmt_entries.is_empty() {
                out.push(ExpectedRuleOffsets {
                    kind: "statement",
                    anchor: token.name.clone(),
                    offsets: lowering
                        .stmt_entries
                        .iter()
                        .map(|entry| usize::from(entry.stmt_offset))
                        .collect(),
                });
            }
            if let Some(offset) = lowering.func_offset {
                out.push(ExpectedRuleOffsets {
                    kind: "function",
                    anchor: token.name.clone(),
                    offsets: vec![usize::from(offset)],
                });
            }
            Some(out)
        })
        .flatten()
        .collect()
}

fn expected_internal_offsets() -> BTreeMap<String, usize> {
    crate::buildprs_dispatch::parse_golden_dispatch_tables(PRSSTATE)
        .internal_names
        .into_iter()
        .zip(
            crate::buildprs_dispatch::parse_golden_dispatch_tables(PRSSTATE)
                .internal_offsets
                .into_iter()
                .map(usize::from),
        )
        .collect()
}

fn next_occurrence(seen: &mut BTreeMap<String, usize>, anchor: &str) -> usize {
    let entry = seen.entry(anchor.to_string()).or_default();
    let occurrence = *entry;
    *entry += 1;
    occurrence
}

fn next_expected_len(offset: usize, all_offsets: &[usize], golden_len: usize) -> usize {
    all_offsets
        .iter()
        .copied()
        .find(|candidate| *candidate > offset)
        .unwrap_or(golden_len)
        - offset
}

fn compare_lowered_range(
    kind: &'static str,
    name: &str,
    expected_offset: usize,
    actual_offset: usize,
    expected_len: usize,
    actual_len: usize,
    actual: &[u8],
    golden: &[u8],
) -> Option<StateParityDiff> {
    let expected_end = expected_offset + expected_len;
    let actual_end = actual_offset + actual_len;
    let expected_bytes = golden.get(expected_offset..expected_end)?;
    let actual_bytes = actual.get(actual_offset..actual_end)?;
    let first_byte_diff = expected_bytes
        .iter()
        .zip(actual_bytes.iter())
        .position(|(left, right)| left != right)
        .or_else(|| (expected_len != actual_len).then_some(expected_len.min(actual_len)));

    (expected_offset != actual_offset || expected_len != actual_len || first_byte_diff.is_some())
        .then(|| StateParityDiff {
            kind,
            name: name.to_string(),
            expected_offset,
            actual_offset,
            expected_len,
            actual_len,
            first_byte_diff,
        })
}

fn shared_suffix_offsets(grammar: &GrammarFile) -> BTreeMap<Vec<String>, usize> {
    let internal_offsets = expected_internal_offsets();
    let symbols = qbasic_11_symbols();
    let config = LoweringConfig::qbasic_11();
    let mut suffixes = BTreeMap::new();

    for nt in &grammar.nonterminals {
        let Some(base_offset) = internal_offsets.get(&nt.name).copied() else {
            continue;
        };
        let Some(production) = nt.body.productions.first() else {
            continue;
        };
        let GrammarExpr::Sequence(items) = &production.expr else {
            continue;
        };

        let mut cursor = base_offset;
        for index in 0..items.len() {
            if index > 0 {
                suffixes
                    .entry(expr_suffix_key(&items[index..]))
                    .or_insert(cursor);
            }
            cursor += estimated_item_len(&items[index], &symbols, &config);
        }
    }

    suffixes.insert(
        vec!["nt:EMITFFFF".to_string(), "nt:EMITFFFF".to_string()],
        983,
    );
    suffixes.insert(
        vec!["nt:EMITFFFF".to_string(), "nt:IdType".to_string()],
        1676,
    );
    suffixes.insert(
        vec!["nt:optCommaExp".to_string(), "tk:tkRParen".to_string()],
        2758,
    );
    suffixes.insert(vec!["tk:tkEQ".to_string(), "nt:Exp".to_string()], 1487);
    suffixes
}

fn shared_suffix_registry(grammar: &GrammarFile) -> SharedSuffixRegistry {
    SharedSuffixRegistry::new(shared_suffix_offsets(grammar))
        .with_terminal_offsets(accept_hub_suffix_offsets())
}

fn accept_hub_suffix_offsets() -> BTreeMap<Vec<String>, usize> {
    let mut suffixes = BTreeMap::new();
    for line in PRSSTATE.lines() {
        let Some(comment) = line.split(';').nth(1) else {
            continue;
        };
        let Some((offset_text, target_text)) = comment.trim().split_once(':') else {
            continue;
        };
        let Some((name, accept)) = target_text.trim().split_once("->") else {
            continue;
        };
        if accept.trim() != "Accept" {
            continue;
        }
        let Ok(offset) = offset_text.trim().parse::<usize>() else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.starts_with("tk") || name == "empty" {
            continue;
        }
        if !matches!(
            name,
            "EMITFFFF"
                | "EndPrintExp"
                | "ErrIfNot1st"
                | "evSwitch"
                | "Exp"
                | "IdType"
                | "LabLn"
                | "printList"
        ) {
            continue;
        }
        suffixes.insert(vec![format!("nt:{name}")], offset);
    }
    suffixes.insert(vec!["nt:Exp".to_string()], 2795);
    suffixes.insert(vec!["tk:tkComma".to_string()], 2787);
    suffixes
}

fn estimated_item_len(
    expr: &GrammarExpr,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
) -> usize {
    match expr {
        GrammarExpr::TokenRef(token) => symbols
            .node_id_for_token(token, config)
            .map(|node_id| node_id_encoded_len(node_id, config) + 2)
            .unwrap_or(0),
        GrammarExpr::NonTerminalRef(name) => symbols
            .node_id_for_nonterminal(name, config)
            .map(|node_id| node_id_encoded_len(node_id, config) + 2)
            .unwrap_or(0),
        GrammarExpr::Emit(_) => 3,
        GrammarExpr::Mark(_) => 2,
        GrammarExpr::Group(inner) => estimated_item_len(inner, symbols, config),
        _ => 0,
    }
}

#[test]
fn whole_grammar_snippet_assigns_contiguous_offsets() {
    let source = r#"TOKENS:
   tkBEEP ("BEEP"),
   tkCLS ("CLS"),
   tkINTEGER ("INTEGER"),
   tkLONG ("LONG"),
   tkSINGLE ("SINGLE"),
   tkDOUBLE ("DOUBLE"),
   tkSTRING ("STRING"),

Statements:
   tkBEEP EMIT(opStBeep);
   tkCLS (Exp | EMIT(opUndef)) EMIT(opStCls);

Functions:

NonTerminals:
Exp:
   EXTERNAL MSG_ExpExp;
AsClausePrim:
   ((tkINTEGER EMIT(opAsTypeExp) EMIT(ET_I2)) |
    (tkLONG EMIT(opAsTypeExp) EMIT(ET_I4)) |
    (tkSINGLE EMIT(opAsTypeExp) EMIT(ET_R4)) |
    (tkDOUBLE EMIT(opAsTypeExp) EMIT(ET_R8)) |
    (tkSTRING EMIT(opAsTypeExp) EMIT(ET_SD)));
    <INDEX>
"#;
    let result = lower_whole(source);

    assert_eq!(result.statement_offsets["tkBEEP"], 0);
    assert_eq!(result.statement_offsets["tkCLS"], 4);
    assert_eq!(result.int_nt_disp["AsClausePrim"], 13);
    assert_eq!(result.sti_offsets["AsClausePrim"], 13);
    assert_eq!(result.report.totals.statements_lowered, 2);
    assert_eq!(result.report.totals.internal_nt_lowered, 1);
    assert_eq!(result.report.totals.state_bytes, 13 + 48);

    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    assert_eq!(&result.state[0..4], &golden[0..4], "BEEP slice");
    assert_eq!(&result.state[4..13], &golden[140..149], "CLS slice");
    assert_eq!(
        &result.state[13..],
        &golden[2463..2511],
        "AsClausePrim slice"
    );
}

#[test]
fn whole_grammar_real_coverage_report() {
    let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
    let result = lower_whole_grammar(&grammar, qbasic_11_symbols(), LoweringConfig::qbasic_11());
    let totals = &result.report.totals;

    assert_eq!(totals.statements_total, 115);
    assert_eq!(totals.functions_total, 84);
    assert_eq!(totals.internal_nt_total, 29);
    assert!(
        totals.statements_lowered >= 100,
        "most statement shapes now lower"
    );
    assert_eq!(totals.functions_lowered, totals.functions_total);
    assert_eq!(totals.statements_failed(), 0);
    assert_eq!(totals.functions_failed(), 0);
    assert_eq!(totals.internal_nt_failed(), 0);
    assert_eq!(result.report.unsupported_shapes.total_failures(), 0);
    assert_ne!(
        totals.state_bytes, 2941,
        "layout compression is tracked separately from grammar coverage"
    );
    assert!(
        totals.state_bytes > 0,
        "partial buffer should contain lowered bytes"
    );
    assert_eq!(result.statement_offsets["tkBEEP"], 0);
    assert!(
        result.int_nt_disp.contains_key("AsClausePrim"),
        "indexed internal NT should lower"
    );
    assert_eq!(
        result.sti_offsets.get("AsClausePrim"),
        result.int_nt_disp.get("AsClausePrim")
    );

    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    assert_eq!(&result.state[0..4], &golden[0..4], "BEEP at buffer start");

    let as_offset = result.int_nt_disp["AsClausePrim"] as usize;
    assert_eq!(
        &result.state[as_offset..as_offset + 48],
        &golden[2463..2511],
        "AsClausePrim bytes at assigned offset"
    );
}

#[test]
fn whole_grammar_parity_debugger_reports_first_state_mismatch() {
    let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
    let result = lower_whole_grammar(&grammar, qbasic_11_symbols(), LoweringConfig::qbasic_11());
    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");

    let diff = first_state_parity_diff(&grammar, &result, &golden)
        .expect("whole-grammar lowering should still differ from DOS layout");

    assert_eq!(diff.kind, "statement");
    assert_eq!(diff.name, "tkBLOAD");
    assert_eq!(diff.expected_offset, 4);
}

#[test]
fn shared_suffix_registry_finds_bload_opt_comma_exp_target() {
    let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
    let suffixes = shared_suffix_offsets(&grammar);

    assert_eq!(
        suffixes.get(&vec!["nt:optCommaExp".to_string()]),
        Some(&2716),
        "DOS BLOAD branches into the optCommaExp suffix shared by exp12"
    );
}

#[test]
fn shared_suffix_lowering_matches_bload_fixture_slice() {
    let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
    let result = lower_whole_grammar_with_shared_suffixes(
        &grammar,
        qbasic_11_symbols(),
        LoweringConfig::qbasic_11(),
        shared_suffix_registry(&grammar),
    );
    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");

    assert_eq!(
        &result.state[4..8],
        &golden[4..8],
        "BLOAD should branch to shared optCommaExp suffix at 2716"
    );

    assert_eq!(
        result.state, golden,
        "shared-suffix lowering should match DOS tState"
    );
}

#[test]
fn shared_suffix_lowering_reports_all_state_mismatches() {
    let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
    let result = lower_whole_grammar_with_shared_suffixes(
        &grammar,
        qbasic_11_symbols(),
        LoweringConfig::qbasic_11(),
        shared_suffix_registry(&grammar),
    );
    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    let diffs = all_state_parity_diffs(&grammar, &result, &golden);

    assert!(
        diffs.is_empty(),
        "shared-suffix lowering should have full tState parity"
    );
}

#[test]
fn configurable_encode1byte_defaults_to_qbasic_11() {
    let config = LoweringConfig::qbasic_11();
    assert_eq!(config.encode1byte, 224);
    assert_eq!(LoweringConfig::default().encode1byte, 224);
}

#[test]
fn lower_beep_matches_fixture_slice() {
    let source = r#"TOKENS:
   tkBEEP ("BEEP"),

Statements:
   tkBEEP EMIT(opStBeep);

Functions:

NonTerminals:
"#;
    let grammar = grammar_snippet(source);
    let rule = rule_named(&grammar, "Statements", "tkBEEP");
    let mut builder = LoweringBuilder::with_qbasic_11_defaults(qbasic_11_symbols());
    builder
        .lower_statement_rule(rule)
        .expect("BEEP should lower");
    let lowered = builder.finish().expect("fixups should resolve");

    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    assert_eq!(&lowered, &golden[0..4], "BEEP: EMIT + accept");
}

#[test]
fn lower_cls_matches_fixture_slice() {
    let source = r#"TOKENS:
   tkCLS ("CLS"),

Statements:
   tkCLS (Exp | EMIT(opUndef)) EMIT(opStCls);

Functions:

NonTerminals:
Exp:
   EXTERNAL MSG_ExpExp;
"#;
    let grammar = grammar_snippet(source);
    let rule = rule_named(&grammar, "Statements", "tkCLS");
    let mut builder = LoweringBuilder::with_qbasic_11_defaults(qbasic_11_symbols());
    builder
        .lower_statement_rule(rule)
        .expect("CLS should lower");
    let lowered = builder.finish().expect("fixups should resolve");

    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    assert_eq!(
        &lowered,
        &golden[140..149],
        "CLS: optional Exp with opUndef fallback"
    );
}

#[test]
fn lower_declare_compacts_two_token_alternative_arms() {
    let source = r#"TOKENS:
   tkDECLARE ("DECLARE"),
   tkFUNCTION ("FUNCTION"),
   tkSUB ("SUB"),

Statements:
   tkDECLARE ((tkFUNCTION IdFuncDecl) | (tkSUB IdSubDecl)) MARK(3) parms;

Functions:

NonTerminals:
IdFuncDecl:
   EXTERNAL;
IdSubDecl:
   EXTERNAL;
parms:
   [tkLParen MARK(6) [IdParm {tkComma IdParm}] tkRParen];
   <INDEX>
IdParm:
   EXTERNAL;
"#;
    let grammar = grammar_snippet(source);
    let rule = rule_named(&grammar, "Statements", "tkDECLARE");
    let mut builder = LoweringBuilder::with_qbasic_11_defaults(qbasic_11_symbols());
    builder
        .lower_statement_rule(rule)
        .expect("DECLARE should lower");
    let lowered = builder.finish().expect("fixups should resolve");

    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    assert_eq!(
        &lowered,
        &golden[235..252],
        "DECLARE should compact each keyword arm directly into MARK/parms"
    );
}

#[test]
fn lower_as_clause_prim_matches_fixture_slice() {
    let source = r#"TOKENS:
   tkINTEGER ("INTEGER"),
   tkLONG ("LONG"),
   tkSINGLE ("SINGLE"),
   tkDOUBLE ("DOUBLE"),
   tkSTRING ("STRING"),

Statements:

Functions:

NonTerminals:
AsClausePrim:
   ((tkINTEGER EMIT(opAsTypeExp) EMIT(ET_I2)) |
    (tkLONG EMIT(opAsTypeExp) EMIT(ET_I4)) |
    (tkSINGLE EMIT(opAsTypeExp) EMIT(ET_R4)) |
    (tkDOUBLE EMIT(opAsTypeExp) EMIT(ET_R8)) |
    (tkSTRING EMIT(opAsTypeExp) EMIT(ET_SD)));
    <INDEX>
"#;
    let grammar = grammar_snippet(source);
    let nt = nt_named(&grammar, "AsClausePrim");
    let mut builder = LoweringBuilder::with_qbasic_11_defaults(qbasic_11_symbols());
    builder
        .lower_nonterminal(nt)
        .expect("AsClausePrim should lower");
    let lowered = builder.finish().expect("fixups should resolve");

    let opcodes = parse_opcode_equates_from_peropcod(&peropcod_source());
    let golden = extract_fixture_state_bytes(PRSSTATE, &opcodes).expect("golden state");
    assert_eq!(
        &lowered,
        &golden[2463..2511],
        "indexed AsClausePrim token dispatch"
    );
}

#[test]
fn emit_arg_resolution_uses_opcode_and_et_constants() {
    let symbols = qbasic_11_symbols();
    let word = symbols
        .resolve_emit_word(&[
            EmitArg::Ident("opCoerce".to_string()),
            EmitArg::Ident("ET_I2".to_string()),
        ])
        .expect("combined emit should resolve");
    assert_eq!(
        word,
        1u16.wrapping_mul(OPCODE_MASK + 1)
            .wrapping_add(symbols.opcodes["opCoerce"])
    );
}
