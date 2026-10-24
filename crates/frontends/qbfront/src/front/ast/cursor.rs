//! The token cursor: looking ahead, eating, failing.

use super::*;

impl Ast<'_> {
    pub(super) fn kw_n(
        &self,
        n: usize,
    ) -> Option<Kw> {
        match self.kinds.get(self.i + n) {
            Some(Tok::Kw(k)) => Some(*k),
            _ => None,
        }
    }

    pub(super) fn kw(&self) -> Option<Kw> {
        self.kw_n(0)
    }

    pub(super) fn is(
        &self,
        k: Kw,
    ) -> bool {
        self.kw() == Some(k)
    }

    pub(super) fn is_n(
        &self,
        n: usize,
        k: Kw,
    ) -> bool {
        self.kw_n(n) == Some(k)
    }

    pub(super) fn tok_n(
        &self,
        n: usize,
    ) -> Option<&Tok> {
        self.kinds.get(self.i + n)
    }

    pub(super) fn period_n(
        &self,
        n: usize,
    ) -> bool {
        matches!(self.tok_n(n), Some(Tok::Period))
    }

    pub(super) fn eat(
        &mut self,
        k: Kw,
    ) -> bool {
        let found = self.is(k);
        self.i += usize::from(found);
        found
    }

    pub(super) fn expect(
        &mut self,
        k: Kw,
    ) -> R<()> {
        if self.eat(k) {
            Ok(())
        } else {
            self.expected = Some((self.i, k));
            Err(self.i)
        }
    }

    pub(super) fn fail<T>(&self) -> R<T> {
        Err(self.i)
    }

    /// `fail`, with BC's name for the reason.
    pub(super) fn fail_because<T>(
        &mut self,
        reason: &'static str,
    ) -> R<T> {
        self.reason = Some(reason);
        Err(self.i)
    }

    /// Whether the token at `at` begins its line, after at most a line number
    /// or a label.
    pub(super) fn first_on_line(
        &self,
        at: usize,
    ) -> bool {
        let start = (0..at).rev().find(|&index| self.kinds[index] == Tok::Kw(Kw::NewLine)).map_or(0, |index| index + 1);
        match &self.kinds[start..at] {
            [] | [Tok::Int] => true,
            [Tok::Ident, Tok::Kw(Kw::Colon)] | [Tok::Int, Tok::Kw(Kw::Colon)] => true,
            _ => false,
        }
    }

    pub(super) fn span(
        &self,
        at: usize,
    ) -> Span {
        self.tokens[at].span
    }

    /// The span of the token last read.
    pub(super) fn previous(&self) -> Span {
        self.tokens[self.i - 1].span
    }

    pub(super) fn previous_end(&self) -> usize {
        self.tokens.get(self.i.wrapping_sub(1)).map_or(0, |token| token.span.end)
    }

    /// Take the reserved word at the cursor: its token index.
    pub(super) fn take(&mut self) -> usize {
        self.i += 1;
        self.i - 1
    }

    pub(super) fn lab_ln(&mut self) -> R<(String, Span)> {
        match &self.tokens.get(self.i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => {
                let out = (name.clone(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            Some(TokenKind::Integer(value, None)) if (0..=65_529).contains(value) => {
                let out = (value.to_string(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            // a line number may carry a fraction: `GOTO 1.5`
            Some(TokenKind::Real(text, None)) if !text.contains('E') => {
                let out = (text.clone(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            _ => self.fail(),
        }
    }

    // ---- expressions
    // --------------------------------------------------------------------
}
