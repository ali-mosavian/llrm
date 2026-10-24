//! Block statements: IF, FOR, WHILE, DO, SELECT.

use super::*;

impl Ast<'_> {
    /// Statements up to a line `stop` recognises, which is left unread.
    pub(super) fn collect_until(
        &mut self,
        stop: fn(&Self) -> bool,
    ) -> R<Vec<Statement>> {
        let mut body = Vec::new();
        loop {
            self.skip_separators();
            if self.i >= self.tokens.len() {
                return self.fail();
            }
            if stop(self) {
                return Ok(body);
            }
            let before = self.out.len();
            self.statement()?;
            body.extend(self.out.split_off(before));
            if !self.ends_statement() {
                return self.fail();
            }
        }
    }

    pub(super) fn at_end_select(&self) -> bool {
        self.is(Kw::END) && self.is_n(1, Kw::SELECT)
    }

    pub(super) fn if_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let condition = self.exp()?;
        if self.is(Kw::GOTO) {
            // IF c GOTO label [ELSE …]: the label is the whole THEN branch
            let goto = self.take();
            let (label, _) = self.lab_ln()?;
            let then_branch = vec![Statement::Goto(label, self.span(goto))];
            return self.finish_if_with(condition, self.span(keyword), then_branch);
        }
        self.expect(Kw::THEN)?;
        // a block IF starts its line
        if self.is(Kw::NewLine) && !self.first_on_line(keyword) {
            return self.fail_because("Must be first statement on the line");
        }
        self.finish_if(condition, self.span(keyword))
    }

    pub(super) fn finish_if_with(
        &mut self,
        condition: Expr,
        keyword_span: Span,
        then_branch: Vec<Statement>,
    ) -> R<()> {
        let mut else_branch = Vec::new();
        if self.eat(Kw::ELSE) {
            // `ELSE` may end the line
            if !(self.is(Kw::NewLine) || self.i >= self.tokens.len()) {
                else_branch = self.single_line_branch()?;
            }
        }
        let end = else_branch.last().or_else(|| then_branch.last()).map_or(condition.span().end, statement_end);
        let span = Span { line: condition.span().line, start: keyword_span.start, end };
        self.out.push(Statement::If { condition, then_branch, else_branch, span });
        Ok(())
    }

    pub(super) fn finish_if(
        &mut self,
        condition: Expr,
        keyword_span: Span,
    ) -> R<()> {
        if self.eat(Kw::NewLine) {
            return self.block_if(condition, keyword_span);
        }
        // `IF c THEN: stmt` and `IF c THEN ELSE stmt`: the THEN branch may
        // start with colons or be empty
        while self.eat(Kw::Colon) {}
        let then_branch = if self.is(Kw::ELSE) { Vec::new() } else { self.single_line_branch()? };
        self.finish_if_with(condition, keyword_span, then_branch)
    }

    pub(super) fn single_line_branch(&mut self) -> R<Vec<Statement>> {
        let mut branch = Vec::new();
        loop {
            let before = self.out.len();
            // A line number where a branch starts is a GOTO to it, not a label.
            let number = match &self.tokens.get(self.i).map(|token| &token.kind) {
                Some(TokenKind::Integer(value, None)) if branch.is_empty() && (0..=65_529).contains(value) => {
                    Some((value.to_string(), self.span(self.i)))
                }
                _ => None,
            };
            if let Some((label, span)) = number {
                self.i += 1;
                self.out.push(Statement::Goto(label, span));
            } else {
                if self.i >= self.tokens.len() {
                    return self.fail();
                }
                self.statement()?;
            }
            let mut parsed = self.out.split_off(before);
            if parsed.is_empty() {
                return self.fail();
            }
            branch.append(&mut parsed);
            if self.i >= self.tokens.len() || self.is(Kw::NewLine) || self.is(Kw::ELSE) {
                break;
            }
            if !self.separator() {
                return self.fail();
            }
            if self.i >= self.tokens.len() || self.is(Kw::NewLine) || self.is(Kw::ELSE) {
                break;
            }
        }
        Ok(branch)
    }

    pub(super) fn block_if(
        &mut self,
        condition: Expr,
        span: Span,
    ) -> R<()> {
        let mut then_branch = Vec::new();
        let mut else_branch = Vec::new();
        let mut in_else = false;
        self.blocks.push(Open { what: "block IF", closer: "END IF", span });
        loop {
            self.skip_separators();
            let closer = self.is(Kw::ELSE) || self.is(Kw::ELSEIF) || (self.is(Kw::END) && self.is_n(1, Kw::IF));
            if closer && !self.first_on_line(self.i) {
                return self.fail_because("Must be first statement on the line");
            }
            if self.eat(Kw::ELSE) {
                if in_else {
                    return self.fail();
                }
                in_else = true;
                continue;
            }
            if self.is(Kw::ELSEIF) {
                if in_else {
                    return self.fail();
                }
                let else_if = self.take();
                let nested_condition = self.exp()?;
                self.expect(Kw::THEN)?;
                let before = self.out.len();
                self.finish_if(nested_condition, self.span(else_if))?;
                let mut nested = self.out.split_off(before);
                if nested.len() != 1 {
                    return self.fail();
                }
                else_branch.push(nested.remove(0));
                self.blocks.pop();
                self.out.push(Statement::If { condition, then_branch, else_branch, span });
                return Ok(());
            }
            if self.is(Kw::END) && self.is_n(1, Kw::IF) {
                self.i += 2;
                self.blocks.pop();
                break;
            }
            if self.i >= self.tokens.len() {
                return self.fail();
            }
            let before = self.out.len();
            self.statement()?;
            let mut parsed = self.out.split_off(before);
            if parsed.is_empty() {
                return self.fail();
            }
            if in_else {
                else_branch.append(&mut parsed);
            } else {
                then_branch.append(&mut parsed);
            }
            if !self.ends_statement() {
                return self.fail();
            }
        }
        self.out.push(Statement::If { condition, then_branch, else_branch, span });
        Ok(())
    }

    pub(super) fn for_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let counter = self.name_ref()?;
        self.expect(Kw::EQ)?;
        let start = self.exp()?;
        self.expect(Kw::TO)?;
        let end = self.exp()?;
        let step = if self.eat(Kw::STEP) { Some(self.exp()?) } else { None };
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        self.blocks.push(Open { what: "FOR loop", closer: "NEXT", span: self.span(keyword) });
        let body = self.collect_until(|ast| ast.is(Kw::NEXT))?;
        self.blocks.pop();
        self.i += 1;
        let mut next_variable = None;
        if let (Some(Tok::Ident), TokenKind::Identifier(name)) = (self.tok_n(0), &self.tokens[self.i].kind) {
            next_variable = Some((name.clone(), self.span(self.i)));
            self.i += 1;
        }
        self.out.push(Statement::For { counter, start, end, step, body, next_variable, span: self.span(keyword) });
        Ok(())
    }

    pub(super) fn while_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let condition = self.exp()?;
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        self.blocks.push(Open { what: "WHILE loop", closer: "WEND", span: self.span(keyword) });
        let body = self.collect_until(|ast| ast.is(Kw::WEND))?;
        self.blocks.pop();
        self.i += 1;
        self.out.push(Statement::While { condition, body, span: self.span(keyword) });
        Ok(())
    }

    pub(super) fn do_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let pre = if self.eat(Kw::WHILE) {
            Some((true, self.exp()?))
        } else if self.eat(Kw::UNTIL) {
            Some((false, self.exp()?))
        } else {
            None
        };
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        self.blocks.push(Open { what: "DO loop", closer: "LOOP", span: self.span(keyword) });
        let body = self.collect_until(|ast| ast.is(Kw::LOOP))?;
        self.blocks.pop();
        self.i += 1;
        let post = if self.eat(Kw::WHILE) {
            Some((true, self.exp()?))
        } else if self.eat(Kw::UNTIL) {
            Some((false, self.exp()?))
        } else {
            None
        };
        if pre.is_some() && post.is_some() {
            return self.fail();
        }
        self.out.push(Statement::Do { pre, post, body, span: self.span(keyword) });
        Ok(())
    }

    pub(super) fn select_statement(&mut self) -> R<()> {
        let keyword = self.take();
        self.expect(Kw::CASE)?;
        let selector = self.exp()?;
        if !(self.eat(Kw::NewLine) || self.separator()) {
            return self.fail();
        }
        self.blocks.push(Open { what: "SELECT CASE", closer: "END SELECT", span: self.span(keyword) });
        let mut arms = Vec::new();
        let mut otherwise = Vec::new();
        let mut otherwise_seen = false;
        loop {
            self.skip_separators();
            if self.at_end_select() {
                self.i += 2;
                self.blocks.pop();
                break;
            }
            // nothing but END SELECT follows CASE ELSE
            if !otherwise_seen {
                self.expect(Kw::CASE)?;
            } else {
                return self.fail();
            }
            let is_else = self.eat(Kw::ELSE);
            otherwise_seen |= is_else;
            let mut matches = Vec::new();
            if !is_else {
                loop {
                    let relation = if self.eat(Kw::IS) { Some(self.rel_op().ok_or(self.i)?) } else { self.rel_op() };
                    if let Some(relation) = relation {
                        self.i += 1;
                        matches.push(CaseItem::Relation(relation, self.exp()?));
                    } else {
                        let lower = self.exp()?;
                        if self.eat(Kw::TO) {
                            matches.push(CaseItem::Range(lower, self.exp()?));
                        } else {
                            matches.push(CaseItem::Value(lower));
                        }
                    }
                    if !self.eat(Kw::Comma) {
                        break;
                    }
                }
            }
            if !(self.eat(Kw::NewLine) || self.separator()) {
                return self.fail();
            }
            let body = self.collect_until(|ast| ast.is(Kw::CASE) || ast.at_end_select())?;
            if is_else {
                otherwise = body;
            } else {
                arms.push((matches, body));
            }
        }
        self.out.push(Statement::Select { selector, arms, otherwise, span: self.span(keyword) });
        Ok(())
    }
}
