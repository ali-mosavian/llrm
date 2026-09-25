use std::collections::BTreeSet;
use crate::dialect::Dialect;
use crate::dialect_extensions::{recognize_statement, ExtensionAction};
use crate::error::ParseError;
use crate::syntax::{
    Binary, Bound, CaseItem, Declaration, Expr, Haystack, Literal, Module, Parameter, PrintItem,
    PrintSeparator, Procedure, ProcedureKind, Span, Statement, TypeName, Unary,
};

use super::engine::{DeclarationForm, ParseResult, ParseState, ParserEngine, ProcedureHeader};
use super::lexer::{lex, FormatSegment, Token, TokenKind};
use super::tables::{self, AstAction, ExternalAction, StatementShape};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseOutput {
    pub module: Module,
    pub actions: Vec<AstAction>,
}

/// Parse QB-family source into the lossless syntax tree used by semantics.
pub fn parse(source: &str, dialect: Dialect) -> Result<Module, ParseError> {
    parse_vertical_slice(source, dialect).map(|output| output.module)
}

/// Parse the first generated-table vertical slice directly into local syntax.
///
/// Unsupported grammar actions return an explicit symbolic error.
pub fn parse_vertical_slice(source: &str, dialect: Dialect) -> Result<ParseOutput, ParseError> {
    let (tokens, private_at) = without_private(for_each(augmented(tuple_assignment(
        returned(lex(source, dialect).map_err(ParseError::from)?, dialect),
        dialect,
    ))));
    let mut state = ParseState::new(tokens);
    state.python_expressions = dialect.python_expressions();
    let engine = ParserEngine::new();
    while state.at < state.tokens.len() {
        while consume_named(&mut state, "tkNewLine") || consume_named(&mut state, "tkColon") {}
        if state.at >= state.tokens.len() {
            break;
        }
        state.unsupported_external.clear();
        let before = state.statements.len();
        let procedures_before = state.procedures.len();
        let open_before = state.open_procedure;
        let action_before = state.sink.actions.len();
        let private = private_at.contains(&state.at);
        let extension = recognize_statement(&state.tokens[state.at..]);
        let result = if let Some(found) = extension {
            let keyword_span = state.tokens[state.at].span;
            let function_span = state
                .tokens
                .get(state.at + 1)
                .map_or(keyword_span, |token| token.span);
            state.at += found.consumed;
            match found.action {
                ExtensionAction::OptionExplicit { .. } => {
                    state
                        .statements
                        .push(Statement::OptionExplicit(keyword_span));
                    ParseResult::GoodSyntax
                }
                ExtensionAction::DefType { type_name, .. } => def_type_list(&mut state, type_name),
                ExtensionAction::OnLocalError { label, .. } => {
                    state.statements.push(Statement::OnError {
                        label,
                        local: true,
                        span: keyword_span,
                    });
                    ParseResult::GoodSyntax
                }
                ExtensionAction::CdeclAliasFunction { name, alias, .. } => extension_procedure(
                    &mut state,
                    name,
                    Some(alias),
                    true,
                    ProcedureKind::Function,
                    function_span,
                ),
                ExtensionAction::AliasFunction { name, alias, .. } => extension_procedure(
                    &mut state,
                    name,
                    Some(alias),
                    false,
                    ProcedureKind::Function,
                    function_span,
                ),
                ExtensionAction::CdeclFunction { name, .. } => extension_procedure(
                    &mut state,
                    name,
                    None,
                    true,
                    ProcedureKind::Function,
                    function_span,
                ),
                ExtensionAction::CdeclAliasSub { name, alias, .. } => extension_procedure(
                    &mut state,
                    name,
                    Some(alias),
                    true,
                    ProcedureKind::Sub,
                    function_span,
                ),
                ExtensionAction::AliasSub { name, alias, .. } => extension_procedure(
                    &mut state,
                    name,
                    Some(alias),
                    false,
                    ProcedureKind::Sub,
                    function_span,
                ),
                ExtensionAction::CdeclSub { name, .. } => extension_procedure(
                    &mut state,
                    name,
                    None,
                    true,
                    ProcedureKind::Sub,
                    function_span,
                ),
            }
        } else {
            statement(&engine, &mut state)
        };
        match result {
            ParseResult::GoodSyntax => {}
            ParseResult::NotFound => {
                return error(&state, "statement is outside the generated AST slice")
            }
            ParseResult::BadSyntax => {
                let external = state
                    .unsupported_external
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ");
                let actions = state.sink.actions[action_before..]
                    .iter()
                    .map(|action| format!("{action:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return error(
                    &state,
                    &format!(
                        "invalid generated-grammar statement; unsupported external actions: {external}; generated actions: {actions}"
                    ),
                );
            }
        }
        if private {
            match state.procedures.get_mut(procedures_before) {
                Some(procedure) if !procedure.declaration => procedure.private = true,
                _ => return error(&state, "PRIVATE must begin a SUB or FUNCTION definition"),
            }
        }
        let mut body_added = false;
        if let Some(index) = open_before {
            if state.open_procedure == Some(index)
                && state.procedures.len() == procedures_before
                && state.statements.len() > before
            {
                let body = state.statements.split_off(before);
                state.procedures[index].body.extend(body);
                body_added = true;
            }
        }
        if state.statements.len() == before
            && state.procedures.len() == procedures_before
            && state.open_procedure == open_before
            && !body_added
        {
            let actions = state
                .sink
                .actions
                .iter()
                .map(|action| format!("{action:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            return error(
                &state,
                &format!(
                    "generated grammar accepted statement but AST actions are missing: {actions}"
                ),
            );
        }
        if !at_named(&state, "tkNewLine") && !at_named(&state, "tkColon") {
            return error(&state, "expected end of statement");
        }
    }
    if state.open_procedure.is_some() {
        return error(&state, "procedure has no matching END");
    }
    let format_strings = state
        .tokens
        .iter()
        .any(|token| matches!(token.kind, TokenKind::FormatString(_)));
    Ok(ParseOutput {
        module: Module {
            statements: state.statements,
            procedures: state.procedures,
            format_strings,
        },
        actions: state.sink.actions,
    })
}

/// The prefix of the function an augmented assignment calls: `x += e`
/// becomes `x = $AUG+(e)`. No source name can spell it.
pub const AUGMENTED: &str = "$AUG";

/// QuickrBASIC's augmented assignment, `target op= value`, rewritten into
/// an ordinary assignment the grammar parses anywhere a statement goes. An
/// operator written directly against `=` cannot occur in QuickBASIC, so the
/// rewrite changes no valid program.
fn augmented(tokens: Vec<Token>) -> Vec<Token> {
    let is = |token: &Token, name: &str| matches!(token.kind, TokenKind::Reserved(id) if id == named(name));
    let operators = ["tkAdd", "tkMinus", "tkMult", "tkDiv", "tkIdiv", "tkPwr", "tkMOD", "tkAND", "tkOR", "tkXOR"];
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        let operator = operators.iter().find(|name| is(token, name));
        let equals = tokens.get(at + 1).filter(|next| {
            is(next, "tkEQ") && next.span.line == token.span.line && next.span.start == token.span.end
        });
        let (Some(operator), Some(equals)) = (operator, equals) else {
            out.push(token.clone());
            at += 1;
            continue;
        };
        let end = token_statement_end(&tokens, at + 2);
        let at_token = |kind: TokenKind| Token { kind, span: token.span };
        out.push(equals.clone());
        out.push(at_token(TokenKind::Identifier(format!("{AUGMENTED}{}", operator))));
        out.push(at_token(TokenKind::Reserved(named("tkLParen"))));
        out.extend(tokens[at + 2..end].iter().cloned());
        out.push(at_token(TokenKind::Reserved(named("tkRParen"))));
        at = end;
    }
    out
}

/// QuickrBASIC's `RETURN value` in a FUNCTION, rewritten into QB's own way
/// to return: `name = value: EXIT FUNCTION`. A bare RETURN still ends a
/// GOSUB, and QB's `RETURN label` has no place left in a FUNCTION.
fn returned(tokens: Vec<Token>, dialect: Dialect) -> Vec<Token> {
    if !dialect.return_values() {
        return tokens;
    }
    let is = |token: Option<&Token>, name: &str| {
        token.is_some_and(|token| matches!(token.kind, TokenKind::Reserved(id) if id == named(name)))
    };
    let mut function: Option<Token> = None;
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        let previous = at.checked_sub(1).and_then(|before| tokens.get(before));
        if is(Some(token), "tkFUNCTION") {
            match tokens.get(at + 1) {
                _ if is(previous, "tkEND") => function = None,
                Some(name @ Token { kind: TokenKind::Identifier(_), .. })
                    if !is(previous, "tkDECLARE") && !is(previous, "tkEXIT") =>
                {
                    function = Some(name.clone())
                }
                _ => {}
            }
        }
        let value_end = || token_statement_end(&tokens, at + 1);
        match &function {
            Some(name) if is(Some(token), "tkRETURN") && value_end() > at + 1 => {
                let end = value_end();
                let reserved = |name: &str| Token {
                    kind: TokenKind::Reserved(named(name)),
                    span: token.span,
                };
                out.push(Token { span: token.span, ..name.clone() });
                out.push(reserved("tkEQ"));
                let value = &tokens[at + 1..end];
                if depth_zero_comma(value).is_some() {
                    out.push(Token {
                        kind: TokenKind::Identifier(TUPLE.into()),
                        span: token.span,
                    });
                    out.push(reserved("tkLParen"));
                    out.extend(value.iter().cloned());
                    out.push(reserved("tkRParen"));
                } else {
                    out.extend(value.iter().cloned());
                }
                out.push(reserved("tkColon"));
                out.push(reserved("tkEXIT"));
                out.push(reserved("tkFUNCTION"));
                at = end;
            }
            _ => {
                out.push(token.clone());
                at += 1;
            }
        }
    }
    out
}

/// The function a tuple calls, on either side of `=`: `a, b = b, a`
/// becomes `$TUPLE(a, b) = $TUPLE(b, a)`. No source name can spell it.
pub const TUPLE: &str = "$TUPLE";

/// Where the first comma outside parentheses stands in `tokens`.
fn depth_zero_comma(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Reserved(id) if id == named("tkLParen") => depth += 1,
            TokenKind::Reserved(id) if id == named("tkRParen") => depth = depth.saturating_sub(1),
            TokenKind::Reserved(id) if id == named("tkComma") && depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

/// QuickrBASIC's tuple assignment, `a, b = x, y`, rewritten into one the
/// grammar parses: `$TUPLE(a, b) = $TUPLE(x, y)`. A statement that starts
/// with a name and has a comma before its `=` is one.
fn tuple_assignment(tokens: Vec<Token>, dialect: Dialect) -> Vec<Token> {
    if !dialect.tuples() {
        return tokens;
    }
    let is = |token: Option<&Token>, name: &str| {
        token.is_some_and(|token| matches!(token.kind, TokenKind::Reserved(id) if id == named(name)))
    };
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let token = &tokens[at];
        let starts = out.last().is_none_or(|last: &Token| {
            ["tkNewLine", "tkColon", "tkTHEN", "tkELSE"].iter().any(|name| is(Some(last), name))
        });
        let end = if starts && matches!(token.kind, TokenKind::Identifier(_)) {
            token_statement_end(&tokens, at)
        } else {
            at
        };
        let statement = &tokens[at..end];
        let equals = statement.iter().position(|one| is(Some(one), "tkEQ"));
        let (Some(equals), Some(comma)) = (equals, depth_zero_comma(statement)) else {
            out.push(token.clone());
            at += 1;
            continue;
        };
        if comma > equals {
            out.push(token.clone());
            at += 1;
            continue;
        }
        let wrapped = |out: &mut Vec<Token>, part: &[Token]| {
            let span = part[0].span;
            out.push(Token { kind: TokenKind::Identifier(TUPLE.into()), span });
            out.push(Token { kind: TokenKind::Reserved(named("tkLParen")), span });
            out.extend(part.iter().cloned());
            out.push(Token { kind: TokenKind::Reserved(named("tkRParen")), span });
        };
        wrapped(&mut out, &statement[..equals]);
        out.push(statement[equals].clone());
        let value = &statement[equals + 1..];
        if value.is_empty() || depth_zero_comma(value).is_none() {
            out.extend(value.iter().cloned());
        } else {
            wrapped(&mut out, value);
        }
        at = end;
    }
    out
}

/// Where the statement running from `at` ends: `:`, a new line, or the ELSE
/// of a one-line IF, outside any parentheses. An ELSE answering an IF inside
/// the statement belongs to a conditional expression.
fn token_statement_end(tokens: &[Token], mut at: usize) -> usize {
    let is = |token: &Token, name: &str| matches!(token.kind, TokenKind::Reserved(id) if id == named(name));
    let mut depth = 0usize;
    let mut conditionals = 0usize;
    while let Some(next) = tokens.get(at) {
        if depth == 0 && is(next, "tkELSE") && conditionals > 0 {
            conditionals -= 1;
            at += 1;
            continue;
        }
        if depth == 0 && (is(next, "tkColon") || is(next, "tkNewLine") || is(next, "tkELSE")) {
            break;
        }
        if is(next, "tkIF") {
            conditionals += 1;
        } else if is(next, "tkLParen") {
            depth += 1;
        } else if is(next, "tkRParen") {
            depth = depth.saturating_sub(1);
        }
        at += 1;
    }
    at
}

/// The function a FOR EACH loop starts from, and the prefix of the name its
/// `AS` declares: `FOR EACH x AS t IN e` becomes
/// `DIM $EACHx AS t: FOR x = $EACH(e) TO 0`. No source name can spell it.
pub const EACH: &str = "$EACH";

