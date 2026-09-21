use crate::error::Diagnostic;
use crate::lexer::Token;
use crate::lexer::TokenKind;
use crate::syntax::BinaryOp;
use crate::syntax::Expr;
use crate::syntax::Function;
use crate::syntax::Module;
use crate::syntax::Parameter;
use crate::syntax::Span;
use crate::syntax::Statement;
use crate::syntax::TypeName;
use crate::syntax::UnaryOp;

pub fn parse(tokens: Vec<Token>) -> Result<Module, Diagnostic> {
    Parser { tokens, at: 0 }.module()
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn module(&mut self) -> Result<Module, Diagnostic> {
        let mut functions = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Eof) {
            if self
                .take(|kind| matches!(kind, TokenKind::Newline))
                .is_some()
            {
                continue;
            }
            functions.push(self.function()?);
        }
        if functions.is_empty() {
            return Err(Diagnostic::new(
                self.peek().span,
                "module contains no functions",
            ));
        }
        Ok(Module { functions })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let start = self
            .expect(|kind| matches!(kind, TokenKind::Fn), "expected 'fn'")?
            .span;
        let (name, _) = self.identifier("expected function name")?;
        self.expect(
            |kind| matches!(kind, TokenKind::LeftParen),
            "expected '(' after function name",
        )?;
        let mut parameters = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RightParen) {
            loop {
                let (parameter_name, span) = self.identifier("expected parameter name")?;
                self.expect(
                    |kind| matches!(kind, TokenKind::Colon),
                    "expected ':' after parameter name",
                )?;
                let type_name = self.type_name()?;
                if type_name == TypeName::Void {
                    return Err(Diagnostic::new(span, "a parameter cannot have type void"));
                }
                parameters.push(Parameter {
                    name: parameter_name,
                    type_name,
                    span,
                });
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                    break;
                }
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RightParen),
            "expected ')' after parameters",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Arrow),
            "expected '->' and a result type",
        )?;
        let result = self.type_name()?;
        let body = self.suite()?;
        Ok(Function {
            name,
            parameters,
            result,
            body,
            span: start,
        })
    }

    fn suite(&mut self) -> Result<Vec<Statement>, Diagnostic> {
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' before block",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before block",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected an indented block",
        )?;
        let mut statements = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            statements.push(self.statement()?);
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated block",
        )?;
        if statements.is_empty() {
            return Err(Diagnostic::new(self.peek().span, "block cannot be empty"));
        }
        Ok(statements)
    }

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        match self.peek().kind {
            TokenKind::Let | TokenKind::Var => self.binding(),
            TokenKind::Return => self.return_statement(),
            TokenKind::If => self.if_statement(),
            TokenKind::While => self.while_statement(),
            TokenKind::Break => {
                let span = self.bump().span;
                self.line_end()?;
                Ok(Statement::Break(span))
            }
            TokenKind::Continue => {
                let span = self.bump().span;
                self.line_end()?;
                Ok(Statement::Continue(span))
            }
            TokenKind::Identifier(_) if matches!(self.peek_n(1).kind, TokenKind::Equal) => {
                let (name, span) = self.identifier("expected assignment target")?;
                self.bump();
                let value = self.expression(0)?;
                self.line_end()?;
                Ok(Statement::Assign { name, value, span })
            }
            _ => {
                let expression = self.expression(0)?;
                self.line_end()?;
                Ok(Statement::Expr(expression))
            }
        }
    }

    fn binding(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.bump().clone();
        let mutable = matches!(token.kind, TokenKind::Var);
        let (name, _) = self.identifier("expected binding name")?;
        let annotation = if self.take(|kind| matches!(kind, TokenKind::Colon)).is_some() {
            let type_name = self.type_name()?;
            if type_name == TypeName::Void {
                return Err(Diagnostic::new(
                    token.span,
                    "a binding cannot have type void",
                ));
            }
            Some(type_name)
        } else {
            None
        };
        self.expect(
            |kind| matches!(kind, TokenKind::Equal),
            "a binding requires an initializer",
        )?;
        let value = self.expression(0)?;
        self.line_end()?;
        Ok(Statement::Bind {
            mutable,
            name,
            annotation,
            value,
            span: token.span,
        })
    }

    fn return_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let value = if matches!(self.peek().kind, TokenKind::Newline) {
            None
        } else {
            Some(self.expression(0)?)
        };
        self.line_end()?;
        Ok(Statement::Return { value, span })
    }

    fn if_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let condition = self.expression(0)?;
        let then_branch = self.suite()?;
        let else_branch = if self.take(|kind| matches!(kind, TokenKind::Else)).is_some() {
            self.suite()?
        } else {
            Vec::new()
        };
        Ok(Statement::If {
            condition,
            then_branch,
            else_branch,
            span,
        })
    }

    fn while_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let condition = self.expression(0)?;
        let body = self.suite()?;
        Ok(Statement::While {
            condition,
            body,
            span,
        })
    }

    fn expression(&mut self, minimum_binding: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.prefix()?;
        loop {
            if matches!(self.peek().kind, TokenKind::LeftParen) {
                if 30 < minimum_binding {
                    break;
                }
                left = self.call(left)?;
                continue;
            }
            let Some((left_binding, right_binding, operation)) = infix(&self.peek().kind) else {
                break;
            };
            if left_binding < minimum_binding {
                break;
            }
            self.bump();
            let right = self.expression(right_binding)?;
            let left_span = left.span();
            let right_span = right.span();
            left = Expr::Binary {
                op: operation,
                left: Box::new(left),
                right: Box::new(right),
                span: Span::new(left_span.line, left_span.column, right_span.end_column),
            };
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer(value, token.span)),
            TokenKind::True => Ok(Expr::Boolean(true, token.span)),
            TokenKind::False => Ok(Expr::Boolean(false, token.span)),
            TokenKind::Identifier(name) => Ok(Expr::Name(name, token.span)),
            TokenKind::Minus | TokenKind::Not => {
                let operation = if matches!(token.kind, TokenKind::Minus) {
                    UnaryOp::Negative
                } else {
                    UnaryOp::Not
                };
                let operand = self.expression(25)?;
                let end = operand.span().end_column;
                Ok(Expr::Unary {
                    op: operation,
                    operand: Box::new(operand),
                    span: Span::new(token.span.line, token.span.column, end),
                })
            }
            TokenKind::LeftParen => {
                let expression = self.expression(0)?;
                self.expect(
                    |kind| matches!(kind, TokenKind::RightParen),
                    "expected ')' after expression",
                )?;
                Ok(expression)
            }
            _ => Err(Diagnostic::new(token.span, "expected expression")),
        }
    }

    fn call(&mut self, callee: Expr) -> Result<Expr, Diagnostic> {
        let (name, start) = match callee {
            Expr::Name(name, span) => (name, span),
            other => {
                return Err(Diagnostic::new(
                    other.span(),
                    "only named functions can be called",
                ))
            }
        };
        self.bump();
        let mut arguments = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RightParen) {
            loop {
                arguments.push(self.expression(0)?);
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                    break;
                }
            }
        }
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RightParen),
            "expected ')' after arguments",
        )?;
        Ok(Expr::Call {
            name,
            arguments,
            span: Span::new(start.line, start.column, close.span.end_column),
        })
    }

    fn type_name(&mut self) -> Result<TypeName, Diagnostic> {
        let token = self.bump();
        match token.kind {
            TokenKind::I16 => Ok(TypeName::I16),
            TokenKind::I32 => Ok(TypeName::I32),
            TokenKind::Bool => Ok(TypeName::Bool),
            TokenKind::Void => Ok(TypeName::Void),
            _ => Err(Diagnostic::new(token.span, "expected a type name")),
        }
    }

    fn line_end(&mut self) -> Result<(), Diagnostic> {
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected end of line",
        )?;
        Ok(())
    }

    fn identifier(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.bump();
        if let TokenKind::Identifier(name) = &token.kind {
            Ok((name.clone(), token.span))
        } else {
            Err(Diagnostic::new(token.span, message))
        }
    }

    fn expect(
        &mut self,
        predicate: impl FnOnce(&TokenKind) -> bool,
        message: &str,
    ) -> Result<&Token, Diagnostic> {
        if predicate(&self.peek().kind) {
            Ok(self.bump())
        } else {
            Err(Diagnostic::new(self.peek().span, message))
        }
    }

    fn take(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> Option<&Token> {
        predicate(&self.peek().kind).then(|| self.bump())
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.at]
    }

    fn peek_n(&self, amount: usize) -> &Token {
        self.tokens
            .get(self.at + amount)
            .unwrap_or_else(|| self.tokens.last().expect("lexer emits EOF"))
    }

    fn bump(&mut self) -> &Token {
        let index = self.at;
        if !matches!(self.tokens[index].kind, TokenKind::Eof) {
            self.at += 1;
        }
        &self.tokens[index]
    }
}

