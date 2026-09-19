use buildprs::buildprs_encoder::{BranchTarget, EncodeConfig, StateDecoder, StateEntry};
use std::collections::BTreeSet;

use crate::syntax::{
    Declaration, Expr, Parameter, Procedure, ProcedureKind, Span, Statement, TypeName,
};

use super::lexer::{Token, TokenKind};
use super::tables;

const ND_BRANCH: usize = 4;
const ENCODE1BYTE: u8 = 224;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseResult {
    GoodSyntax,
    NotFound,
    BadSyntax,
}

#[derive(Default)]
pub(crate) struct AstSink {
    pub actions: Vec<tables::AstAction>,
}

impl AstSink {
    fn checkpoint(&self) -> usize {
        self.actions.len()
    }

    fn rollback(&mut self, checkpoint: usize) {
        self.actions.truncate(checkpoint);
    }

    fn invoke(&mut self, action: tables::AstAction) {
        self.actions.push(action);
    }
}

#[derive(Clone)]
pub(crate) struct Checkpoint {
    at: usize,
    sink: usize,
    expressions: usize,
    statements: usize,
    declarations: usize,
    labels: usize,
    procedure_references: usize,
    type_names: usize,
    literal_values: usize,
    parameters: usize,
    procedure_headers: usize,
    procedures: usize,
    open_procedure: Option<usize>,
    dynamic_arrays: bool,
    declaration_shared: bool,
    declaration_form: Option<DeclarationForm>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeclarationForm {
    Dim,
    Redim,
    Static,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcedureHeader {
    pub name: String,
    pub kind: ProcedureKind,
    pub declaration: bool,
    pub result: Option<TypeName>,
    pub span: Span,
}

pub(crate) struct ParseState {
    pub tokens: Vec<Token>,
    pub at: usize,
    pub sink: AstSink,
    pub expressions: Vec<Expr>,
    pub statements: Vec<Statement>,
    pub declarations: Vec<Declaration>,
    pub labels: Vec<(String, Span)>,
    pub procedure_references: Vec<(String, Span)>,
    pub type_names: Vec<(String, Span)>,
    pub literal_values: Vec<i64>,
    pub parameters: Vec<Parameter>,
    pub procedure_headers: Vec<ProcedureHeader>,
    pub procedures: Vec<Procedure>,
    pub open_procedure: Option<usize>,
    pub dynamic_arrays: bool,
    pub declaration_shared: bool,
    pub declaration_form: Option<DeclarationForm>,
    pub unsupported_external: BTreeSet<&'static str>,
}

impl ParseState {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            at: 0,
            sink: AstSink::default(),
            expressions: Vec::new(),
            statements: Vec::new(),
            declarations: Vec::new(),
            labels: Vec::new(),
            procedure_references: Vec::new(),
            type_names: Vec::new(),
            literal_values: Vec::new(),
            parameters: Vec::new(),
            procedure_headers: Vec::new(),
            procedures: Vec::new(),
            open_procedure: None,
            dynamic_arrays: false,
            declaration_shared: false,
            declaration_form: None,
            unsupported_external: BTreeSet::new(),
        }
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            at: self.at,
            sink: self.sink.checkpoint(),
            expressions: self.expressions.len(),
            statements: self.statements.len(),
            declarations: self.declarations.len(),
            labels: self.labels.len(),
            procedure_references: self.procedure_references.len(),
            type_names: self.type_names.len(),
            literal_values: self.literal_values.len(),
            parameters: self.parameters.len(),
            procedure_headers: self.procedure_headers.len(),
            procedures: self.procedures.len(),
            open_procedure: self.open_procedure,
            dynamic_arrays: self.dynamic_arrays,
            declaration_shared: self.declaration_shared,
            declaration_form: self.declaration_form,
        }
    }

    pub fn rollback(&mut self, checkpoint: Checkpoint) {
        self.at = checkpoint.at;
        self.sink.rollback(checkpoint.sink);
        self.expressions.truncate(checkpoint.expressions);
        self.statements.truncate(checkpoint.statements);
        self.declarations.truncate(checkpoint.declarations);
        self.labels.truncate(checkpoint.labels);
        self.procedure_references
            .truncate(checkpoint.procedure_references);
        self.type_names.truncate(checkpoint.type_names);
        self.literal_values.truncate(checkpoint.literal_values);
        self.parameters.truncate(checkpoint.parameters);
        self.procedure_headers
            .truncate(checkpoint.procedure_headers);
        self.procedures.truncate(checkpoint.procedures);
        self.open_procedure = checkpoint.open_procedure;
        self.dynamic_arrays = checkpoint.dynamic_arrays;
        self.declaration_shared = checkpoint.declaration_shared;
        self.declaration_form = checkpoint.declaration_form;
    }

    pub fn token(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    pub fn token_id(&self) -> Option<u16> {
        match self.token()?.kind {
            TokenKind::Reserved(id) => Some(id),
            _ => None,
        }
    }

    pub fn consume_id(&mut self, id: u16) -> bool {
        if self.token_id() == Some(id) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn mark(&mut self, slot: u8) {
        self.sink.invoke(tables::AstAction::Mark {
            slot,
            token: self.at,
        });
    }

    fn emit(&mut self, word: u16) {
        for action in tables::emit_actions(word).into_iter().flatten() {
            self.sink.invoke(action);
        }
    }
}

pub(crate) struct ParserEngine {
    state: &'static [u8],
    internal: &'static [u16],
    external: &'static [&'static str],
    encode1byte: u8,
}

