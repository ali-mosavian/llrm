//! Checked dialect additions layered over the recovered QBasic grammar.
//!
//! qbasic-port's grammar is the QBasic 1.1 grammar.  Later Microsoft BASIC
//! dialects add a small number of statements, which are declared in
//! `grammar/dialect-extensions.toml` and emitted by `build.rs`.  This module
//! deliberately only recognizes such a statement and reports its source
//! span.  The generated AST builder owns the later conversion into
//! [`crate::syntax::Statement`].

use crate::generated_parser::tables::{
    ExtensionPatternToken, ExtensionTail, GeneratedExtensionAction, DIALECT_EXTENSIONS,
};
use crate::generated_parser::{Token, TokenKind};
use crate::syntax::Span;

/// A dialect statement after the generated table has matched its full prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionMatch {
    pub action: ExtensionAction,
    /// Number of source tokens consumed by the extension itself.  The caller
    /// still owns the statement terminator (`:` or newline).
    pub consumed: usize,
}

/// Typed source actions introduced after QBasic 1.1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtensionAction {
    OptionExplicit {
        span: Span,
    },
    DefByte {
        span: Span,
    },
    OnLocalError {
        label: String,
        span: Span,
    },
    CdeclAliasFunction {
        name: String,
        alias: String,
        span: Span,
    },
    AliasFunction {
        name: String,
        alias: String,
        span: Span,
    },
    CdeclFunction {
        name: String,
        span: Span,
    },
    CdeclAliasSub {
        name: String,
        alias: String,
        span: Span,
    },
    AliasSub {
        name: String,
        alias: String,
        span: Span,
    },
    CdeclSub {
        name: String,
        span: Span,
    },
}