/// QuickrBASIC's `FOR EACH x [AS type] IN iterable`, rewritten into a FOR
/// the grammar parses and pairs with its NEXT. `FOR EACH =` stays a counter
/// named EACH.
fn for_each(tokens: Vec<Token>) -> Vec<Token> {
    let is = |token: Option<&Token>, name: &str| {
        token.is_some_and(|token| matches!(token.kind, TokenKind::Reserved(id) if id == named(name)))
    };
    let word = |token: Option<&Token>, text: &str| {
        token.is_some_and(|token| matches!(&token.kind, TokenKind::Identifier(word) if word == text))
    };
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let variable = match tokens.get(at + 2).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name))
                if is(tokens.get(at), "tkFOR") && word(tokens.get(at + 1), "EACH") =>
            {
                name.clone()
            }
            _ => {
                out.push(tokens[at].clone());
                at += 1;
                continue;
            }
        };
        let end = token_statement_end(&tokens, at);
        let Some(within) = (at + 3..end).find(|&index| word(tokens.get(index), "IN")) else {
            out.push(tokens[at].clone());
            at += 1;
            continue;
        };
        let span = tokens[at].span;
        let token = |kind: TokenKind| Token { kind, span };
        let reserved = |name: &str| token(TokenKind::Reserved(named(name)));
        if is(tokens.get(at + 3), "tkAS") {
            out.push(reserved("tkDIM"));
            out.push(token(TokenKind::Identifier(format!("{EACH}{variable}"))));
            out.extend(tokens[at + 3..within].iter().cloned());
            out.push(reserved("tkColon"));
        } else if within != at + 3 {
            out.push(tokens[at].clone());
            at += 1;
            continue;
        }
        out.push(tokens[at].clone());
        out.push(tokens[at + 2].clone());
        out.push(reserved("tkEQ"));
        out.push(token(TokenKind::Identifier(EACH.into())));
        out.push(reserved("tkLParen"));
        out.extend(tokens[within + 1..end].iter().cloned());
        out.push(reserved("tkRParen"));
        out.push(reserved("tkTO"));
        out.push(token(TokenKind::Integer(0, None)));
        at = end;
    }
    out
}

/// `PRIVATE SUB` and `PRIVATE FUNCTION`: the grammar parses an ordinary
/// header, so PRIVATE leaves the stream and where its SUB or FUNCTION now
/// stands marks the procedure that statement creates.
fn without_private(tokens: Vec<Token>) -> (Vec<Token>, BTreeSet<usize>) {
    let separator = |token: &Token| {
        matches!(token.kind, TokenKind::Reserved(id) if id == named("tkNewLine") || id == named("tkColon"))
    };
    let header = |token: Option<&Token>| {
        token.is_some_and(|token| {
            matches!(token.kind, TokenKind::Reserved(id) if id == named("tkSUB") || id == named("tkFUNCTION"))
        })
    };
    let mut kept = Vec::with_capacity(tokens.len());
    let mut marked = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        let starts = kept.last().is_none_or(separator);
        if starts
            && matches!(&token.kind, TokenKind::Identifier(word) if word == "PRIVATE")
            && header(tokens.get(index + 1))
        {
            marked.insert(kept.len());
            continue;
        }
        kept.push(token.clone());
    }
    (kept, marked)
}

fn statement(engine: &ParserEngine, state: &mut ParseState) -> ParseResult {
    if let Some(Token {
        kind: TokenKind::Integer(value, None),
        span,
    }) = state.token().cloned()
    {
        if !(0..=65_529).contains(&value) {
            return ParseResult::BadSyntax;
        }
        state.at += 1;
        state
            .statements
            .push(Statement::Label(value.to_string(), span));
        if !at_named(state, "tkNewLine") && !at_named(state, "tkColon") {
            state.tokens.insert(
                state.at,
                Token {
                    kind: TokenKind::Reserved(named("tkColon")),
                    span,
                },
            );
        }
        return ParseResult::GoodSyntax;
    }
    if matches!(
        state.token().map(|token| &token.kind),
        Some(TokenKind::ArrayDynamic)
    ) {
        let span = state.token().expect("matched token").span;
        state.at += 1;
        state.dynamic_arrays = true;
        state.statements.push(Statement::Comment(span));
        return ParseResult::GoodSyntax;
    }
    if matches!(
        state.token().map(|token| &token.kind),
        Some(TokenKind::ArrayStatic)
    ) {
        let span = state.token().expect("matched token").span;
        state.at += 1;
        state.dynamic_arrays = false;
        state.statements.push(Statement::Comment(span));
        return ParseResult::GoodSyntax;
    }
    if matches!(
        state.token().map(|token| &token.kind),
        Some(TokenKind::Identifier(_))
    ) {
        return identifier_statement(state);
    }
    let Some(key) = state.token_id() else {
        return ParseResult::NotFound;
    };
    let keyword_span = state.token().expect("token id came from a token").span;
    if key == named("tkDATA") {
        return data_statement(state, keyword_span);
    }
    if key == named("tkREM") || key == named("tkSQuote") {
        state.at += 1;
        state.statements.push(Statement::Comment(keyword_span));
        return ParseResult::GoodSyntax;
    }
    let mut matches = tables::T_STMT_DISPATCH
        .iter()
        .filter(|(irw, _)| *irw == key)
        .map(|(_, offset)| *offset)
        .peekable();
    if matches.peek().is_none() {
        return ParseResult::NotFound;
    }
    state.at += 1;
    state.declaration_shared = false;
    state.declaration_form = None;
    for offset in matches {
        let statement_base = state.statements.len();
        let checkpoint = state.checkpoint();
        let expression_base = state.expressions.len();
        let declaration_base = state.declarations.len();
        let label_base = state.labels.len();
        let procedure_reference_base = state.procedure_references.len();
        let type_name_base = state.type_names.len();
        let literal_base = state.literal_values.len();
        let parameter_base = state.parameters.len();
        let procedure_header_base = state.procedure_headers.len();
        let action_base = state.sink.actions.len();
        let result = engine.parse(state, usize::from(offset));
        if result == ParseResult::GoodSyntax {
            if state.statements.len() == statement_base {
                if !synthesize_statement(
                    state,
                    key,
                    keyword_span,
                    expression_base,
                    declaration_base,
                    label_base,
                    procedure_reference_base,
                    type_name_base,
                    literal_base,
                    parameter_base,
                    procedure_header_base,
                    action_base,
                ) {
                    return ParseResult::BadSyntax;
                }
            }
            return ParseResult::GoodSyntax;
        }
        state.rollback(checkpoint);
    }
    ParseResult::BadSyntax
}

fn identifier_statement(state: &mut ParseState) -> ParseResult {
    let checkpoint = state.checkpoint();
    if assignment(state) == ParseResult::GoodSyntax {
        return ParseResult::GoodSyntax;
    }
    state.rollback(checkpoint);

    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let TokenKind::Identifier(name) = token.kind else {
        return ParseResult::NotFound;
    };
    let line_start =
        state.at == 0 || state.tokens[state.at - 1].kind == TokenKind::Reserved(named("tkNewLine"));
    state.at += 1;
    // Only a line's first name is a label; after that `name:` is a call.
    if line_start && at_named(state, "tkColon") {
        state.statements.push(Statement::Label(name, token.span));
        return ParseResult::GoodSyntax;
    }

    let mut arguments = Vec::new();
    while !at_named(state, "tkNewLine") && !at_named(state, "tkColon") {
        let argument = match expression(state, 0) {
            Ok(argument) => argument,
            Err(result) => return result,
        };
        arguments.push(argument);
        if !consume_named(state, "tkComma") {
            break;
        }
    }
    state.statements.push(Statement::Call {
        name,
        arguments,
        explicit: false,
        span: token.span,
    });
    ParseResult::GoodSyntax
}

pub(crate) fn external_action(
    action: ExternalAction,
    engine: &ParserEngine,
    state: &mut ParseState,
) -> ParseResult {
    let checkpoint = state.checkpoint();
    let result = match action {
        ExternalAction::Assignment => assignment(state),
        ExternalAction::CommaNoEos => {
            if consume_named(state, "tkComma") {
                ParseResult::GoodSyntax
            } else {
                ParseResult::NotFound
            }
        }
        ExternalAction::LineBox => keyword_value(state, "B"),
        ExternalAction::LineFill => keyword_value(state, "F"),
        ExternalAction::LineBoxFill => keyword_value(state, "BF"),
        ExternalAction::Expression => match expression(state, 0) {
            Ok(value) => {
                state.expressions.push(value);
                ParseResult::GoodSyntax
            }
            Err(ParseResult::NotFound) => ParseResult::NotFound,
            Err(other) => other,
        },
        ExternalAction::LiteralString => literal_string(state),
        ExternalAction::IfStatement => if_statement(engine, state),
        ExternalAction::SharedDeclaration => {
            state.declaration_shared = true;
            ParseResult::GoodSyntax
        }
        ExternalAction::StaticDeclaration => {
            state.declaration_form = Some(DeclarationForm::Static);
            ParseResult::GoodSyntax
        }
        ExternalAction::StaticVariableDeclaration => {
            declaration(state, DeclarationForm::Static, false)
        }
        ExternalAction::SharedVariableDeclaration => {
            declaration(state, DeclarationForm::Shared, false)
        }
        ExternalAction::DimDeclaration => declaration(state, DeclarationForm::Dim, false),
        ExternalAction::RedimDeclaration => declaration(state, DeclarationForm::Redim, true),
        ExternalAction::EndPrint => end_print(state, false),
        ExternalAction::EndPrintExpression => end_print(state, true),
        ExternalAction::LabelOrLine => label_or_line(state),
        ExternalAction::LiteralZero => literal_value(state, 0),
        ExternalAction::LiteralOne => literal_value(state, 1),
        ExternalAction::RequireFirstStatement => err_if_not_first(state),
        ExternalAction::FunctionDeclarationName => {
            procedure_name(state, ProcedureKind::Function, true, false)
        }
        ExternalAction::FunctionDefinitionName => {
            procedure_name(state, ProcedureKind::Function, false, false)
        }
        ExternalAction::SubDeclarationName => {
            procedure_name(state, ProcedureKind::Sub, true, false)
        }
        ExternalAction::SubDefinitionName => {
            procedure_name(state, ProcedureKind::Sub, false, false)
        }
        ExternalAction::DefFnName => procedure_name(state, ProcedureKind::Function, false, true),
        ExternalAction::Parameter => parameter(state),
        ExternalAction::AssignableReference
        | ExternalAction::ArrayReference
        | ExternalAction::ForCounter => match assignable(state) {
            Ok(value) => {
                state.expressions.push(value);
                ParseResult::GoodSyntax
            }
            Err(result) => result,
        },
        ExternalAction::ProcedureReference => procedure_reference(state),
        ExternalAction::CallArgument => call_argument(state),
        ExternalAction::ArgumentList => argument_list(state),
        ExternalAction::ConstAssignment => const_assignment(state),
        ExternalAction::DefTypeInteger => def_type_list(state, TypeName::Integer),
        ExternalAction::DefTypeLong => def_type_list(state, TypeName::Long),
        ExternalAction::DefTypeSingle => def_type_list(state, TypeName::Single),
        ExternalAction::DefTypeDouble => def_type_list(state, TypeName::Double),
        ExternalAction::DefTypeString => def_type_list(state, TypeName::String),
        ExternalAction::TypeNameReference => type_name_reference(state),
    };
    if result == ParseResult::NotFound {
        state.rollback(checkpoint);
    }
    result
}

fn keyword_value(state: &mut ParseState, expected: &str) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let TokenKind::Identifier(name) = &token.kind else {
        return ParseResult::NotFound;
    };
    if name != expected {
        return ParseResult::NotFound;
    }
    state.at += 1;
    state.expressions.push(Expr::Name(name.clone(), token.span));
    ParseResult::GoodSyntax
}

fn literal_string(state: &mut ParseState) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let expression = match token.kind {
        TokenKind::String(value) => Expr::Literal(Literal::String(value), token.span),
        TokenKind::FormatString(segments) => match format_string(&segments, token.span, state.python_expressions) {
            Ok(expression) => expression,
            Err(result) => return result,
        },
        _ => return ParseResult::NotFound,
    };
    state.at += 1;
    state.expressions.push(expression);
    ParseResult::GoodSyntax
}

/// The name of the conversion an f-string field makes. No source name can
/// spell it, so no program can call or shadow it.
pub const FORMAT_FIELD: &str = "$FSTRING";

/// `f"…"` as the concatenation of its text and its converted fields, a
/// field's spec passed to the conversion as a string.
fn format_string(
    segments: &[FormatSegment],
    span: Span,
    python_expressions: bool,
) -> Result<Expr, ParseResult> {
    let mut parts = Vec::new();
    for segment in segments {
        parts.push(match segment {
            FormatSegment::Text(text) => Expr::Literal(Literal::String(text.clone()), span),
            FormatSegment::Field { tokens, spec, span } => {
                let mut field = ParseState::new(tokens.clone());
                field.python_expressions = python_expressions;
                let value = expression(&mut field, 0)?;
                if field.at != tokens.len() {
                    return Err(ParseResult::BadSyntax);
                }
                let mut arguments = vec![value];
                arguments.extend(spec.iter().map(|spec| Expr::Literal(Literal::String(spec.clone()), *span)));
                Expr::Apply {
                    name: FORMAT_FIELD.into(),
                    arguments,
                    span: *span,
                }
            }
        });
    }
    let mut parts = parts.into_iter();
    let first = parts.next().unwrap_or(Expr::Literal(Literal::String(String::new()), span));
    Ok(parts.fold(first, |left, right| Expr::Binary {
        op: Binary::Add,
        left: Box::new(left),
        right: Box::new(right),
        span,
    }))
}