impl ParserEngine {
    pub const fn new() -> Self {
        Self {
            state: tables::T_STATE,
            internal: tables::T_INT_NT_DISP,
            external: tables::T_EXT_NT_DISP,
            encode1byte: ENCODE1BYTE,
        }
    }

    fn int_base(&self) -> usize {
        ND_BRANCH + 1
    }

    fn ext_base(&self) -> usize {
        self.int_base() + self.internal.len()
    }

    fn reserved_base(&self) -> usize {
        self.ext_base() + self.external.len()
    }

    pub fn parse(&self, state: &mut ParseState, offset: usize) -> ParseResult {
        let parse_entry = state.checkpoint();
        let config = EncodeConfig::new(self.encode1byte);
        let mut decoder = StateDecoder::with_config_at(self.state, config, offset);
        loop {
            let entry = match decoder.next() {
                Ok(Some(entry)) => entry,
                Ok(None) | Err(_) => return reject(state, parse_entry.clone()),
            };
            match entry {
                StateEntry::Accept => return ParseResult::GoodSyntax,
                StateEntry::Reject => return reject(state, parse_entry.clone()),
                StateEntry::Mark(slot) => state.mark(slot),
                StateEntry::Emit(word) => state.emit(word),
                StateEntry::Branch(target) => match target {
                    BranchTarget::Accept => return ParseResult::GoodSyntax,
                    BranchTarget::Relative(next) | BranchTarget::Absolute(next) => {
                        decoder = StateDecoder::with_config_at(self.state, config, next);
                    }
                },
                StateEntry::Node { node_id, branch } => {
                    let child_entry = state.checkpoint();
                    let id = usize::from(node_id);
                    let result = if id >= self.reserved_base() {
                        let wanted = (id - self.reserved_base()) as u16;
                        if state.consume_id(wanted) {
                            ParseResult::GoodSyntax
                        } else {
                            ParseResult::NotFound
                        }
                    } else if id >= self.ext_base() {
                        let index = id - self.ext_base();
                        self.external
                            .get(index)
                            .map_or(ParseResult::NotFound, |name| {
                                let Some(action) = tables::external_action(name) else {
                                    state.unsupported_external.insert(name);
                                    return ParseResult::NotFound;
                                };
                                super::ast::external_action(action, self, state)
                            })
                    } else if id >= self.int_base() {
                        let index = id - self.int_base();
                        self.internal
                            .get(index)
                            .map_or(ParseResult::NotFound, |next| {
                                self.parse(state, usize::from(*next))
                            })
                    } else {
                        ParseResult::NotFound
                    };
                    match result {
                        ParseResult::GoodSyntax => match branch {
                            BranchTarget::Accept => return ParseResult::GoodSyntax,
                            BranchTarget::Relative(next) | BranchTarget::Absolute(next) => {
                                decoder = StateDecoder::with_config_at(self.state, config, next);
                            }
                        },
                        ParseResult::NotFound => state.rollback(child_entry),
                        ParseResult::BadSyntax => return ParseResult::BadSyntax,
                    }
                }
            }
        }
    }
}