/// Recognize one exact extension prefix at the current statement boundary.
///
/// `None` means the input does not begin with a declared extension.  Syntax is
/// the VBDOS superset for every semantic profile, so extensions carry no
/// dialect mask; the selected [`crate::Dialect`] matters after parsing.
pub fn recognize_statement(tokens: &[Token]) -> Option<ExtensionMatch> {
    for spec in DIALECT_EXTENSIONS {
        if !pattern_matches(spec.pattern, tokens) {
            continue;
        }
        if spec.tail == ExtensionTail::None && !ends_statement(tokens.get(spec.pattern.len())) {
            continue;
        }
        let span = statement_span(&tokens[..spec.pattern.len()]);
        let action = match spec.action {
            GeneratedExtensionAction::OptionExplicit => ExtensionAction::OptionExplicit { span },
            GeneratedExtensionAction::DefByte => ExtensionAction::DefByte { span },
            GeneratedExtensionAction::OnLocalError => ExtensionAction::OnLocalError {
                label: captured_label(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated label capture matched a source label"),
                span,
            },
            GeneratedExtensionAction::CdeclAliasFunction => ExtensionAction::CdeclAliasFunction {
                name: captured_identifier(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated identifier capture matched an identifier"),
                alias: captured_string(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated string capture matched a string literal"),
                span,
            },
            GeneratedExtensionAction::AliasFunction => ExtensionAction::AliasFunction {
                name: captured_identifier(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated identifier capture matched an identifier"),
                alias: captured_string(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated string capture matched a string literal"),
                span,
            },
            GeneratedExtensionAction::CdeclFunction => ExtensionAction::CdeclFunction {
                name: captured_identifier(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated identifier capture matched an identifier"),
                span,
            },
            GeneratedExtensionAction::CdeclAliasSub => ExtensionAction::CdeclAliasSub {
                name: captured_identifier(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated identifier capture matched an identifier"),
                alias: captured_string(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated string capture matched a string literal"),
                span,
            },
            GeneratedExtensionAction::AliasSub => ExtensionAction::AliasSub {
                name: captured_identifier(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated identifier capture matched an identifier"),
                alias: captured_string(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated string capture matched a string literal"),
                span,
            },
            GeneratedExtensionAction::CdeclSub => ExtensionAction::CdeclSub {
                name: captured_identifier(spec.pattern, &tokens[..spec.pattern.len()])
                    .expect("validated identifier capture matched an identifier"),
                span,
            },
        };
        return Some(ExtensionMatch {
            action,
            consumed: spec.pattern.len(),
        });
    }
    None
}

fn ends_statement(token: Option<&Token>) -> bool {
    let Some(token) = token else {
        return true;
    };
    let newline = crate::generated_parser::tables::token_id("tkNewLine")
        .expect("recovered grammar has tkNewLine");
    let colon = crate::generated_parser::tables::token_id("tkColon")
        .expect("recovered grammar has tkColon");
    matches!(token.kind, TokenKind::Reserved(id) if id == newline || id == colon)
}

fn pattern_matches(pattern: &[ExtensionPatternToken], tokens: &[Token]) -> bool {
    pattern.len() <= tokens.len()
        && pattern
            .iter()
            .zip(tokens)
            .all(|(expected, token)| match expected {
                ExtensionPatternToken::Grammar(id) => {
                    matches!(token.kind, TokenKind::Reserved(actual) if actual == *id)
                }
                ExtensionPatternToken::Keyword(word) => {
                    matches!(token.kind, TokenKind::ExtensionKeyword(actual) if actual == *word)
                }
                ExtensionPatternToken::Label => matches!(
                    token.kind,
                    TokenKind::Identifier(_) | TokenKind::Integer(0..=65_529, None)
                ),
                ExtensionPatternToken::Identifier => matches!(token.kind, TokenKind::Identifier(_)),
                ExtensionPatternToken::StringLiteral => matches!(token.kind, TokenKind::String(_)),
            })
}

fn captured_identifier(pattern: &[ExtensionPatternToken], tokens: &[Token]) -> Option<String> {
    captured_text(pattern, tokens, ExtensionPatternToken::Identifier)
}

fn captured_string(pattern: &[ExtensionPatternToken], tokens: &[Token]) -> Option<String> {
    captured_text(pattern, tokens, ExtensionPatternToken::StringLiteral)
}

fn captured_text(
    pattern: &[ExtensionPatternToken],
    tokens: &[Token],
    wanted: ExtensionPatternToken,
) -> Option<String> {
    pattern.iter().zip(tokens).find_map(|(expected, token)| {
        if *expected != wanted {
            return None;
        }
        match &token.kind {
            TokenKind::Identifier(value) | TokenKind::String(value) => Some(value.clone()),
            _ => None,
        }
    })
}

fn captured_label(pattern: &[ExtensionPatternToken], tokens: &[Token]) -> Option<String> {
    pattern
        .iter()
        .zip(tokens)
        .find_map(|(expected, token)| match (expected, &token.kind) {
            (ExtensionPatternToken::Label, TokenKind::Identifier(name)) => Some(name.clone()),
            (ExtensionPatternToken::Label, TokenKind::Integer(value, None))
                if (0..=65_529).contains(value) =>
            {
                Some(value.to_string())
            }
            _ => None,
        })
}

fn statement_span(tokens: &[Token]) -> Span {
    let first = tokens
        .first()
        .expect("matching extension is non-empty")
        .span;
    let last = tokens.last().expect("matching extension is non-empty").span;
    Span {
        line: first.line,
        start: first.start,
        end: last.end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_parser::lex;
    use crate::Dialect;

    #[test]
    fn vbdos_option_explicit_is_table_matched_with_its_full_span() {
        let tokens = lex("option explicit\n", Dialect::VbDos).unwrap();
        let found = recognize_statement(&tokens).unwrap();
        assert_eq!(found.consumed, 2);
        assert_eq!(
            found.action,
            ExtensionAction::OptionExplicit {
                span: Span {
                    line: 1,
                    start: 0,
                    end: 15,
                },
            }
        );
    }

    #[test]
    fn option_explicit_is_inherited_by_every_semantic_profile() {
        for dialect in [
            Dialect::QBasic11,
            Dialect::QuickBasic45,
            Dialect::Pds71,
            Dialect::VbDos,
        ] {
            let tokens = lex("option explicit\n", dialect).unwrap();
            assert!(matches!(
                recognize_statement(&tokens).unwrap().action,
                ExtensionAction::OptionExplicit { .. }
            ));
        }
    }

    #[test]
    fn on_local_error_captures_symbolic_and_numeric_labels_for_pds_and_vbdos() {
        for (source, expected) in [
            ("on local error goto caughtError\n", "CAUGHTERROR"),
            ("on local error goto 100\n", "100"),
        ] {
            for dialect in [Dialect::Pds71, Dialect::VbDos] {
                let tokens = lex(source, dialect).unwrap();
                let found = recognize_statement(&tokens).unwrap();
                assert_eq!(
                    found.action,
                    ExtensionAction::OnLocalError {
                        label: expected.into(),
                        span: Span {
                            line: 1,
                            start: 0,
                            end: source.trim_end().len(),
                        },
                    }
                );
            }
        }
    }

    #[test]
    fn partial_or_non_extension_input_does_not_match() {
        let option = lex("option base 1\n", Dialect::VbDos).unwrap();
        assert_eq!(recognize_statement(&option), None);
        let explicit = lex("explicit\n", Dialect::VbDos).unwrap();
        assert_eq!(recognize_statement(&explicit), None);
        let trailing = lex("option explicit ignored\n", Dialect::VbDos).unwrap();
        assert_eq!(recognize_statement(&trailing), None);
    }
}
