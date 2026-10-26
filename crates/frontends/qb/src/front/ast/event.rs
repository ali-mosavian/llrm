//! ON ... event trapping and the switch statements.

use super::*;

impl Ast<'_> {
    pub(super) fn event(
        &mut self,
        device: &str,
        number: Option<Expr>,
        verb: &str,
        label: Option<(String, Span)>,
        span: Span,
    ) {
        let arguments = vec![
            Expr::Name(device.into(), span),
            number.unwrap_or(Expr::Omitted(span)),
            Expr::Name(verb.into(), span),
            label.map_or(Expr::Omitted(span), |(name, at)| Expr::Name(name, at)),
        ];
        self.runtime("EVENT", arguments, span);
    }

    pub(super) fn on(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        let local = self.is(Kw::LOCAL);
        if (self.is(Kw::ERROR) && (self.is_n(1, Kw::GOTO) || self.is_n(1, Kw::RESUME)))
            || (local && self.is_n(1, Kw::ERROR))
        {
            self.i += 1 + usize::from(local);
            if self.eat(Kw::GOTO) {
                let label =
                    if matches!(
                        self.tokens.get(self.i).map(|token| &token.kind),
                        Some(TokenKind::Integer(0, None))
                    ) {
                        self.i += 1;
                        "0".to_owned()
                    } else {
                        self.lab_ln()?.0
                    };
                self.out.push(Statement::OnError { label, local, span });
                return Ok(());
            }
            self.expect(Kw::RESUME)?;
            self.expect(Kw::NEXT)?;
            self.out.push(Statement::OnErrorResumeNext { local, span });
            return Ok(());
        }
        let named = match self.kw() {
            Some(Kw::KEY) => Some("KEY"),
            Some(Kw::COM) => Some("COM"),
            Some(Kw::SIGNAL) => Some("SIGNAL"),
            Some(Kw::TIMER) if self.is_n(1, Kw::LParen) => Some("TIMER"),
            Some(Kw::PEN) if self.is_n(1, Kw::GOSUB) => Some("PEN"),
            Some(Kw::UEVENT) if self.is_n(1, Kw::GOSUB) => Some("UEVENT"),
            _ => None,
        };
        // ON PEN and ON UEVENT take no argument; elsewhere they are not a
        // selector either
        if named.is_none() && matches!(self.kw(), Some(Kw::PEN | Kw::UEVENT)) {
            return self.fail();
        }
        if let Some(device) = named {
            self.i += 1;
            let number = if self.eat(Kw::LParen) {
                let number = self.exp()?;
                self.expect(Kw::RParen)?;
                Some(number)
            } else {
                None
            };
            self.expect(Kw::GOSUB)?;
            let label = self.lab_ln()?;
            self.event(device, number, "GOSUB", Some(label), span);
            return Ok(());
        }
        let selector = self.imp(First::NoErr)?;
        let gosub = match self.kw() {
            Some(Kw::GOTO) => false,
            Some(Kw::GOSUB) => true,
            _ => return self.fail(),
        };
        self.i += 1;
        let mut labels = vec![self.lab_ln()?];
        while self.eat(Kw::Comma) {
            labels.push(self.lab_ln()?);
        }
        // ON PLAY(n) GOSUB and ON STRIG(n) GOSUB: a trap, whose "selector" is
        // the device call
        if gosub {
            if let Expr::Apply { name, arguments, .. } = &selector {
                if matches!(name.as_str(), "PLAY" | "STRIG") && arguments.len() == 1 && labels.len() == 1 {
                    let device = if name == "PLAY" { "PLAY" } else { "STRIG" };
                    self.event(device, Some(arguments[0].clone()), "GOSUB", labels.pop(), span);
                    return Ok(());
                }
            }
        }
        let number = |value: i64| Expr::Literal(Literal::Integer(value, TypeName::Integer), span);
        let arms = labels
            .into_iter()
            .enumerate()
            .map(|(index, (label, label_span))| {
                let jump = if gosub {
                    Statement::Call {
                        name: "GOSUB".into(),
                        arguments: vec![Expr::Name(label, label_span)],
                        explicit: false,
                        span,
                    }
                } else {
                    Statement::Goto(label, span)
                };
                (vec![CaseItem::Value(number(index as i64 + 1))], vec![jump])
            })
            .collect();
        self.out.push(Statement::Select {
            selector: Expr::Apply { name: ON_SELECTOR.into(), arguments: vec![selector], span },
            arms,
            otherwise: Vec::new(),
            span,
        });
        Ok(())
    }

    pub(super) fn switch_verb(&mut self) -> R<&'static str> {
        let verb = match self.kw() {
            Some(Kw::ON) => "ON",
            Some(Kw::OFF) => "OFF",
            Some(Kw::STOP) => "STOP",
            _ => return self.fail(),
        };
        self.i += 1;
        Ok(verb)
    }

    pub(super) fn simple_statement(
        &mut self,
        k: Kw,
    ) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        match k {
            Kw::BEEP => self.runtime("BEEP", Vec::new(), span),
            // BC warns "Statement Ignored" and skips the rest of the line
            Kw::TRON | Kw::TROFF => {
                while self.i < self.tokens.len() && !self.is(Kw::NewLine) {
                    self.i += 1;
                }
                self.runtime(if k == Kw::TRON { "TRON" } else { "TROFF" }, Vec::new(), span);
            }
            Kw::SYSTEM => {
                let arguments = if self.dialect.exit_codes() { self.optional_exp()? } else { Vec::new() };
                self.runtime("SYSTEM", arguments, span)
            }
            Kw::STOP => self.implicit_call("STOP", Vec::new(), span),
            Kw::RESET => self.implicit_call("RESET", Vec::new(), span),
            Kw::ENDIF => self.runtime("ENDIF", Vec::new(), span),
            Kw::CLS | Kw::SLEEP | Kw::FILES | Kw::RUN | Kw::SHELL => {
                let arguments = self.optional_exp()?;
                let name = match k {
                    Kw::CLS => "CLS",
                    Kw::SLEEP => "SLEEP",
                    Kw::FILES => "FILES",
                    Kw::SHELL => "SHELL",
                    _ => "RUN",
                };
                self.runtime(name, arguments, span);
            }
            Kw::RANDOMIZE => {
                let arguments = self.optional_exp()?;
                self.implicit_call("RANDOMIZE", arguments, span);
            }
            Kw::COLOR | Kw::LOCATE | Kw::SCREEN => {
                let arguments = self.argument_list(false)?;
                let name = match k {
                    Kw::COLOR => "COLOR",
                    Kw::LOCATE => "LOCATE",
                    _ => "SCREEN",
                };
                self.runtime(name, arguments, span);
            }
            Kw::CLEAR => {
                let arguments = self.argument_list(true)?;
                self.implicit_call("CLEAR", arguments, span);
            }
            Kw::ENVIRON | Kw::KILL | Kw::MKDIR | Kw::CHDIR | Kw::RMDIR => {
                let argument = self.exp()?;
                let name = match k {
                    Kw::ENVIRON => "ENVIRON",
                    Kw::KILL => "KILL",
                    Kw::MKDIR => "MKDIR",
                    Kw::CHDIR => "CHDIR",
                    _ => "RMDIR",
                };
                self.implicit_call(name, vec![argument], span);
            }
            Kw::BLOAD => {
                let mut arguments = vec![self.exp()?];
                if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                }
                self.implicit_call("BLOAD", arguments, span);
            }
            Kw::BSAVE => {
                let mut arguments = vec![self.exp()?];
                for _ in 0..2 {
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                }
                self.implicit_call("BSAVE", arguments, span);
            }
            Kw::CHAIN | Kw::DRAW | Kw::ERROR => {
                let argument = self.exp()?;
                let name = match k {
                    Kw::CHAIN => "CHAIN",
                    Kw::DRAW => "DRAW",
                    _ => "ERROR",
                };
                self.runtime(name, vec![argument], span);
            }
            Kw::PLAY => {
                if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
                    let verb = self.switch_verb()?;
                    self.event("PLAY", None, verb, None, span);
                } else {
                    let argument = self.exp()?;
                    self.runtime("PLAY", vec![argument], span);
                }
            }
            Kw::SOUND | Kw::OUT | Kw::POKE | Kw::PCOPY | Kw::NAME => {
                let a = self.exp()?;
                if k == Kw::NAME {
                    self.expect(Kw::AS)?;
                } else {
                    self.expect(Kw::Comma)?;
                }
                let b = self.exp()?;
                let name = match k {
                    Kw::SOUND => "SOUND",
                    Kw::OUT => "OUT",
                    Kw::POKE => "POKE",
                    Kw::PCOPY => "PCOPY",
                    _ => "NAME",
                };
                self.runtime(name, vec![a, b], span);
            }
            Kw::WAIT => {
                let mut arguments = vec![self.exp()?];
                self.expect(Kw::Comma)?;
                arguments.push(self.exp()?);
                if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                }
                self.runtime("WAIT", arguments, span);
            }
            Kw::WIDTH => {
                let mut arguments = Vec::new();
                let mut name = "WIDTH";
                if self.eat(Kw::Lbs) {
                    name = "WIDTH_FILE";
                    arguments.push(self.exp()?);
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                } else if self.eat(Kw::LPRINT) {
                    name = "WIDTH_LPRINT";
                    arguments.push(self.exp()?);
                } else if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                } else {
                    arguments.push(self.exp()?);
                    if self.eat(Kw::Comma) {
                        arguments.push(self.exp()?);
                    }
                }
                self.runtime(name, arguments, span);
            }
            Kw::PALETTE => {
                let mut arguments = Vec::new();
                if self.eat(Kw::USING) {
                    arguments.push(self.transfer_array()?);
                    self.runtime("PALETTE_USING", arguments, span);
                    return Ok(());
                } else if self.starts_exp() {
                    arguments.push(self.exp()?);
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                }
                self.runtime("PALETTE", arguments, span);
            }
            Kw::PSET | Kw::PRESET | Kw::PAINT | Kw::CIRCLE => {
                let (x, y, step) = self.coordinates()?;
                let mut arguments = vec![Self::stepped(x, step), y];
                if k == Kw::CIRCLE {
                    let comma = self.expect(Kw::Comma);
                    comma?;
                    arguments.push(self.exp()?);
                }
                arguments.extend(self.rest(k == Kw::PAINT)?);
                let name = match k {
                    Kw::PSET => "PSET",
                    Kw::PRESET => "PRESET",
                    Kw::PAINT => "PAINT",
                    _ => "CIRCLE",
                };
                self.runtime(name, arguments, span);
            }
            Kw::VIEW => {
                if self.eat(Kw::PRINT) {
                    let mut arguments = Vec::new();
                    if self.starts_exp() {
                        arguments.push(self.exp()?);
                        self.expect(Kw::TO)?;
                        arguments.push(self.exp()?);
                    }
                    self.runtime("VIEW_PRINT", arguments, span);
                } else {
                    let screen = self.eat(Kw::SCREEN);
                    let mut arguments = Vec::new();
                    if self.is(Kw::LParen) {
                        let (x, y, _) = self.coordinates()?;
                        self.expect(Kw::Minus)?;
                        let (x2, y2, _) = self.coordinates()?;
                        arguments.extend([x, y, x2, y2]);
                        arguments.extend(self.rest(false)?);
                    }
                    self.runtime(if screen { "VIEW_SCREEN" } else { "VIEW" }, arguments, span);
                }
            }
            Kw::WINDOW => {
                let screen = self.eat(Kw::SCREEN);
                let mut arguments = Vec::new();
                if self.is(Kw::LParen) {
                    let (x, y, _) = self.coordinates()?;
                    self.expect(Kw::Minus)?;
                    let (x2, y2, _) = self.coordinates()?;
                    arguments.extend([x, y, x2, y2]);
                }
                self.runtime(if screen { "WINDOW_SCREEN" } else { "WINDOW" }, arguments, span);
            }
            Kw::COM | Kw::SIGNAL | Kw::STRIG | Kw::PEN | Kw::TIMER | Kw::UEVENT => {
                let number = if matches!(k, Kw::COM | Kw::SIGNAL | Kw::STRIG) && self.is(Kw::LParen) {
                    self.i += 1;
                    let number = self.exp()?;
                    self.expect(Kw::RParen)?;
                    Some(number)
                } else {
                    None
                };
                let verb = self.switch_verb()?;
                let device = match k {
                    Kw::COM => "COM",
                    Kw::SIGNAL => "SIGNAL",
                    Kw::STRIG => "STRIG",
                    Kw::PEN => "PEN",
                    Kw::TIMER => "TIMER",
                    _ => "UEVENT",
                };
                self.event(device, number, verb, None, span);
            }
            Kw::KEY => {
                if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::LIST)) {
                    let at = self.take();
                    let word = if self.kinds[at] == Tok::Kw(Kw::ON) {
                        "ON"
                    } else if self.kinds[at] == Tok::Kw(Kw::OFF) {
                        "OFF"
                    } else {
                        "LIST"
                    };
                    self.runtime("KEY", vec![Expr::Name(word.into(), self.span(at))], span);
                } else {
                    let number = self.exp()?;
                    if matches!(self.kw(), Some(Kw::ON | Kw::OFF | Kw::STOP)) {
                        let verb = self.switch_verb()?;
                        self.event("KEY", Some(number), verb, None, span);
                    } else {
                        self.expect(Kw::Comma)?;
                        let text = self.exp()?;
                        self.runtime("KEY", vec![number, text], span);
                    }
                }
            }
            Kw::LOCK | Kw::UNLOCK => {
                // LOCK [#]file [, record | [start] TO end]: [file], [file,
                // record] or [file, start|omitted, end]
                self.eat(Kw::Lbs);
                let mut arguments = vec![self.exp()?];
                if self.eat(Kw::Comma) {
                    if self.is(Kw::TO) {
                        arguments.push(Expr::Omitted(span));
                    } else {
                        arguments.push(self.exp()?);
                    }
                    if self.eat(Kw::TO) {
                        arguments.push(self.exp()?);
                    }
                }
                self.runtime(if k == Kw::LOCK { "LOCK" } else { "UNLOCK" }, arguments, span);
            }
            Kw::IOCTL => {
                self.eat(Kw::Lbs);
                let file = self.exp()?;
                self.expect(Kw::Comma)?;
                let text = self.exp()?;
                self.runtime("IOCTL", vec![file, text], span);
            }
            Kw::FIELD => {
                // FIELD [#]file, width AS variable [, width AS variable]...:
                // [file, width, variable, ...]
                self.eat(Kw::Lbs);
                let mut arguments = vec![self.exp()?];
                loop {
                    self.expect(Kw::Comma)?;
                    arguments.push(self.exp()?);
                    self.expect(Kw::AS)?;
                    arguments.push(self.name_ref()?);
                    if !self.is(Kw::Comma) {
                        break;
                    }
                }
                self.runtime("FIELD", arguments, span);
            }
            _ => return Err(keyword),
        }
        Ok(())
    }
}