fn declaration(state: &mut ParseState, form: DeclarationForm, require_array: bool) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let Some(name) = contextual_name(&token.kind) else {
        return ParseResult::NotFound;
    };
    state.at += 1;

    let mut bounds = Vec::new();
    let array = if consume_named(state, "tkLParen") {
        if !consume_named(state, "tkRParen") {
            loop {
                let first = match expression(state, 0) {
                    Ok(value) => value,
                    Err(_) => return ParseResult::BadSyntax,
                };
                let (lower, upper) = if consume_named(state, "tkTO") {
                    let upper = match expression(state, 0) {
                        Ok(value) => value,
                        Err(_) => return ParseResult::BadSyntax,
                    };
                    (Some(first), upper)
                } else {
                    (None, first)
                };
                bounds.push(Bound { lower, upper });
                if !consume_named(state, "tkComma") {
                    if !consume_named(state, "tkRParen") {
                        return ParseResult::BadSyntax;
                    }
                    break;
                }
            }
        }
        true
    } else {
        false
    };
    if require_array && !array {
        return ParseResult::BadSyntax;
    }

    let mut fixed_length = None;
    let type_name = if consume_named(state, "tkAS") {
        let Some(type_name) = declaration_type(state) else {
            return ParseResult::BadSyntax;
        };
        if type_name == TypeName::String && consume_named(state, "tkMult") {
            fixed_length = match expression(state, 0) {
                Ok(value) => Some(value),
                Err(_) => return ParseResult::BadSyntax,
            };
        }
        Some(type_name)
    } else {
        suffix_type(&name)
    };

    state.declaration_form = Some(form);
    state.declarations.push(Declaration {
        name,
        type_name,
        array,
        bounds,
        fixed_length,
        shared: state.declaration_shared,
        dynamic: state.dynamic_arrays,
        span: Span {
            line: token.span.line,
            start: token.span.start,
            end: previous_end(state),
        },
    });
    ParseResult::GoodSyntax
}

fn declaration_type(state: &mut ParseState) -> Option<TypeName> {
    if let Some(integral) = signed_type(state) {
        return Some(integral);
    }
    if state.python_expressions && consume_named(state, "tkLParen") {
        let mut types = vec![declaration_type(state)?];
        while consume_named(state, "tkComma") {
            types.push(declaration_type(state)?);
        }
        return (consume_named(state, "tkRParen") && types.len() > 1).then_some(TypeName::Tuple(types));
    }
    let token = state.token()?.clone();
    let type_name = match token.kind {
        TokenKind::Reserved(id) if id == named("tkINTEGER") => TypeName::Integer,
        TokenKind::Reserved(id) if id == named("tkLONG") => TypeName::Long,
        TokenKind::Reserved(id) if id == named("tkSINGLE") => TypeName::Single,
        TokenKind::Reserved(id) if id == named("tkDOUBLE") => TypeName::Double,
        TokenKind::Reserved(id) if id == named("tkSTRING") => TypeName::String,
        TokenKind::Reserved(id) if id == named("tkANY") => TypeName::Named("ANY".into()),
        TokenKind::Reserved(id) if contextual_reserved_name(id).is_some() => {
            TypeName::Named(contextual_reserved_name(id).expect("guard checked name"))
        }
        TokenKind::Identifier(name) => TypeName::Named(name),
        _ => return None,
    };
    state.at += 1;
    Some(type_name)
}

/// `SIGNED`/`UNSIGNED` followed by `BYTE`, `INTEGER` or `LONG`.  Both words
/// stay identifiers elsewhere, so a TYPE may still be named `SIGNED`.
fn signed_type(state: &mut ParseState) -> Option<TypeName> {
    let TokenKind::Identifier(modifier) = &state.token()?.kind else {
        return None;
    };
    let signed = match modifier.as_str() {
        "SIGNED" => true,
        "UNSIGNED" => false,
        _ => return None,
    };
    let width = match &state.tokens.get(state.at + 1)?.kind {
        TokenKind::Identifier(name) if name == "BYTE" => 1,
        TokenKind::Reserved(id) if *id == named("tkINTEGER") => 2,
        TokenKind::Reserved(id) if *id == named("tkLONG") => 4,
        _ => return None,
    };
    state.at += 2;
    Some(TypeName::Integral { width, signed })
}

fn suffix_type(name: &str) -> Option<TypeName> {
    match name.as_bytes().last() {
        Some(b'%') => Some(TypeName::Integer),
        Some(b'&') => Some(TypeName::Long),
        Some(b'!') => Some(TypeName::Single),
        Some(b'#') => Some(TypeName::Double),
        Some(b'$') => Some(TypeName::String),
        _ => None,
    }
}

fn end_print(state: &mut ParseState, has_expression: bool) -> ParseResult {
    if consume_named(state, "tkComma") || consume_named(state, "tkSColon") {
        return ParseResult::GoodSyntax;
    }
    if at_named(state, "tkNewLine")
        || at_named(state, "tkColon")
        || (has_expression && at_named(state, "tkUSING"))
    {
        return if has_expression {
            ParseResult::GoodSyntax
        } else {
            ParseResult::NotFound
        };
    }
    ParseResult::NotFound
}

fn label_or_line(state: &mut ParseState) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let label = match &token.kind {
        TokenKind::Identifier(name) => name.clone(),
        TokenKind::Integer(value, None) if (0..=65_529).contains(value) => value.to_string(),
        _ => return ParseResult::NotFound,
    };
    state.at += 1;
    state.labels.push((label, token.span));
    ParseResult::GoodSyntax
}

fn procedure_reference(state: &mut ParseState) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let TokenKind::Identifier(name) = token.kind else {
        return ParseResult::NotFound;
    };
    state.at += 1;
    state.procedure_references.push((name, token.span));
    ParseResult::GoodSyntax
}

fn call_argument(state: &mut ParseState) -> ParseResult {
    consume_named(state, "tkBYVAL");
    consume_named(state, "tkSEG");
    match expression(state, 0) {
        Ok(value) => {
            state.expressions.push(value);
            ParseResult::GoodSyntax
        }
        Err(result) => result,
    }
}

fn argument_list(state: &mut ParseState) -> ParseResult {
    let mut count = 0;
    while !at_named(state, "tkNewLine") && !at_named(state, "tkColon") {
        if count == 5 {
            return ParseResult::BadSyntax;
        }
        if at_named(state, "tkComma") {
            let span = state.token().expect("matched comma").span;
            state.at += 1;
            state.expressions.push(Expr::Omitted(span));
            count += 1;
            continue;
        }
        let value = match expression(state, 0) {
            Ok(value) => value,
            Err(result) => return result,
        };
        state.expressions.push(value);
        count += 1;
        if !consume_named(state, "tkComma") {
            break;
        }
        if at_named(state, "tkNewLine") || at_named(state, "tkColon") {
            let span = state.tokens.get(state.at.saturating_sub(1)).map_or(
                Span {
                    line: 1,
                    start: 0,
                    end: 0,
                },
                |token| token.span,
            );
            state.expressions.push(Expr::Omitted(span));
            break;
        }
    }
    ParseResult::GoodSyntax
}

fn type_name_reference(state: &mut ParseState) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let Some(name) = contextual_name(&token.kind) else {
        return ParseResult::NotFound;
    };
    state.at += 1;
    state.type_names.push((name, token.span));
    ParseResult::GoodSyntax
}

fn const_assignment(state: &mut ParseState) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let mut name = match token.kind {
        TokenKind::Identifier(name) => name,
        TokenKind::Reserved(id) if id == named("tkMOD") => "MOD".into(),
        _ => return ParseResult::NotFound,
    };
    state.at += 1;
    while matches!(
        state.token().map(|token| &token.kind),
        Some(TokenKind::Period)
    ) {
        state.at += 1;
        let Some(field) = state.token().cloned() else {
            return ParseResult::BadSyntax;
        };
        let TokenKind::Identifier(field) = field.kind else {
            return ParseResult::BadSyntax;
        };
        state.at += 1;
        name.push('.');
        name.push_str(&field);
    }
    if !consume_named(state, "tkEQ") {
        return ParseResult::BadSyntax;
    }
    let value = match expression(state, 0) {
        Ok(value) => value,
        Err(_) => return ParseResult::BadSyntax,
    };
    state.statements.push(Statement::Const {
        name,
        span: Span {
            line: token.span.line,
            start: token.span.start,
            end: value.span().end,
        },
        value,
    });
    ParseResult::GoodSyntax
}

fn def_type_list(state: &mut ParseState, type_name: TypeName) -> ParseResult {
    let span = state.tokens.get(state.at.saturating_sub(1)).map_or(
        Span {
            line: 1,
            start: 0,
            end: 0,
        },
        |token| token.span,
    );
    let mut ranges = Vec::new();
    loop {
        let Some(first) = default_type_letter(state) else {
            return if ranges.is_empty() {
                ParseResult::NotFound
            } else {
                ParseResult::BadSyntax
            };
        };
        let last = if consume_named(state, "tkMinus") {
            let Some(last) = default_type_letter(state) else {
                return ParseResult::BadSyntax;
            };
            last
        } else {
            first
        };
        if first > last {
            return ParseResult::BadSyntax;
        }
        ranges.push((first, last));
        if !consume_named(state, "tkComma") {
            break;
        }
    }
    state.statements.push(Statement::DefType {
        type_name,
        ranges,
        span,
    });
    ParseResult::GoodSyntax
}

fn default_type_letter(state: &mut ParseState) -> Option<char> {
    let token = state.token()?.clone();
    let TokenKind::Identifier(name) = token.kind else {
        return None;
    };
    if name.len() != 1 || !name.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }
    state.at += 1;
    Some(name.as_bytes()[0].to_ascii_uppercase() as char)
}

fn literal_value(state: &mut ParseState, wanted: i64) -> ParseResult {
    let Some(token) = state.token() else {
        return ParseResult::NotFound;
    };
    if !matches!(token.kind, TokenKind::Integer(value, _) if value == wanted) {
        return ParseResult::NotFound;
    }
    state.at += 1;
    state.literal_values.push(wanted);
    ParseResult::GoodSyntax
}

fn err_if_not_first(state: &ParseState) -> ParseResult {
    let keyword_index = state.at.saturating_sub(1);
    if keyword_index == 0
        || state
            .tokens
            .get(keyword_index.saturating_sub(1))
            .is_some_and(
                |token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkNewLine")),
            )
    {
        ParseResult::GoodSyntax
    } else {
        ParseResult::BadSyntax
    }
}

fn procedure_name(
    state: &mut ParseState,
    kind: ProcedureKind,
    declaration: bool,
    require_fn_prefix: bool,
) -> ParseResult {
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let TokenKind::Identifier(name) = token.kind else {
        return ParseResult::NotFound;
    };
    if require_fn_prefix && !name.get(..2).is_some_and(|prefix| prefix == "FN") {
        return ParseResult::NotFound;
    }
    let result = suffix_type(&name);
    if kind == ProcedureKind::Sub && result.is_some() {
        return ParseResult::BadSyntax;
    }
    let span = state
        .at
        .checked_sub(1)
        .and_then(|index| state.tokens.get(index))
        .map_or(token.span, |keyword| keyword.span);
    state.at += 1;
    state.procedure_headers.push(ProcedureHeader {
        name,
        kind,
        declaration,
        result,
        span,
    });
    ParseResult::GoodSyntax
}

fn parameter(state: &mut ParseState) -> ParseResult {
    let by_value = consume_named(state, "tkBYVAL");
    let segmented = consume_named(state, "tkSEG");
    let Some(token) = state.token().cloned() else {
        return ParseResult::NotFound;
    };
    let TokenKind::Identifier(name) = token.kind else {
        return ParseResult::NotFound;
    };
    state.at += 1;
    let array = if consume_named(state, "tkLParen") {
        if !consume_named(state, "tkRParen") {
            return ParseResult::BadSyntax;
        }
        true
    } else {
        false
    };
    let type_name = if consume_named(state, "tkAS") {
        let Some(type_name) = declaration_type(state) else {
            return ParseResult::BadSyntax;
        };
        if type_name == TypeName::String && consume_named(state, "tkMult") {
            if expression(state, 0).is_err() {
                return ParseResult::BadSyntax;
            }
        }
        Some(type_name)
    } else {
        suffix_type(&name)
    };
    state.parameters.push(Parameter {
        declaration: Declaration {
            name,
            type_name,
            array,
            bounds: Vec::new(),
            fixed_length: None,
            shared: false,
            dynamic: false,
            span: Span {
                line: token.span.line,
                start: token.span.start,
                end: previous_end(state),
            },
        },
        by_value,
        segmented,
    });
    ParseResult::GoodSyntax
}

fn extension_procedure(
    state: &mut ParseState,
    name: String,
    alias: Option<String>,
    cdecl: bool,
    kind: ProcedureKind,
    span: Span,
) -> ParseResult {
    let parameter_base = state.parameters.len();
    if consume_named(state, "tkLParen") && !consume_named(state, "tkRParen") {
        loop {
            if parameter(state) != ParseResult::GoodSyntax {
                return ParseResult::BadSyntax;
            }
            if !consume_named(state, "tkComma") {
                if !consume_named(state, "tkRParen") {
                    return ParseResult::BadSyntax;
                }
                break;
            }
        }
    }
    let result = if kind == ProcedureKind::Function && consume_named(state, "tkAS") {
        let Some(result) = declaration_type(state) else {
            return ParseResult::BadSyntax;
        };
        Some(result)
    } else {
        suffix_type(&name)
    };
    let is_static = consume_named(state, "tkSTATIC");
    let parameters = state.parameters.split_off(parameter_base);
    state.procedures.push(Procedure {
        name,
        alias,
        cdecl,
        kind,
        parameters,
        result,
        body: Vec::new(),
        declaration: true,
        is_static,
        exported: true,
        private: false,
        module_scope: false,
        span,
    });
    ParseResult::GoodSyntax
}

fn data_statement(state: &mut ParseState, span: Span) -> ParseResult {
    state.at += 1;
    let mut values = Vec::new();
    loop {
        let Ok(value) = expression(state, 0) else {
            return ParseResult::BadSyntax;
        };
        values.push(value);
        if !consume_named(state, "tkComma") {
            break;
        }
    }
    if values.is_empty() {
        return ParseResult::BadSyntax;
    }
    state.statements.push(Statement::Data { values, span });
    ParseResult::GoodSyntax
}

