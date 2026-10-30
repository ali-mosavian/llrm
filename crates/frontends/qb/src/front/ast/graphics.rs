//! The graphics statements' coordinates and trailing arguments.

use super::*;

impl Ast<'_> {
    pub(super) fn coordinates(&mut self) -> R<(Expr, Expr, bool)> {
        let step = self.eat(Kw::STEP);
        self.expect(Kw::LParen)?;
        let x = self.exp()?;
        self.expect(Kw::Comma)?;
        let y = self.exp()?;
        self.expect(Kw::RParen)?;
        Ok((x, y, step))
    }

    /// The x of a STEP-relative pair is wrapped, as the table-driven parser
    /// does.
    pub(super) fn stepped(
        x: Expr,
        step: bool,
    ) -> Expr {
        if step {
            let span = x.span();
            Expr::Apply { name: STEP_MARK.into(), arguments: vec![x], span }
        } else {
            x
        }
    }

    /// `, [exp]`s after the leading arguments, an empty slot being Omitted at
    /// its comma.
    pub(super) fn rest(
        &mut self,
        trailing_ok: bool,
    ) -> R<Vec<Expr>> {
        let mut out = Vec::new();
        while self.is(Kw::Comma) {
            let comma = self.take();
            out.push(if self.starts_exp() { self.exp()? } else { Expr::Omitted(self.span(comma)) });
        }
        if !trailing_ok && matches!(out.last(), Some(Expr::Omitted(_))) {
            return self.fail();
        }
        Ok(out)
    }

    pub(super) fn line(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        if self.eat(Kw::INPUT) {
            let (file, prompt, _, keep_cursor) = self.input_prefix()?;
            let destination = self.name_ref()?;
            self.out.push(Statement::LineInput { file, prompt, keep_cursor, destination, span });
            return Ok(());
        }
        let (mut arguments, omitted_first) = if self.is(Kw::Minus) {
            let zero = Expr::Literal(Literal::Integer(0, TypeName::Integer), span);
            (vec![Self::stepped(zero.clone(), true), zero], true)
        } else {
            let (x, y, step) = self.coordinates()?;
            (vec![Self::stepped(x, step), y], false)
        };
        let _ = omitted_first;
        self.expect(Kw::Minus)?;
        let (x, y, step) = self.coordinates()?;
        arguments.push(Self::stepped(x, step));
        arguments.push(y);
        let mut color = None;
        let mut mode = None;
        let mut style = None;
        // BC takes `,color,` and `,,` (nothing after the second comma); VBDOS
        // does not (checker)
        let mut trailing = false;
        if self.eat(Kw::Comma) {
            if self.starts_exp() && !self.at_box_word() {
                color = Some(self.exp()?);
            }
            if self.eat(Kw::Comma) {
                if self.at_box_word() {
                    let at = self.i;
                    let (name, _) = self.identifier()?;
                    mode = Some(Expr::Name(name, self.span(at)));
                }
                if self.eat(Kw::Comma) {
                    if !self.starts_exp() {
                        return self.fail();
                    }
                    style = Some(self.exp()?);
                } else {
                    trailing = mode.is_none();
                }
            } else if color.is_none() {
                return self.fail();
            }
        }
        for slot in [color, mode, style] {
            arguments.push(slot.unwrap_or(Expr::Omitted(span)));
        }
        while arguments.len() > if trailing { 6 } else { 4 } && matches!(arguments.last(), Some(Expr::Omitted(_))) {
            arguments.pop();
        }
        self.runtime("LINE", arguments, span);
        Ok(())
    }

    pub(super) fn at_box_word(&self) -> bool {
        match &self.tokens.get(self.i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => name == "B" || name == "BF",
            _ => false,
        }
    }
}
