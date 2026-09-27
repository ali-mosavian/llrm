//! Lower typed [`GrammarExpr`] trees into `NtParse` state-table bytes.
//!
//! This is the grammar-backed half of reverse `buildprs` generation. The encoder
//! primitives live in [`crate::buildprs_encoder`]; this module maps AST nodes to
//! branch/fixup layouts observed in `prsstate.asm`.

use std::collections::BTreeMap;

use crate::buildprs_dispatch::derive_nonterminal_dispatch_order;
use crate::buildprs_encoder::{BranchTarget, EncodeConfig, EncodeError, StateEncoder, ND_BRANCH};
use crate::buildprs_generator::{parse_opcode_equates_from_peropcod, ENCODE1BYTE_QBASIC_11};
use crate::buildprs_grammar::{
    EmitArg, EmitDirective, GrammarExpr, GrammarFile, GrammarProduction, GrammarRule,
    NonTerminalDef,
};
use crate::buildprs_tokens;

pub const OPCODE_MASK: u16 = 0x03ff;
pub const NUM_NT_INT_QBASIC_11: u16 = 29;
pub const NUM_NT_EXT_QBASIC_11: u16 = 49;
pub const NODE_BASE_QBASIC_11: u16 = ND_BRANCH as u16 + 1;

/// Configurable `ENCODE1BYTE` threshold (QBasic 1.1 golden artifacts use 224).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweringConfig {
    pub encode1byte: u8,
    pub num_nt_int: u16,
    pub num_nt_ext: u16,
}

impl Default for LoweringConfig {
    fn default() -> Self {
        Self::qbasic_11()
    }
}

impl LoweringConfig {
    pub fn qbasic_11() -> Self {
        Self {
            encode1byte: ENCODE1BYTE_QBASIC_11,
            num_nt_int: NUM_NT_INT_QBASIC_11,
            num_nt_ext: NUM_NT_EXT_QBASIC_11,
        }
    }

    pub fn encode_config(&self) -> EncodeConfig {
        EncodeConfig::new(self.encode1byte)
    }
}

/// Symbol lookup for `EMIT(...)` operands and grammar cross-references.
#[derive(Debug, Clone, Default)]
pub struct LoweringSymbols {
    pub opcodes: BTreeMap<String, u16>,
    pub irw_by_token: BTreeMap<String, u16>,
    pub int_nt_index: BTreeMap<String, u16>,
    pub ext_nt_index: BTreeMap<String, u16>,
}

impl LoweringSymbols {
    pub fn from_qbasic_11_grammar(grammar: &GrammarFile, peropcod: &str) -> Self {
        let token_artifacts = buildprs_tokens::generate_token_artifacts(&grammar.tokens);
        let dispatch = derive_nonterminal_dispatch_order(grammar);

        let mut irw_by_token = token_artifacts.irw_ids.clone();
        for (token, irw) in &token_artifacts.tk_to_irw {
            if let Some(id) = token_artifacts.irw_ids.get(irw) {
                irw_by_token.insert(token.clone(), *id);
            }
        }

        Self {
            opcodes: parse_opcode_equates_from_peropcod(peropcod),
            irw_by_token,
            int_nt_index: dispatch
                .internal
                .iter()
                .enumerate()
                .map(|(index, entry)| (entry.name.clone(), index as u16))
                .collect(),
            ext_nt_index: dispatch
                .external
                .iter()
                .enumerate()
                .map(|(index, entry)| (entry.grammar_name.clone(), index as u16))
                .collect(),
        }
    }

    pub fn from_qbasic_11_fixtures(
        prsirw: &str,
        prsstate: &str,
        peropcod: &str,
    ) -> Result<Self, LoweringError> {
        Ok(Self {
            opcodes: parse_opcode_equates_from_peropcod(peropcod),
            irw_by_token: parse_irw_equates(prsirw)?,
            int_nt_index: parse_int_nt_order(prsstate)?,
            ext_nt_index: parse_ext_nt_order(prsstate)?,
        })
    }

    pub fn node_id_for_token(
        &self,
        token: &str,
        config: &LoweringConfig,
    ) -> Result<u16, LoweringError> {
        let irw = self
            .irw_by_token
            .get(token)
            .or_else(|| {
                let key = format!("IRW_{}", token.strip_prefix("tk").unwrap_or(token));
                self.irw_by_token.get(&key)
            })
            .copied()
            .ok_or_else(|| LoweringError::UnknownToken(token.to_string()))?;
        Ok(NODE_BASE_QBASIC_11 + config.num_nt_int + config.num_nt_ext + irw)
    }

    pub fn node_id_for_nonterminal(
        &self,
        name: &str,
        config: &LoweringConfig,
    ) -> Result<u16, LoweringError> {
        if let Some(index) = self.int_nt_index.get(name) {
            return Ok(NODE_BASE_QBASIC_11 + *index);
        }
        if let Some(index) = self.ext_nt_index.get(name) {
            let base = if config.num_nt_int == 0 && config.num_nt_ext == 1 {
                ND_BRANCH as u16
            } else {
                NODE_BASE_QBASIC_11
            };
            return Ok(base + config.num_nt_int + *index);
        }
        Err(LoweringError::UnknownNonTerminal(name.to_string()))
    }

    pub fn resolve_emit_word(&self, args: &[EmitArg]) -> Result<u16, LoweringError> {
        if args.is_empty() {
            return Err(LoweringError::EmptyEmit);
        }

        let primary = resolve_emit_arg(&self.opcodes, &args[0])?;
        if args.len() == 1 {
            return Ok(primary);
        }

        let secondary = resolve_emit_arg(&self.opcodes, &args[1])?;
        Ok(secondary
            .wrapping_mul(OPCODE_MASK + 1)
            .wrapping_add(primary))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    EmptyEmit,
    UnknownEmitSymbol(String),
    UnknownToken(String),
    UnknownNonTerminal(String),
    UnsupportedExpr(&'static str),
    Encode(EncodeError),
    ArtifactParse(String),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyEmit => write!(f, "EMIT directive has no operands"),
            Self::UnknownEmitSymbol(symbol) => write!(f, "unknown EMIT symbol `{symbol}`"),
            Self::UnknownToken(token) => write!(f, "unknown token `{token}`"),
            Self::UnknownNonTerminal(name) => write!(f, "unknown nonterminal `{name}`"),
            Self::UnsupportedExpr(kind) => {
                write!(f, "lowering scaffold does not handle `{kind}` yet")
            }
            Self::Encode(error) => write!(f, "state encode error: {error:?}"),
            Self::ArtifactParse(message) => write!(f, "artifact parse error: {message}"),
        }
    }
}

impl std::error::Error for LoweringError {}

