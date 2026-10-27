//! PRINT, INPUT, OPEN, GET and PUT.

use super::*;

impl Ast<'_> {
    /// A word that can begin a PRINT item that follows another without a
    /// separator.
    pub(super) fn starts_joined(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str | Tok::Format | Tok::Ident | Tok::FnWord) => true,
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                c & (FN0 | FN_WORD | CTX_WORD) != 0
                    || (c & OBJ_WORD != 0 && self.period_n(1))
                    || (c & (FN1 | FN2 | FN23 | FN_BOUND | FN_REF) != 0 && self.is_n(1, Kw::LParen))
                    || matches!(k, Kw::ERROR | Kw::RND)
            }
            _ => false,
        }
    }

    pub(super) fn print(
        &mut self,
        k: Kw,
    ) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let kind = match k {
            Kw::LPRINT => PrintKind::Lprint,
            Kw::WRITE => PrintKind::Write,
            _ => PrintKind::Print,
        };
        let file = if k != Kw::LPRINT && self.eat(Kw::Lbs) {
            let channel = self.exp()?;
            self.expect(Kw::Comma)?;
            Some(channel)
        } else {
            None
        };
        let mut using = None;
        let mut items: Vec<PrintItem> = Vec::new();
        while !(self.ends_statement() || self.is(Kw::ELSE)) {
            if k != Kw::WRITE && using.is_none() && self.eat(Kw::USING) {
                using = Some(self.exp()?);
                // `USING fmt;` must be followed by a value (a comma is
                // "Semicolon missing")
                if !self.eat(Kw::SColon) || !self.starts_exp() {
                    return self.fail();
                }
                continue;
            }
            if matches!(self.kw(), Some(Kw::TAB | Kw::SPC)) && self.is_n(1, Kw::LParen) {
                let name = if self.is(Kw::TAB) { "TAB" } else { "SPC" };
                self.i += 2;
                let argument = self.exp()?;
                self.expect(Kw::RParen)?;
                let arg_span = argument.span();
                // no separator: the next item follows directly
                let separator = if self.eat(Kw::Comma) {
                    PrintSeparator::Comma
                } else {
                    self.eat(Kw::SColon);
                    PrintSeparator::Semicolon
                };
                items.push(PrintItem {
                    value: Expr::Apply { name: name.into(), arguments: vec![argument], span: arg_span },
                    separator,
                });
                continue;
            }
            if self.is(Kw::Comma) || self.is(Kw::SColon) {
                let at = self.take();
                let separator = if self.kinds[at] == Tok::Kw(Kw::Comma) {
                    PrintSeparator::Comma
                } else {
                    PrintSeparator::Semicolon
                };
                items.push(PrintItem { value: Expr::Omitted(self.span(at)), separator });
                continue;
            }
            if !self.starts_exp() {
                return self.fail();
            }
            let mut value = self.exp()?;
            while self.starts_joined() {
                items.push(PrintItem { value, separator: PrintSeparator::Semicolon });
                value = self.imp(First::Joined)?;
            }
            let separator = if self.eat(Kw::Comma) {
                PrintSeparator::Comma
            } else if self.eat(Kw::SColon) {
                PrintSeparator::Semicolon
            } else {
                PrintSeparator::End
            };
            items.push(PrintItem { value, separator });
        }
        // WRITE takes values between commas: none may be left hanging
        if kind == PrintKind::Write && items.last().is_some_and(|item| item.separator == PrintSeparator::Comma) {
            return self.fail();
        }
        self.out.push(Statement::Print { kind, file, using, items, span });
        Ok(())
    }

    /// `[#file,] [prompt ;|,]`: the file, the prompt and whether its separator
    /// was a comma, and whether the statement began with `;`.
    pub(super) fn input_prefix(&mut self) -> R<(Option<Expr>, Option<Expr>, bool, bool)> {
        let mut file = None;
        let mut keep_cursor = false;
        if self.eat(Kw::Lbs) {
            file = Some(self.exp()?);
            self.expect(Kw::Comma)?;
            return Ok((file, None, false, false));
        }
        if self.eat(Kw::SColon) {
            keep_cursor = true;
        }
        let mut prompt = None;
        let mut suppress = false;
        if matches!(self.tok_n(0), Some(Tok::Str)) {
            prompt = Some(self.literal()?);
            if self.eat(Kw::Comma) {
                suppress = true;
            } else {
                self.expect(Kw::SColon)?;
            }
        }
        Ok((file, prompt, suppress, keep_cursor))
    }

    pub(super) fn input(&mut self) -> R<()> {
        let keyword = self.take();
        let (file, prompt, suppress_question_mark, keep_cursor) = self.input_prefix()?;
        let mut destinations = vec![self.name_ref()?];
        while self.eat(Kw::Comma) {
            destinations.push(self.name_ref()?);
        }
        self.out.push(Statement::Input {
            file,
            prompt,
            suppress_question_mark,
            keep_cursor,
            destinations,
            span: self.span(keyword),
        });
        Ok(())
    }

    pub(super) fn open(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let first = self.exp()?;
        if self.eat(Kw::Comma) {
            // OPEN mode$, [#]file, path$[, record length]
            self.eat(Kw::Lbs);
            let file = self.exp()?;
            self.expect(Kw::Comma)?;
            let path = self.exp()?;
            let record_length = if self.eat(Kw::Comma) { Some(self.exp()?) } else { None };
            self.out.push(Statement::OpenClassic { mode: first, file, path, record_length, span });
            return Ok(());
        }
        let mut mode_written = false;
        let mut mode = FileMode::Random;
        // PDS and VBDOS: OPEN database FOR ISAM RecordType "table" AS [#]file
        if self.is(Kw::FOR)
            && self.dialect_has_isam()
            && matches!(
                self.tokens.get(self.i + 1).map(|t| &t.kind),
                Some(TokenKind::Identifier(name)) if name == "ISAM"
            )
        {
            self.i += 2;
            let (record_type, _) = self.identifier()?;
            let table = self.exp()?;
            self.expect(Kw::AS)?;
            self.eat(Kw::Lbs);
            let file = self.exp()?;
            self.out.push(Statement::OpenIsam { database: first, record_type, table, file, span });
            return Ok(());
        }
        if self.eat(Kw::FOR) {
            mode_written = true;
            mode = match self.kw() {
                Some(Kw::APPEND) => FileMode::Append,
                Some(Kw::INPUT) => FileMode::Input,
                Some(Kw::OUTPUT) => FileMode::Output,
                Some(Kw::BINARY) => FileMode::Binary,
                Some(Kw::RANDOM) => FileMode::Random,
                _ => return self.fail(),
            };
            self.i += 1;
        }
        let access = if self.eat(Kw::ACCESS) {
            Some(if self.eat(Kw::READ) {
                if self.eat(Kw::WRITE) { FileAccess::ReadWrite } else { FileAccess::Read }
            } else {
                self.expect(Kw::WRITE)?;
                FileAccess::Write
            })
        } else {
            None
        };
        let lock = if self.eat(Kw::SHARED) {
            Some(FileLock::Shared)
        } else if self.eat(Kw::LOCK) {
            Some(if self.eat(Kw::READ) {
                if self.eat(Kw::WRITE) { FileLock::ReadWrite } else { FileLock::Read }
            } else {
                self.expect(Kw::WRITE)?;
                FileLock::Write
            })
        } else {
            None
        };
        self.expect(Kw::AS)?;
        self.eat(Kw::Lbs);
        let file = self.exp()?;
        let record_length = if self.is(Kw::LEN) && self.is_n(1, Kw::EQ) {
            self.i += 2;
            Some(self.exp()?)
        } else {
            None
        };
        self.out.push(Statement::Open { path: first, mode, file, mode_written, access, lock, record_length, span });
        Ok(())
    }

    /// The array of a graphics GET/PUT: `a%` or `a%(i)`, never `a%()`.
    pub(super) fn transfer_array(&mut self) -> R<Expr> {
        let target = self.name_ref()?;
        if matches!(&target, Expr::Apply { arguments, .. } if arguments.is_empty()) {
            return self.fail();
        }
        Ok(target)
    }

    pub(super) fn get_put(
        &mut self,
        k: Kw,
    ) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let graphic = self.is(Kw::STEP)
            || self.is(Kw::LParen) && {
                let save = self.i;
                let found = self.coordinates().is_ok();
                self.i = save;
                found
            };
        if graphic {
            let (x, y, step) = self.coordinates()?;
            let mut arguments = vec![Self::stepped(x, step), y];
            if k == Kw::GET {
                self.expect(Kw::Minus)?;
                let (x, y, step) = self.coordinates()?;
                arguments.push(Self::stepped(x, step));
                arguments.push(y);
                self.expect(Kw::Comma)?;
                arguments.push(self.transfer_array()?);
                self.runtime("GET", arguments, span);
            } else {
                self.expect(Kw::Comma)?;
                arguments.push(self.transfer_array()?);
                if self.eat(Kw::Comma) {
                    let at = self.i;
                    let mode = match self.kw() {
                        Some(Kw::AND) => "AND",
                        Some(Kw::OR) => "OR",
                        Some(Kw::PRESET) => "PRESET",
                        Some(Kw::PSET) => "PSET",
                        Some(Kw::XOR) => "XOR",
                        _ => return self.fail(),
                    };
                    self.i += 1;
                    arguments.push(Expr::Name(mode.into(), self.span(at)));
                }
                self.runtime("PUT", arguments, span);
            }
            return Ok(());
        }
        self.eat(Kw::Lbs);
        let file = self.exp()?;
        let mut position = None;
        let mut target = None;
        if self.eat(Kw::Comma) {
            if self.starts_exp() {
                position = Some(self.exp()?);
            } else if !self.is(Kw::Comma) {
                return self.fail();
            }
            if self.eat(Kw::Comma) {
                target = Some(self.exp()?);
            }
        }
        self.out.push(Statement::FileTransfer { write: k == Kw::PUT, file, position, target, span });
        Ok(())
    }
}
