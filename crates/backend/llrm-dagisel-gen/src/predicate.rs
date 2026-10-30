//! The C++ boolean expressions of `CheckPatternPredicate`, decided for one CPU.
//!
//! An expression is `!`, `&&`, `||`, parentheses and atoms
//! (`Subtarget->hasX87()`, `shouldOptForSize(MF)`). An atom is true or false
//! for the CPU, or is left to the selector at run time ([`Atom::Dynamic`]).

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Atom(String),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

/// What a CPU says of an atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Yes,
    No,
    /// Decided when the function is selected.
    Dynamic,
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
}

pub fn parse(text: &str) -> Result<Expr, String> {
    let mut parser = Parser { text: text.as_bytes(), at: 0 };
    let expr = parser.or()?;
    parser.skip();
    if parser.at != parser.text.len() {
        return Err(format!("trailing text in `{text}` at {}", parser.at));
    }
    Ok(expr)
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn eat(
        &mut self,
        token: &str,
    ) -> bool {
        self.skip();
        if self.text[self.at..].starts_with(token.as_bytes()) {
            self.at += token.len();
            return true;
        }
        false
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut parts = vec![self.and()?];
        while self.eat("||") {
            parts.push(self.and()?);
        }
        Ok(if parts.len() == 1 { parts.pop().unwrap() } else { Expr::Or(parts) })
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut parts = vec![self.not()?];
        while self.eat("&&") {
            parts.push(self.not()?);
        }
        Ok(if parts.len() == 1 { parts.pop().unwrap() } else { Expr::And(parts) })
    }

    fn not(&mut self) -> Result<Expr, String> {
        self.skip();
        if self.text.get(self.at) == Some(&b'!') && self.text.get(self.at + 1) != Some(&b'=') {
            self.at += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        if self.text.get(self.at) == Some(&b'(') && self.group_is_whole_operand() {
            self.at += 1;
            let inner = self.or()?;
            if !self.eat(")") {
                return Err("unclosed parenthesis".into());
            }
            return Ok(inner);
        }
        self.atom()
    }

    /// Whether the `(` at the cursor wraps a whole operand, not the start of a
    /// call's arguments or of a comparison such as `(a) == b`.
    fn group_is_whole_operand(&self) -> bool {
        let mut depth = 0;
        for (i, &b) in self.text[self.at..].iter().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        let rest = &self.text[self.at + i + 1..];
                        let rest = rest.iter().position(|b| !b.is_ascii_whitespace()).map(|p| &rest[p..]);
                        return rest
                            .is_none_or(|r| r.starts_with(b"&&") || r.starts_with(b"||") || r.starts_with(b")"));
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn atom(&mut self) -> Result<Expr, String> {
        self.skip();
        let start = self.at;
        let mut depth = 0;
        while let Some(&b) = self.text.get(self.at) {
            match b {
                b'(' => depth += 1,
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                b'&' | b'|' if depth == 0 && self.text.get(self.at + 1) == Some(&b) => break,
                _ => {}
            }
            self.at += 1;
        }
        let atom = std::str::from_utf8(&self.text[start..self.at]).unwrap().trim();
        if atom.is_empty() {
            return Err(format!("empty operand at {start}"));
        }
        Ok(Expr::Atom(atom.to_string()))
    }
}

impl Expr {
    /// Replaces each atom the CPU decides by its constant and folds:
    /// `Some(true)`/`Some(false)` when the whole expression is decided,
    /// `None` with the remaining expression otherwise.
    pub fn decide(
        &self,
        verdict: &dyn Fn(&str) -> Result<Verdict, String>,
    ) -> Result<Decided, String> {
        Ok(match self {
            Expr::Atom(a) => match verdict(a)? {
                Verdict::Yes => Decided::Yes,
                Verdict::No => Decided::No,
                Verdict::Dynamic => Decided::Open(self.clone()),
            },
            Expr::Not(e) => match e.decide(verdict)? {
                Decided::Yes => Decided::No,
                Decided::No => Decided::Yes,
                Decided::Open(e) => Decided::Open(Expr::Not(Box::new(e))),
            },
            Expr::And(parts) => {
                let mut open = Vec::new();
                for p in parts {
                    match p.decide(verdict)? {
                        Decided::No => return Ok(Decided::No),
                        Decided::Yes => {}
                        Decided::Open(e) => open.push(e),
                    }
                }
                match open.len() {
                    0 => Decided::Yes,
                    1 => Decided::Open(open.pop().unwrap()),
                    _ => Decided::Open(Expr::And(open)),
                }
            }
            Expr::Or(parts) => {
                let mut open = Vec::new();
                for p in parts {
                    match p.decide(verdict)? {
                        Decided::Yes => return Ok(Decided::Yes),
                        Decided::No => {}
                        Decided::Open(e) => open.push(e),
                    }
                }
                match open.len() {
                    0 => Decided::No,
                    1 => Decided::Open(open.pop().unwrap()),
                    _ => Decided::Open(Expr::Or(open)),
                }
            }
        })
    }

    /// The atoms, with how many expressions use each.
    pub fn atoms<'a>(
        &'a self,
        out: &mut BTreeMap<&'a str, usize>,
    ) {
        match self {
            Expr::Atom(a) => *out.entry(a).or_default() += 1,
            Expr::Not(e) => e.atoms(out),
            Expr::And(p) | Expr::Or(p) => p.iter().for_each(|e| e.atoms(out)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decided {
    Yes,
    No,
    Open(Expr),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(atom: &str) -> Result<Verdict, String> {
        Ok(match atom {
            "Subtarget->hasX87()" => Verdict::Yes,
            "Subtarget->hasCMOV()" => Verdict::No,
            "shouldOptForSize(MF)" => Verdict::Dynamic,
            other => return Err(format!("no verdict for `{other}`")),
        })
    }

    #[test]
    fn parses_and_decides_a_mixed_expression() {
        let e = parse("(Subtarget->hasX87()) && (!Subtarget->hasCMOV() || shouldOptForSize(MF))").unwrap();
        assert_eq!(e.decide(&verdict).unwrap(), Decided::Yes);
        let e = parse("Subtarget->hasCMOV() && shouldOptForSize(MF)").unwrap();
        assert_eq!(e.decide(&verdict).unwrap(), Decided::No);
        let e = parse("Subtarget->hasX87() && shouldOptForSize(MF)").unwrap();
        assert_eq!(e.decide(&verdict).unwrap(), Decided::Open(Expr::Atom("shouldOptForSize(MF)".into())));
    }

    #[test]
    fn calls_with_arguments_and_comparisons_stay_atoms() {
        let e = parse("MF->getFunction().getCallingConv() != CallingConv::HiPE && !Subtarget->hasCMOV()").unwrap();
        let mut atoms = BTreeMap::new();
        e.atoms(&mut atoms);
        assert_eq!(
            atoms.keys().copied().collect::<Vec<_>>(),
            ["MF->getFunction().getCallingConv() != CallingConv::HiPE", "Subtarget->hasCMOV()"]
        );
    }

    #[test]
    fn an_atom_without_a_verdict_is_an_error() {
        assert!(parse("Subtarget->hasFoo()").unwrap().decide(&verdict).is_err());
    }
}