fn synthesize_statement(
    state: &mut ParseState,
    keyword: u16,
    span: Span,
    expression_base: usize,
    declaration_base: usize,
    label_base: usize,
    procedure_reference_base: usize,
    type_name_base: usize,
    literal_base: usize,
    parameter_base: usize,
    procedure_header_base: usize,
    action_base: usize,
) -> bool {
    if state.declarations.len() > declaration_base {
        let declarations = state.declarations.split_off(declaration_base);
        let statement = match state.declaration_form {
            Some(DeclarationForm::Dim) => Statement::Dim(declarations),
            Some(DeclarationForm::Redim) => Statement::Redim(declarations),
            Some(DeclarationForm::Static) => Statement::Static(declarations),
            Some(DeclarationForm::Shared) => Statement::Shared(declarations),
            None => return false,
        };
        state.expressions.truncate(expression_base);
        state.statements.push(statement);
        return true;
    }
    let actions = &state.sink.actions[action_base..];
    if state.procedure_headers.len() > procedure_header_base {
        let headers = state.procedure_headers.split_off(procedure_header_base);
        let [mut header]: [ProcedureHeader; 1] = match headers.try_into() {
            Ok(headers) => headers,
            Err(_) => return false,
        };
        let parameters = state.parameters.split_off(parameter_base);
        let def_fn = keyword == named("tkDEF");
        let definition = if def_fn {
            let values = state.expressions.split_off(expression_base);
            match values.as_slice() {
                [] => None,
                [value] => Some(value.clone()),
                _ => return false,
            }
        } else {
            None
        };
        if header.kind == ProcedureKind::Function && consume_named(state, "tkAS") {
            let Some(result) = declaration_type(state) else {
                return false;
            };
            header.result = Some(result);
        }
        let is_static = state.sink.actions[action_base..]
            .iter()
            .any(|action| matches!(action, AstAction::Mark { slot: 4, .. }))
            || consume_named(state, "tkSTATIC");
        let inline_def_fn = def_fn && definition.is_some();
        let body = definition
            .map(|value| {
                vec![Statement::Assign {
                    target: Expr::Name(header.name.clone(), header.span),
                    value,
                    span: header.span,
                }]
            })
            .unwrap_or_default();
        let procedure = Procedure {
            name: header.name,
            alias: None,
            cdecl: false,
            kind: header.kind,
            parameters,
            result: header.result,
            body,
            declaration: header.declaration,
            is_static,
            exported: !inline_def_fn,
            private: false,
            module_scope: def_fn,
            span: header.span,
        };
        state.procedures.push(procedure);
        if !header.declaration && !inline_def_fn {
            if state.open_procedure.is_some() {
                return false;
            }
            state.open_procedure = Some(state.procedures.len() - 1);
        }
        return true;
    }
    let emitted = actions.iter().find_map(AstAction::statement_shape);
    let dispatched = tables::dispatch_action(keyword).and_then(|action| action.statement_shape());
    let line_input = keyword == named("tkLINE")
        && state.tokens[..state.at].iter().any(|token| {
            token.span.line == span.line
                && token.span.start >= span.end
                && matches!(token.kind, TokenKind::Reserved(id) if id == named("tkINPUT"))
        });
    let Some(descriptor) = (if line_input {
        Some(StatementShape::LineInput)
    } else {
        dispatched.or(emitted)
    }) else {
        return false;
    };
    let arguments = state.expressions.split_off(expression_base);
    let labels = state.labels.split_off(label_base);
    let procedure_references = state
        .procedure_references
        .split_off(procedure_reference_base);
    let type_names = state.type_names.split_off(type_name_base);
    let literal_values = state.literal_values.split_off(literal_base);
    let statement = match descriptor {
        StatementShape::Read => {
            if arguments.is_empty() {
                return false;
            }
            Statement::Read {
                destinations: arguments,
                span,
            }
        }
        StatementShape::LineInput => {
            let channel = actions.contains(&AstAction::LineInputChannel);
            let has_prompt = actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 4, .. }));
            let mut arguments = arguments.into_iter();
            let file = channel.then(|| arguments.next()).flatten();
            let prompt = has_prompt.then(|| arguments.next()).flatten();
            let Some(destination) = arguments.next() else {
                return false;
            };
            if arguments.next().is_some()
                || (channel && file.is_none())
                || (has_prompt && prompt.is_none())
            {
                return false;
            }
            Statement::LineInput {
                file,
                prompt,
                destination,
                span,
            }
        }
        StatementShape::Input => {
            let mut values = arguments.into_iter();
            let file = actions
                .contains(&AstAction::LineInputChannel)
                .then(|| values.next())
                .flatten();
            if actions.contains(&AstAction::LineInputChannel) && file.is_none() {
                return false;
            }
            let has_prompt = actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 4, .. }));
            let prompt = has_prompt.then(|| values.next()).flatten();
            if has_prompt && prompt.is_none() {
                return false;
            }
            let destinations = values.collect::<Vec<_>>();
            if destinations.is_empty() {
                return false;
            }
            Statement::Input {
                file,
                prompt,
                suppress_question_mark: actions
                    .iter()
                    .any(|action| matches!(action, AstAction::Mark { slot: 1, .. })),
                keep_cursor: actions
                    .iter()
                    .any(|action| matches!(action, AstAction::Mark { slot: 2, .. })),
                destinations,
                span,
            }
        }
        StatementShape::Print => {
            let mut values = arguments.into_iter();
            let file = actions
                .contains(&AstAction::PrintChannel)
                .then(|| values.next())
                .flatten();
            if actions.contains(&AstAction::PrintChannel) && file.is_none() {
                return false;
            }
            let mut values = values.collect::<Vec<_>>();
            let using = if actions.contains(&AstAction::PrintUsing) {
                let Some(using_span) = state.tokens[..state.at]
                    .iter()
                    .rfind(|token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkUSING")))
                    .map(|token| token.span)
                else {
                    return false;
                };
                let Some(index) = values
                    .iter()
                    .position(|value| value.span().start >= using_span.end)
                else {
                    return false;
                };
                Some(values.remove(index))
            } else {
                None
            };
            let item_actions = actions
                .iter()
                .filter_map(|action| match *action {
                    AstAction::PrintItemComma => Some((None, PrintSeparator::Comma)),
                    AstAction::PrintItemSemicolon => Some((None, PrintSeparator::Semicolon)),
                    AstAction::PrintTab => Some((Some("TAB"), PrintSeparator::Semicolon)),
                    AstAction::PrintSpace => Some((Some("SPC"), PrintSeparator::Semicolon)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if item_actions.len() > values.len()
                || (!values.is_empty() && item_actions.len() + 1 < values.len())
            {
                return false;
            }
            let items = values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    let span = value.span();
                    let (control, separator) = item_actions
                        .get(index)
                        .copied()
                        .unwrap_or((None, PrintSeparator::End));
                    let value = control.map_or(value.clone(), |name| Expr::Apply {
                        name: name.into(),
                        arguments: vec![value],
                        span,
                    });
                    PrintItem { value, separator }
                })
                .collect();
            Statement::Print {
                file,
                using,
                items,
                span,
            }
        }
        StatementShape::Goto => {
            let [(label, _)]: [(String, Span); 1] = match labels.try_into() {
                Ok(labels) => labels,
                Err(_) => return false,
            };
            if !arguments.is_empty() || !literal_values.is_empty() {
                return false;
            }
            Statement::Goto(label, span)
        }
        StatementShape::OnError => {
            let label = match (labels.as_slice(), literal_values.as_slice()) {
                ([(label, _)], []) => label.clone(),
                ([], [0]) => "0".into(),
                _ => return false,
            };
            if !arguments.is_empty() {
                return false;
            }
            Statement::OnError {
                label,
                local: false,
                span,
            }
        }
        StatementShape::EndProcedure => {
            if !arguments.is_empty() || !labels.is_empty() || !literal_values.is_empty() {
                return false;
            }
            let Some(index) = state.open_procedure else {
                return false;
            };
            let ended_kind = if state.tokens.get(state.at.saturating_sub(1)).is_some_and(
                |token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkFUNCTION")),
            ) {
                ProcedureKind::Function
            } else {
                ProcedureKind::Sub
            };
            if state.procedures[index].kind != ended_kind {
                return false;
            }
            state.open_procedure = None;
            return true;
        }
        StatementShape::ExitProcedure | StatementShape::ExitDo | StatementShape::ExitFor => {
            if !arguments.is_empty() {
                return false;
            }
            let target = match descriptor {
                StatementShape::ExitProcedure => crate::syntax::ExitTarget::Sub,
                StatementShape::ExitDo => crate::syntax::ExitTarget::Do,
                StatementShape::ExitFor => crate::syntax::ExitTarget::For,
                _ => unreachable!("matched exit statement shape"),
            };
            let target = if target == crate::syntax::ExitTarget::Sub
                && state.tokens.get(state.at.saturating_sub(1)).is_some_and(
                    |token| {
                        matches!(token.kind, TokenKind::Reserved(id) if id == named("tkFUNCTION"))
                    },
                )
            {
                crate::syntax::ExitTarget::Function
            } else {
                target
            };
            Statement::Exit(target, span)
        }
        StatementShape::Runtime(name) => {
            let mut arguments = arguments;
            if name == "CIRCLE" {
                arguments.extend(actions.iter().filter_map(|action| {
                    match action {
                        AstAction::Mark {
                            slot: u8::MAX,
                            token,
                        } => state
                            .tokens
                            .get(*token)
                            .map(|token| Expr::Omitted(token.span)),
                        _ => None,
                    }
                }));
                arguments.sort_by_key(|argument| {
                    let span = argument.span();
                    (span.line, span.start)
                });
                if actions
                    .iter()
                    .any(|action| matches!(action, AstAction::Unsupported("opCircleAspect")))
                    && arguments.len() == 6
                {
                    let span = arguments[5].span();
                    arguments.insert(5, Expr::Omitted(span));
                }
            }
            if name == "PUT" {
                if let Some((mode, mode_span)) =
                    state.tokens[..state.at]
                        .iter()
                        .rev()
                        .find_map(|token| match token.kind {
                            TokenKind::Reserved(id) if id == named("tkAND") => {
                                Some(("AND", token.span))
                            }
                            TokenKind::Reserved(id) if id == named("tkOR") => {
                                Some(("OR", token.span))
                            }
                            TokenKind::Reserved(id) if id == named("tkPRESET") => {
                                Some(("PRESET", token.span))
                            }
                            TokenKind::Reserved(id) if id == named("tkPSET") => {
                                Some(("PSET", token.span))
                            }
                            TokenKind::Reserved(id) if id == named("tkXOR") => {
                                Some(("XOR", token.span))
                            }
                            _ => None,
                        })
                {
                    arguments.push(Expr::Name(mode.into(), mode_span));
                }
            }
            Statement::Runtime {
                name: name.into(),
                arguments,
                span,
            }
        }
        StatementShape::LegacyImplicitCall(name) => {
            let arguments = if name == "GOSUB" {
                let [(label, label_span)]: [(String, Span); 1] = match labels.try_into() {
                    Ok(labels) => labels,
                    Err(_) => return false,
                };
                vec![Expr::Name(label, label_span)]
            } else {
                if !labels.is_empty() {
                    return false;
                }
                arguments
            };
            Statement::Call {
                name: name.into(),
                arguments,
                explicit: false,
                span,
            }
        }
        StatementShape::LegacyImplicitCallWithLabels(name) => {
            let mut label_arguments = labels
                .into_iter()
                .map(|(label, label_span)| Expr::Name(label, label_span))
                .collect::<Vec<_>>();
            if !arguments.is_empty() {
                return false;
            }
            Statement::Call {
                name: name.into(),
                arguments: {
                    label_arguments.shrink_to_fit();
                    label_arguments
                },
                explicit: false,
                span,
            }
        }
        StatementShape::RuntimeWithLabels(name) => {
            if !arguments.is_empty() {
                return false;
            }
            Statement::Runtime {
                name: name.into(),
                arguments: labels
                    .into_iter()
                    .map(|(label, label_span)| Expr::Name(label, label_span))
                    .collect(),
                span,
            }
        }
        StatementShape::Call => {
            let [(name, _)]: [(String, Span); 1] = match procedure_references.try_into() {
                Ok(names) => names,
                Err(_) => return false,
            };
            Statement::Call {
                name,
                arguments,
                explicit: true,
                span,
            }
        }
        StatementShape::OptionBaseZero | StatementShape::OptionBaseOne => {
            if !arguments.is_empty() || !labels.is_empty() {
                return false;
            }
            Statement::OptionBase(
                i64::from(matches!(descriptor, StatementShape::OptionBaseOne)),
                span,
            )
        }
        StatementShape::DefSeg => {
            let value = match arguments.as_slice() {
                [] => None,
                [value] => Some(value.clone()),
                _ => return false,
            };
            Statement::DefSeg { value, span }
        }
        StatementShape::Erase => {
            if arguments.is_empty() {
                return false;
            }
            Statement::Erase(arguments)
        }
        StatementShape::Open => {
            let mode = if actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 1, .. }))
            {
                crate::syntax::FileMode::Append
            } else if actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 2, .. }))
            {
                crate::syntax::FileMode::Input
            } else if actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 3, .. }))
            {
                crate::syntax::FileMode::Output
            } else if actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 5, .. }))
            {
                crate::syntax::FileMode::Binary
            } else {
                return false;
            };
            let [path, file]: [Expr; 2] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            Statement::Open {
                path,
                mode,
                file,
                span,
            }
        }
        StatementShape::Close => Statement::Close {
            files: arguments,
            span,
        },
        StatementShape::Resume => {
            let target = if actions
                .iter()
                .any(|action| matches!(action, AstAction::Mark { slot: 4, .. }))
            {
                crate::syntax::ResumeTarget::Next
            } else if let [(label, _)] = labels.as_slice() {
                crate::syntax::ResumeTarget::Label(label.clone())
            } else if literal_values.as_slice() == [0] {
                crate::syntax::ResumeTarget::Label("0".into())
            } else if labels.is_empty() && literal_values.is_empty() {
                crate::syntax::ResumeTarget::Current
            } else {
                return false;
            };
            Statement::Resume { target, span }
        }
        StatementShape::TypeDeclaration => {
            let [(name, _)]: [(String, Span); 1] = match type_names.try_into() {
                Ok(names) => names,
                Err(_) => return false,
            };
            let Some(fields) = type_declaration_fields(state) else {
                return false;
            };
            Statement::TypeDecl { name, fields, span }
        }
        StatementShape::For | StatementShape::ForStep => {
            let has_step = matches!(descriptor, StatementShape::ForStep);
            let expected = if has_step { 4 } else { 3 };
            if arguments.len() != expected {
                return false;
            }
            let mut arguments = arguments.into_iter();
            let counter = arguments.next().expect("checked FOR counter");
            let start = arguments.next().expect("checked FOR start");
            let end = arguments.next().expect("checked FOR end");
            let step = arguments.next();
            let Some(body) = block_until_next(state) else {
                return false;
            };
            Statement::For {
                counter,
                start,
                end,
                step,
                body,
                span,
            }
        }
        StatementShape::Do | StatementShape::DoWhile | StatementShape::DoUntil => {
            let pre = match descriptor {
                StatementShape::Do => {
                    if !arguments.is_empty() {
                        return false;
                    }
                    None
                }
                StatementShape::DoWhile | StatementShape::DoUntil => {
                    let [condition]: [Expr; 1] = match arguments.try_into() {
                        Ok(arguments) => arguments,
                        Err(_) => return false,
                    };
                    Some((matches!(descriptor, StatementShape::DoWhile), condition))
                }
                _ => unreachable!("matched DO shape"),
            };
            let Some((body, post)) = block_until_loop(state) else {
                return false;
            };
            if pre.is_some() && post.is_some() {
                return false;
            }
            Statement::Do {
                pre,
                post,
                body,
                span,
            }
        }
        StatementShape::While => {
            let [condition]: [Expr; 1] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            let Some(body) = block_until_wend(state) else {
                return false;
            };
            Statement::While {
                condition,
                body,
                span,
            }
        }
        StatementShape::Seek => {
            let [file, position]: [Expr; 2] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            Statement::Seek {
                file,
                position,
                span,
            }
        }
        StatementShape::FileTransferRead | StatementShape::FileTransferWrite => {
            let [file, position, target]: [Expr; 3] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            Statement::FileTransfer {
                write: matches!(descriptor, StatementShape::FileTransferWrite),
                file,
                position: Some(position),
                target,
                span,
            }
        }
        StatementShape::FileTransferReadUnpositioned
        | StatementShape::FileTransferWriteUnpositioned => {
            let [file, target]: [Expr; 2] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            Statement::FileTransfer {
                write: matches!(descriptor, StatementShape::FileTransferWriteUnpositioned),
                file,
                position: None,
                target,
                span,
            }
        }
        StatementShape::MidAssignment => {
            if !matches!(arguments.len(), 3 | 4) {
                return false;
            }
            let mut arguments = arguments;
            let value = arguments.pop().expect("checked MID$ source");
            let right_parenthesis = state.tokens[..state.at]
                .iter()
                .rfind(|token| {
                    matches!(token.kind, TokenKind::Reserved(id) if id == named("tkRParen"))
                })
                .map_or(value.span().end, |token| token.span.end);
            let target_span = Span {
                line: span.line,
                start: span.start,
                end: right_parenthesis,
            };
            Statement::Assign {
                target: Expr::Apply {
                    name: "MID$".into(),
                    arguments,
                    span: target_span,
                },
                span: Span {
                    line: span.line,
                    start: span.start,
                    end: value.span().end,
                },
                value,
            }
        }
        StatementShape::LegacySetCall(name) => {
            let [left, right]: [Expr; 2] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            let comparison_span = Span {
                line: left.span().line,
                start: left.span().start,
                end: right.span().end,
            };
            Statement::Call {
                name: name.into(),
                arguments: vec![Expr::Binary {
                    op: Binary::Eq,
                    left: Box::new(left),
                    right: Box::new(right),
                    span: comparison_span,
                }],
                explicit: false,
                span,
            }
        }
        StatementShape::Select => {
            let [selector]: [Expr; 1] = match arguments.try_into() {
                Ok(arguments) => arguments,
                Err(_) => return false,
            };
            let Some((arms, otherwise)) = select_case_block(state) else {
                return false;
            };
            Statement::Select {
                selector,
                arms,
                otherwise,
                span,
            }
        }
    };
    state.statements.push(statement);
    true
}