fn reject(state: &mut ParseState, entry: Checkpoint) -> ParseResult {
    if state.at == entry.at {
        state.rollback(entry);
        ParseResult::NotFound
    } else {
        ParseResult::BadSyntax
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildprs::buildprs_encoder::StateEncoder;

    #[test]
    fn generated_tables_retain_the_recovered_qbasic_dimensions() {
        assert_eq!(tables::T_STATE.len(), 2941);
        assert_eq!(tables::T_INT_NT_DISP.len(), 29);
        assert_eq!(tables::T_EXT_NT_DISP.len(), 49);
        assert_eq!(tables::TOKENS.len(), 246);
        assert_eq!(
            tables::emit_actions(0),
            [Some(tables::AstAction::Unsupported("opBol")), None]
        );
        assert_eq!(
            tables::emit_actions(u16::MAX),
            [Some(tables::AstAction::OperandPlaceholder), None]
        );
    }

    #[test]
    fn rejected_alternative_restores_emitted_actions_and_ast_builder() {
        let config = EncodeConfig::new(ENCODE1BYTE);
        let mut encoded = StateEncoder::with_config(config);
        encoded.emit(0);
        encoded
            .node(
                u16::try_from(ND_BRANCH + 1 + usize::from(tables::token_id("tkLET").unwrap()))
                    .unwrap(),
                BranchTarget::Accept,
            )
            .unwrap();
        encoded.reject();
        let bytes = Box::leak(encoded.into_bytes().into_boxed_slice());
        let engine = ParserEngine {
            state: bytes,
            internal: &[],
            external: &[],
            encode1byte: ENCODE1BYTE,
        };
        let tokens = crate::generated_parser::lex("", crate::dialect::Dialect::QuickBasic45)
            .unwrap();
        let mut state = ParseState::new(tokens);
        state.expressions.push(Expr::Name(
            "SENTINEL".into(),
            crate::syntax::Span {
                line: 1,
                start: 0,
                end: 0,
            },
        ));

        assert_eq!(engine.parse(&mut state, 0), ParseResult::NotFound);
        assert!(state.sink.actions.is_empty());
        assert_eq!(state.expressions.len(), 1);
        assert_eq!(state.at, 0);
    }

    #[test]
    fn undefined_operand_is_not_misreported_as_a_secondary_action() {
        let config = EncodeConfig::new(ENCODE1BYTE);
        let mut encoded = StateEncoder::with_config(config);
        encoded.emit(u16::MAX);
        encoded.accept();
        let bytes = Box::leak(encoded.into_bytes().into_boxed_slice());
        let engine = ParserEngine {
            state: bytes,
            internal: &[],
            external: &[],
            encode1byte: ENCODE1BYTE,
        };
        let tokens = crate::generated_parser::lex("", crate::dialect::Dialect::QuickBasic45)
            .unwrap();
        let mut state = ParseState::new(tokens);

        assert_eq!(engine.parse(&mut state, 0), ParseResult::GoodSyntax);
        assert_eq!(
            state.sink.actions,
            vec![tables::AstAction::OperandPlaceholder]
        );
    }

    #[test]
    fn retained_fact_claims_are_backed_by_the_handwritten_source_ast() {
        for (external, facts) in tables::EXTERNAL_RETAINED_FACTS {
            assert!(!facts.is_empty(), "{external}");
            for fact in *facts {
                assert!(
                    tables::SOURCE_FACT_VOCABULARY.contains(fact),
                    "{external} claims unmodelled source fact {fact}"
                );
            }
        }
        assert!(!tables::SOURCE_FACT_VOCABULARY.contains(&"calling_convention"));
        assert!(!tables::SOURCE_FACT_VOCABULARY.contains(&"dialect_origin"));
        assert!(!tables::SOURCE_FACT_VOCABULARY.contains(&"far"));
        assert!(!tables::SOURCE_FACT_VOCABULARY.contains(&"huge"));
    }
}