impl From<EncodeError> for LoweringError {
    fn from(error: EncodeError) -> Self {
        Self::Encode(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Label(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixupTarget {
    Label(Label),
    Offset(usize),
    GlobalOffset(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BranchFixup {
    operand_pos: usize,
    cursor_after_operand: usize,
    target: FixupTarget,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SharedSuffixRegistry {
    offsets: BTreeMap<Vec<String>, usize>,
    terminal_offsets: BTreeMap<Vec<String>, usize>,
}

impl SharedSuffixRegistry {
    pub fn new(offsets: BTreeMap<Vec<String>, usize>) -> Self {
        Self {
            offsets,
            terminal_offsets: BTreeMap::new(),
        }
    }

    pub fn with_terminal_offsets(mut self, terminal_offsets: BTreeMap<Vec<String>, usize>) -> Self {
        self.terminal_offsets = terminal_offsets;
        self
    }

    fn offset_for(&self, key: &[String]) -> Option<usize> {
        self.offsets.get(key).copied()
    }

    fn terminal_offset_for(&self, key: &[String]) -> Option<usize> {
        self.terminal_offsets.get(key).copied()
    }
}

pub fn qbasic_11_shared_suffix_registry(
    grammar: &GrammarFile,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
) -> SharedSuffixRegistry {
    SharedSuffixRegistry::new(qbasic_11_shared_suffix_offsets(grammar, symbols, config))
        .with_terminal_offsets(qbasic_11_accept_hub_suffix_offsets())
}

fn qbasic_11_shared_suffix_offsets(
    grammar: &GrammarFile,
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
) -> BTreeMap<Vec<String>, usize> {
    let internal_offsets = qbasic_11_internal_offsets();
    let mut suffixes = BTreeMap::new();

    for nt in &grammar.nonterminals {
        let Some(base_offset) = internal_offsets.get(nt.name.as_str()).copied() else {
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
            cursor += estimated_qbasic_11_item_len(&items[index], symbols, config);
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

fn qbasic_11_accept_hub_suffix_offsets() -> BTreeMap<Vec<String>, usize> {
    BTreeMap::from([
        (vec!["nt:EMITFFFF".to_string()], 2447),
        (vec!["nt:EndPrintExp".to_string()], 2915),
        (vec!["nt:ErrIfNot1st".to_string()], 431),
        (vec!["nt:evSwitch".to_string()], 1711),
        (vec!["nt:Exp".to_string()], 2795),
        (vec!["nt:IdType".to_string()], 2525),
        (vec!["nt:LabLn".to_string()], 1062),
        (vec!["nt:printList".to_string()], 1294),
        (vec!["tk:tkComma".to_string()], 2787),
    ])
}

fn qbasic_11_internal_offsets() -> BTreeMap<&'static str, usize> {
    BTreeMap::from([
        ("AsClausePrim", 2463),
        ("AsClause", 2511),
        ("AsClauseAny", 2518),
        ("caseItem", 2535),
        ("commaExp", 2722),
        ("commaOptExp", 2556),
        ("commaOptExpNil", 2562),
        ("commaOptExpNull", 2571),
        ("coordStep", 2580),
        ("coord2Step", 2597),
        ("EMITFFFF", 2614),
        ("event", 2618),
        ("evSwitch", 2691),
        ("exp12", 2713),
        ("expCommaExp", 2719),
        ("fn1arg", 2725),
        ("fn12arg", 2731),
        ("fn2arg", 2737),
        ("fn23arg", 2743),
        ("fnBoundArg", 2752),
        ("lbsExpComma", 2761),
        ("lbsInpExpComma", 2775),
        ("optCommaExp", 2790),
        ("optFilenum", 2793),
        ("parms", 2805),
        ("parms1", 2815),
        ("printItem", 2831),
        ("printList", 2882),
        ("printUsingItem", 2900),
    ])
}

fn estimated_qbasic_11_item_len(
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
        GrammarExpr::Group(inner) => estimated_qbasic_11_item_len(inner, symbols, config),
        _ => 0,
    }
}

/// Incremental state-table builder with branch backpatching.
#[derive(Debug, Clone)]
pub struct LoweringBuilder {
    encoder: StateEncoder,
    labels: BTreeMap<Label, usize>,
    next_label: usize,
    fixups: Vec<BranchFixup>,
    symbols: LoweringSymbols,
    config: LoweringConfig,
    shared_suffixes: SharedSuffixRegistry,
    production_terminated: bool,
    base_offset: usize,
}

impl LoweringBuilder {
    pub fn new(symbols: LoweringSymbols, config: LoweringConfig) -> Self {
        Self {
            encoder: StateEncoder::with_config(config.encode_config()),
            labels: BTreeMap::new(),
            next_label: 0,
            fixups: Vec::new(),
            symbols,
            config,
            shared_suffixes: SharedSuffixRegistry::default(),
            production_terminated: false,
            base_offset: 0,
        }
    }

    pub fn with_shared_suffixes(
        symbols: LoweringSymbols,
        config: LoweringConfig,
        shared_suffixes: SharedSuffixRegistry,
    ) -> Self {
        Self {
            shared_suffixes,
            ..Self::new(symbols, config)
        }
    }

    pub fn with_base_offset(mut self, base_offset: usize) -> Self {
        self.base_offset = base_offset;
        self
    }

    pub fn with_qbasic_11_defaults(symbols: LoweringSymbols) -> Self {
        Self::new(symbols, LoweringConfig::qbasic_11())
    }

    pub fn pos(&self) -> usize {
        self.encoder.bytes().len()
    }

    pub(crate) fn create_label(&mut self) -> Label {
        let label = Label(self.next_label);
        self.next_label += 1;
        label
    }

    pub(crate) fn bind_label(&mut self, label: Label) {
        self.labels.insert(label, self.pos());
    }

    pub fn lower_production(
        &mut self,
        production: &GrammarProduction,
    ) -> Result<(), LoweringError> {
        match &production.expr {
            GrammarExpr::Sequence(items) => self.lower_terminal_sequence(items)?,
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                self.emit_expr_node_to_accept(&production.expr)?;
                self.reject();
                self.production_terminated = true;
            }
            _ => self.lower_expr(&production.expr)?,
        }
        if !self.production_terminated {
            self.accept();
        }
        Ok(())
    }

    pub fn lower_statement_rule(&mut self, rule: &GrammarRule) -> Result<(), LoweringError> {
        self.lower_production(&rule.production)
    }

    pub fn lower_nonterminal(&mut self, nt: &NonTerminalDef) -> Result<(), LoweringError> {
        if nt.external {
            return Err(LoweringError::UnsupportedExpr("EXTERNAL nonterminal"));
        }

        let production = nt
            .body
            .productions
            .first()
            .ok_or(LoweringError::UnsupportedExpr("empty nonterminal body"))?;

        if nt.has_index {
            match self.lower_indexed_alternative(&production.expr) {
                Ok(()) => Ok(()),
                Err(LoweringError::UnsupportedExpr("indexed alternative"))
                | Err(LoweringError::UnsupportedExpr("indexed nonterminal")) => {
                    self.lower_expr(&production.expr)?;
                    if !self.production_terminated {
                        self.accept();
                    }
                    Ok(())
                }
                Err(error) => Err(error),
            }
        } else if let Some(list) = split_optional_marked_paren_list(&production.expr) {
            self.lower_optional_marked_paren_list(&list)?;
            if !self.production_terminated {
                self.accept();
            }
            Ok(())
        } else {
            match &production.expr {
                GrammarExpr::Sequence(items) => {
                    if let Some(args) = self.split_parenthesized_shared_tail(items) {
                        self.lower_parenthesized_shared_tail(&args)?;
                    } else if let Some(args) = split_parenthesized_two_part_arg(items) {
                        self.lower_parenthesized_two_part_arg(&args, 2812)?;
                    } else if let Some(args) = split_parenthesized_single_arg(items) {
                        self.lower_parenthesized_single_arg(&args, 2812)?;
                    } else if let Some(args) = split_parenthesized_optional_second_arg(items) {
                        self.lower_parenthesized_optional_second_arg(&args, 2812)?;
                    } else {
                        let lowered_shared_terminal =
                            if let Some((last, prefix)) = items.split_last() {
                                if let Some(offset) = self.shared_terminal_offset_for(last) {
                                    if self.estimated_items_len(prefix).is_some_and(|prefix_len| {
                                        offset == self.base_offset + prefix_len
                                    }) {
                                        self.lower_sequence(prefix)?;
                                        self.emit_expr_node_to_accept(last)?;
                                        self.reject();
                                    } else {
                                        self.lower_terminal_shared_suffix(prefix, offset)?;
                                    }
                                    self.production_terminated = true;
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                        if !lowered_shared_terminal {
                            self.lower_expr(&production.expr)?;
                        }
                    }
                }
                _ => self.lower_expr(&production.expr)?,
            }
            if !self.production_terminated {
                self.accept();
            }
            Ok(())
        }
    }

    pub fn finish(mut self) -> Result<Vec<u8>, LoweringError> {
        self.resolve_fixups()?;
        Ok(self.encoder.into_bytes())
    }

    pub fn accept(&mut self) {
        self.encoder.accept();
    }

    pub fn reject(&mut self) {
        self.encoder.reject();
    }

    pub fn lower_expr(&mut self, expr: &GrammarExpr) -> Result<(), LoweringError> {
        match expr {
            GrammarExpr::Empty => Ok(()),
            GrammarExpr::Sequence(items) => self.lower_sequence(items),
            GrammarExpr::Alternative(items) => self.lower_alternative(items),
            GrammarExpr::Group(inner) => self.lower_expr(inner),
            GrammarExpr::Optional(inner) => self.lower_optional(inner),
            GrammarExpr::Repeat(inner) => self.lower_repeat(inner),
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.lower_required_node(node_id)
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.lower_required_node(node_id)
            }
            GrammarExpr::Emit(emit) => self.lower_emit(emit),
            GrammarExpr::Mark(mark) => {
                self.encoder.mark(mark.slot);
                Ok(())
            }
            GrammarExpr::External { .. } => Err(LoweringError::UnsupportedExpr("EXTERNAL")),
        }
    }

    fn lower_sequence(&mut self, items: &[GrammarExpr]) -> Result<(), LoweringError> {
        if let Some((prefix, suffix)) = split_alternative_prefix(items) {
            let suffix_label = self.create_label();
            self.lower_alternative_forward(prefix, suffix_label, false)?;
            self.bind_label(suffix_label);
            return self.lower_sequence(suffix);
        }

        if let Some((prefix, node, terminal_offset)) = self.self_required_terminal_suffix(items) {
            self.lower_sequence(prefix)?;
            self.lower_required_node_to_offset(node, terminal_offset)?;
            self.reject();
            self.production_terminated = true;
            return Ok(());
        }

        if let Some((first, suffix_target)) = self.shared_suffix_target(items) {
            if let Some(first_len) = self.estimated_expr_len(first) {
                if suffix_target == self.base_offset + first_len {
                    self.lower_required_expr(first)?;
                    self.emit_expr_node_to_accept(&items[1])?;
                    self.reject();
                    self.production_terminated = true;
                    return Ok(());
                }
            }
            self.lower_required_node_to_offset(first, suffix_target)?;
            self.reject();
            self.production_terminated = true;
            return Ok(());
        }

        for (index, item) in items.iter().enumerate() {
            if let GrammarExpr::Optional(inner) = item {
                if index + 1 < items.len() {
                    self.lower_optional_continuing(inner)?;
                    continue;
                }
            }
            if index + 1 < items.len() {
                if let Some(alternatives) = alternative_items(item) {
                    let join = self.create_label();
                    self.lower_alternative_forward(alternatives, join, false)?;
                    self.bind_label(join);
                    continue;
                }
            }
            self.lower_expr(item)?;
        }
        Ok(())
    }

    fn lower_terminal_sequence(&mut self, items: &[GrammarExpr]) -> Result<(), LoweringError> {
        if let Some(statement) = split_print_list_statement(items) {
            return self.lower_print_list_statement(&statement);
        }

        if let Some(args) = split_parenthesized_single_arg(items) {
            return self.lower_parenthesized_single_arg(&args, 2812);
        }

        if let Some(args) = split_parenthesized_optional_second_arg(items) {
            return self.lower_parenthesized_optional_second_arg(&args, 2812);
        }

        if is_line_graphics_statement_sequence(items) {
            return self.lower_line_graphics_statement();
        }

        if is_line_input_prompt_sequence(items) {
            return self.lower_line_input_statement();
        }

        if let Some(prefix) = split_marked_range_sequence(items) {
            if let Some(exp_offset) = self
                .shared_suffixes
                .terminal_offset_for(&["nt:Exp".to_string()])
            {
                return self.lower_marked_range_sequence(prefix, exp_offset);
            }
        }

        if is_open_statement_sequence(items) {
            return self.lower_open_statement();
        }

        if is_paint_statement_sequence(items) {
            return self.lower_paint_statement();
        }

        if let Some((optional, terminal)) = split_optional_prefix_terminal(items) {
            return self.lower_optional_prefix_terminal(optional, terminal);
        }

        if is_input_prompt_repeat_sequence(items) {
            return self.lower_input_statement();
        }

        if let Some(opcodes) = file_record_statement_ops(items) {
            if let Some(emitffff_offset) = self
                .shared_suffixes
                .terminal_offset_for(&["nt:EMITFFFF".to_string()])
            {
                return self.lower_file_record_statement(&opcodes, emitffff_offset);
            }
        }

        if is_put_graphics_statement_sequence(items) {
            if let Some(emitffff_offset) = self
                .shared_suffixes
                .terminal_offset_for(&["nt:EMITFFFF".to_string()])
            {
                return self.lower_put_graphics_statement(emitffff_offset);
            }
        }

        if let Some((prefix, suffix)) = split_alternative_prefix(items) {
            if let [terminal_suffix] = suffix {
                if let Some(offset) = self.shared_terminal_offset_for(terminal_suffix) {
                    if let Some(branches) = suffix_dispatch_branches(prefix) {
                        return self.lower_suffix_dispatch_branches(&branches, offset);
                    }
                    if let Some((branches, fallback)) = split_node_arms_with_fallback(prefix) {
                        return self
                            .lower_suffix_node_arms_with_fallback(&branches, fallback, offset);
                    }
                }
            }
            if let Some(offset) = self.shared_suffixes.offset_for(&expr_suffix_key(suffix)) {
                if let Some((node_arm, fallback)) = split_node_arm_with_fallback(prefix) {
                    return self.lower_suffix_node_arm_with_fallback(&node_arm, fallback, offset);
                }
                if let Some(branches) = suffix_dispatch_branches(prefix) {
                    return self.lower_suffix_dispatch_branches(&branches, offset);
                }
            }
            let suffix_label = self.create_label();
            self.lower_alternative_forward(prefix, suffix_label, false)?;
            self.bind_label(suffix_label);
            return self.lower_terminal_sequence(suffix);
        }

        if let Some((prefix, alternatives, suffix)) = split_mid_sequence_alternative_suffix(items) {
            if let Some(offset) = self.shared_suffixes.offset_for(&expr_suffix_key(suffix)) {
                self.lower_sequence(prefix)?;
                if let Some((node_arm, fallback)) = split_node_arm_with_fallback(alternatives) {
                    return self.lower_suffix_node_arm_with_fallback(&node_arm, fallback, offset);
                }
                if let Some(branches) = suffix_dispatch_branches(alternatives) {
                    return self.lower_suffix_dispatch_branches(&branches, offset);
                }
            }
        }

        if let Some((prefix, suffix_offset)) = self.terminal_shared_suffix_target(items) {
            self.lower_terminal_shared_suffix(prefix, suffix_offset)?;
            self.production_terminated = true;
            return Ok(());
        }

        if let Some((prefix, node, terminal_offset)) = self.self_required_terminal_suffix(items) {
            self.lower_sequence(prefix)?;
            self.lower_required_node_to_offset(node, terminal_offset)?;
            self.reject();
            self.production_terminated = true;
            return Ok(());
        }

        if self.shared_suffix_target(items).is_some() {
            return self.lower_sequence(items);
        }

        let Some((last, prefix)) = items.split_last() else {
            return Ok(());
        };
        self.lower_sequence(prefix)?;
        if self.production_terminated {
            return Ok(());
        }
        if let Some(offset) = self.shared_terminal_offset_for(last) {
            self.branch_to_offset(offset)?;
            self.production_terminated = true;
            return Ok(());
        }
        if let GrammarExpr::Repeat(repeat) = last {
            self.lower_repeat_accept_on_empty(repeat)?;
            self.production_terminated = true;
            return Ok(());
        }
        match last {
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                self.emit_expr_node_to_accept(last)?;
                self.reject();
                self.production_terminated = true;
                Ok(())
            }
            _ => self.lower_expr(last),
        }
    }

    fn lower_alternative(&mut self, items: &[GrammarExpr]) -> Result<(), LoweringError> {
        if let Some(dispatch) = split_print_item_dispatch(items) {
            if dispatch.comma_emit.is_none()
                || self
                    .shared_terminal_offset_for(dispatch.end_print_exp)
                    .is_some()
            {
                return self.lower_print_item_dispatch(&dispatch);
            }
        }

        if let Some(branch) = split_node_body_with_accepting_fallback(items) {
            return self.lower_node_body_with_accepting_fallback(&branch);
        }

        if let Some(branch) = split_selector_with_optional_accept_fallback(items) {
            return self.lower_selector_with_optional_accept_fallback(&branch);
        }

        if let Some(branch) = split_node_accept_or_emit_fallback(items) {
            return self.lower_node_accept_or_emit_fallback(branch);
        }

        if let Some(dispatch) = split_accepting_nodes_with_emit_suffix_fallback(items) {
            return self.lower_accepting_nodes_with_emit_suffix_fallback(&dispatch);
        }

        if let Some(branch) = split_node_accept_or_emit_suffix(items) {
            if let Some(offset) = self.shared_terminal_offset_for(branch.suffix) {
                return self.lower_node_accept_or_emit_suffix(branch, offset);
            }
        }

        if let Some(branch) = split_suffix_sequence_with_accepting_nodes(items) {
            if let Some(offset) = self.shared_terminal_offset_for(branch.suffix) {
                return self.lower_suffix_sequence_with_accepting_nodes(branch, offset);
            }
        }

        if let Some(branch) = split_node_emit_with_emit_fallback(items) {
            return self.lower_node_emit_with_emit_fallback(branch);
        }

        if let Some(dispatch) = split_priority_emit_dispatch(items) {
            return self.lower_priority_emit_dispatch(&dispatch);
        }

        if is_open_statement_alternative(items) {
            return self.lower_open_statement();
        }

        if is_on_statement_alternative(items) {
            let emitffff_offset = self
                .shared_suffixes
                .terminal_offset_for(&["nt:EMITFFFF".to_string()]);
            let labln_offset = self
                .shared_suffixes
                .terminal_offset_for(&["nt:LabLn".to_string()]);
            if let (Some(emitffff_offset), Some(labln_offset)) = (emitffff_offset, labln_offset) {
                return self.lower_on_statement(emitffff_offset, labln_offset);
            }
        }

        if is_next_statement_alternative(items) {
            if let Some(emitffff_offset) = self
                .shared_suffixes
                .terminal_offset_for(&["nt:EMITFFFF".to_string()])
            {
                return self.lower_next_statement(emitffff_offset);
            }
        }

        if is_common_statement_alternative(items) {
            return self.lower_common_statement_alternative();
        }

        if let Some((branches, suffix)) = trailing_suffix_dispatch_branches(items) {
            if let Some(offset) = self.shared_terminal_offset_for(suffix) {
                return self.lower_suffix_dispatch_branches(&branches, offset);
            }
        }

        if let Some((prefix_alternatives, suffix)) = split_shared_tail_alternative(items) {
            let suffix_label = self.create_label();
            self.lower_alternative_forward(&prefix_alternatives, suffix_label, false)?;
            self.bind_label(suffix_label);
            return self.lower_sequence(&suffix);
        }

        if let Some((branches, fallback)) = split_node_arms_with_fallback(items) {
            return self.lower_node_arms_with_fallback(&branches, fallback);
        }

        if let Some(branches) = alternative_dispatch_branches(items) {
            return self.lower_dispatch_alternative(&branches);
        }

        let done = self.create_label();
        self.lower_alternative_forward(items, done, true)?;
        self.bind_label(done);
        Ok(())
    }

    fn lower_dispatch_alternative(
        &mut self,
        branches: &[AlternativeDispatchBranch<'_>],
    ) -> Result<(), LoweringError> {
        let labels = (0..branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (branch, label) in branches.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(branch.node, label)?;
        }
        self.reject();

        for (branch, label) in branches.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            if let [GrammarExpr::Repeat(repeat)] = branch.tail {
                self.lower_repeat_accept_on_empty(repeat)?;
            } else {
                self.lower_sequence(branch.tail)?;
                self.accept();
            }
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_print_item_dispatch(
        &mut self,
        dispatch: &PrintItemDispatch<'_>,
    ) -> Result<(), LoweringError> {
        let tab_body = self.create_label();
        let spc_body = self.create_label();
        let exp_body = self.create_label();
        let item_semi_body = self.create_label();
        let item_comma_body = self.create_label();
        let optional_separator = self.create_label();

        self.emit_expr_node_to_accept(dispatch.end_print)?;
        self.emit_expr_node_to_label(dispatch.tab_token, tab_body)?;
        self.emit_expr_node_to_label(dispatch.spc_token, spc_body)?;
        if let Some(comma_emit) = dispatch.comma_emit {
            let comma_body = self.create_label();
            let semi_body = self.create_label();
            self.emit_expr_node_to_label(dispatch.comma_token, comma_body)?;
            self.emit_expr_node_to_label(dispatch.semicolon_token, semi_body)?;
            self.emit_expr_node_to_label(dispatch.exp_node, exp_body)?;
            self.reject();

            self.bind_label(exp_body);
            self.emit_expr_node_to_label(dispatch.comma_token, item_comma_body)?;
            self.emit_expr_node_to_label(dispatch.semicolon_token, item_semi_body)?;
            let end_print_exp_offset = self
                .shared_terminal_offset_for(dispatch.end_print_exp)
                .ok_or(LoweringError::UnsupportedExpr("print item dispatch"))?;
            self.branch_to_offset(end_print_exp_offset)?;

            self.bind_label(item_semi_body);
            self.lower_emit(dispatch.item_semi_emit)?;
            self.accept();
            self.bind_label(item_comma_body);
            self.lower_emit(dispatch.item_comma_emit)?;
            self.accept();
            self.bind_label(semi_body);
            self.lower_emit(dispatch.semicolon_emit)?;
            self.accept();
            self.bind_label(comma_body);
            self.lower_emit(comma_emit)?;
            self.accept();
            self.bind_label(spc_body);
            self.lower_required_expr(dispatch.arg_node)?;
            self.lower_emit(dispatch.spc_emit)?;
            self.accept();
            self.bind_label(tab_body);
            self.lower_required_expr(dispatch.arg_node)?;
            self.lower_emit(dispatch.tab_emit)?;
            self.accept();
        } else {
            self.emit_expr_node_to_label(dispatch.exp_node, exp_body)?;
            self.reject();

            self.bind_label(exp_body);
            self.emit_expr_node_to_label(dispatch.comma_token, item_semi_body)?;
            self.emit_expr_node_to_label(dispatch.semicolon_token, item_semi_body)?;
            self.emit_expr_node_to_accept(dispatch.end_print_exp)?;
            self.reject();

            self.bind_label(item_semi_body);
            self.lower_emit(dispatch.item_semi_emit)?;
            self.accept();
            self.bind_label(spc_body);
            self.lower_required_expr(dispatch.arg_node)?;
            self.lower_emit(dispatch.spc_emit)?;
            self.branch_to_label(optional_separator)?;
            self.bind_label(tab_body);
            self.lower_required_expr(dispatch.arg_node)?;
            self.lower_emit(dispatch.tab_emit)?;
            self.bind_label(optional_separator);
            self.emit_expr_node_to_accept(dispatch.semicolon_token)?;
            self.emit_expr_node_to_accept(dispatch.comma_token)?;
            self.accept();
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_suffix_dispatch_branches(
        &mut self,
        branches: &[SuffixDispatchBranch],
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        let labels = (0..branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (branch, label) in branches.iter().zip(labels.iter().copied()) {
            for node in &branch.nodes {
                self.emit_expr_node_to_label(node, label)?;
            }
        }
        self.reject();

        for (branch, label) in branches.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            self.lower_sequence(&branch.tail)?;
            self.branch_to_offset(suffix_offset)?;
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_suffix_node_arm_with_fallback(
        &mut self,
        node_arm: &AlternativeDispatchBranch<'_>,
        fallback: &GrammarExpr,
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        let node_body = self.create_label();
        self.emit_expr_node_to_label(node_arm.node, node_body)?;
        self.lower_suffix_body(fallback, suffix_offset)?;
        self.bind_label(node_body);
        self.lower_sequence(node_arm.tail)?;
        self.branch_to_offset(suffix_offset)?;
        self.production_terminated = true;
        Ok(())
    }

    fn lower_suffix_node_arms_with_fallback(
        &mut self,
        branches: &[AlternativeDispatchBranch<'_>],
        fallback: &GrammarExpr,
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        let labels = (0..branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (branch, label) in branches.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(branch.node, label)?;
        }
        self.lower_suffix_body(fallback, suffix_offset)?;

        for (branch, label) in branches.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            self.lower_sequence(branch.tail)?;
            self.branch_to_offset(suffix_offset)?;
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_suffix_body(
        &mut self,
        expr: &GrammarExpr,
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        match ungroup_expr(expr) {
            GrammarExpr::Sequence(items) => self.lower_sequence(items)?,
            _ => self.lower_expr(expr)?,
        }
        self.branch_to_offset(suffix_offset)
    }

    fn lower_file_record_statement(
        &mut self,
        opcodes: &FileRecordOpcodes,
        emitffff_offset: usize,
    ) -> Result<(), LoweringError> {
        let after_filenum = self.create_label();
        let comma_body = self.create_label();
        let rec_present = self.create_label();
        let rec2_id = self.create_label();
        let rec2_emit = self.create_label();
        let rec3_id = self.create_label();
        let rec3_emit = self.create_label();

        self.emit_nonterminal_to_label("optFilenum", after_filenum)?;
        self.reject();

        self.bind_label(after_filenum);
        self.emit_token_to_label("tkComma", comma_body)?;
        self.emit_symbol(&opcodes.default_record)?;
        self.accept();

        self.bind_label(comma_body);
        self.emit_nonterminal_to_label("Exp", rec_present)?;
        self.emit_token_to_label("tkComma", rec2_id)?;
        self.reject();
        self.bind_label(rec2_id);
        self.emit_nonterminal_to_label("IdAryElemRef", rec2_emit)?;
        self.reject();
        self.bind_label(rec2_emit);
        self.emit_symbol(&opcodes.empty_record_with_id)?;
        self.branch_to_offset(emitffff_offset)?;

        self.bind_label(rec_present);
        self.emit_token_to_label("tkComma", rec3_id)?;
        self.emit_symbol(&opcodes.record_without_id)?;
        self.accept();
        self.bind_label(rec3_id);
        self.emit_nonterminal_to_label("IdAryElemRef", rec3_emit)?;
        self.reject();
        self.bind_label(rec3_emit);
        self.emit_symbol(&opcodes.record_with_id)?;
        self.branch_to_offset(emitffff_offset)?;

        self.production_terminated = true;
        Ok(())
    }

    fn lower_put_graphics_statement(
        &mut self,
        emitffff_offset: usize,
    ) -> Result<(), LoweringError> {
        let after_coord = self.create_label();
        let after_comma = self.create_label();
        let after_id = self.create_label();
        let raster_body = self.create_label();
        let and_body = self.create_label();
        let or_body = self.create_label();
        let preset_body = self.create_label();
        let pset_body = self.create_label();
        let xor_body = self.create_label();

        self.emit_nonterminal_to_label("coordStep", after_coord)?;
        self.reject();
        self.bind_label(after_coord);
        self.emit_token_to_label("tkComma", after_comma)?;
        self.reject();
        self.bind_label(after_comma);
        self.emit_nonterminal_to_label("IdAryGetPut", after_id)?;
        self.reject();
        self.bind_label(after_id);
        self.emit_symbol("opStGraphicsPut")?;
        self.emit_token_to_label("tkComma", raster_body)?;
        self.branch_to_offset(emitffff_offset)?;

        self.bind_label(raster_body);
        self.emit_token_to_label("tkAND", and_body)?;
        self.emit_token_to_label("tkOR", or_body)?;
        self.emit_token_to_label("tkPRESET", preset_body)?;
        self.emit_token_to_label("tkPSET", pset_body)?;
        self.emit_token_to_label("tkXOR", xor_body)?;
        self.reject();
        self.bind_label(xor_body);
        self.emit_number(4)?;
        self.accept();
        self.bind_label(pset_body);
        self.emit_number(3)?;
        self.accept();
        self.bind_label(preset_body);
        self.emit_number(2)?;
        self.accept();
        self.bind_label(or_body);
        self.emit_number(0)?;
        self.accept();
        self.bind_label(and_body);
        self.emit_number(1)?;
        self.accept();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_input_statement(&mut self) -> Result<(), LoweringError> {
        let prompt_join = self.create_label();
        let first_emit = self.create_label();
        let repeat_start = self.create_label();
        let repeat_body = self.create_label();
        let repeat_emit = self.create_label();

        self.lower_input_prompt_prefix(prompt_join)?;
        self.bind_label(prompt_join);
        self.encoder.mark(8);
        self.emit_nonterminal_to_label("IdAryElemRef", first_emit)?;
        self.reject();
        self.bind_label(first_emit);
        self.emit_symbol("opStInput")?;

        self.bind_label(repeat_start);
        self.emit_token_to_label("tkComma", repeat_body)?;
        self.emit_symbol("opInputEos")?;
        self.accept();
        self.bind_label(repeat_body);
        self.emit_nonterminal_to_label("IdAryElemRef", repeat_emit)?;
        self.reject();
        self.bind_label(repeat_emit);
        self.emit_symbol("opStInput")?;
        self.branch_to_label(repeat_start)?;

        self.production_terminated = true;
        Ok(())
    }

    fn lower_line_graphics_statement(&mut self) -> Result<(), LoweringError> {
        let after_coord = self.create_label();
        let after_minus = self.create_label();
        let after_coord2 = self.create_label();
        let color_body = self.create_label();
        let color_mark = self.create_label();
        let after_color = self.create_label();
        let style_body = self.create_label();
        let rwbf_mark = self.create_label();
        let rwb_body = self.create_label();
        let rwf_mark = self.create_label();
        let style_join = self.create_label();
        let comma_mark = self.create_label();

        self.emit_nonterminal_to_label("coordStep", after_coord)?;
        self.bind_label(after_coord);
        self.emit_token_to_label("tkMinus", after_minus)?;
        self.reject();
        self.bind_label(after_minus);
        self.emit_nonterminal_to_label("coord2Step", after_coord2)?;
        self.reject();
        self.bind_label(after_coord2);

        self.emit_token_to_label("tkComma", color_body)?;
        self.accept();
        self.bind_label(color_body);
        self.emit_nonterminal_to_label("Exp", color_mark)?;
        self.branch_to_label(after_color)?;
        self.bind_label(color_mark);
        self.encoder.mark(1);
        self.bind_label(after_color);

        self.emit_token_to_label("tkComma", style_body)?;
        self.accept();
        self.bind_label(style_body);
        self.emit_nonterminal_to_label("RwBF", rwbf_mark)?;
        self.emit_nonterminal_to_label("RwB", rwb_body)?;
        self.branch_to_label(style_join)?;
        self.bind_label(rwb_body);
        self.emit_nonterminal_to_label("RwF", rwf_mark)?;
        self.encoder.mark(2);
        self.branch_to_label(style_join)?;
        self.bind_label(rwf_mark);
        self.encoder.mark(3);
        self.branch_to_label(style_join)?;
        self.bind_label(rwbf_mark);
        self.encoder.mark(3);

        self.bind_label(style_join);
        self.emit_nonterminal_to_label("commaExp", comma_mark)?;
        self.accept();
        self.bind_label(comma_mark);
        self.encoder.mark(4);
        self.accept();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_line_input_statement(&mut self) -> Result<(), LoweringError> {
        let after_input = self.create_label();
        let prompt_join = self.create_label();

        self.emit_token_to_label("tkINPUT", after_input)?;
        self.reject();
        self.bind_label(after_input);
        self.lower_input_prompt_prefix(prompt_join)?;
        self.bind_label(prompt_join);
        let id_node = self
            .symbols
            .node_id_for_nonterminal("IdAryElemRef", &self.config)?;
        self.emit_node_to_accept(id_node)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_input_prompt_prefix(&mut self, prompt_join: Label) -> Result<(), LoweringError> {
        let lbs_prompt = self.create_label();
        let semicolon_prompt = self.create_label();
        let literal_prompt = self.create_label();
        let literal_comma = self.create_label();
        let semicolon_literal = self.create_label();
        let semicolon_comma = self.create_label();

        self.emit_nonterminal_to_label("lbsInpExpComma", lbs_prompt)?;
        self.emit_token_to_label("tkSColon", semicolon_prompt)?;
        self.emit_nonterminal_to_label("LitString", literal_prompt)?;
        self.branch_to_label(prompt_join)?;

        self.bind_label(literal_prompt);
        self.encoder.mark(4);
        self.emit_token_to_label("tkSColon", prompt_join)?;
        self.emit_token_to_label("tkComma", literal_comma)?;
        self.reject();
        self.bind_label(literal_comma);
        self.encoder.mark(1);
        self.branch_to_label(prompt_join)?;

        self.bind_label(semicolon_prompt);
        self.encoder.mark(2);
        self.emit_nonterminal_to_label("LitString", semicolon_literal)?;
        self.branch_to_label(prompt_join)?;
        self.bind_label(semicolon_literal);
        self.encoder.mark(4);
        self.emit_token_to_label("tkSColon", prompt_join)?;
        self.emit_token_to_label("tkComma", semicolon_comma)?;
        self.reject();
        self.bind_label(semicolon_comma);
        self.encoder.mark(1);
        self.branch_to_label(prompt_join)?;

        self.bind_label(lbs_prompt);
        self.encoder.mark(16);

        Ok(())
    }

    fn lower_marked_range_sequence(
        &mut self,
        prefix: &GrammarExpr,
        exp_offset: usize,
    ) -> Result<(), LoweringError> {
        let after_prefix = self.create_label();
        let comma_body = self.create_label();
        let exp_body = self.create_label();
        let to_body = self.create_label();
        let second_exp = self.create_label();

        self.emit_expr_node_to_label(prefix, after_prefix)?;
        self.reject();
        self.bind_label(after_prefix);

        self.emit_token_to_label("tkComma", comma_body)?;
        self.accept();
        self.bind_label(comma_body);
        self.emit_nonterminal_to_label("Exp", exp_body)?;
        self.emit_token_to_label("tkTO", to_body)?;
        self.reject();
        self.bind_label(to_body);
        self.encoder.mark(3);
        self.branch_to_offset(exp_offset)?;

        self.bind_label(exp_body);
        self.encoder.mark(1);
        self.emit_token_to_label("tkTO", second_exp)?;
        self.accept();
        self.bind_label(second_exp);
        self.encoder.mark(2);
        self.branch_to_offset(exp_offset)?;

        self.production_terminated = true;
        Ok(())
    }

    fn lower_next_statement(&mut self, emitffff_offset: usize) -> Result<(), LoweringError> {
        let id_body = self.create_label();
        let first_operand = self.create_label();
        let repeat_start = self.create_label();
        let repeat_body = self.create_label();
        let repeat_emit = self.create_label();
        let repeat_operand = self.create_label();

        self.emit_nonterminal_to_label("IdFor", id_body)?;
        self.emit_symbol("opStNext")?;
        self.lower_required_node_to_offset(
            &GrammarExpr::NonTerminalRef("EMITFFFF".to_string()),
            emitffff_offset,
        )?;
        self.reject();

        self.bind_label(id_body);
        self.emit_symbol("opStNextId")?;
        self.emit_nonterminal_to_label("EMITFFFF", first_operand)?;
        self.reject();
        self.bind_label(first_operand);
        self.emit_nonterminal_to_label("EMITFFFF", repeat_start)?;
        self.reject();

        self.bind_label(repeat_start);
        self.emit_token_to_label("tkComma", repeat_body)?;
        self.accept();
        self.bind_label(repeat_body);
        self.emit_nonterminal_to_label("IdFor", repeat_emit)?;
        self.reject();
        self.bind_label(repeat_emit);
        self.emit_symbol("opStNextId")?;
        self.emit_nonterminal_to_label("EMITFFFF", repeat_operand)?;
        self.reject();
        self.bind_label(repeat_operand);
        self.emit_nonterminal_to_label("EMITFFFF", repeat_start)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_node_emit_with_emit_fallback(
        &mut self,
        branch: NodeEmitFallbackBranch<'_>,
    ) -> Result<(), LoweringError> {
        let exp_body = self.create_label();

        self.emit_expr_node_to_label(branch.node, exp_body)?;
        self.lower_emit(branch.fallback_emit)?;
        self.accept();
        self.bind_label(exp_body);
        self.lower_emit(branch.matched_emit)?;
        self.accept();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_node_accept_or_emit_suffix(
        &mut self,
        branch: NodeAcceptOrEmitSuffix<'_>,
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        self.emit_expr_node_to_accept(branch.node)?;
        self.lower_emit(branch.emit)?;
        self.branch_to_offset(suffix_offset)?;
        self.production_terminated = true;
        Ok(())
    }

    fn lower_node_accept_or_emit_fallback(
        &mut self,
        branch: NodeAcceptOrEmitFallback<'_>,
    ) -> Result<(), LoweringError> {
        self.emit_expr_node_to_accept(branch.node)?;
        self.lower_emit(branch.fallback_emit)?;
        self.accept();
        self.production_terminated = true;
        Ok(())
    }

    fn lower_node_body_with_accepting_fallback(
        &mut self,
        branch: &NodeBodyAcceptingFallback<'_>,
    ) -> Result<(), LoweringError> {
        let body = self.create_label();

        self.emit_expr_node_to_label(branch.node, body)?;
        self.emit_expr_node_to_accept(branch.fallback)?;
        self.reject();

        self.bind_label(body);
        self.lower_sequence_emit_accept(branch.body)?;

        self.production_terminated = true;
        Ok(())
    }

    fn lower_accepting_nodes_with_emit_suffix_fallback(
        &mut self,
        dispatch: &AcceptingNodesEmitSuffixFallback<'_>,
    ) -> Result<(), LoweringError> {
        let labels = (0..dispatch.branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for node in &dispatch.accepting_nodes {
            self.emit_expr_node_to_accept(node)?;
        }
        for (branch, label) in dispatch.branches.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(branch.node, label)?;
        }

        self.lower_emit(dispatch.fallback_emit)?;
        self.emit_expr_node_to_accept(dispatch.fallback_suffix)?;
        self.reject();

        for (branch, label) in dispatch.branches.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            self.lower_sequence_emit_accept(branch.tail)?;
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_selector_with_optional_accept_fallback(
        &mut self,
        branch: &SelectorOptionalAcceptFallback<'_>,
    ) -> Result<(), LoweringError> {
        let selected_body = self.create_label();
        let fallback_terminal = self.create_label();
        let selector_body = self.create_label();

        self.emit_expr_node_to_label(branch.primary_node, selected_body)?;
        self.emit_expr_node_to_label(branch.optional_fallback_node, fallback_terminal)?;
        self.bind_label(fallback_terminal);
        self.emit_expr_node_to_accept(branch.fallback_terminal)?;
        self.reject();

        self.bind_label(selected_body);
        self.emit_expr_node_to_label(branch.selector_node, selector_body)?;
        self.lower_emit(branch.default_emit)?;
        self.accept();

        self.bind_label(selector_body);
        self.lower_required_expr(branch.selector_operand)?;
        self.lower_emit(branch.selector_emit)?;
        self.accept();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_suffix_sequence_with_accepting_nodes(
        &mut self,
        branch: SuffixSequenceAcceptingNodes<'_>,
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        let body = self.create_label();

        self.emit_expr_node_to_label(branch.node, body)?;
        for node in branch.accepting_nodes {
            self.emit_expr_node_to_accept(node)?;
        }
        self.reject();

        self.bind_label(body);
        self.lower_sequence(branch.body)?;
        self.branch_to_offset(suffix_offset)?;

        self.production_terminated = true;
        Ok(())
    }

    fn lower_priority_emit_dispatch(
        &mut self,
        dispatch: &PriorityEmitDispatch<'_>,
    ) -> Result<(), LoweringError> {
        let labels = (0..dispatch.branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (branch, label) in dispatch.branches.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(branch.node, label)?;
        }

        match &dispatch.fallback {
            PriorityEmitFallback::Emit(emit) => {
                self.lower_emit(emit)?;
                self.accept();
            }
            PriorityEmitFallback::NestedFinalEmit {
                first_node,
                first_matched_node,
                first_fallback_emit,
                second_prefix,
                final_emit,
            } => self.lower_priority_nested_final_emit_fallback(
                first_node,
                first_matched_node,
                first_fallback_emit,
                second_prefix,
                final_emit,
            )?,
        }

        for (branch, label) in dispatch.branches.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            self.lower_priority_emit_body(&branch.body)?;
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_priority_emit_body(
        &mut self,
        body: &PriorityEmitBody<'_>,
    ) -> Result<(), LoweringError> {
        match body {
            PriorityEmitBody::Sequence(items) => self.lower_sequence_emit_accept(items),
            PriorityEmitBody::NestedNodeFallback {
                matched_items,
                fallback_emit,
            } => {
                let matched_items = flatten_single_group_sequence(matched_items);
                let (last, prefix) = matched_items
                    .split_last()
                    .ok_or(LoweringError::UnsupportedExpr("priority emit body"))?;
                let GrammarExpr::Emit(matched_emit) = last else {
                    return Err(LoweringError::UnsupportedExpr("priority emit body"));
                };
                let (node, rest) = prefix
                    .split_first()
                    .ok_or(LoweringError::UnsupportedExpr("priority emit body"))?;
                let matched = self.create_label();
                self.emit_expr_node_to_label(node, matched)?;
                self.lower_emit(fallback_emit)?;
                self.accept();
                self.bind_label(matched);
                self.lower_sequence(rest)?;
                self.lower_emit(matched_emit)?;
                self.accept();
                Ok(())
            }
        }
    }

    fn lower_priority_nested_final_emit_fallback(
        &mut self,
        first_node: &GrammarExpr,
        first_matched_node: &GrammarExpr,
        first_fallback_emit: &EmitDirective,
        second_prefix: &[GrammarExpr],
        final_emit: &EmitDirective,
    ) -> Result<(), LoweringError> {
        let first_body = self.create_label();
        let second_body = self.create_label();
        let final_body = self.create_label();

        self.emit_expr_node_to_label(first_node, first_body)?;
        let (second_node, second_rest) = second_prefix
            .split_first()
            .ok_or(LoweringError::UnsupportedExpr("priority emit fallback"))?;
        self.emit_expr_node_to_label(second_node, second_body)?;
        self.reject();

        self.bind_label(second_body);
        self.lower_sequence_to_label(second_rest, final_body)?;

        self.bind_label(first_body);
        self.emit_expr_node_to_label(first_matched_node, final_body)?;
        self.lower_emit(first_fallback_emit)?;

        self.bind_label(final_body);
        self.lower_emit(final_emit)?;
        self.accept();
        Ok(())
    }

    fn lower_print_list_statement(
        &mut self,
        statement: &PrintListStatement<'_>,
    ) -> Result<(), LoweringError> {
        let list_start = self.create_label();
        let item_tail = self.create_label();
        let separator_body = self.create_label();

        self.lower_emit(statement.statement_emit)?;
        self.emit_expr_node_to_label(statement.optional_prefix, list_start)?;

        self.bind_label(list_start);
        self.emit_expr_node_to_label(statement.item_node, item_tail)?;
        self.lower_emit(statement.empty_eos_emit)?;
        self.accept();

        self.bind_label(item_tail);
        for separator in &statement.separators {
            self.emit_expr_node_to_label(separator, separator_body)?;
        }
        self.lower_emit(statement.item_eos_emit)?;
        self.accept();

        self.bind_label(separator_body);
        self.lower_emit(statement.separator_emit)?;
        self.emit_expr_node_to_label(statement.item_node, item_tail)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_parenthesized_optional_second_arg(
        &mut self,
        args: &ParenthesizedOptionalSecondArg<'_>,
        close_paren_offset: usize,
    ) -> Result<(), LoweringError> {
        let second_arg = self.create_label();

        self.lower_required_expr(args.open)?;
        self.lower_required_expr(args.first)?;
        self.emit_expr_node_to_label(args.comma, second_arg)?;
        self.branch_to_offset(close_paren_offset)?;
        self.bind_label(second_arg);
        self.lower_required_node_to_offset(args.second, close_paren_offset)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_parenthesized_single_arg(
        &mut self,
        args: &ParenthesizedSingleArg<'_>,
        close_paren_offset: usize,
    ) -> Result<(), LoweringError> {
        self.lower_required_expr(args.open)?;
        self.lower_required_node_to_offset(args.argument, close_paren_offset)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_parenthesized_shared_tail(
        &mut self,
        args: &ParenthesizedSharedTail<'_>,
    ) -> Result<(), LoweringError> {
        self.lower_required_expr(args.open)?;
        self.lower_required_node_to_offset(args.first, args.tail_offset)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_parenthesized_two_part_arg(
        &mut self,
        args: &ParenthesizedTwoPartArg<'_>,
        close_paren_offset: usize,
    ) -> Result<(), LoweringError> {
        self.lower_required_expr(args.open)?;
        self.lower_required_expr(args.first)?;
        self.lower_required_node_to_offset(args.second, close_paren_offset)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_optional_marked_paren_list(
        &mut self,
        list: &OptionalMarkedParenList<'_>,
    ) -> Result<(), LoweringError> {
        let body = self.create_label();
        let repeat_tail = self.create_label();
        let repeat_item = self.create_label();

        self.emit_expr_node_to_label(list.open, body)?;
        self.accept();

        self.bind_label(body);
        self.encoder.mark(list.mark);
        if list.first_required {
            self.emit_expr_node_to_label(list.item, repeat_tail)?;
            self.reject();
        } else {
            self.lower_required_node_to_offset(list.item, 2823)?;
            self.emit_expr_node_to_accept(list.close)?;
            self.reject();
            self.production_terminated = true;
            return Ok(());
        }

        self.bind_label(repeat_tail);
        self.emit_expr_node_to_label(list.separator, repeat_item)?;
        self.emit_expr_node_to_accept(list.close)?;
        self.reject();

        self.bind_label(repeat_item);
        self.emit_expr_node_to_label(list.item, repeat_tail)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_sequence_to_label(
        &mut self,
        items: &[GrammarExpr],
        target: Label,
    ) -> Result<(), LoweringError> {
        let (last, prefix) = items
            .split_last()
            .ok_or(LoweringError::UnsupportedExpr("sequence to label"))?;
        self.lower_sequence(prefix)?;
        self.emit_expr_node_to_label(last, target)?;
        self.reject();
        Ok(())
    }

    fn lower_sequence_emit_accept(&mut self, items: &[GrammarExpr]) -> Result<(), LoweringError> {
        let (last, prefix) = items
            .split_last()
            .ok_or(LoweringError::UnsupportedExpr("priority emit body"))?;
        let GrammarExpr::Emit(emit) = last else {
            return Err(LoweringError::UnsupportedExpr("priority emit body"));
        };
        self.lower_sequence(prefix)?;
        self.lower_emit(emit)?;
        self.accept();
        Ok(())
    }

    fn lower_token_mark_dispatch(
        &mut self,
        arms: &[TokenMarkArm<'_>],
        join: Label,
    ) -> Result<(), LoweringError> {
        let labels = (0..arms.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (arm, label) in arms.iter().zip(labels.iter().copied()) {
            self.emit_token_to_label(arm.token, label)?;
        }
        self.reject();

        for (index, (arm, label)) in arms.iter().zip(labels.iter().copied()).enumerate().rev() {
            self.bind_label(label);
            self.encoder.mark(arm.mark);
            if index != 0 {
                self.branch_to_label(join)?;
            }
        }

        Ok(())
    }

    fn lower_on_statement(
        &mut self,
        emitffff_offset: usize,
        labln_offset: usize,
    ) -> Result<(), LoweringError> {
        let event_body = self.create_label();
        let error_body = self.create_label();
        let exp_body = self.create_label();
        let gosub_mark = self.create_label();
        let goto_mark = self.create_label();
        let after_branch_mark = self.create_label();
        let labln_success = self.create_label();
        let labln_repeat = self.create_label();
        let error_after_goto = self.create_label();
        let error_lit0 = self.create_label();
        let event_after_gosub = self.create_label();
        let event_lit0 = self.create_label();

        self.emit_nonterminal_to_label("event", event_body)?;
        self.emit_token_to_label("tkERROR", error_body)?;
        self.emit_nonterminal_to_label("Exp", exp_body)?;
        self.reject();

        self.bind_label(exp_body);
        self.emit_token_to_label("tkGOTO", goto_mark)?;
        self.emit_token_to_label("tkGOSUB", gosub_mark)?;
        self.reject();
        self.bind_label(gosub_mark);
        self.encoder.mark(2);
        self.branch_to_label(after_branch_mark)?;
        self.bind_label(goto_mark);
        self.encoder.mark(1);
        self.bind_label(after_branch_mark);
        self.emit_nonterminal_to_label("LabLn", labln_repeat)?;
        self.reject();
        self.bind_label(labln_repeat);
        self.emit_token_to_label("tkComma", labln_success)?;
        self.accept();
        self.bind_label(labln_success);
        self.emit_nonterminal_to_label("LabLn", labln_repeat)?;
        self.reject();

        self.bind_label(error_body);
        self.emit_token_to_label("tkGOTO", error_after_goto)?;
        self.reject();
        self.bind_label(error_after_goto);
        self.emit_nonterminal_to_label("Lit0", error_lit0)?;
        self.emit_symbol("opStOnError")?;
        self.branch_to_offset(labln_offset)?;
        self.bind_label(error_lit0);
        self.emit_symbol("opStOnError")?;
        self.branch_to_offset(emitffff_offset)?;

        self.bind_label(event_body);
        self.emit_token_to_label("tkGOSUB", event_after_gosub)?;
        self.reject();
        self.bind_label(event_after_gosub);
        self.emit_nonterminal_to_label("Lit0", event_lit0)?;
        self.emit_symbol("opEvGosub")?;
        let labln_node = self
            .symbols
            .node_id_for_nonterminal("LabLn", &self.config)?;
        self.emit_node_to_accept(labln_node)?;
        self.reject();
        self.bind_label(event_lit0);
        self.emit_symbol("opEvGosub")?;
        self.branch_to_offset(emitffff_offset)?;

        self.production_terminated = true;
        Ok(())
    }

    fn lower_open_statement(&mut self) -> Result<(), LoweringError> {
        let after_exp = self.create_label();
        let mode_body = self.create_label();
        let access_join = self.create_label();
        let access_body = self.create_label();
        let lock_join = self.create_label();
        let access_read = self.create_label();
        let access_write = self.create_label();
        let access_read_write = self.create_label();
        let lock_body = self.create_label();
        let shared_body = self.create_label();
        let as_or_comma = self.create_label();
        let lock_read = self.create_label();
        let lock_write = self.create_label();
        let lock_read_write = self.create_label();
        let as_body = self.create_label();
        let old_filenum = self.create_label();
        let old_exp12 = self.create_label();
        let old_mark = self.create_label();
        let len_option = self.create_label();
        let len_eq = self.create_label();
        let len_exp = self.create_label();
        let len_mark = self.create_label();

        self.emit_nonterminal_to_label("Exp", after_exp)?;
        self.reject();
        self.bind_label(after_exp);

        self.emit_token_to_label("tkFOR", mode_body)?;
        self.branch_to_label(access_join)?;
        self.bind_label(mode_body);
        self.lower_token_mark_dispatch(
            &[
                TokenMarkArm {
                    token: "tkAPPEND",
                    mark: 1,
                },
                TokenMarkArm {
                    token: "tkINPUT",
                    mark: 2,
                },
                TokenMarkArm {
                    token: "tkOUTPUT",
                    mark: 3,
                },
                TokenMarkArm {
                    token: "tkRANDOM",
                    mark: 4,
                },
                TokenMarkArm {
                    token: "tkBINARY",
                    mark: 5,
                },
            ],
            access_join,
        )?;

        self.bind_label(access_join);
        self.emit_token_to_label("tkACCESS", access_body)?;
        self.branch_to_label(lock_join)?;
        self.bind_label(access_body);
        self.emit_token_to_label("tkREAD", access_read)?;
        self.emit_token_to_label("tkWRITE", access_write)?;
        self.reject();
        self.bind_label(access_write);
        self.encoder.mark(7);
        self.branch_to_label(lock_join)?;
        self.bind_label(access_read);
        self.encoder.mark(6);
        self.emit_token_to_label("tkWRITE", access_read_write)?;
        self.branch_to_label(lock_join)?;
        self.bind_label(access_read_write);
        self.encoder.mark(8);

        self.bind_label(lock_join);
        self.emit_token_to_label("tkLOCK", lock_body)?;
        self.emit_token_to_label("tkSHARED", shared_body)?;
        self.branch_to_label(as_or_comma)?;
        self.bind_label(shared_body);
        self.encoder.mark(12);
        self.branch_to_label(as_or_comma)?;
        self.bind_label(lock_body);
        self.emit_token_to_label("tkREAD", lock_read)?;
        self.emit_token_to_label("tkWRITE", lock_write)?;
        self.reject();
        self.bind_label(lock_write);
        self.encoder.mark(10);
        self.branch_to_label(as_or_comma)?;
        self.bind_label(lock_read);
        self.emit_token_to_label("tkWRITE", lock_read_write)?;
        self.encoder.mark(9);
        self.branch_to_label(as_or_comma)?;
        self.bind_label(lock_read_write);
        self.encoder.mark(11);

        self.bind_label(as_or_comma);
        self.emit_token_to_label("tkAS", as_body)?;
        self.emit_token_to_label("tkComma", old_filenum)?;
        self.reject();

        self.bind_label(old_filenum);
        self.emit_nonterminal_to_label("optFilenum", old_exp12)?;
        self.reject();
        self.bind_label(old_exp12);
        self.emit_nonterminal_to_label("exp12", old_mark)?;
        self.reject();
        self.bind_label(old_mark);
        self.encoder.mark(14);
        self.accept();

        self.bind_label(as_body);
        self.emit_nonterminal_to_label("optFilenum", len_option)?;
        self.reject();
        self.bind_label(len_option);
        self.emit_token_to_label("tkLEN", len_eq)?;
        self.accept();
        self.bind_label(len_eq);
        self.emit_token_to_label("tkEQ", len_exp)?;
        self.reject();
        self.bind_label(len_exp);
        self.emit_nonterminal_to_label("Exp", len_mark)?;
        self.reject();
        self.bind_label(len_mark);
        self.encoder.mark(13);
        self.accept();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_paint_statement(&mut self) -> Result<(), LoweringError> {
        let after_coord = self.create_label();
        let after_first = self.create_label();
        let after_second = self.create_label();
        let paint3 = self.create_label();

        self.emit_nonterminal_to_label("coordStep", after_coord)?;
        self.reject();
        self.bind_label(after_coord);
        self.emit_nonterminal_to_label("commaOptExp", after_first)?;
        self.reject();
        self.bind_label(after_first);
        self.emit_nonterminal_to_label("commaOptExp", after_second)?;
        self.reject();
        self.bind_label(after_second);
        self.emit_nonterminal_to_label("commaExp", paint3)?;
        self.emit_symbol("opStPaint2")?;
        self.accept();
        self.bind_label(paint3);
        self.emit_symbol("opStPaint3")?;
        self.accept();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_optional_prefix_terminal(
        &mut self,
        optional: &GrammarExpr,
        terminal: &GrammarExpr,
    ) -> Result<(), LoweringError> {
        let after_optional = self.create_label();

        self.emit_expr_node_to_label(optional, after_optional)?;
        self.bind_label(after_optional);
        self.emit_expr_node_to_accept(terminal)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_optional_node_mark_alternatives(
        &mut self,
        arms: &[NodeMarkArm<'_>],
    ) -> Result<(), LoweringError> {
        let labels = (0..arms.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (arm, label) in arms.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(arm.node, label)?;
        }
        self.accept();

        for (arm, label) in arms.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            self.encoder.mark(arm.mark);
            self.accept();
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_node_arms_with_fallback(
        &mut self,
        branches: &[AlternativeDispatchBranch<'_>],
        fallback: &GrammarExpr,
    ) -> Result<(), LoweringError> {
        let labels = (0..branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (branch, label) in branches.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(branch.node, label)?;
        }

        self.lower_terminal_alternative_body(fallback, true, true)?;

        for (index, (branch, label)) in branches
            .iter()
            .zip(labels.iter().copied())
            .enumerate()
            .rev()
        {
            self.bind_label(label);
            self.lower_terminal_items(branch.tail, true, index == 0)?;
        }

        self.production_terminated = true;
        Ok(())
    }

    fn lower_terminal_alternative_body(
        &mut self,
        expr: &GrammarExpr,
        allow_shared_terminal: bool,
        share_through_node_prefix: bool,
    ) -> Result<(), LoweringError> {
        match ungroup_expr(expr) {
            GrammarExpr::Sequence(items) => {
                self.lower_terminal_items(items, allow_shared_terminal, share_through_node_prefix)
            }
            GrammarExpr::Empty => {
                self.accept();
                Ok(())
            }
            _ => {
                self.lower_expr(expr)?;
                self.accept();
                Ok(())
            }
        }
    }

    fn lower_terminal_items(
        &mut self,
        items: &[GrammarExpr],
        allow_shared_terminal: bool,
        share_through_node_prefix: bool,
    ) -> Result<(), LoweringError> {
        let Some((last, prefix)) = items.split_last() else {
            self.accept();
            return Ok(());
        };

        self.lower_sequence(prefix)?;
        if allow_shared_terminal && (share_through_node_prefix || !contains_required_node(prefix)) {
            if let Some(offset) = self.shared_terminal_offset_for(last) {
                return self.branch_to_offset(offset);
            }
        }

        match last {
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                self.emit_expr_node_to_accept(last)?;
                self.reject();
                Ok(())
            }
            _ => {
                self.lower_expr(last)?;
                self.accept();
                Ok(())
            }
        }
    }

    fn lower_common_statement_alternative(&mut self) -> Result<(), LoweringError> {
        let shared_arm = self.create_label();
        let common_suffix = self.create_label();

        self.emit_token_to_label("tkSHARED", shared_arm)?;
        self.lower_common_prefix(false, common_suffix)?;

        self.bind_label(shared_arm);
        self.emit_symbol("opShared")?;
        self.lower_common_prefix(true, common_suffix)?;

        self.bind_label(common_suffix);
        let idary = self.create_label();
        let comma = self.create_label();
        let repeated_id = self.create_label();
        self.emit_nonterminal_to_label("ACTIONidCommon", idary)?;
        self.reject();
        self.bind_label(idary);
        self.emit_nonterminal_to_label("IdAryI", comma)?;
        self.reject();
        self.bind_label(comma);
        self.emit_token_to_label("tkComma", repeated_id)?;
        self.accept();
        self.bind_label(repeated_id);
        self.emit_nonterminal_to_label("IdAryI", comma)?;
        self.reject();

        self.production_terminated = true;
        Ok(())
    }

    fn lower_common_prefix(&mut self, shared: bool, suffix: Label) -> Result<(), LoweringError> {
        self.emit_symbol("opStCommon")?;
        let slash_alt = self.create_label();
        self.emit_nonterminal_to_label("EMITFFFF", slash_alt)?;
        self.reject();

        self.bind_label(slash_alt);
        let slash_body = self.create_label();
        let after_slash = self.create_label();
        let slash_success = if shared { after_slash } else { suffix };
        self.emit_token_to_label("tkDiv", slash_body)?;
        self.emit_nonterminal_to_label("EMITFFFF", slash_success)?;
        self.reject();

        self.bind_label(slash_body);
        let close_slash = self.create_label();
        self.emit_nonterminal_to_label("IdNamCom", close_slash)?;
        self.reject();
        self.bind_label(close_slash);
        self.emit_token_to_label("tkDiv", slash_success)?;
        self.reject();

        if shared {
            self.bind_label(after_slash);
            self.emit_nonterminal_to_label("ACTIONidShared", suffix)?;
            self.reject();
        }

        Ok(())
    }

    fn emit_symbol(&mut self, symbol: &str) -> Result<(), LoweringError> {
        self.lower_emit(&EmitDirective {
            args: vec![EmitArg::Ident(symbol.to_string())],
        })
    }

    fn emit_number(&mut self, value: u32) -> Result<(), LoweringError> {
        self.lower_emit(&EmitDirective {
            args: vec![EmitArg::Number(value)],
        })
    }

    fn emit_token_to_label(&mut self, token: &str, target: Label) -> Result<(), LoweringError> {
        let node_id = self.symbols.node_id_for_token(token, &self.config)?;
        self.emit_node_to_label(node_id, target)
    }

    fn emit_nonterminal_to_label(
        &mut self,
        name: &str,
        target: Label,
    ) -> Result<(), LoweringError> {
        let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
        self.emit_node_to_label(node_id, target)
    }

    fn lower_alternative_forward(
        &mut self,
        items: &[GrammarExpr],
        success: Label,
        terminating_reject: bool,
    ) -> Result<(), LoweringError> {
        if !terminating_reject {
            if let Some(branches) = alternative_dispatch_branches(items) {
                return self.lower_dispatch_alternative_to_success(&branches, success);
            }
            if let Some((node_arm, fallback_arm)) = split_node_arm_with_fallback(items) {
                let node_label = self.create_label();
                self.emit_expr_node_to_label(node_arm.node, node_label)?;
                self.lower_alternative_arm_fallthrough(fallback_arm)?;
                self.branch_to_label(success)?;
                self.bind_label(node_label);
                self.lower_sequence(node_arm.tail)?;
                return self.branch_to_label(success);
            }
        }

        for (index, alt) in items.iter().enumerate() {
            if index + 1 == items.len() {
                if !terminating_reject
                    && matches!(
                        ungroup_expr(alt),
                        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
                    )
                {
                    self.lower_alternative_arm(alt, success)?;
                } else {
                    self.lower_alternative_arm_fallthrough(alt)?;
                }
            } else {
                self.lower_alternative_arm(alt, success)?;
            }
        }
        if terminating_reject {
            self.reject();
        }
        Ok(())
    }

    fn lower_dispatch_alternative_to_success(
        &mut self,
        branches: &[AlternativeDispatchBranch<'_>],
        success: Label,
    ) -> Result<(), LoweringError> {
        let labels = (0..branches.len())
            .map(|_| self.create_label())
            .collect::<Vec<_>>();

        for (branch, label) in branches.iter().zip(labels.iter().copied()) {
            self.emit_expr_node_to_label(branch.node, label)?;
        }
        self.reject();

        for (branch, label) in branches.iter().zip(labels.iter().copied()).rev() {
            self.bind_label(label);
            if let [tail] = branch.tail {
                if matches!(
                    ungroup_expr(tail),
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
                ) {
                    self.emit_expr_node_to_label(tail, success)?;
                    self.reject();
                    continue;
                }
            }
            self.lower_sequence(branch.tail)?;
            self.branch_to_label(success)?;
        }

        Ok(())
    }

    fn lower_alternative_arm_fallthrough(
        &mut self,
        expr: &GrammarExpr,
    ) -> Result<(), LoweringError> {
        match expr {
            GrammarExpr::Group(inner) => self.lower_alternative_arm_fallthrough(inner),
            GrammarExpr::Sequence(items) if !items.is_empty() => self.lower_sequence(items),
            GrammarExpr::Emit(emit) => self.lower_emit(emit),
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => self.lower_expr(expr),
            GrammarExpr::Optional(inner) => self.lower_optional(inner),
            GrammarExpr::Repeat(inner) => self.lower_repeat(inner),
            GrammarExpr::Empty => Ok(()),
            _ => self.lower_expr(expr),
        }
    }

    fn lower_alternative_arm(
        &mut self,
        expr: &GrammarExpr,
        success: Label,
    ) -> Result<(), LoweringError> {
        match expr {
            GrammarExpr::Group(inner) => self.lower_alternative_arm(inner, success),
            GrammarExpr::Sequence(items) if !items.is_empty() => {
                self.lower_sequence_arm(items, success)
            }
            GrammarExpr::Emit(emit) => {
                self.lower_emit(emit)?;
                self.branch_to_label(success)
            }
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_label(node_id, success)
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_label(node_id, success)
            }
            GrammarExpr::Optional(inner) => {
                self.lower_optional(inner)?;
                self.branch_to_label(success)
            }
            GrammarExpr::Repeat(inner) => {
                self.lower_repeat(inner)?;
                self.branch_to_label(success)
            }
            GrammarExpr::Empty => self.branch_to_label(success),
            _ => {
                self.lower_expr(expr)?;
                self.branch_to_label(success)
            }
        }
    }

    fn lower_sequence_arm(
        &mut self,
        items: &[GrammarExpr],
        success: Label,
    ) -> Result<(), LoweringError> {
        if let [first, second] = items {
            if matches!(
                ungroup_expr(first),
                GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
            ) && matches!(
                ungroup_expr(second),
                GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
            ) {
                let tail_label = self.create_label();
                self.emit_expr_node_to_label(first, tail_label)?;
                self.bind_label(tail_label);
                self.emit_expr_node_to_label(second, success)?;
                self.reject();
                return Ok(());
            }
        }

        match items {
            [GrammarExpr::TokenRef(token), tail @ (GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_))] =>
            {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                let tail_label = self.create_label();
                self.emit_node_to_label(node_id, tail_label)?;
                self.bind_label(tail_label);
                self.emit_expr_node_to_label(tail, success)?;
                self.reject();
                Ok(())
            }
            [GrammarExpr::NonTerminalRef(name), tail @ (GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_))] =>
            {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                let tail_label = self.create_label();
                self.emit_node_to_label(node_id, tail_label)?;
                self.bind_label(tail_label);
                self.emit_expr_node_to_label(tail, success)?;
                self.reject();
                Ok(())
            }
            [GrammarExpr::TokenRef(token), tail @ ..] => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                let tail_label = self.create_label();
                self.emit_node_to_label(node_id, tail_label)?;
                self.bind_label(tail_label);
                self.lower_sequence(tail)?;
                self.branch_to_label(success)
            }
            [GrammarExpr::NonTerminalRef(name), tail @ ..] => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                let tail_label = self.create_label();
                self.emit_node_to_label(node_id, tail_label)?;
                self.bind_label(tail_label);
                self.lower_sequence(tail)?;
                self.branch_to_label(success)
            }
            [GrammarExpr::Emit(emit)] => {
                self.lower_emit(emit)?;
                self.branch_to_label(success)
            }
            _ => {
                self.lower_sequence(items)?;
                self.branch_to_label(success)
            }
        }
    }

    fn lower_optional(&mut self, inner: &GrammarExpr) -> Result<(), LoweringError> {
        if let Some(arms) = split_node_mark_alternatives(inner) {
            return self.lower_optional_node_mark_alternatives(&arms);
        }

        match ungroup_expr(inner) {
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_accept(node_id)?;
                self.accept();
                self.production_terminated = true;
                return Ok(());
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_accept(node_id)?;
                self.accept();
                self.production_terminated = true;
                return Ok(());
            }
            GrammarExpr::Sequence(items) if !items.is_empty() => {
                return self.lower_optional_sequence(items);
            }
            _ => {}
        }

        let join = self.create_label();
        self.emit_unconditional_branch(join)?;
        self.lower_expr(inner)?;
        self.bind_label(join);
        Ok(())
    }

    fn lower_optional_sequence(&mut self, items: &[GrammarExpr]) -> Result<(), LoweringError> {
        let Some((first, rest)) = items.split_first() else {
            return Ok(());
        };
        if let [terminal] = rest {
            if let Some(offset) = self.shared_terminal_offset_for(terminal) {
                self.lower_required_node_to_offset(first, offset)?;
                self.accept();
                self.production_terminated = true;
                return Ok(());
            }
        }
        let body = self.create_label();
        match first {
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_label(node_id, body)?;
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_label(node_id, body)?;
            }
            _ => {
                let join = self.create_label();
                self.emit_unconditional_branch(join)?;
                self.lower_sequence(items)?;
                self.bind_label(join);
                return Ok(());
            }
        }
        self.accept();
        self.bind_label(body);
        if let [GrammarExpr::Mark(mark), terminal] = rest {
            if let Some(offset) = self.shared_terminal_offset_for(terminal) {
                self.encoder.mark(mark.slot);
                self.branch_to_offset(offset)?;
                self.production_terminated = true;
                return Ok(());
            }
        }
        if let [GrammarExpr::Repeat(repeat)] = rest {
            self.lower_repeat_accept_on_empty(repeat)?;
            self.production_terminated = true;
            return Ok(());
        }
        if let [argument, GrammarExpr::Repeat(repeat), terminator] = rest {
            self.lower_required_expr(argument)?;
            return self.lower_repeat_until_accept(repeat, terminator);
        }
        self.lower_sequence(rest)
    }

    fn lower_optional_continuing(&mut self, inner: &GrammarExpr) -> Result<(), LoweringError> {
        match ungroup_expr(inner) {
            GrammarExpr::Sequence(items) if !items.is_empty() => {
                let Some((first, rest)) = items.split_first() else {
                    return Ok(());
                };
                let body = self.create_label();
                let join = self.create_label();
                match first {
                    GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                        self.emit_expr_node_to_label(first, body)?;
                        self.branch_to_label(join)?;
                        self.bind_label(body);
                        self.lower_sequence(rest)?;
                        self.bind_label(join);
                        Ok(())
                    }
                    _ => {
                        self.emit_unconditional_branch(join)?;
                        self.lower_sequence(items)?;
                        self.bind_label(join);
                        Ok(())
                    }
                }
            }
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {
                let join = self.create_label();
                self.emit_expr_node_to_label(ungroup_expr(inner), join)?;
                self.bind_label(join);
                Ok(())
            }
            _ => self.lower_optional(inner),
        }
    }

    fn lower_required_expr(&mut self, expr: &GrammarExpr) -> Result<(), LoweringError> {
        match expr {
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.lower_required_node(node_id)
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.lower_required_node(node_id)
            }
            GrammarExpr::Group(inner) => self.lower_required_expr(inner),
            _ => self.lower_expr(expr),
        }
    }

    fn lower_repeat_until_accept(
        &mut self,
        repeat: &GrammarExpr,
        terminator: &GrammarExpr,
    ) -> Result<(), LoweringError> {
        let loop_start = self.create_label();
        let repeat_body = self.create_label();
        self.bind_label(loop_start);

        let repeat_items = match ungroup_expr(repeat) {
            GrammarExpr::Sequence(items) if !items.is_empty() => items.as_slice(),
            _ => return Err(LoweringError::UnsupportedExpr("repeat terminator")),
        };
        let Some((repeat_first, repeat_rest)) = repeat_items.split_first() else {
            return Err(LoweringError::UnsupportedExpr("repeat terminator"));
        };

        self.emit_expr_node_to_label(repeat_first, repeat_body)?;
        self.emit_expr_node_to_accept(terminator)?;
        self.reject();
        self.bind_label(repeat_body);
        if let [item] = repeat_rest {
            self.emit_expr_node_to_label(item, loop_start)?;
        } else {
            self.lower_sequence(repeat_rest)?;
            self.branch_to_label(loop_start)?;
        }
        self.reject();
        self.production_terminated = true;
        Ok(())
    }

    fn lower_repeat_accept_on_empty(&mut self, repeat: &GrammarExpr) -> Result<(), LoweringError> {
        let loop_start = self.create_label();
        let repeat_body = self.create_label();
        self.bind_label(loop_start);

        let repeat_items = match ungroup_expr(repeat) {
            GrammarExpr::Sequence(items) if !items.is_empty() => items.as_slice(),
            _ => return Err(LoweringError::UnsupportedExpr("repeat accept")),
        };
        let Some((repeat_first, repeat_rest)) = repeat_items.split_first() else {
            return Err(LoweringError::UnsupportedExpr("repeat accept"));
        };

        self.emit_expr_node_to_label(repeat_first, repeat_body)?;
        self.accept();
        self.bind_label(repeat_body);
        if let [item] = repeat_rest {
            self.emit_expr_node_to_label(item, loop_start)?;
            self.reject();
        } else {
            self.lower_sequence(repeat_rest)?;
            self.branch_to_label(loop_start)?;
        }
        Ok(())
    }

    fn emit_expr_node_to_label(
        &mut self,
        expr: &GrammarExpr,
        target: Label,
    ) -> Result<(), LoweringError> {
        match expr {
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_label(node_id, target)
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_label(node_id, target)
            }
            GrammarExpr::Group(inner) => self.emit_expr_node_to_label(inner, target),
            _ => Err(LoweringError::UnsupportedExpr("repeat terminator")),
        }
    }

    fn emit_expr_node_to_accept(&mut self, expr: &GrammarExpr) -> Result<(), LoweringError> {
        match expr {
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_accept(node_id)
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_accept(node_id)
            }
            GrammarExpr::Group(inner) => self.emit_expr_node_to_accept(inner),
            _ => Err(LoweringError::UnsupportedExpr("repeat terminator")),
        }
    }

    fn lower_repeat(&mut self, inner: &GrammarExpr) -> Result<(), LoweringError> {
        let loop_start = self.create_label();
        self.bind_label(loop_start);
        match inner {
            GrammarExpr::Group(grouped) => self.lower_repeat(grouped),
            GrammarExpr::TokenRef(token) => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_label(node_id, loop_start)
            }
            GrammarExpr::NonTerminalRef(name) => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_label(node_id, loop_start)
            }
            GrammarExpr::Sequence(items) if !items.is_empty() => {
                self.lower_repeat_sequence(items, loop_start)
            }
            _ => Err(LoweringError::UnsupportedExpr("Repeat")),
        }
    }

    fn lower_repeat_sequence(
        &mut self,
        items: &[GrammarExpr],
        loop_start: Label,
    ) -> Result<(), LoweringError> {
        let done = self.create_label();
        let tail = self.create_label();
        match items {
            [GrammarExpr::TokenRef(token), rest @ ..] => {
                let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                self.emit_node_to_label(node_id, tail)?;
                self.branch_to_label(done)?;
                self.bind_label(tail);
                self.lower_sequence(rest)?;
                self.branch_to_label(loop_start)?;
                self.bind_label(done);
                Ok(())
            }
            [GrammarExpr::Group(grouped), rest @ ..] => {
                self.lower_repeat_sequence_grouped_first(grouped, rest, tail, done, loop_start)
            }
            [GrammarExpr::NonTerminalRef(name), rest @ ..] => {
                let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                self.emit_node_to_label(node_id, tail)?;
                self.branch_to_label(done)?;
                self.bind_label(tail);
                self.lower_sequence(rest)?;
                self.branch_to_label(loop_start)?;
                self.bind_label(done);
                Ok(())
            }
            _ => Err(LoweringError::UnsupportedExpr("Repeat")),
        }
    }

    fn lower_repeat_sequence_grouped_first(
        &mut self,
        grouped: &GrammarExpr,
        rest: &[GrammarExpr],
        tail: Label,
        done: Label,
        loop_start: Label,
    ) -> Result<(), LoweringError> {
        let GrammarExpr::Alternative(alternatives) = grouped else {
            return Err(LoweringError::UnsupportedExpr("Repeat"));
        };

        for alternative in alternatives {
            match ungroup_expr(alternative) {
                GrammarExpr::TokenRef(token) => {
                    let node_id = self.symbols.node_id_for_token(token, &self.config)?;
                    self.emit_node_to_label(node_id, tail)?;
                }
                GrammarExpr::NonTerminalRef(name) => {
                    let node_id = self.symbols.node_id_for_nonterminal(name, &self.config)?;
                    self.emit_node_to_label(node_id, tail)?;
                }
                _ => return Err(LoweringError::UnsupportedExpr("Repeat")),
            }
        }

        self.branch_to_label(done)?;
        self.bind_label(tail);
        self.lower_sequence(rest)?;
        self.branch_to_label(loop_start)?;
        self.bind_label(done);
        Ok(())
    }

    fn lower_indexed_alternative(&mut self, expr: &GrammarExpr) -> Result<(), LoweringError> {
        let branches = indexed_branches(expr)?;
        let header_size = indexed_header_size(&branches, &self.symbols, &self.config)?;
        let body_size = indexed_body_size(&branches);
        let first_body = self.pos() + header_size + 1;

        for (index, branch) in branches.iter().enumerate() {
            let body_offset = first_body + (branches.len() - 1 - index) * body_size;
            let node_id = self
                .symbols
                .node_id_for_token(&branch.token, &self.config)?;
            self.emit_node_to_offset(node_id, body_offset)?;
        }
        self.reject();

        for branch in branches.iter().rev() {
            for emit in &branch.emits {
                self.lower_emit(emit)?;
            }
            self.accept();
        }

        Ok(())
    }

    fn emit_node_to_offset(
        &mut self,
        node_id: u16,
        target_offset: usize,
    ) -> Result<(), LoweringError> {
        let (operand_pos, cursor_after_operand) =
            self.encoder.emit_node_deferred_branch(node_id)?;
        self.fixups.push(BranchFixup {
            operand_pos,
            cursor_after_operand,
            target: FixupTarget::Offset(target_offset),
        });
        Ok(())
    }

    fn emit_node_to_global_offset(
        &mut self,
        node_id: u16,
        target_offset: usize,
    ) -> Result<(), LoweringError> {
        let (operand_pos, cursor_after_operand) =
            self.encoder.emit_node_deferred_branch(node_id)?;
        self.fixups.push(BranchFixup {
            operand_pos,
            cursor_after_operand,
            target: FixupTarget::GlobalOffset(target_offset),
        });
        Ok(())
    }

    fn lower_emit(&mut self, emit: &EmitDirective) -> Result<(), LoweringError> {
        let word = self.symbols.resolve_emit_word(&emit.args)?;
        self.encoder.emit(word);
        Ok(())
    }

    fn lower_required_node(&mut self, node_id: u16) -> Result<(), LoweringError> {
        let success = self.create_label();
        self.emit_node_to_label(node_id, success)?;
        self.reject();
        self.bind_label(success);
        Ok(())
    }

    fn lower_required_node_to_offset(
        &mut self,
        expr: &GrammarExpr,
        target_offset: usize,
    ) -> Result<(), LoweringError> {
        let node_id = match expr {
            GrammarExpr::TokenRef(token) => self.symbols.node_id_for_token(token, &self.config)?,
            GrammarExpr::NonTerminalRef(name) => {
                self.symbols.node_id_for_nonterminal(name, &self.config)?
            }
            _ => return Err(LoweringError::UnsupportedExpr("shared suffix prefix")),
        };
        self.emit_node_to_global_offset(node_id, target_offset)
    }

    fn shared_suffix_target<'a>(
        &self,
        items: &'a [GrammarExpr],
    ) -> Option<(&'a GrammarExpr, usize)> {
        if items.len() < 2 {
            return None;
        }
        match &items[0] {
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_) => {}
            _ => return None,
        }
        if !matches!(items.get(1), Some(GrammarExpr::NonTerminalRef(_))) || items.len() != 2 {
            return None;
        }
        let key = expr_suffix_key(&items[1..]);
        if key.as_slice() != ["nt:optCommaExp".to_string()] {
            return None;
        }
        self.shared_suffixes
            .offset_for(&key)
            .map(|offset| (&items[0], offset))
    }

    fn terminal_shared_suffix_target<'a>(
        &self,
        items: &'a [GrammarExpr],
    ) -> Option<(&'a [GrammarExpr], usize)> {
        (1..items.len()).find_map(|index| {
            if items[index..].len() < 2 {
                return None;
            }
            let offset = self
                .shared_suffixes
                .offset_for(&expr_suffix_key(&items[index..]))?;
            let prefix_len = self.estimated_items_len(&items[..index])?;
            if offset == self.base_offset + prefix_len {
                return None;
            }
            Some((&items[..index], offset))
        })
    }

    fn estimated_items_len(&self, items: &[GrammarExpr]) -> Option<usize> {
        items
            .iter()
            .map(|item| self.estimated_expr_len(item))
            .sum::<Option<usize>>()
    }

    fn estimated_expr_len(&self, expr: &GrammarExpr) -> Option<usize> {
        match expr {
            GrammarExpr::TokenRef(token) => self
                .symbols
                .node_id_for_token(token, &self.config)
                .ok()
                .map(|node_id| node_id_encoded_len(node_id, &self.config) + 2),
            GrammarExpr::NonTerminalRef(name) => self
                .symbols
                .node_id_for_nonterminal(name, &self.config)
                .ok()
                .map(|node_id| node_id_encoded_len(node_id, &self.config) + 2),
            GrammarExpr::Emit(_) => Some(3),
            GrammarExpr::Mark(_) => Some(2),
            GrammarExpr::Group(inner) => self.estimated_expr_len(inner),
            GrammarExpr::Sequence(items) => self.estimated_items_len(items),
            _ => None,
        }
    }

    fn lower_terminal_shared_suffix(
        &mut self,
        prefix: &[GrammarExpr],
        suffix_offset: usize,
    ) -> Result<(), LoweringError> {
        let Some((last, rest)) = prefix.split_last() else {
            return self.branch_to_offset(suffix_offset);
        };

        if matches!(
            ungroup_expr(last),
            GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
        ) {
            self.lower_sequence(rest)?;
            self.lower_required_node_to_offset(last, suffix_offset)?;
            self.reject();
            return Ok(());
        }

        self.lower_sequence(prefix)?;
        self.branch_to_offset(suffix_offset)
    }

    fn self_required_terminal_suffix<'a>(
        &self,
        items: &'a [GrammarExpr],
    ) -> Option<(&'a [GrammarExpr], &'a GrammarExpr, usize)> {
        (0..items.len().saturating_sub(1)).find_map(|index| {
            let suffix = &items[index..];
            let [node, terminal] = suffix else {
                return None;
            };
            if !matches!(
                ungroup_expr(node),
                GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
            ) {
                return None;
            }
            let suffix_offset = self.shared_suffixes.offset_for(&expr_suffix_key(suffix))?;
            let prefix_len = self.estimated_items_len(&items[..index])?;
            if suffix_offset != self.base_offset + prefix_len {
                return None;
            }
            let terminal_offset = self.shared_terminal_offset_for(terminal)?;
            Some((&items[..index], node, terminal_offset))
        })
    }

    fn split_parenthesized_shared_tail<'a>(
        &self,
        items: &'a [GrammarExpr],
    ) -> Option<ParenthesizedSharedTail<'a>> {
        let [open @ GrammarExpr::TokenRef(open_token), first, tail, close @ GrammarExpr::TokenRef(close_token)] =
            items
        else {
            return None;
        };
        if open_token != "tkLParen"
            || close_token != "tkRParen"
            || !is_node_expr(first)
            || !is_node_expr(tail)
        {
            return None;
        }
        let tail_offset = self
            .shared_suffixes
            .offset_for(&expr_suffix_key(&[tail.clone(), close.clone()]))?;
        let prefix_len = self.estimated_items_len(&items[..2])?;
        if tail_offset == self.base_offset + prefix_len {
            return None;
        }
        Some(ParenthesizedSharedTail {
            open,
            first: ungroup_expr(first),
            tail_offset,
        })
    }

    fn shared_terminal_offset_for(&self, expr: &GrammarExpr) -> Option<usize> {
        let offset = self
            .shared_suffixes
            .terminal_offset_for(&expr_suffix_key(std::slice::from_ref(expr)))?;
        if offset == self.base_offset + self.pos() {
            None
        } else {
            Some(offset)
        }
    }

    fn emit_node_to_label(&mut self, node_id: u16, target: Label) -> Result<(), LoweringError> {
        let (operand_pos, cursor_after_operand) =
            self.encoder.emit_node_deferred_branch(node_id)?;
        self.fixups.push(BranchFixup {
            operand_pos,
            cursor_after_operand,
            target: FixupTarget::Label(target),
        });
        Ok(())
    }

    fn emit_node_to_accept(&mut self, node_id: u16) -> Result<(), LoweringError> {
        self.encoder
            .node(node_id, BranchTarget::Accept)
            .map_err(LoweringError::from)
    }

    fn emit_unconditional_branch(&mut self, target: Label) -> Result<(), LoweringError> {
        let (operand_pos, cursor_after_operand) =
            self.encoder.emit_node_deferred_branch(ND_BRANCH as u16)?;
        self.fixups.push(BranchFixup {
            operand_pos,
            cursor_after_operand,
            target: FixupTarget::Label(target),
        });
        Ok(())
    }

    fn branch_to_label(&mut self, target: Label) -> Result<(), LoweringError> {
        self.emit_unconditional_branch(target)
    }

    fn branch_to_offset(&mut self, target: usize) -> Result<(), LoweringError> {
        self.emit_node_to_global_offset(ND_BRANCH as u16, target)
    }

    fn resolve_fixups(&mut self) -> Result<(), LoweringError> {
        let mut fixups = self.fixups.clone();
        fixups.sort_by_key(|fixup| fixup.operand_pos);
        let mut wide_operand_positions = Vec::new();
        for fixup in &fixups {
            if matches!(
                self.fixup_branch_target(fixup, 0, &[])?.0,
                BranchTarget::Absolute(_)
            ) {
                wide_operand_positions.push(fixup.operand_pos);
            }
        }
        for fixup in fixups {
            let shift_before_operand = shift_before(fixup.operand_pos, &wide_operand_positions);
            let (target, cursor_after_operand) =
                self.fixup_branch_target(&fixup, shift_before_operand, &wide_operand_positions)?;

            self.encoder.patch_branch_operand(
                fixup.operand_pos + shift_before_operand,
                cursor_after_operand,
                target,
            )?;
        }
        Ok(())
    }

    fn fixup_branch_target(
        &self,
        fixup: &BranchFixup,
        shift_before_operand: usize,
        wide_operand_positions: &[usize],
    ) -> Result<(BranchTarget, usize), LoweringError> {
        let cursor_after_operand = fixup.cursor_after_operand + shift_before_operand;
        match fixup.target {
            FixupTarget::Label(label) => {
                let target_pos = *self.labels.get(&label).ok_or_else(|| {
                    LoweringError::ArtifactParse(format!("unbound label {:?}", label.0))
                })?;
                let shift_before_target = shift_before(target_pos, wide_operand_positions);
                Ok((
                    BranchTarget::Relative(target_pos + shift_before_target),
                    cursor_after_operand,
                ))
            }
            FixupTarget::Offset(offset) => {
                Ok((BranchTarget::Relative(offset), cursor_after_operand))
            }
            FixupTarget::GlobalOffset(offset) => Ok((
                self.offset_branch_target(offset, cursor_after_operand),
                cursor_after_operand,
            )),
        }
    }

    fn offset_branch_target(&self, offset: usize, cursor_after_operand: usize) -> BranchTarget {
        let cursor_global = self.base_offset + cursor_after_operand;
        let rel = offset as i64 - cursor_global as i64;
        let half = self.config.encode1byte as i64 / 2;
        let threshold = self.config.encode1byte as i64;
        let can_encode_relative = (0..=half).contains(&rel)
            || (rel < 0 && rel + threshold > half && rel + threshold < threshold);
        if can_encode_relative && offset >= self.base_offset {
            BranchTarget::Relative(offset - self.base_offset)
        } else {
            BranchTarget::Absolute(offset)
        }
    }
}

fn shift_before(position: usize, wide_operand_positions: &[usize]) -> usize {
    wide_operand_positions
        .iter()
        .filter(|operand_pos| **operand_pos < position)
        .count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedBranch {
    token: String,
    emits: Vec<EmitDirective>,
}

fn split_alternative_prefix(items: &[GrammarExpr]) -> Option<(&[GrammarExpr], &[GrammarExpr])> {
    if items.len() < 2 {
        return None;
    }
    let alts = alternative_items(&items[0])?;
    if alts.is_empty() {
        return None;
    }
    Some((alts, &items[1..]))
}

fn split_mid_sequence_alternative_suffix(
    items: &[GrammarExpr],
) -> Option<(&[GrammarExpr], &[GrammarExpr], &[GrammarExpr])> {
    let index = items
        .iter()
        .position(|item| alternative_items(item).is_some())?;
    if index + 1 >= items.len() {
        return None;
    }
    let alternatives = alternative_items(&items[index])?;
    Some((&items[..index], alternatives, &items[index + 1..]))
}

fn split_optional_prefix_terminal(items: &[GrammarExpr]) -> Option<(&GrammarExpr, &GrammarExpr)> {
    let [GrammarExpr::Optional(optional), terminal] = items else {
        return None;
    };
    if !matches!(
        ungroup_expr(optional),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) || !matches!(
        ungroup_expr(terminal),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) {
        return None;
    }
    Some((ungroup_expr(optional), ungroup_expr(terminal)))
}

#[derive(Debug, Clone, Copy)]
struct NodeMarkArm<'a> {
    node: &'a GrammarExpr,
    mark: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TokenMarkArm<'a> {
    token: &'a str,
    mark: u8,
}

#[derive(Debug, Clone)]
struct PrintListStatement<'a> {
    statement_emit: &'a EmitDirective,
    optional_prefix: &'a GrammarExpr,
    item_node: &'a GrammarExpr,
    separators: Vec<&'a GrammarExpr>,
    separator_emit: &'a EmitDirective,
    item_eos_emit: &'a EmitDirective,
    empty_eos_emit: &'a EmitDirective,
}

#[derive(Debug, Clone, Copy)]
struct ParenthesizedOptionalSecondArg<'a> {
    open: &'a GrammarExpr,
    first: &'a GrammarExpr,
    comma: &'a GrammarExpr,
    second: &'a GrammarExpr,
}

#[derive(Debug, Clone, Copy)]
struct ParenthesizedSingleArg<'a> {
    open: &'a GrammarExpr,
    argument: &'a GrammarExpr,
}

#[derive(Debug, Clone, Copy)]
struct ParenthesizedSharedTail<'a> {
    open: &'a GrammarExpr,
    first: &'a GrammarExpr,
    tail_offset: usize,
}

#[derive(Debug, Clone, Copy)]
struct ParenthesizedTwoPartArg<'a> {
    open: &'a GrammarExpr,
    first: &'a GrammarExpr,
    second: &'a GrammarExpr,
}

#[derive(Debug, Clone, Copy)]
struct OptionalMarkedParenList<'a> {
    open: &'a GrammarExpr,
    mark: u8,
    item: &'a GrammarExpr,
    separator: &'a GrammarExpr,
    close: &'a GrammarExpr,
    first_required: bool,
}

fn split_print_list_statement(items: &[GrammarExpr]) -> Option<PrintListStatement<'_>> {
    let [GrammarExpr::Emit(statement_emit), GrammarExpr::Optional(optional_prefix), list_alternative] =
        items
    else {
        return None;
    };
    if !is_node_expr(optional_prefix) {
        return None;
    }
    let [item_arm, empty_arm] = alternative_items(list_alternative)? else {
        return None;
    };
    let GrammarExpr::Emit(empty_eos_emit) = ungroup_expr(empty_arm) else {
        return None;
    };
    let GrammarExpr::Sequence(item_items) = ungroup_expr(item_arm) else {
        return None;
    };
    let [item_node, GrammarExpr::Repeat(repeat), GrammarExpr::Emit(item_eos_emit)] =
        item_items.as_slice()
    else {
        return None;
    };
    if !is_node_expr(item_node) {
        return None;
    }
    let GrammarExpr::Sequence(repeat_items) = ungroup_expr(repeat) else {
        return None;
    };
    let [separator_alternative, GrammarExpr::Emit(separator_emit), repeated_node] =
        repeat_items.as_slice()
    else {
        return None;
    };
    if !is_node_expr(repeated_node) || ungroup_expr(repeated_node) != ungroup_expr(item_node) {
        return None;
    }
    let separators = alternative_items(separator_alternative)?
        .iter()
        .map(ungroup_expr)
        .filter(|expr| is_node_expr(expr))
        .collect::<Vec<_>>();
    if separators.len() < 2 {
        return None;
    }
    Some(PrintListStatement {
        statement_emit,
        optional_prefix: ungroup_expr(optional_prefix),
        item_node: ungroup_expr(item_node),
        separators,
        separator_emit,
        item_eos_emit,
        empty_eos_emit,
    })
}

fn split_optional_marked_paren_list(expr: &GrammarExpr) -> Option<OptionalMarkedParenList<'_>> {
    let GrammarExpr::Optional(inner) = expr else {
        return None;
    };
    let GrammarExpr::Sequence(items) = ungroup_expr(inner) else {
        return None;
    };
    let [open, GrammarExpr::Mark(mark), middle @ .., close] = items.as_slice() else {
        return None;
    };
    if !matches!(open, GrammarExpr::TokenRef(token) if token == "tkLParen")
        || !matches!(close, GrammarExpr::TokenRef(token) if token == "tkRParen")
    {
        return None;
    }

    let (item, repeat, first_required) = match middle {
        [GrammarExpr::Optional(optional)] => {
            let GrammarExpr::Sequence(optional_items) = ungroup_expr(optional) else {
                return None;
            };
            let [item, GrammarExpr::Repeat(repeat)] = optional_items.as_slice() else {
                return None;
            };
            (item, repeat.as_ref(), false)
        }
        [item, GrammarExpr::Repeat(repeat)] => (item, repeat.as_ref(), true),
        _ => return None,
    };
    if !is_node_expr(item) {
        return None;
    }
    let GrammarExpr::Sequence(repeat_items) = ungroup_expr(repeat) else {
        return None;
    };
    let [separator, repeated_item] = repeat_items.as_slice() else {
        return None;
    };
    if !is_node_expr(separator)
        || !is_node_expr(repeated_item)
        || ungroup_expr(repeated_item) != ungroup_expr(item)
    {
        return None;
    }
    Some(OptionalMarkedParenList {
        open: ungroup_expr(open),
        mark: mark.slot,
        item: ungroup_expr(item),
        separator: ungroup_expr(separator),
        close: ungroup_expr(close),
        first_required,
    })
}

fn split_parenthesized_two_part_arg(items: &[GrammarExpr]) -> Option<ParenthesizedTwoPartArg<'_>> {
    let [open @ GrammarExpr::TokenRef(open_token), first, second, GrammarExpr::TokenRef(close_token)] =
        items
    else {
        return None;
    };
    if open_token != "tkLParen"
        || close_token != "tkRParen"
        || !is_node_expr(first)
        || !is_node_expr(second)
    {
        return None;
    }
    Some(ParenthesizedTwoPartArg {
        open,
        first: ungroup_expr(first),
        second: ungroup_expr(second),
    })
}

fn split_parenthesized_single_arg(items: &[GrammarExpr]) -> Option<ParenthesizedSingleArg<'_>> {
    let [open @ GrammarExpr::TokenRef(open_token), argument, GrammarExpr::TokenRef(close_token)] =
        items
    else {
        return None;
    };
    if open_token != "tkLParen" || close_token != "tkRParen" || !is_node_expr(argument) {
        return None;
    }
    Some(ParenthesizedSingleArg {
        open,
        argument: ungroup_expr(argument),
    })
}

fn split_parenthesized_optional_second_arg(
    items: &[GrammarExpr],
) -> Option<ParenthesizedOptionalSecondArg<'_>> {
    let [open @ GrammarExpr::TokenRef(open_token), first, GrammarExpr::Optional(optional), GrammarExpr::TokenRef(close_token)] =
        items
    else {
        return None;
    };
    if open_token != "tkLParen" || close_token != "tkRParen" || !is_node_expr(first) {
        return None;
    }
    let GrammarExpr::Sequence(optional_items) = ungroup_expr(optional) else {
        return None;
    };
    let [comma @ GrammarExpr::TokenRef(comma_token), second] = optional_items.as_slice() else {
        return None;
    };
    if comma_token != "tkComma" || !is_node_expr(second) {
        return None;
    }
    Some(ParenthesizedOptionalSecondArg {
        open,
        first: ungroup_expr(first),
        comma,
        second: ungroup_expr(second),
    })
}

fn split_node_mark_alternatives(expr: &GrammarExpr) -> Option<Vec<NodeMarkArm<'_>>> {
    let alternatives = alternative_items(expr)?;
    if alternatives.is_empty() {
        return None;
    }
    alternatives
        .iter()
        .map(|alternative| {
            let GrammarExpr::Sequence(items) = ungroup_expr(alternative) else {
                return None;
            };
            let [node, GrammarExpr::Mark(mark)] = items.as_slice() else {
                return None;
            };
            if !matches!(
                ungroup_expr(node),
                GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
            ) {
                return None;
            }
            Some(NodeMarkArm {
                node: ungroup_expr(node),
                mark: mark.slot,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileRecordOpcodes {
    default_record: String,
    record_without_id: String,
    empty_record_with_id: String,
    record_with_id: String,
}

fn file_record_statement_ops(items: &[GrammarExpr]) -> Option<FileRecordOpcodes> {
    let [GrammarExpr::NonTerminalRef(first), alternative] = items else {
        return None;
    };
    if first != "optFilenum" || alternative_items(alternative).is_none() {
        return None;
    }

    let mut names = Vec::new();
    collect_emit_idents(alternative, &mut names);
    let default_record = names
        .iter()
        .find(|name| !name.contains("Rec") && name.ends_with('1'))?;
    let record_without_id = names
        .iter()
        .find(|name| !name.contains("Rec") && name.ends_with('2'))?;
    let empty_record_with_id = names.iter().find(|name| name.ends_with("Rec2"))?;
    let record_with_id = names.iter().find(|name| name.ends_with("Rec3"))?;
    Some(FileRecordOpcodes {
        default_record: (*default_record).clone(),
        record_without_id: (*record_without_id).clone(),
        empty_record_with_id: (*empty_record_with_id).clone(),
        record_with_id: (*record_with_id).clone(),
    })
}

fn collect_emit_idents(expr: &GrammarExpr, out: &mut Vec<String>) {
    match expr {
        GrammarExpr::Emit(emit) => {
            out.extend(emit.args.iter().filter_map(|arg| match arg {
                EmitArg::Ident(ident) => Some(ident.clone()),
                EmitArg::Number(_) => None,
            }));
        }
        GrammarExpr::Sequence(items) | GrammarExpr::Alternative(items) => {
            for item in items {
                collect_emit_idents(item, out);
            }
        }
        GrammarExpr::Group(inner) | GrammarExpr::Optional(inner) | GrammarExpr::Repeat(inner) => {
            collect_emit_idents(inner, out);
        }
        _ => {}
    }
}

fn is_put_graphics_statement_sequence(items: &[GrammarExpr]) -> bool {
    let [GrammarExpr::NonTerminalRef(coord), GrammarExpr::TokenRef(comma), GrammarExpr::NonTerminalRef(id), graphics_emit, raster] =
        items
    else {
        return false;
    };

    coord == "coordStep"
        && comma == "tkComma"
        && id == "IdAryGetPut"
        && expr_is_emit_ident(graphics_emit, "opStGraphicsPut")
        && expr_contains_token(raster, "tkXOR")
}

fn is_input_prompt_repeat_sequence(items: &[GrammarExpr]) -> bool {
    let [GrammarExpr::Optional(prompt), GrammarExpr::Mark(mark), GrammarExpr::NonTerminalRef(id), input_emit, GrammarExpr::Repeat(repeat), eos_emit] =
        items
    else {
        return false;
    };

    is_input_prompt_expr(prompt)
        && mark.slot == 8
        && id == "IdAryElemRef"
        && expr_is_emit_ident(input_emit, "opStInput")
        && expr_contains_emit_ident(repeat, "opStInput")
        && expr_is_emit_ident(eos_emit, "opInputEos")
}

fn is_line_graphics_statement_sequence(items: &[GrammarExpr]) -> bool {
    let [GrammarExpr::Optional(coord), GrammarExpr::TokenRef(minus), GrammarExpr::NonTerminalRef(coord2), GrammarExpr::Optional(options)] =
        items
    else {
        return false;
    };

    matches!(ungroup_expr(coord), GrammarExpr::NonTerminalRef(name) if name == "coordStep")
        && minus == "tkMinus"
        && coord2 == "coord2Step"
        && expr_contains_mark_slot(options, 4)
}

fn is_line_input_prompt_sequence(items: &[GrammarExpr]) -> bool {
    let [GrammarExpr::TokenRef(input), GrammarExpr::Optional(prompt), GrammarExpr::NonTerminalRef(id)] =
        items
    else {
        return false;
    };

    input == "tkINPUT" && id == "IdAryElemRef" && is_input_prompt_expr(prompt)
}

fn is_input_prompt_expr(expr: &GrammarExpr) -> bool {
    expr_contains_nonterminal(expr, "lbsInpExpComma")
        && expr_contains_token(expr, "tkSColon")
        && expr_contains_nonterminal(expr, "LitString")
        && expr_contains_mark_slot(expr, 16)
        && expr_contains_mark_slot(expr, 4)
        && expr_contains_mark_slot(expr, 2)
        && expr_contains_mark_slot(expr, 1)
}

fn split_marked_range_sequence(items: &[GrammarExpr]) -> Option<&GrammarExpr> {
    let [prefix, GrammarExpr::Optional(options)] = items else {
        return None;
    };
    if !matches!(
        ungroup_expr(prefix),
        GrammarExpr::TokenRef(_) | GrammarExpr::NonTerminalRef(_)
    ) {
        return None;
    }

    (expr_contains_token(options, "tkComma")
        && expr_contains_token(options, "tkTO")
        && expr_contains_nonterminal(options, "Exp")
        && expr_contains_mark_slot(options, 1)
        && expr_contains_mark_slot(options, 2)
        && expr_contains_mark_slot(options, 3))
    .then_some(ungroup_expr(prefix))
}

fn is_open_statement_sequence(items: &[GrammarExpr]) -> bool {
    let [GrammarExpr::NonTerminalRef(first), alternative] = items else {
        return false;
    };

    first == "Exp"
        && alternative_items(alternative).is_some()
        && expr_contains_mark_slot(alternative, 14)
        && expr_contains_mark_slot(alternative, 13)
        && expr_contains_token(alternative, "tkACCESS")
        && expr_contains_token(alternative, "tkLOCK")
}

fn is_paint_statement_sequence(items: &[GrammarExpr]) -> bool {
    let [GrammarExpr::NonTerminalRef(coord), GrammarExpr::NonTerminalRef(first), GrammarExpr::NonTerminalRef(second), alternative] =
        items
    else {
        return false;
    };

    coord == "coordStep"
        && first == "commaOptExp"
        && second == "commaOptExp"
        && expr_contains_emit_ident(alternative, "opStPaint2")
        && expr_contains_emit_ident(alternative, "opStPaint3")
}

fn is_open_statement_alternative(alternatives: &[GrammarExpr]) -> bool {
    let [modern, legacy] = alternatives else {
        return false;
    };

    expr_starts_with(modern, "Exp")
        && expr_contains_token(modern, "tkACCESS")
        && expr_contains_token(modern, "tkLOCK")
        && expr_contains_mark_slot(modern, 13)
        && expr_contains_token(legacy, "tkComma")
        && expr_contains_mark_slot(legacy, 14)
}

include!("buildprs_lowering_patterns.rs");

fn indexed_branches(expr: &GrammarExpr) -> Result<Vec<IndexedBranch>, LoweringError> {
    let alts = match expr {
        GrammarExpr::Alternative(items) => items.as_slice(),
        GrammarExpr::Group(inner) => match inner.as_ref() {
            GrammarExpr::Alternative(items) => items.as_slice(),
            _ => return Err(LoweringError::UnsupportedExpr("indexed nonterminal")),
        },
        _ => return Err(LoweringError::UnsupportedExpr("indexed nonterminal")),
    };

    alts.iter()
        .map(|alt| {
            let GrammarExpr::Group(inner) = alt else {
                return Err(LoweringError::UnsupportedExpr("indexed alternative"));
            };
            let GrammarExpr::Sequence(items) = inner.as_ref() else {
                return Err(LoweringError::UnsupportedExpr("indexed alternative"));
            };
            let (token, emits) = parse_indexed_sequence(items)?;
            Ok(IndexedBranch { token, emits })
        })
        .collect()
}

fn node_id_encoded_len(node_id: u16, config: &LoweringConfig) -> usize {
    if node_id < config.encode1byte as u16 {
        1
    } else {
        2
    }
}

fn indexed_header_size(
    branches: &[IndexedBranch],
    symbols: &LoweringSymbols,
    config: &LoweringConfig,
) -> Result<usize, LoweringError> {
    let header_bytes = branches
        .iter()
        .map(|branch| {
            let node_id = symbols.node_id_for_token(&branch.token, config)?;
            Ok(node_id_encoded_len(node_id, config) + 1)
        })
        .sum::<Result<usize, LoweringError>>()?;
    Ok(header_bytes)
}

fn indexed_body_size(branches: &[IndexedBranch]) -> usize {
    let emit_bytes = branches
        .iter()
        .map(|branch| branch.emits.len() * 3)
        .max()
        .unwrap_or(0);
    emit_bytes + 1
}

fn parse_indexed_sequence(
    items: &[GrammarExpr],
) -> Result<(String, Vec<EmitDirective>), LoweringError> {
    let mut iter = items.iter();
    let token = match iter.next() {
        Some(GrammarExpr::TokenRef(token)) => token.clone(),
        _ => return Err(LoweringError::UnsupportedExpr("indexed alternative")),
    };
    let emits = iter
        .filter_map(|item| match item {
            GrammarExpr::Emit(emit) => Some(emit.clone()),
            _ => None,
        })
        .collect();
    Ok((token, emits))
}

fn resolve_emit_arg(opcodes: &BTreeMap<String, u16>, arg: &EmitArg) -> Result<u16, LoweringError> {
    match arg {
        EmitArg::Number(value) => Ok(*value as u16),
        EmitArg::Ident(name) => {
            if let Some(value) = lookup_opcode(opcodes, name) {
                return Ok(*value);
            }
            if let Some(value) = known_emit_symbol(name) {
                return Ok(value);
            }
            Err(LoweringError::UnknownEmitSymbol(name.clone()))
        }
    }
}

fn lookup_opcode<'a>(opcodes: &'a BTreeMap<String, u16>, symbol: &str) -> Option<&'a u16> {
    opcodes.get(symbol).or_else(|| {
        opcodes
            .iter()
            .find_map(|(name, value)| name.eq_ignore_ascii_case(symbol).then_some(value))
    })
}

fn known_emit_symbol(symbol: &str) -> Option<u16> {
    match symbol.to_ascii_uppercase().as_str() {
        "UNDEFINED" => Some(0xffff),
        "ET_IMP" => Some(0),
        "ET_I2" => Some(1),
        "ET_I4" => Some(2),
        "ET_R4" => Some(3),
        "ET_R8" => Some(4),
        "ET_SD" => Some(5),
        "ET_FS" => Some(6),
        _ => None,
    }
}

fn parse_irw_equates(text: &str) -> Result<BTreeMap<String, u16>, LoweringError> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("IRW_") {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let name = parts
            .next()
            .unwrap_or_default()
            .trim_end_matches(':')
            .to_string();
        let _eq = parts.next();
        let value = parts.next().ok_or_else(|| {
            LoweringError::ArtifactParse(format!("malformed IRW line `{trimmed}`"))
        })?;
        let parsed = parse_asm_number(value)?;
        out.insert(name.clone(), parsed);
        if name.starts_with("IRW_") {
            let token = format!("tk{}", &name[4..]);
            out.entry(token).or_insert(parsed);
        }
    }
    Ok(out)
}

fn parse_int_nt_order(text: &str) -> Result<BTreeMap<String, u16>, LoweringError> {
    parse_named_dw_table(text, "tIntNtDisp")
}

fn parse_ext_nt_order(text: &str) -> Result<BTreeMap<String, u16>, LoweringError> {
    let mut out = BTreeMap::new();
    let mut in_section = false;
    let mut index = 0u16;

    for line in text.lines() {
        let trimmed = line.split(';').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if !in_section {
            if label_declares(trimmed, "tExtNtDisp") {
                in_section = true;
            }
            continue;
        }
        if is_label_declaration(trimmed) && !trimmed.starts_with("DW") {
            break;
        }
        if let Some(name) = trimmed.strip_prefix("DW") {
            let symbol = name.trim().trim_start_matches("Nt");
            if !symbol.is_empty() && !symbol.chars().all(|ch| ch.is_ascii_digit()) {
                out.insert(symbol.to_string(), index);
                index += 1;
            }
        }
    }

    Ok(out)
}

fn parse_named_dw_table(text: &str, label: &str) -> Result<BTreeMap<String, u16>, LoweringError> {
    let mut out = BTreeMap::new();
    let mut in_section = false;
    let mut index = 0u16;

    for line in text.lines() {
        let trimmed = line.split(';').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if !in_section {
            if label_declares(trimmed, label) {
                in_section = true;
            }
            continue;
        }
        if is_label_declaration(trimmed) && !trimmed.to_ascii_lowercase().starts_with("dw") {
            break;
        }
        if let Some(comment_name) = line.split(';').nth(1) {
            let name = comment_name.trim();
            if !name.is_empty() {
                out.insert(name.to_string(), index);
                index += 1;
            }
        }
    }

    Ok(out)
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

/// Outcome of lowering one statement or function rule in a whole-grammar pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleLoweringOutcome {
    Lowered {
        anchor: String,
        offset: u16,
        byte_len: usize,
    },
    Failed {
        anchor: String,
        error: LoweringError,
    },
}

/// Outcome of lowering one internal nonterminal in a whole-grammar pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalNtLoweringOutcome {
    Lowered {
        name: String,
        offset: u16,
        byte_len: usize,
        indexed: bool,
    },
    Failed {
        name: String,
        error: LoweringError,
    },
}

/// Aggregate coverage counters for a whole-grammar lowering run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringCoverageTotals {
    pub statements_total: usize,
    pub statements_lowered: usize,
    pub functions_total: usize,
    pub functions_lowered: usize,
    pub internal_nt_total: usize,
    pub internal_nt_lowered: usize,
    pub state_bytes: usize,
}

impl LoweringCoverageTotals {
    pub fn statements_failed(&self) -> usize {
        self.statements_total
            .saturating_sub(self.statements_lowered)
    }

    pub fn functions_failed(&self) -> usize {
        self.functions_total.saturating_sub(self.functions_lowered)
    }

    pub fn internal_nt_failed(&self) -> usize {
        self.internal_nt_total
            .saturating_sub(self.internal_nt_lowered)
    }
}

/// Counts of unsupported grammar shapes grouped by lowering error kind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsupportedShapeSummary {
    pub unsupported_expr: BTreeMap<&'static str, usize>,
    pub unknown_token: usize,
    pub unknown_nonterminal: usize,
    pub unknown_emit_symbol: usize,
    pub encode: usize,
    pub other: usize,
}

impl UnsupportedShapeSummary {
    pub fn record(&mut self, error: &LoweringError) {
        match error {
            LoweringError::UnsupportedExpr(kind) => {
                *self.unsupported_expr.entry(kind).or_default() += 1;
            }
            LoweringError::UnknownToken(_) => self.unknown_token += 1,
            LoweringError::UnknownNonTerminal(_) => self.unknown_nonterminal += 1,
            LoweringError::UnknownEmitSymbol(_) => self.unknown_emit_symbol += 1,
            LoweringError::Encode(_) => self.encode += 1,
            LoweringError::EmptyEmit | LoweringError::ArtifactParse(_) => self.other += 1,
        }
    }

    pub fn total_failures(&self) -> usize {
        self.unsupported_expr.values().sum::<usize>()
            + self.unknown_token
            + self.unknown_nonterminal
            + self.unknown_emit_symbol
            + self.encode
            + self.other
    }
}

/// Structured progress report from a whole-grammar lowering attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeGrammarLoweringReport {
    pub statements: Vec<RuleLoweringOutcome>,
    pub functions: Vec<RuleLoweringOutcome>,
    pub internal_nonterminals: Vec<InternalNtLoweringOutcome>,
    pub totals: LoweringCoverageTotals,
    pub unsupported_shapes: UnsupportedShapeSummary,
}

impl WholeGrammarLoweringReport {
    pub fn lowered_statement_anchors(&self) -> impl Iterator<Item = &str> {
        self.statements.iter().filter_map(|outcome| match outcome {
            RuleLoweringOutcome::Lowered { anchor, .. } => Some(anchor.as_str()),
            RuleLoweringOutcome::Failed { .. } => None,
        })
    }

    pub fn lowered_function_anchors(&self) -> impl Iterator<Item = &str> {
        self.functions.iter().filter_map(|outcome| match outcome {
            RuleLoweringOutcome::Lowered { anchor, .. } => Some(anchor.as_str()),
            RuleLoweringOutcome::Failed { .. } => None,
        })
    }
}

/// Contiguous `tState` buffer plus offset maps produced by whole-grammar lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeGrammarLoweringResult {
    pub state: Vec<u8>,
    /// Byte offset into [`Self::state`] for each lowered statement anchor (`tk*`).
    /// Only holds the FIRST offset when multiple statement rules share an anchor.
    /// Use [`Self::statement_offset_list`] for the full ordered list (multiple per anchor).
    pub statement_offsets: BTreeMap<String, u16>,
    /// All (anchor, offset) pairs in grammar order, including multiple entries per anchor.
    /// When two grammar rules share the same keyword (e.g. file GET and graphics GET both
    /// start with `tkGET`), this list contains two entries for `tkGET`.  The runtime parser
    /// tries each offset in order and stops at the first successful parse.
    pub statement_offset_list: Vec<(String, u16)>,
    /// Byte offset into [`Self::state`] for each lowered function anchor (`tk*`).
    pub function_offsets: BTreeMap<String, u16>,
    /// `tIntNtDisp`-ordered offsets for successfully lowered internal nonterminals.
    pub int_nt_disp: BTreeMap<String, u16>,
    /// `STI_*` offsets for successfully lowered `<INDEX>` internal nonterminals.
    pub sti_offsets: BTreeMap<String, u16>,
    pub report: WholeGrammarLoweringReport,
}

/// Lower all statement rules, function rules, and internal nonterminals into one
/// contiguous `tState` buffer in DOS `buildprs` emission order.
///
/// Rules that hit unsupported grammar shapes are skipped in the output buffer but
/// recorded in [`WholeGrammarLoweringResult::report`].
pub fn lower_whole_grammar(
    grammar: &GrammarFile,
    symbols: LoweringSymbols,
    config: LoweringConfig,
) -> WholeGrammarLoweringResult {
    lower_whole_grammar_with_shared_suffixes(
        grammar,
        symbols,
        config,
        SharedSuffixRegistry::default(),
    )
}

pub fn lower_whole_grammar_with_shared_suffixes(
    grammar: &GrammarFile,
    symbols: LoweringSymbols,
    config: LoweringConfig,
    shared_suffixes: SharedSuffixRegistry,
) -> WholeGrammarLoweringResult {
    let dispatch = derive_nonterminal_dispatch_order(grammar);
    let mut state = Vec::new();
    let mut statement_offsets = BTreeMap::new();
    let mut statement_offset_list: Vec<(String, u16)> = Vec::new();
    let mut function_offsets = BTreeMap::new();
    let mut int_nt_disp = BTreeMap::new();
    let mut sti_offsets = BTreeMap::new();
    let mut unsupported_shapes = UnsupportedShapeSummary::default();
    let enable_rule_aliasing = !shared_suffixes.offsets.is_empty();
    let mut rule_aliases = BTreeMap::<Vec<u8>, u16>::new();

    let mut statements = Vec::new();
    for rule in &grammar.statements.rules {
        let outcome = lower_rule_with_symbols(
            &mut state,
            &mut statement_offsets,
            &mut statement_offset_list,
            rule,
            &symbols,
            config,
            &shared_suffixes,
            if enable_rule_aliasing {
                Some(&mut rule_aliases)
            } else {
                None
            },
            |builder| builder.lower_statement_rule(rule),
        );
        if let RuleLoweringOutcome::Failed { error, .. } = &outcome {
            unsupported_shapes.record(error);
        }
        statements.push(outcome);
    }

    let mut function_offset_list: Vec<(String, u16)> = Vec::new();
    let mut functions = Vec::new();
    for rule in &grammar.functions.rules {
        let outcome = lower_rule_with_symbols(
            &mut state,
            &mut function_offsets,
            &mut function_offset_list,
            rule,
            &symbols,
            config,
            &shared_suffixes,
            if enable_rule_aliasing {
                Some(&mut rule_aliases)
            } else {
                None
            },
            |builder| builder.lower_statement_rule(rule),
        );
        if let RuleLoweringOutcome::Failed { error, .. } = &outcome {
            unsupported_shapes.record(error);
        }
        functions.push(outcome);
    }

    let mut internal_nonterminals = Vec::new();
    for entry in &dispatch.internal {
        let nt = grammar
            .nonterminals
            .iter()
            .find(|candidate| candidate.name == entry.name)
            .expect("dispatch order should reference grammar nonterminals");
        let outcome = lower_internal_nt_into_buffer(
            &mut state,
            &mut int_nt_disp,
            &mut sti_offsets,
            nt,
            symbols.clone(),
            config,
            &shared_suffixes,
        );
        if let InternalNtLoweringOutcome::Failed { error, .. } = &outcome {
            unsupported_shapes.record(error);
        }
        internal_nonterminals.push(outcome);
    }

    let statements_lowered = statements
        .iter()
        .filter(|outcome| matches!(outcome, RuleLoweringOutcome::Lowered { .. }))
        .count();
    let functions_lowered = functions
        .iter()
        .filter(|outcome| matches!(outcome, RuleLoweringOutcome::Lowered { .. }))
        .count();
    let internal_nt_lowered = internal_nonterminals
        .iter()
        .filter(|outcome| matches!(outcome, InternalNtLoweringOutcome::Lowered { .. }))
        .count();
    let state_bytes = state.len();

    WholeGrammarLoweringResult {
        state,
        statement_offsets,
        statement_offset_list,
        function_offsets,
        int_nt_disp,
        sti_offsets,
        report: WholeGrammarLoweringReport {
            statements,
            functions,
            internal_nonterminals,
            totals: LoweringCoverageTotals {
                statements_total: grammar.statements.rules.len(),
                statements_lowered,
                functions_total: grammar.functions.rules.len(),
                functions_lowered,
                internal_nt_total: dispatch.internal.len(),
                internal_nt_lowered,
                state_bytes,
            },
            unsupported_shapes,
        },
    }
}

fn lower_rule_with_symbols(
    state: &mut Vec<u8>,
    offsets: &mut BTreeMap<String, u16>,
    offset_list: &mut Vec<(String, u16)>,
    rule: &GrammarRule,
    symbols: &LoweringSymbols,
    config: LoweringConfig,
    shared_suffixes: &SharedSuffixRegistry,
    mut rule_aliases: Option<&mut BTreeMap<Vec<u8>, u16>>,
    lower: impl FnOnce(&mut LoweringBuilder) -> Result<(), LoweringError>,
) -> RuleLoweringOutcome {
    let offset = match u16::try_from(state.len()) {
        Ok(offset) => offset,
        Err(_) => {
            return RuleLoweringOutcome::Failed {
                anchor: rule.anchor.clone(),
                error: LoweringError::ArtifactParse("tState buffer exceeds u16".to_string()),
            };
        }
    };

    let mut builder =
        LoweringBuilder::with_shared_suffixes(symbols.clone(), config, shared_suffixes.clone())
            .with_base_offset(offset as usize);
    match lower(&mut builder) {
        Ok(()) => match builder.finish() {
            Ok(bytes) => {
                let byte_len = bytes.len();
                let offset = if allow_rule_alias(rule) {
                    if let Some(rule_aliases) = rule_aliases.as_deref_mut() {
                        if let Some(existing_offset) = rule_aliases.get(&bytes).copied() {
                            existing_offset
                        } else {
                            rule_aliases.insert(bytes.clone(), offset);
                            state.extend(bytes);
                            offset
                        }
                    } else {
                        state.extend(bytes);
                        offset
                    }
                } else {
                    if let Some(rule_aliases) = rule_aliases.as_deref_mut() {
                        rule_aliases.entry(bytes.clone()).or_insert(offset);
                    }
                    state.extend(bytes);
                    offset
                };
                // Always record in the ordered list (allows multiple entries per anchor).
                offset_list.push((rule.anchor.clone(), offset));
                // Only insert the FIRST offset into the BTreeMap so existing callers
                // that key by anchor name still get a deterministic value.
                offsets.entry(rule.anchor.clone()).or_insert(offset);
                RuleLoweringOutcome::Lowered {
                    anchor: rule.anchor.clone(),
                    offset,
                    byte_len,
                }
            }
            Err(error) => RuleLoweringOutcome::Failed {
                anchor: rule.anchor.clone(),
                error,
            },
        },
        Err(error) => RuleLoweringOutcome::Failed {
            anchor: rule.anchor.clone(),
            error,
        },
    }
}

fn allow_rule_alias(rule: &GrammarRule) -> bool {
    matches!(rule.anchor.as_str(), "tkCOLOR" | "tkPSET")
        || matches!(
            &rule.production.expr,
            GrammarExpr::NonTerminalRef(name) if matches!(name.as_str(), "fn23arg" | "fnBoundArg")
        )
}

fn lower_internal_nt_into_buffer(
    state: &mut Vec<u8>,
    int_nt_disp: &mut BTreeMap<String, u16>,
    sti_offsets: &mut BTreeMap<String, u16>,
    nt: &NonTerminalDef,
    symbols: LoweringSymbols,
    config: LoweringConfig,
    shared_suffixes: &SharedSuffixRegistry,
) -> InternalNtLoweringOutcome {
    let offset = match u16::try_from(state.len()) {
        Ok(offset) => offset,
        Err(_) => {
            return InternalNtLoweringOutcome::Failed {
                name: nt.name.clone(),
                error: LoweringError::ArtifactParse("tState buffer exceeds u16".to_string()),
            };
        }
    };

    let mut builder =
        LoweringBuilder::with_shared_suffixes(symbols.clone(), config, shared_suffixes.clone())
            .with_base_offset(offset as usize);
    match builder.lower_nonterminal(nt) {
        Ok(()) => match builder.finish() {
            Ok(bytes) => {
                let mut byte_len = bytes.len();
                let offset = if let Some(alias_offset) =
                    internal_shared_body_offset(nt, shared_suffixes, usize::from(offset))
                {
                    match lower_internal_nt_bytes(
                        nt,
                        symbols.clone(),
                        config,
                        shared_suffixes,
                        alias_offset,
                    ) {
                        Ok(alias_bytes) => byte_len = alias_bytes.len(),
                        Err(error) => {
                            return InternalNtLoweringOutcome::Failed {
                                name: nt.name.clone(),
                                error,
                            };
                        }
                    }
                    match u16::try_from(alias_offset) {
                        Ok(alias_offset) => alias_offset,
                        Err(_) => {
                            return InternalNtLoweringOutcome::Failed {
                                name: nt.name.clone(),
                                error: LoweringError::ArtifactParse(
                                    "tState buffer exceeds u16".to_string(),
                                ),
                            };
                        }
                    }
                } else {
                    byte_len =
                        logical_internal_byte_len(usize::from(offset), byte_len, int_nt_disp);
                    state.extend(bytes);
                    offset
                };
                int_nt_disp.insert(nt.name.clone(), offset);
                if nt.has_index {
                    sti_offsets.insert(nt.name.clone(), offset);
                }
                InternalNtLoweringOutcome::Lowered {
                    name: nt.name.clone(),
                    offset,
                    byte_len,
                    indexed: nt.has_index,
                }
            }
            Err(error) => InternalNtLoweringOutcome::Failed {
                name: nt.name.clone(),
                error,
            },
        },
        Err(error) => InternalNtLoweringOutcome::Failed {
            name: nt.name.clone(),
            error,
        },
    }
}

fn logical_internal_byte_len(
    offset: usize,
    byte_len: usize,
    int_nt_disp: &BTreeMap<String, u16>,
) -> usize {
    let end = offset + byte_len;
    int_nt_disp
        .values()
        .map(|existing_offset| usize::from(*existing_offset))
        .filter(|existing_offset| offset < *existing_offset && *existing_offset < end)
        .min()
        .map_or(byte_len, |existing_offset| existing_offset - offset)
}

fn lower_internal_nt_bytes(
    nt: &NonTerminalDef,
    symbols: LoweringSymbols,
    config: LoweringConfig,
    shared_suffixes: &SharedSuffixRegistry,
    base_offset: usize,
) -> Result<Vec<u8>, LoweringError> {
    let mut builder =
        LoweringBuilder::with_shared_suffixes(symbols, config, shared_suffixes.clone())
            .with_base_offset(base_offset);
    builder.lower_nonterminal(nt)?;
    builder.finish()
}

fn internal_shared_body_offset(
    nt: &NonTerminalDef,
    shared_suffixes: &SharedSuffixRegistry,
    current_offset: usize,
) -> Option<usize> {
    let production = nt.body.productions.first()?;
    let key = match &production.expr {
        GrammarExpr::Sequence(items) => expr_suffix_key(items),
        expr => vec![expr_key(expr)],
    };
    let offset = shared_suffixes.offset_for(&key)?;
    (offset > current_offset).then_some(offset)
}

fn parse_asm_number(token: &str) -> Result<u16, LoweringError> {
    let token = token.trim();
    if let Some(hex) = token.strip_suffix('H').or_else(|| token.strip_suffix('h')) {
        return u16::from_str_radix(hex, 16)
            .map_err(|error| LoweringError::ArtifactParse(error.to_string()));
    }
    token
        .parse::<u16>()
        .map_err(|error| LoweringError::ArtifactParse(error.to_string()))
}

#[cfg(test)]
#[path = "buildprs_lowering_tests.rs"]
mod tests;