fn block_until_next(state: &mut ParseState) -> Option<Vec<Statement>> {
    if !consume_named(state, "tkNewLine") && !consume_named(state, "tkColon") {
        return None;
    }
    let mut body = Vec::new();
    loop {
        while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
        if consume_named(state, "tkNEXT") {
            if matches!(
                state.token().map(|token| &token.kind),
                Some(TokenKind::Identifier(_))
            ) {
                state.at += 1;
            }
            return Some(body);
        }
        let before = state.statements.len();
        if statement(&ParserEngine::new(), state) != ParseResult::GoodSyntax {
            return None;
        }
        body.extend(state.statements.split_off(before));
        if state.at < state.tokens.len()
            && !at_named(state, "tkNewLine")
            && !at_named(state, "tkColon")
        {
            return None;
        }
    }
}

fn block_until_loop(state: &mut ParseState) -> Option<(Vec<Statement>, Option<(bool, Expr)>)> {
    if !consume_named(state, "tkNewLine") {
        return None;
    }
    let mut body = Vec::new();
    loop {
        while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
        if consume_named(state, "tkLOOP") {
            let post = if consume_named(state, "tkWHILE") {
                Some((true, expression(state, 0).ok()?))
            } else if consume_named(state, "tkUNTIL") {
                Some((false, expression(state, 0).ok()?))
            } else {
                None
            };
            return Some((body, post));
        }
        let before = state.statements.len();
        if statement(&ParserEngine::new(), state) != ParseResult::GoodSyntax {
            return None;
        }
        body.extend(state.statements.split_off(before));
        if state.at < state.tokens.len()
            && !at_named(state, "tkNewLine")
            && !at_named(state, "tkColon")
        {
            return None;
        }
    }
}

fn block_until_wend(state: &mut ParseState) -> Option<Vec<Statement>> {
    if !consume_named(state, "tkNewLine") && !consume_named(state, "tkColon") {
        return None;
    }
    let mut body = Vec::new();
    loop {
        while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
        if consume_named(state, "tkWEND") {
            return Some(body);
        }
        let before = state.statements.len();
        if statement(&ParserEngine::new(), state) != ParseResult::GoodSyntax {
            return None;
        }
        body.extend(state.statements.split_off(before));
        if state.at < state.tokens.len()
            && !at_named(state, "tkNewLine")
            && !at_named(state, "tkColon")
        {
            return None;
        }
    }
}

type SelectArms = Vec<(Vec<CaseItem>, Vec<Statement>)>;

fn select_case_block(state: &mut ParseState) -> Option<(SelectArms, Vec<Statement>)> {
    if !consume_named(state, "tkNewLine") {
        return None;
    }
    let mut arms = Vec::new();
    let mut otherwise = Vec::new();
    loop {
        while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
        if at_named(state, "tkEND")
            && state.tokens.get(state.at + 1).is_some_and(
                |token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkSELECT")),
            )
        {
            state.at += 2;
            return Some((arms, otherwise));
        }
        if !consume_named(state, "tkCASE") {
            return None;
        }
        let is_else = consume_named(state, "tkELSE");
        let mut matches = Vec::new();
        if !is_else {
            loop {
                let has_is = consume_named(state, "tkIS");
                if has_is || at_case_relation(state) {
                    let relation = case_relation(state)?;
                    matches.push(CaseItem::Relation(relation, expression(state, 0).ok()?));
                } else {
                    let lower = expression(state, 0).ok()?;
                    if consume_named(state, "tkTO") {
                        matches.push(CaseItem::Range(lower, expression(state, 0).ok()?));
                    } else {
                        matches.push(CaseItem::Value(lower));
                    }
                }
                if !consume_named(state, "tkComma") {
                    break;
                }
            }
        }
        if !consume_named(state, "tkNewLine") && !consume_named(state, "tkColon") {
            return None;
        }
        let mut body = Vec::new();
        loop {
            while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
            if at_named(state, "tkCASE")
                || at_named(state, "tkEND")
                    && state.tokens.get(state.at + 1).is_some_and(|token| {
                        matches!(token.kind, TokenKind::Reserved(id) if id == named("tkSELECT"))
                    })
            {
                break;
            }
            let before = state.statements.len();
            if statement(&ParserEngine::new(), state) != ParseResult::GoodSyntax {
                return None;
            }
            body.extend(state.statements.split_off(before));
            if state.at < state.tokens.len()
                && !at_named(state, "tkNewLine")
                && !at_named(state, "tkColon")
            {
                return None;
            }
        }
        if is_else {
            otherwise = body;
        } else {
            arms.push((matches, body));
        }
    }
}

fn at_case_relation(state: &ParseState) -> bool {
    match state.token().map(|token| &token.kind) {
        Some(TokenKind::Comparison(_)) => true,
        Some(TokenKind::Reserved(id)) => {
            *id == named("tkEQ") || *id == named("tkLT") || *id == named("tkGT")
        }
        _ => false,
    }
}

fn case_relation(state: &mut ParseState) -> Option<Binary> {
    let relation = match state.token().map(|token| &token.kind)? {
        TokenKind::Comparison(op) => *op,
        TokenKind::Reserved(id) if *id == named("tkEQ") => Binary::Eq,
        TokenKind::Reserved(id) if *id == named("tkLT") => Binary::Less,
        TokenKind::Reserved(id) if *id == named("tkGT") => Binary::Greater,
        _ => return None,
    };
    state.at += 1;
    Some(relation)
}

fn assignment(state: &mut ParseState) -> ParseResult {
    let start = state.token().map(|token| token.span);
    let target = match assignable(state) {
        Ok(target) => target,
        Err(result) => return result,
    };
    if !consume_named(state, "tkEQ") {
        return ParseResult::BadSyntax;
    }
    let value = match expression(state, 0) {
        Ok(value) => value,
        Err(_) => return ParseResult::BadSyntax,
    };
    let begin = start.unwrap_or_else(|| target.span());
    state.statements.push(Statement::Assign {
        target,
        span: Span {
            line: begin.line,
            start: begin.start,
            end: value.span().end,
        },
        value,
    });
    ParseResult::GoodSyntax
}

fn type_declaration_fields(state: &mut ParseState) -> Option<Vec<Declaration>> {
    if !consume_named(state, "tkNewLine") {
        return None;
    }
    let mut fields = Vec::new();
    loop {
        while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
        if at_named(state, "tkEND")
            && state.tokens.get(state.at + 1).is_some_and(
                |token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkTYPE")),
            )
        {
            state.at += 2;
            return Some(fields);
        }
        let token = state.token().cloned()?;
        let Some(name) = contextual_name(&token.kind) else {
            return None;
        };
        state.at += 1;
        let mut bounds = Vec::new();
        let array = consume_named(state, "tkLParen");
        if array && !consume_named(state, "tkRParen") {
            loop {
                let upper = expression(state, 0).ok()?;
                bounds.push(Bound { lower: None, upper });
                if !consume_named(state, "tkComma") {
                    if !consume_named(state, "tkRParen") {
                        return None;
                    }
                    break;
                }
            }
        }
        if !consume_named(state, "tkAS") {
            return None;
        }
        let type_name = declaration_type(state)?;
        let fixed_length = if type_name == TypeName::String && consume_named(state, "tkMult") {
            Some(expression(state, 0).ok()?)
        } else {
            None
        };
        fields.push(Declaration {
            name,
            type_name: Some(type_name),
            array,
            bounds,
            fixed_length,
            shared: false,
            dynamic: false,
            span: token.span,
        });
        if !at_named(state, "tkNewLine") {
            return None;
        }
    }
}