fn infix(kind: &TokenKind) -> Option<(u8, u8, BinaryOp)> {
    Some(match kind {
        TokenKind::EqualEqual => (5, 6, BinaryOp::Equal),
        TokenKind::NotEqual => (5, 6, BinaryOp::NotEqual),
        TokenKind::Less => (5, 6, BinaryOp::Less),
        TokenKind::LessEqual => (5, 6, BinaryOp::LessEqual),
        TokenKind::Greater => (5, 6, BinaryOp::Greater),
        TokenKind::GreaterEqual => (5, 6, BinaryOp::GreaterEqual),
        TokenKind::Plus => (10, 11, BinaryOp::Add),
        TokenKind::Minus => (10, 11, BinaryOp::Subtract),
        TokenKind::Star => (20, 21, BinaryOp::Multiply),
        TokenKind::Slash => (20, 21, BinaryOp::Divide),
        TokenKind::Percent => (20, 21, BinaryOp::Remainder),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::lexer::lex;

    use super::*;

    #[test]
    fn parses_precedence_and_nested_blocks() {
        let module = parse(
            lex("fn choose(value: i16) -> i16:\n\
                 \x20\x20\x20\x20if value < 0:\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20return -value\n\
                 \x20\x20\x20\x20else:\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20return value + 2 * 3\n")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].body.len(), 1);
        let Statement::If { else_branch, .. } = &module.functions[0].body[0] else {
            panic!("expected if")
        };
        let Statement::Return {
            value: Some(Expr::Binary { op, right, .. }),
            ..
        } = &else_branch[0]
        else {
            panic!("expected return expression")
        };
        assert_eq!(*op, BinaryOp::Add);
        assert!(matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::Multiply,
                ..
            }
        ));
    }
}