fn if_statement(_engine: &ParserEngine, state: &mut ParseState) -> ParseResult {
    let Some(condition) = state.expressions.pop() else {
        return ParseResult::BadSyntax;
    };
    if !consume_named(state, "tkTHEN") {
        return ParseResult::BadSyntax;
    }
    let keyword_span = state.tokens[..state.at]
        .iter()
        .rfind(|token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkIF")))
        .map_or(condition.span(), |token| token.span);
    finish_if_statement(state, condition, keyword_span)
}

fn finish_if_statement(state: &mut ParseState, condition: Expr, keyword_span: Span) -> ParseResult {
    if consume_named(state, "tkNewLine") {
        return block_if_statement(state, condition, keyword_span);
    }
    let Ok(then_branch) = single_line_if_branch(state, true) else {
        return ParseResult::BadSyntax;
    };
    let mut else_branch = Vec::new();
    if consume_named(state, "tkELSE") {
        let Ok(parsed) = single_line_if_branch(state, false) else {
            return ParseResult::BadSyntax;
        };
        else_branch = parsed;
    }
    let end = else_branch
        .last()
        .or_else(|| then_branch.last())
        .map(statement_end)
        .unwrap_or(condition.span().end);
    let span = Span {
        line: condition.span().line,
        start: keyword_span.start,
        end,
    };
    state.statements.push(Statement::If {
        condition,
        then_branch,
        else_branch,
        span,
    });
    ParseResult::GoodSyntax
}

fn single_line_if_branch(
    state: &mut ParseState,
    stop_at_else: bool,
) -> Result<Vec<Statement>, ParseResult> {
    let mut branch = Vec::new();
    loop {
        let before = state.statements.len();
        if statement(&ParserEngine::new(), state) != ParseResult::GoodSyntax {
            return Err(ParseResult::BadSyntax);
        }
        let mut parsed = state.statements.split_off(before);
        if parsed.is_empty() {
            return Err(ParseResult::BadSyntax);
        }
        branch.append(&mut parsed);

        if state.at >= state.tokens.len()
            || at_named(state, "tkNewLine")
            || (stop_at_else && at_named(state, "tkELSE"))
        {
            break;
        }
        if !consume_named(state, "tkColon") {
            return Err(ParseResult::BadSyntax);
        }
        if state.at >= state.tokens.len()
            || at_named(state, "tkNewLine")
            || (stop_at_else && at_named(state, "tkELSE"))
        {
            break;
        }
    }
    Ok(branch)
}

fn block_if_statement(state: &mut ParseState, condition: Expr, span: Span) -> ParseResult {
    let mut then_branch = Vec::new();
    let mut else_branch = Vec::new();
    let mut in_else = false;
    loop {
        while consume_named(state, "tkNewLine") || consume_named(state, "tkColon") {}
        if consume_named(state, "tkELSE") {
            if in_else {
                return ParseResult::BadSyntax;
            }
            in_else = true;
            continue;
        }
        if at_named(state, "tkELSEIF") {
            if in_else {
                return ParseResult::BadSyntax;
            }
            let else_if_span = state.token().expect("matched ELSEIF").span;
            state.at += 1;
            let nested_condition = match expression(state, 0) {
                Ok(condition) => condition,
                Err(_) => return ParseResult::BadSyntax,
            };
            if !consume_named(state, "tkTHEN") {
                return ParseResult::BadSyntax;
            }
            let before = state.statements.len();
            if finish_if_statement(state, nested_condition, else_if_span) != ParseResult::GoodSyntax
            {
                return ParseResult::BadSyntax;
            }
            let mut nested = state.statements.split_off(before);
            if nested.len() != 1 {
                return ParseResult::BadSyntax;
            }
            else_branch.push(nested.remove(0));
            state.statements.push(Statement::If {
                condition,
                then_branch,
                else_branch,
                span,
            });
            return ParseResult::GoodSyntax;
        }
        if at_named(state, "tkEND")
            && state.tokens.get(state.at + 1).is_some_and(
                |token| matches!(token.kind, TokenKind::Reserved(id) if id == named("tkIF")),
            )
        {
            state.at += 2;
            break;
        }
        if state.at >= state.tokens.len() {
            return ParseResult::BadSyntax;
        }
        let before = state.statements.len();
        if statement(&ParserEngine::new(), state) != ParseResult::GoodSyntax {
            return ParseResult::BadSyntax;
        }
        let mut parsed = state.statements.split_off(before);
        if parsed.is_empty() {
            return ParseResult::BadSyntax;
        }
        if in_else {
            else_branch.append(&mut parsed);
        } else {
            then_branch.append(&mut parsed);
        }
        if state.at < state.tokens.len()
            && !at_named(state, "tkNewLine")
            && !at_named(state, "tkColon")
        {
            return ParseResult::BadSyntax;
        }
    }
    state.statements.push(Statement::If {
        condition,
        then_branch,
        else_branch,
        span,
    });
    ParseResult::GoodSyntax
}

fn assignable(state: &mut ParseState) -> Result<Expr, ParseResult> {
    let Some(token) = state.token().cloned() else {
        return Err(ParseResult::NotFound);
    };
    let Some(name) = contextual_name(&token.kind) else {
        return Err(ParseResult::NotFound);
    };
    state.at += 1;
    let mut value = Expr::Name(name, token.span);
    loop {
        if matches!(
            state.token().map(|token| &token.kind),
            Some(TokenKind::Period)
        ) {
            state.at += 1;
            let Some(field) = state.token().cloned() else {
                return Err(ParseResult::BadSyntax);
            };
            let Some(name) = contextual_name(&field.kind) else {
                return Err(ParseResult::BadSyntax);
            };
            state.at += 1;
            let span = Span {
                line: value.span().line,
                start: value.span().start,
                end: field.span.end,
            };
            value = Expr::Field {
                base: Box::new(value),
                name,
                span,
            };
        } else if consume_named(state, "tkLParen") {
            let arguments = expression_list(state)?;
            let end = previous_end(state);
            let span = Span {
                line: value.span().line,
                start: value.span().start,
                end,
            };
            value = match value {
                Expr::Name(name, _) => Expr::Apply {
                    name,
                    arguments,
                    span,
                },
                base => Expr::Index {
                    base: Box::new(base),
                    indices: arguments,
                    span,
                },
            };
        } else {
            break;
        }
    }
    Ok(value)
}

fn expression(state: &mut ParseState, minimum: u8) -> Result<Expr, ParseResult> {
    let mut left = if let Some(op) = unary(state) {
        let start = state.tokens[state.at - 1].span;
        let operand = expression(state, 80)?;
        let span = Span {
            line: start.line,
            start: start.start,
            end: operand.span().end,
        };
        Expr::Unary {
            op,
            operand: Box::new(operand),
            span,
        }
    } else {
        primary(state)?
    };
    // Whether this loop built `left` from a comparison, which another
    // comparison then chains rather than compares.
    let mut chained = false;
    let span_of = |left: &Expr, right: &Expr| Span {
        line: left.span().line,
        start: left.span().start,
        end: right.span().end,
    };
    loop {
        if let Some(negated) = in_operator(state) {
            if COMPARISON_BINDING < minimum {
                break;
            }
            state.at += if negated { 2 } else { 1 };
            let haystack = haystack(state)?;
            let end = previous_end(state);
            let start = left.span();
            left = Expr::In {
                needle: Box::new(left),
                haystack,
                negated,
                span: Span { end, ..start },
            };
            chained = false;
            continue;
        }
        let Some((op, left_binding, right_binding)) = binary(state) else {
            break;
        };
        if left_binding < minimum {
            break;
        }
        state.at += 1;
        let right = expression(state, right_binding)?;
        let span = span_of(&left, &right);
        let comparison = left_binding == COMPARISON_BINDING;
        left = match left {
            Expr::Binary {
                op: first_op,
                left: first,
                right: middle,
                ..
            } if chained && comparison && state.python_expressions => Expr::Chain {
                first,
                rest: vec![(first_op, *middle), (op, right)],
                span,
            },
            Expr::Chain { first, mut rest, .. } if chained && comparison => {
                rest.push((op, right));
                Expr::Chain { first, rest, span }
            }
            left => Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            },
        };
        chained = comparison;
    }
    // `then IF condition ELSE otherwise` binds loosest, and nests only to
    // the right, as in Python.
    if state.python_expressions && minimum == 0 && consume_named(state, "tkIF") {
        let condition = expression(state, 1)?;
        if !consume_named(state, "tkELSE") {
            return Err(ParseResult::BadSyntax);
        }
        let otherwise = expression(state, 0)?;
        let span = span_of(&left, &otherwise);
        left = Expr::Conditional {
            condition: Box::new(condition),
            then: Box::new(left),
            otherwise: Box::new(otherwise),
            span,
        };
    }
    Ok(left)
}

const COMPARISON_BINDING: u8 = 60;

/// `IN` or `NOT IN` after an operand: Some(negated).
fn in_operator(state: &ParseState) -> Option<bool> {
    if !state.python_expressions {
        return None;
    }
    let is_in = |token: Option<&Token>| {
        token.is_some_and(|token| matches!(&token.kind, TokenKind::Identifier(word) if word == "IN"))
    };
    if is_in(state.token()) {
        Some(false)
    } else if at_named(state, "tkNOT") && is_in(state.tokens.get(state.at + 1)) {
        Some(true)
    } else {
        None
    }
}

/// IN's right side: `(a, b, …)` is a list of values; anything else, a
/// parenthesized one included, is a string or an array.
fn haystack(state: &mut ParseState) -> Result<Haystack, ParseResult> {
    let start = state.at;
    if consume_named(state, "tkLParen") {
        let first = expression(state, 0)?;
        if consume_named(state, "tkComma") {
            let mut values = vec![first, expression(state, 0)?];
            while consume_named(state, "tkComma") {
                values.push(expression(state, 0)?);
            }
            if !consume_named(state, "tkRParen") {
                return Err(ParseResult::BadSyntax);
            }
            return Ok(Haystack::Values(values));
        }
        state.at = start;
    }
    Ok(Haystack::Container(Box::new(expression(
        state,
        COMPARISON_BINDING + 1,
    )?)))
}

fn primary(state: &mut ParseState) -> Result<Expr, ParseResult> {
    let Some(token) = state.token().cloned() else {
        return Err(ParseResult::NotFound);
    };
    state.at += 1;
    match token.kind {
        TokenKind::Integer(value, suffix) => Ok(Expr::Literal(
            Literal::Integer(
                value,
                if suffix == Some('&') {
                    TypeName::Long
                } else {
                    TypeName::Integer
                },
            ),
            token.span,
        )),
        TokenKind::Real(value, suffix) => Ok(Expr::Literal(
            Literal::Real(
                value,
                if suffix == Some('#') {
                    TypeName::Double
                } else {
                    TypeName::Single
                },
            ),
            token.span,
        )),
        TokenKind::String(value) => Ok(Expr::Literal(Literal::String(value), token.span)),
        TokenKind::FormatString(segments) => {
            format_string(&segments, token.span, state.python_expressions)
        }
        TokenKind::Identifier(name) => name_or_apply(state, name, token.span),
        TokenKind::Reserved(id) if id == named("tkLParen") => {
            let value = expression(state, 0)?;
            if !consume_named(state, "tkRParen") {
                return Err(ParseResult::BadSyntax);
            }
            // Grouping only changes meaning around a reference.
            Ok(match value {
                Expr::Name(..) | Expr::Apply { .. } | Expr::Index { .. } | Expr::Field { .. } => {
                    Expr::Unary {
                        op: Unary::Grouped,
                        span: Span {
                            line: token.span.line,
                            start: token.span.start,
                            end: previous_end(state),
                        },
                        operand: Box::new(value),
                    }
                }
                value => value,
            })
        }
        TokenKind::Reserved(id)
            if tables::T_FUNC_DISPATCH
                .iter()
                .any(|(function, _)| *function == id) =>
        {
            let name = tables::token_spelling(id)
                .expect("function token has spelling")
                .to_ascii_uppercase();
            name_or_apply(state, name, token.span)
        }
        TokenKind::Reserved(id) if contextual_reserved_name(id).is_some() => name_or_apply(
            state,
            contextual_reserved_name(id).expect("guard checked name"),
            token.span,
        ),
        _ => {
            state.at -= 1;
            Err(ParseResult::NotFound)
        }
    }
}

fn name_or_apply(state: &mut ParseState, name: String, start: Span) -> Result<Expr, ParseResult> {
    let mut value = Expr::Name(name, start);
    loop {
        if matches!(
            state.token().map(|token| &token.kind),
            Some(TokenKind::Period)
        ) {
            state.at += 1;
            let Some(field) = state.token().cloned() else {
                return Err(ParseResult::BadSyntax);
            };
            let Some(name) = contextual_name(&field.kind) else {
                return Err(ParseResult::BadSyntax);
            };
            state.at += 1;
            let span = Span {
                line: value.span().line,
                start: value.span().start,
                end: field.span.end,
            };
            value = Expr::Field {
                base: Box::new(value),
                name,
                span,
            };
        } else if consume_named(state, "tkLParen") {
            let arguments = expression_list(state)?;
            let span = Span {
                line: value.span().line,
                start: value.span().start,
                end: previous_end(state),
            };
            value = match value {
                Expr::Name(name, _) => Expr::Apply {
                    name,
                    arguments,
                    span,
                },
                base => Expr::Index {
                    base: Box::new(base),
                    indices: arguments,
                    span,
                },
            };
        } else {
            return Ok(value);
        }
    }
}

fn expression_list(state: &mut ParseState) -> Result<Vec<Expr>, ParseResult> {
    let mut values = Vec::new();
    if consume_named(state, "tkRParen") {
        return Ok(values);
    }
    loop {
        values.push(expression(state, 0)?);
        if !consume_named(state, "tkComma") {
            if !consume_named(state, "tkRParen") {
                return Err(ParseResult::BadSyntax);
            }
            return Ok(values);
        }
    }
}

fn unary(state: &mut ParseState) -> Option<Unary> {
    let op = if at_named(state, "tkAdd") {
        Unary::Positive
    } else if at_named(state, "tkMinus") {
        Unary::Negative
    } else if at_named(state, "tkNOT") {
        Unary::Not
    } else {
        return None;
    };
    state.at += 1;
    Some(op)
}

fn binary(state: &ParseState) -> Option<(Binary, u8, u8)> {
    let pair = match state.token().map(|token| &token.kind)? {
        TokenKind::Comparison(op) => (*op, COMPARISON_BINDING),
        TokenKind::Reserved(id) if *id == named("tkIMP") => (Binary::Imp, 10),
        TokenKind::Reserved(id) if *id == named("tkEQV") => (Binary::Eqv, 20),
        TokenKind::Reserved(id) if *id == named("tkXOR") => (Binary::Xor, 30),
        TokenKind::Reserved(id) if *id == named("tkOR") => (Binary::Or, 40),
        TokenKind::Reserved(id) if *id == named("tkAND") => (Binary::And, 50),
        TokenKind::Reserved(id) if *id == named("tkEQ") => (Binary::Eq, COMPARISON_BINDING),
        TokenKind::Reserved(id) if *id == named("tkLT") => (Binary::Less, COMPARISON_BINDING),
        TokenKind::Reserved(id) if *id == named("tkGT") => (Binary::Greater, COMPARISON_BINDING),
        TokenKind::Reserved(id) if *id == named("tkAdd") => (Binary::Add, 70),
        TokenKind::Reserved(id) if *id == named("tkMinus") => (Binary::Subtract, 70),
        TokenKind::Reserved(id) if *id == named("tkMOD") => (Binary::Modulo, 75),
        TokenKind::Reserved(id) if *id == named("tkIdiv") => (Binary::IntegerDivide, 75),
        TokenKind::Reserved(id) if *id == named("tkMult") => (Binary::Multiply, 80),
        TokenKind::Reserved(id) if *id == named("tkDiv") => (Binary::Divide, 80),
        TokenKind::Reserved(id) if *id == named("tkPwr") => return Some((Binary::Power, 90, 90)),
        _ => return None,
    };
    Some((pair.0, pair.1, pair.1 + 1))
}

fn named(name: &str) -> u16 {
    tables::token_id(name).unwrap_or_else(|| panic!("qbasbnf token {name} is missing"))
}

/// The recovered scanner reserves every grammar spelling, while the source
/// AST still needs names such as WIDTH, PALETTE, and NAME in identifier-only
/// positions.  Only the grammar's structural keyword subset stays reserved
/// there; statement and intrinsic spellings remain contextual names.
fn contextual_name(kind: &TokenKind) -> Option<String> {
    match kind {
        TokenKind::Identifier(name) => Some(name.clone()),
        TokenKind::Reserved(id) => contextual_reserved_name(*id),
        _ => None,
    }
}

fn contextual_reserved_name(id: u16) -> Option<String> {
    let spelling = tables::token_spelling(id)?.to_ascii_uppercase();
    if !spelling
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        || matches!(
            spelling.as_str(),
            "ALIAS"
                | "AND"
                | "AS"
                | "BYVAL"
                | "CALL"
                | "CASE"
                | "CDECL"
                | "CONST"
                | "DECLARE"
                | "DIM"
                | "DO"
                | "ELSE"
                | "ELSEIF"
                | "END"
                | "EQV"
                | "EXIT"
                | "FOR"
                | "FUNCTION"
                | "GOTO"
                | "IF"
                | "IMP"
                | "LET"
                | "LOOP"
                | "MOD"
                | "NEXT"
                | "NOT"
                | "OPTION"
                | "OR"
                | "REM"
                | "SEG"
                | "SELECT"
                | "SHARED"
                | "STATIC"
                | "STEP"
                | "SUB"
                | "THEN"
                | "TO"
                | "TYPE"
                | "UNTIL"
                | "USING"
                | "WEND"
                | "WHILE"
                | "XOR"
        )
    {
        None
    } else {
        Some(spelling)
    }
}

fn at_named(state: &ParseState, name: &str) -> bool {
    state.token_id() == Some(named(name))
}

fn consume_named(state: &mut ParseState, name: &str) -> bool {
    state.consume_id(named(name))
}

fn previous_end(state: &ParseState) -> usize {
    state
        .at
        .checked_sub(1)
        .and_then(|index| state.tokens.get(index))
        .map_or(0, |token| token.span.end)
}

fn statement_end(statement: &Statement) -> usize {
    match statement {
        Statement::Dim(items)
        | Statement::Static(items)
        | Statement::Shared(items)
        | Statement::Redim(items) => items.last().map_or(0, |item| item.span.end),
        Statement::Erase(items) => items.last().map_or(0, |item| item.span().end),
        Statement::DefType { span, .. }
        | Statement::TypeDecl { span, .. }
        | Statement::Const { span, .. }
        | Statement::Assign { span, .. }
        | Statement::Label(_, span)
        | Statement::Goto(_, span)
        | Statement::CallOrGoto(_, span)
        | Statement::If { span, .. }
        | Statement::For { span, .. }
        | Statement::While { span, .. }
        | Statement::Do { span, .. }
        | Statement::Select { span, .. }
        | Statement::Call { span, .. }
        | Statement::Comment(span)
        | Statement::OptionExplicit(span)
        | Statement::OptionBase(_, span)
        | Statement::Exit(_, span)
        | Statement::DefSeg { span, .. }
        | Statement::OnError { span, .. }
        | Statement::Resume { span, .. }
        | Statement::Open { span, .. }
        | Statement::Close { span, .. }
        | Statement::LineInput { span, .. }
        | Statement::FileTransfer { span, .. }
        | Statement::Seek { span, .. }
        | Statement::Print { span, .. }
        | Statement::Input { span, .. }
        | Statement::Data { span, .. }
        | Statement::Read { span, .. }
        | Statement::Runtime { span, .. } => span.end,
    }
}

fn error<T>(state: &ParseState, message: &str) -> Result<T, ParseError> {
    let span = state.token().map_or(
        Span {
            line: 1,
            start: 0,
            end: 0,
        },
        |token| token.span,
    );
    Err(ParseError {
        span,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{ExitTarget, FileMode, ResumeTarget};

    fn module(source: &str, dialect: Dialect) -> Module {
        parse_vertical_slice(source, dialect).unwrap().module
    }

    fn all_dialects() -> [Dialect; 4] {
        [
            Dialect::QBasic11,
            Dialect::QuickBasic45,
            Dialect::Pds71,
            Dialect::VbDos,
        ]
    }

    fn name(expression: &Expr) -> &str {
        match expression {
            Expr::Name(name, _) => name,
            other => panic!("expected name, got {other:#?}"),
        }
    }

    fn integer(expression: &Expr) -> i64 {
        match expression {
            Expr::Literal(Literal::Integer(value, _), _) => *value,
            other => panic!("expected integer literal, got {other:#?}"),
        }
    }

    fn assignment(statement: &Statement) -> (&Expr, &Expr) {
        match statement {
            Statement::Assign { target, value, .. } => (target, value),
            other => panic!("expected assignment, got {other:#?}"),
        }
    }

    #[test]
    fn generated_let_retains_precedence_target_and_action() {
        let output =
            parse_vertical_slice("let total& = 2 + 3 * 4\r\n", Dialect::QuickBasic45).unwrap();
        let parsed = output.module;
        assert_eq!(parsed.statements.len(), 1);
        let (target, value) = assignment(&parsed.statements[0]);
        assert_eq!(name(target), "TOTAL&");
        let Expr::Binary {
            op: Binary::Add,
            left,
            right,
            ..
        } = value
        else {
            panic!("expected addition, got {value:#?}");
        };
        assert_eq!(integer(left), 2);
        let Expr::Binary {
            op: Binary::Multiply,
            left,
            right,
            ..
        } = &**right
        else {
            panic!("expected multiply on add right, got {right:#?}");
        };
        assert_eq!((integer(left), integer(right)), (3, 4));
        assert!(output.actions.contains(&AstAction::Unsupported("opStLet")));
    }

    #[test]
    fn generated_intrinsic_expression_retains_calls_and_suffixes() {
        let parsed = module("answer# = sin(1#) + cos(2#)\r\n", Dialect::QuickBasic45);
        let (target, value) = assignment(&parsed.statements[0]);
        assert_eq!(name(target), "ANSWER#");
        let Expr::Binary {
            op: Binary::Add,
            left,
            right,
            ..
        } = value
        else {
            panic!("expected intrinsic addition, got {value:#?}");
        };
        let Expr::Apply {
            name: left_name,
            arguments: left_args,
            ..
        } = &**left
        else {
            panic!("expected sin application, got {left:#?}");
        };
        let Expr::Apply {
            name: right_name,
            arguments: right_args,
            ..
        } = &**right
        else {
            panic!("expected cos application, got {right:#?}");
        };
        assert_eq!((left_name.as_str(), right_name.as_str()), ("SIN", "COS"));
        assert_eq!(left_args.len(), 1);
        assert_eq!(right_args.len(), 1);
    }

    #[test]
    fn generated_single_line_if_builds_nested_assignments() {
        let parsed = module(
            "if leftValue < rightValue then result = leftValue else result = rightValue\r\n",
            Dialect::QuickBasic45,
        );
        let Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected if, got {:#?}", parsed.statements[0]);
        };
        let Expr::Binary {
            op: Binary::Less,
            left,
            right,
            ..
        } = condition
        else {
            panic!("expected comparison, got {condition:#?}");
        };
        assert_eq!((name(left), name(right)), ("LEFTVALUE", "RIGHTVALUE"));
        assert_eq!(name(assignment(&then_branch[0]).1), "LEFTVALUE");
        assert_eq!(name(assignment(&else_branch[0]).1), "RIGHTVALUE");
    }

    #[test]
    fn generated_block_if_collects_else_and_resumes_after_end_if() {
        let source = concat!(
            "if leftValue < rightValue then\r\n",
            "result = leftValue\r\n",
            "else\r\n",
            "result = rightValue\r\n",
            "end if\r\n",
            "observed = result\r\n",
        );
        let parsed = module(source, Dialect::QuickBasic45);
        assert_eq!(parsed.statements.len(), 2);
        let Statement::If {
            then_branch,
            else_branch,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected block if, got {:#?}", parsed.statements[0]);
        };
        assert_eq!(name(assignment(&then_branch[0]).1), "LEFTVALUE");
        assert_eq!(name(assignment(&else_branch[0]).1), "RIGHTVALUE");
        assert_eq!(name(assignment(&parsed.statements[1]).0), "OBSERVED");
    }

    #[test]
    fn generated_labels_are_distinct_from_the_following_statement() {
        let symbolic = module("handler:\r\nobserved = 7\r\n", Dialect::QuickBasic45);
        assert!(
            matches!(&symbolic.statements[..], [Statement::Label(label, _), Statement::Assign { .. }] if label == "HANDLER")
        );
        let numbered = module("100 observed = 7 \\ divisor\r\n", Dialect::QuickBasic45);
        assert!(
            matches!(&numbered.statements[..], [Statement::Label(label, _), Statement::Assign { .. }] if label == "100")
        );
    }

    #[test]
    fn generated_symbolic_label_separates_inline_data() {
        let parsed = module("mono: data 15, 7, 0\r\n", Dialect::QuickBasic45);
        assert!(matches!(
            &parsed.statements[..],
            [Statement::Label(label, _), Statement::Data { values, .. }]
                if label == "MONO" && values.len() == 3
        ));
    }

    #[test]
    fn generated_implicit_call_keeps_arguments_and_explicitness() {
        let source = "visit firstValue, secondValue + 1\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        let Statement::Call {
            name: call_name,
            arguments,
            explicit,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected implicit call, got {:#?}", parsed.statements[0]);
        };
        assert_eq!(call_name, "VISIT");
        assert!(!explicit);
        assert_eq!(name(&arguments[0]), "FIRSTVALUE");
        assert!(matches!(
            &arguments[1],
            Expr::Binary {
                op: Binary::Add,
                ..
            }
        ));
    }

    #[test]
    fn implicit_call_parentheses_group_the_first_argument() {
        // QGL's ENT module stopped at the closing parenthesis and discarded
        // both the following arithmetic and the second argument.
        let parsed = module(
            "qglMousePos (screenWidth - 1) * yaw / 360, screenHeight * pitch\r\n",
            Dialect::VbDos,
        );
        let Statement::Call {
            name,
            arguments,
            explicit,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected implicit call, got {:#?}", parsed.statements[0]);
        };
        assert_eq!(name, "QGLMOUSEPOS");
        assert!(!explicit);
        assert_eq!(arguments.len(), 2);
        assert!(matches!(
            arguments[0],
            Expr::Binary {
                op: Binary::Divide,
                ..
            }
        ));
        assert!(matches!(
            arguments[1],
            Expr::Binary {
                op: Binary::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn generated_simple_statement_descriptors_have_specific_ast_nodes() {
        assert!(matches!(
            module("option base 1\r\n", Dialect::QuickBasic45).statements[0],
            Statement::OptionBase(1, _)
        ));
        assert!(
            matches!(module("call updateShared(observedValue)\r\n", Dialect::QuickBasic45).statements[0], Statement::Call { ref name, explicit: true, .. } if name == "UPDATESHARED")
        );
        assert!(
            matches!(module("screen 1\r\n", Dialect::QuickBasic45).statements[0], Statement::Runtime { ref name, .. } if name == "SCREEN")
        );
        assert!(
            matches!(module("randomize seedValue\r\n", Dialect::QuickBasic45).statements[0], Statement::Call { ref name, explicit: false, .. } if name == "RANDOMIZE")
        );
        assert!(
            matches!(module("environ \"QBCOMPAT=Alpha42\"\r\n", Dialect::QuickBasic45).statements[0], Statement::Call { ref name, explicit: false, .. } if name == "ENVIRON")
        );
        assert!(
            matches!(module("gosub firstPart\r\n", Dialect::QuickBasic45).statements[0], Statement::Call { ref name, ref arguments, explicit: false, .. } if name == "GOSUB" && matches!(&arguments[..], [Expr::Name(label, _)] if label == "FIRSTPART"))
        );
    }

    #[test]
    fn generated_option_explicit_is_inherited_by_every_semantic_profile() {
        let source = "option explicit\r\ndim observedValue as long\r\n";
        for dialect in all_dialects() {
            let parsed = module(source, dialect);
            assert!(matches!(parsed.statements[0], Statement::OptionExplicit(_)));
            assert!(matches!(parsed.statements[1], Statement::Dim(_)));
        }
    }

    #[test]
    fn generated_alias_sub_declaration_keeps_vbdos_abi_source_facts() {
        let source = "declare sub consume alias \"CONSUME\" (seg buffer as any)\r\n";
        for dialect in all_dialects() {
            let parsed = module(source, dialect);
            let [procedure] = &parsed.procedures[..] else {
                panic!("expected one declaration");
            };
            assert_eq!(procedure.name, "CONSUME");
            assert_eq!(procedure.alias.as_deref(), Some("CONSUME"));
            assert!(!procedure.cdecl);
            assert_eq!(procedure.kind, ProcedureKind::Sub);
            assert!(procedure.declaration);
            assert!(procedure.parameters[0].segmented);
            assert_eq!(
                procedure.parameters[0].declaration.type_name,
                Some(TypeName::Named("ANY".into()))
            );
        }
    }

    #[test]
    fn generated_on_local_error_keeps_the_typed_label_for_every_profile() {
        for dialect in all_dialects() {
            for (source, label) in [
                ("on local error goto caughtError\r\n", "CAUGHTERROR"),
                ("on local error goto 100\r\n", "100"),
            ] {
                let parsed = module(source, dialect);
                assert!(
                    matches!(&parsed.statements[0], Statement::OnError { label: actual, local: true, .. } if actual == label),
                    "{dialect:?}: {source:?} -> {parsed:#?}"
                );
            }
        }
    }

    #[test]
    fn generated_file_and_error_statements_have_specific_ast_nodes() {
        assert!(matches!(
            module("def seg = varseg(values(1))\r\n", Dialect::QuickBasic45).statements[0],
            Statement::DefSeg { value: Some(_), .. }
        ));
        assert!(matches!(
            module("erase cacheList\r\n", Dialect::QuickBasic45).statements[0],
            Statement::Erase(_)
        ));
        assert!(matches!(
            module(
                "open \"ITEM.DAT\" for output as #fileNumber\r\n",
                Dialect::QuickBasic45
            )
            .statements[0],
            Statement::Open {
                mode: FileMode::Output,
                ..
            }
        ));
        assert!(matches!(
            module("close #fileNumber\r\n", Dialect::QuickBasic45).statements[0],
            Statement::Close { .. }
        ));
        assert!(
            matches!(module("restore secondData\r\n", Dialect::QuickBasic45).statements[0], Statement::Runtime { ref name, .. } if name == "RESTORE")
        );
        assert!(matches!(
            module("resume next\r\n", Dialect::QuickBasic45).statements[0],
            Statement::Resume {
                target: ResumeTarget::Next,
                ..
            }
        ));
        assert!(
            matches!(module("return finished\r\n", Dialect::QuickBasic45).statements[0], Statement::Call { ref name, explicit: false, .. } if name == "RETURN")
        );
    }

    #[test]
    fn generated_declarations_preserve_const_default_type_and_udt_fields() {
        assert!(
            matches!(module("const limit = 7\r\n", Dialect::QuickBasic45).statements[0], Statement::Const { ref name, ref value, .. } if name == "LIMIT" && integer(value) == 7)
        );
        assert!(
            matches!(module("defint a-c, x-z\r\n", Dialect::QuickBasic45).statements[0], Statement::DefType { ref ranges, .. } if ranges == &vec![('A', 'C'), ('X', 'Z')])
        );
        let parsed = module(
            concat!(
                "type Vertex\r\n",
                "x as integer\r\n",
                "label as string * 12\r\n",
                "end type\r\n"
            ),
            Dialect::QuickBasic45,
        );
        let Statement::TypeDecl { name, fields, .. } = &parsed.statements[0] else {
            panic!("expected UDT")
        };
        assert_eq!(name, "VERTEX");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].fixed_length.as_ref().map(integer), Some(12));
    }

    #[test]
    fn generated_loop_blocks_collect_bodies_and_conditions() {
        let parsed = module(
            concat!(
                "for index = 1 to 4 step 2\r\n",
                "total = total + index\r\n",
                "next index\r\n"
            ),
            Dialect::QuickBasic45,
        );
        assert!(
            matches!(&parsed.statements[0], Statement::For { counter, start, end, step: Some(step), body, .. } if name(counter) == "INDEX" && integer(start) == 1 && integer(end) == 4 && integer(step) == 2 && body.len() == 1)
        );
        let pre = module(
            concat!(
                "do while index < 4\r\n",
                "index = index + 1\r\n",
                "loop\r\n"
            ),
            Dialect::QuickBasic45,
        );
        assert!(
            matches!(&pre.statements[0], Statement::Do { pre: Some((true, _)), post: None, body, .. } if body.len() == 1)
        );
        let post = module(
            concat!(
                "do\r\n",
                "index = index + 1\r\n",
                "loop until index = 4\r\n"
            ),
            Dialect::QuickBasic45,
        );
        assert!(
            matches!(&post.statements[0], Statement::Do { pre: None, post: Some((false, _)), body, .. } if body.len() == 1)
        );
    }

    #[test]
    fn generated_specialized_statements_remain_distinct_from_runtime_fallbacks() {
        let while_block = module(
            concat!("while index < 4\r\n", "index = index + 1\r\n", "wend\r\n"),
            Dialect::QuickBasic45,
        );
        assert!(
            matches!(&while_block.statements[0], Statement::While { body, .. } if body.len() == 1)
        );
        assert!(matches!(
            module("seek #fileNumber, 3\r\n", Dialect::QuickBasic45).statements[0],
            Statement::Seek { .. }
        ));
        for source in [
            "put #fileNumber, 1, firstValue\r\n",
            "get #fileNumber, 3, readValue\r\n",
        ] {
            assert!(
                matches!(
                    module(source, Dialect::QuickBasic45).statements[0],
                    Statement::FileTransfer { .. }
                ),
                "{source:?}"
            );
        }
        assert!(matches!(
            module(
                "mid$(dynamicText, 2, 3) = \"XYZ\"\r\n",
                Dialect::QuickBasic45
            )
            .statements[0],
            Statement::Assign { .. }
        ));
        for (source, expected) in [
            ("lset leftFixed = sourceText\r\n", "LSET"),
            ("rset rightFixed = sourceText\r\n", "RSET"),
        ] {
            assert!(
                matches!(module(source, Dialect::QuickBasic45).statements[0], Statement::Call { ref name, explicit: false, .. } if name == expected),
                "{source:?}"
            );
        }
        for (source, expected) in [
            ("bsave \"ITEM.BSV\", varptr(sourceText), 4\r\n", "BSAVE"),
            ("bload \"ITEM.BSV\", varptr(targetText)\r\n", "BLOAD"),
        ] {
            assert!(
                matches!(module(source, Dialect::QuickBasic45).statements[0], Statement::Call { ref name, explicit: false, .. } if name == expected),
                "{source:?}"
            );
        }
    }

    #[test]
    fn generated_select_case_collects_arms_and_else() {
        let source = concat!(
            "select case choice\r\n",
            "case 1, 2\r\n",
            "result = 10\r\n",
            "case else\r\n",
            "result = 20\r\n",
            "end select\r\n",
        );
        let parsed = module(source, Dialect::QuickBasic45);
        let Statement::Select {
            selector,
            arms,
            otherwise,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected select")
        };
        assert_eq!(name(selector), "CHOICE");
        assert_eq!(arms.len(), 1);
        assert_eq!(arms[0].0.len(), 2);
        assert_eq!(otherwise.len(), 1);
    }

    #[test]
    fn generated_block_if_accepts_udt_conditions_and_def_seg_body() {
        let source = concat!(
            "if secondVertex.x = &H1234 and secondVertex.label = \"node\" and peek(varptr(secondVertex)) = &H34 then\r\n",
            "def seg\r\n",
            "print \"PASS udt\"\r\n",
            "else\r\n",
            "def seg\r\n",
            "print \"FAIL udt\"\r\n",
            "end if\r\n",
        );
        let parsed = module(source, Dialect::QuickBasic45);
        let Statement::If {
            then_branch,
            else_branch,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected if")
        };
        assert!(matches!(
            then_branch[0],
            Statement::DefSeg { value: None, .. }
        ));
        assert!(matches!(
            else_branch[0],
            Statement::DefSeg { value: None, .. }
        ));
    }

    #[test]
    fn generated_line_input_uses_input_action_not_the_shared_line_keyword() {
        let source = "line input #fileNumber, text\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        assert!(matches!(parsed.statements[0], Statement::LineInput { .. }));
    }

    #[test]
    fn generated_if_span_includes_a_nested_exit_statement() {
        let source = "if index = 3 then exit for\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        let Statement::If { then_branch, .. } = &parsed.statements[0] else {
            panic!("expected if")
        };
        assert!(matches!(
            then_branch[0],
            Statement::Exit(ExitTarget::For, _)
        ));
    }

    #[test]
    fn generated_dim_preserves_bounds_types_and_shared_action() {
        let source = "dim shared cells(1 to 7, 3) as long, label as string * 12\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        let Statement::Dim(declarations) = &parsed.statements[0] else {
            panic!("expected dim")
        };
        assert_eq!(declarations.len(), 2);
        assert!(declarations[0].shared && declarations[0].array);
        assert_eq!(declarations[0].bounds.len(), 2);
        assert_eq!(declarations[1].fixed_length.as_ref().map(integer), Some(12));
    }

    #[test]
    fn generated_print_preserves_file_channel_and_item_separators() {
        let source = "print #fileNumber, \"A\"; value\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        let Statement::Print {
            file: Some(_),
            items,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected file print")
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].separator, PrintSeparator::Semicolon);
        assert_eq!(items[1].separator, PrintSeparator::End);
    }

    #[test]
    fn generated_actions_are_scoped_to_the_statement_that_emitted_them() {
        let output = parse_vertical_slice(
            "dim prior as integer\r\nrandomize 1\r\n",
            Dialect::QuickBasic45,
        )
        .unwrap();
        assert!(
            matches!(&output.module.statements[..], [Statement::Dim(_), Statement::Call { name, explicit: false, .. }] if name == "RANDOMIZE")
        );
        assert!(output.actions.contains(&AstAction::Dim));
        assert!(output
            .actions
            .contains(&AstAction::LegacyImplicitCall("RANDOMIZE")));
    }

    #[test]
    fn generated_goto_preserves_symbolic_and_numeric_targets() {
        for (source, label) in [("goto handler\r\n", "HANDLER"), ("goto 32000\r\n", "32000")] {
            let output = parse_vertical_slice(source, Dialect::QuickBasic45).unwrap();
            assert!(
                matches!(&output.module.statements[0], Statement::Goto(actual, _) if actual == label)
            );
            assert!(output.actions.contains(&AstAction::Goto));
        }
    }

    #[test]
    fn generated_on_error_distinguishes_disable_zero_from_a_label() {
        for (source, label) in [
            ("on error goto handler\r\n", "HANDLER"),
            ("on error goto 0\r\n", "0"),
        ] {
            let parsed = module(source, Dialect::QuickBasic45);
            assert!(
                matches!(&parsed.statements[0], Statement::OnError { label: actual, local: false, .. } if actual == label)
            );
        }
    }

    #[test]
    fn generated_zero_argument_procedure_declarations_preserve_kind_and_result() {
        let source = "declare sub probe\r\ndeclare function value&\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        assert_eq!(parsed.procedures.len(), 2);
        assert!(
            matches!(&parsed.procedures[0], Procedure { name, kind: ProcedureKind::Sub, declaration: true, parameters, .. } if name == "PROBE" && parameters.is_empty())
        );
        assert!(
            matches!(&parsed.procedures[1], Procedure { name, kind: ProcedureKind::Function, result: Some(TypeName::Long), declaration: true, .. } if name == "VALUE&")
        );
    }

    #[test]
    fn generated_procedure_definitions_collect_body_until_matching_end() {
        let source = concat!(
            "sub probe\r\n",
            "observed = 7\r\n",
            "end sub\r\n",
            "function value&\r\n",
            "value& = 11\r\n",
            "end function\r\n",
        );
        let parsed = module(source, Dialect::QuickBasic45);
        assert_eq!(parsed.procedures.len(), 2);
        assert!(
            matches!(&parsed.procedures[0], Procedure { name, kind: ProcedureKind::Sub, body, declaration: false, .. } if name == "PROBE" && body.len() == 1)
        );
        assert!(
            matches!(&parsed.procedures[1], Procedure { name, kind: ProcedureKind::Function, body, declaration: false, .. } if name == "VALUE&" && body.len() == 1)
        );
    }

    #[test]
    fn generated_procedure_parameters_preserve_byval_seg_array_and_type() {
        let source = "declare function mix&(leftValue as long, byval middleValue as long, seg values() as integer)\r\n";
        let parsed = module(source, Dialect::QuickBasic45);
        let procedure = &parsed.procedures[0];
        assert_eq!(procedure.parameters.len(), 3);
        assert!(!procedure.parameters[0].by_value);
        assert!(procedure.parameters[1].by_value);
        assert!(procedure.parameters[2].segmented);
        assert!(procedure.parameters[2].declaration.array);
    }

    #[test]
    fn generated_function_headers_preserve_explicit_result_types() {
        let source = concat!(
            "declare function mix_long(byval left_value as long) as long\r\n",
            "function mix_long(byval left_value as long) as long\r\n",
            "mix_long = left_value + 1\r\n",
            "end function\r\n",
        );
        let parsed = module(source, Dialect::VbDos);
        assert_eq!(parsed.procedures.len(), 2);
        assert!(parsed
            .procedures
            .iter()
            .all(|procedure| procedure.result == Some(TypeName::Long)));
    }

    #[test]
    fn generated_cdecl_alias_is_inherited_and_preserves_the_source_abi_contract() {
        for dialect in all_dialects() {
            let parsed = module(
                "declare function compat_external cdecl alias \"compat_external\" (byval source_value as long) as long\r\n",
                dialect,
            );
            let [procedure] = parsed.procedures.as_slice() else {
                panic!("expected one external declaration for {dialect:?}")
            };
            assert_eq!(procedure.name, "COMPAT_EXTERNAL");
            assert_eq!(procedure.alias.as_deref(), Some("compat_external"));
            assert!(procedure.cdecl && procedure.declaration);
            assert_eq!(procedure.kind, ProcedureKind::Function);
            assert_eq!(procedure.result, Some(TypeName::Long));
            assert_eq!(procedure.parameters.len(), 1);
            assert!(procedure.parameters[0].by_value);
            assert_eq!(
                procedure.parameters[0].declaration.type_name,
                Some(TypeName::Long)
            );
        }
    }
}
